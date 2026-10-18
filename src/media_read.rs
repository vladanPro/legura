//! Private referenced-image reads. The HTTP caller must supply a verified session subject.
use crate::media_image::{MAX_IMAGE_DIMENSION, MAX_IMAGE_PIXELS};
use crate::storage::{ObjectKey, ObjectRef, ObjectStorage, StoreId, MAX_OBJECT_BYTES};
use axonyx_runtime::server::AxHttpResponse;
use image::{ImageFormat, ImageReader, Limits};
use rusqlite::{Connection, OptionalExtension};
use sha2::{Digest, Sha256};
use std::io::Cursor;

#[derive(Debug, PartialEq, Eq)]
pub enum MediaReadError {
    Forbidden,
    NotFound,
    Unavailable,
}

impl std::fmt::Display for MediaReadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Private media is unavailable")
    }
}
impl std::error::Error for MediaReadError {}

/// Not serializable: image bytes must never cross the native JSON registry.
pub struct PrivateMedia {
    bytes: Vec<u8>,
    content_type: &'static str,
}

impl PrivateMedia {
    pub fn into_native_response(
        self,
    ) -> Result<
        axonyx_runtime::native_response::AxNativeBinaryResponse,
        axonyx_runtime::native_response::AxNativeResponseError,
    > {
        use axonyx_runtime::native_response::{AxNativeBinaryResponse, AxNativeBinaryType};
        let kind = match self.content_type {
            "image/png" => AxNativeBinaryType::Png,
            "image/jpeg" => AxNativeBinaryType::Jpeg,
            _ => return Err(axonyx_runtime::native_response::AxNativeResponseError::Unavailable),
        };
        AxNativeBinaryResponse::new(kind, self.bytes)
    }

    pub fn into_response(self) -> AxHttpResponse {
        AxHttpResponse::bytes(200, self.content_type, self.bytes)
            .with_header("Cache-Control", "private, no-store")
            .with_header("X-Content-Type-Options", "nosniff")
            .with_header("Content-Disposition", "inline")
            .with_header("Cross-Origin-Resource-Policy", "same-origin")
    }
}

