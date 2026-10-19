# Private Media HTTP V0

`GET /api/media/:id` is a compiled `.ax` route returning `Binary` through
`Native.privateResponse("legura.media.read", { id: params.id })`. It does not
use a public directory, base64 JSON, browser-provided identity or a handwritten
generated server. Gallery images use this endpoint without client JavaScript.
The empty-library placeholder remains. Saved posts can use the
[private image picker](post-image-picker-v0.md); public delivery and inline
rich-text images are not enabled.

## Registry Dependencies

The application uses registry runtime 0.7.1. The nested generated-consumer proof
pins registry core/runtime exactly to 0.7.1, and CI installs registry CLI 0.7.1:

```sh
cargo install cargo-axonyx --version '=0.7.1' --locked
```

This follows source-pinned PR 64 and the coordinated 0.7 compatibility boundary;
the new enum cases were deliberately not published as a compatible 0.6.x patch.
No framework/runtime source override is required. The editor's independent Git
pin is unchanged. Installing/updating the application runtime does not update
your global CLI: old CLI 0.6.x cannot compile these routes. Required CI remains
authoritative for adoption; package publication alone is not CMS acceptance.

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

Required CI and operator activation remain separate gates; no operator database,
configuration, global CLI or running CMS is updated by these tests.

Historical source evidence (2026-10-18, PR 64): the complete scoped Media HTTP/browser/archive
and recovery-activation proof passed with the preceding exact Git pins. Chromium desktop
1440px and mobile 390px passed with JS on/off, including decoded-image checks,
pagination, technical details, native uploads and no console errors/overflow.
The read callback's unit test also proves rejected forged input, current-user
revocation/reinstatement, destroyed-session denial and disabled-storage denial.
Rust library/media/archive/maintenance tests, the nested generated native-content
consumer, formatting and strict product Clippy passed. The registry-adoption PR
repeats those checks using published Axonyx packages; historical source results
are not a substitute. This is not a production security audit.

Registry acceptance (2026-10-19): core/runtime/CLI 0.7.0 were published and
the CLI was freshly installed from crates.io into an isolated tool directory.
The complete command above passed with that installed ToolPath and without
RuntimeSource or ToolManifest overrides. Exact image responses, private/security
headers, scoped uploads, anonymous/forged/revoked authority, disabled/missing/
corrupt storage, JS/no-JS desktop/mobile image decoding and archive/recovery
activation passed again. Root tests (107), the nested generated native-content
consumer, formatting and strict product Clippy also passed against registry
dependencies. Required remote CI remains a separate PR merge gate. No operator
database, configuration, running process or global CLI was modified.

## Axonyx 0.7.1 Adoption

The application and nested native-content lockfiles now resolve registry
core/runtime 0.7.1. All CI CLI installations, including source formatting, use
published cargo-axonyx 0.7.1 instead of a historical source revision. The editor
Git revision and UI version are unchanged. This patch adds compound request/auth
guard fixes and a Tokio dependency bound that preserves the declared Rust 1.82
floor; it does not introduce another browser/WASM ABI or product migration.

Adoption acceptance must repeat the existing isolated product tests with the
installed registry CLI. Publication of Axonyx itself is not a substitute for
Legura's required CI gates. No operator configuration, database, running process
or global toolchain is changed by this source update.
