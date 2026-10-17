use axonyx_runtime::server::{AxFileStorage, AxIncomingFile};
use image::ImageEncoder;
use legura::{
    backup_archive,
    media_library::MediaLibraryStorage,
    media_upload::MEDIA_CAPABILITY,
    storage::{LocalStorage, StoreId, MAX_OBJECT_BYTES},
};
use rusqlite::Connection;
use std::{
    fs,
    io::Read,
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicBool, Ordering},
};

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let mut random = [0; 16];
        getrandom::getrandom(&mut random).unwrap();
        let root = std::env::temp_dir().canonicalize().unwrap().join(format!(
            "legura-archive-{:032x}",
            u128::from_le_bytes(random)
        ));
        fs::create_dir(&root).unwrap();
        fs::create_dir(root.join("objects")).unwrap();
        let database = Connection::open(root.join("snapshot.sqlite")).unwrap();
        database
            .execute_batch(include_str!("media-http/schema.sql"))
            .unwrap();
        Self(root)
    }
    fn upload(&self) -> String {
        let storage = MediaLibraryStorage::open(
            &self.0.join("objects"),
            StoreId::parse("local-media").unwrap(),
            &self.0.join("snapshot.sqlite"),
        )
        .unwrap();
        let mut bytes = Vec::new();
        image::codecs::png::PngEncoder::new(&mut bytes)
            .write_image(&[1, 2, 3, 255], 1, 1, image::ExtendedColorType::Rgba8)
            .unwrap();
        storage
            .save_file(
                MEDIA_CAPABILITY,
                &AxIncomingFile {
                    field_name: "image".into(),
                    file_name: "ignored.svg".into(),
                    content_type: None,
                    bytes,
                },
            )
            .unwrap()
            .id
    }
    fn pack(&self, destination: &str) -> anyhow::Result<backup_archive::ArchiveReport> {
        let store = StoreId::parse("local-media").unwrap();
        let storage =
            LocalStorage::open(&self.0.join("objects"), store.clone(), MAX_OBJECT_BYTES).unwrap();
        backup_archive::pack_snapshot(
            &self.0.join("snapshot.sqlite"),
            &storage,
            &store,
            &self.0.join(destination),
        )
    }
    fn verify_bytes(&self, bytes: &[u8]) -> anyhow::Result<backup_archive::ArchiveReport> {
        let path = self.0.join("hostile.legura");
        fs::write(&path, bytes).unwrap();
        backup_archive::verify(&path)
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
            .starts_with("legura-archive-"));
        fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn live_backup_captures_committed_wal_and_excludes_later_uploads() {
    use legura::storage::{ObjectRef, ObjectStorage, StorageError, StoredObject};
    let fixture = Fixture::new();
    let connection = Connection::open(fixture.0.join("snapshot.sqlite")).unwrap();
    connection
        .pragma_update(None, "journal_mode", "WAL")
        .unwrap();
    connection
        .pragma_update(None, "wal_autocheckpoint", 0)
        .unwrap();
    // Pin an older WAL reader so closing the upload connection cannot checkpoint
    // and remove the WAL before the online backup is exercised.
    connection
        .execute_batch("BEGIN; SELECT COUNT(*) FROM legura_media;")
        .unwrap();
    let first = fixture.upload();
    assert!(
        fs::metadata(fixture.0.join("snapshot.sqlite-wal"))
            .unwrap()
            .len()
            > 0
    );
    struct LaterUpload<'a> {
        fixture: &'a Fixture,
        storage: LocalStorage,
        called: AtomicBool,
    }
    impl ObjectStorage for LaterUpload<'_> {
        fn put(&self, _: &[u8]) -> Result<StoredObject, StorageError> {
            panic!("backup attempted storage write")
        }
        fn get(&self, reference: &ObjectRef) -> Result<Vec<u8>, StorageError> {
            // A real committed upload after the private DB snapshot exists.
            if !self.called.swap(true, Ordering::SeqCst) {
                self.fixture.upload();
            }
            self.storage.get(reference)
        }
    }
    let store = StoreId::parse("local-media").unwrap();
    let storage = LaterUpload {
        fixture: &fixture,
        storage: LocalStorage::open(&fixture.0.join("objects"), store.clone(), MAX_OBJECT_BYTES)
            .unwrap(),
        called: AtomicBool::new(false),
    };
    let archive = fixture.0.join("live.legura");
    assert_eq!(
        backup_archive::backup(
            &fixture.0.join("snapshot.sqlite"),
            &storage,
            &store,
            &archive
        )
        .unwrap()
        .files,
        2
    );
    assert!(storage.called.load(Ordering::SeqCst));
    connection.execute_batch("COMMIT").unwrap();
    assert_eq!(
        connection
            .query_row("SELECT COUNT(*) FROM legura_media", [], |row| row
                .get::<_, i64>(0))
            .unwrap(),
        2
    );
    let mut names = Vec::new();
    for entry in tar::Archive::new(fs::File::open(&archive).unwrap())
        .entries()
        .unwrap()
    {
        let mut entry = entry.unwrap();
        let name = entry.path().unwrap().to_string_lossy().into_owned();
        if name == "database.sqlite" {
            let mut bytes = Vec::new();
            entry.read_to_end(&mut bytes).unwrap();
            // Test-only inspection, not a product restore/extraction API.
            let recovered = fixture.0.join("inspected.sqlite");
            fs::write(&recovered, bytes).unwrap();
            let copied =
                Connection::open_with_flags(recovered, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
                    .unwrap();
            assert_eq!(
                copied
                    .query_row("SELECT COUNT(*) FROM legura_media", [], |row| row
                        .get::<_, i64>(0))
                    .unwrap(),
                1
            );
            assert_eq!(
                copied
                    .query_row("SELECT id FROM legura_media", [], |row| row
                        .get::<_, String>(0))
                    .unwrap(),
                first
            );
        }
        names.push(name);
    }
    assert_eq!(
        names,
        vec![
            "database.sqlite".to_owned(),
            format!("media/local-media/{first}"),
            "manifest.json".to_owned()
        ]
    );
}

