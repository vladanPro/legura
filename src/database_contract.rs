//! One migration/compatibility contract for maintenance and recovered data.
use anyhow::{ensure, Result};
use axonyx_runtime::backend::AxMigration;
use rusqlite::Connection;
use sha2::{Digest, Sha256};

pub const MIGRATIONS: &[(&str, &str, &str)] = &[
    (
        "20261008000000_001",
        include_str!("../db/migrations/20261008000000_001_identity/up.sql"),
        include_str!("../db/migrations/20261008000000_001_identity/down.sql"),
    ),
    (
        "20261009000000_002",
        include_str!("../db/migrations/20261009000000_002_posts/up.sql"),
        include_str!("../db/migrations/20261009000000_002_posts/down.sql"),
    ),
    (
        "20261012000000_003",
        include_str!("../db/migrations/20261012000000_003_richtext/up.sql"),
        include_str!("../db/migrations/20261012000000_003_richtext/down.sql"),
    ),
    (
        "20261018000000_004",
        include_str!("../db/migrations/20261018000000_004_media/up.sql"),
        include_str!("../db/migrations/20261018000000_004_media/down.sql"),
    ),
    (
        "20261019000000_005",
        include_str!("../db/migrations/20261019000000_005_post_images/up.sql"),
        include_str!("../db/migrations/20261019000000_005_post_images/down.sql"),
    ),
    (
        "20261023000000_006",
        include_str!("../db/migrations/20261023000000_006_media_cleanup/up.sql"),
        include_str!("../db/migrations/20261023000000_006_media_cleanup/down.sql"),
    ),
];

pub fn migration_hash(up: &str, down: &str) -> String {
    let mut hash = Sha256::new();
    hash.update(b"axonyx-migration-v1\0");
    hash.update(up.replace("\r\n", "\n"));
    hash.update(b"\0");
    hash.update(down.replace("\r\n", "\n"));
    format!("{:x}", hash.finalize())
}

pub fn embedded_migrations() -> Vec<AxMigration> {
    MIGRATIONS
        .iter()
        .map(|(version, up, down)| AxMigration {
            version: (*version).into(),
            name: match *version {
                "20261008000000_001" => "identity",
                "20261009000000_002" => "posts",
                "20261012000000_003" => "richtext",
                "20261018000000_004" => "media",
                "20261019000000_005" => "post_images",
                "20261023000000_006" => "media_cleanup",
                _ => unreachable!("embedded migration name is missing"),
            }
            .into(),
            checksum: migration_hash(up, down),
            up_sql: (*up).into(),
            down_sql: (*down).into(),
        })
        .collect()
}

