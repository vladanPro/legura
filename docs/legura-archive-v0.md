# .legura archive V0

Experimental container for a **caller-provided frozen SQLite/media snapshot**.
It is not a production installer, live-database backup command, CMS updater or
restore implementation. Existing database-only backup and its refusal of media
references remain unchanged.

## Format decision

Use a standard uncompressed USTAR/TAR file with a recognizable `.legura` suffix.
Do not invent a binary archive format. V0 deliberately has no compression,
encryption, passwords, signatures, embedded executable code or auto-installation.

Entry order:

```text
database.sqlite
media/<logical-store-id>/<canonical-object-key>
...referenced objects only...
manifest.json
```

The manifest has format `1`, product `legura`, application version, one logical
store and a unique path inventory with exact byte length and SHA-256 per payload.
It is appended last, followed by the TAR terminator. Physical storage paths,
client filenames, environment secrets and orphan files are not included. The
database itself remains sensitive: it can contain drafts and password hashes.

## Source-pilot commands

```sh
cargo build --locked --release --bin legura-maintenance
./target/release/legura-maintenance archive-pack-snapshot frozen.sqlite local-media /private/media backup.legura
./target/release/legura-maintenance archive-verify backup.legura
```

Use `.exe` on Windows and the actual Cargo target path if shared. Parent paths
must exist and remain private/trusted. The output must be NEW; existing files or
symlinks are never replaced. Unix files are 0600; Windows inherits operator ACLs.
The commands use explicit arguments, not `.env` or process database configuration.

**Never pass the running application's DB file to archive-pack-snapshot.** The
input must be a trusted standalone immutable snapshot, with the media schema and
without WAL/SHM/journal sidecars. Sidecar absence alone does not prove that a DB is
offline. Snapshot acquisition, quiet-period coordination and complete product
schema compatibility checks are not supplied by this command. Do not improvise
by copying a live `.db` file. The next slice owns SQLite backup-API integration.

Packing streams the database and then visits its referenced immutable objects
through the existing media audit and bounded storage adapter. It hashes the exact
bytes being written; failed reads/hash checks do not produce a successful
package. It finishes and syncs the archive, then re-verifies the entire container.
Ordinary failure removes only its newly created output when possible. A crash
or cleanup failure can leave an incomplete file; the extension is not proof of
success. No automatic retry, repair or orphan cleanup is attempted.

## Safe reading boundary

Verification is read-only and **does not extract any files**. It accepts only
regular USTAR payloads with exact allowed paths. It rejects path traversal,
absolute/backslash paths, links, directories/devices/FIFOs, extension headers,
duplicates, hidden concatenated entries, missing/changed inventories, mismatched
hashes, unknown product/format/version and entries after the final manifest.
The current pilot accepts only its own application version; future compatibility
policy is a separate release gate.

Provisional bounds: SQLite up to 1 GiB, each image up to 8 MiB, 10,000 images,
manifest up to 4 MiB and the physical archive up to 8 GiB. Database/object hashing
is streamed; objects are bounded in memory one at a time. There are no compressed
payloads to expand. The verifier inspects all TAR headers rather than stopping
at an earlier terminator. Count-only JSON reports payload files/bytes, excluding
the manifest and TAR overhead.

SHA-256 detects corruption, not an attacker replacing both payloads and manifest.
Container verification does not execute or certify SQLite contents, check the
CMS's complete schema/migration history or establish that a hostile archive's
database references match its media inventory. Those semantic checks belong to
trusted snapshot creation and the future restore validator. Treat bundles as
private, trusted operator material, not authenticated remote input.

## Acceptance and next gate

```sh
cargo test --locked --test media-archive
cargo test --locked --lib --bin legura-maintenance --test media-check
cargo run --locked --manifest-path tests/native-content/Cargo.toml
```

Tests inspect a real uploaded image and database in the TAR without changing
sources; check empty media, ignored orphans, no-clobber and 0600 on Unix; refuse
missing/corrupted objects and sidecars; and exercise corrupt/truncated archives,
dangerous raw headers, oversize declarations, duplicate manifests/inventories,
altered store/format/version and both operator CLI commands. Both Cargo lockfiles
carry the archive dependency; no npm/runtime browser dependency is introduced.

Next: create a consistent live SQLite snapshot, validate the full product schema,
package its referenced objects, then implement recovery into NEW database/media
paths with key preservation, semantic validation and no-clobber acceptance.
Only after recovery is proven should operator media activation be considered.
