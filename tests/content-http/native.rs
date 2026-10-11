use axonyx_runtime::native::{AxNativeError, AxNativeRegistry};
use legura::content_preparation::{prepare_content_form, ContentFormInput, ContentInput};
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
struct RenderedContent {
    html: String,
}

pub fn register(registry: &mut AxNativeRegistry) -> Result<(), AxNativeError> {
    registry.register("legura.content.prepare", |input: ContentFormInput| {
        Ok::<_, ()>(prepare_content_form(input))
    })?;
    registry.register("legura.content.render", |input: ContentInput| {
        legura::content::PostContent::from_stored(
            &input.content_format,
            &input.body,
            input.content_document.as_deref(),
        )
        .map(|content| RenderedContent {
            html: content.render_html(),
        })
        .map_err(|_| ())
    })
}
