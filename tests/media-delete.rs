use legura::media_delete::{
    delete_for_administrator, retry_cleanup_for_administrator, DeleteError, DeleteOutcome,
};
use legura::storage::{ObjectRef, ObjectRemoval, StorageError, StoreId};
use rusqlite::Connection;
use std::sync::atomic::{AtomicUsize, Ordering};

struct Removal {
    calls: AtomicUsize,
    fail: bool,
    missing: bool,
}
impl ObjectRemoval for Removal {
    fn remove(&self, _: &ObjectRef) -> Result<(), StorageError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if self.fail {
            Err(StorageError::WrongStore)
        } else if self.missing {
            Err(StorageError::NotFound)
        } else {
            Ok(())
        }
    }
}
fn fixture() -> (Connection, Removal, StoreId, String) {
    let db = Connection::open_in_memory().unwrap();
    for (_, up, _) in legura::database_contract::MIGRATIONS {
        db.execute_batch(up).unwrap();
    }
    db.execute(
        "INSERT INTO legura_users VALUES ('owner','owner@example.test','admin')",
        [],
    )
    .unwrap();
    let id = "a".repeat(64);
    db.execute(
        "INSERT INTO legura_media VALUES (?, 'local-media', 'image/png', 10, 1, 1, ?)",
        [&id, &id],
    )
    .unwrap();
    (
        db,
        Removal {
            calls: AtomicUsize::new(0),
            fail: false,
            missing: false,
        },
        StoreId::parse("local-media").unwrap(),
        id,
    )
}
fn count(db: &Connection) -> u32 {
    db.query_row("SELECT count(*) FROM legura_media", [], |r| r.get(0))
        .unwrap()
}
fn pending(db: &Connection) -> u32 {
    db.query_row("SELECT count(*) FROM legura_media_cleanup", [], |r| {
        r.get(0)
    })
    .unwrap()
}

#[test]
fn denies_missing_or_deleted_administrator_before_storage_access() {
    let (mut db, storage, store, id) = fixture();
    for subject in [None, Some(""), Some("missing"), Some("owner' OR 1=1 --")] {
        assert_eq!(
            delete_for_administrator(&mut db, &storage, &store, subject, &id),
            Err(DeleteError::Forbidden)
        );
    }
    db.execute("DELETE FROM legura_users", []).unwrap();
    assert_eq!(
        delete_for_administrator(&mut db, &storage, &store, Some("owner"), &id),
        Err(DeleteError::Forbidden)
    );
    assert_eq!(storage.calls.load(Ordering::SeqCst), 0);
    assert_eq!(count(&db), 1);
    assert_eq!(pending(&db), 0);
}

#[test]
fn rejects_paths_unknown_keys_and_other_stores() {
    let (mut db, storage, store, id) = fixture();
    for key in ["../private", "", &"b".repeat(64)] {
        assert_eq!(
            delete_for_administrator(&mut db, &storage, &store, Some("owner"), key),
            Err(DeleteError::NotFound)
        );
    }
    let other = StoreId::parse("other").unwrap();
    assert_eq!(
        delete_for_administrator(&mut db, &storage, &other, Some("owner"), &id),
        Err(DeleteError::NotFound)
    );
    assert_eq!(count(&db), 1);
    assert_eq!(storage.calls.load(Ordering::SeqCst), 0);
}

#[test]
fn draft_and_published_references_both_block_removal_until_all_detached() {
    let (mut db, storage, store, id) = fixture();
    db.execute_batch(
        "INSERT INTO legura_posts(id,title,slug,body,status) VALUES
        ('one','One','one','Body','draft'),('two','Two','two','Body','published');",
    )
    .unwrap();
    for post in ["one", "two"] {
        db.execute(
            "INSERT INTO legura_post_images VALUES (?,?,'Keep')",
            [post, &id],
        )
        .unwrap();
    }
    for post in ["two", "one"] {
        assert_eq!(
            delete_for_administrator(&mut db, &storage, &store, Some("owner"), &id),
            Err(DeleteError::InUse)
        );
        assert_eq!(count(&db), 1);
        assert_eq!(storage.calls.load(Ordering::SeqCst), 0);
        db.execute("DELETE FROM legura_post_images WHERE post_id=?", [post])
            .unwrap();
    }
    assert_eq!(
        delete_for_administrator(&mut db, &storage, &store, Some("owner"), &id),
        Ok(DeleteOutcome::Removed)
    );
    assert_eq!(count(&db), 0);
    assert_eq!(pending(&db), 0);
    assert_eq!(storage.calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        delete_for_administrator(&mut db, &storage, &store, Some("owner"), &id),
        Err(DeleteError::NotFound)
    );
    assert_eq!(storage.calls.load(Ordering::SeqCst), 1);
    assert!(db
        .execute("INSERT INTO legura_post_images VALUES ('one',?,'')", [&id])
        .is_err());
}

