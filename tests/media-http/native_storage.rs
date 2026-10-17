use axonyx_runtime::server::AxFileStorage;
use legura::media_library::MediaLibraryStorage;
use legura::storage::StoreId;
use std::path::PathBuf;
use std::sync::Arc;

pub fn configure(
    _fallback: Arc<dyn AxFileStorage>,
) -> Result<Arc<dyn AxFileStorage>, Box<dyn std::error::Error + Send + Sync>> {
    let root = PathBuf::from(std::env::var("AX_TEST_MEDIA_ROOT")?);
    let database = PathBuf::from(std::env::var("AX_TEST_MEDIA_DB")?);
    Ok(Arc::new(MediaLibraryStorage::open(
        &root,
        StoreId::parse("local-media")?,
        &database,
    )?))
}
