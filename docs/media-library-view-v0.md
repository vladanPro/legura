# Media library view V0

This is an isolated acceptance pilot, not operator media activation. The root
application still has no media route, storage hook, media migration or larger
request budget. SQLite backup/restore still does not include media files.

## View

The disposable `/admin/media-proof` route composes the existing Foundry
AdminFrame, Sidebar, Card, Grid and Button. It shows an empty state, total count,
processed MIME, dimensions, byte size and canonical object ID. The two-column
grid becomes one column on small screens. There are no fabricated thumbnails:
objects remain private and have no public image URL.

The reusable `MediaPlaceholder` component renders an inline SVG image outline,
semantic Foundry colors and an accessible label without JavaScript or external
requests. The empty library says "No image selected"; stored images without an
enabled preview say "Preview unavailable", not that their files are missing.

The optional AdminFrame `mediaHref` defaults to empty. Existing installations
receive no dead link. Only the fixture supplies it and selects the media item.

## Listing and upload

The loader verifies the current database-backed administrator before counting
or listing rows. Bound LIMIT/OFFSET queries return at most ten entries, ordered
by immutable ID. This is deterministic ID order, not upload-time order.
The existing integer pagination helper clamps oversized pages; a shared native
page-request normalizer now avoids reusing post-specific filter inputs.

Pagination uses ordinary links. A native multipart form saves an image and
redirects back to the library without requiring browser JavaScript. The JSON
proof action remains separate; both paths use the same image/storage service
and current administrator/CSRF boundary. Files are never submitted as JSON.
Invalid uploads remain native framework validation responses; this slice does
not claim polished file-field retry or an asynchronous uploader.

The count and list are separate queries, not a consistent snapshot. Concurrent
uploads can shift page boundaries between requests. There is no deletion,
selection, alt text, post attachment or media-inclusive backup in this slice.

## Acceptance

```powershell
pwsh -File scripts/smoke-auth.ps1 -Media -MediaBrowser -ToolPath <reviewed-cli> -Port 3963
```

Uses published storage-startup CLI 0.6.15 from crates.io. Migration 004 is now a
versioned product migration; the fixture alone enables its protected library
route, explicit private directory and 9 MiB request limit. It creates disposable
SQLite, uploads eleven real images, preserves the
intentional orphan from its metadata-failure test, and verifies:

- Anonymous, invalid-CSRF, cross-origin and revoked access remain denied.
- Native upload returns HTTP 303 to the library.
- Empty state, selected sidebar, ten/one item pages and stable disjoint IDs.
- Invalid/SQL-like/huge page inputs normalize or clamp safely.
- Listing remains private/no-store and survives server restart.
- Chromium checks desktop 1440px and mobile 390px with JavaScript on and off:
  links, selected sidebar, upload controls, no page overflow or console errors.
- Two additional images are selected and submitted through the actual browser
  file input, one per JavaScript mode, and persisted counts are checked afterward.

Playwright is the existing test-only browser dependency, not a product runtime
dependency. The isolated session cookie goes to the test process on stdin.
Optional `LEGURA_MEDIA_SCREENSHOTS` writes local screenshots outside the repo.

The startup hook is published, and versioned migration plus isolated media-aware
backup/recovery are available. Before product activation: explicit operator
opt-in, upload budget, delivery and upgrade policy remain. No active operator
configuration is changed here.

## Card Details

Cards keep the media type, dimensions and byte size visible. The internal Media
ID is not a user-facing filename or the SHA-256 integrity checksum. It is now in
a collapsed native `details` disclosure, labeled Technical details, rather than
always occupying the card. IDs remain in the data contract and pagination keys;
storage checksums, object bytes and schema are unchanged. Original upload names
are not persisted yet; no invented filename is presented as an original name.

The disclosure works without JavaScript. Its existing Foundry CodeBlock can copy
the ID when the shared UI runtime is initialized; no extra product JavaScript or
dependency is added. Without that enhancement the inactive Copy button is hidden
and the revealed text can still be selected/copied normally. Browser acceptance
checks the initial hidden ID, open/close behavior, actual clipboard contents and
mobile width in both JS/no-JS modes.
