# Native Editor Retry V0

This acceptance extends the isolated regular post editor, using registry runtime
0.6.9 and the pinned Git editor package. It does not activate the operator editor
or claim that the editor itself has been published on Cargo/npm.

The form explicitly allowlists title, slug, status, body and content_document.
Identity and content-format hidden controls stay server-owned. The pinned editor
uses a disabled hidden textarea with data-ax-retain-disabled=true. New runtime
0.6.9 can replay escaped text into that control and mark data-ax-retained=true,
without enabling submission. JSON/debug form metadata still excludes inputs.

The editor validates marked replay before using it instead of stored content.
Valid recovery becomes the initial reset/history state. Invalid or over-limit
recovery remains disabled and preserved; it must not fall back to stale storage.
No arbitrary DOM HTML is serialized. No-JS rich editing remains read-only and
the document/save controls remain disabled. Every actual save still uses server
preparation, format checks, auth/CSRF and parameterized SQL.

## Proof

```powershell
pwsh -File scripts/smoke-auth.ps1 -PostEditor -NativeRetry -Mode javascript `
  -Port 3952
```

NativeRetry requires JavaScript PostEditor mode. With no source flags, it tests
the registry runtime and pinned Git editor. Optional candidate comparisons must
specify both RuntimeSource and EditorSource, never a silently mixed proof.
JavaScript is enabled to edit, but the test calls the browser's native form.submit
to bypass fetch interception and navigate to the actual 422 HTML response. It
proves title/slug/document recovery for new and edited posts, no insert/update
after rejection, and subsequent valid save/reload/public reading. The edit case
includes an unsaved paragraph and Unicode alongside existing marks/link.

The registry compiled content job runs NativeRetry without source flags. The
separate Native Editor Retry CI job pins both candidate commits and applies
Cargo patches only to disposable fixtures. EditorSource overrides the actual Cargo package before locating
its embedded asset bundle; no hand-edited generated backend is used.

Initial Windows acceptance passed native retry plus JavaScript-disabled fallback with
source candidates, runtime 270 unit tests, editor 15 Rust tests and Chromium/
Firefox/WebKit suites. A textarea reset regression found during development was
fixed by initializing its defaultValue from the validated initial document;
explicit replay marking avoids changing ordinary stored-data remount behavior.
Browser plugin/skill is absent; existing Playwright remains test-only.

Side-by-side asset installation and saved-only private preview have separate
accepted proofs. Still separate: automatic operator activation, dedicated
invalid-document recovery UX, native plain-mode query preservation and operator
activation. The 64 KiB encoded request budget and 32 KiB document budget remain;
oversize transport/conflict recovery is not covered by this field-level 422 proof.
The runtime patch is separate from editor licensing/publication. Operator
DB/.env/server/global CLI are not changed by this proof.

This proof builds and runs the app-specific compiled server. An installed CLI's
development preview uses the runtime compiled into that CLI; updating the app
lock alone does not replace it. Use `cargo ax build --clean --compiled` followed
by `cargo ax run start --compiled` for this pilot. CLI preview parity requires
rebuilding/updating the CLI against runtime 0.6.9 and separate acceptance.

Registry acceptance on Windows passed with CLI 0.6.12, crates.io runtime 0.6.9
and the locked Git editor package, without RuntimeSource/EditorSource flags.
The proof installed the package generation and exercised create/edit/reload,
actual native 422 navigation, unsaved document retention and corrected saves.
The 29 CMS library tests, nested native-content proof, formatting and strict
Clippy also passed. All three Linux jobs remain required before integration.
