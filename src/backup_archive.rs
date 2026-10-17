//! Experimental SQLite/media archives and inactive recovery to a new directory.
//! No schema upgrade, installation certification or application switch occurs.
use crate::storage::{
    ObjectKey, ObjectRef, ObjectStorage, StorageError, StoreId, StoredObject, MAX_OBJECT_BYTES,
};
use anyhow::{bail, ensure, Context, Result};
use rusqlite::{
    backup::{Backup, StepResult},
    Connection, OpenFlags,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs::{self, File, OpenOptions},
    io::{self, Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    sync::Mutex,
    time::{Duration, Instant},
};

const MAX_DATABASE_BYTES: u64 = 1024 * 1024 * 1024;
const MAX_ARCHIVE_BYTES: u64 = 8 * 1024 * 1024 * 1024;
const MAX_MANIFEST_BYTES: u64 = 4 * 1024 * 1024;
const MAX_OBJECTS: usize = 10_000;

struct SnapshotWorkspace(PathBuf);
impl SnapshotWorkspace {
    fn new() -> Result<Self> {
        let mut random = [0; 16];
        getrandom::getrandom(&mut random)
            .map_err(|_| anyhow::anyhow!("Snapshot randomness unavailable"))?;
        let root = std::env::temp_dir().join(format!(
            "legura-snapshot-{:032x}",
            u128::from_le_bytes(random)
        ));
        let builder = fs::DirBuilder::new();
        #[cfg(unix)]
        let builder = {
            use std::os::unix::fs::DirBuilderExt;
            let mut builder = builder;
            builder.mode(0o700);
            builder
        };
        builder.create(&root)?;
        Ok(Self(root))
    }
}
impl Drop for SnapshotWorkspace {
    fn drop(&mut self) {
        // Only known files inside our newly created private workspace. Never
        // recurse, follow links or remove an operator-provided directory.
        for suffix in ["", "-wal", "-shm", "-journal"] {
            let _ = fs::remove_file(self.0.join(format!("snapshot.sqlite{suffix}")));
        }
        let _ = fs::remove_dir(&self.0);
    }
}

fn private_file(path: &Path) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(path)
}

