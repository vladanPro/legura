//! Read-only storage revalidation before an editor receives a document.
use crate::content::{ContentError, PostContent};
use crate::content_preparation::{ContentInput, PreparedColumns};
use axonyx_runtime::native::{AxNativeError, AxNativeRegistry};

pub fn load_content(input: ContentInput) -> Result<PreparedColumns, ContentError> {
    let content = PostContent::from_stored(
        &input.content_format,
        &input.body,
        input.content_document.as_deref(),
    )?;
    let columns = content.storage_columns()?;
    Ok(PreparedColumns {
        body: columns.body,
        content_format: columns.content_format.into(),
        content_document: columns.content_document,
    })
}

pub fn register(registry: &mut AxNativeRegistry) -> Result<(), AxNativeError> {
    registry.register("legura.content.load", load_content)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn valid_rich_storage_has_canonical_marks_and_json() {
        let document = json!({"format":"legura-richtext","version":1,"blocks":[
            {"type":"paragraph","children":[{"text":"Text","marks":["italic","bold"]}]}
        ]});
        let loaded = load_content(ContentInput {
            content_format: "legura-richtext".into(),
            body: "Text".into(),
            content_document: Some(serde_json::to_string_pretty(&document).unwrap()),
        })
        .unwrap();
        assert_eq!(loaded.body, "Text");
        assert_eq!(loaded.content_format, "legura-richtext");
        let canonical = loaded.content_document.unwrap();
        assert!(!canonical.contains('\n'));
        let normalized: serde_json::Value = serde_json::from_str(&canonical).unwrap();
        assert_eq!(
            normalized["blocks"][0]["children"][0]["marks"],
            json!(["bold", "italic"])
        );
    }

    #[test]
    fn legacy_plain_storage_is_not_tightened_or_converted() {
        for body in ["\t\n ", "<script>literal</script>", "🙂"] {
            let loaded = load_content(ContentInput {
                content_format: "plain-text".into(),
                body: body.into(),
                content_document: None,
            })
            .unwrap();
            assert_eq!(loaded.body, body);
            assert_eq!(loaded.content_format, "plain-text");
            assert!(loaded.content_document.is_none());
        }
    }

    #[test]
    fn corrupt_storage_never_returns_editor_input() {
        let valid = json!({"format":"legura-richtext","version":1,"blocks":[
            {"type":"paragraph","children":[{"text":"Text","marks":[]}]}
        ]})
        .to_string();
        for (format, body, document) in [
            ("plain-text", "Text", Some("{}")),
            ("legura-richtext", "Text", None),
            ("legura-richtext", "Text", Some("{}")),
            ("legura-richtext", "Different", Some(valid.as_str())),
            ("html", "Text", None),
        ] {
            assert!(load_content(ContentInput {
                content_format: format.into(),
                body: body.into(),
                content_document: document.map(str::to_owned),
            })
            .is_err());
        }
    }
}
