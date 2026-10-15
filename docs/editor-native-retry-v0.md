# Native Editor Retry V0

This source-only acceptance extends the isolated regular post editor. It does
not activate the operator editor or claim a new published runtime/editor package.

The form explicitly allowlists title, slug, status, body and content_document.
Identity and content-format hidden controls stay server-owned. New editor source
uses a disabled hidden textarea with data-ax-retain-disabled=true. New runtime
source can replay escaped text into that control and mark data-ax-retained=true,
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
  -Port 3952 -RuntimeSource /path/to/runtime/crates/axonyx-runtime `
  -EditorSource /path/to/legura-editor
```

NativeRetry requires both explicit source flags and JavaScript PostEditor mode.
JavaScript is enabled to edit, but the test calls the browser's native form.submit
to bypass fetch interception and navigate to the actual 422 HTML response. It
proves title/slug/document recovery for new and edited posts, no insert/update
after rejection, and subsequent valid save/reload/public reading. The edit case
includes an unsaved paragraph and Unicode alongside existing marks/link.

The separate Native Editor Retry CI job pins both candidate commits, applies
Cargo patches only to disposable fixtures, and keeps registry-only acceptance
jobs unchanged. EditorSource overrides the actual Cargo package before locating
its embedded asset bundle; no hand-edited generated backend is used.

Windows acceptance passed native retry plus JavaScript-disabled fallback with
source candidates, runtime 270 unit tests, editor 15 Rust tests and Chromium/
Firefox/WebKit suites. A textarea reset regression found during development was
fixed by initializing its defaultValue from the validated initial document;
explicit replay marking avoids changing ordinary stored-data remount behavior.
Browser plugin/skill is absent; existing Playwright remains test-only.

Still separate: automatic asset installation, private rich preview, dedicated
invalid-document recovery UX, native plain-mode query preservation and operator
activation. The 64 KiB encoded request budget and 32 KiB document budget remain;
oversize transport/conflict recovery is not covered by this field-level 422 proof.
No release, root dependency pin or operator DB/.env/server/global CLI was changed.
