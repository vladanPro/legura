# Public Rich-text Rendering V0

The compiled `/posts/:slug` loader reads only published rows. It passes stored
format/body/document to `legura.content.render`, registered in `src/native.rs`.
The callback rehydrates `PostContent`, checks document limits/version/structure
and safe links, and verifies the stored body equals the rich plain projection.
It then renders fixed HTML through the editor's Rust renderer. Invalid storage
fails the request; it is never interpreted as arbitrary HTML or silently flattened.

`PublishedStory` exposes only title, body and server-generated HTML to ASX.
`PostStory` passes that HTML to the existing `<Html>` primitive. **Html is not a
sanitizer or a nominal trusted-HTML type.** Never wire request/form/DB strings
directly into `contentHtml`; this path is safe because its sole public loader
producer validates and renders through Rust. Ordinary title/text interpolation
still escapes values. Legacy plain text is escaped exactly once, not inferred
as HTML or converted into a document.

Paragraphs, H2/H3, bold/italic, flat ordered/unordered lists and validated
HTTP/HTTPS links retain their markup. Reading needs no editor JavaScript or
client mount. Draft/missing posts return 404. Corrupt documents, unsupported
links and projection mismatches return a failed server response rather than
partial/untrusted content. This is a scoped validation boundary, not a general
HTML sanitizer or complete security audit.

The private saved preview still uses escaped plain projection. Editor assets and
rich create/edit forms remain isolated proof fixtures; this change does not
activate the operator editor, migrate its database or publish the editor package.
Use compiled mode with CLI 0.6.12/runtime 0.6.8 and the pinned Git editor.

## Acceptance

```powershell
pwsh -File scripts/smoke-auth.ps1 -Content -Port 3952
pwsh -File scripts/smoke-auth.ps1 -PostEditor -Mode javascript -Port 3952
pwsh -File scripts/smoke-auth.ps1 -Posts -BackupRestore -Port 3952
```

HTTP fixtures verify anonymous published reading, private drafts, fixed heading/
list/link/mark markup, escaped malicious text, no double escaping and safe 500s
after deliberately corrupting only the disposable SQLite database. Original
records are restored before the auth lifecycle continues.

The Chromium editor proof creates a rich draft, verifies public 404, publishes
through the normal authenticated update action, then reads formatted output in
an anonymous JavaScript-disabled context. It asserts no editor script assets and
390/1280px overflow bounds, saves screenshots and checks editor page errors.
Existing test-only Playwright is used because the Browser plugin/skill is absent;
it is not a CMS runtime dependency. Linux CI is required before integration.

Windows acceptance on 2026-10-13 passed 16 library tests, strict library Clippy,
format checks, Content HTTP, PostEditor javascript/native and normal Posts HTTP
with backup/restore. All use published Axonyx packages with no source overrides.
Public mobile/desktop screenshots were saved and the mobile rendering inspected.
The operator checkout/database/configuration/server were not changed. No new
Cargo/editor release was made for this product-only slice.
