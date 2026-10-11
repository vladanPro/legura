//! Recoverable editorial validation; transport/type errors remain registry errors.
use crate::content::{ContentError, PostContent};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContentInput {
    pub content_format: String,
    pub body: String,
    pub content_document: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PreparedColumns {
    pub body: String,
    pub content_format: String,
    pub content_document: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ContentPreparation {
    pub columns: Option<PreparedColumns>,
    pub errors: BTreeMap<String, String>,
}

pub fn prepare_content(input: ContentInput) -> ContentPreparation {
    let result = match input.content_format.as_str() {
        "plain-text" if input.content_document.is_none() => PostContent::plain(&input.body),
        "plain-text" => Err(ContentError::InconsistentStorage),
        "legura-richtext" => input
            .content_document
            .as_deref()
            .ok_or(ContentError::InvalidDocument)
            .and_then(PostContent::rich),
        _ => Err(ContentError::UnsupportedFormat),
    }
    .and_then(|content| content.storage_columns());

    match result {
        Ok(columns) => ContentPreparation {
            columns: Some(PreparedColumns {
                body: columns.body,
                content_format: columns.content_format.into(),
                content_document: columns.content_document,
            }),
            errors: BTreeMap::new(),
        },
        Err(error) => {
            let field = match error {
                ContentError::InvalidBody => "body",
                ContentError::UnsupportedFormat => "content_format",
                _ => "content_document",
            };
            ContentPreparation {
                columns: None,
                errors: BTreeMap::from([(field.into(), error.to_string())]),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn plain_content_remains_literal_and_rich_body_is_derived() {
        let plain = prepare_content(ContentInput {
            content_format: "plain-text".into(),
            body: "<script>literal</script>".into(),
            content_document: None,
        });
        assert!(plain.errors.is_empty());
        assert_eq!(plain.columns.unwrap().body, "<script>literal</script>");
        let document = json!({"format":"legura-richtext","version":1,"blocks":[
            {"type":"paragraph","children":[{"text":"Actual text","marks":["bold"]}]}
        ]})
        .to_string();
        let rich = prepare_content(ContentInput {
            content_format: "legura-richtext".into(),
            body: "forged search projection".into(),
            content_document: Some(document),
        });
        assert!(rich.errors.is_empty());
        let columns = rich.columns.unwrap();
        assert_eq!(columns.body, "Actual text");
        assert_eq!(columns.content_format, "legura-richtext");
        assert!(columns.content_document.is_some());
        let value = serde_json::to_value(&columns).unwrap();
        assert!(value.get("html").is_none());
    }

    #[test]
    fn invalid_content_returns_field_errors_and_no_writable_columns() {
        for (format, body, document, field) in [
            ("plain-text", " ", None, "body"),
            ("plain-text", "private", Some("{}"), "content_document"),
            ("legura-richtext", "private", None, "content_document"),
            (
                "legura-richtext",
                "private",
                Some("private"),
                "content_document",
            ),
            ("html-private", "private", None, "content_format"),
        ] {
            let prepared = prepare_content(ContentInput {
                content_format: format.into(),
                body: body.into(),
                content_document: document.map(str::to_owned),
            });
            assert!(prepared.columns.is_none());
            assert_eq!(prepared.errors.len(), 1);
            assert!(!prepared.errors[field].contains("private"));
        }
    }

    #[test]
    fn input_contract_rejects_extra_fields_and_wrong_types() {
        for value in [
            json!({"content_format":"plain-text","body":"Text","html":"untrusted"}),
            json!({"content_format":"plain-text","body":42}),
            json!({"body":"Text"}),
        ] {
            assert!(serde_json::from_value::<ContentInput>(value).is_err());
        }
    }
}
