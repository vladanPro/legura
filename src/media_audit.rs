//! Read-only referenced-object integrity check; not a backup or image decoder.
use crate::media_image::{MAX_IMAGE_DIMENSION, MAX_IMAGE_PIXELS};
use crate::storage::{ObjectKey, ObjectRef, ObjectStorage, StoreId, MAX_OBJECT_BYTES};
use rusqlite::Connection;
use serde::Serialize;
use sha2::{Digest, Sha256};

#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct MediaInventory {
    pub objects: u64,
    pub bytes: u64,
}

#[derive(Debug, PartialEq, Eq)]
pub enum MediaAuditError {
    Database,
    InvalidMetadata,
    WrongStore,
    UnreadableObject,
    ObjectMismatch,
}

impl std::fmt::Display for MediaAuditError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Database => "Media metadata could not be read",
            Self::InvalidMetadata => "Media metadata is invalid",
            Self::WrongStore => "Media references an unconfigured store",
            Self::UnreadableObject => "A referenced media object is missing or unreadable",
            Self::ObjectMismatch => "Media object bytes do not match stored metadata",
        })
    }
}
impl std::error::Error for MediaAuditError {}

/// Visit only referenced immutable objects, with one bounded object in memory.
/// The caller supplies an explicit DB connection and storage capability. This
/// does not scan or delete orphans, certify image decoding or repair metadata.
pub fn audit(
    database: &Connection,
    storage: &impl ObjectStorage,
    store: &StoreId,
) -> Result<MediaInventory, MediaAuditError> {
    let mut query = database
        .prepare("SELECT id, store_id, content_type, byte_size, width, height, sha256 FROM legura_media ORDER BY id")
        .map_err(|_| MediaAuditError::Database)?;
    let rows = query
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, u64>(3)?,
                row.get::<_, u32>(4)?,
                row.get::<_, u32>(5)?,
                row.get::<_, String>(6)?,
            ))
        })
        .map_err(|_| MediaAuditError::Database)?;
    let mut report = MediaInventory {
        objects: 0,
        bytes: 0,
    };
    for row in rows {
        let (key, store_id, mime, size, width, height, hash) =
            row.map_err(|_| MediaAuditError::InvalidMetadata)?;
        let key = ObjectKey::parse(&key).map_err(|_| MediaAuditError::InvalidMetadata)?;
        let actual_store =
            StoreId::parse(&store_id).map_err(|_| MediaAuditError::InvalidMetadata)?;
        if !matches!(mime.as_str(), "image/png" | "image/jpeg")
            || size == 0
            || size > MAX_OBJECT_BYTES
            || width == 0
            || height == 0
            || width > MAX_IMAGE_DIMENSION
            || height > MAX_IMAGE_DIMENSION
            || u64::from(width) * u64::from(height) > MAX_IMAGE_PIXELS
            || ObjectKey::parse(&hash).is_err()
        {
            return Err(MediaAuditError::InvalidMetadata);
        }
        if &actual_store != store {
            return Err(MediaAuditError::WrongStore);
        }
        let bytes = storage
            .get(&ObjectRef {
                store: actual_store,
                key,
            })
            .map_err(|_| MediaAuditError::UnreadableObject)?;
        if bytes.len() as u64 != size || format!("{:x}", Sha256::digest(&bytes)) != hash {
            return Err(MediaAuditError::ObjectMismatch);
        }
        report.objects = report
            .objects
            .checked_add(1)
            .ok_or(MediaAuditError::InvalidMetadata)?;
        report.bytes = report
            .bytes
            .checked_add(size)
            .ok_or(MediaAuditError::InvalidMetadata)?;
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::{StorageError, StoredObject};
    use std::collections::HashMap;

    struct Objects(HashMap<String, Vec<u8>>);
    impl ObjectStorage for Objects {
        fn put(&self, _: &[u8]) -> Result<StoredObject, StorageError> {
            panic!("read-only audit attempted a write")
        }
        fn get(&self, reference: &ObjectRef) -> Result<Vec<u8>, StorageError> {
            self.0
                .get(reference.key.as_str())
                .cloned()
                .ok_or(StorageError::NotFound)
        }
    }

    fn fixture() -> (Connection, Objects, StoreId) {
        let database = Connection::open_in_memory().unwrap();
        // Deliberately no CHECK constraints: audit must not trust DB enforcement.
        database.execute_batch("CREATE TABLE legura_media(id TEXT PRIMARY KEY, store_id TEXT, content_type TEXT, byte_size INTEGER, width INTEGER, height INTEGER, sha256 TEXT)").unwrap();
        let bytes = b"immutable processed bytes".to_vec();
        let key = "a".repeat(64);
        database
            .execute(
                "INSERT INTO legura_media VALUES (?, 'local-media', 'image/png', ?, 1, 1, ?)",
                rusqlite::params![
                    key,
                    bytes.len() as u64,
                    format!("{:x}", Sha256::digest(&bytes))
                ],
            )
            .unwrap();
        (
            database,
            Objects(HashMap::from([(key, bytes)])),
            StoreId::parse("local-media").unwrap(),
        )
    }

    #[test]
    fn verifies_referenced_bytes_without_writing_or_counting_orphans() {
        let (database, mut objects, store) = fixture();
        objects.0.insert("b".repeat(64), b"unreferenced".to_vec());
        database.pragma_update(None, "query_only", true).unwrap();
        let changes = database.total_changes();
        let report = audit(&database, &objects, &store).unwrap();
        assert_eq!(
            report,
            MediaInventory {
                objects: 1,
                bytes: 25
            }
        );
        assert_eq!(database.total_changes(), changes);
        assert_eq!(objects.0.len(), 2);
    }

    #[test]
    fn empty_library_is_valid_but_missing_schema_is_not() {
        let (database, objects, store) = fixture();
        database.execute("DELETE FROM legura_media", []).unwrap();
        assert_eq!(
            audit(&database, &objects, &store).unwrap(),
            MediaInventory {
                objects: 0,
                bytes: 0
            }
        );
        database.execute("DROP TABLE legura_media", []).unwrap();
        assert_eq!(
            audit(&database, &objects, &store),
            Err(MediaAuditError::Database)
        );
    }

    #[test]
    fn missing_truncated_and_same_size_changed_objects_fail_closed() {
        let (database, mut objects, store) = fixture();
        let key = "a".repeat(64);
        let original = objects.0.remove(&key).unwrap();
        assert_eq!(
            audit(&database, &objects, &store),
            Err(MediaAuditError::UnreadableObject)
        );
        objects.0.insert(key.clone(), vec![0; original.len() - 1]);
        assert_eq!(
            audit(&database, &objects, &store),
            Err(MediaAuditError::ObjectMismatch)
        );
        objects.0.insert(key, vec![0; original.len()]);
        assert_eq!(
            audit(&database, &objects, &store),
            Err(MediaAuditError::ObjectMismatch)
        );
    }

    #[test]
    fn malformed_metadata_and_wrong_store_fail_without_exposing_values() {
        for update in [
            "id = '../private'",
            "store_id = '../private'",
            "content_type = 'text/html'",
            "byte_size = -1",
            "byte_size = 0",
            "byte_size = 8388609",
            "byte_size = NULL",
            "width = 0",
            "height = 8193",
            "width = 5000, height = 5000",
            "sha256 = 'bad'",
        ] {
            let (database, objects, store) = fixture();
            database
                .execute(&format!("UPDATE legura_media SET {update}"), [])
                .unwrap();
            assert_eq!(
                audit(&database, &objects, &store),
                Err(MediaAuditError::InvalidMetadata),
                "{update}"
            );
        }
        let (database, objects, store) = fixture();
        database
            .execute("UPDATE legura_media SET store_id = 'other-store'", [])
            .unwrap();
        assert_eq!(
            audit(&database, &objects, &store),
            Err(MediaAuditError::WrongStore)
        );
        assert!(!MediaAuditError::WrongStore
            .to_string()
            .contains("other-store"));
    }
}
