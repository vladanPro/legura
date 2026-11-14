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

Media Deletion Service V0 adds an administrator-checked Rust service
that refuses referenced images and commits metadata removal before file cleanup.
Cleanup failure is explicit, not reported as complete deletion. See
media-deletion-service-v0.md. No operator installation is changed by this work.

Media Deletion UI V0 adds native confirmation, a guarded action, operation-level
validation feedback and an honest cleanup warning. See media-deletion-ui-v0.md;
isolated compiled HTTP/browser acceptance and required PR CI passed.

Media Cleanup Receipts V0 records private-byte cleanup intent in the same
transaction as metadata deletion and gives administrators a bounded retry path
after failure or restart. See media-cleanup-receipts-v0.md. This is not automatic
garbage collection or an operator deployment.

Post Image Foundation V0 adds an optional per-post featured-image association,
post-specific alt text, guarded transactional selection and lossless rollback.
See post-image-foundation-v0.md. This persistence phase does not yet add an
editor picker, registered HTTP mutation or public delivery of draft/private media.
Private Post Image Picker V0 subsequently adds a saved-post image screen,
guarded save/remove actions and JS/no-JS forms. See post-image-picker-v0.md.
Public delivery and inline rich-text images remain separate gates.

Published Image Read V0 adds a Rust-only current-publication/association grant
and reuses exact-byte validation. See published-image-read-v0.md. It does not
register a public endpoint; a distinct public native binary transport and actual
anonymous HTTP/browser acceptance are still required.

Public Post Images V0 connects that service through Axonyx 0.8.0 public binary
callbacks, with per-request published association checks, escaped alt text and
an unavailable-image placeholder. See public-post-images-v0.md for current
acceptance evidence; library access remains private and inline images are later.

Product Media Library V0 promotes the tested route into `/admin/media` with
server-owned conditional navigation and default-off storage. See
product-media-library-v0.md for opt-in and acceptance boundaries. The
private-media-http-v0.md extension adds protected image previews; coordinated
registry acceptance, post attachments, deletion and operator activation remain
separate gates.

Rich-text Migration V1 is integrated into dev: migration 003 preserves legacy
plain rows, blocks destructive rich rollback, and supports old/new snapshots.
See richtext-migration-v1.md. Public rich rendering is integrated into dev;
see public-richtext-rendering-v0.md. Private rich preview now reuses that renderer
under administrator guards; see posts-preview-v0.md. Operator editor activation remains separate
work; no operator database is upgraded by these tests.

Editor Storage Validation V0 adds read-only server revalidation in the isolated
edit loaders before mounting a document. See editor-storage-validation-v0.md.
This does not activate the editor in the operator installation or provide a
damaged-document repair screen. Storage validation is integrated into
dev; the native retry source proof is also integrated (editor-native-retry-v0.md),
but still needs a runtime release/editor pin update before registry activation.

Local operator acceptance now covers the maintenance wrapper, live SQLite backup,
verified no-clobber recovery and unchanged running application/configuration.
See operator-quickstart.md. Actual content/login recovery remains covered by
the isolated Backup / Restore HTTP gate. This is not a prebuilt installation,
service supervisor, automatic upgrade or production release.

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
see posts-delete-v1.md. Post Trash supersedes direct permanent deletion:
move a post to Trash from its edit screen, then restore it as a draft or
confirm permanent deletion from Trash. Public post and featured-image URLs
are unavailable while the post is trashed or restored but unpublished.
Trash keeps the post's slug reserved and does not expire automatically.
Standalone Pages have the same Trash lifecycle. Trashing unpublishes the page
and hides its menu entries. Restoring keeps it private until explicitly
republished; permanent deletion removes the page's menu entries.
Its follow-ups include concurrent-conflict and oversized-request editor recovery.
Normal duplicate-slug feedback and long native 422 retries are covered by the
editor validation change. See posts-v0.md for acceptance and limitations.

Acceptance: a fresh installation can create an administrator, publish a post,
restart without data loss, and serve the post publicly while keeping drafts and
write operations private.

Administration listing adds status filters, bounded ten-row SQLite pagination
and literal title search through native GET forms. See posts-listing-v0.md for
the registry tooling requirement, acceptance coverage and Unicode/adapter limits.
New posts record their creator and UTC creation time. The admin list shows both;
legacy rows stay unknown, and account email is not sent to public pages.

