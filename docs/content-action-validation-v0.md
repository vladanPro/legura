# Content Action Validation V0

The isolated native-content workspace now compiles a real `.ax` action at build
time and executes its generated Rust with a configured native registry. It also
checks the equivalent library preview path. Active CMS actions are unchanged.

The fixture uses:

```ax
data prepared = Native.call("legura.content.prepare", {
  content_format: input.content_format,
  body: input.body,
  content_document: input.content_document
})
require prepared["columns"] != null else invalid(prepared["errors"])
```

Native results currently use JSON indexing. Dot access to an untyped native
result is a compiler gap, not the recommended final typed surface. Native form
input uses `content_document: String = ""`; the product `ContentFormInput`
adapter converts exactly the empty string to no document. Nonempty whitespace
and malformed JSON are not silently discarded.

## Proven Boundaries

- A fixture-only bearer guard runs before native validation; rejected requests
  return the action 403 contract and do not invoke the validator.
- Invalid documents, blank bodies and unsupported formats return the 422 action
  contract with field errors and no submitted-content leakage.
- An instrumented follow-up callback is never invoked on validation failure.
- Valid plain and rich form inputs reach the follow-up callback. Rich body is
  derived rather than copied from the submitted projection, and its canonical
  document passes rehydration.
- Rejected and valid plain action behavior passes in both generated Rust and
  library preview.

The bearer literal is only a test marker, not a CMS authentication design. The
follow-up callback counts invocations and echoes validated columns: it performs
no database write. These tests prove ordering, not SQL transactions, live HTTP
status handling, CSRF, native-form retry or production authorization. Existing
CMS session/administrator/CSRF policies remain required for actual integration.

```sh
cargo test --locked --lib
cargo run --locked --manifest-path tests/native-content/Cargo.toml
```

The build script writes only to Cargo OUT_DIR; generated application files are
not edited. The separate proof workspace uses pinned development compiler/runtime
code while the CMS retains published dependencies. No active migration, user
database, editor asset or running server is changed.

Next: isolated compiled HTTP CMS acceptance with real authorization/CSRF and
disposable persistence, before enabling the format migration and editor UI.
