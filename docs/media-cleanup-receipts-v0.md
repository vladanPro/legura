# Media Cleanup Receipts V0

This is an isolated source candidate, not an operator upgrade or production
release. The current media deletion service removes database metadata before
private bytes. Migration 006 now records the validated object key and configured
store in the same immediate SQLite transaction that removes metadata. A failed
commit never touches the file. A crash or filesystem error after commit leaves a
durable receipt; keys with pending receipts cannot be reused for new media rows.

Successful file removal (including an already-missing file) clears the receipt.
If clearing the receipt fails, the operation still reports cleanup required; a
later retry treats the already-missing file as success. The admin media page
shows bounded, paginated pending receipts for the active store and offers an
administrator/session/CSRF-protected POST retry. The Rust service rechecks the
current administrator and committed receipt; it refuses arbitrary IDs, a
different store and any object still present in the media library. No request
can supply a filesystem path. Automatic background retries and cross-store
operator tooling are not part of this slice.

Rollback of migration 006 refuses while any receipt remains. Database backups
include receipts, but an archive does not copy the orphaned bytes: after recovery
the same receipt can be cleared by a retry that observes a missing file. Do not
claim atomic SQLite/filesystem deletion or automatically enable media in an
existing installation. Existing operator data/configuration and running services
are unchanged.

Acceptance: Rust tests cover authorization, referenced-image refusal, SQL/commit
failure, no key reuse, restart and missing-file retry, and rollback refusal.
The disposable compiled HTTP/browser proof covers session, CSRF, admin display,
restart and successful native retry alongside the existing upload, post-image,
archive and recovery checks. Required CI remains a separate gate.
