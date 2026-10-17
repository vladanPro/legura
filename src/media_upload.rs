//! Validated image storage bridge; request authorization belongs to the caller.
use crate::media_image::{MediaImage, MediaImageError};
use crate::storage::{ObjectStorage, StorageError, StoredObject};
use axonyx_runtime::backend::{AxRuntimeError, AxRuntimeResult};
use axonyx_runtime::server::{AxFileRef, AxFileStorage, AxIncomingFile};
use std::fmt;
use std::sync::{Mutex, TryLockError};

pub const MEDIA_CAPABILITY: &str = "legura-media";

pub struct StoredMedia {
    pub object: StoredObject,
    pub width: u32,
    pub height: u32,
    pub content_type: &'static str,
    pub extension: &'static str,
}

#[derive(Debug)]
pub enum MediaUploadError {
    UnknownCapability,
    Busy,
    Unavailable,
    InvalidImage(MediaImageError),
    Storage(StorageError),
}

impl fmt::Display for MediaUploadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::UnknownCapability => "Media storage capability is unavailable",
            Self::Busy => "Media processing is busy; retry later",
            Self::Unavailable | Self::Storage(_) => "Media storage is unavailable",
            Self::InvalidImage(_) => "Choose a supported image within the upload limits",
        })
    }
}

impl std::error::Error for MediaUploadError {}

/// Construct once per process and share. At most one decode/write runs per
/// instance; excess work is rejected, not queued on an async executor.
pub struct MediaUploadStorage<S> {
    store: S,
    processing: Mutex<()>,
}

impl<S: ObjectStorage> MediaUploadStorage<S> {
    pub fn new(store: S) -> Self {
        Self {
            store,
            processing: Mutex::new(()),
        }
    }

    /// No filename, declared MIME or client path becomes a storage identifier.
    /// This method grants no administrator, session or CSRF authority.
    pub fn save_image(
        &self,
        capability: &str,
        file: &AxIncomingFile,
    ) -> Result<StoredMedia, MediaUploadError> {
        if capability != MEDIA_CAPABILITY {
            return Err(MediaUploadError::UnknownCapability);
        }
        let _permit = self.processing.try_lock().map_err(|error| match error {
            TryLockError::WouldBlock => MediaUploadError::Busy,
            TryLockError::Poisoned(_) => MediaUploadError::Unavailable,
        })?;
        let image = MediaImage::prepare(&file.bytes).map_err(MediaUploadError::InvalidImage)?;
        let object = self
            .store
            .put(image.bytes())
            .map_err(MediaUploadError::Storage)?;
        Ok(StoredMedia {
            object,
            width: image.width(),
            height: image.height(),
            content_type: image.content_type(),
            extension: image.extension(),
        })
    }
}

