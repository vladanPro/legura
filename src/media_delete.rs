//! Unreferenced media removal behind the guarded native administrator action.
use crate::storage::{ObjectKey, ObjectRef, ObjectRemoval, StorageError, StoreId};
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};

#[derive(Debug, PartialEq, Eq)]
pub enum DeleteError {
    Forbidden,
    NotFound,
    InUse,
    Unavailable,
}

impl std::fmt::Display for DeleteError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Forbidden => "Media deletion is forbidden",
            Self::NotFound => "Media was not found",
            Self::InUse => "Remove this image from its posts before deleting it",
            Self::Unavailable => "Media deletion is unavailable",
        })
    }
}
impl std::error::Error for DeleteError {}

#[derive(Debug, PartialEq, Eq)]
pub enum DeleteOutcome {
    Removed,
    /// Metadata is committed absent. Private bytes may remain; do not report full deletion.
    CleanupRequired(ObjectRef),
}

/// Caller supplies the verified session subject and startup-owned DB/store.
/// An immediate transaction serializes selection with deletion. Never remove bytes
/// until commit succeeds: a crash/cleanup failure may orphan bytes, not break a post.
/// Keys must be immutable and never reused by application writers.
pub fn delete_for_administrator(
    database: &mut Connection,
    storage: &impl ObjectRemoval,
    store: &StoreId,
    subject: Option<&str>,
    id: &str,
) -> Result<DeleteOutcome, DeleteError> {
    use DeleteError::*;
    let subject = subject.filter(|s| !s.is_empty()).ok_or(Forbidden)?;
    database
        .pragma_update(None, "foreign_keys", true)
        .map_err(|_| Unavailable)?;
    let enabled: bool = database
        .pragma_query_value(None, "foreign_keys", |r| r.get(0))
        .map_err(|_| Unavailable)?;
    if !enabled {
        return Err(Unavailable);
    }
    let transaction = database
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|_| Unavailable)?;
    let role: Option<String> = transaction
        .query_row("SELECT role FROM legura_users WHERE id=?", [subject], |r| {
            r.get(0)
        })
        .optional()
        .map_err(|_| Unavailable)?;
    if role.as_deref() != Some("admin") {
        return Err(Forbidden);
    }
    let key = ObjectKey::parse(id).map_err(|_| NotFound)?;
    let actual_store: Option<String> = transaction
        .query_row("SELECT store_id FROM legura_media WHERE id=?", [id], |r| {
            r.get(0)
        })
        .optional()
        .map_err(|_| Unavailable)?;
    if actual_store.as_deref() != Some(store.as_str()) {
        return Err(NotFound);
    }
    let in_use: bool = transaction
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM legura_post_images WHERE media_id=?)",
            [id],
            |r| r.get(0),
        )
        .map_err(|_| Unavailable)?;
    if in_use {
        return Err(InUse);
    }
    let changed = transaction
        .execute(
            "DELETE FROM legura_media WHERE id=? AND store_id=?",
            params![id, store.as_str()],
        )
        .map_err(|_| Unavailable)?;
    if changed != 1 {
        return Err(Unavailable);
    }
    transaction.commit().map_err(|_| Unavailable)?;
    let reference = ObjectRef {
        store: store.clone(),
        key,
    };
    Ok(match storage.remove(&reference) {
        Ok(()) | Err(StorageError::NotFound) => DeleteOutcome::Removed,
        Err(_) => DeleteOutcome::CleanupRequired(reference),
    })
}
