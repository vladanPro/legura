# Regular Post Editor Proof V0

PostEditor mode substitutes the actual /admin/posts/new and
/admin/posts/:id/edit routes in a disposable compiled CMS, reusing AdminFrame,
Foundry form primitives, the pinned editor, native preparation and migration003.
It is an isolated activation proof, not a change to the operator's forms.

New posts offer rich text or explicit plain text. Existing posts keep their
stored format. UpdatePost refuses implicit plain-to-rich or rich-to-plain
conversion before preparing/writing content. Rich body is derived on the server
from validated JSON, never trusted from a hidden field or editable browser HTML.
Title/slug/status validation, identity, CSRF and parameterized SQL remain in place.
The editor loader revalidates stored content through Rust before returning its
document to ASX; see editor-storage-validation-v0.md. Corrupt rows fail before mount.

Without JavaScript, plain-text authoring stays available. Rich editing is
read-only and its Save/document controls remain disabled to avoid flattening.
Browser tests check rich create/edit/reload, document preservation after a title
422, stored safe HTML, plain post preservation and refused format changes.
Desktop/mobile overflow and runtime errors are checked.

The disposable fixtures use canonical package assets and shared model code.
Only the browser test harness uses Node/Playwright (Browser plugin unavailable);
the CMS runtime remains Rust. Published CLI 0.6.12, runtime 0.6.8 and UI 0.0.86
provide native registration, domain validation and the editor icons. CI now uses
these registry packages without framework/runtime/UI source checkouts. The
editor itself remains a pinned Git dependency, not a published Cargo package.
Do not hand-edit generated Rust or silently replace the working registry path.

```powershell
cargo install cargo-axonyx --version 0.6.12 --locked
pwsh -File scripts/smoke-auth.ps1 -PostEditor -Mode javascript -Port 3952
pwsh -File scripts/smoke-auth.ps1 -PostEditor -Mode native -Port 3952
```

Use a separate test port. Source override flags remain optional for framework
development; they are not used by the registry acceptance gate.
Required Linux CI precedes merging; operator DB, .env and server3940 are untouched.
Public rich ASX rendering, explicit conversion UX, native full-page 422 recovery,
concurrent edits and broad CMS cross-engine coverage remain separate gates.

Registry acceptance on Windows (2026-10-13) passed all five fixture modes:
Content HTTP, ContentEditor javascript/native and PostEditor javascript/native.
The CLI was installed in an isolated temporary root, leaving global tooling
unchanged. No ToolManifest, RuntimeSource or UiSource flag was supplied. Browser
tests exercised rich save/reload, validation recovery, safe stored rendering and
plain/rich conversion refusal; mobile screenshots were inspected. The original
operator checkout, database, environment and server were not changed. Linux CI
must pass before this registry gate merges into dev.
