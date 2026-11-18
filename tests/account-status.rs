use legura::database_contract::MIGRATIONS;
use rusqlite::{Connection, Result};

const LEGACY_SESSION_TABLE: &str = "CREATE TABLE ax_sessions (
    id TEXT PRIMARY KEY NOT NULL, subject TEXT NOT NULL, data_json TEXT NOT NULL,
    created_at_unix INTEGER NOT NULL, last_seen_at_unix INTEGER NOT NULL,
    expires_at_unix INTEGER NOT NULL
);";

fn issue_session(db: &Connection, id: &str, user: &str, data: &str) -> Result<usize> {
    db.execute(
        "INSERT INTO ax_sessions VALUES (?1, ?2, ?3, 1, 1, 10)",
        [id, user, data],
    )
}

#[test]
fn status_transition_revokes_sessions_and_rejects_stale_login_writes() {
    let mut db = Connection::open_in_memory().unwrap();
    let index = MIGRATIONS
        .iter()
        .position(|(version, _, _)| *version == "20261118000000_019")
        .unwrap();
    let (prior, status) = MIGRATIONS.split_at(index);
    let (_, up, down) = status[0];
    for (_, sql, _) in prior {
        db.execute_batch(sql).unwrap();
    }
    db.execute_batch(LEGACY_SESSION_TABLE).unwrap();
    db.execute_batch(
        "INSERT INTO legura_users VALUES ('owner','owner@example.test','admin'),
                                        ('editor','editor@example.test','editor');
         INSERT INTO legura_installation (id,site_name,admin_id) VALUES (1,'Site','owner');
         INSERT INTO legura_posts (id,title,slug,body,created_by)
           VALUES ('post','Title','title','Body','editor');",
    )
    .unwrap();
    issue_session(&db, "owner-cookie", "owner", "{}").unwrap();
    issue_session(&db, "editor-cookie", "editor", "{}").unwrap();
    db.execute_batch(up).unwrap();
    assert!(db
        .execute(
            "UPDATE legura_users SET status='inactive', session_generation='new' WHERE id='owner'",
            [],
        )
        .is_err());
    assert!(db
        .execute(
            "UPDATE legura_users SET status='inactive' WHERE id='editor'",
            []
        )
        .is_err());

    db.execute(
        "UPDATE legura_users SET status='inactive', session_generation='blocked' WHERE id='editor'",
        [],
    )
    .unwrap();
    let remaining: String = db
        .query_row("SELECT group_concat(subject) FROM ax_sessions", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(remaining, "owner");
    assert!(issue_session(&db, "late", "editor", "{}").is_err());
    assert!(issue_session(
        &db,
        "blocked",
        "editor",
        r#"{"legura_generation":"blocked"}"#
    )
    .is_err());
    let tx = db.transaction().unwrap();
    assert!(tx.execute_batch(down).is_err());
    tx.rollback().unwrap();

    db.execute(
        "UPDATE legura_users SET status='active', session_generation='current' WHERE id='editor'",
        [],
    )
    .unwrap();
    assert!(issue_session(&db, "still-late", "editor", "{}").is_err());
    issue_session(&db, "fresh", "editor", r#"{"legura_generation":"current"}"#).unwrap();
    let author: String = db
        .query_row(
            "SELECT created_by FROM legura_posts WHERE id='post'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(author, "editor");
    db.execute_batch(down).unwrap();
}
