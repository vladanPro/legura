# Roadmap

This is a scoped plan, not a completion percentage or promised release date.

## 0. Framework Gates

- Resolve preview/compiled action-validation divergence (framework issue #313).
- Preserve allowlisted, non-secret submitted values after native 422 responses
  (framework issue #314).
- Prove session login/logout, authorization, CSRF, and validation in an isolated
  admin flow. Never treat the presence of primitives as proof of product safety.

## 1. First End-to-End CMS

- Axonyx application with SQLite schema and versioned migrations.
- First-run setup: site name and first administrator.
- One-time setup locking, including concurrent setup requests.
- Password hashing, sessions, login/logout, and protected admin actions.
- Posts list, create/edit/delete, draft/published, and unique-slug handling.
- Simple editor and safe public rendering of published content.
- Empty/error/pending states, accessible controls, and mobile administration.
- API/browser tests for unauthorized requests, invalid input, and publishing.

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
