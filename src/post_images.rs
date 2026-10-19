//! Private post-image associations. No HTTP route or public image grant is created.
use crate::media_read::{read_for_administrator, MediaReadError};
use crate::storage::{ObjectStorage, StoreId};
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const MAX_ALT_CHARACTERS: usize = 300;

#[derive(Debug, PartialEq, Eq)]
pub enum PostImageError {
    Forbidden,
    PostNotFound,
    InvalidAlt,
    InvalidImage,
    Unavailable,
}

impl std::fmt::Display for PostImageError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Forbidden => "Post image access is forbidden",
            Self::PostNotFound => "Post was not found",
            Self::InvalidAlt => "Enter alternative text of at most 300 characters",
            Self::InvalidImage => "Choose an available library image",
            Self::Unavailable => "Post image is unavailable",
        })
    }
}
impl std::error::Error for PostImageError {}

pub struct ImageSelection<'a> {
    pub media_id: &'a str,
    pub alt: &'a str,
}

/// Internal native-call input. The .ax action supplies administrator_id from its
/// guarded loader, never from an action parameter or a client-provided subject.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SetImageInput {
    pub administrator_id: String,
    pub post_id: String,
    pub media_id: String,
    pub alt: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RemoveImageInput {
    pub administrator_id: String,
    pub post_id: String,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ImageMutationResult {
    authorized: bool,
    found: bool,
    saved: bool,
    errors: BTreeMap<String, String>,
}

pub(crate) fn mutation_result(
    result: Result<(), PostImageError>,
) -> Result<ImageMutationResult, axonyx_runtime::native::AxNativeError> {
    let mut output = ImageMutationResult {
        authorized: true,
        found: true,
        saved: false,
        errors: BTreeMap::new(),
    };
    match result {
        Ok(()) => output.saved = true,
        Err(PostImageError::Forbidden) => output.authorized = false,
        Err(PostImageError::PostNotFound) => output.found = false,
        Err(error @ PostImageError::InvalidAlt) => {
            output.errors.insert("alt".into(), error.to_string());
        }
        Err(error @ PostImageError::InvalidImage) => {
            output.errors.insert("media_id".into(), error.to_string());
        }
        Err(PostImageError::Unavailable) => {
            return Err(axonyx_runtime::native::AxNativeError::Rejected);
        }
    }
    Ok(output)
}

/// `subject` must come from the runtime's verified session, never request input.
/// Caller owns the startup-validated DB/store. Future HTTP callers must also
/// enforce CSRF. An immediate transaction keeps current authority and selection
/// checks in the same write snapshot; missing/corrupt bytes never become selected.
pub fn set_for_administrator(
    database: &mut Connection,
    storage: &impl ObjectStorage,
    store: &StoreId,
    subject: Option<&str>,
    post_id: &str,
    selection: Option<ImageSelection<'_>>,
) -> Result<(), PostImageError> {
    use PostImageError::{Forbidden, InvalidAlt, InvalidImage, PostNotFound, Unavailable};
    let subject = subject.filter(|value| !value.is_empty()).ok_or(Forbidden)?;
    database
        .pragma_update(None, "foreign_keys", true)
        .map_err(|_| Unavailable)?;
    let enabled: bool = database
        .pragma_query_value(None, "foreign_keys", |row| row.get(0))
        .map_err(|_| Unavailable)?;
    if !enabled {
        return Err(Unavailable);
    }
    let transaction = database
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|_| Unavailable)?;
    let role = transaction
        .query_row(
            "SELECT role FROM legura_users WHERE id = ?",
            [subject],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|_| Unavailable)?;
    if role.as_deref() != Some("admin") {
        return Err(Forbidden);
    }
    let exists = transaction
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM legura_posts WHERE id = ?)",
            [post_id],
            |row| row.get::<_, bool>(0),
        )
        .map_err(|_| Unavailable)?;
    if !exists {
        return Err(PostNotFound);
    }
    match selection {
        Some(image) => {
            if image.alt.chars().count() > MAX_ALT_CHARACTERS || image.alt.contains('\0') {
                return Err(InvalidAlt);
            }
            read_for_administrator(&transaction, storage, store, Some(subject), image.media_id)
                .map_err(|error| match error {
                    MediaReadError::Forbidden => Forbidden,
                    MediaReadError::NotFound => InvalidImage,
                    MediaReadError::Unavailable => Unavailable,
                })?;
            transaction
                .execute(
                    "INSERT INTO legura_post_images(post_id, media_id, alt) VALUES (?, ?, ?) \
                 ON CONFLICT(post_id) DO UPDATE SET media_id=excluded.media_id, alt=excluded.alt",
                    params![post_id, image.media_id, image.alt],
                )
                .map_err(|_| Unavailable)?;
        }
        None => {
            transaction
                .execute(
                    "DELETE FROM legura_post_images WHERE post_id = ?",
                    [post_id],
                )
                .map_err(|_| Unavailable)?;
        }
    }
    transaction.commit().map_err(|_| Unavailable)
}
