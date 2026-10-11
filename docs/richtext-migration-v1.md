# Rich-text Migration V1

Migration 003 adds explicit content_format and nullable content_document to
legura_posts. Existing rows keep every body byte and default to plain-text with
no document. Old insert/update forms still operate on plain-text posts.
This schema change does not activate the rich editor or convert any post.

Use a verified backup before any operator upgrade. The standard Axonyx migration
executor applies the SQL transactionally and records its checksum. Rerunning
migrate is a no-op; do not execute these SQL files manually without a transaction.
No operator database has been migrated by the implementation tests.

The down migration refuses to run while any rich post/document exists.
It does not flatten JSON to text or discard formatting. If rich posts exist,
keep the upgraded schema or restore an explicitly chosen pre-upgrade snapshot
to a new database. Such restoration intentionally loses edits made after that
snapshot; it is not a merge or an automatic rollback.

The maintenance tool accepts exactly the known two-migration legacy prefix or
the three-migration upgraded history, validating every recorded checksum.
Backup/restore of a legacy snapshot leaves its schema unchanged; restore never
implicitly upgrades it. Upgraded snapshots also revalidate every stored content
document/projection through PostContent before being accepted.
Hash, no-clobber, foreign-key and SQLite integrity protections stay in place.

Verified locally with disposable databases:
- An old literal body survives migration byte-for-byte.
- Runtime migration replay is a no-op.
- Canonical rich JSON with two links survives backup and restore.
- Rollback refuses rich content and leaves no partial guard table/history.
- Plain-only rollback succeeds without changing existing body text.
- Pre-upgrade backups remain readable/restorable without implicit conversion.
- Malformed rich JSON is refused before a verified manifest is written.

Run cargo test --locked --bin legura-maintenance --lib.
The registry compiled Posts/BackupRestore smoke and source ContentEditor smoke
also exercise the actual migration rather than a separate prototype SQL file.
CI remains required before merging. Concurrent operator upgrade, PostgreSQL,
automatic repair, conversion UI and active rich post forms are outside this gate.