#[test]
fn live_backup_cli_is_new_only_and_never_creates_missing_sources() {
    let fixture = Fixture::new();
    fixture.upload();
    let output = fixture.0.join("live.legura");
    let run = |source: &str| {
        Command::new(env!("CARGO_BIN_EXE_legura-maintenance"))
            .arg("archive-backup")
            .arg(fixture.0.join(source))
            .arg("local-media")
            .arg(fixture.0.join("objects"))
            .arg(&output)
            .env("AX_SECRET_DB_URL", "ignore-this-environment-value")
            .output()
            .unwrap()
    };
    let missing = run("missing.sqlite");
    assert!(!missing.status.success());
    assert!(!fixture.0.join("missing.sqlite").exists());
    assert!(!output.exists());
    let complete = run("snapshot.sqlite");
    assert!(
        complete.status.success(),
        "{}",
        String::from_utf8_lossy(&complete.stderr)
    );
    let original = fs::read(&output).unwrap();
    assert!(!run("snapshot.sqlite").status.success());
    assert_eq!(fs::read(&output).unwrap(), original);
    assert_eq!(backup_archive::verify(&output).unwrap().files, 2);
}

#[test]
fn packages_snapshot_and_referenced_images_as_standard_tar_without_mutating_sources() {
    let fixture = Fixture::new();
    let id = fixture.upload();
    let database = fs::read(fixture.0.join("snapshot.sqlite")).unwrap();
    let image = fs::read(fixture.0.join("objects").join(&id)).unwrap();
    fs::write(fixture.0.join("objects").join("b".repeat(64)), b"orphan").unwrap();
    let report = fixture.pack("site.legura").unwrap();
    assert_eq!(report.files, 2);
    assert_eq!(report.bytes, (database.len() + image.len()) as u64);
    assert_eq!(
        backup_archive::verify(&fixture.0.join("site.legura")).unwrap(),
        report
    );
    let mut contents = Vec::new();
    for entry in tar::Archive::new(fs::File::open(fixture.0.join("site.legura")).unwrap())
        .entries()
        .unwrap()
    {
        let mut entry = entry.unwrap();
        let name = entry.path().unwrap().to_string_lossy().into_owned();
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes).unwrap();
        contents.push((name, bytes));
    }
    assert_eq!(contents.len(), 3);
    assert_eq!(contents[0], ("database.sqlite".into(), database.clone()));
    assert_eq!(
        contents[1],
        (format!("media/local-media/{id}"), image.clone())
    );
    assert_eq!(contents[2].0, "manifest.json");
    let manifest: serde_json::Value = serde_json::from_slice(&contents[2].1).unwrap();
    assert_eq!(manifest["store"], "local-media");
    assert_eq!(manifest["files"].as_object().unwrap().len(), 2);
    assert_eq!(
        fs::read(fixture.0.join("snapshot.sqlite")).unwrap(),
        database
    );
    assert_eq!(fs::read(fixture.0.join("objects").join(id)).unwrap(), image);
    assert_eq!(fs::read_dir(fixture.0.join("objects")).unwrap().count(), 2);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            fs::metadata(fixture.0.join("site.legura"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
    }
}

