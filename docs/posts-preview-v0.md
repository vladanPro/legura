# Private Saved Post Preview V0

The editor offers Preview saved post, opening a new tab with the last saved
post. It does not save editor inputs, publish a draft, create a revision or
send an action request. Save changes first to include them in the preview.
The original editor tab and its unsaved title/body remain intact.

`/admin/posts/:id/preview` uses the guarded `loadSavedPostPreview(id)`
binding. Authenticated identity and current administrator role are required
before reading the post. Missing/deleted posts return 404 for an authorized
administrator; anonymous HTML/data requests return 403. Responses use no-store.
The URL is not a public/shareable preview token.

Private and public post routes use `legura.content.render` to revalidate stored
format/document/plain projection and produce fixed Rust markup. They compose
the same PostStory component, passing that validated HTML to the existing Html
primitive; Html itself is not a sanitizer. Plain text remains escaped once.
Malformed documents, unsafe links and projection mismatches fail closed, with
authorization preceding content validation. The private route adds AdminFrame,
status badge and a saved-version notice. It is a content preview, not a promise
that the administration shell matches every future public theme pixel-for-pixel.
The public route still fetches published posts only; draft slugs remain 404.

The back-to-editor link returns the preview tab to the saved editor. The
noopener link prevents access to the original editor window. No new client
script, schema migration, backend mutation or public endpoint was introduced.

## Acceptance

Use published cargo-axonyx 0.6.12 with the pinned registry runtime/UI:

```powershell
./scripts/smoke-auth.ps1 -Port 3942 -Posts -Mode javascript
./scripts/smoke-auth.ps1 -Port 3942 -Posts -Mode native
```

The disposable SQLite fixture verifies real new-tab navigation for both browser
modes, saved versus unsaved values, draft/published status, noopener, back links,
anonymous HTML/data denial, no-store, missing/deleted 404s and escaped script text.
Existing public rendering, CRUD, overview, search and pagination checks remain.
Desktop and 390/320px screenshot/overflow/link-target checks run on private
previews. Browser plugin not available; repository Playwright is the fallback.
Linux registry CI remains the integration gate. This is not a production CMS
release or a full security/accessibility/cross-browser audit.

Rich-text preview also runs in PostEditor mode. The test creates a rich draft,
leaves title/document edits unsaved, opens the real noopener link and verifies
stored title/marks/link/status without mutation requests or public publication.
The original tab's title/document remain unchanged. Anonymous HTML/data denial,
authenticated JavaScript-disabled reading, no-store and 390/1280px layout are
checked. HTTP fixtures cover matching public/private heading/list/link markup,
malicious literal text and safe failures on corrupt stored documents.

This preview integration does not activate the rich editor in the operator CMS
or require a new package release. The actual rich forms remain isolated fixtures;
the registry native bridge and renderer are already available.

All five registry-only Windows acceptance flows passed on 2026-10-10 with
published CLI 0.6.10: posts and auth Chromium JS/no-JS, plus HTTP posts/auth and
backup recovery. Desktop/mobile preview screenshots were inspected. Tests
used disposable SQLite data, not developer/live databases or global tooling.

2026-10-14: registry Content HTTP and PostEditor javascript acceptance passed
for the rich-preview candidate, plus source NativeRetry with the same preview.
Normal Posts javascript/native acceptance also passed. Mobile rich draft preview screenshot
was inspected. The rich proof verifies no POST requests, unchanged stored HTML
and editor tab inputs, anonymous HTML/data403, authenticated no-JS rich reading,
no-store and a still-private public draft slug. Required CI gates integration.
No operator DB/config/server/global CLI change or package release was made.
