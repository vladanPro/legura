# Legura CMS

A self-hosted content management system built on Axonyx.

For source formatting, format-on-save and the optional staged-content guard,
see [Source formatting](docs/source-formatting-v1.md).

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
This branch uses registry runtime 0.6.9 and CI CLI 0.6.13; required acceptance now
includes HTTP posts and both browser modes alongside the original auth tests.
The Posts acceptance/integration gate is recorded in docs/posts-v0.md.

Editor validation adds field-level duplicate-slug feedback and preserves long
allowlisted text during native 422 retries within the 64 KiB request budget.
The runtime fix is published as 0.6.6; core, CLI and UI are unchanged. Concurrent
DB conflicts and oversized requests still have separate recovery limitations.

Post Deletion V1 adds a separate permanent-delete confirmation page, current
administrator checks and slug-confirmed POST mutation. It has no trash or undo.
See [the deletion contract](docs/posts-delete-v1.md) for acceptance and boundaries.

Backup / Restore V1 adds a local Rust maintenance tool for verified SQLite
snapshots and recovery to a new database, never overwriting an existing file.
See [the operator procedure](docs/backup-restore-v1.md). Configuration secrets,
encryption, scheduling and future media backup are outside this first version.
For source-pilot start/stop and wrapper commands that locate the shared Cargo
target automatically, see [Local Operator Quickstart](docs/operator-quickstart.md).
The experimental [Native Package V0](docs/native-package-v0.md) runs an already
initialized compatible installation without source or Cargo at runtime. Native
DB initialization and new local HTTP configuration with random secrets are
available through the native maintenance tool; a
production installer remains a separate gate.

Setup/login now have keyboard skip navigation, named forms, linked field errors
and small-screen acceptance with and without JavaScript. See the scoped
[accessibility proof](docs/setup-login-accessibility-v1.md); this is not a full
accessibility audit or production certification.

The shared administration frame composes Foundry AppShell and Sidebar for
overview, posts and editor screens. See [the layout contract](docs/admin-shell-v0.md)
and [Light/Dark appearance](docs/appearance-v0.md). These remain development
pilots, not an installable product release.

The [posts table contract](docs/posts-table-v1.md) explains published Foundry UI
adoption and why search and pagination remain server-owned.

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
The [posts listing pilot](docs/posts-listing-v0.md) documents filtering,
pagination, title search and its compiled cargo-axonyx 0.6.12 tooling requirement.
The [administration overview](docs/admin-overview-v0.md) documents database-backed
post counts, filtered-list shortcuts and the first-draft prompt.
The [private saved preview](docs/posts-preview-v0.md) lets an administrator
review a post without publishing it or losing unsaved editor text.

Create/update share pure post validation through `app/domain/posts.ax`.
The [shared validation proof](docs/shared-post-validation-v0.md) records the
published compiler fix and registry-only HTTP/backup acceptance. The editor
package remains pinned to Git; operator activation is separate.

The rich-text proofs now use published CLI 0.6.13, runtime 0.6.9 and
UI 0.0.86 without source overrides. See [post editor acceptance](docs/post-editor-forms-v0.md).
This is not editor publication or activation in the operator installation.

Public published-post rendering now validates stored content through Rust before
passing fixed rich markup into ASX. See [the rendering boundary](docs/public-richtext-rendering-v0.md).
Private saved preview uses the same validated rich renderer under administrator
guards. [Active form integration](docs/active-editor-v0.md) connects the real
create/edit forms; applying it to an existing operator installation remains a
separate, backup-first step.

Isolated editor loaders also revalidate stored documents on the server before
mounting them, without repairing or rewriting the database. See
[the editor read boundary](docs/editor-storage-validation-v0.md).

Build tooling can export the pinned editor's embedded ASX/CSS/JS bundle to a new
directory with a hash inventory. See [package assets](docs/editor-package-assets-v0.md).
Source prepare/build installs and verifies the pinned generation; it does not
upgrade an existing operator checkout automatically.

The [side-by-side installation helper](docs/editor-managed-install-v0.md) installs
content-addressed editor generations and refuses local changes. Activation stays
explicit; there is no overwrite or automatic cleanup of old generations.

Before activation, [content-check](docs/editor-content-check-v0.md) can audit a
specified SQLite database read-only and report content compatibility counts,
without printing post content or repairing rows.

The [setup/auth pilot](docs/setup-auth-v0.md) documents local initialization and
the acceptance gate. HTTP acceptance is not a security audit or production
readiness proof. Do not deploy this development pilot publicly.

## Development

The [rich-text storage contract](docs/richtext-storage-v0.md) now backs real CMS
forms through the pinned editor package and native Rust validation. New posts
default to rich text, with explicit plain-text creation at `/admin/posts/new/plain`.
Existing plain posts retain their format; rich editing requires JavaScript.
No public CMS release, automatic conversion or operator upgrade is implied.

For a local SQLite trial, follow the [tested source preparation](docs/installation.md#local-sqlite-pilot).
`pwsh -File scripts/local.ps1 -Task prepare` initializes/checks/builds;
`pwsh -File scripts/local.ps1 -Task start` serves the setup screen on localhost.
This is a development pilot, not a production installer or prebuilt release.

Feature branches target `dev`. Completed releases move from `dev` to `main`
through a pull request. Only documented, tested behavior should be described as
available. Package names and commands in planning documents are not implemented.

License selection remains pending; public visibility alone is not a license.