#[test]
fn refuses_existing_destinations_missing_objects_and_nonstandalone_snapshots() {
    let fixture = Fixture::new();
    let id = fixture.upload();
    fs::write(fixture.0.join("existing.legura"), b"keep").unwrap();
    assert!(fixture.pack("existing.legura").is_err());
    assert_eq!(
        fs::read(fixture.0.join("existing.legura")).unwrap(),
        b"keep"
    );
    fs::write(fixture.0.join("snapshot.sqlite-wal"), b"sidecar").unwrap();
    assert!(fixture.pack("sidecar.legura").is_err());
    assert!(!fixture.0.join("sidecar.legura").exists());
    fs::remove_file(fixture.0.join("snapshot.sqlite-wal")).unwrap();
    fs::remove_file(fixture.0.join("objects").join(id)).unwrap();
    assert!(fixture.pack("incomplete.legura").is_err());
    assert!(!fixture.0.join("incomplete.legura").exists());
}

#[test]
fn empty_media_snapshot_and_corrupted_source_object_have_explicit_outcomes() {
    let fixture = Fixture::new();
    assert_eq!(fixture.pack("empty.legura").unwrap().files, 1);
    let id = fixture.upload();
    let path = fixture.0.join("objects").join(id);
    let mut bytes = fs::read(&path).unwrap();
    bytes[0] ^= 1;
    fs::write(&path, &bytes).unwrap();
    assert!(fixture.pack("corrupt.legura").is_err());
    assert!(!fixture.0.join("corrupt.legura").exists());
    assert_eq!(fs::read(&path).unwrap(), bytes);
}

#[test]
fn longest_store_id_roundtrips_through_standard_ustar_prefix() {
    let fixture = Fixture::new();
    fixture.upload();
    let store = StoreId::parse(&"a".repeat(64)).unwrap();
    let database = Connection::open(fixture.0.join("snapshot.sqlite")).unwrap();
    database
        .execute("UPDATE legura_media SET store_id = ?", [store.as_str()])
        .unwrap();
    drop(database);
    let storage =
        LocalStorage::open(&fixture.0.join("objects"), store.clone(), MAX_OBJECT_BYTES).unwrap();
    let path = fixture.0.join("long-store.legura");
    let packed =
        backup_archive::pack_snapshot(&fixture.0.join("snapshot.sqlite"), &storage, &store, &path)
            .unwrap();
    assert_eq!(backup_archive::verify(&path).unwrap(), packed);
}

#[test]
fn corruption_truncation_and_concatenated_payloads_are_rejected() {
    let fixture = Fixture::new();
    fixture.upload();
    fixture.pack("site.legura").unwrap();
    let original = fs::read(fixture.0.join("site.legura")).unwrap();
    let mut corrupted = original.clone();
    corrupted[512 + 123] ^= 1;
    assert!(fixture.verify_bytes(&corrupted).is_err());
    assert!(fixture
        .verify_bytes(&original[..original.len() - 512])
        .is_err());
    let mut concatenated = original.clone();
    concatenated.extend_from_slice(&original);
    assert!(fixture
        .verify_bytes(&concatenated)
        .unwrap_err()
        .to_string()
        .contains("after manifest"));
    let mut trailing = original.clone();
    trailing.extend_from_slice(b"garbage");
    assert!(fixture.verify_bytes(&trailing).is_err());
}

fn raw_archive(path: &str, kind: tar::EntryType, size: u64) -> Vec<u8> {
    let mut output = tar::Builder::new(Vec::new());
    let mut first = tar::Header::new_ustar();
    first.set_path("database.sqlite").unwrap();
    first.set_size(1);
    first.set_mode(0o600);
    first.set_cksum();
    output.append(&first, b"x".as_slice()).unwrap();
    let mut header = tar::Header::new_ustar();
    // Bypass the writer's path sanitization to exercise hostile raw TAR headers.
    header.as_mut_bytes()[..path.len()].copy_from_slice(path.as_bytes());
    header.set_entry_type(kind);
    header.set_size(size);
    header.set_mode(0o600);
    header.set_cksum();
    if size > 1 {
        output.finish().unwrap();
        let mut bytes = output.into_inner().unwrap();
        bytes.truncate(bytes.len() - 1024);
        bytes.extend_from_slice(header.as_bytes());
        bytes.extend_from_slice(&[0; 1024]);
        bytes
    } else {
        output.append(&header, b"x".as_slice()).unwrap();
        output.finish().unwrap();
        output.into_inner().unwrap()
    }
}

