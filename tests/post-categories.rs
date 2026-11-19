use legura::database_contract::MIGRATIONS;
use rusqlite::{params, Connection};

fn fixture(foreign_keys: bool) -> Connection {
    let db = Connection::open_in_memory().unwrap();
    db.pragma_update(None, "foreign_keys", foreign_keys)
        .unwrap();
    for (_, up, _) in MIGRATIONS {
        db.execute_batch(up).unwrap();
    }
    db.execute_batch(
        "INSERT INTO legura_posts (id,title,slug,body,status) VALUES
           ('one','First','first','Body','published'),
           ('two','Second','second','Body','draft');
         INSERT INTO legura_categories VALUES
           ('news','News','news'), ('rust','Rust','rust');",
    )
    .unwrap();
    db
}

fn count(db: &Connection, table: &str) -> i64 {
    assert!(matches!(
        table,
        "legura_posts" | "legura_categories" | "legura_post_categories"
    ));
    db.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
        row.get(0)
    })
    .unwrap()
}

fn attach(db: &Connection, post: &str, category: &str) -> rusqlite::Result<usize> {
    db.execute(
        "INSERT INTO legura_post_categories VALUES (?1,?2)",
        [post, category],
    )
}

#[test]
fn multiple_categories_and_shared_categories_are_supported() {
    for enabled in [false, true] {
        let db = fixture(enabled);
        attach(&db, "one", "news").unwrap();
        attach(&db, "one", "rust").unwrap();
        attach(&db, "two", "news").unwrap();
        assert_eq!(count(&db, "legura_post_categories"), 3);
        assert!(attach(&db, "one", "news").is_err());
        assert!(db
            .prepare("PRAGMA foreign_key_check")
            .unwrap()
            .query([])
            .unwrap()
            .next()
            .unwrap()
            .is_none());
    }
}

