# Rich-text Storage Contract V0

This is a Rust product contract and isolated SQLite prototype, not an enabled
CMS editor integration. Current create/edit forms and public routes remain
plain text. No operator database or active migration was changed.

## Explicit Formats

- `plain-text`: existing `body` remains byte-for-byte unchanged; no document.
- `legura-richtext`: canonical validated JSON in `content_document`; `body` is
  its plain-text search/compatibility projection, never the editing source.

Never infer a format from a string starting with `{` or `<`. Existing posts keep
their format unless an author explicitly chooses conversion in a future UI.
No-JS rich editing must be read-only until a lossless path exists; it must never
submit a flattened body in place of the rich document.

`PostContent` has no unchecked public constructor. `plain` checks nonblank body
and the existing 20000-scalar budget. `rich` uses the real `legura-editor` Rust
validator and additionally checks the plain projection, including separators,
against that same budget. `storage_columns` emits canonical JSON, not HTML.
`from_stored` validates again on reads and rejects unknown formats, missing or
unexpected documents, and mismatched projections. Corrupt rich content fails
closed; it is not reinterpreted as plain text or returned as arbitrary HTML.
Legacy plain reads retain nonempty whitespace-only rows allowed by older SQLite
forms; the nonblank rule applies to new writes, not retroactive conversion.

`render_html` escapes plain text and delegates rich rendering to a validated
Document. It does not accept user-authored raw HTML. Caller-owned session, role,
CSRF, title/slug/status validation and database transactions remain mandatory.
This contract alone is neither an authorization service nor a storage service.

## SQLite Proof

`tests/fixtures/richtext-columns.sql` is intentionally outside `db/migrations`.
An in-memory test applies it over the real existing posts table, proves an old
post is unchanged, persists/reloads canonical rich JSON, and checks basic
format/document coupling. SQLite constraints do not replace Rust validation.

```sh
cargo test --locked --lib
```

The editor dependency is pinned to a full Git revision, not a local path or a
moving branch. The editor is still unpublished; no Cargo/npm release is implied.

## Remaining Integration Gate

The published Axonyx toolchain does not yet expose a proven way for `.ax`
actions/queries to invoke this native validator, or for `.asx` to consume its
validated rich output. Do not hand-edit generated backend Rust or interpolate
a submitted HTML string to bypass that boundary.

Next steps: implement a typed native-package bridge with preview/compiled parity;
then add the active migration and schema regeneration, guarded create/update
handlers, packaged editor assets/lifecycle, and shared saved/public rendering.
Test rejected JSON and forged formats, authorization/CSRF, native/JS retries,
backup/restore and old plain-text posts in disposable apps before activation.
Keep the current 64 KiB encoded request limit; JSON/form overhead is a separate
boundary from the 32 KiB document limit. Do not promise all limit-sized documents
fit every encoded form until that flow is measured.
