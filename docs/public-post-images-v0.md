# Public Post Images V0

The public post story can show its selected featured image. The anonymous
GET /api/post-images/:slug/:id route uses Native.publicResponse and the existing
current-publication/association service. Arbitrary media IDs, drafts, obsolete
slugs and detached/replaced images do not grant access. The private /api/media
route still requires a verified current administrator.

Axonyx core/runtime/CLI 0.8.0 supplies this distinct public binary channel.
Application and nested-test lockfiles and CI pins adopt that registry release.
The UI/editor versions and database migrations do not change.

The presentation callback returns only image URL, alt text and selection state;
it never returns image bytes as JSON. It checks the existing image bytes before
emitting the URL. The actual byte request independently checks its current grant
again. This intentionally performs validation twice for a page plus image read;
there is no cached publication authorization. Already-admitted responses cannot
be recalled after a concurrent unpublish.

ASX escapes the alt attribute and URL. Slugs are percent-encoded as a single URL
path segment. A post without an image has no empty frame. A selected but missing
or corrupt image shows a textual placeholder while the article remains readable.
Disabled media emits no image UI and the byte endpoint returns 404. A failure
after HTML rendering can still fail the image request; no client onerror script
or image-cache/CDN behavior is introduced.

Responses retain private/no-store, nosniff and same-origin policy. MIME and
exact bytes use the existing decoder, digest, size and store validation. Storage
paths and SQL errors are not exposed. No session is invented for anonymous reads.

## Acceptance

Local acceptance passed with the published 0.8.0 CLI and registry runtime:

- `cargo test --locked --lib --tests` and strict Clippy (`-D warnings`).
- The nested native-content proof and formatting of all 41 handwritten sources.
- `scripts/smoke-auth.ps1 -Media -ScopedMedia -MediaBrowser`: anonymous exact
  bytes, draft/unknown/wrong-image denial, publish/unpublish/rename/replace/detach
  transitions, escaped alt text and corrupt-image fallback.
- Chromium decoded the public image at desktop/mobile widths with JavaScript
  enabled and disabled, without session cookies or horizontal overflow.
- Existing private-picker, authentication, archive/recovery and isolated
  recovery-activation checks passed after restoring the fixture association.

Remote CI remains a separate merge gate; local success does not imply CI success.

No operator checkout, database, .env or running CMS is changed by this work.
This is not a new Legura installer or production deployment.
