//! Experimental container for a caller-provided frozen SQLite/media snapshot.
//! No extraction, schema upgrade, live snapshot or application switch occurs.
use crate::storage::{
    ObjectKey, ObjectRef, ObjectStorage, StorageError, StoreId, StoredObject, MAX_OBJECT_BYTES,
};
use anyhow::{bail, ensure, Context, Result};
use rusqlite::{Connection, OpenFlags};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs::{self, File, OpenOptions},
    io::{self, Read, Seek, SeekFrom},
    path::Path,
    sync::Mutex,
};

const MAX_DATABASE_BYTES: u64 = 1024 * 1024 * 1024;
const MAX_ARCHIVE_BYTES: u64 = 8 * 1024 * 1024 * 1024;
const MAX_MANIFEST_BYTES: u64 = 4 * 1024 * 1024;
const MAX_OBJECTS: usize = 10_000;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FileRecord {
    bytes: u64,
    sha256: String,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    format: u32,
    product: String,
    version: String,
    store: String,
    #[serde(deserialize_with = "unique_files")]
    files: BTreeMap<String, FileRecord>,
}

fn unique_files<'de, D: serde::Deserializer<'de>>(
    input: D,
) -> std::result::Result<BTreeMap<String, FileRecord>, D::Error> {
    struct Unique;
    impl<'de> serde::de::Visitor<'de> for Unique {
        type Value = BTreeMap<String, FileRecord>;
        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("a bounded inventory with unique paths")
        }
        fn visit_map<M: serde::de::MapAccess<'de>>(
            self,
            mut map: M,
        ) -> std::result::Result<Self::Value, M::Error> {
            let mut files = BTreeMap::new();
            while let Some((path, record)) = map.next_entry::<String, FileRecord>()? {
                if files.len() > MAX_OBJECTS || files.insert(path, record).is_some() {
                    return Err(serde::de::Error::custom(
                        "Duplicate or excessive manifest inventory",
                    ));
                }
            }
            Ok(files)
        }
    }
    input.deserialize_map(Unique)
}

#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct ArchiveReport {
    pub files: usize,
    pub bytes: u64,
}

struct HashReader<R> {
    inner: R,
    hash: Sha256,
    bytes: u64,
}
impl<R: Read> Read for HashReader<R> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        let count = self.inner.read(buffer)?;
        self.hash.update(&buffer[..count]);
        self.bytes += count as u64;
        Ok(count)
    }
}

fn append<R: Read>(
    builder: &mut tar::Builder<File>,
    path: &str,
    size: u64,
    input: R,
) -> io::Result<FileRecord> {
    let mut header = tar::Header::new_ustar();
    header.set_entry_type(tar::EntryType::Regular);
    header.set_size(size);
    header.set_mode(0o600);
    header.set_mtime(0);
    header.set_uid(0);
    header.set_gid(0);
    let mut reader = HashReader {
        inner: input,
        hash: Sha256::new(),
        bytes: 0,
    };
    builder.append_data(&mut header, path, &mut reader)?;
    if reader.bytes != size {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Archive input changed size",
        ));
    }
    Ok(FileRecord {
        bytes: size,
        sha256: format!("{:x}", reader.hash.finalize()),
    })
}

struct Writer {
    builder: tar::Builder<File>,
    files: BTreeMap<String, FileRecord>,
    bytes: u64,
}

// The audit hashes the exact returned bytes. Any mismatch leaves an incomplete
// archive, never a successful package; the manifest is appended only afterward.
struct CopyingStore<'a, S> {
    source: &'a S,
    writer: Mutex<Writer>,
}
impl<S: ObjectStorage> ObjectStorage for CopyingStore<'_, S> {
    fn put(&self, _: &[u8]) -> Result<StoredObject, StorageError> {
        Err(StorageError::InvalidReference)
    }
    fn get(&self, reference: &ObjectRef) -> Result<Vec<u8>, StorageError> {
        let bytes = self.source.get(reference)?;
        let mut writer = self
            .writer
            .lock()
            .map_err(|_| StorageError::Io(io::Error::other("Archive writer unavailable")))?;
        let path = format!(
            "media/{}/{}",
            reference.store.as_str(),
            reference.key.as_str()
        );
        if bytes.len() as u64 > MAX_OBJECT_BYTES
            || writer.files.len() > MAX_OBJECTS
            || writer.bytes + bytes.len() as u64 > MAX_ARCHIVE_BYTES
            || writer.files.contains_key(&path)
        {
            return Err(StorageError::TooLarge);
        }
        let record = append(
            &mut writer.builder,
            &path,
            bytes.len() as u64,
            bytes.as_slice(),
        )
        .map_err(StorageError::Io)?;
        writer.bytes += record.bytes;
        writer.files.insert(path, record);
        Ok(bytes)
    }
}

