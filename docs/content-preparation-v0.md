# Content Preparation V0

This is a product validation contract and isolated native-registry proof. Active
CMS forms, database migrations and public rendering remain unchanged.

`prepare_content(ContentInput)` accepts an explicit format, plain body and an
optional document. It returns `ContentPreparation` with either validated storage
columns and no errors, or field errors and no columns. Invalid content must not
become an empty successful write. This function does not write to a database.

- Plain text remains literal, and a supplied document is rejected.
- Rich text must pass the real editor document validator. Its body projection
  is derived server-side, never trusted from the submitted body.
- Canonical document JSON is returned for storage; HTML is not returned.
- Blank/oversized text maps to `body`, unsupported formats to `content_format`,
  and invalid or inconsistent documents to `content_document`.
- Error messages do not echo submitted content or format names.

The native-registry request DTO denies unknown fields. Wrong JSON types and
transport budgets remain registry failures; editorial validation is recoverable
data. A future `.ax` action must convert those field errors to a native 422 and
stop before persistence. Merely receiving this DTO does not authorize a write.
Existing auth, administrator checks, CSRF and transaction handling stay required.

## Isolated Proof

```sh
cargo test --locked --lib
cargo run --locked --manifest-path tests/native-content/Cargo.toml
```

The second command uses a separate workspace pinned to the merged development
runtime revision containing `AxNativeRegistry`. The CMS itself retains its
published runtime dependency. The proof exercises typed registration/calls,
canonical rich content rehydration, forged projection replacement, sanitized
editorial errors, oversized text and rejected wrong types/extra fields. It has
no server listener or database connection and is not a compiled CMS HTTP test.

Next: action-level field-error recovery through the compiled app registration,
then migration/assets and typed public rendering. Do not hand-edit generated
backend Rust or allow arbitrary HTML to skip these gates.

The next proof now exists as [Content Action Validation V0](content-action-validation-v0.md):
generated `.ax` action dispatch and preview guards, not a live HTTP or DB test.
