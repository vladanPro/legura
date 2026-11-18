//! Listing normalization and integer pagination; no database or request access.
use axonyx_runtime::native::{AxNativeError, AxNativeRegistry};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PostFilterInput {
    pub status: String,
    pub page: String,
    pub search: String,
}

#[derive(Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PostFilters {
    pub status: String,
    pub search: String,
    pub requested_page: i64,
}

pub fn normalize_post_filters(input: PostFilterInput) -> PostFilters {
    let status = match input.status.as_str() {
        "draft" | "published" | "trash" => input.status,
        _ => "all".to_owned(),
    };
    PostFilters {
        status,
        search: normalize_search(&input.search),
        requested_page: normalize_page(&input.page),
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct UserFilterInput {
    role: String,
    status: String,
    page: String,
    search: String,
}

#[derive(Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct UserFilters {
    role: String,
    status: String,
    search: String,
    requested_page: i64,
}

fn normalize_user_filters(input: UserFilterInput) -> UserFilters {
    let role = match input.role.as_str() {
        "admin" | "editor" | "subscriber" => input.role,
        _ => "all".to_owned(),
    };
    let status = match input.status.as_str() {
        "active" | "inactive" => input.status,
        _ => "all".to_owned(),
    };
    UserFilters {
        role,
        status,
        search: normalize_search(&input.search),
        requested_page: normalize_page(&input.page),
    }
}

fn normalize_search(search: &str) -> String {
    // Preserve the SQLite trim/substr text-prefix semantics used by Posts.
    search
        .trim_matches(' ')
        .split('\0')
        .next()
        .unwrap_or_default()
        .chars()
        .take(200)
        .collect()
}

fn normalize_page(page: &str) -> i64 {
    let page = page.split('\0').next().unwrap_or_default();
    if !page.is_empty() && page.bytes().all(|byte| byte.is_ascii_digit()) {
        page.bytes()
            .fold(0_i64, |value, digit| {
                value
                    .saturating_mul(10)
                    .saturating_add(i64::from(digit - b'0'))
            })
            .max(1)
    } else {
        1
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PageRequestInput {
    pub page: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PageRequest {
    pub requested_page: i64,
}

pub fn normalize_page_request(input: PageRequestInput) -> PageRequest {
    PageRequest {
        requested_page: normalize_page(&input.page),
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PaginationInput {
    pub total: i64,
    pub requested_page: i64,
    pub page_size: i64,
}

#[derive(Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Pagination {
    pub page: i64,
    pub total_pages: i64,
    pub offset: i64,
    pub page_size: i64,
}

pub fn paginate(input: PaginationInput) -> Result<Pagination, &'static str> {
    if input.total < 0 || input.page_size < 1 {
        return Err("invalid pagination bounds");
    }
    let total_pages =
        (input.total / input.page_size + i64::from(input.total % input.page_size != 0)).max(1);
    let page = input.requested_page.clamp(1, total_pages);
    Ok(Pagination {
        page,
        total_pages,
        offset: (page - 1) * input.page_size,
        page_size: input.page_size,
    })
}

pub fn register(registry: &mut AxNativeRegistry) -> Result<(), AxNativeError> {
    registry.register("legura.pagination.request", |input: PageRequestInput| {
        Ok::<_, ()>(normalize_page_request(input))
    })?;
    registry.register("legura.posts.filters", |input: PostFilterInput| {
        Ok::<_, ()>(normalize_post_filters(input))
    })?;
    registry.register("legura.users.filters", |input: UserFilterInput| {
        Ok::<_, ()>(normalize_user_filters(input))
    })?;
    registry.register("legura.pagination", paginate)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_filters_accept_only_account_roles_and_keep_literal_search_text() {
        for (role, expected) in [
            ("admin", "admin"),
            ("editor", "editor"),
            ("subscriber", "subscriber"),
            ("trash", "all"),
        ] {
            assert_eq!(
                normalize_user_filters(UserFilterInput {
                    role: role.to_owned(),
                    status: "all".to_owned(),
                    page: "003".to_owned(),
                    search: "  QA_%  ".to_owned(),
                }),
                UserFilters {
                    role: expected.to_owned(),
                    status: "all".to_owned(),
                    search: "QA_%".to_owned(),
                    requested_page: 3
                },
            );
        }
    }

    #[test]
    fn user_status_filters_are_allowlisted_and_compose_with_roles() {
        for (status, expected) in [
            ("active", "active"),
            ("inactive", "inactive"),
            ("all", "all"),
            ("", "all"),
            ("ACTIVE", "all"),
            ("inactive' OR 1=1 --", "all"),
        ] {
            let filters = normalize_user_filters(UserFilterInput {
                role: "editor".to_owned(),
                status: status.to_owned(),
                page: "999".to_owned(),
                search: "  QA_%  ".to_owned(),
            });
            assert_eq!(filters.status, expected);
            assert_eq!(filters.role, "editor");
            assert_eq!(filters.search, "QA_%");
            assert_eq!(filters.requested_page, 999);
        }
    }

    #[test]
    fn filter_normalization_matches_the_previous_sqlite_expression() {
        let db = rusqlite::Connection::open_in_memory().unwrap();
        let pages = [
            "",
            "0",
            "1",
            "0012",
            "-1",
            "1.2",
            " 2",
            "+2",
            "１２",
            "2e3",
            "9223372036854775808",
            "999999999999999999999999999999",
            "2\0x",
            "\0",
        ];
        for status in [
            "draft",
            "published",
            "all",
            "Draft",
            " draft",
            "unknown",
            "draft\0x",
        ] {
            for search in [
                "",
                "  hello  ",
                "\thello\t",
                "%_",
                "élan",
                "🙂 a",
                "a \0 b",
                "  \0  ",
            ] {
                for page in pages {
                    let expected: (String, String, i64) = db.query_row(
                        "SELECT CASE WHEN ?1 IN ('draft', 'published', 'trash') THEN ?1 ELSE 'all' END, substr(trim(?2), 1, 200), CASE WHEN ?3 <> '' AND ?3 NOT GLOB '*[^0-9]*' THEN MAX(CAST(?3 AS INTEGER), 1) ELSE 1 END",
                        rusqlite::params![status, search, page], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                    ).unwrap();
                    let actual = normalize_post_filters(PostFilterInput {
                        status: status.into(),
                        search: search.into(),
                        page: page.into(),
                    });
                    assert_eq!(
                        (actual.status, actual.search, actual.requested_page),
                        expected,
                        "{status:?} {search:?} {page:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn search_bounds_count_unicode_scalars_not_utf16_or_bytes() {
        let input = format!("  {}  ", "🙂".repeat(201));
        let actual = normalize_post_filters(PostFilterInput {
            status: "all".into(),
            search: input.clone(),
            page: "1".into(),
        });
        let db = rusqlite::Connection::open_in_memory().unwrap();
        let expected: String = db
            .query_row("SELECT substr(trim(?), 1, 200)", [input], |row| row.get(0))
            .unwrap();
        assert_eq!(actual.search, expected);
        assert_eq!(actual.search.chars().count(), 200);
    }

    #[test]
    fn pagination_matches_sql_for_realistic_totals_and_pages() {
        let db = rusqlite::Connection::open_in_memory().unwrap();
        for total in [0, 1, 9, 10, 11, 20, 21, 1000] {
            for requested_page in [1, 2, 10, 100, i64::MAX] {
                let expected: i64 = db
                    .query_row(
                        "SELECT MIN(?1, MAX((?2 + 9) / 10, 1))",
                        rusqlite::params![requested_page, total],
                        |row| row.get(0),
                    )
                    .unwrap();
                let actual = paginate(PaginationInput {
                    total,
                    requested_page,
                    page_size: 10,
                })
                .unwrap();
                assert_eq!(actual.page, expected);
                assert_eq!(actual.offset, (expected - 1) * 10);
            }
        }
    }

    #[test]
    fn pagination_is_exact_and_bounded_without_integer_overflow() {
        for total in [0, 1, i64::MAX - 1, i64::MAX] {
            for page_size in [1, 10, i64::MAX] {
                let actual = paginate(PaginationInput {
                    total,
                    requested_page: i64::MAX,
                    page_size,
                })
                .unwrap();
                assert_eq!(actual.page, actual.total_pages);
                assert!(actual.offset <= total);
                assert!(actual.offset >= 0);
            }
        }
        assert_eq!(
            paginate(PaginationInput {
                total: 11,
                requested_page: -10,
                page_size: 10
            })
            .unwrap()
            .page,
            1
        );
        assert!(paginate(PaginationInput {
            total: -1,
            requested_page: 1,
            page_size: 10
        })
        .is_err());
        assert!(paginate(PaginationInput {
            total: 1,
            requested_page: 1,
            page_size: 0
        })
        .is_err());
    }

    #[test]
    fn native_callbacks_enforce_types_and_fail_closed() {
        let mut registry = AxNativeRegistry::default();
        register(&mut registry).unwrap();
        assert!(register(&mut registry).is_err());
        for payload in [
            serde_json::json!({"page": 2}),
            serde_json::json!({"page": "2", "extra": true}),
            serde_json::json!({}),
        ] {
            assert!(registry.call("legura.pagination.request", payload).is_err());
        }
        for (page, expected) in [
            ("", 1),
            ("-1", 1),
            ("0012", 12),
            ("999999999999999999999", i64::MAX),
        ] {
            assert_eq!(
                registry
                    .call(
                        "legura.pagination.request",
                        serde_json::json!({"page": page})
                    )
                    .unwrap(),
                serde_json::json!({"requested_page": expected})
            );
        }
        for payload in [
            serde_json::json!({"total": "11", "requested_page": 1, "page_size": 10}),
            serde_json::json!({"total": 11, "requested_page": 1, "page_size": 10, "extra": true}),
            serde_json::json!({"total": -1, "requested_page": 1, "page_size": 10}),
        ] {
            assert!(registry.call("legura.pagination", payload).is_err());
        }
        let value = registry
            .call(
                "legura.pagination",
                serde_json::json!({"total": 11, "requested_page": 99, "page_size": 10}),
            )
            .unwrap();
        assert_eq!(
            value,
            serde_json::json!({"page": 2, "total_pages": 2, "offset": 10, "page_size": 10})
        );
    }
}
