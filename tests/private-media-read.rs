use axonyx_runtime::server::{AxFileStorage, AxIncomingFile};
use image::ImageEncoder;
use legura::media_library::MediaLibraryStorage;
use legura::media_read::{read_for_administrator, MediaReadError};
use legura::media_upload::MEDIA_CAPABILITY;
use legura::storage::{LocalStorage, StoreId, MAX_OBJECT_BYTES};
use rusqlite::Connection;
use std::{fs, path::PathBuf};

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let mut random = [0; 16];
        getrandom::getrandom(&mut random).unwrap();
        let root = std::env::temp_dir().canonicalize().unwrap().join(format!(
            "legura-private-read-{:032x}",
            u128::from_le_bytes(random)
        ));
        fs::create_dir(&root).unwrap();
        fs::create_dir(root.join("objects")).unwrap();
        let fixture = Self(root);
        let db = fixture.database();
        db.execute_batch(include_str!(
            "../db/migrations/20261008000000_001_identity/up.sql"
        ))
        .unwrap();
        db.execute_batch(include_str!(
            "../db/migrations/20261018000000_004_media/up.sql"
        ))
        .unwrap();
        db.execute(
            "INSERT INTO legura_users VALUES ('owner','owner@example.test','admin')",
            [],
        )
        .unwrap();
        fixture
    }
    fn store_id(&self) -> StoreId {
        StoreId::parse("local-media").unwrap()
    }
    fn database(&self) -> Connection {
        Connection::open(self.0.join("library.db")).unwrap()
    }
    fn storage(&self) -> LocalStorage {
        LocalStorage::open(&self.0.join("objects"), self.store_id(), MAX_OBJECT_BYTES).unwrap()
    }
    fn upload(&self, jpeg: bool) -> String {
        let mut bytes = Vec::new();
        if jpeg {
            image::codecs::jpeg::JpegEncoder::new(&mut bytes)
                .encode(&[1, 2, 3], 1, 1, image::ExtendedColorType::Rgb8)
                .unwrap();
        } else {
            image::codecs::png::PngEncoder::new(&mut bytes)
                .write_image(&[1, 2, 3, 255], 1, 1, image::ExtendedColorType::Rgba8)
                .unwrap();
        }
        MediaLibraryStorage::open(
            &self.0.join("objects"),
            self.store_id(),
            &self.0.join("library.db"),
        )
        .unwrap()
        .save_file(
            MEDIA_CAPABILITY,
            &AxIncomingFile {
                field_name: "image".into(),
                file_name: "ignored.html".into(),
                content_type: Some("text/html".into()),
                bytes,
            },
        )
        .unwrap()
        .id
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
            .starts_with("legura-private-read-"));
        fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn real_canonical_png_and_jpeg_reopen_as_private_bytes_without_mutation() {
    for jpeg in [false, true] {
        let fixture = Fixture::new();
        let id = fixture.upload(jpeg);
        let bytes = fs::read(fixture.0.join("objects").join(&id)).unwrap();
        let db_before = fs::read(fixture.0.join("library.db")).unwrap();
        let db = fixture.database();
        let storage = fixture.storage();
        let response =
            read_for_administrator(&db, &storage, &fixture.store_id(), Some("owner"), &id)
                .unwrap()
                .into_response();
        assert_eq!(
            response.content_type,
            if jpeg { "image/jpeg" } else { "image/png" }
        );
        assert_eq!(response.body.into_bytes(), bytes);
        assert_eq!(fs::read(fixture.0.join("library.db")).unwrap(), db_before);
        assert_eq!(
            fs::read(fixture.0.join("objects").join(&id)).unwrap(),
            bytes
        );
        db.execute("DELETE FROM legura_users WHERE id='owner'", [])
            .unwrap();
        assert_eq!(
            read_for_administrator(&db, &storage, &fixture.store_id(), Some("owner"), &id).err(),
            Some(MediaReadError::Forbidden)
        );
    }
}

#[test]
fn local_reads_refuse_missing_corrupt_oversize_and_nonregular_objects() {
    for mode in ["missing", "corrupt", "oversize", "directory"] {
        let fixture = Fixture::new();
        let id = fixture.upload(false);
        let path = fixture.0.join("objects").join(&id);
        match mode {
            "missing" => fs::remove_file(&path).unwrap(),
            "corrupt" => {
                let mut bytes = fs::read(&path).unwrap();
                bytes[0] ^= 1;
                fs::write(&path, bytes).unwrap();
            }
            "oversize" => fs::OpenOptions::new()
                .write(true)
                .open(&path)
                .unwrap()
                .set_len(MAX_OBJECT_BYTES + 1)
                .unwrap(),
            _ => {
                fs::remove_file(&path).unwrap();
                fs::create_dir(&path).unwrap();
            }
        }
        assert_eq!(
            read_for_administrator(
                &fixture.database(),
                &fixture.storage(),
                &fixture.store_id(),
                Some("owner"),
                &id
            )
            .err(),
            Some(MediaReadError::Unavailable)
        );
    }
}

#[cfg(unix)]
#[test]
fn final_symlink_is_refused_even_when_target_bytes_match_metadata() {
    let fixture = Fixture::new();
    let id = fixture.upload(false);
    let object = fixture.0.join("objects").join(&id);
    let target = fixture.0.join("outside-store.png");
    fs::rename(&object, &target).unwrap();
    std::os::unix::fs::symlink(&target, &object).unwrap();
    assert_eq!(
        read_for_administrator(
            &fixture.database(),
            &fixture.storage(),
            &fixture.store_id(),
            Some("owner"),
            &id
        )
        .err(),
        Some(MediaReadError::Unavailable)
    );
}

#[test]
fn configured_published_reads_use_existing_storage_and_current_database_state() {
    use legura::published_image::PublishedImageError;
    let fixture = Fixture::new();
    let database = fixture.database();
    for index in [1, 2, 4] {
        database
            .execute_batch(legura::database_contract::MIGRATIONS[index].1)
            .unwrap();
    }
    let id = fixture.upload(false);
    database.execute_batch(
        "INSERT INTO legura_posts(id,title,slug,body,status) VALUES ('post','Title','story','Body','published');",
    ).unwrap();
    database
        .execute(
            "INSERT INTO legura_post_images VALUES ('post', ?, '')",
            [&id],
        )
        .unwrap();
    let service = MediaLibraryStorage::open(
        &fixture.0.join("objects"),
        fixture.store_id(),
        &fixture.0.join("library.db"),
    )
    .unwrap();
    assert_eq!(
        service
            .read_published_post_image("story", &id)
            .unwrap()
            .into_response()
            .status,
        200
    );
    database
        .execute("UPDATE legura_posts SET status='draft' WHERE id='post'", [])
        .unwrap();
    assert!(matches!(
        service.read_published_post_image("story", &id),
        Err(PublishedImageError::NotFound)
    ));
    // No cached publication grant, even when the service instance is reused.
    database
        .execute(
            "UPDATE legura_posts SET status='published' WHERE id='post'",
            [],
        )
        .unwrap();
    assert!(service.read_published_post_image("story", &id).is_ok());
    database
        .execute("DELETE FROM legura_post_images", [])
        .unwrap();
    assert!(matches!(
        service.read_published_post_image("story", &id),
        Err(PublishedImageError::NotFound)
    ));
    assert!(fixture.0.join("objects").join(&id).is_file());
}
