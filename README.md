# Legura CMS

A self-hosted content management system built on Axonyx.

Legura takes its name from the Serbian word for alloy: distinct elements forming
one useful whole. Content, design, and application behavior belong together,
without forcing the user to learn the framework underneath.

## Status

Initial setup/auth pilot is integrated into dev. This repository does not yet
contain an installable CMS. There is no production release, installer, or
published package. The isolated compiled setup/auth smoke passes with source
Axonyx tooling, including setup races, CSRF, sessions and authorization. Required
typed-action-query/expression fixes (#323/#324) are published in core 0.6.4,
runtime 0.6.3 and CLI 0.6.7. Registry-only compiled HTTP acceptance passed on
2026-10-09; no source framework checkout is needed for that test.

Browser acceptance subsequently found lost action redirects and missing CSRF
proof on native validation retries in those published versions. The fixes are
published in core 0.6.5/runtime 0.6.4/CLI 0.6.8. Isolated HTTP and Chromium
JS/no-JS acceptance pass with registry packages on Windows (2026-10-09), without
source overrides. Required Linux registry/browser CI passed and browser PR #2
is integrated into dev.

Posts V0 adds private create/edit, draft/published status, public published pages
and plain-text rendering. Its typed not-found framework fixes are published in
core 0.6.6/runtime 0.6.5/CLI 0.6.9/scaffold 0.6.7 after green release CI.
This branch uses registry runtime 0.6.5 and CI CLI 0.6.9; required acceptance now
includes HTTP posts and both browser modes alongside the original auth tests.
The Posts acceptance/integration gate is recorded in docs/posts-v0.md.

## Product

- Own your server, content, files, and database.
- Install and configure through a guided browser setup.
- Write and publish through an approachable administration interface.
- Extend through Axonyx-powered themes and application integrations over time.
- Use native binaries or source builds; Docker is optional.

Legura is an independent product, not a WordPress fork or compatibility layer.
Its official presentation website will be separate from the installed CMS.

## First Milestone

One administrator, secure login/logout, post creation/editing/deletion,
draft/published status, and public published-post pages backed by SQLite.
No page builder, marketplace, or plugin execution platform in this milestone.

See [the roadmap](docs/roadmap.md), [product boundary](docs/product.md), and
[installation model](docs/installation.md).
The [Posts V0 contract](docs/posts-v0.md) records implemented scope and remaining gates.

The [setup/auth pilot](docs/setup-auth-v0.md) documents local initialization and
the acceptance gate. HTTP acceptance is not a security audit or production
readiness proof. Do not deploy this development pilot publicly.

## Development

Feature branches target `dev`. Completed releases move from `dev` to `main`
through a pull request. Only documented, tested behavior should be described as
available. Package names and commands in planning documents are not implemented.

License selection remains pending; public visibility alone is not a license.