#[test]
fn raw_paths_links_extensions_duplicates_and_oversized_headers_fail_before_extraction() {
    let fixture = Fixture::new();
    for path in [
        "../escape",
        "/absolute",
        "C:\\private",
        "./database.sqlite",
        "media/x/../escape",
        "media/x/not-a-key",
    ] {
        let error = fixture
            .verify_bytes(&raw_archive(path, tar::EntryType::Regular, 1))
            .unwrap_err();
        assert!(
            !error.to_string().contains("manifest is missing"),
            "{path}: {error}"
        );
    }
    for kind in [
        tar::EntryType::Symlink,
        tar::EntryType::Link,
        tar::EntryType::Directory,
        tar::EntryType::Fifo,
        tar::EntryType::XHeader,
    ] {
        assert!(fixture
            .verify_bytes(&raw_archive("other", kind, 1))
            .unwrap_err()
            .to_string()
            .contains("regular USTAR"));
    }
    assert!(fixture
        .verify_bytes(&raw_archive("database.sqlite", tar::EntryType::Regular, 1))
        .unwrap_err()
        .to_string()
        .contains("Duplicate"));
    let path = format!("media/x/{}", "a".repeat(64));
    assert!(fixture
        .verify_bytes(&raw_archive(&path, tar::EntryType::Regular, 8388609))
        .unwrap_err()
        .to_string()
        .contains("entry exceeds"));
    assert!(fixture
        .verify_bytes(&raw_archive(
            "manifest.json",
            tar::EntryType::Regular,
            4 * 1024 * 1024 + 1
        ))
        .unwrap_err()
        .to_string()
        .contains("entry exceeds"));
    assert!(!fixture.0.join("escape").exists());
}

#[test]
fn altered_inventory_store_and_contract_are_refused() {
    let fixture = Fixture::new();
    fixture.upload();
    fixture.pack("site.legura").unwrap();
    let source = fs::read(fixture.0.join("site.legura")).unwrap();
    for mutation in ["hash", "size", "inventory", "store", "format", "version"] {
        let mut builder = tar::Builder::new(Vec::new());
        for entry in tar::Archive::new(source.as_slice()).entries().unwrap() {
            let mut entry = entry.unwrap();
            let path = entry.path().unwrap().into_owned();
            let mut bytes = Vec::new();
            entry.read_to_end(&mut bytes).unwrap();
            if path == std::path::Path::new("manifest.json") {
                let mut value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
                match mutation {
                    "hash" => value["files"]["database.sqlite"]["sha256"] = "0".repeat(64).into(),
                    "size" => value["files"]["database.sqlite"]["bytes"] = 1.into(),
                    "inventory" => {
                        value["files"]
                            .as_object_mut()
                            .unwrap()
                            .remove("database.sqlite");
                    }
                    "store" => value["store"] = "other-store".into(),
                    "format" => value["format"] = 2.into(),
                    "version" => value["version"] = "future".into(),
                    _ => unreachable!(),
                }
                bytes = serde_json::to_vec(&value).unwrap();
            }
            let mut header = tar::Header::new_ustar();
            header.set_mode(0o600);
            header.set_size(bytes.len() as u64);
            builder
                .append_data(&mut header, path, bytes.as_slice())
                .unwrap();
        }
        builder.finish().unwrap();
        assert!(
            fixture
                .verify_bytes(&builder.into_inner().unwrap())
                .is_err(),
            "{mutation}"
        );
    }
}

#[test]
fn maintenance_cli_packs_and_verifies_without_reading_environment_configuration() {
    let fixture = Fixture::new();
    fixture.upload();
    let packed = Command::new(env!("CARGO_BIN_EXE_legura-maintenance"))
        .args(["archive-pack-snapshot"])
        .arg(fixture.0.join("snapshot.sqlite"))
        .arg("local-media")
        .arg(fixture.0.join("objects"))
        .arg(fixture.0.join("cli.legura"))
        .env("AX_SECRET_DB_URL", "not-the-source")
        .output()
        .unwrap();
    assert!(
        packed.status.success(),
        "{}",
        String::from_utf8_lossy(&packed.stderr)
    );
    let checked = Command::new(env!("CARGO_BIN_EXE_legura-maintenance"))
        .arg("archive-verify")
        .arg(fixture.0.join("cli.legura"))
        .output()
        .unwrap();
    assert!(checked.status.success());
    assert_eq!(packed.stdout, checked.stdout);
}