/// `subject` comes from the server's session verifier, never a query/body field.
/// Current DB authority is checked before metadata or object bytes are accessed.
/// Call only with the startup-validated DB/store capability; this is not a route.
pub fn read_for_administrator(
    database: &Connection,
    storage: &impl ObjectStorage,
    store: &StoreId,
    subject: Option<&str>,
    id: &str,
) -> Result<PrivateMedia, MediaReadError> {
    use MediaReadError::{Forbidden, NotFound, Unavailable};
    let subject = subject.filter(|value| !value.is_empty()).ok_or(Forbidden)?;
    let administrator = database
        .query_row(
            "SELECT role FROM legura_users WHERE id = ?",
            [subject],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|_| Unavailable)?;
    if administrator.as_deref() != Some("admin") {
        return Err(Forbidden);
    }
    let key = ObjectKey::parse(id).map_err(|_| NotFound)?;
    let metadata = database
        .query_row(
            "SELECT store_id, content_type, byte_size, width, height, sha256 FROM legura_media WHERE id = ?",
            [key.as_str()],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?,
                row.get::<_, u64>(2)?, row.get::<_, u32>(3)?, row.get::<_, u32>(4)?, row.get::<_, String>(5)?)),
        )
        .optional()
        .map_err(|_| Unavailable)?
        .ok_or(NotFound)?;
    let (actual_store, mime, size, width, height, hash) = metadata;
    let actual_store = StoreId::parse(&actual_store).map_err(|_| Unavailable)?;
    if &actual_store != store
        || size == 0
        || size > MAX_OBJECT_BYTES
        || width == 0
        || width > MAX_IMAGE_DIMENSION
        || height == 0
        || height > MAX_IMAGE_DIMENSION
        || u64::from(width) * u64::from(height) > MAX_IMAGE_PIXELS
        || ObjectKey::parse(&hash).is_err()
    {
        return Err(Unavailable);
    }
    let (format, content_type) = match mime.as_str() {
        "image/png" => (ImageFormat::Png, "image/png"),
        "image/jpeg" => (ImageFormat::Jpeg, "image/jpeg"),
        _ => return Err(Unavailable),
    };
    let bytes = storage
        .get(&ObjectRef {
            store: actual_store,
            key,
        })
        .map_err(|_| Unavailable)?;
    if bytes.len() as u64 != size
        || format!("{:x}", Sha256::digest(&bytes)) != hash
        || image::guess_format(&bytes).map_err(|_| Unavailable)? != format
    {
        return Err(Unavailable);
    }
    let mut reader = ImageReader::with_format(Cursor::new(&bytes), format);
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_IMAGE_DIMENSION);
    limits.max_image_height = Some(MAX_IMAGE_DIMENSION);
    limits.max_alloc = Some(MAX_OBJECT_BYTES);
    reader.limits(limits);
    if reader.into_dimensions().map_err(|_| Unavailable)? != (width, height) {
        return Err(Unavailable);
    }
    Ok(PrivateMedia {
        bytes,
        content_type,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::{StorageError, StoredObject};
    use image::ImageEncoder;
    struct Objects {
        bytes: Vec<u8>,
        reads: std::sync::atomic::AtomicUsize,
    }
    impl ObjectStorage for Objects {
        fn put(&self, _: &[u8]) -> Result<StoredObject, StorageError> {
            panic!("read attempted a write")
        }
        fn get(&self, _: &ObjectRef) -> Result<Vec<u8>, StorageError> {
            self.reads.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            if self.bytes.is_empty() {
                return Err(StorageError::NotFound);
            }
            Ok(self.bytes.clone())
        }
    }
    fn fixture() -> (Connection, Objects, StoreId, String) {
        let database = Connection::open_in_memory().unwrap();
        database
            .execute_batch(include_str!(
                "../db/migrations/20261008000000_001_identity/up.sql"
            ))
            .unwrap();
        database
            .execute_batch(include_str!(
                "../db/migrations/20261018000000_004_media/up.sql"
            ))
            .unwrap();
        database
            .execute(
                "INSERT INTO legura_users VALUES ('owner','owner@example.test','admin')",
                [],
            )
            .unwrap();
        let mut bytes = Vec::new();
        image::codecs::png::PngEncoder::new(&mut bytes)
            .write_image(&[1, 2, 3, 255], 1, 1, image::ExtendedColorType::Rgba8)
            .unwrap();
        let id = "a".repeat(64);
        database
            .execute(
                "INSERT INTO legura_media VALUES (?, 'local-media', 'image/png', ?, 1, 1, ?)",
                rusqlite::params![
                    id,
                    bytes.len() as u64,
                    format!("{:x}", Sha256::digest(&bytes))
                ],
            )
            .unwrap();
        (
            database,
            Objects {
                bytes,
                reads: Default::default(),
            },
            StoreId::parse("local-media").unwrap(),
            id,
        )
    }
    fn error(result: Result<PrivateMedia, MediaReadError>) -> MediaReadError {
        result.err().expect("must deny")
    }

    #[test]
    fn current_authority_precedes_metadata_and_storage_even_after_revocation() {
        let (database, storage, store, id) = fixture();
        for subject in [None, Some(""), Some("missing"), Some("owner' OR 1=1 --")] {
            assert_eq!(
                error(read_for_administrator(
                    &database, &storage, &store, subject, &id
                )),
                MediaReadError::Forbidden
            );
        }
        assert_eq!(storage.reads.load(std::sync::atomic::Ordering::SeqCst), 0);
        read_for_administrator(&database, &storage, &store, Some("owner"), &id).unwrap();
        database
            .execute("DELETE FROM legura_users WHERE id = 'owner'", [])
            .unwrap();
        assert_eq!(
            error(read_for_administrator(
                &database,
                &storage,
                &store,
                Some("owner"),
                &id
            )),
            MediaReadError::Forbidden
        );
        assert_eq!(storage.reads.load(std::sync::atomic::Ordering::SeqCst), 1);
    }

    #[test]
    fn binary_response_is_exact_private_and_not_json() {
        let (database, storage, store, id) = fixture();
        let response = read_for_administrator(&database, &storage, &store, Some("owner"), &id)
            .unwrap()
            .into_response();
        assert_eq!(response.status, 200);
        assert_eq!(response.content_type, "image/png");
        assert_eq!(
            response.header_value("Cache-Control"),
            Some("private, no-store")
        );
        assert_eq!(
            response.header_value("X-Content-Type-Options"),
            Some("nosniff")
        );
        assert_eq!(response.header_value("Content-Disposition"), Some("inline"));
        assert_eq!(
            response.header_value("Cross-Origin-Resource-Policy"),
            Some("same-origin")
        );
        assert_eq!(response.body.into_bytes(), storage.bytes);
    }

    #[test]
    fn malformed_unreferenced_and_wrong_store_keys_never_read_objects() {
        let (database, storage, store, id) = fixture();
        for key in [
            "../secret",
            "https://private/image",
            &"b".repeat(64),
            &id.to_uppercase(),
        ] {
            assert_eq!(
                error(read_for_administrator(
                    &database,
                    &storage,
                    &store,
                    Some("owner"),
                    key
                )),
                MediaReadError::NotFound
            );
        }
        assert_eq!(
            error(read_for_administrator(
                &database,
                &storage,
                &StoreId::parse("other").unwrap(),
                Some("owner"),
                &id
            )),
            MediaReadError::Unavailable
        );
        assert_eq!(storage.reads.load(std::sync::atomic::Ordering::SeqCst), 0);
    }

    #[test]
    fn missing_changed_or_mismatched_objects_are_not_served() {
        for mutation in 0..5 {
            let (database, mut storage, store, id) = fixture();
            match mutation {
                0 => storage.bytes.clear(),
                1 => storage.bytes[0] ^= 1,
                2 => {
                    database
                        .execute("UPDATE legura_media SET byte_size=1", [])
                        .unwrap();
                }
                3 => {
                    database
                        .execute("UPDATE legura_media SET content_type='image/jpeg'", [])
                        .unwrap();
                }
                _ => {
                    database
                        .execute("UPDATE legura_media SET width=2", [])
                        .unwrap();
                }
            }
            let error = error(read_for_administrator(
                &database,
                &storage,
                &store,
                Some("owner"),
                &id,
            ));
            assert_eq!(error, MediaReadError::Unavailable);
            assert_eq!(error.to_string(), "Private media is unavailable");
        }
    }
}
