//! Bounded, immutable object storage. This module grants no HTTP upload authority.
use cap_fs_ext::{FollowSymlinks, OpenOptionsFollowExt, OpenOptionsSyncExt};
use cap_std::fs::{Dir, OpenOptions};
use sha2::{Digest, Sha256};
use std::fmt;
use std::io::{self, Read, Write};
use std::path::Path;

pub const MAX_OBJECT_BYTES: u64 = 8 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoreId(String);

impl StoreId {
    pub fn parse(value: &str) -> Result<Self, StorageError> {
        if value.is_empty()
            || value.len() > 64
            || !value
                .bytes()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-')
        {
            return Err(StorageError::InvalidReference);
        }
        Ok(Self(value.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ObjectKey(String);

impl ObjectKey {
    pub fn parse(value: &str) -> Result<Self, StorageError> {
        if value.len() != 64
            || !value
                .bytes()
                .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
        {
            return Err(StorageError::InvalidReference);
        }
        Ok(Self(value.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    fn random() -> Result<Self, StorageError> {
        let mut bytes = [0; 32];
        getrandom::getrandom(&mut bytes).map_err(|_| StorageError::Randomness)?;
        Ok(Self(hex(&bytes)))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ObjectRef {
    pub store: StoreId,
    pub key: ObjectKey,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoredObject {
    pub reference: ObjectRef,
    pub byte_size: u64,
    pub sha256: String,
}

#[derive(Debug)]
pub enum StorageError {
    InvalidReference,
    InvalidLimit,
    WrongStore,
    Empty,
    TooLarge,
    NotFound,
    NotRegularFile,
    Randomness,
    Io(io::Error),
}

impl fmt::Display for StorageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::InvalidReference => "invalid storage reference",
            Self::InvalidLimit => "invalid storage object limit",
            Self::WrongStore => "storage reference belongs to another store",
            Self::Empty => "storage object is empty",
            Self::TooLarge => "storage object exceeds size limit",
            Self::NotFound => "storage object not found",
            Self::NotRegularFile => "storage object is not a regular file",
            Self::Randomness => "storage key generation failed",
            Self::Io(_) => "storage operation failed",
        };
        f.write_str(message)
    }
}

impl std::error::Error for StorageError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            _ => None,
        }
    }
}

/// Small-object synchronous boundary. Callers must enforce authorization and
/// concurrency limits; network adapters and streaming are not implemented here.
pub trait ObjectStorage: Send + Sync {
    fn put(&self, bytes: &[u8]) -> Result<StoredObject, StorageError>;
    fn get(&self, reference: &ObjectRef) -> Result<Vec<u8>, StorageError>;
}

pub struct LocalStorage {
    directory: Dir,
    id: StoreId,
    limit: u64,
}

impl LocalStorage {
    /// The operator provisions an existing private directory. It is never
    /// created from request input, and ambient authority is acquired only here.
    pub fn open(root: &Path, id: StoreId, limit: u64) -> Result<Self, StorageError> {
        if limit == 0 || limit > MAX_OBJECT_BYTES {
            return Err(StorageError::InvalidLimit);
        }
        let directory =
            Dir::open_ambient_dir(root, cap_std::ambient_authority()).map_err(StorageError::Io)?;
        Ok(Self {
            directory,
            id,
            limit,
        })
    }

    fn put_at(&self, key: ObjectKey, bytes: &[u8]) -> Result<StoredObject, StorageError> {
        if bytes.is_empty() {
            return Err(StorageError::Empty);
        }
        if bytes.len() as u64 > self.limit {
            return Err(StorageError::TooLarge);
        }
        let mut options = OpenOptions::new();
        options
            .write(true)
            .create_new(true)
            .follow(FollowSymlinks::No);
        let mut file = self
            .directory
            .open_with(key.as_str(), &options)
            .map_err(StorageError::Io)?;
        if let Err(error) = file.write_all(bytes).and_then(|()| file.sync_all()) {
            drop(file);
            // Only our newly created object can be removed. Cleanup failure may
            // leave an unreferenced object; no successful reference is returned.
            let _ = self.directory.remove_file(key.as_str());
            return Err(StorageError::Io(error));
        }
        Ok(StoredObject {
            reference: ObjectRef {
                store: self.id.clone(),
                key,
            },
            byte_size: bytes.len() as u64,
            sha256: hex(&Sha256::digest(bytes)),
        })
    }
}

impl ObjectStorage for LocalStorage {
    fn put(&self, bytes: &[u8]) -> Result<StoredObject, StorageError> {
        self.put_at(ObjectKey::random()?, bytes)
    }

    fn get(&self, reference: &ObjectRef) -> Result<Vec<u8>, StorageError> {
        if reference.store != self.id {
            return Err(StorageError::WrongStore);
        }
        let mut options = OpenOptions::new();
        options.read(true).follow(FollowSymlinks::No).nonblock(true);
        let file = self
            .directory
            .open_with(reference.key.as_str(), &options)
            .map_err(|error| {
                if error.kind() == io::ErrorKind::NotFound {
                    StorageError::NotFound
                } else {
                    StorageError::Io(error)
                }
            })?;
        let metadata = file.metadata().map_err(StorageError::Io)?;
        if !metadata.is_file() {
            return Err(StorageError::NotRegularFile);
        }
        if metadata.len() > self.limit {
            return Err(StorageError::TooLarge);
        }
        let mut bytes = Vec::new();
        file.take(self.limit + 1)
            .read_to_end(&mut bytes)
            .map_err(StorageError::Io)?;
        if bytes.len() as u64 > self.limit {
            return Err(StorageError::TooLarge);
        }
        if bytes.is_empty() {
            return Err(StorageError::Empty);
        }
        Ok(bytes)
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|value| format!("{value:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "legura-storage-{}",
                ObjectKey::random().unwrap().as_str()
            ));
            fs::create_dir(&root).unwrap();
            Self(root)
        }
        fn store(&self) -> LocalStorage {
            LocalStorage::open(&self.0, StoreId::parse("primary").unwrap(), 16).unwrap()
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }

    #[test]
    fn rejects_paths_urls_and_noncanonical_keys() {
        for value in [
            "",
            "../secret",
            "/tmp/object",
            "C:\\secret",
            "https://host/key",
            &"A".repeat(64),
            &"a".repeat(63),
            &"a".repeat(65),
        ] {
            assert!(ObjectKey::parse(value).is_err(), "{value}");
        }
        for value in ["", "../primary", "Primary", "s3://bucket", &"a".repeat(65)] {
            assert!(StoreId::parse(value).is_err());
        }
        assert_eq!(
            ObjectKey::parse(&"a".repeat(64)).unwrap().as_str(),
            "a".repeat(64)
        );
    }

    #[test]
    fn roundtrip_survives_reopen_with_provider_independent_metadata() {
        let fixture = Fixture::new();
        let object = fixture.store().put(b"abc").unwrap();
        assert_eq!(object.byte_size, 3);
        assert_eq!(
            object.sha256,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(fixture.store().get(&object.reference).unwrap(), b"abc");
        let adapter: Box<dyn ObjectStorage> = Box::new(fixture.store());
        assert_eq!(adapter.get(&object.reference).unwrap(), b"abc");
    }

    #[test]
    fn size_limits_do_not_create_rejected_objects() {
        let fixture = Fixture::new();
        let store = fixture.store();
        assert!(matches!(store.put(b""), Err(StorageError::Empty)));
        assert!(matches!(store.put(&[1; 17]), Err(StorageError::TooLarge)));
        assert_eq!(fs::read_dir(&fixture.0).unwrap().count(), 0);
        let object = store.put(&[1; 16]).unwrap();
        assert_eq!(store.get(&object.reference).unwrap(), [1; 16]);
        fs::write(fixture.0.join(object.reference.key.as_str()), [1; 17]).unwrap();
        assert!(matches!(
            store.get(&object.reference),
            Err(StorageError::TooLarge)
        ));
    }

    #[test]
    fn never_overwrites_a_collision_or_trusts_a_store_mismatch() {
        let fixture = Fixture::new();
        let store = fixture.store();
        let key = ObjectKey::parse(&"b".repeat(64)).unwrap();
        let object = store.put_at(key.clone(), b"original").unwrap();
        assert!(store.put_at(key, b"changed").is_err());
        assert_eq!(store.get(&object.reference).unwrap(), b"original");
        let wrong = ObjectRef {
            store: StoreId::parse("other").unwrap(),
            ..object.reference
        };
        assert!(matches!(store.get(&wrong), Err(StorageError::WrongStore)));
    }

    #[test]
    fn concurrent_puts_use_distinct_immutable_keys() {
        let fixture = Fixture::new();
        let store = std::sync::Arc::new(fixture.store());
        let handles: Vec<_> = (0..16)
            .map(|value| {
                let store = store.clone();
                std::thread::spawn(move || (value, store.put(&[value]).unwrap()))
            })
            .collect();
        let mut keys = std::collections::BTreeSet::new();
        for handle in handles {
            let (value, object) = handle.join().unwrap();
            assert!(keys.insert(object.reference.key.as_str().to_owned()));
            assert_eq!(store.get(&object.reference).unwrap(), [value]);
        }
        assert_eq!(fs::read_dir(&fixture.0).unwrap().count(), 16);
    }

    #[test]
    fn missing_empty_directory_and_invalid_configuration_fail() {
        let fixture = Fixture::new();
        let store = fixture.store();
        let key = ObjectKey::parse(&"c".repeat(64)).unwrap();
        let reference = ObjectRef {
            store: StoreId::parse("primary").unwrap(),
            key,
        };
        assert!(matches!(store.get(&reference), Err(StorageError::NotFound)));
        fs::write(fixture.0.join(reference.key.as_str()), []).unwrap();
        assert!(matches!(store.get(&reference), Err(StorageError::Empty)));
        fs::remove_file(fixture.0.join(reference.key.as_str())).unwrap();
        fs::create_dir(fixture.0.join(reference.key.as_str())).unwrap();
        assert!(store.get(&reference).is_err());
        for limit in [0, MAX_OBJECT_BYTES + 1] {
            assert!(matches!(
                LocalStorage::open(&fixture.0, StoreId::parse("primary").unwrap(), limit),
                Err(StorageError::InvalidLimit)
            ));
        }
        assert!(LocalStorage::open(
            &fixture.0.join("missing"),
            StoreId::parse("primary").unwrap(),
            16
        )
        .is_err());
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlinks_inside_and_outside_the_capability() {
        let fixture = Fixture::new();
        let other = Fixture::new();
        fs::write(other.0.join("secret"), b"outside").unwrap();
        let store = fixture.store();
        let object = store.put(b"inside").unwrap();
        for (name, target) in [
            ("d".repeat(64), other.0.join("secret")),
            (
                "e".repeat(64),
                fixture.0.join(object.reference.key.as_str()),
            ),
        ] {
            std::os::unix::fs::symlink(target, fixture.0.join(&name)).unwrap();
            let key = ObjectKey::parse(&name).unwrap();
            let reference = ObjectRef {
                store: StoreId::parse("primary").unwrap(),
                key: key.clone(),
            };
            assert!(store.get(&reference).is_err());
            assert!(store.put_at(key, b"changed").is_err());
        }
        assert_eq!(fs::read(other.0.join("secret")).unwrap(), b"outside");
        assert_eq!(store.get(&object.reference).unwrap(), b"inside");
    }
}
