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
guards as 404 instead of query 500/action 422. Core 0.6.6, runtime 0.6.5,
CLI 0.6.9 and scaffold 0.6.7 are published after green dev-to-main release PRs
#237 (runtime) and #334 (framework), with package verification and GitHub tags.
The application pins registry runtime 0.6.5 and CI CLI 0.6.9; source overrides
are unnecessary for normal development. Registry acceptance is the merge gate.

Passed on Windows with published packages only: HTTP auth/posts plus the
original auth and new posts Chromium tests in both JavaScript and native modes.
CLI 0.6.9 was installed into a separate test root; no global tool was replaced.
Linux required CI repeats this same sequence before dev integration.

Reproduce registry acceptance (run sequentially with CLI 0.6.9):

```powershell
pwsh -File scripts/smoke-auth.ps1 -Posts
pwsh -File scripts/smoke-auth.ps1 -Mode javascript
pwsh -File scripts/smoke-auth.ps1 -Mode native
pwsh -File scripts/smoke-auth.ps1 -Posts -Mode javascript
pwsh -File scripts/smoke-auth.ps1 -Posts -Mode native
```

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

- Required registry HTTP/posts and both auth/posts browser modes must pass
  locally and in Linux CI before this feature is merged into dev.
- Duplicate slugs detected before a write return 422 with a slug field message;
  editing a post may retain its own slug. DB uniqueness remains authoritative:
  a concurrent conflicting write can still return generic 409, without editor
  recovery. Pattern and whitespace constraint violations also remain generic 409.
- Source runtime removes the 4 KiB replay field cap while retaining the 64 KiB
  encoded request budget, 32-control cap, secret exclusions and explicit form
  allowlist. Source HTTP and JS/no-JS browser acceptance pass, including long
  Unicode text and escaped markup. This runtime fix is not published yet; do not
  merge registry acceptance until the dependency is released and pinned.
- The 20000-character editor limit does not override the server's 64 KiB encoded
  request limit. Large URL-encoded Unicode submissions may receive 413 before
  action validation; that response does not retain the editor text.
- Mobile overflow and browser behavior passed; this is not an accessibility or
  security audit, and no deployment/backup/upgrade readiness is claimed.