General Settings manages public header title, tagline and icon, plus
installation/public addresses, the administrative contact, and content language.
Regional preferences are stored but not yet applied to date or calendar rendering.
Public registration and Subscriber accounts are intentionally unavailable.

The administration overview shows guarded database-backed total/draft/published
counts and links to those lists, plus a first-draft prompt for empty installations.
A separate Trash card links to both collections with independent counts; trashed
posts are excluded from the ordinary post counts.
See admin-overview-v0.md for the behavior and acceptance gate.

Private saved preview reuses the guarded editor loader and the public story
renderer, opening a separate tab without saving/publishing. It is not an
unsaved live preview or a shareable token. See posts-preview-v0.md.

## 2. Self-Hosted Release Gate

Media image validation adds a bounded still-image contract before upload transport:
JPEG/PNG/WebP decode, EXIF orientation, re-encoding and input/output/pixel limits.
See media-image-validation-v0.md. Storage and image validation alone do not provide
an upload endpoint, media library or media-inclusive backup/restore.

Media Upload Adapter V0 connects validated bytes to the existing AxFileStorage
trait with a compiled/preview action proof; see media-upload-adapter-v0.md.
Application storage registration in generated startup remains an integration
gate. It is not yet a CMS upload route or an authorization/CSRF proof.

Rich-text storage has a validated Rust content contract, SQLite compatibility
proof and native package bridge. Real create/edit forms now use the locked
editor generation; see active-editor-v0.md. Plain/rich conversion is explicit
future work, not automatic. Applying changes to an existing operator installation
still requires backup/recovery rehearsal; there is no production updater.

Native content now also has a disposable compiled HTTP proof with
real setup/session/administrator/CSRF checks, SQLite persistence, restart and
safe stored-content rehydration. See content-http-proof-v0.md. Local Windows
acceptance passes; the registry-only editor gate still requires Linux CI. This does not activate
rich storage or the editor in the operator CMS.

The isolated browser slice imports the pinned RichTextEditor component/assets
through the embedded package bundle, with test-only create/edit forms and native content actions.
See content-editor-browser-v0.md for Chromium save/reload/422/lifecycle and no-JS
read-only acceptance. This still does not activate the operator editor/migration.

Editor Managed Installation V0 adds explicit side-by-side generations, read-only
checks and refusal of local asset/receipt conflicts. The disposable browser proof
uses the reported component/CSS/module paths. See editor-managed-install-v0.md.
Activation, interrupted-install recovery and the broader upgrade gate remain open.

Editor Content Check V0 adds a local read-only compatibility scan of all stored
post formats/documents, with count-only output and failure on invalid content.
See editor-content-check-v0.md. It is one pre-activation check, not activation or
a substitute for tested backups, runtime compatibility and recovery.

