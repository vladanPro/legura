//! Explicit post content formats. Never infer rich text from a body string.
use legura_editor::Document;
use std::fmt;

const MAX_BODY_CHARS: usize = 20_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContentError {
    InvalidBody,
    InvalidDocument,
    UnsupportedFormat,
    InconsistentStorage,
    Serialization,
}

impl fmt::Display for ContentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidBody => "Content requires nonblank text of at most 20000 characters",
            Self::InvalidDocument => "Invalid or unsupported rich-text document",
            Self::UnsupportedFormat => "Unsupported post content format",
            Self::InconsistentStorage => "Stored post content fields are inconsistent",
            Self::Serialization => "Could not serialize post content",
        })
    }
}
impl std::error::Error for ContentError {}

/// Untrusted storage DTO. Rehydrate through PostContent::from_stored before use.
pub struct ContentColumns {
    pub body: String,
    pub content_format: &'static str,
    pub content_document: Option<String>,
}

/// Validated content, with no unchecked public constructor.
#[derive(Debug, Clone)]
pub struct PostContent {
    body: String,
    document: Option<Document>,
}

impl PostContent {
    pub fn plain(body: &str) -> Result<Self, ContentError> {
        validate_body(body)?;
        Ok(Self {
            body: body.into(),
            document: None,
        })
    }

    pub fn rich(json: &str) -> Result<Self, ContentError> {
        let document = Document::parse_json(json).map_err(|_| ContentError::InvalidDocument)?;
        let body = document.plain_text();
        // Include block separators in the existing SQLite body budget.
        validate_body(&body)?;
        Ok(Self {
            body,
            document: Some(document),
        })
    }

    pub fn from_stored(format: &str, body: &str, json: Option<&str>) -> Result<Self, ContentError> {
        match (format, json) {
            ("plain-text", None) => {
                // Legacy SQLite rows can contain whitespace accepted by older forms.
                // Reading must not silently tighten editorial policy or rewrite them.
                if body.is_empty() || body.chars().count() > MAX_BODY_CHARS {
                    return Err(ContentError::InvalidBody);
                }
                Ok(Self {
                    body: body.into(),
                    document: None,
                })
            }
            ("legura-richtext", Some(json)) => {
                let content = Self::rich(json)?;
                if content.body != body {
                    return Err(ContentError::InconsistentStorage);
                }
                Ok(content)
            }
            ("plain-text" | "legura-richtext", _) => Err(ContentError::InconsistentStorage),
            _ => Err(ContentError::UnsupportedFormat),
        }
    }

    pub fn storage_columns(&self) -> Result<ContentColumns, ContentError> {
        Ok(ContentColumns {
            body: self.body.clone(),
            content_format: if self.document.is_some() {
                "legura-richtext"
            } else {
                "plain-text"
            },
            content_document: self
                .document
                .as_ref()
                .map(Document::to_json)
                .transpose()
                .map_err(|_| ContentError::Serialization)?,
        })
    }

    /// Fixed server markup, consumed by the public loader after storage validation.
    pub fn render_html(&self) -> String {
        if let Some(document) = &self.document {
            return document.render_html();
        }
        let mut html = String::new();
        for character in self.body.chars() {
            match character {
                '&' => html.push_str("&amp;"),
                '<' => html.push_str("&lt;"),
                '>' => html.push_str("&gt;"),
                '"' => html.push_str("&quot;"),
                '\'' => html.push_str("&#39;"),
                _ => html.push(character),
            }
        }
        html
    }
}

