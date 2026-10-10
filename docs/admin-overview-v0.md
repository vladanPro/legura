# Administration Content Overview V0

The administration landing page shows the real total, draft and published post
counts, with links to the existing filtered lists. A fresh installation shows
zero counts and a first-draft prompt; the prompt disappears after a post is
created and returns after the last post is deleted.

The page composes registry Foundry Grid and Card inside the shared AdminFrame.
Cards use three desktop columns and one mobile column. The existing account
and sign-out form remain. No new browser script, schema migration, analytics
tracking or cached client count is introduced.

`loadAdminOverview()` verifies the current authenticated administrator before
reading counts. One aggregate SQL statement produces all three counts and
normalizes empty SUM results to zero. The administrator identity lookup and
aggregation are separate reads, not a transactional snapshot guarantee.

The existing `loadAdministrator()` query remains unchanged for write actions.
Published tooling currently rejects calling one query from another query, so
the overview explicitly repeats the same identity/role checks. Query
composition is a framework follow-up, not a reason to relax the guard here.

Counts refresh on server navigation/reload, not through realtime subscriptions.
This is a SQLite product pilot, not verified PostgreSQL/MySQL integration.

## Acceptance

Use published cargo-axonyx 0.6.10 with the pinned registry runtime/UI:

```powershell
./scripts/smoke-auth.ps1 -Port 3942 -Posts -Mode javascript
./scripts/smoke-auth.ps1 -Port 3942 -Posts -Mode native
```

The disposable fixture tests zero counts, create, publish/unpublish and delete
transitions, then eleven posts with six drafts and five published. Overview
links must reach the correct real lists. Anonymous HTML and data-refresh
requests must be rejected. Screenshot/overflow/44px link checks cover desktop,
390px and 320px in both browser modes. The existing CRUD/auth/listing acceptance
remains in place. Linux registry CI is the integration gate; this is not a
production CMS release or a full accessibility/cross-browser audit.

On 2026-10-10, all five registry-only Windows acceptance flows passed: HTTP
auth/posts/backup recovery, browser auth with JS and without JS, and browser
posts with JS and without JS. Screenshots were inspected on desktop/mobile.
Browser plugin not available; repository Playwright was used. Developer/live
databases, global tooling and the development server on port 3000 were untouched.
