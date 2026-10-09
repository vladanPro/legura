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
  persistence. Registry acceptance is blocked on the typed not-found patch
  (runtime PR #236/framework PR #333) and its release. Do not merge the Posts
  feature before the published-package gate passes.
- Next verify accessible controls and mobile setup/login before
  treating the pilot as a reusable installation flow.

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
Its follow-ups include field-level slug conflict feedback and preserving long
editor text during native validation retries. See posts-v0.md.

Acceptance: a fresh installation can create an administrator, publish a post,
restart without data loss, and serve the post publicly while keeping drafts and
write operations private.

## 2. Self-Hosted Release Gate

- Document native source build and prebuilt-binary installation.
- Document service startup/restart, reverse proxy, TLS, and configuration.
- Separate replaceable program assets from persistent data and secrets.
- Backup/restore proof, migration failure handling, and upgrade procedure.
- Supported platform matrix, dependency versions, and deployment smoke tests.
- Optional Docker packaging without making Docker a requirement.

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
