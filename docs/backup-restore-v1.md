# Backup / Restore V1

Local operator tool for the current SQLite pilot. This is not a web endpoint,
cloud backup service or production installer. Use trusted local storage and
backups you created; a SHA-256 checksum detects corruption, not a malicious
backup whose manifest was also replaced.

## Build and Backup

For the local source pilot, [Operator Quickstart](operator-quickstart.md) provides
`scripts/maintenance.ps1` commands that discover the Cargo target directory.
The direct Rust tool commands below remain available for explicit operator paths.

From the Legura repository:

```sh
cargo build --locked --release --bin legura-maintenance
./target/release/legura-maintenance backup data/legura.db backups/before-delete
./target/release/legura-maintenance verify backups/before-delete
```

On Windows the executable has `.exe`. If Cargo uses a shared target directory,
use that directory instead of `./target` (see `cargo metadata --no-deps`). Paths
are literal filesystem paths, not SQLite URLs. Parent directories must exist.
The backup directory must be new: there is no overwrite option.

SQLite's online backup API captures a consistent snapshot, including committed
WAL records. Do not substitute a raw copy of a live `.db` file. Under sustained
writes or locks, the snapshot attempt can time out; retry in a quiet period.
V1 validates SQLite integrity, foreign keys, required product columns and the
exact two migration versions/checksums embedded in this maintenance build.

A completed bundle contains `database.sqlite` and `manifest.json` (format,
product/version, database SHA-256). The manifest is written last; an interrupted
or failed directory is not a verified backup. Never modify a bundle during
verification or restore. Do not use network filesystems or untrusted shared
directories. Symlink files and SQLite sidecars are rejected.

The database includes administrator records, password hashes, site settings,
public posts, private drafts and migration history. Backups are sensitive:
Unix directories/files use 0700/0600; Windows inherits directory ACLs, which the
operator must restrict. Encryption, off-site copies, retention and scheduling
are operator responsibilities, not implemented features.

## Restore Without Overwriting

```sh
./target/release/legura-maintenance restore backups/before-delete data/recovered.db
```

Restore verifies the trusted bundle and creates a NEW database file. It refuses
an existing file, symlink or orphaned destination SQLite sidecar. The existing
database is never overwritten. A restore is the whole snapshot, not a merge:
later edits and deletions are absent from the recovered copy. Failed operations
must not be treated as successful; preserve the original database and bundle.

To switch the application:

1. Stop every CMS process using the old database.
2. Restore to a new path; preserve the old database and its sidecars together.
3. Set `AX_SECRET_DB_URL=sqlite://data/recovered.db` in your private configuration.
4. Rotate `AX_SECRET_SESSION_KEY` to invalidate previous sessions. The backup
   does not contain `.env`, session/setup secrets, TLS settings or program files.
5. Start the same compatible application build. Verify public content, login,
   private drafts and the setup lock. Keep the original database for rollback.

For the source pilot, an explicit start command (from the repository root) is:

```sh
cargo ax run start --compiled --host 127.0.0.1 --port 3940
```

Unlike scripts/local.ps1, this does not require the fixed local data/legura.db
path. Check for conflicting process environment values before starting; they
can override .env. Do not rerun local init/prepare on the recovered configuration.

Do not switch configuration while the server is running or expose restore as
an admin HTTP action. No automatic server stop/start or atomic service switch
is provided. This version refuses older/newer migration histories rather than
attempting schema conversion. Media/uploads are not yet part of the product;
future file storage needs its own coordinated backup policy.

## Acceptance

```sh
cargo test --locked --bin legura-maintenance
pwsh ./scripts/smoke-auth.ps1 -Posts -BackupRestore -Port 3941
```

Unit tests cover committed WAL data, deletion recovery, identity/drafts,
no-clobber paths, corruption, incomplete bundles, foreign-key damage,
incompatible migration history/format and sidecars. Linux also checks private
permissions and symlinks. The disposable compiled HTTP fixture takes a live
backup, deletes a published post, restores to a new database, restarts on it,
checks public recovery, fresh login, private drafts and setup lock, then switches
back and proves the original database is still deleted. No real database is used.