/// The source must be a trusted, standalone, immutable SQLite snapshot. Never
/// pass a live DB file. Its referenced objects must remain immutable as well.
pub fn pack_snapshot(
    snapshot: &Path,
    storage: &impl ObjectStorage,
    store: &StoreId,
    target: &Path,
) -> Result<ArchiveReport> {
    ensure!(
        fs::symlink_metadata(snapshot)?.file_type().is_file(),
        "Snapshot must be a regular file"
    );
    for suffix in ["-wal", "-shm", "-journal"] {
        let mut name = snapshot.as_os_str().to_os_string();
        name.push(suffix);
        match fs::symlink_metadata(Path::new(&name)) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            _ => bail!("Snapshot has a SQLite sidecar or cannot be inspected"),
        }
    }
    let source = File::open(snapshot)?;
    let size = source.metadata()?.len();
    ensure!(
        size > 0 && size <= MAX_DATABASE_BYTES,
        "Snapshot exceeds container limits"
    );
    let database = Connection::open_with_flags(snapshot, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    database.execute_batch("PRAGMA trusted_schema = OFF; PRAGMA query_only = ON;")?;
    ensure!(
        database.query_row("PRAGMA integrity_check", [], |row| row.get::<_, String>(0))? == "ok",
        "Snapshot integrity check failed"
    );
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let output = options
        .open(target)
        .context("Archive destination must be new")?;
    let result = (|| {
        let mut writer = Writer {
            builder: tar::Builder::new(output),
            files: BTreeMap::new(),
            bytes: size,
        };
        let record = append(&mut writer.builder, "database.sqlite", size, source)?;
        writer.files.insert("database.sqlite".into(), record);
        let copying = CopyingStore {
            source: storage,
            writer: Mutex::new(writer),
        };
        crate::media_audit::audit(&database, &copying, store)?;
        let mut writer = copying
            .writer
            .into_inner()
            .map_err(|_| anyhow::anyhow!("Archive writer unavailable"))?;
        let manifest = serde_json::to_vec(&Manifest {
            format: 1,
            product: "legura".into(),
            version: env!("CARGO_PKG_VERSION").into(),
            store: store.as_str().into(),
            files: writer.files,
        })?;
        ensure!(
            manifest.len() as u64 <= MAX_MANIFEST_BYTES,
            "Archive manifest exceeds limit"
        );
        append(
            &mut writer.builder,
            "manifest.json",
            manifest.len() as u64,
            manifest.as_slice(),
        )?;
        writer.builder.finish()?;
        writer.builder.into_inner()?.sync_all()?;
        verify(target)
    })();
    if result.is_err() {
        let _ = fs::remove_file(target);
    }
    result
}

fn path_limit(path: &str) -> Result<u64> {
    match path {
        "database.sqlite" => Ok(MAX_DATABASE_BYTES),
        "manifest.json" => Ok(MAX_MANIFEST_BYTES),
        _ => {
            let parts: Vec<_> = path.split('/').collect();
            ensure!(
                parts.len() == 3 && parts[0] == "media",
                "Unsupported archive path"
            );
            StoreId::parse(parts[1])?;
            ObjectKey::parse(parts[2])?;
            Ok(MAX_OBJECT_BYTES)
        }
    }
}

