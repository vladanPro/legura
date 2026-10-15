# Editor Content Check V0

Before activating rich editing, check that stored posts satisfy the same Rust
content contract used by the editor loader and public renderer. This command is
a compatibility audit, not an activation command or a complete readiness report.

```powershell
cargo run --locked --bin legura-maintenance -- content-check /path/to/legura.db
```

The packaged native maintenance binary accepts the same arguments without Cargo:

```text
legura-maintenance content-check /path/to/legura.db
```

The explicit SQLite path must already exist. No .env parsing, credentials,
remote connections, migrations or writes are performed. The existing maintenance
read-only opener and integrity/migration checks run first. Missing rich-text
columns are a compatibility failure, not an empty or successful audit.

The JSON summary contains only `total`, `plain`, `rich` and `invalid` counts.
Invalid rows produce a nonzero exit code after the summary. Operational/schema
errors fail without a successful summary. No IDs, titles, bodies, slugs or JSON
documents are printed. There is no public HTTP endpoint for this operation.

Validation covers both drafts and published posts, using PostContent::from_stored:
legacy plain text stays literal, rich documents must be supported and valid,
and their body projection must match. Unknown formats, incorrect SQLite value
types, invalid UTF-8, missing documents and over-budget values count as invalid.
Values are byte-bounded in the SELECT before copying into Rust; they are never
silently truncated into a valid document. The body budget still counts Unicode
scalars, not bytes. The scan streams rows rather than collecting all posts.

The audit never repairs, converts, publishes or deletes content. Run on a verified
backup or with the CMS stopped for an operator decision: a successful check is
not a guarantee that a running application's data cannot change later. SQLite
may use normal read locks/WAL sidecars; read-only does not mean lock-free.

## Acceptance And Remaining Gates

Rust unit tests cover plain/rich counts, legacy literal content, malformed JSON,
projection mismatch, missing/oversized/wrong-type values, invalid UTF-8, Unicode
limits, empty tables and incompatible schemas. CLI integration creates a disposable
migrated database, checks success/failure status and count-only output, compares
database bytes before/after rejection, and verifies a missing DB is not created.
Backup/restore validation shares the bounded content scan while preserving its
existing fail-closed behavior. Local Windows acceptance passed: 29 library tests,
12 maintenance tests, the CLI integration test, strict Clippy and formatting.
Linux CI remains required before integration.

A green audit alone does not enable the editor. Installed assets, compatible
generated routes/actions and native helpers, backup/recovery procedure and the
registry native full-page 422 acceptance remain separate activation requirements.
The editor is still Git-pinned and unpublished; licensing remains unresolved.
