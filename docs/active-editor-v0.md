# Active Post Editor V0

The real CMS forms now consume the pinned Git editor package, rather than only
test replacements. This is source integration, not automatic operator activation,
a published editor crate, or a production CMS release.

## Authoring

- `/admin/posts/new` creates rich content; `/admin/posts/new/plain` creates plain
  text with or without JavaScript. A stable plain route preserves its mode during
  native validation navigation; the former query-only fixture could not do this.
- Both creation routes share NewPostForm and PostFields, and the same CreatePost
  action. Edit uses validated stored format/document; there is no silent conversion.
- Create/update share title/slug/status validation in `app/domain/posts.ax`.
  `legura.content.prepare` validates content in Rust and derives the rich plain-text
  projection before SQL writes. Submitted browser HTML is never storage input.
- Rich Save and its document stay disabled until the package mounts valid content.
  Without JavaScript rich content is read-only; existing plain posts stay editable.
- Failed validation retains allowlisted fields and marked documents within the
  existing transport budgets. Successful update returns to the same editor;
  Cancel returns to the listing. Saved preview remains separate and never saves.

## Assets And Build

`scripts/editor-assets.ps1 -Task install` checks that product imports, stylesheets
and host module explicitly match the pinned package generation, then installs or
verifies that generation using the Rust helper. `-Task check` is read-only.
Local prepare/build and native packaging call install; local check calls check.
Generation directories/receipts are build inputs, ignored in Git. App-owned
`public/content-editor/host.js` mounts/destroys package instances across navigation
and synchronizes the Save button. It does not implement editing or validation.

A package change needs deliberate source-binding activation, not a rewritten
import or a forced repair. Edited, missing or unexpected files in an installed
generation fail closed. Old generations remain intact. Do not delete them as a
routine build repair. The editor remains pinned to Git; no npm runtime is used.

Use CLI 0.6.13, registry runtime 0.6.9 and UI 0.0.86. Build/run compiled mode:
the preview server deliberately cannot execute this application's Rust callbacks.

## Acceptance

```powershell
pwsh -File scripts/smoke-auth.ps1 -ActiveEditor -NativeRetry -Mode javascript -Port 3948
pwsh -File scripts/smoke-auth.ps1 -ActiveEditor -Mode native -Port 3948
```

ActiveEditor forbids dependency source overrides and preserves real forms,
loaders, actions and native registration in a disposable SQLite application.
Only supplementary inspection routes are fixture code. The source-comparison
PostEditor proof remains separate. Existing auth/posts, maintenance, package,
content tests and all mandatory Linux CI jobs remain required. Browser tooling
is test-only Playwright because the Browser plugin/skill is unavailable.

The operator checkout, running server, `.env` and database are not changed by
these tests. Before applying to that installation: stop its CMS, create and verify
a private backup, rehearse recovery to a new database, audit content compatibility
and build/run against the copy first. Existing plain rows are not converted.
No automatic updater, concurrent-edit recovery or rich no-JS editing is promised.
