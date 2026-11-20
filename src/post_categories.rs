//! Atomic add/remove of one category assignment, preserving every other selection.
use axonyx_runtime::{
    backend::{AxDataTransport, AxDatabaseDriver, AxEnv},
    native::{AxNativeError, AxNativeRegistry},
};
use rusqlite::{Connection, OpenFlags, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use std::{path::Path, time::Duration};

// actor_id is supplied only by the guarded server query, never an action argument.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Assignment {
    actor_id: String,
    post_id: String,
    category_id: String,
    assigned: bool,
}

#[derive(Debug, Deserialize, Serialize)]
struct AssignmentResult {
    authorized: bool,
    found: bool,
    available: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReturnLocation {
    post_id: String,
    page: String,
    search: String,
}

fn return_url(input: ReturnLocation) -> String {
    let mut url = url::Url::parse("http://legura.invalid/").expect("fixed local URL");
    url.path_segments_mut()
        .expect("fixed URL supports path segments")
        .extend(["admin", "posts", &input.post_id, "categories"]);
    let page = crate::pagination::normalize_page_request(crate::pagination::PageRequestInput {
        page: input.page,
    })
    .requested_page;
    url.query_pairs_mut()
        .append_pair("page", &page.to_string())
        .append_pair("q", &crate::pagination::normalize_search(&input.search));
    format!("{}?{}", url.path(), url.query().unwrap_or_default())
}

fn apply(db: &mut Connection, input: Assignment) -> Result<AssignmentResult, AxNativeError> {
    let mut result = AssignmentResult {
        authorized: false,
        found: false,
        available: false,
    };
    let tx = db
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|_| AxNativeError::Rejected)?;
    let role: Option<String> = tx
        .query_row(
            "SELECT role FROM legura_users WHERE id=?1 AND status='active'",
            [&input.actor_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|_| AxNativeError::Rejected)?;
    result.authorized = role
        .as_deref()
        .is_some_and(crate::authorization::allows_content);
    if !result.authorized {
        return Ok(result);
    }
    result.found = tx
        .prepare("SELECT 1 FROM legura_posts WHERE id=?1 AND trashed=0")
        .and_then(|mut sql| sql.exists([&input.post_id]))
        .map_err(|_| AxNativeError::Rejected)?;
    if !result.found {
        return Ok(result);
    }
    result.available = tx
        .prepare("SELECT 1 FROM legura_categories WHERE id=?1")
        .and_then(|mut sql| sql.exists([&input.category_id]))
        .map_err(|_| AxNativeError::Rejected)?;
    if !result.available {
        return Ok(result);
    }
    if input.assigned {
        tx.execute("INSERT INTO legura_post_categories(post_id,category_id) VALUES (?1,?2) ON CONFLICT(post_id,category_id) DO NOTHING", [&input.post_id, &input.category_id]).map_err(|_| AxNativeError::Rejected)?;
    } else {
        tx.execute(
            "DELETE FROM legura_post_categories WHERE post_id=?1 AND category_id=?2",
            [&input.post_id, &input.category_id],
        )
        .map_err(|_| AxNativeError::Rejected)?;
    }
    tx.commit().map_err(|_| AxNativeError::Rejected)?;
    Ok(result)
}

pub fn register(
    registry: &mut AxNativeRegistry,
    env: &AxEnv,
    base: &Path,
) -> Result<(), AxNativeError> {
    registry.register(
        "legura.post_categories.return_url",
        |input: ReturnLocation| Ok::<_, ()>(return_url(input)),
    )?;
    let config = env.database_config().map_err(|_| AxNativeError::Rejected)?;
    if config.driver != AxDatabaseDriver::Sqlite || config.transport != AxDataTransport::Direct {
        return Err(AxNativeError::Rejected);
    }
    let url = config.url.as_deref().ok_or(AxNativeError::Rejected)?;
    let relative = url
        .strip_prefix("sqlite://")
        .or_else(|| url.strip_prefix("sqlite:"))
        .ok_or(AxNativeError::Rejected)?;
    let path = base
        .join(relative)
        .canonicalize()
        .map_err(|_| AxNativeError::Rejected)?;
    if !path.is_file() {
        return Err(AxNativeError::Rejected);
    }
    registry.register("legura.post_categories.set", move |input: Assignment| {
        let mut db = Connection::open_with_flags(&path, OpenFlags::SQLITE_OPEN_READ_WRITE)
            .map_err(|_| AxNativeError::Rejected)?;
        db.busy_timeout(Duration::from_secs(5))
            .map_err(|_| AxNativeError::Rejected)?;
        db.execute_batch("PRAGMA trusted_schema=OFF; PRAGMA foreign_keys=ON;")
            .map_err(|_| AxNativeError::Rejected)?;
        apply(&mut db, input)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn return_location_keeps_search_and_page_on_a_fixed_local_route() {
        assert_eq!(
            return_url(ReturnLocation {
                post_id: "post".into(),
                page: "2".into(),
                search: " news & draft ".into()
            }),
            "/admin/posts/post/categories?page=2&q=news+%26+draft"
        );
        let route = return_url(ReturnLocation {
            post_id: "//external.test".into(),
            page: "invalid".into(),
            search: "?next=https://external.test".into(),
        });
        assert!(route.starts_with("/admin/posts/%2F%2Fexternal.test/categories?page=1&q="));
    }

    fn fixture() -> Connection {
        let db = Connection::open_in_memory().unwrap();
        for (_, up, _) in crate::database_contract::MIGRATIONS {
            db.execute_batch(up).unwrap();
        }
        db.execute_batch("INSERT INTO legura_users(id,email,role) VALUES ('admin','admin@example.test','admin'), ('editor','editor@example.test','editor'), ('subscriber','subscriber@example.test','subscriber');
            INSERT INTO legura_posts(id,title,slug,body,status) VALUES ('one','One','one','Body','published'), ('two','Two','two','Body','draft');
            INSERT INTO legura_categories VALUES ('news','News','news'),('rust','Rust','rust');").unwrap();
        db
    }

    fn request(actor: &str, post: &str, category: &str, assigned: bool) -> Assignment {
        Assignment {
            actor_id: actor.into(),
            post_id: post.into(),
            category_id: category.into(),
            assigned,
        }
    }

    fn assignments(db: &Connection) -> Vec<(String, String)> {
        db.prepare(
            "SELECT post_id,category_id FROM legura_post_categories ORDER BY post_id,category_id",
        )
        .unwrap()
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap()
    }

    #[test]
    fn admin_and_editor_add_remove_idempotently_without_replacing_other_links() {
        let mut db = fixture();
        for actor in ["admin", "editor"] {
            for _ in 0..2 {
                assert!(
                    apply(&mut db, request(actor, "one", "news", true))
                        .unwrap()
                        .available
                );
            }
            apply(&mut db, request(actor, "one", "rust", true)).unwrap();
            apply(&mut db, request(actor, "two", "news", true)).unwrap();
            for _ in 0..2 {
                assert!(
                    apply(&mut db, request(actor, "one", "news", false))
                        .unwrap()
                        .available
                );
            }
            assert_eq!(
                assignments(&db),
                vec![("one".into(), "rust".into()), ("two".into(), "news".into())]
            );
        }
        let unchanged: (String, String) = db
            .query_row(
                "SELECT body,status FROM legura_posts WHERE id='one'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(unchanged, ("Body".into(), "published".into()));
    }

    #[test]
    fn subscriber_unknown_and_deactivated_actors_cannot_mutate() {
        let mut db = fixture();
        db.execute("UPDATE legura_users SET status='inactive',session_generation='disabled' WHERE id='editor'",[]).unwrap();
        for actor in ["subscriber", "missing", "editor"] {
            assert!(
                !apply(&mut db, request(actor, "one", "news", true))
                    .unwrap()
                    .authorized
            );
        }
        assert!(assignments(&db).is_empty());
    }

    #[test]
    fn missing_or_trashed_posts_and_deleted_categories_are_rejected() {
        let mut db = fixture();
        db.execute(
            "UPDATE legura_posts SET status='draft',trashed=1 WHERE id='two'",
            [],
        )
        .unwrap();
        for post in ["missing", "two"] {
            assert!(
                !apply(&mut db, request("editor", post, "news", true))
                    .unwrap()
                    .found
            );
        }
        assert!(
            !apply(&mut db, request("editor", "one", "missing", true))
                .unwrap()
                .available
        );
        assert!(assignments(&db).is_empty());
    }

    #[test]
    fn sql_failure_rolls_back_without_losing_previous_assignments() {
        let mut db = fixture();
        apply(&mut db, request("admin", "one", "news", true)).unwrap();
        db.execute_batch("CREATE TRIGGER fail_selection BEFORE INSERT ON legura_post_categories BEGIN SELECT RAISE(ABORT,'fixture failure'); END;").unwrap();
        assert!(apply(&mut db, request("editor", "one", "rust", true)).is_err());
        assert_eq!(assignments(&db), vec![("one".into(), "news".into())]);
    }
}
