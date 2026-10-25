# Published Post Image Read V0

This is a Rust service boundary, not a shipped public image endpoint. Public
posts do not render featured images yet. `/api/media/:id` remains private and
requires the current administrator. No directory is exposed as static files.

## Grant

`read_for_published_post` accepts a post slug and the exact expected media ID,
using the configured database and object-store capability. One SQLite read
transaction checks the current published post, its current image association
and the media metadata before returning validated bytes. Arbitrary library IDs,
drafts, missing posts, missing selections and obsolete associations fail closed.
SQL inputs are bound parameters. No administrator identity is invented to reuse
the private read service.

The common byte validator enforces the same store, bounded size, PNG/JPEG MIME,
hash and image-dimension checks used by private reads. It is crate-private and
does not establish a grant by itself. Neither result type serializes bytes into
JSON. The configured storage adapter uses a read-only connection, query-only
mode, disabled trusted schema and a bounded busy timeout.

Responses retain `private, no-store`, `nosniff`, inline disposition and same-origin
resource policy. Public accessibility is not permission to cache: future HTTP
delivery must recheck the association for every request and must not return a
conditional 304 before authorization. Previously downloaded bytes and responses
already admitted under a read snapshot cannot be recalled after unpublishing.

Replacing or detaching an image, deleting/unpublishing a post or changing its
slug prevents subsequent reads through the old grant. Reusing the same image
on a different published post does not grant access through a draft slug.
Neither reads nor detaching delete media objects.

## Acceptance

`cargo test --locked --test post-images --test private-media-read` tests the
service with isolated databases and object stores, including exact-grant
denials before storage access, publication/association transitions, unchanged
library rows, exact response bytes/headers and corrupt/missing/wrong-store
objects. Existing private read tests cover the shared validation boundary.

## Next Boundary

Axonyx 0.7.1 only exposes session-verified `Native.privateResponse` for native
binary routes. A separate explicit public binary callback contract is required
before exposing this service through a public route. Do not weaken the existing
private callback contract, manufacture a session subject, use raw static storage
or embed base64 images through `Native.call` as a workaround.

That transport needs compiled/preview parity, bounded responses, safe errors and
no-store headers. Then Legura can register the product grant and add escaped
image/alt markup with a missing-image fallback. Actual anonymous HTTP and browser
proof (draft, unpublish, replacement, disabled storage, missing bytes, recovery,
JS/no-JS and desktop/mobile) remains necessary before claiming public delivery.

Framework tracking: https://github.com/vladanPro/axonyx-framework/issues/376.