impl<S: ObjectStorage> AxFileStorage for MediaUploadStorage<S> {
    fn save_file(&self, capability: &str, file: &AxIncomingFile) -> AxRuntimeResult<AxFileRef> {
        let saved = self
            .save_image(capability, file)
            .map_err(|error| match error {
                MediaUploadError::UnknownCapability => AxRuntimeError::Forbidden,
                MediaUploadError::InvalidImage(_) => AxRuntimeError::invalid_input("image"),
                MediaUploadError::Busy => {
                    AxRuntimeError::message("Media processing is busy; retry later")
                }
                MediaUploadError::Unavailable | MediaUploadError::Storage(_) => {
                    AxRuntimeError::message("Media storage is unavailable")
                }
            })?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::{ObjectKey, ObjectRef, StoreId};
    use sha2::{Digest, Sha256};
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[derive(Default)]
    struct TestStore {
        bytes: Mutex<Vec<u8>>,
        writes: AtomicUsize,
        fail: bool,
    }
    impl ObjectStorage for TestStore {
        fn put(&self, bytes: &[u8]) -> Result<StoredObject, StorageError> {
            self.writes.fetch_add(1, Ordering::SeqCst);
            if self.fail {
                return Err(StorageError::Io(std::io::Error::other("private/path")));
            }
            *self.bytes.lock().unwrap() = bytes.to_vec();
            Ok(StoredObject {
                reference: ObjectRef {
                    store: StoreId::parse("local-media").unwrap(),
                    key: ObjectKey::parse(&"a".repeat(64)).unwrap(),
                },
                byte_size: bytes.len() as u64,
                sha256: format!("{:x}", Sha256::digest(bytes)),
            })
        }
        fn get(&self, _reference: &ObjectRef) -> Result<Vec<u8>, StorageError> {
            Ok(self.bytes.lock().unwrap().clone())
        }
    }

    fn incoming(bytes: Vec<u8>) -> AxIncomingFile {
        AxIncomingFile {
            field_name: "image".into(),
            file_name: "../../private/<script>.svg".into(),
            content_type: Some("text/html".into()),
            bytes,
        }
    }

    fn png() -> Vec<u8> {
        use image::ImageEncoder;
        let mut bytes = Vec::new();
        image::codecs::png::PngEncoder::new(&mut bytes)
            .write_image(&[10, 20, 30, 255], 1, 1, image::ExtendedColorType::Rgba8)
            .unwrap();
        bytes
    }

    #[test]
    fn bridge_stores_processed_bytes_and_returns_only_trusted_file_metadata() {
        let storage = MediaUploadStorage::new(TestStore::default());
        let mut bytes = png();
        bytes.extend_from_slice(b"untrusted trailer");
        let file = incoming(bytes);
        let result = storage.save_file(MEDIA_CAPABILITY, &file).unwrap();
        let persisted = storage.store.bytes.lock().unwrap();
        assert_eq!(result.id, "a".repeat(64));
        assert_eq!(result.file_name, format!("{}.png", result.id));
        assert_eq!(result.storage, "local-media");
        assert_eq!(result.content_type.as_deref(), Some("image/png"));
        assert_eq!(result.size, persisted.len() as u64);
        assert_ne!(*persisted, file.bytes);
        assert_eq!(image::load_from_memory(&persisted).unwrap().width(), 1);
        assert_eq!(storage.store.writes.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn detailed_result_retains_processed_digest_and_dimensions_for_future_sql_metadata() {
        let storage = MediaUploadStorage::new(TestStore::default());
        let saved = storage
            .save_image(MEDIA_CAPABILITY, &incoming(png()))
            .unwrap();
        assert_eq!((saved.width, saved.height), (1, 1));
        assert_eq!(
            saved.object.sha256,
            format!(
                "{:x}",
                Sha256::digest(&*storage.store.bytes.lock().unwrap())
            )
        );
    }

    #[test]
    fn invalid_images_and_unknown_capabilities_cannot_write() {
        let storage = MediaUploadStorage::new(TestStore::default());
        assert!(matches!(
            storage.save_file("other", &incoming(png())),
            Err(AxRuntimeError::Forbidden)
        ));
        for bytes in [
            vec![],
            b"<svg/>".to_vec(),
            b"\x89PNG\r\n\x1a\nforged".to_vec(),
            vec![0; crate::storage::MAX_OBJECT_BYTES as usize + 1],
        ] {
            assert!(matches!(
                storage.save_file(MEDIA_CAPABILITY, &incoming(bytes)),
                Err(AxRuntimeError::InvalidInput { .. })
            ));
        }
        assert_eq!(storage.store.writes.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn busy_and_poisoned_processing_fail_closed_without_writes() {
        let storage = MediaUploadStorage::new(TestStore::default());
        let permit = storage.processing.lock().unwrap();
        assert!(matches!(
            storage.save_image(MEDIA_CAPABILITY, &incoming(png())),
            Err(MediaUploadError::Busy)
        ));
        drop(permit);
        std::thread::scope(|scope| {
            assert!(scope
                .spawn(|| {
                    let _permit = storage.processing.lock().unwrap();
                    panic!("test-only poison");
                })
                .join()
                .is_err());
        });
        assert!(matches!(
            storage.save_image(MEDIA_CAPABILITY, &incoming(png())),
            Err(MediaUploadError::Unavailable)
        ));
        assert_eq!(storage.store.writes.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn internal_storage_details_are_not_exposed_by_the_runtime_bridge() {
        let storage = MediaUploadStorage::new(TestStore {
            fail: true,
            ..TestStore::default()
        });
        let error = storage
            .save_file(MEDIA_CAPABILITY, &incoming(png()))
            .unwrap_err();
        assert_eq!(
            error.public_error_payload()["message"],
            "Media storage is unavailable"
        );
        assert!(!error.to_string().contains("private"));
    }
}
