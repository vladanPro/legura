//! Read-only public-post grants. This module does not register an HTTP route.
use crate::media_read::{read_verified_image, MediaReadError, PrivateMedia};
use crate::storage::{ObjectKey, ObjectStorage, StoreId};
use axonyx_runtime::server::AxHttpResponse;
use rusqlite::{params, Connection, OptionalExtension};

#[derive(Default, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImagePresentation {
    pub selected: bool,
    pub src: String,
    pub alt: String,
}

pub fn presentation(
    database: &mut Connection,
    storage: &impl ObjectStorage,
    store: &StoreId,
    slug: &str,
) -> ImagePresentation {
    let selection = database.query_row(
        "SELECT i.media_id, i.alt FROM legura_posts p JOIN legura_post_images i ON i.post_id=p.id WHERE p.slug=? AND p.status='published'",
        [slug], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
    ).optional();
    let Ok(Some((id, alt))) = selection else {
        return ImagePresentation::default();
    };
    let src = if read_for_published_post(database, storage, store, slug, &id).is_ok() {
        // Encode one URL path segment; the browser must not reinterpret a slug as a query or fragment.
        let encoded: String = slug
            .bytes()
            .map(|byte| {
                if byte.is_ascii_alphanumeric() || b"-._~".contains(&byte) {
                    char::from(byte).to_string()
                } else {
                    format!("%{byte:02X}")
                }
            })
            .collect();
        format!("/api/post-images/{encoded}/{id}")
    } else {
        String::new()
    };
    ImagePresentation {
        selected: true,
        src,
        alt,
    }
}

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
    pub fn into_native_response(
        self,
    ) -> Result<
        axonyx_runtime::native_response::AxNativeBinaryResponse,
        axonyx_runtime::native_response::AxNativeResponseError,
    > {
        self.image.into_native_response()
    }

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
