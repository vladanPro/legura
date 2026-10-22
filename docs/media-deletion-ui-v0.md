# Media Deletion UI V0

Integration of the guarded deletion service. No operator installation or database
is changed by source tests.

Library items link to /admin/media/:id/delete. GET renders a private preview,
permanent-deletion warning, explicit checkbox and cancel link; GET never mutates.
The confirmation form posts to the stable /admin/media action route. This lets
no-JS validation re-render the library after a partially completed deletion,
instead of trying to load a now-removed /admin/media/:id/delete page. The library
has an action-bound feedback form with empty messages hidden, because Axonyx applies
native validation only inside a matching form. A future form-result destination
contract could remove that narrow workaround. The POST action obtains the
administrator from the verified session, checks media
availability and confirmation, then calls the service with server-owned identity.
Normal action CSRF enforcement remains required. Posted role/identity fields do
not grant authority. The service rechecks current DB authority and associations.

Expected in-use failure returns existing invalid(...) feedback (422), rendered
inside OperationFeedback. The field name `operation` is an application convention
for an operation-level message, not a new DSL or runtime error type. Confirmation
has its own field error. Native HTML retries work without JavaScript; no close
button or clear-on-edit behavior is claimed in this slice.

Success redirects (303) to the library. CleanupRequired returns operation-level
422 feedback, stating that the library record is gone but private file cleanup
requires administrator attention. It does not offer mutation replay. No request
input supplies the redirect URL. Durable cleanup receipts are future work.
Missing/stale media returns 404; rejected internal errors stay generic.

No migration, forced deletion, trash, recursive cleanup, or orphan sweeper is
introduced. Deletion remains subject to the crash/failure policy documented in
media-deletion-service-v0.md. Existing backup archives are independent snapshots.

Acceptance adds isolated HTTP and existing test-only Chromium checks for session,
CSRF, missing confirmation, referenced-image refusal, native feedback, cancellation,
successful removal, stale submissions, cleanup warning and restart. Browser modes
cover JS/no-JS at mobile width. These run after archive/recovery tests so destructive
fixture actions do not alter their expected snapshot. Compiled HTTP and Chromium
acceptance pass locally on Windows; required CI and deployment are separate gates.