#[test]
fn recovery_preserves_keys_bytes_and_source_without_activating_configuration() {
    let fixture = Fixture::new();
    let key = fixture.upload();
    let original_db = fs::read(fixture.0.join("snapshot.sqlite")).unwrap();
    let original_object = fs::read(fixture.0.join("objects").join(&key)).unwrap();
    let report = fixture.pack("site.legura").unwrap();
    let target = fixture.0.join("recovered");
    assert_eq!(
        backup_archive::restore(&fixture.0.join("site.legura"), &target).unwrap(),
        report
    );
    assert_eq!(
        fs::read(target.join("database.sqlite")).unwrap(),
        original_db
    );
    let object = target.join("media/local-media").join(&key);
    assert_eq!(fs::read(&object).unwrap(), original_object);
    assert_eq!(
        fs::read(fixture.0.join("snapshot.sqlite")).unwrap(),
        original_db
    );
    assert_eq!(
        fs::read(fixture.0.join("objects").join(&key)).unwrap(),
        original_object
    );
    let receipt: serde_json::Value =
        serde_json::from_slice(&fs::read(target.join("recovery.json")).unwrap()).unwrap();
    assert_eq!(receipt["status"], "recovered-not-activated");
    assert_eq!(receipt["cms_compatibility"], "not-checked");
    assert_eq!(receipt["inventory"]["files"], 2);
    assert!(!target.join(".env").exists());
    assert_eq!(fs::read_dir(&target).unwrap().count(), 3);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        for directory in [
            &target,
            &target.join("media"),
            &target.join("media/local-media"),
        ] {
            assert_eq!(
                fs::metadata(directory).unwrap().permissions().mode() & 0o777,
                0o700
            );
        }
        for file in [
            &object,
            &target.join("database.sqlite"),
            &target.join("recovery.json"),
        ] {
            assert_eq!(
                fs::metadata(file).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
    }
    // Even an empty existing directory is not ours to merge into or clean up.
    let empty = fixture.0.join("existing-empty");
    fs::create_dir(&empty).unwrap();
    for destination in [&empty, &target, &fixture.0.join("snapshot.sqlite")] {
        assert!(backup_archive::restore(&fixture.0.join("site.legura"), destination).is_err());
    }
    assert!(empty.is_dir());
    assert_eq!(fs::read(object).unwrap(), original_object);
}

// Recompute every hash so these fixtures pass container verification. Recovery
// must independently validate the SQLite/media relationship, not trust hashes.
fn rewritten_bundle(
    fixture: &Fixture,
    mutate: impl FnOnce(&mut std::collections::BTreeMap<String, Vec<u8>>),
) -> PathBuf {
    use sha2::{Digest, Sha256};
    let mut payloads = std::collections::BTreeMap::new();
    let source = fs::read(fixture.0.join("site.legura")).unwrap();
    for entry in tar::Archive::new(source.as_slice()).entries().unwrap() {
        let mut entry = entry.unwrap();
        let path = entry.path().unwrap().to_str().unwrap().to_owned();
        if path != "manifest.json" {
            let mut bytes = Vec::new();
            entry.read_to_end(&mut bytes).unwrap();
            payloads.insert(path, bytes);
        }
    }
    mutate(&mut payloads);
    let files: serde_json::Map<String, serde_json::Value> = payloads
        .iter()
        .map(|(path, bytes)| {
            (
                path.clone(),
                serde_json::json!({
                    "bytes": bytes.len(), "sha256": format!("{:x}", Sha256::digest(bytes))
                }),
            )
        })
        .collect();
    let manifest = serde_json::to_vec(&serde_json::json!({
        "format": 1, "product": "legura", "version": env!("CARGO_PKG_VERSION"),
        "store": "local-media", "files": files,
    }))
    .unwrap();
    let output = fixture.0.join("rewritten.legura");
    let mut builder = tar::Builder::new(fs::File::create(&output).unwrap());
    for (path, bytes) in payloads
        .iter()
        .map(|(p, b)| (p.as_str(), b.as_slice()))
        .chain(std::iter::once(("manifest.json", manifest.as_slice())))
    {
        let mut header = tar::Header::new_ustar();
        header.set_size(bytes.len() as u64);
        header.set_mode(0o600);
        builder.append_data(&mut header, path, bytes).unwrap();
    }
    builder.finish().unwrap();
    output
}

#[test]
fn semantic_recovery_rejects_missing_extra_changed_objects_and_invalid_sqlite() {
    let fixture = Fixture::new();
    let key = fixture.upload();
    fixture.pack("site.legura").unwrap();
    let object = format!("media/local-media/{key}");
    for mutation in [
        "missing",
        "extra",
        "changed",
        "sqlite",
        "foreign-key",
        "duplicate-id",
    ] {
        let archive = rewritten_bundle(&fixture, |payloads| match mutation {
            "missing" => {
                payloads.remove(&object);
            }
            "extra" => {
                payloads.insert(format!("media/local-media/{}", "f".repeat(64)), vec![1]);
            }
            "changed" => {
                payloads.insert(object.clone(), vec![1]);
            }
            "sqlite" => {
                payloads.insert("database.sqlite".into(), b"not SQLite".to_vec());
            }
            "foreign-key" | "duplicate-id" => {
                let path = fixture.0.join("edited.sqlite");
                fs::write(&path, &payloads["database.sqlite"]).unwrap();
                let database = Connection::open(&path).unwrap();
                if mutation == "foreign-key" {
                    database.execute_batch("PRAGMA foreign_keys=OFF; CREATE TABLE parent(id INTEGER PRIMARY KEY); CREATE TABLE child(parent_id INTEGER REFERENCES parent(id)); INSERT INTO child VALUES(99)").unwrap();
                } else {
                    database.execute_batch("CREATE TABLE copied AS SELECT * FROM legura_media; DROP TABLE legura_media; ALTER TABLE copied RENAME TO legura_media; INSERT INTO legura_media SELECT * FROM legura_media").unwrap();
                    // Keep payload counts equal despite duplicate DB references.
                    payloads.insert(format!("media/local-media/{}", "f".repeat(64)), vec![1]);
                }
                drop(database);
                payloads.insert("database.sqlite".into(), fs::read(&path).unwrap());
                fs::remove_file(path).unwrap();
            }
            _ => unreachable!(),
        });
        assert!(backup_archive::verify(&archive).is_ok(), "{mutation}");
        let destination = fixture.0.join("refused");
        assert!(
            backup_archive::restore(&archive, &destination).is_err(),
            "{mutation}"
        );
        assert!(
            !destination.exists(),
            "failed recovery left output: {mutation}"
        );
    }
    assert!(fixture.0.join("objects").join(&key).exists());
}

#[test]
fn recovery_cli_supports_empty_media_and_refuses_repeated_or_corrupt_input() {
    let fixture = Fixture::new();
    let report = fixture.pack("site.legura").unwrap();
    let target = fixture.0.join("cli-recovered");
    let invoke = || {
        Command::new(env!("CARGO_BIN_EXE_legura-maintenance"))
            .arg("archive-restore")
            .arg(fixture.0.join("site.legura"))
            .arg(&target)
            .env("AX_SECRET_DB_URL", "must-not-be-used")
            .output()
            .unwrap()
    };
    let output = invoke();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&output.stdout).unwrap()["files"],
        report.files
    );
    assert!(String::from_utf8_lossy(&output.stderr).contains("NOT checked"));
    assert!(!invoke().status.success());
    assert!(target.join("recovery.json").exists());
    let corrupt = fixture.0.join("corrupt.legura");
    fs::write(
        &corrupt,
        raw_archive("../escape", tar::EntryType::Regular, 1),
    )
    .unwrap();
    let destination = fixture.0.join("no-output");
    assert!(backup_archive::restore(&corrupt, &destination).is_err());
    assert!(!destination.exists());
    assert!(!fixture.0.join("escape").exists());
}

#[cfg(unix)]
#[test]
fn recovery_refuses_symlink_destinations_without_touching_their_targets() {
    let fixture = Fixture::new();
    fixture.pack("site.legura").unwrap();
    let target = fixture.0.join("existing");
    fs::create_dir(&target).unwrap();
    let link = fixture.0.join("link");
    std::os::unix::fs::symlink(&target, &link).unwrap();
    assert!(backup_archive::restore(&fixture.0.join("site.legura"), &link).is_err());
    assert!(fs::symlink_metadata(link).unwrap().file_type().is_symlink());
    assert_eq!(fs::read_dir(target).unwrap().count(), 0);
}