#[test]
fn failed_delete_or_commit_never_removes_bytes() {
    for trigger in [
        "CREATE TRIGGER failure BEFORE DELETE ON legura_media BEGIN SELECT RAISE(ABORT,'private-db-marker'); END;",
        "CREATE TABLE pending(id TEXT REFERENCES legura_media(id) DEFERRABLE INITIALLY DEFERRED);
         CREATE TRIGGER failure AFTER DELETE ON legura_media BEGIN INSERT INTO pending VALUES (OLD.id); END;",
    ] {
        let (mut db, storage, store, id) = fixture();
        db.execute_batch(trigger).unwrap();
        let error = delete_for_administrator(&mut db, &storage, &store, Some("owner"), &id).unwrap_err();
        assert_eq!(error, DeleteError::Unavailable);
        assert!(!error.to_string().contains("private-db-marker"));
        assert_eq!(count(&db), 1);
        assert_eq!(pending(&db), 0);
        assert_eq!(storage.calls.load(Ordering::SeqCst), 0);
        assert!(db.is_autocommit());
    }
}

#[test]
fn cleanup_failure_is_explicit_and_does_not_restore_read_authority() {
    let (mut db, mut storage, store, id) = fixture();
    storage.fail = true;
    let outcome = delete_for_administrator(&mut db, &storage, &store, Some("owner"), &id).unwrap();
    let DeleteOutcome::CleanupRequired(reference) = outcome else {
        panic!("cleanup must be reported");
    };
    assert_eq!(reference.key.as_str(), id);
    assert_eq!(reference.store, store);
    assert_eq!(count(&db), 0);
    assert_eq!(pending(&db), 1);
    assert!(db.is_autocommit());
}

#[test]
fn cleanup_receipt_can_be_retried_only_by_administrator_for_same_store() {
    let (mut db, mut storage, store, id) = fixture();
    storage.fail = true;
    assert!(matches!(
        delete_for_administrator(&mut db, &storage, &store, Some("owner"), &id),
        Ok(DeleteOutcome::CleanupRequired(_))
    ));
    assert_eq!(pending(&db), 1);
    storage.fail = false;
    for subject in [None, Some("missing")] {
        assert_eq!(
            retry_cleanup_for_administrator(&mut db, &storage, &store, subject, &id),
            Err(DeleteError::Forbidden)
        );
    }
    let other = StoreId::parse("other").unwrap();
    assert_eq!(
        retry_cleanup_for_administrator(&mut db, &storage, &other, Some("owner"), &id),
        Err(DeleteError::NotFound)
    );
    assert_eq!(
        retry_cleanup_for_administrator(&mut db, &storage, &store, Some("owner"), "../x"),
        Err(DeleteError::NotFound)
    );
    assert_eq!(storage.calls.load(Ordering::SeqCst), 1);
    assert!(db
        .execute(
            "INSERT INTO legura_media VALUES (?, 'local-media', 'image/png', 10, 1, 1, ?)",
            [&id, &id],
        )
        .is_err());
    assert_eq!(
        retry_cleanup_for_administrator(&mut db, &storage, &store, Some("owner"), &id),
        Ok(DeleteOutcome::Removed)
    );
    assert_eq!(pending(&db), 0);
    assert_eq!(storage.calls.load(Ordering::SeqCst), 2);
    assert_eq!(
        retry_cleanup_for_administrator(&mut db, &storage, &store, Some("owner"), &id),
        Err(DeleteError::NotFound)
    );
}

