//! Register the product helpers used by compiled loaders and actions.
use axonyx_runtime::native::{AxNativeError, AxNativeRegistry};

pub fn register(registry: &mut AxNativeRegistry) -> Result<(), AxNativeError> {
    legura::media_config::register_availability(
        registry,
        &axonyx_runtime::backend::AxEnv::from_env(),
        &std::env::current_dir().map_err(|_| AxNativeError::Rejected)?,
    )?;
    legura::media_config::register_private_reads(
        registry,
        &axonyx_runtime::backend::AxEnv::from_env(),
        &std::env::current_dir().map_err(|_| AxNativeError::Rejected)?,
    )?;
    legura::pagination::register(registry)?;
    legura::media_config::register_media_deletion(
        registry,
        &axonyx_runtime::backend::AxEnv::from_env(),
        &std::env::current_dir().map_err(|_| AxNativeError::Rejected)?,
    )?;
    legura::media_config::register_published_reads(
        registry,
        &axonyx_runtime::backend::AxEnv::from_env(),
        &std::env::current_dir().map_err(|_| AxNativeError::Rejected)?,
    )?;
    legura::media_config::register_post_images(
        registry,
        &axonyx_runtime::backend::AxEnv::from_env(),
        &std::env::current_dir().map_err(|_| AxNativeError::Rejected)?,
    )?;
    legura::content_loading::register(registry)?;
    registry.register(
        "legura.content.prepare",
        |input: legura::content_preparation::ContentFormInput| {
            Ok::<_, ()>(legura::content_preparation::prepare_content_form(input))
        },
    )?;
    legura::content_rendering::register(registry)
}
