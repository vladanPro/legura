# Isolated Content Editor Browser V0

This proof connects the real `RichTextEditor.asx` component and browser assets
from the pinned `legura-editor` Cargo dependency to an isolated compiled CMS.
It does not activate rich storage or replace the operator's current post editor.

`-ContentEditor` extends the disposable `-Content` setup/auth fixture. Cargo
metadata locates the one pinned editor package; its ASX, CSS and three JS modules
are staged without rewriting their source. Test-only create/edit pages and
actions use native Rust preparation before parameterized SQLite writes. The
regular published-package CMS path and the operator's environment/data stay intact.

The small host module manages mount/destroy, disabled save state, replacement
roots and pagehide/pageshow lifecycle. Browser editing stays in the package.
The observer does not serialize DOM HTML or bypass server validation. This is
explicit test application integration, not a shipped automatic asset installer.

```powershell
pwsh -File scripts/smoke-auth.ps1 -ContentEditor -Mode javascript -Port 3946 `
  -ToolManifest /path/to/framework/Cargo.toml `
  -RuntimeSource /path/to/runtime/crates/axonyx-runtime
pwsh -File scripts/smoke-auth.ps1 -ContentEditor -Mode native -Port 3946 `
  -ToolManifest /path/to/framework/Cargo.toml `
  -RuntimeSource /path/to/runtime/crates/axonyx-runtime
```

Chromium acceptance exercises real setup, typing, selected bold, field-error 422
without document loss, create/save redirect, server-loaded document import,
update/save/reload, detached-root teardown/remount and safe stored HTML readback.
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
