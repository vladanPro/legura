# .legura archive V0

Source-pilot archive creation using either a SQLite-owned live snapshot or a
caller-provided frozen SQLite/media snapshot. It is not a production installer,
CMS updater or installation-ready restore. Inactive recovery into a new directory
is available below. Existing database-only backup and its
refusal of media references remain unchanged. Full product schema/migration
compatibility checking is a separate explicit command; see
[Recovery Compatibility V0](recovery-compatibility-v0.md).

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
./target/release/legura-maintenance archive-backup data/legura.db local-media /private/media backup.legura
./target/release/legura-maintenance archive-pack-snapshot frozen.sqlite local-media /private/media backup.legura
./target/release/legura-maintenance archive-verify backup.legura
./target/release/legura-maintenance archive-restore backup.legura /private/new-recovery
./target/release/legura-maintenance archive-check-recovery backup.legura /private/new-recovery
```

Use `.exe` on Windows and the actual Cargo target path if shared. Parent paths
must exist and remain private/trusted. The output must be NEW; existing files or
symlinks are never replaced. Unix files are 0600; Windows inherits operator ACLs.
The commands use explicit arguments, not `.env` or process database configuration.
The two creation commands above are alternatives, not sequential steps writing
the same output file.

`archive-backup` opens the supplied database read-only and uses SQLite's online
backup API in 256-page steps, including committed WAL data. It captures a private
standalone snapshot before reading its media references. Uploads committed after snapshot completion
stay in the running database but are not mixed into the earlier archive.
The current append-only object store must remain immutable; concurrent deletion
or provider migration is not supported. Missing/changed objects fail the archive.

Snapshots use a random new OS-temporary workspace (Unix directory 0700, file 0600;
Windows inherits operator ACLs). Known snapshot/sidecar files are removed on normal
return or error without recursive deletion. Crashes or cleanup failures can leave
private residue; restrict the OS temporary directory too. No config is switched.
A 60-second soft deadline, bounded busy waits and 1 GiB database limit refuse
unbounded snapshot attempts; individual SQLite/OS calls cannot be hard-preempted.

**Never pass the running application's DB file to archive-pack-snapshot.** The
input must be a trusted standalone immutable snapshot, with the media schema and
without WAL/SHM/journal sidecars. Sidecar absence alone does not prove that a DB is
offline. Snapshot acquisition, quiet-period coordination and complete product
schema compatibility checks are not supplied by this command. Do not improvise
by copying a live `.db` file. Use `archive-backup` for SQLite-owned acquisition.

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
database references match its media inventory. The recovery command performs
the SQLite/media semantic checks described below. Treat bundles as
private, trusted operator material, not authenticated remote input.

## Inactive recovery boundary

`archive-restore` verifies the complete container before creating a NEW private
directory. It never merges into an existing directory, including an empty one,
or overwrites an existing file/symlink. The parent directory and source archive
must be trusted and remain operator-controlled throughout the operation. Unix
directories use 0700 and payload/receipt files 0600; Windows inherits parent ACLs.

The same open archive handle is scanned again during copying, with all entry
limits and checksums rechecked. Only the exact database and canonical media paths
are written with create-new semantics; TAR permissions, links and extraction
helpers are not used. Object keys and logical store identity are preserved.

Recovered layout:

```text
new-recovery/
  database.sqlite
  media/<store-id>/<original-object-key>
  recovery.json
```

Before returning success, the command checks SQLite integrity, foreign keys,
unique media IDs and every media row against its actual recovered bytes. Missing,
changed or additional/unreferenced archive objects fail recovery, even when an
altered archive has self-consistent checksums. The image audit checks metadata
bounds and stored hashes, not a fresh decode of every image.

`recovery.json` is written last with `status: recovered-not-activated` and
`cms_compatibility: not-checked`. It is a data recovery receipt, NOT permission to
start a CMS. Complete product schema/migration history and rich-text compatibility
remain separate gates; no migrations, executable code, `.env`, service settings
or session secrets are restored or switched. Do not start this database before
those compatibility checks. Any later activation requires independent session
secrets so old sessions cannot resume.

On ordinary failure, cleanup removes only files/directories created by this
attempt, nonrecursively. Crashes, permission changes or cleanup failures can leave
a partial private directory; never treat directory existence as success and do
not retry by overwriting it. The receipt/file syncs are not a full power-loss
durability guarantee. Protection against a malicious host owner changing the
parent directory, files or archive in place is outside this local operator tool.

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

Live-source tests keep committed data in WAL, add a second real upload after the
private snapshot is captured and inspect the archive's SQLite/image set. Only
the earlier upload is captured while the source retains both. Additional tests
cover private temporary files, deadline failure/cleanup, live CLI no-clobber and
missing-source refusal without creation. The HTTP/browser proof now creates and
verifies an archive while the disposable CMS remains running, then confirms its
protected library is still usable and row/object counts are unchanged.

Recovery tests cover byte/key preservation, no-clobber of files and directories,
empty media, source preservation, Unix permissions/symlinks and CLI output. Bundles
with recomputed valid checksums but missing/extra/changed objects, duplicate IDs,
invalid SQLite or foreign key violations are refused and ordinary partial output
is removed. The disposable HTTP proof also recovers the live archive, independently
audits recovered images and compares identity, credentials, posts, media and
migration rows without switching the running application.

The explicit read-only `archive-check-recovery` command now checks current product
schema/migrations, plain/rich content and recovered bytes against the original
archive. It does not modify this receipt or activate a service. See the strict
current-version policy in recovery-compatibility-v0.md; a separately approved
activation procedure remains next. Operator media remains inactive.
