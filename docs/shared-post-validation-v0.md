# Shared Post Validation V0 - compiler gate

## Intended boundary

`app/domain/posts.ax` should own pure plain-text post rules and messages.
CreatePost and UpdatePost should explicitly import its `validatePost` function.
Actions keep authorization, missing-post handling, database uniqueness checks,
mutations and redirects. Database constraints still protect concurrent writes.
General pagination utilities belong in a shared module, not post validation.

The feature branch imports one `validatePost` from both actions. Core/runtime
0.6.8 and CLI 0.6.12 are published and the registry-only acceptance below passed.
Operator checkout/DB/server remain unchanged. Remote Legura CI must pass before
merging this feature into dev; this is not an operator activation or CMS release.
Rich documents retain their independent Rust preparation boundary.

## Reproduction (2026-10-12)

The original action reproduction lives in `tests/domain-validation/`, outside
app discovery; it now imports the canonical helper in `app/domain/posts.ax`.
The same candidate in app passed cargo-axonyx 0.6.11 `check`, but compiled Rust
with core/runtime 0.6.7 failed with:

- E0277: `.length` emits fallible `__ax_length(...)?` inside a pure function
  returning PostValidation, not Result/Option.
- E0308: the record return expression is emitted as serde_json::Value despite
  the function declaring PostValidation.
- E0507: String action inputs are passed by value out of a borrowed input,
  instead of generating the appropriate ownership conversion.

Function parameters are direct identifiers (`title`), not `input.title`.
The candidate uses bracket access for serialized action data. Dot access on
that data is another code-generation boundary, not proven by this fixture.
Multiline backend object literals and ternary expressions were not accepted
by the tested parser; they are deliberately absent from the reproduction.

The runtime release fixes these failures with internal fallible helper ABI,
checked record conversion and compiler-owned String argument cloning. The
compiled native consumer tests nested calls and valid/invalid action behavior
against preview. No generated Rust was edited manually.

## Existing behavior acceptance

`tests/posts-http.ps1` adds empty/oversized field checks for both mutations,
invalid status, preserved content after rejected updates, and accepted exact
maximum lengths. Existing tests cover authorization, uniqueness, persistence,
escaping and deletion. The isolated compiled Posts HTTP and backup/restore
proof passed with registry core/runtime 0.6.8, UI 0.0.86 and CLI 0.6.12 installed
with --locked in an isolated root. No ToolManifest/RuntimeSource/UiSource
overrides were passed. Both Cargo locks now use registry runtime/core, including
the native-content fixture. The editor retains its independent pinned Git package.
Native-content preparation, 9 library and 12 maintenance tests also passed.

Admin listing SQL/pagination and experimental rich-editor proofs are unchanged.
