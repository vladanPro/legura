//! Read-only content compatibility audit. Never return post bodies or documents.
use crate::content::PostContent;
use rusqlite::{types::ValueRef, Connection};

#[derive(Debug, Default, PartialEq, Eq, serde::Serialize)]
pub struct ContentAudit {
    pub total: u64,
    pub plain: u64,
    pub rich: u64,
    pub invalid: u64,
}

pub fn audit(connection: &Connection) -> rusqlite::Result<ContentAudit> {
    // Bound values before copying into Rust. Oversized/wrong-type storage becomes
    // invalid, never a truncated document that could accidentally pass validation.
    let mut statement = connection.prepare(
        "SELECT
          CASE WHEN typeof(body) = 'text' AND length(CAST(body AS BLOB)) <= 80000 THEN body END,
          CASE WHEN typeof(content_format) = 'text' AND length(CAST(content_format AS BLOB)) <= 64 THEN content_format END,
          CASE WHEN typeof(content_document) = 'text' AND length(CAST(content_document AS BLOB)) <= 32768 THEN content_document END,
          typeof(content_document)
         FROM legura_posts",
    )?;
    let mut rows = statement.query([])?;
    let mut report = ContentAudit::default();
    while let Some(row) = rows.next()? {
        report.total += 1;
        let text = |column| match row.get_ref(column) {
            Ok(ValueRef::Text(bytes)) => std::str::from_utf8(bytes).ok(),
            _ => None,
        };
        let document = text(2);
        let valid = match (text(0), text(1), text(3)) {
            (Some(body), Some(format), Some(kind))
                if kind == "null" || (kind == "text" && document.is_some()) =>
            {
                PostContent::from_stored(format, body, document).is_ok()
            }
            _ => false,
        };
        if !valid {
            report.invalid += 1;
        } else if text(1) == Some("plain-text") {
            report.plain += 1;
        } else {
            report.rich += 1;
        }
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::params;

    fn database() -> Connection {
        let connection = Connection::open_in_memory().unwrap();
        connection
            .execute_batch("CREATE TABLE legura_posts(body, content_format, content_document);")
            .unwrap();
        connection
    }

    fn document(text: &str) -> String {
        serde_json::json!({"format":"legura-richtext","version":1,"blocks":[
            {"type":"paragraph","children":[{"text":text,"marks":[]}]}
        ]})
        .to_string()
    }

    #[test]
    fn valid_legacy_and_rich_rows_are_counted_without_rewriting() {
        let connection = database();
        let json = document("Hello 🙂");
        connection
            .execute(
                "INSERT INTO legura_posts VALUES (?1, 'legura-richtext', ?2)",
                params!["Hello 🙂", json],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO legura_posts VALUES (?1, 'plain-text', NULL)",
                ["<script>literal</script>\n\t"],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO legura_posts VALUES (?1, 'plain-text', NULL)",
                ["\t\n"],
            )
            .unwrap();
        let before = connection.total_changes();
        assert_eq!(
            audit(&connection).unwrap(),
            ContentAudit {
                total: 3,
                plain: 2,
                rich: 1,
                invalid: 0
            }
        );
        assert_eq!(connection.total_changes(), before);
        let stored: String = connection
            .query_row(
                "SELECT content_document FROM legura_posts WHERE content_format='legura-richtext'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(stored, json);
    }

    #[test]
    fn invalid_storage_is_counted_not_repaired_or_included_in_report() {
        let connection = database();
        for (body, format, json) in [
            ("secret post", "html", None),
            ("secret post", "plain-text", Some("{}".into())),
            ("secret post", "legura-richtext", None),
            (
                "secret post",
                "legura-richtext",
                Some("{bad private json".into()),
            ),
            ("mismatch", "legura-richtext", Some(document("private"))),
            ("", "plain-text", None),
            ("body", "legura-richtext", Some("x".repeat(32769))),
        ] {
            connection
                .execute(
                    "INSERT INTO legura_posts VALUES (?1, ?2, ?3)",
                    params![body, format, json],
                )
                .unwrap();
        }
        connection.execute_batch("INSERT INTO legura_posts VALUES (42, 'plain-text', NULL), ('private', 'plain-text', x'00'), (CAST(x'FF' AS TEXT), 'plain-text', NULL);").unwrap();
        let before = connection.total_changes();
        let report = audit(&connection).unwrap();
        assert_eq!(
            report,
            ContentAudit {
                total: 10,
                invalid: 10,
                ..ContentAudit::default()
            }
        );
        assert_eq!(connection.total_changes(), before);
        assert_eq!(
            serde_json::to_string(&report).unwrap(),
            "{\"total\":10,\"plain\":0,\"rich\":0,\"invalid\":10}"
        );
    }

    #[test]
    fn empty_and_incompatible_schemas_are_distinguished() {
        assert_eq!(audit(&database()).unwrap(), ContentAudit::default());
        let connection = Connection::open_in_memory().unwrap();
        connection
            .execute_batch("CREATE TABLE legura_posts(body TEXT);")
            .unwrap();
        assert!(audit(&connection).is_err());
        assert_eq!(connection.total_changes(), 0);
    }

    #[test]
    fn body_budget_counts_unicode_scalars_without_accepting_truncation() {
        let connection = database();
        for body in ["🙂".repeat(20000), "🙂".repeat(20001)] {
            connection
                .execute(
                    "INSERT INTO legura_posts VALUES (?1, 'plain-text', NULL)",
                    [body],
                )
                .unwrap();
        }
        assert_eq!(
            audit(&connection).unwrap(),
            ContentAudit {
                total: 2,
                plain: 1,
                rich: 0,
                invalid: 1
            }
        );
    }
}
