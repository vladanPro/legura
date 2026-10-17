//! Inactive media persistence service. The caller must enforce administrator/CSRF policy.
use crate::media_upload::{MediaUploadError, MediaUploadStorage, StoredMedia};
use crate::storage::{LocalStorage, StoreId, MAX_OBJECT_BYTES};
use axonyx_runtime::backend::{AxRuntimeError, AxRuntimeResult};
use axonyx_runtime::server::{AxFileRef, AxFileStorage, AxIncomingFile};
use rusqlite::{params, Connection, OpenFlags};
use std::path::{Path, PathBuf};
use std::time::Duration;

#[derive(Debug)]
pub struct MediaLibraryUnavailable;
impl std::fmt::Display for MediaLibraryUnavailable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Media library is unavailable")
    }
}
impl std::error::Error for MediaLibraryUnavailable {}

pub struct MediaLibraryStorage {
    images: MediaUploadStorage<LocalStorage>,
    database: PathBuf,
}

impl MediaLibraryStorage {
    /// Both the database/schema and private directory must already exist.
    /// No env reading, directory creation, schema migration or DB creation occurs.
    pub fn open(
        root: &Path,
        store_id: StoreId,
        database: &Path,
    ) -> Result<Self, MediaLibraryUnavailable> {
        let images = LocalStorage::open(root, store_id, MAX_OBJECT_BYTES)
            .map_err(|_| MediaLibraryUnavailable)?;
        let service = Self {
            images: MediaUploadStorage::new(images),
            database: database.to_owned(),
        };
        let connection = service.connect()?;
        connection.prepare("SELECT id, store_id, content_type, byte_size, width, height, sha256 FROM legura_media LIMIT 0")
            .map_err(|_| MediaLibraryUnavailable)?;
        Ok(service)
    }

    fn connect(&self) -> Result<Connection, MediaLibraryUnavailable> {
        let connection = Connection::open_with_flags(
            &self.database,
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .map_err(|_| MediaLibraryUnavailable)?;
        connection
            .busy_timeout(Duration::from_secs(5))
            .map_err(|_| MediaLibraryUnavailable)?;
        Ok(connection)
    }
}

impl AxFileStorage for MediaLibraryStorage {
    fn save_file(&self, capability: &str, file: &AxIncomingFile) -> AxRuntimeResult<AxFileRef> {
        let saved = self
            .images
            .save_image(capability, file)
            .map_err(|error| match error {
                MediaUploadError::UnknownCapability => AxRuntimeError::Forbidden,
                MediaUploadError::InvalidImage(_) => AxRuntimeError::invalid_input("image"),
                MediaUploadError::Busy => {
                    AxRuntimeError::message("Media processing is busy; retry later")
                }
                _ => AxRuntimeError::message("Media storage is unavailable"),
            })?;
        let connection = self
            .connect()
            .map_err(|_| AxRuntimeError::message("Media metadata could not be saved"))?;
        insert_metadata(&connection, &saved)
            .map_err(|_| AxRuntimeError::message("Media metadata could not be saved"))?;
        let id = saved.object.reference.key.as_str().to_owned();
        Ok(AxFileRef {
            file_name: format!("{id}.{}", saved.extension),
            id,
            storage: saved.object.reference.store.as_str().to_owned(),
            content_type: Some(saved.content_type.to_owned()),
            size: saved.object.byte_size,
        })
    }
}

fn insert_metadata(connection: &Connection, saved: &StoredMedia) -> rusqlite::Result<()> {
    connection.execute("INSERT INTO legura_media (id, store_id, content_type, byte_size, width, height, sha256) VALUES (?, ?, ?, ?, ?, ?, ?)",
        params![saved.object.reference.key.as_str(), saved.object.reference.store.as_str(), saved.content_type,
            saved.object.byte_size, saved.width, saved.height, saved.object.sha256])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::{ObjectKey, ObjectRef, StoredObject};

    fn saved() -> StoredMedia {
        StoredMedia {
            object: StoredObject {
                reference: ObjectRef {
                    store: StoreId::parse("local-media").unwrap(),
                    key: ObjectKey::parse(&"a".repeat(64)).unwrap(),
                },
                byte_size: 200,
                sha256: "b".repeat(64),
            },
            width: 12,
            height: 10,
            content_type: "image/png",
            extension: "png",
        }
    }

    #[test]
    fn inserts_processed_metadata_without_overwriting_existing_objects() {
        let connection = Connection::open_in_memory().unwrap();
        connection
            .execute_batch(include_str!("../tests/media-http/schema.sql"))
            .unwrap();
        let saved = saved();
        insert_metadata(&connection, &saved).unwrap();
        assert!(insert_metadata(&connection, &saved).is_err());
        let row = connection.query_row("SELECT id, store_id, content_type, byte_size, width, height, sha256 FROM legura_media", [], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?, row.get::<_, u64>(3)?, row.get::<_, u32>(4)?, row.get::<_, u32>(5)?, row.get::<_, String>(6)?))
        }).unwrap();
        assert_eq!(
            row,
            (
                "a".repeat(64),
                "local-media".into(),
                "image/png".into(),
                200,
                12,
                10,
                "b".repeat(64)
            )
        );
    }

    #[test]
    fn metadata_constraints_and_sql_errors_fail_without_rows() {
        let connection = Connection::open_in_memory().unwrap();
        connection
            .execute_batch(include_str!("../tests/media-http/schema.sql"))
            .unwrap();
        let mut invalid = saved();
        invalid.content_type = "text/html";
        assert!(insert_metadata(&connection, &invalid).is_err());
        invalid = saved();
        invalid.width = 0;
        assert!(insert_metadata(&connection, &invalid).is_err());
        invalid = saved();
        invalid.object.byte_size = MAX_OBJECT_BYTES + 1;
        assert!(insert_metadata(&connection, &invalid).is_err());
        assert_eq!(
            connection
                .query_row("SELECT COUNT(*) FROM legura_media", [], |r| r
                    .get::<_, i64>(0))
                .unwrap(),
            0
        );
    }

    #[test]
    fn startup_requires_an_existing_database_and_schema_without_creating_them() {
        let mut nonce = [0; 16];
        getrandom::getrandom(&mut nonce).unwrap();
        let root = std::env::temp_dir().join(format!(
            "legura-media-preflight-{:032x}",
            u128::from_le_bytes(nonce)
        ));
        std::fs::create_dir(&root).unwrap();
        struct Cleanup(std::path::PathBuf);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
        let _cleanup = Cleanup(root.clone());
        let database = root.join("library.db");
        let open =
            || MediaLibraryStorage::open(&root, StoreId::parse("test-store").unwrap(), &database);
        assert!(open().is_err());
        assert!(!database.exists());
        let connection = Connection::open(&database).unwrap();
        assert!(open().is_err());
        assert_eq!(
            connection
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type='table'",
                    [],
                    |row| row.get::<_, i64>(0)
                )
                .unwrap(),
            0
        );
        connection
            .execute_batch(include_str!("../tests/media-http/schema.sql"))
            .unwrap();
        assert!(open().is_ok());
    }
}
