use std::collections::BTreeMap;

use axonyx_runtime::native::{AxNativeError, AxNativeRegistry};
use serde::{Deserialize, Serialize};
use url::Url;

#[derive(Deserialize, Serialize)]
struct GeneralForm {
    site_title: String,
    tagline: String,
    site_icon: String,
    legura_url: String,
    site_address: String,
    admin_email: String,
}

#[derive(Serialize)]
struct CheckedForm {
    valid: bool,
    values: GeneralForm,
    errors: BTreeMap<&'static str, &'static str>,
}

fn valid_address(value: &str) -> bool {
    value.is_empty()
        || (value.len() <= 2048
            && (value.starts_with("http://") || value.starts_with("https://"))
            && !value.contains(|c: char| c.is_whitespace() || c.is_control() || c == '\\')
            && Url::parse(value).is_ok_and(|url| {
                url.host_str().is_some()
                    && url.username().is_empty()
                    && url.password().is_none()
                    && url.query().is_none()
                    && url.fragment().is_none()
            }))
}

fn valid_icon(value: &str) -> bool {
    value.is_empty()
        || (value.len() <= 256
            && value.strip_prefix('/').is_some_and(|path| {
                path.split('/').all(|part| {
                    !part.is_empty()
                        && part != "."
                        && part != ".."
                        && part
                            .bytes()
                            .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
                })
            })
            && [".png", ".ico", ".webp", ".jpg", ".jpeg", ".svg"]
                .iter()
                .any(|ext| value.to_ascii_lowercase().ends_with(ext)))
}

fn validate(mut values: GeneralForm) -> CheckedForm {
    for value in [
        &mut values.site_title,
        &mut values.tagline,
        &mut values.site_icon,
        &mut values.legura_url,
        &mut values.site_address,
        &mut values.admin_email,
    ] {
        *value = value.trim().to_owned();
    }
    let mut errors = BTreeMap::new();
    if values.site_title.is_empty() || values.site_title.chars().count() > 120 {
        errors.insert(
            "site_title",
            "Enter a site title of at most 120 characters.",
        );
    }
    if values.tagline.chars().count() > 160 {
        errors.insert("tagline", "Enter a tagline of at most 160 characters.");
    }
    if !valid_icon(&values.site_icon) {
        errors.insert(
            "site_icon",
            "Enter a public image path on this site, such as /favicon.png.",
        );
    }
    if values.admin_email.len() > 254 || !axonyx_runtime::validation::email(&values.admin_email) {
        errors.insert("admin_email", "Enter a valid email address.");
    }
    for (key, value) in [
        ("legura_url", &values.legura_url),
        ("site_address", &values.site_address),
    ] {
        if !valid_address(value) {
            errors.insert(
                key,
                "Enter an HTTP or HTTPS URL without credentials, query or fragment.",
            );
        }
    }
    CheckedForm {
        valid: errors.is_empty(),
        values,
        errors,
    }
}

pub fn register(registry: &mut AxNativeRegistry) -> Result<(), AxNativeError> {
    registry.register("legura.settings.validate", |input: GeneralForm| {
        serde_json::to_value(validate(input))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_accept_only_http_addresses_and_local_icon_paths() {
        assert!(valid_address("https://example.test/blog"));
        assert!(valid_address("http://localhost:3940"));
        assert!(!valid_address("https://user:pass@example.test"));
        assert!(!valid_address("javascript:alert(1)"));
        assert!(!valid_address("https://example.test/?secret=value"));
        assert!(valid_icon("/assets/site-icon.png"));
        for value in [
            "//elsewhere.test/icon.png",
            "/../icon.png",
            "/%2e%2e/icon.png",
            "/file.js",
            "/icon.png?x",
        ] {
            assert!(!valid_icon(value));
        }
    }
}
