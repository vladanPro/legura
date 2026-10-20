//! Read-only public-post grants. This module does not register an HTTP route.
use crate::media_read::{read_verified_image, MediaReadError, PrivateMedia};
use crate::storage::{ObjectKey, ObjectStorage, StoreId};
use axonyx_runtime::server::AxHttpResponse;
use rusqlite::{params, Connection, OptionalExtension};

#[derive(Debug, PartialEq, Eq)]
pub enum PublishedImageError {
    NotFound,
    Unavailable,
}

impl std::fmt::Display for PublishedImageError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::NotFound => "Post image was not found",
            Self::Unavailable => "Post image is unavailable",
        })
    }
}
impl std::error::Error for PublishedImageError {}

/// Not serializable; bytes must not travel through the native JSON registry.
pub struct PublishedImage {
    image: PrivateMedia,
}

impl PublishedImage {
    pub fn into_response(self) -> AxHttpResponse {
        // Public access is not a cache grant: recheck publication on every read.
        self.image.into_response()
    }
}

/// Grant only the current selection of this exact published slug. Including the
/// expected media ID makes an old image URL fail after replacement, rather than
/// silently returning new bytes. The read snapshot spans the grant and metadata.
/// A response already admitted before unpublish cannot be recalled afterwards.
pub fn read_for_published_post(
    database: &mut Connection,
    storage: &impl ObjectStorage,
    store: &StoreId,
    slug: &str,
    media_id: &str,
) -> Result<PublishedImage, PublishedImageError> {
    use PublishedImageError::{NotFound, Unavailable};
    let key = ObjectKey::parse(media_id).map_err(|_| NotFound)?;
    let transaction = database.transaction().map_err(|_| Unavailable)?;
    let granted = transaction
        .query_row(
            "SELECT 1 FROM legura_posts p \
             JOIN legura_post_images i ON i.post_id = p.id \
             WHERE p.slug = ? AND p.status = 'published' AND i.media_id = ?",
            params![slug, key.as_str()],
            |row| row.get::<_, i32>(0),
        )
        .optional()
        .map_err(|_| Unavailable)?;
    if granted.is_none() {
        return Err(NotFound);
    }
    let image = read_verified_image(&transaction, storage, store, key.as_str()).map_err(
        |error| match error {
            MediaReadError::NotFound => NotFound,
            MediaReadError::Forbidden | MediaReadError::Unavailable => Unavailable,
        },
    )?;
    transaction.commit().map_err(|_| Unavailable)?;
    Ok(PublishedImage { image })
}
