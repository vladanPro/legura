# Posts V0

## Product Contract

One current database administrator can list, create and edit posts. Each post
has a title, unique lowercase ASCII slug, plain-text body, and draft/published
status. Public readers see published posts only. Unpublishing removes both
the public list entry and detail response. This is not rich text or a block editor.

Routes: /admin/posts, /admin/posts/new, /admin/posts/:id/edit, /posts and
/posts/:slug. All private queries and mutations check current admin identity;
hidden form IDs are untrusted and must resolve to a real post on the server.

SQLite owns uniqueness and value constraints, including concurrent writes.
Public content is escaped ASX text, never raw HTML. Content reads are dynamic,
not build-time exports. No database or authorization behavior moves to JavaScript.

## Acceptance Gate

Isolated compiled HTTP and Chromium tests must prove anonymous write rejection,
CSRF rejection, invalid input, duplicate slug conflicts, persistence/restart,
private draft visibility, create/edit/publish/unpublish, missing IDs and safe
text rendering. Keep existing auth tests. Use published Axonyx packages only.

V0 excludes deletion, media, rich-text formatting, revisions, pagination, search,
multi-user permissions and optimistic concurrency. Updates are last-write-wins;
do not claim collaborative editing or production CMS readiness.

## Verification Status (2026-10-09)

Passed on Windows using isolated source overrides: compiled HTTP auth/posts
acceptance and Chromium posts flows with and without JavaScript, desktop and
mobile editor checks. Stored script markup remains plain escaped text. Database
contracts were regenerated from a disposable migrated SQLite database.

Upstream fixes: runtime PR #236 and framework PR #333 preserve typed not-found
guards as 404 instead of query 500/action 422. Source versions are core 0.6.6,
runtime 0.6.5, CLI 0.6.9 and scaffold 0.6.7; they are not yet published.
The application's registry pins remain unchanged until that release is verified.

Reproduce source acceptance (run sequentially):

```powershell
pwsh -File scripts/smoke-auth.ps1 -Posts -ToolManifest ../axonyx-framework/Cargo.toml -RuntimeSource ../axonyx-framework/vendor/axonyx-runtime/crates/axonyx-runtime
pwsh -File scripts/smoke-auth.ps1 -Posts -Mode javascript -ToolManifest ../axonyx-framework/Cargo.toml -RuntimeSource ../axonyx-framework/vendor/axonyx-runtime/crates/axonyx-runtime
pwsh -File scripts/smoke-auth.ps1 -Posts -Mode native -ToolManifest ../axonyx-framework/Cargo.toml -RuntimeSource ../axonyx-framework/vendor/axonyx-runtime/crates/axonyx-runtime
```

Add `-RefreshSchema` to regenerate tracked schema/types from the isolated
fixture with a portable database URL. It never migrates the developer's database.
No source override is committed to Cargo.toml.

## Remaining Gates And UX Limits

- Publish the upstream patch, update registry pins and CI CLI, then repeat HTTP,
  both posts browser modes and existing auth acceptance on registry packages.
- Slug uniqueness/pattern and whitespace constraints safely return generic 409.
  Field-level conflict feedback and recovery are not implemented yet.
- Native 422 replay has the framework's 4 KiB per-field limit. The editor allows
  longer valid content; retry retention of long content is not yet guaranteed.
- Mobile overflow and browser behavior passed; this is not an accessibility or
  security audit, and no deployment/backup/upgrade readiness is claimed.
