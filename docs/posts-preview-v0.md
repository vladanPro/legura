# Private Saved Post Preview V0

The editor offers Preview saved post, opening a new tab with the last saved
post. It does not save editor inputs, publish a draft, create a revision or
send an action request. Save changes first to include them in the preview.
The original editor tab and its unsaved title/body remain intact.

`/admin/posts/:id/preview` reuses the existing guarded `loadEditablePost(id)`
binding. Authenticated identity and current administrator role are required
before reading the post. Missing/deleted posts return 404 for an authorized
administrator; anonymous HTML/data requests return 403. Responses use no-store.
The URL is not a public/shareable preview token.

Private and public post routes compose the same local PostStory component for
escaped title/body rendering. The private route adds the shared AdminFrame,
status badge and a saved-version notice. It is a content preview, not a promise
that the administration shell matches every future public theme pixel-for-pixel.
The public route still fetches published posts only; draft slugs remain 404.

The back-to-editor link returns the preview tab to the saved editor. The
noopener link prevents access to the original editor window. No new client
script, schema migration, backend mutation or public endpoint was introduced.

## Acceptance

Use published cargo-axonyx 0.6.10 with the pinned registry runtime/UI:

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

All five registry-only Windows acceptance flows passed on 2026-10-10 with
published CLI 0.6.10: posts and auth Chromium JS/no-JS, plus HTTP posts/auth and
backup recovery. Desktop/mobile preview screenshots were inspected. Tests
used disposable SQLite data, not developer/live databases or global tooling.
