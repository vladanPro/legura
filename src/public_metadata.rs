use axonyx_runtime::native::{AxNativeError, AxNativeRegistry};
use serde::{Deserialize, Serialize};
use url::Url;

const DESCRIPTION_LIMIT: usize = 160;

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum Collection {
    Posts,
    Pages,
}

impl Collection {
    fn path_segment(&self) -> &'static str {
        match self {
            Self::Posts => "posts",
            Self::Pages => "pages",
        }
    }
}

#[derive(Deserialize)]
struct MetadataInput {
    body: String,
    site_address: String,
    collection: Collection,
    slug: String,
}

#[derive(Deserialize, Serialize)]
struct Metadata {
    description: Option<String>,
    canonical: Option<String>,
}

fn description(body: &str) -> Option<String> {
    let normalized = body.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut chars = normalized.chars();
    let mut excerpt: String = chars.by_ref().take(DESCRIPTION_LIMIT).collect();
    if chars.next().is_some() {
        excerpt.pop();
        excerpt.push('\u{2026}');
    }
    (!excerpt.is_empty()).then_some(excerpt)
}

fn canonical(input: &MetadataInput) -> Option<String> {
    let slug = &input.slug;
    if !crate::general_settings::valid_address(&input.site_address)
        || slug.is_empty()
        || slug.len() > 120
        || slug.starts_with('-')
        || slug.ends_with('-')
        || !slug
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
    {
        return None;
    }
    let mut url = Url::parse(&input.site_address).ok()?;
    url.path_segments_mut()
        .ok()?
        .pop_if_empty()
        .push(input.collection.path_segment())
        .push(slug);
    Some(url.into())
}

pub fn register(registry: &mut AxNativeRegistry) -> Result<(), AxNativeError> {
    registry.register("legura.public.metadata", |input: MetadataInput| {
        Ok::<_, ()>(Metadata {
            description: description(&input.body),
            canonical: canonical(&input),
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn descriptions_use_plain_text_with_a_unicode_safe_cap() {
        assert_eq!(
            description("one\n \t two & three").as_deref(),
            Some("one two & three")
        );
        assert_eq!(description(" \n "), None);
        let exact = "\u{e9}".repeat(DESCRIPTION_LIMIT);
        assert_eq!(description(&exact).as_deref(), Some(exact.as_str()));
        let truncated = description(&(exact + " more")).unwrap();
        assert_eq!(truncated.chars().count(), DESCRIPTION_LIMIT);
        assert!(truncated.ends_with('\u{2026}'));
    }

    #[test]
    fn canonicals_only_use_the_configured_address_and_a_valid_public_path() {
        let mut input = MetadataInput {
            body: String::new(),
            site_address: "https://example.test/journal/".into(),
            collection: Collection::Posts,
            slug: "some-story".into(),
        };
        assert_eq!(
            canonical(&input).as_deref(),
            Some("https://example.test/journal/posts/some-story")
        );
        input.collection = Collection::Pages;
        input.site_address = "https://example.test/journal".into();
        assert_eq!(
            canonical(&input).as_deref(),
            Some("https://example.test/journal/pages/some-story")
        );
        for address in [
            "",
            "//example.test",
            "https://user@example.test",
            "https://example.test/?q=1",
            "https://example.test/#fragment",
        ] {
            input.site_address = address.into();
            assert_eq!(canonical(&input), None);
        }
        input.site_address = "https://example.test/".into();
        input.slug = "../private".into();
        assert_eq!(canonical(&input), None);
    }
}
