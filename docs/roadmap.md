# Roadmap

This is a scoped plan, not a completion percentage or promised release date.

## 0. Framework Gates

- Completed: preview/compiled action-validation parity and allowlisted,
  non-secret submitted values after native 422 responses (issues #313/#314).
- Completed on source tooling: typed action queries, backend expression parity
  and safe compiled action conflicts (issues #323/#324), now published.
- Verified on 2026-10-09: isolated compiled HTTP setup/login/logout acceptance,
  authorization, CSRF, validation, concurrent setup rollback and persistence.
  See setup-auth-v0.md for the exact boundaries and reproduction command.
- Completed: core 0.6.4, runtime 0.6.3, CLI 0.6.7 and scaffold 0.6.5 published.
  Registry-only compiled HTTP acceptance passed without a source CLI checkout.
- Completed: Linux registry HTTP acceptance passed; pilot PR #1 merged into dev.
- Browser fixes published: core 0.6.5, runtime 0.6.4, CLI 0.6.8, scaffold 0.6.6.
  Registry-only HTTP and Chromium JS/no-JS flows pass on Windows, including
  native validation retry and explicit redirects. Required Linux registry
  HTTP/browser CI passed; browser PR #2 merged into dev.
- Posts V0 source acceptance passes: create/edit, private drafts, public
  publish/unpublish, escaped text, missing-post responses, constraints and
  persistence. Typed not-found fixes are published in core 0.6.6/runtime 0.6.5/
  CLI 0.6.9/scaffold 0.6.7. Registry pins/CI now use these versions and run both
  auth and posts acceptance. Registry HTTP and browser gates passed and Posts V0
  PR #3 is integrated into dev.
- Editor validation uses published runtime 0.6.6: normal duplicate-slug errors
  are field-level 422 responses, and long native form values fit within the
  unchanged 64 KiB encoded request budget. Registry acceptance passed on Windows
  and Linux; PR #4 is integrated into dev.
- Setup/Login Accessibility V1 adds keyboard navigation, linked field errors
  and 320/390px acceptance in both browser modes. Its Linux CI gate must pass
  before merging; see setup-login-accessibility-v1.md. This is not a WCAG audit.

Never treat the presence of primitives or passing HTTP tests as proof of product
safety. Query-guard redirects remain a separate UX task (framework issue #326);
the current pilot intentionally uses explicit 403 guards.

## 1. First End-to-End CMS

- Axonyx application with SQLite schema and versioned migrations.
- First-run setup: site name and first administrator.
- One-time setup locking, including concurrent setup requests.
- Password hashing, sessions, login/logout, and protected admin actions.
- Posts list, create/edit/delete, draft/published, and unique-slug handling.
- Simple editor and safe public rendering of published content.
- Empty/error/pending states, accessible controls, and mobile administration.
- API/browser tests for unauthorized requests, invalid input, and publishing.

Posts V0 deliberately excludes deletion; that is a separate product task.
Post Deletion V1 adds a permanent-delete confirmation route and POST-only,
slug-confirmed mutation. Its registry acceptance gate passed and PR #5 merged;
see posts-delete-v1.md. No trash or undo is included.
Its follow-ups include concurrent-conflict and oversized-request editor recovery.
Normal duplicate-slug feedback and long native 422 retries are covered by the
editor validation change. See posts-v0.md for acceptance and limitations.

Acceptance: a fresh installation can create an administrator, publish a post,
restart without data loss, and serve the post publicly while keeping drafts and
write operations private.

Administration listing adds status filters, bounded ten-row SQLite pagination
and literal title search through native GET forms. See posts-listing-v0.md for
the registry tooling requirement, acceptance coverage and Unicode/adapter limits.

The administration overview shows guarded database-backed total/draft/published
counts and links to those lists, plus a first-draft prompt for empty installations.
See admin-overview-v0.md for the behavior and acceptance gate.

Private saved preview reuses the guarded editor loader and the public story
renderer, opening a separate tab without saving/publishing. It is not an
unsaved live preview or a shareable token. See posts-preview-v0.md.

## 2. Self-Hosted Release Gate

Local source preparation has an executable first-run acceptance gate:
prepare/init/check/start, secret preservation, local-only database/config
safeguards and setup-to-administration proof. See installation.md. This does
not complete production packaging, service supervision or the upgrade gate.

- Document native source build and prebuilt-binary installation.
- Document service startup/restart, reverse proxy, TLS, and configuration.
- Separate replaceable program assets from persistent data and secrets.
- Backup/restore proof, migration failure handling, and upgrade procedure.
- Supported platform matrix, dependency versions, and deployment smoke tests.
- Optional Docker packaging without making Docker a requirement.

Backup / Restore V1 is the current scoped implementation: a local SQLite tool,
verified snapshots and recovery to a new file without replacing the active DB.
See backup-restore-v1.md. Its unit and registry HTTP recovery gates must pass
before merging. This does not close the broader installation/upgrade release gate.

Backup / Restore V1 passed those gates and PR #6 is integrated into dev.

## 3. Further Database Adapters

- PostgreSQL product integration and migration/behavior parity tests.
- MySQL adapter support in Axonyx, then Legura integration with parity tests.
- User-supplied databases host Legura-owned tables; arbitrary existing schemas
  are not automatically understood. Database credentials stay server-only.

## 4. Product Expansion

- Media library with storage limits, safe upload handling, and deletion policy.
- Pages, navigation, SEO fields, and richer editorial workflow.
- Themes with stable content/rendering contracts and installation boundaries.
- Roles and permissions based on demonstrated needs.
- Extension API, then optional block editor and marketplace research.

Expand only after the first end-to-end CMS is usable without its authors.
