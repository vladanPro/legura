# Regular Post Editor Proof V0

PostEditor mode substitutes the actual /admin/posts/new and
/admin/posts/:id/edit routes in a disposable compiled CMS, reusing AdminFrame,
Foundry form primitives, the pinned editor, native preparation and migration003.
It is a source-tooling activation proof, not a change to the operator's forms.

New posts offer rich text or explicit plain text. Existing posts keep their
stored format. UpdatePost refuses implicit plain-to-rich or rich-to-plain
conversion before preparing/writing content. Rich body is derived on the server
from validated JSON, never trusted from a hidden field or editable browser HTML.
Title/slug/status validation, identity, CSRF and parameterized SQL remain in place.

Without JavaScript, plain-text authoring stays available. Rich editing is
read-only and its Save/document controls remain disabled to avoid flattening.
Browser tests check rich create/edit/reload, document preservation after a title
422, stored safe HTML, plain post preservation and refused format changes.
Desktop/mobile overflow and runtime errors are checked.

The disposable fixtures use canonical package assets and shared model code.
Only the browser test harness uses Node/Playwright (Browser plugin unavailable);
the CMS runtime remains Rust. The published CLI 0.6.10 lacks src/native.rs
registry startup wiring, so source tooling is required before activating these
handlers. Native icons also need a UI release before registry-only imports work.
Do not hand-edit generated Rust or silently replace the working registry path.

Run scripts/smoke-auth.ps1 with PostEditor, Mode javascript or native, an
explicit UiSource, ToolManifest and RuntimeSource. Use a separate test port.
Required Linux CI precedes merging; operator DB, .env and server3940 are untouched.
Public rich ASX rendering, explicit conversion UX, native full-page 422 recovery,
concurrent edits and broad CMS cross-engine coverage remain separate gates.
