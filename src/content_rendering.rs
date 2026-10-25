//! The only source of rich HTML for public post pages is validated storage.
use crate::content::{ContentError, PostContent};
use crate::content_preparation::ContentInput;
use axonyx_runtime::native::{AxNativeError, AxNativeRegistry};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RenderedContent {
    pub html: String,
}

pub fn render_content(input: ContentInput) -> Result<RenderedContent, ContentError> {
    PostContent::from_stored(
        &input.content_format,
        &input.body,
        input.content_document.as_deref(),
    )
    .map(|content| RenderedContent {
        html: content.render_html(),
    })
}

pub fn register(registry: &mut AxNativeRegistry) -> Result<(), AxNativeError> {
    registry.register("legura.content.render", render_content)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn only_validated_storage_becomes_markup() {
        let json = json!({"format":"legura-richtext","version":1,"blocks":[
            {"type":"heading","level":2,"children":[{"text":"<Title>","marks":["bold"]}]},
            {"type":"list-item","list":"ordered","children":[{"text":"Read","marks":["italic"],"link":"https://example.com/?a=1&b=2"}]}
        ]}).to_string();
        let rendered = render_content(ContentInput {
            content_format: "legura-richtext".into(),
            body: "<Title>\nRead".into(),
            content_document: Some(json.clone()),
        })
        .unwrap();
        assert!(rendered
            .html
            .contains("<h2><strong>&lt;Title&gt;</strong></h2>"));
        assert!(rendered.html.contains("<ol><li>"));
        assert!(rendered.html.contains("<em>Read</em>"));
        assert!(rendered.html.contains("a=1&amp;b=2"));
        assert!(render_content(ContentInput {
            content_format: "legura-richtext".into(),
            body: "different".into(),
            content_document: Some(json),
        })
        .is_err());
    }

    #[test]
    fn plain_storage_is_literal_and_corrupt_documents_fail_closed() {
        let rendered = render_content(ContentInput {
            content_format: "plain-text".into(),
            body: "<script>alert(1)</script>".into(),
            content_document: None,
        })
        .unwrap();
        assert_eq!(rendered.html, "&lt;script&gt;alert(1)&lt;/script&gt;");
        for document in [
            "{}",
            r#"{"format":"legura-richtext","version":1,"blocks":[{"type":"paragraph","children":[{"text":"private","marks":[],"link":"javascript:alert(1)"}]}]}"#,
        ] {
            assert!(render_content(ContentInput {
                content_format: "legura-richtext".into(),
                body: "private".into(),
                content_document: Some(document.into()),
            })
            .is_err());
        }
    }
}
