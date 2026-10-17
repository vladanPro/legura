# Recovery Compatibility V0

This source-pilot gate checks a recovered SQLite/media directory against its
original `.legura` archive and this maintenance build. It does not activate an
installation, upgrade a database, change service configuration or restore secrets.

## Operator commands

```sh
legura-maintenance archive-verify backup.legura
legura-maintenance archive-restore backup.legura /private/new-recovery
legura-maintenance archive-check-recovery backup.legura /private/new-recovery
```

Use the actual built binary path and `.exe` on Windows. All paths are explicit;
process DB configuration and `.env` are not used. Keep the archive and recovered
directory private and operator-controlled. Do not accept unauthenticated remote
archives. Run recovery into a NEW directory, never into the running installation.

`archive-check-recovery` is read-only. It:

- Verifies the archive again and requires the original inactive recovery receipt.
- Refuses symlink directories, payload files and receipts; bounds receipt reads.
- Rehashes the recovered database and every archived image against that archive.
- Checks SQLite integrity/FKs and the complete known current migration prefix.
- Checks migration checksums and names, refusing unknown/future/older histories.
- Compares actual product tables/indexes/triggers/views and their DDL against a
  private in-memory schema built from this binary's embedded migrations.
- Revalidates every stored post through the existing plain/rich content contract.
- Audits exact media references, byte sizes, hashes and metadata bounds.

On success it prints the same count-only JSON inventory and exits zero. The
message still says the recovery is inactive. Failure exits nonzero, without
repairing rows, applying migrations, deleting recovery data or updating receipts.

The receipt records what `archive-restore` originally did. Its historical
`cms_compatibility: not-checked` remains unchanged; the explicit check is a fresh
point-in-time result, not a durable activation certificate. Any later modification
requires another check. No startup integration or automatic configuration switch
is included. A later activation procedure must use independent session secrets.

## Current product migration

`20261018000000_004_media` is now the canonical product schema for image metadata.
Tests use this same SQL instead of a second proof-only migration. New explicit
initialization/preparation applies it through the usual migration executor; no
operator database is migrated merely by building or testing this branch.

Its rollback refuses a nonempty media table transactionally, preserving all rows
and migration history. Empty-table rollback is supported and leaves migration 003.
No image files are deleted by rollback. An empty table does not enable upload,
public serving, thumbnails, post attachments or operator media navigation.

Existing database-only maintenance continues to accept known historical backups
(002/003) and current empty-media databases. It still refuses referenced media.
The stricter recovery check requires all four current migrations: it will not
silently upgrade old snapshots or approve the previous proof-only media history.

## Deliberately strict pilot policy

This is the current SQLite product contract, not a general semantic SQL schema
equivalence engine. Even equivalent manually rewritten DDL, custom indexes,
plugin tables or triggers are refused until an explicit extension/upgrade policy
exists. Only line-ending differences in product DDL are normalized. The runtime's
own migration ledger DDL is not compared; its known versions/checksums/names are
checked separately. Compatibility does not authenticate the source of an archive.

The optional runtime-owned `ax_sessions` table and expiry index appear only after
the session store starts. They have a separate narrow column/PK/index-shape check
for the pinned runtime; they are not a wildcard exemption for other runtime-like
tables or triggers. Session rows are private archived data, not restored login
authority: future activation must still rotate independent session secrets.

Image checks validate stored metadata and bytes, not a fresh image decode. This
gate does not audit password strength, transport security, host permissions,
external storage providers, capacity or production service configuration. It is
not a sandbox against the host owner swapping files during local validation.

## Acceptance

```sh
cargo test --locked --test media-archive
cargo test --locked --bin legura-maintenance
cargo clippy --locked --lib --tests -- -D warnings
```

Tests cover a real current-schema archive and upload, repeatable read-only CLI
checks, changed recovered bytes, migration checksum/name/history failures,
missing/future migrations, schema drift, extra objects, invalid rich documents
and protected media rollback. The disposable HTTP/browser smoke also checks its
recovered live archive with the same command before continuing source auth tests.
The subsequent [isolated activation proof](recovery-activation-proof-v0.md) now
tests key rotation, fresh login, recovered-only writes/restart and return to the
preserved source. It is not a product activation or service-management command.
