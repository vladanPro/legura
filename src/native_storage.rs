use axonyx_runtime::backend::AxEnv;
use axonyx_runtime::server::AxFileStorage;
use std::sync::Arc;

pub fn configure(
    fallback: Arc<dyn AxFileStorage>,
) -> Result<Arc<dyn AxFileStorage>, Box<dyn std::error::Error + Send + Sync>> {
    Ok(legura::media_config::configure(
        &AxEnv::from_env(),
        &std::env::current_dir()?,
        fallback,
    )?)
}