fn snapshot_live(source: &Path, target: &Path, budget: Duration) -> Result<()> {
    ensure!(
        fs::symlink_metadata(source)?.file_type().is_file(),
        "Database must be a regular file"
    );
    let deadline = Instant::now() + budget;
    let source = Connection::open_with_flags(source, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    source.busy_timeout(budget.min(Duration::from_secs(5)))?;
    source.execute_batch("PRAGMA trusted_schema = OFF; PRAGMA query_only = ON;")?;
    let pages: u64 = source.pragma_query_value(None, "page_count", |row| row.get(0))?;
    let page_size: u64 = source.pragma_query_value(None, "page_size", |row| row.get(0))?;
    ensure!(
        pages
            .checked_mul(page_size)
            .is_some_and(|bytes| bytes <= MAX_DATABASE_BYTES),
        "Database exceeds snapshot limit"
    );
    let file = private_file(target)?;
    let mut destination = Connection::open_with_flags(target, OpenFlags::SQLITE_OPEN_READ_WRITE)?;
    destination.busy_timeout(budget.min(Duration::from_secs(5)))?;
    {
        let backup = Backup::new(&source, &mut destination)?;
        loop {
            ensure!(
                Instant::now() < deadline,
                "SQLite snapshot timed out; retry during a quiet period"
            );
            let state = backup.step(256)?;
            ensure!(
                file.metadata()?.len() <= MAX_DATABASE_BYTES,
                "Database exceeds snapshot limit"
            );
            if matches!(state, StepResult::Done) {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    destination.pragma_update(None, "journal_mode", "DELETE")?;
    drop(destination);
    file.sync_all()?;
    Ok(())
}

/// Capture a consistent SQLite snapshot (including committed WAL pages), then
/// archive only that snapshot's immutable referenced objects. This is not a
/// complete CMS compatibility check or a restore implementation.
pub fn backup(
    database: &Path,
    storage: &impl ObjectStorage,
    store: &StoreId,
    target: &Path,
) -> Result<ArchiveReport> {
    match fs::symlink_metadata(target) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        _ => bail!("Archive destination must be new"),
    }
    let workspace = SnapshotWorkspace::new()?;
    let snapshot = workspace.0.join("snapshot.sqlite");
    snapshot_live(database, &snapshot, Duration::from_secs(60))?;
    pack_snapshot(&snapshot, storage, store, target)
}

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

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
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
    let output = private_file(target).context("Archive destination must be new")?;
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
    let mut file = open_archive(path)?;
    Ok(scan(&mut file, |_, _| Ok(()))?.1)
}

fn open_archive(path: &Path) -> Result<File> {
    ensure!(
        fs::symlink_metadata(path)?.file_type().is_file(),
        "Archive must be a regular file"
    );
    Ok(File::open(path)?)
}

// One parser owns validation and bounded payload consumption for both commands.
// Recovery scans the same open handle again and rechecks all copied hashes.
fn scan(
    file: &mut File,
    mut consume: impl FnMut(&str, &mut dyn Read) -> Result<()>,
) -> Result<(Manifest, ArchiveReport)> {
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
            consume(&path, &mut reader)?;
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
    let report = ArchiveReport {
        files: observed.len(),
        bytes: observed.values().map(|record| record.bytes).sum(),
    };
    Ok((manifest, report))
}

fn private_directory(path: &Path) -> io::Result<()> {
    let builder = fs::DirBuilder::new();
    #[cfg(unix)]
    let builder = {
        use std::os::unix::fs::DirBuilderExt;
        let mut builder = builder;
        builder.mode(0o700);
        builder
    };
    builder.create(path)
}

// Only track paths we created. Failure cleanup is nonrecursive and never opens
// or removes an existing destination directory, even when it is empty.
struct RecoveryWorkspace {
    root: PathBuf,
    files: Vec<PathBuf>,
    directories: Vec<PathBuf>,
    complete: bool,
}
impl RecoveryWorkspace {
    fn new(root: &Path) -> Result<Self> {
        private_directory(root).context("Recovery destination must be a new directory")?;
        Ok(Self {
            root: root.to_owned(),
            files: Vec::new(),
            directories: vec![root.to_owned()],
            complete: false,
        })
    }
    fn directory(&mut self, path: PathBuf) -> Result<()> {
        private_directory(&path)?;
        self.directories.push(path);
        Ok(())
    }
    fn copy(&mut self, relative: &str, input: &mut dyn Read) -> Result<()> {
        let path = self.root.join(relative);
        let mut output = private_file(&path)?;
        self.files.push(path);
        io::copy(input, &mut output)?;
        output.sync_all()?;
        Ok(())
    }
}
impl Drop for RecoveryWorkspace {
    fn drop(&mut self) {
        if !self.complete {
            for file in self.files.iter().rev() {
                let _ = fs::remove_file(file);
            }
            for directory in self.directories.iter().rev() {
                let _ = fs::remove_dir(directory);
            }
        }
    }
}

/// Recover a trusted operator bundle into NEW private paths without activating
/// it. Validates SQLite integrity/FKs and exact media references, not the full
/// CMS migration history. A separate compatibility gate must precede startup.
pub fn restore(archive: &Path, destination: &Path) -> Result<ArchiveReport> {
    let mut file = open_archive(archive)?;
    let (expected, _) = scan(&mut file, |_, _| Ok(()))?;
    let store = StoreId::parse(&expected.store)?;
    let mut workspace = RecoveryWorkspace::new(destination)?;
    workspace.directory(destination.join("media"))?;
    let media_root = destination.join("media").join(store.as_str());
    workspace.directory(media_root.clone())?;
    file.rewind()?;
    let (manifest, report) = scan(&mut file, |path, input| workspace.copy(path, input))?;
    ensure!(
        manifest.files == expected.files && manifest.store == expected.store,
        "Archive changed during recovery"
    );
    let database = Connection::open_with_flags(
        destination.join("database.sqlite"),
        OpenFlags::SQLITE_OPEN_READ_ONLY,
    )?;
    database.execute_batch("PRAGMA trusted_schema = OFF; PRAGMA query_only = ON;")?;
    ensure!(
        database.query_row("PRAGMA integrity_check", [], |row| row.get::<_, String>(0))? == "ok",
        "Recovered database integrity check failed"
    );
    ensure!(
        !database.prepare("PRAGMA foreign_key_check")?.exists([])?,
        "Recovered database has foreign key violations"
    );
    let storage = crate::storage::LocalStorage::open(&media_root, store.clone(), MAX_OBJECT_BYTES)?;
    ensure!(
        database.query_row(
            "SELECT COUNT(*) = COUNT(DISTINCT id) FROM legura_media",
            [],
            |row| row.get::<_, bool>(0),
        )?,
        "Recovered media identifiers must be unique"
    );
    let inventory = crate::media_audit::audit(&database, &storage, &store)?;
    ensure!(
        inventory.objects == (manifest.files.len() - 1) as u64,
        "Archive contains unreferenced media objects"
    );
    drop(storage);
    drop(database);
    // This receipt certifies inactive data recovery only, never CMS startup.
    let receipt = serde_json::to_vec(&serde_json::json!({
        "format": 1,
        "status": "recovered-not-activated",
        "cms_compatibility": "not-checked",
        "store": store.as_str(),
        "inventory": report,
    }))?;
    workspace.copy("recovery.json", &mut receipt.as_slice())?;
    workspace.complete = true;
    Ok(report)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RecoveryReceipt {
    format: u32,
    status: String,
    cms_compatibility: String,
    store: String,
    inventory: ArchiveReport,
}

/// Read-only, point-in-time compatibility check against the original archive.
/// Never upgrades the database, rewrites the receipt or activates a service.
pub fn check_recovery(archive: &Path, root: &Path) -> Result<ArchiveReport> {
    // Strip trailing separators/`.` before lstat so a final symlink cannot be
    // treated as a directory merely because the supplied path ends in `/`.
    let root = root.components().collect::<PathBuf>();
    let root = root.as_path();
    let mut file = open_archive(archive)?;
    let (manifest, report) = scan(&mut file, |_, _| Ok(()))?;
    for path in [
        root.to_owned(),
        root.join("media"),
        root.join("media").join(&manifest.store),
    ] {
        ensure!(
            fs::symlink_metadata(path)?.file_type().is_dir(),
            "Recovery directories must not be symlinks"
        );
    }
    let receipt_path = root.join("recovery.json");
    ensure!(
        fs::symlink_metadata(&receipt_path)?.file_type().is_file(),
        "Recovery receipt must be a regular file"
    );
    let receipt_file = File::open(receipt_path)?;
    ensure!(
        receipt_file.metadata()?.len() <= 4096,
        "Recovery receipt exceeds limit"
    );
    let receipt: RecoveryReceipt = serde_json::from_reader(receipt_file.take(4097))?;
    ensure!(
        receipt.format == 1
            && receipt.status == "recovered-not-activated"
            && receipt.cms_compatibility == "not-checked"
            && receipt.store == manifest.store
            && receipt.inventory == report,
        "Recovery receipt does not match archive"
    );
    for (path, expected) in &manifest.files {
        let path = root.join(path);
        ensure!(
            fs::symlink_metadata(&path)?.file_type().is_file(),
            "Recovered payload must be a regular file"
        );
        let file = File::open(path)?;
        ensure!(
            file.metadata()?.len() == expected.bytes,
            "Recovered payload size changed"
        );
        let mut reader = HashReader {
            inner: file.take(expected.bytes + 1),
            hash: Sha256::new(),
            bytes: 0,
        };
        io::copy(&mut reader, &mut io::sink())?;
        ensure!(
            reader.bytes == expected.bytes
                && format!("{:x}", reader.hash.finalize()) == expected.sha256,
            "Recovered payload checksum changed"
        );
    }
    let database = Connection::open_with_flags(
        root.join("database.sqlite"),
        OpenFlags::SQLITE_OPEN_READ_ONLY,
    )?;
    database.execute_batch("PRAGMA trusted_schema = OFF; PRAGMA query_only = ON;")?;
    crate::database_contract::validate_current(&database)?;
    let store = StoreId::parse(&manifest.store)?;
    let storage = crate::storage::LocalStorage::open(
        &root.join("media").join(store.as_str()),
        store.clone(),
        MAX_OBJECT_BYTES,
    )?;
    let inventory = crate::media_audit::audit(&database, &storage, &store)?;
    ensure!(
        inventory.objects == (report.files - 1) as u64,
        "Recovered media inventory differs from archive"
    );
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn temporary_snapshot_is_private_cleaned_and_timeout_never_becomes_success() {
        let workspace = SnapshotWorkspace::new().unwrap();
        let root = workspace.0.clone();
        let source = root.join("source.sqlite");
        let connection = Connection::open(&source).unwrap();
        connection
            .execute_batch("CREATE TABLE marker(value TEXT); INSERT INTO marker VALUES ('keep')")
            .unwrap();
        drop(connection);
        let target = root.join("snapshot.sqlite");
        assert!(snapshot_live(&source, &target, Duration::ZERO).is_err());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(&root).unwrap().permissions().mode() & 0o777,
                0o700
            );
            assert_eq!(
                fs::metadata(&target).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
        fs::remove_file(&source).unwrap();
        drop(workspace);
        assert!(!root.exists());
    }
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