/// Validate the container without unpacking anything. Hashes detect corruption,
/// not an attacker replacing both payload and manifest. Not schema compatibility.
pub fn verify(path: &Path) -> Result<ArchiveReport> {
    ensure!(
        fs::symlink_metadata(path)?.file_type().is_file(),
        "Archive must be a regular file"
    );
    let mut file = File::open(path)?;
    let length = file.metadata()?.len();
    ensure!(
        (1024..=MAX_ARCHIVE_BYTES).contains(&length) && length % 512 == 0,
        "Archive size is invalid"
    );
    file.seek(SeekFrom::End(-1024))?;
    let mut trailer = [0; 1024];
    file.read_exact(&mut trailer)?;
    ensure!(trailer == [0; 1024], "Archive is incomplete");
    file.rewind()?;
    let mut archive = tar::Archive::new(file);
    // Inspect concatenated/hidden entries rather than stopping at the first TAR terminator.
    archive.set_ignore_zeros(true);
    let mut observed = BTreeMap::new();
    let mut total = 0u64;
    let mut manifest = None;
    for entry in archive.entries()?.raw(true) {
        let mut entry = entry?;
        ensure!(manifest.is_none(), "Unexpected data after manifest");
        ensure!(
            entry.header().entry_type() == tar::EntryType::Regular
                && entry.header().as_ustar().is_some(),
            "Only regular USTAR entries are supported"
        );
        ensure!(
            entry.link_name_bytes().is_none(),
            "Archive links are not supported"
        );
        let path = std::str::from_utf8(&entry.path_bytes())?.to_owned();
        let limit = path_limit(&path)?;
        ensure!(!observed.contains_key(&path), "Duplicate archive entry");
        ensure!(
            !observed.is_empty() || path == "database.sqlite",
            "Database must be the first archive entry"
        );
        ensure!(
            entry.size() > 0 && entry.size() <= limit,
            "Archive entry exceeds limit"
        );
        total = total
            .checked_add(entry.size())
            .context("Archive size overflow")?;
        ensure!(
            total <= MAX_ARCHIVE_BYTES && observed.len() <= MAX_OBJECTS + 1,
            "Archive exceeds limits"
        );
        if path == "manifest.json" {
            let mut bytes = Vec::new();
            entry.read_to_end(&mut bytes)?;
            manifest = Some(serde_json::from_slice::<Manifest>(&bytes)?);
        } else {
            let declared = entry.size();
            let mut reader = HashReader {
                inner: entry,
                hash: Sha256::new(),
                bytes: 0,
            };
            io::copy(&mut reader, &mut io::sink())?;
            ensure!(reader.bytes == declared, "Truncated archive entry");
            observed.insert(
                path,
                FileRecord {
                    bytes: reader.bytes,
                    sha256: format!("{:x}", reader.hash.finalize()),
                },
            );
        }
    }
    let manifest = manifest.context("Archive manifest is missing")?;
    ensure!(
        manifest.format == 1
            && manifest.product == "legura"
            && manifest.version == env!("CARGO_PKG_VERSION"),
        "Unsupported archive contract"
    );
    StoreId::parse(&manifest.store)?;
    ensure!(
        observed == manifest.files,
        "Archive inventory or checksum mismatch"
    );
    let prefix = format!("media/{}/", manifest.store);
    ensure!(
        observed
            .keys()
            .all(|path| path == "database.sqlite" || path.starts_with(&prefix)),
        "Archive contains an unconfigured store"
    );
    Ok(ArchiveReport {
        files: observed.len(),
        bytes: observed.values().map(|record| record.bytes).sum(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn manifest_rejects_duplicate_inventory_paths_and_unknown_fields() {
        let record = r#"{"bytes":1,"sha256":"a"}"#;
        let json = format!(
            r#"{{"format":1,"product":"legura","version":"0.0.1","store":"local-media","files":{{"database.sqlite":{record},"database.sqlite":{record}}}}}"#
        );
        assert!(serde_json::from_str::<Manifest>(&json).is_err());
        let json = r#"{"format":1,"product":"legura","version":"0.0.1","store":"local-media","files":{},"extra":true}"#;
        assert!(serde_json::from_str::<Manifest>(json).is_err());
    }
}