fn validate_body(body: &str) -> Result<(), ContentError> {
    if body.trim().is_empty() || body.chars().count() > MAX_BODY_CHARS {
        return Err(ContentError::InvalidBody);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::{params, Connection};
    use serde_json::json;

    fn document(text: &str) -> String {
        json!({"format":"legura-richtext","version":1,"blocks":[
            {"type":"heading","level":2,"children":[{"text":text,"marks":["bold"]}]},
            {"type":"list-item","list":"unordered","children":[{"text":"Read","marks":[],"link":"https://example.com/?a=1&b=2"}]}
        ]}).to_string()
    }

    #[test]
    fn plain_text_is_never_guessed_or_converted() {
        for body in [
            "<script>private</script>\nNext",
            "{\"format\":\"legura-richtext\"}",
            "🙂 Plain",
            "\t\n",
            "\u{00a0}",
        ] {
            let content = PostContent::from_stored("plain-text", body, None).unwrap();
            let stored = content.storage_columns().unwrap();
            assert_eq!(stored.body, body);
            assert_eq!(stored.content_format, "plain-text");
            assert!(stored.content_document.is_none());
            assert!(!content.render_html().contains("<script>"));
        }
    }

    #[test]
    fn rich_content_round_trips_without_using_html_as_storage() {
        let content = PostContent::rich(&document("<Title>🙂")).unwrap();
        let stored = content.storage_columns().unwrap();
        assert_eq!(stored.body, "<Title>🙂\nRead");
        assert_eq!(stored.content_format, "legura-richtext");
        let reloaded = PostContent::from_stored(
            stored.content_format,
            &stored.body,
            stored.content_document.as_deref(),
        )
        .unwrap();
        assert_eq!(reloaded.render_html(), content.render_html());
        assert!(reloaded
            .render_html()
            .contains("<h2><strong>&lt;Title&gt;🙂</strong></h2>"));
        assert!(reloaded.render_html().contains("<ul><li><a href="));
    }

    #[test]
    fn inconsistent_and_unknown_storage_fails_closed() {
        for (format, body, json) in [
            ("html", "private", None),
            ("plain-text", "private", Some("{}")),
            ("legura-richtext", "private", None),
            ("legura-richtext", "private", Some("{}")),
            (
                "legura-richtext",
                "different",
                Some(document("Title").as_str()),
            ),
        ] {
            let error = PostContent::from_stored(format, body, json).unwrap_err();
            assert!(!error.to_string().contains("private"));
        }
    }

    #[test]
    fn invalid_rich_documents_and_empty_or_oversize_content_are_rejected() {
        for body in ["", " \n\t", &"a".repeat(MAX_BODY_CHARS + 1)] {
            assert!(PostContent::plain(body).is_err());
        }
        let raw = json!({"format":"legura-richtext","version":1,"blocks":[
            {"type":"paragraph","children":[{"text":"private","marks":[],"link":"javascript:private"}]}
        ]}).to_string();
        assert_eq!(
            PostContent::rich(&raw).unwrap_err(),
            ContentError::InvalidDocument
        );
        let blank = json!({"format":"legura-richtext","version":1,"blocks":[
            {"type":"paragraph","children":[{"text":" ","marks":[]}]}
        ]})
        .to_string();
        assert_eq!(
            PostContent::rich(&blank).unwrap_err(),
            ContentError::InvalidBody
        );
        let raw = json!({"format":"legura-richtext","version":1,"blocks":[
            {"type":"paragraph","children":[{"text":"a".repeat(MAX_BODY_CHARS),"marks":[]}]},
            {"type":"paragraph","children":[{"text":"","marks":[]}]}
        ]})
        .to_string();
        assert_eq!(
            PostContent::rich(&raw).unwrap_err(),
            ContentError::InvalidBody
        );
    }

    #[test]
    fn sqlite_migration_preserves_old_rows_and_round_trips_rich_json() {
        let db = Connection::open_in_memory().unwrap();
        db.execute_batch(include_str!(
            "../db/migrations/20261009000000_002_posts/up.sql"
        ))
        .unwrap();
        let old = "<script>literal</script>\nExisting body";
        db.execute("INSERT INTO legura_posts(id,title,slug,body,status) VALUES ('old','Old','old',?1,'draft')", [old]).unwrap();
        db.execute_batch(include_str!(
            "../db/migrations/20261012000000_003_richtext/up.sql"
        ))
        .unwrap();
        let columns: (String, String, Option<String>) = db
            .query_row(
                "SELECT content_format,body,content_document FROM legura_posts WHERE id='old'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(columns, ("plain-text".into(), old.into(), None));
        let stored = PostContent::rich(&document("Title"))
            .unwrap()
            .storage_columns()
            .unwrap();
        db.execute("INSERT INTO legura_posts(id,title,slug,body,status,content_format,content_document) VALUES ('rich','Rich','rich',?1,'published',?2,?3)", params![stored.body, stored.content_format, stored.content_document]).unwrap();
        let loaded = db
            .query_row(
                "SELECT content_format,body,content_document FROM legura_posts WHERE id='rich'",
                [],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, Option<String>>(2)?,
                    ))
                },
            )
            .unwrap();
        assert!(
            PostContent::from_stored(&loaded.0, &loaded.1, loaded.2.as_deref())
                .unwrap()
                .render_html()
                .contains("<h2>")
        );
        assert!(db
            .execute(
                "UPDATE legura_posts SET content_document=NULL WHERE id='rich'",
                []
            )
            .is_err());
        assert!(db
            .execute(
                "UPDATE legura_posts SET content_format='html' WHERE id='old'",
                []
            )
            .is_err());
        assert_eq!(
            db.query_row("SELECT body FROM legura_posts WHERE id='old'", [], |row| {
                row.get::<_, String>(0)
            })
            .unwrap(),
            old
        );
    }
}
