use image::ImageEncoder;
use legura::post_images::{set_for_administrator, ImageSelection, PostImageError};
use legura::published_image::{read_for_published_post, PublishedImageError};
use legura::storage::{ObjectRef, ObjectStorage, StorageError, StoreId, StoredObject};
use rusqlite::{params, Connection};
use sha2::{Digest, Sha256};
use std::sync::atomic::{AtomicUsize, Ordering};

#[test]
fn public_grant_requires_the_exact_current_published_association() {
    let (mut db, objects, store, id) = fixture();
    for post in ["one", "two"] {
        set_for_administrator(
            &mut db,
            &objects,
            &store,
            Some("owner"),
            post,
            Some(ImageSelection {
                media_id: &id,
                alt: "<literal>",
            }),
        )
        .unwrap();
    }
    let before = objects.reads.load(Ordering::SeqCst);
    for (slug, media) in [
        ("first", id.as_str()),
        ("missing", id.as_str()),
        ("second' OR 1=1 --", id.as_str()),
        ("second", "../private"),
        ("second", "b".repeat(64).as_str()),
    ] {
        assert!(matches!(
            read_for_published_post(&mut db, &objects, &store, slug, media),
            Err(PublishedImageError::NotFound)
        ));
    }
    assert_eq!(objects.reads.load(Ordering::SeqCst), before);
    let response = read_for_published_post(&mut db, &objects, &store, "second", &id)
        .unwrap()
        .into_response();
    assert_eq!(response.status, 200);
    assert_eq!(response.content_type, "image/png");
    assert_eq!(
        response.header_value("Cache-Control"),
        Some("private, no-store")
    );
    assert_eq!(
        response.header_value("X-Content-Type-Options"),
        Some("nosniff")
    );
    assert_eq!(
        response.header_value("Cross-Origin-Resource-Policy"),
        Some("same-origin")
    );
    assert_eq!(response.body.into_bytes(), objects.bytes);
}

#[test]
fn unpublish_detach_replace_slug_change_and_delete_revoke_old_public_grants() {
    let (mut db, objects, store, id) = fixture();
    let replacement = "b".repeat(64);
    set_for_administrator(
        &mut db,
        &objects,
        &store,
        Some("owner"),
        "two",
        Some(ImageSelection {
            media_id: &id,
            alt: "",
        }),
    )
    .unwrap();
    db.execute("UPDATE legura_posts SET status='draft' WHERE id='two'", [])
        .unwrap();
    assert!(matches!(
        read_for_published_post(&mut db, &objects, &store, "second", &id),
        Err(PublishedImageError::NotFound)
    ));
    db.execute(
        "UPDATE legura_posts SET status='published' WHERE id='two'",
        [],
    )
    .unwrap();
    assert!(read_for_published_post(&mut db, &objects, &store, "second", &id).is_ok());
    set_for_administrator(
        &mut db,
        &objects,
        &store,
        Some("owner"),
        "two",
        Some(ImageSelection {
            media_id: &replacement,
            alt: "",
        }),
    )
    .unwrap();
    assert!(matches!(
        read_for_published_post(&mut db, &objects, &store, "second", &id),
        Err(PublishedImageError::NotFound)
    ));
    assert!(read_for_published_post(&mut db, &objects, &store, "second", &replacement).is_ok());
    db.execute("UPDATE legura_posts SET slug='renamed' WHERE id='two'", [])
        .unwrap();
    assert!(matches!(
        read_for_published_post(&mut db, &objects, &store, "second", &replacement),
        Err(PublishedImageError::NotFound)
    ));
    assert!(read_for_published_post(&mut db, &objects, &store, "renamed", &replacement).is_ok());
    set_for_administrator(&mut db, &objects, &store, Some("owner"), "two", None).unwrap();
    assert!(matches!(
        read_for_published_post(&mut db, &objects, &store, "renamed", &replacement),
        Err(PublishedImageError::NotFound)
    ));
    set_for_administrator(
        &mut db,
        &objects,
        &store,
        Some("owner"),
        "two",
        Some(ImageSelection {
            media_id: &replacement,
            alt: "",
        }),
    )
    .unwrap();
    db.execute("DELETE FROM legura_posts WHERE id='two'", [])
        .unwrap();
    assert!(matches!(
        read_for_published_post(&mut db, &objects, &store, "renamed", &replacement),
        Err(PublishedImageError::NotFound)
    ));
    assert_eq!(
        db.query_row("SELECT count(*) FROM legura_media", [], |r| r
            .get::<_, u64>(0))
            .unwrap(),
        2
    );
}

