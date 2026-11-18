//! Atomic administrator password updates behind the guarded Profile action.
use axonyx_runtime::backend::{AxDataTransport, AxDatabaseDriver, AxEnv};
use axonyx_runtime::native::{AxNativeError, AxNativeRegistry};
use rusqlite::{params, Connection, OpenFlags, TransactionBehavior};
use serde::Deserialize;
use std::{path::Path, time::Duration};

// Identity, hash and generation are supplied by guarded server queries, never
// copied from action input. Plaintext passwords stay in the Password DSL calls.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PasswordUpdate {
    administrator_id: String,
    expected_hash: String,
    new_hash: String,
    session_generation: String,
}

fn update(connection: &mut Connection, input: &PasswordUpdate) -> Result<bool, AxNativeError> {
    if !input.new_hash.starts_with("$argon2id$v=19$") || input.new_hash.len() > 1024 {
        return Err(AxNativeError::Rejected);
    }
    let mut random = [0u8; 32];
    getrandom::getrandom(&mut random).map_err(|_| AxNativeError::Rejected)?;
    let generation: String = random.iter().map(|byte| format!("{byte:02x}")).collect();
    let tx = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|_| AxNativeError::Rejected)?;
    let changed = tx
        .execute(
            "UPDATE legura_credentials SET password_hash=?1
         WHERE user_id=?2 AND password_hash=?3 AND EXISTS (
           SELECT 1 FROM legura_users u WHERE u.id=?2 AND u.role='admin'
             AND u.status='active' AND u.session_generation=?4
             AND u.email=legura_credentials.email
         )",
            params![
                input.new_hash,
                input.administrator_id,
                input.expected_hash,
                input.session_generation
            ],
        )
        .map_err(|_| AxNativeError::Rejected)?;
    if changed != 1 {
        return Ok(false);
    }
    let changed = tx
        .execute(
            "UPDATE legura_users SET session_generation=?1 WHERE id=?2 AND session_generation=?3",
            params![generation, input.administrator_id, input.session_generation],
        )
        .map_err(|_| AxNativeError::Rejected)?;
    if changed != 1 {
        return Err(AxNativeError::Rejected);
    }
    // Migration 019 deletes all of this account's sessions on generation change.
    tx.commit().map_err(|_| AxNativeError::Rejected)?;
    Ok(true)
}

pub fn register(
    registry: &mut AxNativeRegistry,
    env: &AxEnv,
    base: &Path,
) -> Result<(), AxNativeError> {
    let config = env.database_config().map_err(|_| AxNativeError::Rejected)?;
    if config.driver != AxDatabaseDriver::Sqlite || config.transport != AxDataTransport::Direct {
        return Err(AxNativeError::Rejected);
    }
    let url = config.url.as_deref().ok_or(AxNativeError::Rejected)?;
    let path = url
        .strip_prefix("sqlite://")
        .or_else(|| url.strip_prefix("sqlite:"))
        .ok_or(AxNativeError::Rejected)?;
    let path = base
        .join(path)
        .canonicalize()
        .map_err(|_| AxNativeError::Rejected)?;
    if !path.is_file() {
        return Err(AxNativeError::Rejected);
    }
    registry.register("legura.profile.password", move |input: PasswordUpdate| {
        let mut connection = Connection::open_with_flags(&path, OpenFlags::SQLITE_OPEN_READ_WRITE)
            .map_err(|_| AxNativeError::Rejected)?;
        connection
            .busy_timeout(Duration::from_secs(5))
            .map_err(|_| AxNativeError::Rejected)?;
        connection
            .execute_batch("PRAGMA trusted_schema=OFF; PRAGMA foreign_keys=ON;")
            .map_err(|_| AxNativeError::Rejected)?;
        update(&mut connection, &input)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use axonyx_runtime::password::AxPassword;

    fn fixture() -> (Connection, PasswordUpdate) {
        let db = Connection::open_in_memory().unwrap();
        for (_, up, _) in crate::database_contract::MIGRATIONS {
            db.execute_batch(up).unwrap();
        }
        db.execute_batch(
            "INSERT INTO legura_users (id,email,role) VALUES
               ('owner','owner@example.test','admin'), ('editor','editor@example.test','editor');
             INSERT INTO legura_installation VALUES (1,'Site','owner');
             INSERT INTO ax_sessions VALUES ('old','owner','{}',1,1,10);
             INSERT INTO ax_sessions VALUES ('other','editor','{}',1,1,10);",
        )
        .unwrap();
        let expected_hash = AxPassword::hash("Original test password").unwrap();
        db.execute(
            "INSERT INTO legura_credentials VALUES ('owner','owner@example.test',?1)",
            [&expected_hash],
        )
        .unwrap();
        (
            db,
            PasswordUpdate {
                administrator_id: "owner".into(),
                expected_hash,
                new_hash: AxPassword::hash("Replacement test password").unwrap(),
                session_generation: "".into(),
            },
        )
    }

    #[test]
    fn password_hash_and_session_revocation_commit_together() {
        let (mut db, input) = fixture();
        assert!(update(&mut db, &input).unwrap());
        let hash: String = db
            .query_row(
                "SELECT password_hash FROM legura_credentials WHERE user_id='owner'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(AxPassword::verify("Replacement test password", &hash).unwrap());
        assert!(!AxPassword::verify("Original test password", &hash).unwrap());
        let remaining: String = db
            .query_row("SELECT subject FROM ax_sessions", [], |r| r.get(0))
            .unwrap();
        assert_eq!(remaining, "editor");
        assert!(
            !update(&mut db, &input).unwrap(),
            "stale credentials cannot overwrite a newer password"
        );
        assert!(db
            .execute(
                "INSERT INTO ax_sessions VALUES ('late','owner','{}',1,1,10)",
                []
            )
            .is_err());
    }

    #[test]
    fn failed_revocation_rolls_back_the_password_write() {
        let (mut db, input) = fixture();
        db.execute_batch(
            "CREATE TRIGGER fail_revocation BEFORE DELETE ON ax_sessions
            BEGIN SELECT RAISE(ABORT, 'fixture'); END;",
        )
        .unwrap();
        assert!(update(&mut db, &input).is_err());
        let hash: String = db
            .query_row(
                "SELECT password_hash FROM legura_credentials WHERE user_id='owner'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(hash, input.expected_hash);
        assert!(db
            .prepare("SELECT 1 FROM ax_sessions WHERE id='old'")
            .unwrap()
            .exists([])
            .unwrap());
    }

    #[test]
    fn authority_and_generation_are_rechecked_inside_the_transaction() {
        let (mut db, mut input) = fixture();
        input.session_generation = "stale".into();
        assert!(!update(&mut db, &input).unwrap());
        input.session_generation.clear();
        input.administrator_id = "editor".into();
        db.execute(
            "INSERT INTO legura_credentials VALUES ('editor','editor@example.test',?1)",
            [&input.expected_hash],
        )
        .unwrap();
        assert!(!update(&mut db, &input).unwrap());
        input.administrator_id = "missing".into();
        assert!(!update(&mut db, &input).unwrap());
        input.new_hash = "plaintext".into();
        assert!(update(&mut db, &input).is_err());
    }
}