#[test]
fn deleting_category_removes_only_its_assignments() {
    for enabled in [false, true] {
        let db = fixture(enabled);
        attach(&db, "one", "news").unwrap();
        attach(&db, "two", "news").unwrap();
        attach(&db, "one", "rust").unwrap();
        db.execute("DELETE FROM legura_categories WHERE id='news'", [])
            .unwrap();
        assert_eq!(count(&db, "legura_posts"), 2);
        assert_eq!(count(&db, "legura_categories"), 1);
        assert_eq!(count(&db, "legura_post_categories"), 1);
        let post: (String, String, String) = db
            .query_row(
                "SELECT title,body,status FROM legura_posts WHERE id='one'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(post, ("First".into(), "Body".into(), "published".into()));
    }
}

#[test]
fn deleting_post_keeps_categories_and_other_posts_assignments() {
    for enabled in [false, true] {
        let db = fixture(enabled);
        attach(&db, "one", "news").unwrap();
        attach(&db, "two", "news").unwrap();
        db.execute("DELETE FROM legura_posts WHERE id='one'", [])
            .unwrap();
        assert_eq!(count(&db, "legura_categories"), 2);
        assert_eq!(count(&db, "legura_posts"), 1);
        assert_eq!(count(&db, "legura_post_categories"), 1);
    }
}

#[test]
fn unknown_references_and_parent_identity_changes_are_rejected() {
    for enabled in [false, true] {
        let db = fixture(enabled);
        assert!(attach(&db, "missing", "news").is_err());
        assert!(attach(&db, "one", "missing").is_err());
        attach(&db, "one", "news").unwrap();
        for sql in [
            "UPDATE legura_post_categories SET post_id='missing'",
            "UPDATE legura_post_categories SET category_id='missing'",
            "UPDATE legura_posts SET id='renamed' WHERE id='one'",
            "UPDATE legura_categories SET id='renamed' WHERE id='news'",
        ] {
            assert!(db.execute(sql, []).is_err(), "accepted {sql}");
        }
        assert_eq!(count(&db, "legura_post_categories"), 1);
    }
}

#[test]
fn trash_preserves_assignments_for_restore_without_publishing() {
    let db = fixture(true);
    attach(&db, "one", "news").unwrap();
    db.execute(
        "UPDATE legura_posts SET status='draft',trashed=1 WHERE id='one'",
        [],
    )
    .unwrap();
    assert_eq!(count(&db, "legura_post_categories"), 1);
    db.execute("UPDATE legura_posts SET trashed=0 WHERE id='one'", [])
        .unwrap();
    let status: String = db
        .query_row(
            "SELECT status FROM legura_posts WHERE id='one'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(status, "draft");
}

#[test]
fn names_and_slugs_have_bounded_nonempty_contracts() {
    let db = fixture(true);
    for name in [
        "".to_owned(),
        " ".into(),
        " News".into(),
        "News ".into(),
        "a\0b".into(),
        "a\nb".into(),
        "a\rb".into(),
        "x".repeat(81),
    ] {
        assert!(db
            .execute(
                "INSERT INTO legura_categories VALUES ('bad',?1,'valid')",
                [name]
            )
            .is_err());
    }
    for slug in [
        "".to_owned(),
        "News".into(),
        "two words".into(),
        "../news".into(),
        "a/b".into(),
        "-news".into(),
        "news-".into(),
        "a\0b".into(),
        "x".repeat(121),
    ] {
        assert!(
            db.execute(
                "INSERT INTO legura_categories VALUES ('bad','Valid',?1)",
                [&slug]
            )
            .is_err(),
            "accepted invalid slug {slug:?}"
        );
    }
    assert!(db
        .execute(
            "INSERT INTO legura_categories VALUES ('duplicate','Other','news')",
            []
        )
        .is_err());
    db.execute(
        "INSERT INTO legura_categories VALUES ('valid',?1,?2)",
        params!["x".repeat(80), "x".repeat(120)],
    )
    .unwrap();
    db.execute(
        "INSERT INTO legura_categories VALUES ('unicode',?1,'kultura-2')",
        ["Kultura i umetnost \u{17e}"],
    )
    .unwrap();
}

#[test]
fn migration_preserves_legacy_posts_and_empty_rollback_is_lossless() {
    let mut db = Connection::open_in_memory().unwrap();
    let index = MIGRATIONS
        .iter()
        .position(|(version, _, _)| *version == "20261119000000_020")
        .unwrap();
    for (_, up, _) in &MIGRATIONS[..index] {
        db.execute_batch(up).unwrap();
    }
    db.execute("INSERT INTO legura_posts(id,title,slug,body) VALUES ('legacy','Legacy','legacy','Original body')", []).unwrap();
    let (_, up, down) = MIGRATIONS[index];
    db.execute_batch(up).unwrap();
    assert_eq!(count(&db, "legura_categories"), 0);
    assert_eq!(count(&db, "legura_post_categories"), 0);
    let tx = db.transaction().unwrap();
    tx.execute_batch(down).unwrap();
    tx.commit().unwrap();
    assert_eq!(count(&db, "legura_posts"), 1);
    let body: String = db
        .query_row(
            "SELECT body FROM legura_posts WHERE id='legacy'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(body, "Original body");
    db.execute_batch(up).unwrap();
}

#[test]
fn rollback_refuses_editorial_data_and_preserves_it_transactionally() {
    for attached in [false, true] {
        let mut db = fixture(true);
        if attached {
            attach(&db, "one", "news").unwrap();
        }
        let (_, _, down) = MIGRATIONS
            .iter()
            .find(|(version, _, _)| *version == "20261119000000_020")
            .unwrap();
        let tx = db.transaction().unwrap();
        assert!(tx.execute_batch(down).is_err());
        tx.rollback().unwrap();
        assert_eq!(count(&db, "legura_categories"), 2);
        assert_eq!(count(&db, "legura_post_categories"), i64::from(attached));
        attach(&db, "two", "rust").unwrap();
    }
}
