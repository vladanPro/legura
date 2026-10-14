use axonyx_runtime::native::{AxNativeError, AxNativeRegistry};
use legura::content_preparation::{prepare_content_form, ContentFormInput};

pub fn register(registry: &mut AxNativeRegistry) -> Result<(), AxNativeError> {
    legura::pagination::register(registry)?;
    legura::content_loading::register(registry)?;
    legura::content_rendering::register(registry)?;
    registry.register("legura.content.prepare", |input: ContentFormInput| {
        Ok::<_, ()>(prepare_content_form(input))
    })
}