Native Package V0 adds a runtime-only directory (server, maintenance tool, assets
and file inventory), with disposable relocated/no-toolchain acceptance. It is
for a restored installation or a new SQLite DB using embedded migrations.
Fresh DB init and native local HTTP configuration are no-clobber. Session/setup
secrets are generated independently from OS randomness. A new-only HTTPS-mode
configuration now supports external SQLite/media paths, but TLS, proxy binding,
service supervision and live upgrades remain operator-managed.
The same-host fresh-package rehearsal now also verifies publication after
restart and live backup/restore into a new DB with rotated session secrets.
Windows acceptance and the Linux package smoke passed, including fresh setup,
publication, restart and backup/restore in disposable installations. This is
not yet a public download, signed artifact or supported production upgrade.
See native-package-v0.md. No signed archive or production platform matrix yet.
The native-package smoke now includes a disposable same-version replacement:
an invalid candidate leaves the old server online; a verified candidate checks
a media-inclusive archive and starts against external SQLite/media, preserving
an uploaded image, content, login and configuration. A valid candidate with bad
media configuration also fails startup, after which the original package is
restarted and its content checked. A separate CI gate builds the pinned `0.0.1`
and current `0.0.2` maintenance tools and proves that the new reader restores
old database-only and media-inclusive backups, while the old reader rejects
new backups. A separate native-package CI gate builds both versions and tests
a disposable `0.0.1` to `0.0.2` server cutover over external SQLite/media,
preserving a published post, fresh administrator login and exact private image
bytes.
Operator-driven upgrade, schema evolution and rollback after new writes remain
open. The isolated cutover gate now verifies archives from both sides of
a new `0.0.2` post and proves that an automatic return to the `0.0.1` snapshot
would discard it. The final test archives now follow a stopped-writer boundary
before verification and inactive recovery; this proves only the isolated
single-server flow, not a multi-instance write freeze. The
[operator rehearsal](native-package-v0.md#operator-rehearsal-backup-to-preflight)
ends at read-only preflight. The separate `upgrade-rehearse` command can apply
schema migrations to an inactive copy and verify a new archive without cutover.
See [post-cutover recovery](post-cutover-recovery-v0.md) for rollback limits.

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

Public Theme Boundary V0 separates the public home/blog Foundry root from the
stable setup/login/admin appearance without changing content or auth routes.
The public default is fixed; theme selection and installation remain later
product work. The `/posts` subtree has public error/404 pages, while global
fallbacks still use the backoffice frame. See public-theme-boundary-v0.md.

The `.legura` archive V0 container packages a supplied frozen SQLite/media snapshot
as standard TAR with a final hash inventory and read-only safe verification.
Its source-pilot archive-backup command now captures live SQLite through the
backup API before reading referenced images. Inactive no-clobber recovery now
copies to a NEW private directory, preserving keys and checking SQLite/FKs plus
exact media references. The separate read-only recovery check validates current
product migration history, actual schema, content and original archived bytes.
Configuration/service switching is not provided. See legura-archive-v0.md and
recovery-compatibility-v0.md.
The existing database-only backup refusal remains in place.

Read-only Media Integrity Check V0 verifies referenced objects/size/SHA-256 via
an explicit local operator command, without repair or activation. Database-only
backup refuses media references. See media-integrity-check-v0.md. Coordinated
SQLite + file recovery is source-pilot only; current-schema compatibility now has
an explicit read-only gate. Separately approved activation and historical upgrade
policy remain next; no production installer is claimed.
The isolated media smoke now exercises recovery activation with independent
session secrets, fresh login, separate writes/restart, failed-start refusal and
return to the untouched source. This is acceptance only, not operator/service
activation; see recovery-activation-proof-v0.md.
Native media configuration now uses the product startup hook, shared application
DB settings and explicit disabled/local modes rather than a test-only adapter.
The startup hook is published in CLI 0.6.15 and Media CI installs that exact
registry version. Operator activation remains separate; see native-media-config-v0.md.
New-only private recovery configuration generation now prepares validated paths
and independent secrets without switching services or replacing settings; see
recovery-private-config-v0.md. Registry CLI consumer acceptance is required before
merging the adoption PR; no operator upgrade is performed by that test.

Media Library View V0 builds on the isolated upload proof with Foundry admin
navigation, empty state, processed metadata, bounded pagination and a native
upload-to-redirect flow. See media-library-view-v0.md. Product activation and
media-aware recovery remain separate gates; no public object serving is enabled.

An isolated Media Upload HTTP source pilot exercises the current administrator,
CSRF, canonical image storage and SQLite metadata through the generated server;
see media-upload-http-v0.md. Its migration/route/startup hook are fixture-only.
No operator media activation, production library or media backup is implied.

Media Storage V0 begins with an inactive local adapter and provider-neutral object
references; see media-storage-v0.md. Upload/image validation, library UI, post
images and media-aware backups are separate gates, not shipped by this foundation.

- Media library with storage limits, safe upload handling, and deletion policy.
- Pages V0: standalone plain-text drafts, private saved preview and public
  URLs under `/pages/`. Root aliases depend on Axonyx route precedence so
  dynamic pages do not shadow static pages or assets. Navigation V0 supports
  manually curated header/footer menus. SEO fields and a richer editorial
  workflow remain open.
- Themes with stable content/rendering contracts and installation boundaries.
- Roles and permissions based on demonstrated needs.
- Extension API, then optional block editor and marketplace research.

Expand only after the first end-to-end CMS is usable without its authors.