pub fn validate_structure(connection: &Connection) -> Result<usize> {
    let integrity: String = connection.query_row("PRAGMA integrity_check", [], |row| row.get(0))?;
    ensure!(integrity == "ok", "SQLite integrity check failed");
    ensure!(
        !connection.prepare("PRAGMA foreign_key_check")?.exists([])?,
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
    ensure!(
        (2..=MIGRATIONS.len()).contains(&count),
        "unsupported migration history"
    );
    for (version, up, down) in MIGRATIONS.iter().take(count) {
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
    Ok(count)
}

pub fn validate(connection: &Connection) -> Result<()> {
    if validate_structure(connection)? >= 3 {
        ensure!(
            crate::content_audit::audit(connection)?.invalid == 0,
            "Stored post content is invalid; backup verification refused"
        );
    }
    Ok(())
}

type SchemaRow = (String, String, String, Option<String>);
fn schema(connection: &Connection) -> rusqlite::Result<Vec<SchemaRow>> {
    let mut statement = connection.prepare(
        "SELECT type, name, tbl_name, sql FROM sqlite_schema
         WHERE name NOT GLOB 'sqlite_*' AND name <> '_axonyx_migrations'
           AND NOT (type = 'table' AND name = 'ax_sessions')
           AND NOT (type = 'index' AND name = 'ax_sessions_expires_at_idx' AND tbl_name = 'ax_sessions')
         ORDER BY type, name",
    )?;
    let rows = statement.query_map([], |row| {
        Ok((
            row.get(0)?,
            row.get(1)?,
            row.get(2)?,
            row.get::<_, Option<String>>(3)?
                .map(|sql| sql.replace("\r\n", "\n")),
        ))
    })?;
    rows.collect()
}

// Runtime 0.6.9 creates these two objects at first session-store startup, not
// through product migrations. Recognize only that bounded runtime contract.
fn validate_runtime_sessions(connection: &Connection) -> Result<()> {
    let objects: usize = connection.query_row(
        "SELECT COUNT(*) FROM sqlite_schema WHERE name IN ('ax_sessions', 'ax_sessions_expires_at_idx')",
        [], |row| row.get(0),
    )?;
    if objects == 0 {
        return Ok(());
    }
    ensure!(
        objects == 2,
        "Recovered runtime session schema is incomplete"
    );
    let kind: String = connection.query_row(
        "SELECT type FROM sqlite_schema WHERE name='ax_sessions'",
        [],
        |row| row.get(0),
    )?;
    ensure!(
        kind == "table",
        "Recovered runtime sessions must be a table"
    );
    let mut statement = connection.prepare("PRAGMA table_info(ax_sessions)")?;
    let rows = statement.query_map([], |row| {
        Ok((
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, bool>(3)?,
            row.get::<_, Option<String>>(4)?,
            row.get::<_, u32>(5)?,
        ))
    })?;
    let columns = rows.collect::<rusqlite::Result<Vec<_>>>()?;
    let expected = [
        ("id", "TEXT", 1),
        ("subject", "TEXT", 0),
        ("data_json", "TEXT", 0),
        ("created_at_unix", "INTEGER", 0),
        ("last_seen_at_unix", "INTEGER", 0),
        ("expires_at_unix", "INTEGER", 0),
    ]
    .map(|(name, kind, pk)| (name.to_owned(), kind.to_owned(), true, None, pk));
    ensure!(
        columns == expected,
        "Recovered runtime session columns are incompatible"
    );
    let mut statement = connection.prepare("PRAGMA index_info(ax_sessions_expires_at_idx)")?;
    let rows = statement.query_map([], |row| row.get::<_, String>(2))?;
    ensure!(
        rows.collect::<rusqlite::Result<Vec<_>>>()? == ["expires_at_unix"],
        "Recovered runtime session expiry index is incompatible"
    );
    let index: (String, String) = connection.query_row(
        "SELECT type, tbl_name FROM sqlite_schema WHERE name='ax_sessions_expires_at_idx'",
        [],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    ensure!(
        index == ("index".into(), "ax_sessions".into()),
        "Recovered runtime expiry index belongs to another table"
    );
    let mut statement = connection.prepare("PRAGMA index_list(ax_sessions)")?;
    let rows = statement.query_map([], |row| {
        Ok((
            row.get::<_, String>(1)?,
            row.get::<_, bool>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, bool>(4)?,
        ))
    })?;
    let mut indexes = rows.collect::<rusqlite::Result<Vec<_>>>()?;
    indexes.sort();
    ensure!(
        indexes
            == vec![
                (
                    "ax_sessions_expires_at_idx".into(),
                    false,
                    "c".into(),
                    false
                ),
                (
                    "sqlite_autoindex_ax_sessions_1".into(),
                    true,
                    "pk".into(),
                    false
                ),
            ],
        "Recovered runtime session indexes are incompatible"
    );
    Ok(())
}

/// Strict source-pilot recovery gate. Only the exact current product schema is
/// accepted; alternate DDL, plugin tables and historical versions need a later
/// explicit compatibility policy. No user database migrations are executed.
pub fn validate_current(connection: &Connection) -> Result<()> {
    let ledger_type: String = connection.query_row(
        "SELECT type FROM sqlite_schema WHERE name = '_axonyx_migrations'",
        [],
        |row| row.get(0),
    )?;
    ensure!(
        ledger_type == "table",
        "Recovered migration history must be a table"
    );
    ensure!(validate_structure(connection)? == MIGRATIONS.len(),
        "Recovered database requires the current product migrations; no automatic upgrade was performed");
    let expected = Connection::open_in_memory()?;
    for (_, up, _) in MIGRATIONS {
        expected.execute_batch(up)?;
    }
    ensure!(
        schema(connection)? == schema(&expected)?,
        "Recovered product schema differs from this maintenance build"
    );
    validate_runtime_sessions(connection)?;
    for migration in embedded_migrations() {
        let name: String = connection.query_row(
            "SELECT name FROM _axonyx_migrations WHERE version = ?",
            [&migration.version],
            |row| row.get(0),
        )?;
        ensure!(
            name == migration.name,
            "Recovered migration name differs from this maintenance build"
        );
    }
    ensure!(
        crate::content_audit::audit(connection)?.invalid == 0,
        "Recovered post content is incompatible"
    );
    Ok(())
}
