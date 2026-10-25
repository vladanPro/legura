use axonyx_runtime::server::{AxFileStorage, AxIncomingFile};
use image::ImageEncoder;
use legura::{
    media_library::MediaLibraryStorage, media_upload::MEDIA_CAPABILITY, storage::StoreId,
};
use rusqlite::Connection;
use std::{fs, path::PathBuf, process::Command};

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let mut random = [0; 16];
        getrandom::getrandom(&mut random).unwrap();
        let root = std::env::temp_dir().canonicalize().unwrap().join(format!(
            "legura-media-check-{:032x}",
            u128::from_le_bytes(random)
        ));
        fs::create_dir(&root).unwrap();
        fs::create_dir(root.join("objects")).unwrap();
        let database = Connection::open(root.join("library.db")).unwrap();
        database
            .execute_batch(include_str!(
                "../db/migrations/20261018000000_004_media/up.sql"
            ))
            .unwrap();
        Self(root)
    }
    fn check(&self, database: &str, store: &str) -> std::process::Output {
        Command::new(env!("CARGO_BIN_EXE_legura-maintenance"))
            .arg("media-check")
            .arg(self.0.join(database))
            .arg(store)
            .arg(self.0.join("objects"))
            .output()
            .unwrap()
    }
    fn upload(&self) -> axonyx_runtime::server::AxFileRef {
        let library = MediaLibraryStorage::open(
            &self.0.join("objects"),
            StoreId::parse("local-media").unwrap(),
            &self.0.join("library.db"),
        )
        .unwrap();
        let mut bytes = Vec::new();
        image::codecs::png::PngEncoder::new(&mut bytes)
            .write_image(&[50, 70, 90, 255], 1, 1, image::ExtendedColorType::Rgba8)
            .unwrap();
        library
            .save_file(
                MEDIA_CAPABILITY,
                &AxIncomingFile {
                    field_name: "image".into(),
                    file_name: "not-trusted.svg".into(),
                    content_type: Some("text/html".into()),
                    bytes,
                },
            )
            .unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let root = self.0.canonicalize().unwrap();
        assert_eq!(
            root.parent().unwrap(),
            std::env::temp_dir().canonicalize().unwrap()
        );
        assert!(root
            .file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("legura-media-check-"));
        fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn real_upload_is_audited_read_only_and_corrupt_or_missing_bytes_fail() {
    let fixture = Fixture::new();
    let empty = fixture.check("library.db", "local-media");
    assert!(empty.status.success());
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&empty.stdout).unwrap(),
        serde_json::json!({"objects":0,"bytes":0})
    );
    let saved = fixture.upload();
    let database = fs::read(fixture.0.join("library.db")).unwrap();
    let object_path = fixture.0.join("objects").join(&saved.id);
    let object = fs::read(&object_path).unwrap();
    // Orphans are not repaired, deleted or counted as referenced media.
    let orphan = fixture.0.join("objects").join("b".repeat(64));
    fs::write(&orphan, b"unreferenced").unwrap();
    let valid = fixture.check("library.db", "local-media");
    assert!(
        valid.status.success(),
        "{}",
        String::from_utf8_lossy(&valid.stderr)
    );
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&valid.stdout).unwrap(),
        serde_json::json!({"objects":1,"bytes":saved.size})
    );
    assert_eq!(fs::read(fixture.0.join("library.db")).unwrap(), database);
    assert_eq!(fs::read(&object_path).unwrap(), object);
    assert_eq!(fs::read(&orphan).unwrap(), b"unreferenced");
    assert!(!fixture.check("library.db", "other-store").status.success());
    let mut corrupted = object.clone();
    corrupted[0] ^= 1;
    fs::write(&object_path, &corrupted).unwrap();
    for expected in ["corrupted", "missing"] {
        if expected == "missing" {
            fs::remove_file(&object_path).unwrap();
        }
        let rejected = fixture.check("library.db", "local-media");
        assert_eq!(rejected.status.code(), Some(1));
        assert!(rejected.stdout.is_empty());
        let error = String::from_utf8_lossy(&rejected.stderr);
        assert!(!error.contains(&saved.id));
        assert!(!error.contains("library.db"));
        assert_eq!(fs::read(fixture.0.join("library.db")).unwrap(), database);
    }
    assert!(!fixture.check("absent.db", "local-media").status.success());
    assert!(!fixture.0.join("absent.db").exists());
}

#[cfg(unix)]
#[test]
fn referenced_symlink_is_refused_without_changing_external_file() {
    let fixture = Fixture::new();
    let saved = fixture.upload();
    let object = fixture.0.join("objects").join(saved.id);
    let external = fixture.0.join("outside.png");
    let bytes = fs::read(&object).unwrap();
    fs::write(&external, &bytes).unwrap();
    fs::remove_file(&object).unwrap();
    std::os::unix::fs::symlink(&external, &object).unwrap();
    assert!(!fixture.check("library.db", "local-media").status.success());
    assert_eq!(fs::read(&external).unwrap(), bytes);
    assert!(fs::symlink_metadata(&object)
        .unwrap()
        .file_type()
        .is_symlink());
}
