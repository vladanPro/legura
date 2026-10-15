//! Register the product helpers used by compiled loaders and actions.
use axonyx_runtime::native::{AxNativeError, AxNativeRegistry};

pub fn register(registry: &mut AxNativeRegistry) -> Result<(), AxNativeError> {
    legura::pagination::register(registry)?;
    legura::content_loading::register(registry)?;
    registry.register(
        "legura.content.prepare",
        |input: legura::content_preparation::ContentFormInput| {
            Ok::<_, ()>(legura::content_preparation::prepare_content_form(input))
        },
    )?;
    legura::content_rendering::register(registry)
}
