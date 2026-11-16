use legura::database_contract::MIGRATIONS;
use rusqlite::Connection;

#[test]
fn role_migration_preserves_owner_credentials_and_post_attribution() {
    let (prior, migration) = MIGRATIONS.split_at(MIGRATIONS.len() - 1);
    let (version, up, down) = migration[0];
    assert_eq!(version, "20261115000000_018");
    for foreign_keys in [false, true] {
        let mut db = Connection::open_in_memory().unwrap();
        db.pragma_update(None, "foreign_keys", foreign_keys)
            .unwrap();
        for (_, sql, _) in prior {
            db.execute_batch(sql).unwrap();
        }
        db.execute_batch(
            "INSERT INTO legura_users VALUES ('owner','owner@example.test','admin');
             INSERT INTO legura_credentials VALUES ('owner','owner@example.test','fixture');
             INSERT INTO legura_installation VALUES (1,'Test site','owner');
             INSERT INTO legura_posts (id,title,slug,body,created_by) VALUES ('post','Title','title','Body','owner');",
        )
        .unwrap();

        let tx = db.transaction().unwrap();
        tx.execute_batch(up).unwrap();
        tx.commit().unwrap();
        assert!(!db
            .prepare("PRAGMA foreign_key_check")
            .unwrap()
            .exists([])
            .unwrap());
        let preserved: (String, String, String) = db
            .query_row(
                "SELECT i.admin_id, c.password_hash, p.created_by FROM legura_installation i
                 JOIN legura_credentials c ON c.user_id = i.admin_id
                 JOIN legura_posts p ON p.created_by = i.admin_id",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(
            preserved,
            ("owner".into(), "fixture".into(), "owner".into())
        );
        assert!(db
            .execute("UPDATE legura_users SET role='editor' WHERE id='owner'", [])
            .is_err());
        assert!(db
            .execute("DELETE FROM legura_users WHERE id='owner'", [])
            .is_err());
        assert!(db
            .execute(
                "INSERT INTO legura_users VALUES ('bad','bad@example.test','unknown')",
                []
            )
            .is_err());
        db.execute(
            "INSERT INTO legura_users VALUES ('reader','reader@example.test','subscriber')",
            [],
        )
        .unwrap();
        let tx = db.transaction().unwrap();
        assert!(
            tx.execute_batch(down).is_err(),
            "downgrade must preserve subscriber"
        );
        tx.rollback().unwrap();
        db.execute("DELETE FROM legura_users WHERE id='reader'", [])
            .unwrap();
        let tx = db.transaction().unwrap();
        tx.execute_batch(down).unwrap();
        tx.commit().unwrap();
        assert!(!db
            .prepare("PRAGMA foreign_key_check")
            .unwrap()
            .exists([])
            .unwrap());
    }
}
