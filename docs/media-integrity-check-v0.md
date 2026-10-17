# Media integrity check V0

Read-only operator boundary and prerequisite for media-inclusive backup. This
does not create, copy, restore, repair or activate media. It does not upgrade a
database, enable an HTTP endpoint or replace installation compatibility checks.

## Explicit local paths

```sh
cargo build --locked --release --bin legura-maintenance
./target/release/legura-maintenance media-check data/legura.db local-media /private/legura-media
```

On Windows use `.exe`; a shared Cargo target changes the executable path, not the
arguments. Supply an existing SQLite file, logical store ID and existing private
directory. No `.env` lookup, URL guessing, file creation or schema migration is
performed. The PowerShell maintenance wrapper has not gained a media command.

Success prints only JSON counts, for example `{"objects":3,"bytes":219}`. Missing
schema, invalid metadata, an unknown store, unreadable/missing files or a
size/SHA-256 mismatch exit nonzero without a successful inventory. Errors do not
include object keys, file contents or database row values.

## Contract

- One explicit store per check. Wrong-store rows are refused, not skipped.
- References and checksums must be canonical lowercase 64-character hex strings.
- Metadata must have supported processed MIME, nonzero bounded size and dimensions.
- Stream database rows in one SELECT, reading at most one referenced object at a
  time through the bounded ObjectStorage capability. The local adapter refuses
  final-component symlinks and nonregular files.
- Compare exact length and SHA-256 to metadata. No HTML, browser or JS is involved.
- Unreferenced objects are not counted, scanned, removed or repaired.

The audit checks byte integrity, not whether those bytes decode as an image. MIME
and dimensions are range/allowlist checks; they are not independently decoded.
Upload validation remains the image decoder/re-encoder boundary. SHA-256 is not
authentication against someone who can replace both the database and files.

A single SQLite query holds a read view while scanning. Immutable objects make
this useful with the current append-only service, but it is not a snapshot of the
whole CMS and may hold a read lock on rollback-journal databases. Use a quiet
period or a trusted SQLite snapshot for large inventories. Trusted OS owners can
still alter files. Future deletion/provider migration requires coordinated rules.

## Database-only backup safety

The existing V1 backup/verify/restore now refuses a database containing media
references. Empty media tables remain compatible. Backup checks before creating
a bundle and again on the actual SQLite snapshot, so an upload between preflight
and snapshot cannot produce an apparently complete database-only backup. Verify
also refuses media rows even if the database checksum was recomputed.

This is deliberate refusal, not media-inclusive backup support. The next slice
must snapshot SQLite, copy its referenced immutable objects with hashes checked
while copying, finalize a manifest last, and prove restore to NEW database/media
paths without overwriting an installation. Do not enable operator media first.

## Acceptance

```sh
cargo test --locked --lib
cargo test --locked --test media-check
cargo test --locked --bin legura-maintenance
```

Tests cover empty/missing schema, malformed metadata without relying on schema
CHECKs, wrong stores, missing/truncated/same-size corrupted bytes, no successful
partial inventory, a real validated image upload, unchanged DB/file bytes,
ignored orphan objects and missing DB refusal without creation. Linux additionally
tests a referenced symlink without changing its external target. Backup tests
cover initial/snapshot/verify/restore media refusal and preserve existing rows.

The isolated media HTTP/browser proof also invokes the actual maintenance command
against its live disposable SQLite and uploaded objects, comparing inventory with
the persisted upload count and canonical byte sizes. Required CI covers both
Linux and Windows; no developer database or running operator binary is used.
