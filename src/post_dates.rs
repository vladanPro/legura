use axonyx_runtime::native::{AxNativeError, AxNativeRegistry};
use chrono::DateTime;
use chrono_tz::Tz;
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
struct PostDates {
    posts: Vec<PostRow>,
    timezone: String,
    date_format: String,
    time_format: String,
}

#[derive(Deserialize, Serialize)]
struct PostRow {
    id: String,
    title: String,
    slug: String,
    status: String,
    author: Option<String>,
    created_at: Option<String>,
    #[serde(default)]
    created_label: Option<String>,
}

fn format_creation(timestamp: &str, zone: Tz, date: &str, time: &str) -> Option<String> {
    let local = DateTime::parse_from_rfc3339(timestamp)
        .ok()?
        .with_timezone(&zone);
    let date = match date {
        "dd/mm/yyyy" => "%d/%m/%Y",
        "mm/dd/yyyy" => "%m/%d/%Y",
        _ => "%Y-%m-%d",
    };
    let time = if time == "12h" { "%-I:%M %p" } else { "%H:%M" };
    Some(format!("{} {}", local.format(date), local.format(time)))
}

fn format_posts(mut input: PostDates) -> Vec<PostRow> {
    let zone = input.timezone.parse().unwrap_or(Tz::UTC);
    for post in &mut input.posts {
        post.created_label = post.created_at.as_deref().and_then(|created| {
            format_creation(created, zone, &input.date_format, &input.time_format)
        });
    }
    input.posts
}

pub fn register(registry: &mut AxNativeRegistry) -> Result<(), AxNativeError> {
    registry.register("legura.posts.format_dates", |input: PostDates| {
        Ok::<_, ()>(format_posts(input))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_local_date_rollover_and_daylight_saving() {
        let belgrade = "Europe/Belgrade".parse().unwrap();
        assert_eq!(
            format_creation("2026-07-01T22:15:00Z", belgrade, "dd/mm/yyyy", "12h").as_deref(),
            Some("02/07/2026 12:15 AM")
        );
        assert_eq!(
            format_creation("2026-01-01T22:15:00Z", belgrade, "mm/dd/yyyy", "24h").as_deref(),
            Some("01/01/2026 23:15")
        );
        assert_eq!(format_creation("invalid", Tz::UTC, "", ""), None);
    }
}
