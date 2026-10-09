# Post Deletion V1

## Contract

An administrator follows Delete post from the editor to
`/admin/posts/:id/delete`. GET displays the current title, slug and a permanent
deletion warning, with no write. Cancel deletion returns to the editor unchanged.

The form requires the current URL slug as explicit confirmation. Its POST
`DeletePost` action checks the current database administrator again, loads the
untrusted submitted ID and compares confirmation against the stored slug.
The final DELETE filters by both ID and confirmed slug. The runtime enforces
POST-only admission and CSRF; no mutation is delegated to client JavaScript.

Wrong confirmation returns a field-level 422 response. Missing or already
deleted IDs return 404. Anonymous and revoked identities return 403. A successful
deletion redirects to the admin list. Published list/detail and private editor
reads no longer expose the deleted record. Slugs may be reused by a new post.

Confirmation is deliberately not automatically retained after native validation.
There is no trash, undo, media cascade, audit log or revision recovery. This is
one-administrator SQLite behavior, not a multi-user authorization system.

The action is not an optimistic-concurrency transaction. A concurrent slug
change between confirmation read and DELETE may make that DELETE a no-op; it
must not delete a row whose stored slug no longer matches the confirmation.
Concurrent deletes may also finish as no-ops rather than always returning 404.

## Acceptance

Keep all existing registry-only auth/posts gates. Add HTTP proof for GET safety,
CSRF/auth rejection, wrong confirmation, mismatched IDs, revoked identities with
valid session/CSRF proof, published/draft deletion, repeat submission, unaffected
rows, restart persistence and slug reuse.

Chromium must prove cancellation, field errors, correction/resubmission, public
removal and missing editor pages with JavaScript enabled and disabled. Check
mobile overflow, console health and save confirmation screenshots.

Use isolated fixture databases and published CLI 0.6.9/runtime 0.6.6/UI 0.0.84.
No new Cargo/npm package, schema migration or production deployment is required.
The required Linux Registry Compiled Auth gate must pass before merging to dev.