#[test]
fn cleanup_receipt_survives_database_reopen_and_missing_object_is_retryable() {
    let (db, mut storage, store, id) = fixture();
    let mut random = [0u8; 16];
    getrandom::getrandom(&mut random).unwrap();
    let name: String = random.iter().map(|byte| format!("{byte:02x}")).collect();
    let path = std::env::temp_dir().join(format!("legura-cleanup-{name}.sqlite"));
    db.backup("main", &path, None).unwrap();
    let mut connection = Connection::open(&path).unwrap();
    storage.fail = true;
    assert!(matches!(
        delete_for_administrator(&mut connection, &storage, &store, Some("owner"), &id),
        Ok(DeleteOutcome::CleanupRequired(_))
    ));
    drop(connection);
    let mut reopened = Connection::open(&path).unwrap();
    assert_eq!(pending(&reopened), 1);
    storage.fail = false;
    storage.missing = true;
    assert_eq!(
        retry_cleanup_for_administrator(&mut reopened, &storage, &store, Some("owner"), &id),
        Ok(DeleteOutcome::Removed)
    );
    assert_eq!(pending(&reopened), 0);
    drop(reopened);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn cleanup_migration_cannot_roll_back_with_pending_private_bytes() {
    let (mut db, mut storage, store, id) = fixture();
    storage.fail = true;
    assert!(matches!(
        delete_for_administrator(&mut db, &storage, &store, Some("owner"), &id),
        Ok(DeleteOutcome::CleanupRequired(_))
    ));
    let down = legura::database_contract::MIGRATIONS.last().unwrap().2;
    let rollback = db.transaction().unwrap();
    assert!(rollback.execute_batch(down).is_err());
    rollback.rollback().unwrap();
    assert_eq!(pending(&db), 1);
    storage.fail = false;
    assert_eq!(
        retry_cleanup_for_administrator(&mut db, &storage, &store, Some("owner"), &id),
        Ok(DeleteOutcome::Removed)
    );
    db.execute_batch(down).unwrap();
    assert!(db.prepare("SELECT id FROM legura_media_cleanup").is_err());
}

#[test]
fn cleanup_migration_adds_receipts_without_rewriting_existing_media() {
    let db = Connection::open_in_memory().unwrap();
    for (_, up, _) in legura::database_contract::MIGRATIONS.iter().take(5) {
        db.execute_batch(up).unwrap();
    }
    let id = "a".repeat(64);
    db.execute(
        "INSERT INTO legura_media VALUES (?, 'local-media', 'image/png', 10, 1, 1, ?)",
        [&id, &id],
    )
    .unwrap();
    db.execute_batch(legura::database_contract::MIGRATIONS.last().unwrap().1)
        .unwrap();
    assert_eq!(count(&db), 1);
    assert_eq!(pending(&db), 0);
}

#[test]
fn already_missing_object_is_successful_metadata_removal() {
    let (mut db, mut storage, store, id) = fixture();
    storage.missing = true;
    assert_eq!(
        delete_for_administrator(&mut db, &storage, &store, Some("owner"), &id),
        Ok(DeleteOutcome::Removed)
    );
    assert_eq!(count(&db), 0);
}

#[test]
fn competing_selection_writer_blocks_delete_and_its_committed_reference_is_rechecked() {
    let (db, storage, store, id) = fixture();
    let mut random = [0u8; 16];
    getrandom::getrandom(&mut random).unwrap();
    let name: String = random.iter().map(|byte| format!("{byte:02x}")).collect();
    let path = std::env::temp_dir().join(format!("legura-delete-{name}.sqlite"));
    db.backup("main", &path, None).unwrap();
    let writer = Connection::open(&path).unwrap();
    let mut deleter = Connection::open(&path).unwrap();
    deleter.busy_timeout(std::time::Duration::ZERO).unwrap();
    writer.execute_batch("PRAGMA foreign_keys=ON; BEGIN IMMEDIATE;
        INSERT INTO legura_posts(id,title,slug,body,status) VALUES ('one','One','one','Body','draft');").unwrap();
    writer
        .execute(
            "INSERT INTO legura_post_images VALUES ('one',?,'Keep')",
            [&id],
        )
        .unwrap();
    assert_eq!(
        delete_for_administrator(&mut deleter, &storage, &store, Some("owner"), &id),
        Err(DeleteError::Unavailable)
    );
    assert_eq!(storage.calls.load(Ordering::SeqCst), 0);
    writer.execute_batch("COMMIT").unwrap();
    assert_eq!(
        delete_for_administrator(&mut deleter, &storage, &store, Some("owner"), &id),
        Err(DeleteError::InUse)
    );
    assert_eq!(count(&deleter), 1);
    assert_eq!(storage.calls.load(Ordering::SeqCst), 0);
    drop(deleter);
    drop(writer);
    std::fs::remove_file(path).unwrap();
}
