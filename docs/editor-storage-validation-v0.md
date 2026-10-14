# Editor Storage Validation V0

The isolated post and content-editor loaders authorize the administrator before
reading a row, then call `legura.content.load`. This Rust callback rehydrates
`PostContent` and returns validated body, explicit format and canonical document
JSON. Unsupported versions/structures/links and projection mismatches fail the
request before ASX emits editor controls or browser assets.

This is a read boundary, not submitted-form preparation: existing plain rows,
including legacy whitespace accepted by earlier forms, stay literal and retain
their format. Rich JSON normalization is applied only to the response; reading
does not rewrite the DB or repair corrupt records. Null/plain and nonnull/rich
storage coupling still applies. The callback has no database or request access.
It reuses the same validated model as public rendering and maintenance checks.

The package's browser validation remains defense in depth. This does not remove
write validation, CSRF/administrator checks, or DB constraints. Failed loads are
currently generic server errors; a dedicated operator recovery screen is not
implemented. Do not silently flatten or overwrite damaged content.

## Acceptance

```powershell
pwsh -File scripts/smoke-auth.ps1 -PostEditor -Mode javascript -Port 3952
pwsh -File scripts/smoke-auth.ps1 -PostEditor -Mode native -Port 3952
pwsh -File scripts/smoke-auth.ps1 -ContentEditor -Mode javascript -Port 3952
pwsh -File scripts/smoke-auth.ps1 -ContentEditor -Mode native -Port 3952
```

Browser fixtures use only the GUID-named disposable SQLite DB supplied by the
smoke harness. The corruption helper resolves paths, requires the OS temp root,
`legura-auth-<32 hex>/data/legura.db`, and restores each original row in `finally`.
It uses existing Python SQLite tooling, not an application corruption endpoint.

Tests load noncanonical valid JSON, verify canonical data-document in the HTML
without a DB write, and keep literal `</textarea><script>` text contained. They
inject invalid structure/version/link/projection, require authenticated 500 with
no editor root/scripts/private marker, and anonymous 403 before storage validation.
After restoring the row, loading succeeds again. Post-form tests also retain
legacy whitespace-only plain text without conversion. The no-JS hidden submission
field deliberately remains empty/disabled; the server document is data-document,
not that field. Editing remains an isolated proof, not operator activation.

Existing test-only Playwright is used because the Browser plugin/skill is absent.
No browser/runtime dependency or new Cargo release is introduced. Native full-page
422 recovery, private rich preview, asset installation and operator activation
remain separate gates. Required Linux CI precedes dev integration.

Windows registry acceptance on 2026-10-13 passed all four browser fixture modes,
19 library tests, strict library Clippy and formatting checks. Mobile no-JS
fallback screenshot was inspected. An initial test incorrectly read the empty
disabled submission field; it now asserts the server's data-document attribute
in both modes, without enabling no-JS submission. No operator DB/config/server,
global CLI or package publication was changed. Linux CI is still required.