#[test]
fn published_grant_does_not_bypass_exact_byte_and_store_validation() {
    let (mut db, mut objects, store, id) = fixture();
    set_for_administrator(
        &mut db,
        &objects,
        &store,
        Some("owner"),
        "two",
        Some(ImageSelection {
            media_id: &id,
            alt: "",
        }),
    )
    .unwrap();
    let original = objects.bytes.clone();
    objects.bytes[0] ^= 1;
    assert!(matches!(
        read_for_published_post(&mut db, &objects, &store, "second", &id),
        Err(PublishedImageError::Unavailable)
    ));
    objects.bytes.clear();
    assert!(matches!(
        read_for_published_post(&mut db, &objects, &store, "second", &id),
        Err(PublishedImageError::Unavailable)
    ));
    objects.bytes = original;
    let before = objects.reads.load(Ordering::SeqCst);
    assert!(matches!(
        read_for_published_post(
            &mut db,
            &objects,
            &StoreId::parse("other").unwrap(),
            "second",
            &id
        ),
        Err(PublishedImageError::Unavailable)
    ));
    assert_eq!(objects.reads.load(Ordering::SeqCst), before);
    db.execute("UPDATE legura_media SET width=2 WHERE id=?", [&id])
        .unwrap();
    assert!(matches!(
        read_for_published_post(&mut db, &objects, &store, "second", &id),
        Err(PublishedImageError::Unavailable)
    ));
}

struct Objects {
    bytes: Vec<u8>,
    reads: AtomicUsize,
}
impl ObjectStorage for Objects {
    fn put(&self, _: &[u8]) -> Result<StoredObject, StorageError> {
        panic!("selection must never write image bytes")
    }
    fn get(&self, _: &ObjectRef) -> Result<Vec<u8>, StorageError> {
        self.reads.fetch_add(1, Ordering::SeqCst);
        if self.bytes.is_empty() {
            Err(StorageError::NotFound)
        } else {
            Ok(self.bytes.clone())
        }
    }
}

fn fixture() -> (Connection, Objects, StoreId, String) {
    let database = Connection::open_in_memory().unwrap();
    for (_, up, _) in legura::database_contract::MIGRATIONS {
        database.execute_batch(up).unwrap();
    }
    database
        .execute_batch(
            "INSERT INTO legura_users VALUES ('owner','owner@example.test','admin');
         INSERT INTO legura_posts(id,title,slug,body,status) VALUES
         ('one','First','first','Body','draft'), ('two','Second','second','Body','published');",
        )
        .unwrap();
    let mut bytes = Vec::new();
    image::codecs::png::PngEncoder::new(&mut bytes)
        .write_image(&[1, 2, 3, 255], 1, 1, image::ExtendedColorType::Rgba8)
        .unwrap();
    for id in ["a".repeat(64), "b".repeat(64)] {
        database
            .execute(
                "INSERT INTO legura_media VALUES (?, 'local-media', 'image/png', ?, 1, 1, ?)",
                params![
                    id,
                    bytes.len() as u64,
                    format!("{:x}", Sha256::digest(&bytes))
                ],
            )
            .unwrap();
    }
    (
        database,
        Objects {
            bytes,
            reads: AtomicUsize::new(0),
        },
        StoreId::parse("local-media").unwrap(),
        "a".repeat(64),
    )
}

