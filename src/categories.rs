//! Pure category form validation; SQL constraints remain the final write boundary.
use axonyx_runtime::native::{AxNativeError, AxNativeRegistry};
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CategoryForm {
    name: String,
    slug: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct CategoryValidation {
    name: bool,
    slug: bool,
}

fn validate(form: CategoryForm) -> CategoryValidation {
    CategoryValidation {
        name: (1..=80).contains(&form.name.chars().count())
            && form.name.trim_matches(' ') == form.name
            && !form.name.contains(['\0', '\n', '\r']),
        slug: (1..=120).contains(&form.slug.len())
            && form
                .slug
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
            && form
                .slug
                .as_bytes()
                .first()
                .is_some_and(u8::is_ascii_alphanumeric)
            && form
                .slug
                .as_bytes()
                .last()
                .is_some_and(u8::is_ascii_alphanumeric),
    }
}

pub fn register(registry: &mut AxNativeRegistry) -> Result<(), AxNativeError> {
    registry.register("legura.categories.validate", |form: CategoryForm| {
        Ok::<_, ()>(validate(form))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn form_rules_match_the_database_for_boundary_and_malformed_values() {
        let db = rusqlite::Connection::open_in_memory().unwrap();
        for (_, up, _) in crate::database_contract::MIGRATIONS {
            db.execute_batch(up).unwrap();
        }
        for name in [
            "".into(),
            "Valid".into(),
            " leading".into(),
            "trailing ".into(),
            "a\0b".into(),
            "a\nb".into(),
            "a\rb".into(),
            "\u{17e}".repeat(80),
            "x".repeat(81),
        ] {
            for slug in [
                "", "valid", "new-2", "-bad", "bad-", "Bad", "a/b", "a\0b", "a b",
            ] {
                let checked = validate(CategoryForm {
                    name: name.clone(),
                    slug: slug.into(),
                });
                let accepted = db
                    .execute(
                        "INSERT INTO legura_categories VALUES ('test',?1,?2)",
                        rusqlite::params![name, slug],
                    )
                    .is_ok();
                assert_eq!(accepted, checked.name && checked.slug, "{name:?}, {slug:?}");
                db.execute("DELETE FROM legura_categories", []).unwrap();
            }
        }
        assert!(
            !validate(CategoryForm {
                name: "Valid".into(),
                slug: "x".repeat(121)
            })
            .slug
        );
    }
}
