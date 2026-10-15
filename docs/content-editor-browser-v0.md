# Isolated Content Editor Browser V0

This proof connects the real `RichTextEditor.asx` component and browser assets
from the pinned `legura-editor` Cargo dependency to an isolated compiled CMS.
It does not activate rich storage or replace the operator's current post editor.

`-ContentEditor` extends the disposable `-Content` setup/auth fixture. The Rust
installer reads embedded ASX, CSS and three JS modules from the pinned
editor package, without reading checkout internals or rewriting source. These
assets are installed as a content-addressed generation in the disposable
application. Fixture pages use the reported component/CSS paths, while their
application-owned lifecycle host imports the reported JS module. Test-only create/edit pages and
actions use native Rust preparation before parameterized SQLite writes. The
regular published-package CMS path and the operator's environment/data stay intact.
The editor pin now includes the joined Foundry toolbar and selection-aware link
popover. Published UI 0.0.86 provides the native icons; runtime 0.6.9 and CLI
0.6.12 provide compiled native registration. No source override is required.
Optional UiSource/RuntimeSource overrides remain confined to disposable fixtures.
EditorSource can override the package only inside that fixture. See
editor-package-assets-v0.md for the bundle inventory and
editor-managed-install-v0.md for installation and local-change refusal.

The small host module manages mount/destroy, disabled save state, replacement
roots and pagehide/pageshow lifecycle. Browser editing stays in the package.
The observer does not serialize DOM HTML or bypass server validation. This is
explicit test application integration, not a shipped automatic asset installer.

The edit loader revalidates stored document/body consistency before ASX emits
editor fields. See editor-storage-validation-v0.md for the read-only boundary
and corruption tests; normalization never rewrites the stored row.

```powershell
cargo install cargo-axonyx --version 0.6.12 --locked
pwsh -File scripts/smoke-auth.ps1 -ContentEditor -Mode javascript -Port 3946
pwsh -File scripts/smoke-auth.ps1 -ContentEditor -Mode native -Port 3946
```

Chromium acceptance exercises real setup, typing, selected bold, field-error 422
without document loss, create/save redirect, server-loaded document import,
update/save/reload, detached-root teardown/remount and safe stored HTML readback.
Two independently linked text ranges survive SQLite persistence, reload and
remount, alongside their formatting. The protected readback also verifies both
addresses in Rust-rendered safe HTML.
Desktop 1280px and mobile 390px checks assert no horizontal page overflow and
save screenshots under the OS temp `legura-content-editor-qa` directory.

No-JS coverage seeds through the real authenticated CSRF-protected action, then
opens the saved editor with JavaScript disabled. The original plain projection
is readable, the input stays read-only, document submission is disabled, and
the save button is disabled. This is read-only fallback, not no-JS rich editing.

Playwright is existing test-only tooling, not part of the CMS runtime; the local
Browser plugin/skill was not available. The server and content validation remain
Rust. This integration gate covers Chromium only. Physical IME, OS clipboard,
cross-engine CMS editing, native full-page 422 retries, unsaved-navigation UX,
autosave, concurrent editing, active migration and backup compatibility remain
separate gates. Package browser tests do not replace those product gates.