fn selected(database: &Connection, post: &str) -> Option<(String, String)> {
    use rusqlite::OptionalExtension;
    database
        .query_row(
            "SELECT media_id, alt FROM legura_post_images WHERE post_id=?",
            [post],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .unwrap()
}

#[test]
fn selection_replacement_detach_and_reuse_do_not_change_posts_or_objects() {
    let (mut db, objects, store, id) = fixture();
    for post in ["one", "two"] {
        set_for_administrator(
            &mut db,
            &objects,
            &store,
            Some("owner"),
            post,
            Some(ImageSelection {
                media_id: &id,
                alt: "Original",
            }),
        )
        .unwrap();
    }
    let replacement = "b".repeat(64);
    set_for_administrator(
        &mut db,
        &objects,
        &store,
        Some("owner"),
        "one",
        Some(ImageSelection {
            media_id: &replacement,
            alt: "Replacement",
        }),
    )
    .unwrap();
    assert_eq!(
        selected(&db, "one"),
        Some((replacement, "Replacement".into()))
    );
    assert_eq!(selected(&db, "two"), Some((id.clone(), "Original".into())));
    for _ in 0..2 {
        set_for_administrator(&mut db, &objects, &store, Some("owner"), "one", None).unwrap();
    }
    assert_eq!(selected(&db, "one"), None);
    assert_eq!(
        db.query_row("SELECT count(*) FROM legura_media", [], |row| row
            .get::<_, u64>(0))
            .unwrap(),
        2
    );
    assert_eq!(
        db.query_row(
            "SELECT status FROM legura_posts WHERE id='one'",
            [],
            |row| row.get::<_, String>(0)
        )
        .unwrap(),
        "draft"
    );
    assert_eq!(objects.reads.load(Ordering::SeqCst), 3);
}

#[test]
fn current_authority_is_required_for_selection_and_detach_before_object_access() {
    let (mut db, objects, store, id) = fixture();
    for subject in [None, Some(""), Some("missing"), Some("owner' OR 1=1 --")] {
        for selection in [
            None,
            Some(ImageSelection {
                media_id: &id,
                alt: "",
            }),
        ] {
            assert_eq!(
                set_for_administrator(&mut db, &objects, &store, subject, "one", selection),
                Err(PostImageError::Forbidden)
            );
        }
    }
    assert_eq!(objects.reads.load(Ordering::SeqCst), 0);
    set_for_administrator(
        &mut db,
        &objects,
        &store,
        Some("owner"),
        "one",
        Some(ImageSelection {
            media_id: &id,
            alt: "Keep",
        }),
    )
    .unwrap();
    db.execute("DELETE FROM legura_users WHERE id='owner'", [])
        .unwrap();
    assert_eq!(
        set_for_administrator(&mut db, &objects, &store, Some("owner"), "one", None),
        Err(PostImageError::Forbidden)
    );
    assert_eq!(selected(&db, "one"), Some((id, "Keep".into())));
}

#[test]
fn missing_posts_and_invalid_inputs_never_select_or_read_an_object() {
    let (mut db, objects, store, id) = fixture();
    assert_eq!(
        set_for_administrator(
            &mut db,
            &objects,
            &store,
            Some("owner"),
            "missing",
            Some(ImageSelection {
                media_id: &id,
                alt: ""
            })
        ),
        Err(PostImageError::PostNotFound)
    );
    for alt in ["x".repeat(301), "bad\0alt".into()] {
        assert_eq!(
            set_for_administrator(
                &mut db,
                &objects,
                &store,
                Some("owner"),
                "one",
                Some(ImageSelection {
                    media_id: &id,
                    alt: &alt
                })
            ),
            Err(PostImageError::InvalidAlt)
        );
    }
    for id in ["../private".into(), "c".repeat(64), "' OR 1=1 --".into()] {
        assert_eq!(
            set_for_administrator(
                &mut db,
                &objects,
                &store,
                Some("owner"),
                "one",
                Some(ImageSelection {
                    media_id: &id,
                    alt: ""
                })
            ),
            Err(PostImageError::InvalidImage)
        );
    }
    assert_eq!(selected(&db, "one"), None);
    assert_eq!(objects.reads.load(Ordering::SeqCst), 0);
}

#[test]
fn corrupt_missing_and_wrong_store_images_preserve_the_previous_selection() {
    let (mut db, mut objects, store, id) = fixture();
    set_for_administrator(
        &mut db,
        &objects,
        &store,
        Some("owner"),
        "one",
        Some(ImageSelection {
            media_id: &id,
            alt: "Keep",
        }),
    )
    .unwrap();
    let replacement = "b".repeat(64);
    let good = objects.bytes.clone();
    for bytes in [Vec::new(), vec![0, 1, 2]] {
        objects.bytes = bytes;
        assert_eq!(
            set_for_administrator(
                &mut db,
                &objects,
                &store,
                Some("owner"),
                "one",
                Some(ImageSelection {
                    media_id: &replacement,
                    alt: "Rejected"
                })
            ),
            Err(PostImageError::Unavailable)
        );
        assert_eq!(selected(&db, "one"), Some((id.clone(), "Keep".into())));
    }
    objects.bytes = good;
    db.execute(
        "UPDATE legura_media SET store_id='other-store' WHERE id=?",
        [&replacement],
    )
    .unwrap();
    assert_eq!(
        set_for_administrator(
            &mut db,
            &objects,
            &store,
            Some("owner"),
            "one",
            Some(ImageSelection {
                media_id: &replacement,
                alt: "Rejected"
            })
        ),
        Err(PostImageError::Unavailable)
    );
    assert_eq!(selected(&db, "one"), Some((id, "Keep".into())));
    objects.bytes.clear();
    let reads = objects.reads.load(Ordering::SeqCst);
    set_for_administrator(&mut db, &objects, &store, Some("owner"), "one", None).unwrap();
    assert_eq!(selected(&db, "one"), None);
    assert_eq!(objects.reads.load(Ordering::SeqCst), reads);
}

#[test]
fn alt_uses_unicode_scalar_bounds_and_parameterized_literal_storage() {
    let (mut db, objects, store, id) = fixture();
    for alt in [
        "\u{e9}".repeat(300),
        "'); DROP TABLE legura_posts; -- <script>literal</script>".into(),
        String::new(),
    ] {
        set_for_administrator(
            &mut db,
            &objects,
            &store,
            Some("owner"),
            "one",
            Some(ImageSelection {
                media_id: &id,
                alt: &alt,
            }),
        )
        .unwrap();
        assert_eq!(selected(&db, "one"), Some((id.clone(), alt)));
    }
    assert_eq!(
        db.query_row("SELECT count(*) FROM legura_posts", [], |row| row
            .get::<_, u64>(0))
            .unwrap(),
        2
    );
}

#[test]
fn failed_writes_are_atomic_and_do_not_leak_database_details() {
    let (mut db, objects, store, id) = fixture();
    set_for_administrator(
        &mut db,
        &objects,
        &store,
        Some("owner"),
        "one",
        Some(ImageSelection {
            media_id: &id,
            alt: "Keep",
        }),
    )
    .unwrap();
    db.execute_batch("CREATE TRIGGER fail_update BEFORE UPDATE ON legura_post_images BEGIN SELECT RAISE(ABORT,'private-db-marker'); END;").unwrap();
    let error = set_for_administrator(
        &mut db,
        &objects,
        &store,
        Some("owner"),
        "one",
        Some(ImageSelection {
            media_id: &id,
            alt: "Rejected",
        }),
    )
    .unwrap_err();
    assert_eq!(error, PostImageError::Unavailable);
    assert!(!error.to_string().contains("private-db-marker"));
    assert_eq!(selected(&db, "one"), Some((id, "Keep".into())));
    assert!(db.is_autocommit());
}

#[test]
fn deletion_semantics_hold_with_foreign_keys_enabled_or_disabled() {
    for foreign_keys in [true, false] {
        let (mut db, objects, store, id) = fixture();
        for post in ["one", "two"] {
            set_for_administrator(
                &mut db,
                &objects,
                &store,
                Some("owner"),
                post,
                Some(ImageSelection {
                    media_id: &id,
                    alt: "Shared image",
                }),
            )
            .unwrap();
        }
        db.pragma_update(None, "foreign_keys", foreign_keys)
            .unwrap();
        assert!(db
            .execute("DELETE FROM legura_media WHERE id=?", [&id])
            .is_err());
        db.execute("DELETE FROM legura_posts WHERE id='one'", [])
            .unwrap();
        assert_eq!(selected(&db, "one"), None);
        assert!(selected(&db, "two").is_some());
        assert!(db
            .execute("DELETE FROM legura_media WHERE id=?", [&id])
            .is_err());
        db.execute("DELETE FROM legura_posts WHERE id='two'", [])
            .unwrap();
        assert_eq!(selected(&db, "two"), None);
        assert_eq!(
            db.query_row("SELECT count(*) FROM legura_media", [], |row| row
                .get::<_, u64>(0))
                .unwrap(),
            2
        );
        assert!(!db
            .prepare("PRAGMA foreign_key_check")
            .unwrap()
            .exists([])
            .unwrap());
    }
}

#[test]
fn migration_preserves_existing_posts_and_media_and_rejects_dangling_references() {
    let (db, _, _, id) = fixture();
    let (_, up, down) = legura::database_contract::MIGRATIONS.last().unwrap();
    db.execute_batch(down).unwrap();
    db.execute_batch(up).unwrap();
    assert_eq!(
        db.query_row("SELECT count(*) FROM legura_posts", [], |row| row
            .get::<_, u64>(0))
            .unwrap(),
        2
    );
    assert_eq!(
        db.query_row("SELECT count(*) FROM legura_media", [], |row| row
            .get::<_, u64>(0))
            .unwrap(),
        2
    );
    assert_eq!(selected(&db, "one"), None);
    db.pragma_update(None, "foreign_keys", true).unwrap();
    for (post, image, alt) in [
        ("missing", id.as_str(), ""),
        ("one", "missing", ""),
        ("one", id.as_str(), "bad\0alt"),
    ] {
        assert!(db
            .execute(
                "INSERT INTO legura_post_images VALUES (?,?,?)",
                params![post, image, alt]
            )
            .is_err());
    }
    assert_eq!(selected(&db, "one"), None);
}
