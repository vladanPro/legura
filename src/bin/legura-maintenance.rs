use anyhow::{bail, ensure, Context, Result};
use axonyx_runtime::backend::{runtime_from_env, AxEnv, AxMigration, AxMigrationExecutor};
use rusqlite::{
    backup::{Backup, StepResult},
    Connection, OpenFlags,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    env, fs,
    io::Read,
    path::Path,
    time::{Duration, Instant},
};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    format: u32,
    product: String,
    version: String,
    database_sha256: String,
}

const MIGRATIONS: &[(&str, &str, &str)] = &[
    (
        "20261008000000_001",
        include_str!("../../db/migrations/20261008000000_001_identity/up.sql"),
        include_str!("../../db/migrations/20261008000000_001_identity/down.sql"),
    ),
    (
        "20261009000000_002",
        include_str!("../../db/migrations/20261009000000_002_posts/up.sql"),
        include_str!("../../db/migrations/20261009000000_002_posts/down.sql"),
    ),
];

fn main() {
    if let Err(error) = run() {
        eprintln!("Legura maintenance failed: {error:#}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let args: Vec<_> = env::args_os().skip(1).collect();
    match args.as_slice() {
        [command, destination] if command == "init" => {
            initialize(Path::new(destination))?;
            println!("Empty database initialized with compatible migrations. Configure independent secrets before starting browser setup.");
        }
        [command, source, destination] if command == "backup" => {
            backup(Path::new(source), Path::new(destination))?;
            println!("Backup verified. Protect this directory: it contains private content and password hashes.");
        }
        [command, bundle] if command == "verify" => {
            verify(Path::new(bundle))?;
            println!("Backup integrity and migration compatibility verified.");
        }
        [command, bundle, destination] if command == "restore" => {
            restore(Path::new(bundle), Path::new(destination))?;
            println!("Restored to a NEW database. Stop the CMS before switching its database configuration; rotate the session key.");
        }
        _ => bail!("usage: legura-maintenance init <new-database-file> | backup <database-file> <new-backup-directory> | verify <backup-directory> | restore <backup-directory> <new-database-file>"),
    }
    Ok(())
}

fn regular_file(path: &Path) -> Result<()> {
    ensure!(
        fs::symlink_metadata(path)?.file_type().is_file(),
        "expected a regular file, not a symlink"
    );
    Ok(())
}

fn open_readonly(path: &Path) -> Result<Connection> {
    regular_file(path)?;
    let connection = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    connection.busy_timeout(Duration::from_secs(5))?;
    connection.execute_batch("PRAGMA trusted_schema = OFF;")?;
    Ok(connection)
}

fn migration_hash(up: &str, down: &str) -> String {
    let mut hash = Sha256::new();
    hash.update(b"axonyx-migration-v1\0");
    hash.update(up.replace("\r\n", "\n"));
    hash.update(b"\0");
    hash.update(down.replace("\r\n", "\n"));
    format!("{:x}", hash.finalize())
}

fn embedded_migrations() -> Vec<AxMigration> {
    MIGRATIONS
        .iter()
        .map(|(version, up, down)| AxMigration {
            version: (*version).into(),
            name: match *version {
                "20261008000000_001" => "identity",
                "20261009000000_002" => "posts",
                _ => unreachable!("embedded migration name is missing"),
            }
            .into(),
            checksum: migration_hash(up, down),
            up_sql: (*up).into(),
            down_sql: (*down).into(),
        })
        .collect()
}

fn initialize(path: &Path) -> Result<()> {
    initialize_with_migrations(path, &embedded_migrations())
}

fn initialize_with_migrations(path: &Path, migrations: &[AxMigration]) -> Result<()> {
    let path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        env::current_dir()?.join(path)
    };
    let url = format!(
        "sqlite:{}",
        path.to_str().context("database path must be UTF-8")?
    );
    reject_destination_sidecars(&path)?;
    // Reserve only a NEW file. Never infer a database from process env or .env.
    let file = private_file(&path)?;
    let result = (|| {
        let env = AxEnv::new()
            .with_secret("db_url", url)
            .with_secret("db_dialect", "sqlite");
        let runtime = runtime_from_env(env)?;
        AxMigrationExecutor::apply_migrations(&runtime, migrations)?;
        validate(&open_readonly(&path)?)?;
        file.sync_all()?;
        Ok(())
    })();
    drop(file);
    if result.is_err() {
        // Only our newly reserved file is removed; existing destinations never reach here.
        let _ = fs::remove_file(&path);
    }
    result
}

fn validate(connection: &Connection) -> Result<()> {
    let integrity: String = connection.query_row("PRAGMA integrity_check", [], |row| row.get(0))?;
    ensure!(integrity == "ok", "SQLite integrity check failed");
    ensure!(
        !connection
            .prepare("PRAGMA foreign_key_check")?
            .query([])?
            .next()?
            .is_some(),
        "foreign-key integrity check failed"
    );
    for sql in [
        "SELECT id, email, role FROM legura_users LIMIT 0",
        "SELECT user_id, email, password_hash FROM legura_credentials LIMIT 0",
        "SELECT id, site_name, admin_id FROM legura_installation LIMIT 0",
        "SELECT id, title, slug, body, status FROM legura_posts LIMIT 0",
    ] {
        connection.prepare(sql)?;
    }
    let count: usize =
        connection.query_row("SELECT count(*) FROM _axonyx_migrations", [], |row| {
            row.get(0)
        })?;
    ensure!(count == MIGRATIONS.len(), "unsupported migration history");
    for (version, up, down) in MIGRATIONS {
        let checksum: String = connection.query_row(
            "SELECT checksum FROM _axonyx_migrations WHERE version = ?1",
            [version],
            |row| row.get(0),
        )?;
        ensure!(
            checksum == migration_hash(up, down),
            "migration checksum does not match this maintenance build"
        );
    }
    Ok(())
}

fn digest(path: &Path) -> Result<String> {
    regular_file(path)?;
    let mut file = fs::File::open(path)?;
    let mut hash = Sha256::new();
    let mut buffer = [0; 65536];
    loop {
        let size = file.read(&mut buffer)?;
        if size == 0 {
            break;
        }
        hash.update(&buffer[..size]);
    }
    Ok(format!("{:x}", hash.finalize()))
}

fn private_file(path: &Path) -> Result<fs::File> {
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    Ok(options.open(path)?)
}

// SQLite's backup API includes committed WAL data; copying the .db file does not.
fn reject_destination_sidecars(target: &Path) -> Result<()> {
    for suffix in ["-wal", "-shm", "-journal"] {
        let sidecar = target.with_file_name(format!(
            "{}{suffix}",
            target
                .file_name()
                .context("database filename is missing")?
                .to_string_lossy()
        ));
        ensure!(
            fs::symlink_metadata(sidecar).is_err(),
            "destination has a SQLite sidecar; choose a different filename"
        );
    }
    Ok(())
}

fn snapshot(source: &Connection, target: &Path) -> Result<()> {
    reject_destination_sidecars(target)?;
    let file = private_file(target)?;
    let result = (|| {
        let mut destination = Connection::open(target)?;
        destination.busy_timeout(Duration::from_secs(5))?;
        {
            let backup = Backup::new(source, &mut destination)?;
            let deadline = Instant::now() + Duration::from_secs(60);
            loop {
                if matches!(backup.step(256)?, StepResult::Done) {
                    break;
                }
                ensure!(
                    Instant::now() < deadline,
                    "snapshot timed out; retry during a quiet period"
                );
                std::thread::sleep(Duration::from_millis(10));
            }
        }
        destination.pragma_update(None, "journal_mode", "DELETE")?;
        validate(&destination)?;
        drop(destination);
        file.sync_all()?;
        Ok(())
    })();
    drop(file);
    if result.is_err() {
        let _ = fs::remove_file(target);
    }
    result
}

fn backup(database: &Path, bundle: &Path) -> Result<()> {
    let source = open_readonly(database).context("cannot open source database")?;
    let builder = fs::DirBuilder::new();
    #[cfg(unix)]
    let builder = {
        use std::os::unix::fs::DirBuilderExt;
        let mut builder = builder;
        builder.mode(0o700);
        builder
    };
    builder
        .create(bundle)
        .context("backup directory must not already exist")?;
    let snapshot_path = bundle.join("database.sqlite");
    snapshot(&source, &snapshot_path)?;
    let manifest = Manifest {
        format: 1,
        product: "legura".into(),
        version: env!("CARGO_PKG_VERSION").into(),
        database_sha256: digest(&snapshot_path)?,
    };
    let mut file = private_file(&bundle.join("manifest.json"))?;
    serde_json::to_writer_pretty(&mut file, &manifest)?;
    file.sync_all()?;
    verify(bundle)?;
    Ok(())
}

fn verify(bundle: &Path) -> Result<()> {
    ensure!(
        fs::symlink_metadata(bundle)?.file_type().is_dir(),
        "backup must be a real directory"
    );
    let manifest_path = bundle.join("manifest.json");
    regular_file(&manifest_path)?;
    ensure!(
        fs::metadata(&manifest_path)?.len() <= 4096,
        "manifest too large"
    );
    let manifest: Manifest = serde_json::from_reader(fs::File::open(manifest_path)?)?;
    ensure!(
        manifest.format == 1
            && manifest.product == "legura"
            && manifest.version == env!("CARGO_PKG_VERSION"),
        "unsupported backup format or product version"
    );
    let database = bundle.join("database.sqlite");
    ensure!(
        manifest.database_sha256 == digest(&database)?,
        "backup checksum mismatch"
    );
    // A bundle must be immutable and contain a standalone snapshot, not WAL state.
    for suffix in ["-wal", "-shm", "-journal"] {
        ensure!(
            fs::symlink_metadata(bundle.join(format!("database.sqlite{suffix}"))).is_err(),
            "unexpected SQLite sidecar in backup"
        );
    }
    validate(&open_readonly(&database)?)
}

fn restore(bundle: &Path, target: &Path) -> Result<()> {
    verify(bundle)?;
    // create_new inside snapshot refuses existing paths, including symlinks.
    // Keep the trusted bundle immutable during verification and restore.
    snapshot(&open_readonly(&bundle.join("database.sqlite"))?, target)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);

    struct Fixture(std::path::PathBuf);
    impl Fixture {
        fn new() -> Self {
            let path = env::temp_dir().join(format!(
                "legura-backup-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).unwrap();
            let connection = Connection::open(path.join("source.db")).unwrap();
            connection.execute_batch("PRAGMA journal_mode=WAL; CREATE TABLE _axonyx_migrations(version TEXT PRIMARY KEY, checksum TEXT);").unwrap();
            for (version, up, down) in MIGRATIONS {
                connection.execute_batch(up).unwrap();
                connection
                    .execute(
                        "INSERT INTO _axonyx_migrations VALUES (?1, ?2)",
                        [version.to_string(), migration_hash(up, down)],
                    )
                    .unwrap();
            }
            connection.execute_batch("INSERT INTO legura_users VALUES ('owner','owner@example.test','admin'); INSERT INTO legura_credentials VALUES ('owner','owner@example.test','fixture-hash'); INSERT INTO legura_installation VALUES (1,'Fixture','owner'); INSERT INTO legura_posts VALUES ('post','Published','story','Before deletion','published'); INSERT INTO legura_posts VALUES ('draft','Draft','draft','Private','draft');").unwrap();
            Self(path)
        }
        fn path(&self, name: &str) -> std::path::PathBuf {
            self.0.join(name)
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn initialization_uses_embedded_migrations_and_is_empty() {
        let fixture = Fixture::new();
        let path = fixture.path("fresh.db");
        initialize(&path).unwrap();
        let connection = open_readonly(&path).unwrap();
        validate(&connection).unwrap();
        for table in [
            "legura_users",
            "legura_credentials",
            "legura_installation",
            "legura_posts",
        ] {
            assert_eq!(
                connection
                    .query_row(&format!("SELECT count(*) FROM {table}"), [], |row| row
                        .get::<_, i64>(0))
                    .unwrap(),
                0
            );
        }
        let history: (String, String) = connection
            .query_row(
                "SELECT name, applied_at FROM _axonyx_migrations ORDER BY version LIMIT 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(history.0, "identity");
        assert!(!history.1.is_empty());
    }

    #[test]
    fn initialization_never_replaces_existing_files_or_sidecars() {
        let fixture = Fixture::new();
        let before = digest(&fixture.path("source.db")).unwrap();
        assert!(initialize(&fixture.path("source.db")).is_err());
        assert_eq!(digest(&fixture.path("source.db")).unwrap(), before);
        fs::write(fixture.path("empty.db"), b"").unwrap();
        assert!(initialize(&fixture.path("empty.db")).is_err());
        for suffix in ["-wal", "-shm", "-journal"] {
            let sidecar = fixture.path(&format!("fresh.db{suffix}"));
            fs::write(&sidecar, b"preserve").unwrap();
            assert!(initialize(&fixture.path("fresh.db")).is_err());
            assert!(!fixture.path("fresh.db").exists());
            assert_eq!(fs::read(&sidecar).unwrap(), b"preserve");
            fs::remove_file(sidecar).unwrap();
        }
    }

    #[test]
    fn initialization_failure_rolls_back_and_removes_only_its_new_file() {
        let fixture = Fixture::new();
        let mut migrations = embedded_migrations();
        migrations[1].up_sql = "INVALID SQL".into();
        assert!(initialize_with_migrations(&fixture.path("failed.db"), &migrations).is_err());
        assert!(!fixture.path("failed.db").exists());
        validate(&open_readonly(&fixture.path("source.db")).unwrap()).unwrap();
        assert!(initialize(&fixture.path("absent-parent/fresh.db")).is_err());
        assert!(!fixture.path("absent-parent").exists());
    }

    #[test]
    fn wal_snapshot_and_restore_recover_deleted_content_and_identity() {
        let fixture = Fixture::new();
        let source = Connection::open(fixture.path("source.db")).unwrap();
        source.execute_batch("PRAGMA wal_autocheckpoint=0; UPDATE legura_posts SET body='Committed in WAL' WHERE id='post';").unwrap();
        assert!(fixture.path("source.db-wal").metadata().unwrap().len() > 0);
        backup(&fixture.path("source.db"), &fixture.path("bundle")).unwrap();
        source
            .execute("DELETE FROM legura_posts WHERE id='post'", [])
            .unwrap();
        restore(&fixture.path("bundle"), &fixture.path("recovered.db")).unwrap();
        let restored = open_readonly(&fixture.path("recovered.db")).unwrap();
        let body: String = restored
            .query_row("SELECT body FROM legura_posts WHERE id='post'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(body, "Committed in WAL");
        let credentials: String = restored
            .query_row("SELECT password_hash FROM legura_credentials", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(credentials, "fixture-hash");
        let drafts: i64 = restored
            .query_row(
                "SELECT count(*) FROM legura_posts WHERE status='draft'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(drafts, 1);
        assert_eq!(
            source
                .query_row(
                    "SELECT count(*) FROM legura_posts WHERE id='post'",
                    [],
                    |r| r.get::<_, i64>(0)
                )
                .unwrap(),
            0
        );
    }

    #[test]
    fn existing_destinations_are_never_overwritten() {
        let fixture = Fixture::new();
        backup(&fixture.path("source.db"), &fixture.path("bundle")).unwrap();
        assert!(backup(&fixture.path("source.db"), &fixture.path("bundle")).is_err());
        let before = digest(&fixture.path("source.db")).unwrap();
        assert!(restore(&fixture.path("bundle"), &fixture.path("source.db")).is_err());
        assert_eq!(digest(&fixture.path("source.db")).unwrap(), before);
    }

    #[test]
    fn corruption_and_incomplete_bundles_fail_before_creating_target() {
        let fixture = Fixture::new();
        backup(&fixture.path("source.db"), &fixture.path("bundle")).unwrap();
        fs::write(fixture.path("bundle/database.sqlite"), b"corrupted").unwrap();
        assert!(restore(&fixture.path("bundle"), &fixture.path("recovered.db")).is_err());
        assert!(!fixture.path("recovered.db").exists());
        fs::remove_file(fixture.path("bundle/manifest.json")).unwrap();
        assert!(verify(&fixture.path("bundle")).is_err());
    }

    #[test]
    fn incompatible_schema_missing_source_and_sidecars_are_rejected() {
        let fixture = Fixture::new();
        assert!(backup(&fixture.path("absent.db"), &fixture.path("absent-bundle")).is_err());
        assert!(!fixture.path("absent.db").exists());
        backup(&fixture.path("source.db"), &fixture.path("bundle")).unwrap();
        fs::write(fixture.path("bundle/database.sqlite-wal"), b"unexpected").unwrap();
        assert!(verify(&fixture.path("bundle")).is_err());
        let source = Connection::open(fixture.path("source.db")).unwrap();
        source
            .execute("UPDATE _axonyx_migrations SET checksum='wrong'", [])
            .unwrap();
        assert!(backup(&fixture.path("source.db"), &fixture.path("incompatible")).is_err());
        assert!(!fixture.path("incompatible/manifest.json").exists());
    }

    #[test]
    fn restore_refuses_orphaned_destination_wal() {
        let fixture = Fixture::new();
        backup(&fixture.path("source.db"), &fixture.path("bundle")).unwrap();
        fs::write(fixture.path("recovered.db-wal"), b"preserve me").unwrap();
        assert!(restore(&fixture.path("bundle"), &fixture.path("recovered.db")).is_err());
        assert!(!fixture.path("recovered.db").exists());
        assert_eq!(
            fs::read(fixture.path("recovered.db-wal")).unwrap(),
            b"preserve me"
        );
    }

    #[test]
    fn foreign_key_damage_and_unknown_format_are_rejected() {
        let fixture = Fixture::new();
        backup(&fixture.path("source.db"), &fixture.path("bundle")).unwrap();
        let manifest = fixture.path("bundle/manifest.json");
        let mut data: Manifest =
            serde_json::from_reader(fs::File::open(&manifest).unwrap()).unwrap();
        data.format = 2;
        fs::write(&manifest, serde_json::to_vec(&data).unwrap()).unwrap();
        assert!(restore(&fixture.path("bundle"), &fixture.path("recovered.db")).is_err());
        assert!(!fixture.path("recovered.db").exists());
        let source = Connection::open(fixture.path("source.db")).unwrap();
        source
            .execute_batch(
                "PRAGMA foreign_keys=OFF; UPDATE legura_credentials SET user_id='missing';",
            )
            .unwrap();
        assert!(backup(&fixture.path("source.db"), &fixture.path("broken")).is_err());
        assert!(!fixture.path("broken/manifest.json").exists());
    }

    #[cfg(unix)]
    #[test]
    fn permissions_and_symlink_targets() {
        use std::os::unix::fs::{symlink, PermissionsExt};
        let fixture = Fixture::new();
        backup(&fixture.path("source.db"), &fixture.path("bundle")).unwrap();
        assert_eq!(
            fs::metadata(fixture.path("bundle"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o700
        );
        assert_eq!(
            fs::metadata(fixture.path("bundle/database.sqlite"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
        symlink(fixture.path("source.db"), fixture.path("linked.db")).unwrap();
        assert!(initialize(&fixture.path("linked.db")).is_err());
        assert!(backup(&fixture.path("linked.db"), &fixture.path("linked-bundle")).is_err());
        assert!(restore(&fixture.path("bundle"), &fixture.path("linked.db")).is_err());
        initialize(&fixture.path("fresh.db")).unwrap();
        assert_eq!(
            fs::metadata(fixture.path("fresh.db"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
    }
}
