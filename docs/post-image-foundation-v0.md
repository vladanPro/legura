# Post Image Foundation V0

This is a product persistence boundary, not a shipped editor control or public
image endpoint. Existing media remain private and disabled unless configured.
The subsequent [private picker](post-image-picker-v0.md) consumes this service;
this document records the narrower foundation boundary.

## Editorial Model

Each post may have one optional featured image from the current media library.
`legura_post_images` stores the post ID, media ID and post-specific alternative
text. The same immutable library image can be reused by different posts with
different alt text. Empty alt is allowed for a decorative image; otherwise use
meaningful text. The limit is 300 Unicode scalar values; NUL is rejected.

Migration 005 creates the association table without rewriting post bodies or
existing media. Removing a selection does not delete an object. Deleting a post
removes its association, not the image. A referenced image cannot be deleted.
Deletion triggers preserve these semantics on existing SQLite connections that
do not enable foreign-key enforcement. The selection service explicitly enables
and verifies foreign-key checks on its own connection.

Rollback refuses while any selections remain, rather than silently discarding
editorial data. Detach explicitly before rolling back. Existing installations
must be backed up before any separately authorized upgrade; these tests never
migrate the operator database.

## Authority And Atomicity

`post_images::set_for_administrator` requires a startup-validated DB/storage
capability and the runtime-verified session subject, never a client identity.
It checks the current administrator and post in an immediate write transaction.
Selection/replacement additionally reuses the existing private read validator:
canonical library reference, configured store, bounds, MIME, dimensions and
exact stored bytes/hash. Missing or corrupt images do not replace a valid choice.
Detach requires the same current authority but does not read or require old
image bytes, so a broken selection can be removed.

SQL is parameterized; errors expose no SQL, paths or stored values. Alt text is
stored as literal text, not trusted HTML. A future ASX consumer must escape it.
Write transactions preserve the previous selection on validation/storage/SQL
failure. Independent object deletion or disk corruption after selection still
requires read-time validation; a saved association is not a permanent byte grant.

No HTTP/native registry mutation is registered in this phase. Future actions
must enforce verified sessions and CSRF before invoking the service. This is not
claiming network authentication merely because a Rust helper checks a subject.

## Acceptance

```sh
cargo test --locked --test post-images --test media-archive
```

The product's Linux/Windows media-storage CI job includes the association tests.
They cover selection, replacement, detach, reuse, authorization/revocation,
invalid inputs, Unicode bounds, failed-write rollback and deletion with SQLite
foreign keys both enabled and disabled. Archive acceptance proves selections
and alt text survive recovery, and the real migration executor refuses lossy
rollback. Required CI remains a merge gate; local compilation alone is not proof.

## Next Boundaries

- An accessible library picker and separate save/remove controls in post edit.
- Current-session/CSRF-protected actions calling the service, with validation UI.
- Public delivery only through a current published-post association; no raw
  public media directory and no draft-image URL that bypasses post visibility.
- Safe public/preview markup, missing-image fallback and JS/no-JS browser proof.

Inline rich-text images, galleries, thumbnails, public caching/CDN, media
deletion UI and external storage providers are not part of this foundation.
