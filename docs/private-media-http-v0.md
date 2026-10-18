# Private Media HTTP V0 - Source Pilot

`GET /api/media/:id` is a compiled `.ax` route returning `Binary` through
`Native.privateResponse("legura.media.read", { id: params.id })`. It does not
use a public directory, base64 JSON, browser-provided identity or a handwritten
generated server. Gallery images use this endpoint without client JavaScript.
The empty-library placeholder remains; post images/attachments are not enabled.

## Unpublished Dependencies

Runtime is pinned to `3e27ed4c44d54dd3ea6fbaf1e68016f78d32f1dd` from
vladanPro/axonyx-runtime. CI CLI is pinned to
`965b7fa06ef3f77f0aac75f1df7960bd2cf50a4a` from vladanPro/axonyx-framework:

```sh
cargo install cargo-axonyx --git https://github.com/vladanPro/axonyx-framework --rev 965b7fa06ef3f77f0aac75f1df7960bd2cf50a4a --locked --debug
```

These are temporary exact source pins, NOT a new published 0.6.x feature. Public
enum additions require coordinated core/runtime/CLI 0.7 publication and registry
consumer acceptance before returning to registry dependencies. The historical
required CI label "Registry Compiled Auth" is kept to preserve branch protection;
its steps explicitly install pinned source tooling during this pilot.

## Authority And Storage

The runtime verifies the actual session before calling the Rust capability.
Only the resulting trusted subject is used; the input accepts exactly an ID.
Every read opens the configured DB read-only and checks the current administrator
role before metadata or object access. Revoked users/roles are not cached grants.
Disabled storage denies access; invalid configuration fails startup. Existing
private paths/schema/store validation and default-off activation are unchanged.

The read service verifies the canonical logical key, metadata/store, bounded
size/dimensions, PNG/JPEG MIME, SHA-256, format and dimensions before returning
exact bytes. Errors are closed 403/404/500 responses, without configuration or
storage paths. Responses use private/no-store, nosniff and same-origin policy;
the binary channel never sets session cookies. API validation remains always.

The 8 MiB per-object bound is not an aggregate memory/DoS guarantee. Validation
does not decode all pixels on every read, sandbox host administrators, revoke
already downloaded images or abort reads already authorized in flight. Missing
or corrupt objects return a safe error; automatic broken-image repair UI,
conditional/range responses, CDN/public post delivery and thumbnails are not
part of this slice.

## Isolated Acceptance

```sh
pwsh -File scripts/smoke-auth.ps1 -Media -ScopedMedia -MediaBrowser -Port 3953
```

The proof uses only randomized disposable roots and current source, not the
operator installation. It covers real uploads, exact binary bytes/headers,
anonymous/forged/revoked authority, disabled storage, missing and
corrupt objects, server restarts, archive/recovery activation and independent
session keys. Browser acceptance verifies decoded images, navigation and uploads
on desktop/mobile with JavaScript enabled and disabled. Browser plugin is absent
in CI, so the existing test-only Playwright is used; it is not a CMS dependency.
Optional screenshots are written only to LEGURA_MEDIA_SCREENSHOTS when set.

Required CI and coordinated release remain separate gates; no operator database,
configuration, global CLI or running CMS is updated by these tests.

Local Windows evidence (2026-10-18): the complete scoped Media HTTP/browser/archive
and recovery-activation proof passes with these exact pins. Chromium desktop
1440px and mobile 390px pass with JS on/off, including decoded-image checks,
pagination, technical details, native uploads and no console errors/overflow.
The read callback's unit test also proves rejected forged input, current-user
revocation/reinstatement, destroyed-session denial and disabled-storage denial.
Rust library/media/archive/maintenance tests, the nested generated native-content
consumer, formatting and strict product Clippy pass. Remote required CI remains
authoritative; this is not a production security audit or registry release.
