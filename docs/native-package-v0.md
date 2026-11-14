# Native Package V0

Experimental native runtime package for a new or restored compatible SQLite
installation. Not a production release or an automated installer. Its generated
server uses Axum/Tokio and registry runtime/UI; source files are not needed at
runtime. The executable must match the destination OS/architecture and system
libraries. This pilot only packages the build machine's native platform.

## Contents

- legura-server (Windows: .exe): compiled application/backend.
- legura-maintenance (Windows: .exe): trusted backup/verify/restore tool.
- dist/: generated runtime assets, excluding Melt inspection reports.
- start.ps1: optional local loopback launcher, PowerShell 7.4+.
- package.json: product/platform and SHA-256 file inventory, not an npm package.

Database, .env, credentials, source, migrations and developer backups are NOT
included. The inventory detects accidental changes, not malicious replacements;
there is no package signing or automatic update mechanism in V0.

For forgotten administrator passwords, stop the server and verify a backup
first. Run `legura-maintenance reset-password data/legura.db --offline` (use
`legura-maintenance.exe` on Windows) with the new password piped through
standard input from a trusted password manager or hidden prompt, not as a
command-line argument. The reset preserves content and revokes the owner's
existing sessions. Start the server and sign in again.

## Verify A Fresh Package

Before creating `data/` or `.env`, run the maintenance binary from the extracted
package directory:

```sh
./legura-maintenance verify-package .
```

On Windows, use `./legura-maintenance.exe verify-package .`. This read-only
check rejects missing, added, changed or linked package files and validates the
SHA-256 inventory in `package.json`. It is deliberately strict: once private
configuration or content has been added, use the backup/content checks instead
of treating `verify-package` as a running-site health check. The manifest is
stored beside the files and is **not** a publisher signature; obtain the package
from a trusted source and verify a separately published release checksum when
one exists.

## Candidate Archive

After building and verifying a fresh native package directory, create a
platform-specific archive in an existing directory outside the package:

```sh
pwsh -File scripts/archive-package.ps1 -PackageDirectory ../legura-native-pilot -OutputDirectory ../artifacts
```

This creates `legura-<version>-<platform>.zip` on Windows or
`legura-<version>-<platform>.tar.gz` on Linux, with a matching `.sha256`
file, refusing to replace either. Extract into a new directory, verify the
archive against the separately obtained checksum, then run `verify-package`
there before adding data/ or .env. The checksum detects corruption but does
not authenticate the publisher. These artifacts are not signed, published or
supported as production installers yet. CI retains verified Windows x64 and
Linux x64 candidates for seven days as workflow artifacts, not public releases.
The Windows test uses the native binaries with a system-only PATH; the Linux
test runs in a non-root container without a build toolchain. Both start a new
site from the extracted archive, publish a post with an image, and verify
backup recovery and fresh login.

## Prepare Existing Installation

Back up and verify the compatible installation first. Use the maintenance tool
to restore its trusted backup to a NEW database file in a private data/ directory.
Configure a private .env in the package root with a SQLite URL pointing to that
restored database, SQLite dialect, a new independent session key, setup token
and the appropriate secure-cookie setting. Preserve the original installation.

For a local HTTP rehearsal, the source pilot's configuration uses
AX_SECRET_DB_URL=sqlite://data/legura.db and
AX_SECRET_SESSION_COOKIE_SECURE=false. Real external hosting needs HTTPS,
secure cookies and service supervision; the convenience launcher is local only.
Rotate session keys when recovering. Never copy a running database's .db file
alone: use the verified snapshot/restore API.

On Unix use owner-only permissions for .env, database directories and backups;
on Windows restrict ACLs. Process environment can override .env: clear conflicting
AX_SECRET_* values before starting. Keep secrets out of public assets and Git.

## Initialize A New Database

Create a private data/ directory first (Unix: `mkdir -m 700 data`; Windows:
create data/ and restrict its ACLs). From the package root:

```sh
./legura-maintenance init data/legura.db
```

Use .exe on Windows. This reserves only a NEW file and applies the migrations
embedded in the executable through Axonyx's transactional migration executor.
It refuses existing files (even empty ones), symlinks and orphaned SQLite
sidecars. Parent directories must exist and be trusted/private. Failed SQL
rolls back and removes only the newly created file. It never loads .env or
process DB configuration, creates an administrator, upgrades or resets content.

For local HTTP, generate a NEW private .env from the package root:

```sh
./legura-maintenance config-local .
```

This validates `data/legura.db`, creates independent OS-random 32-byte session
and setup secrets, and never prints them. Existing .env files (even empty ones
or symlinks) are refused, not updated. Unix files are created with mode 0600;
Windows storage ACLs remain the operator's responsibility. Read the setup token
privately from .env, never through logs or public assets. The generated values are:

```dotenv
AX_SECRET_DB_URL=sqlite://data/legura.db
AX_SECRET_DB_DIALECT=sqlite
AX_SECRET_SESSION_KEY=<independent random 32-byte hex value>
AX_SECRET_SETUP_TOKEN=<another independent random 32-byte hex value>
AX_SECRET_SESSION_COOKIE_SECURE=false
```

The placeholders above describe generated secrets, not literal values to copy.
The false cookie setting is for local HTTP only; this command is not a production
configuration assistant. Clear conflicting AX_SECRET_* process overrides before
starting. Start the server, open /setup and supply the owner token to
create the first administrator. Setup locks after success. Repeating init cannot
reset the installation; this is not a migration/upgrade command.

## Fresh HTTPS-Mode Configuration

For a new installation intended to run behind an operator-managed HTTPS proxy,
keep persistent data outside the replaceable program package. In a private
directory, create `legura.db` with `legura-maintenance init` and an empty
`media/` directory. Then, from a fresh extracted package with no `.env`:

```sh
./legura-maintenance init /private/legura-data/legura.db
./legura-maintenance config-https . /private/legura-data
```

Use `.exe` on Windows. `config-https` accepts only the current, empty product
database and an empty media directory. It writes a NEW `.env` with absolute
SQLite/media paths, independent random session/setup secrets and
`AX_SECRET_SESSION_COOKIE_SECURE=true`. It never replaces a configuration,
initializes data, changes an existing installation or prints secret values.
The media store ID is randomly generated for this fresh configuration. On Unix, the new
`.env` requests mode 0600; protect the data/media directories separately. On
Windows, restrict ACLs to the service identity.

This mode does **not** provide TLS, configure a reverse proxy, bind a public
listener or install a service. `start.ps1` still binds loopback; the operator
must deliberately configure HTTPS and supervision before external use. Never
switch a live installation to this command; use the checked recovery path
instead. Keep ambient `AX_SECRET_*` overrides clear at startup.

## Run Without Cargo

From the package directory:

```sh
pwsh -File start.ps1 -Port 3940
```

Open http://127.0.0.1:3940/setup for a new DB, or /login for an installed DB.
Stop with Ctrl+C. The optional launcher does not invoke Cargo, Git, Node or the
Axonyx CLI. PowerShell is not a runtime requirement of the native executable;
an operator can start it directly from the package root:

```sh
AXONYX_HOST=127.0.0.1 AXONYX_PORT=3940 ./legura-server
```

This direct command is for Unix shells. Working directory must be the package
root because dist/ and .env are resolved there. Readiness endpoint:
/__axonyx/ready; health endpoint: /__axonyx/health. Readiness is not proof that
all CMS migrations or operator security settings are correct.
The isolated native-package smoke also starts this binary directly with an
external SQLite/media directory and checks setup and Secure/HttpOnly cookies.
For an operator-managed Linux service example, see
[Native Service V0](native-service-v0.md).

## Maintenance

Use explicit paths, with parent directories already present:

```sh
./legura-maintenance backup data/legura.db backups/before-change
./legura-maintenance verify backups/before-change
./legura-maintenance restore backups/before-change data/recovered.db
```

Windows executables have .exe. Restores never overwrite an existing file or
change configuration. Stop all instances before switching DB configuration.
The compatible maintenance binary embeds migration checksums and rejects
unsupported migration histories. Automatic upgrades and schema conversion are
not supported. Replace program files/assets deliberately, never .env or data/.
If the database references uploaded images, the database-only `backup` refuses
it. Use the media-inclusive `archive-backup` with the configured store ID and
private media root instead (use your actual paths and store ID):

```sh
./legura-maintenance archive-backup /private/legura.db local-media /private/media /private/before-change.legura
./legura-maintenance archive-verify /private/before-change.legura
```

This archive contains private database content and referenced image bytes.
Keep it outside the package and protect it like credentials.

## Package Replacement Rehearsal V0

The isolated `-Package` smoke also tests a **same-version package replacement**.
It runs the original package against separate SQLite/media paths, uploads an
image, rejects a damaged candidate while the original remains online, and
verifies a clean candidate. Before stopping the original, it creates a
media-inclusive archive and has the candidate verify, recover and check that
archive's database, content and media compatibility. A valid package with a
broken media-root configuration must then fail startup; the original package
is restarted and its post, image, login and private configuration are checked.
After that, the correctly configured candidate starts
against the same external database/media and a byte-identical copy of the
private `.env`. The published post, private image bytes, media library,
administrator login, locked setup and configuration must survive. Only
disposable data is used; this does not prove an upgrade between two versions.

### Operator Rehearsal: Backup to Preflight

This procedure ends with an **inactive** recovery. It does not configure or
start the candidate, switch traffic, or run migrations. Use an isolated
installation for a rehearsal. The examples use absolute paths on one host;
substitute your actual paths and store ID, and use `.exe` on Windows. Stop at
the first nonzero exit code.

1. Obtain the candidate from a trusted source, verify the distributed archive
   checksum separately, and extract it into a **new** directory. Checksums and
   the package manifest verify integrity, not publisher identity. Before adding
   `.env`, check the extracted inventory using the candidate's maintenance
   binary:

   ```sh
   /opt/legura-candidate/legura-maintenance verify-package /opt/legura-candidate
   ```

2. Keep the database and media outside either package. For a final backup,
   block incoming writes, stop every server and worker that can write to those
   paths, and confirm the processes have exited. A single closed port is not
   enough. A backup taken earlier while writes were allowed is preliminary.
   Use the old installation's maintenance binary to create a **new** private
   archive while writers remain stopped:

   ```sh
   /opt/legura-old/legura-maintenance archive-backup /private/legura.db local-media /private/media /private/before-upgrade.legura
   ```

3. Restore to a **new** private directory, never over the source. Run preflight
   on that same archive and inactive directory, with the candidate still
   unconfigured:

   ```sh
   /opt/legura-candidate/legura-maintenance archive-restore /private/before-upgrade.legura /private/upgrade-recovery
   /opt/legura-candidate/legura-maintenance upgrade-preflight /opt/legura-candidate /private/before-upgrade.legura /private/upgrade-recovery
   ```

Preflight checks the candidate inventory, archived and restored bytes, schema,
migration history, content, and media. **Pass** means the candidate understands
the recovered data as-is, not that the service is ready. **Fail** means stop:
preserve the archive and source, diagnose the error, and do not activate the
candidate. A schema mismatch is not an instruction to migrate the live database.
If the source resumes accepting writes, this archive is no longer the final
snapshot for any later cutover.

### Schema Changes: Isolated Rehearsal

Use the verified candidate's maintenance binary with a trusted archive and a
new workspace under an operator-controlled private directory:

```sh
/opt/legura-candidate/legura-maintenance upgrade-rehearse /private/before-upgrade.legura /private/schema-rehearsal
```

The command accepts the exact known SQLite schema from the media-cleanup
migration onward, with matching migration history. It restores into
`restored-source/`, migrates only that copy, writes `rehearsed.legura`, and
verifies it by restoring again into `verified-recovery/`. Budget space for both
restores and the new archive. A zero exit code reports the applied migration
count and archive inventory; a nonzero exit means stop, even if some workspace
artifacts exist. Retry with a new directory. Keep all outputs private and
inactive. No configuration, service, or original archive is changed. Success
does not authorize activation or prove that the input is a final cutover backup.

From a source checkout, run the isolated SQLite-only migration rehearsal:

```sh
cargo test --locked --bin legura-maintenance migration_rehearsal_only_writes_to_restored_copies
```

The test uses its own temporary backup and two restores. One copy reaches the
current schema; on the other, a deliberately failing statement rolls back the
schema change and its migration ledger entry. The source database and backup
remain unchanged. There is no supported in-place operator schema upgrade yet.
After new writes, switching back to the old binary or restoring
the old archive in place is not a safe rollback.

Native Package Archive CI also rehearses the upgrade from the pinned pre-Pages
package (`f4b065a`) to the current Pages, language, Navigation, and public
interface, appearance and Trash schema. It requires exactly seven migrations on an
isolated recovery, byte-identical source data and image, fresh administrator
login, and persistent page, menu, language and appearance changes after
restarting the recovered candidate. Run the same check
with two trusted, unconfigured native package directories:

```sh
python -B scripts/smoke-schema-upgrade.py /path/to/prior /path/to/candidate
```

The test copies both packages before use and does not accept a live installation.

Both backup formats record their producer version. The `0.0.2` maintenance
reader explicitly accepts `0.0.1` and `0.0.2` backups; the `0.0.1` reader
still rejects `0.0.2` backups. CI builds the pinned `0.0.1` maintenance binary
and the current `0.0.2` binary separately, then verifies and restores both
database-only and media-inclusive `0.0.1` backups with the new reader. It also
checks that the old reader refuses new backups. This is backup compatibility,
not a schema migration. The separate cross-version native-package gate builds
both versions as independent packages and exercises a disposable server cutover
with external SQLite/media, a verified pre-cutover archive, public content,
fresh administrator login and exact private image bytes. The final archives in
that test are made only after the corresponding single test server has exited.
This does not prove that a real multi-instance deployment has no other writers.
The gate does not test rollback after the new version writes or provide an
operator upgrade command.
See post-cutover-recovery-v0.md for the tested data-loss boundary after the new
version accepts writes.

## Clean Linux Acceptance

CI also tests a clean Ubuntu 24.04 container with no network, source checkout
or developer toolchain. It checks the archive checksum, starts the native
binary as an unprivileged user, completes setup, publishes a post with an
image, and restores a media-inclusive backup into a separate installation.
Python drives the HTTP test only; the Legura binaries do not require it.
This loopback test does not cover a public TLS or VPS deployment.

## Build From Source

Build prerequisites are Git, Rust/native toolchain, CLI 0.8.8 and PowerShell
7.4+. Stop servers using the same Cargo target, or use an independent target.
From the initialized source checkout:

```sh
pwsh -File scripts/package.ps1 -OutputDirectory ../legura-native-pilot
```

Output directory must be new and its parent must exist. Partial output without
package.json is not a completed package. Production configuration assistance,
signed release artifacts, supported platform matrix, service installers and
upgrade acceptance remain subsequent gates. Do not advertise this as a finished
CMS installer.

## Acceptance

```sh
pwsh -File scripts/smoke-local.ps1 -Port 3943 -Package
```

The disposable fixture packages its own initialized installation, validates the
inventory/hashes, archives it, checks the archive checksum, extracts it into a
new directory and verifies the extracted inventory before restoring a trusted
snapshot and rotating the test session key. The extracted launcher runs from
another working directory with Cargo/Git/Node
absent from PATH. HTTP checks prove readiness, login, restored published/private
posts, private admin/data guards, setup lock, CSS and denied configuration/source
paths. No developer database, secrets or server are used. Windows passed locally;
Linux acceptance is a separate required CI gate, not assumed from that result.
The gate also initializes a second empty DB and private local configuration with
the packaged maintenance binary, refuses repeat init/config without changes,
creates an administrator through browser
setup, publishes a post, and verifies logout/login without Cargo in runtime PATH.
It then restarts that same installation, backs up its live database, verifies
the snapshot, restores into a NEW file, rotates the session key and checks the
recovered published post and administrator login. This is a same-host native
rehearsal, not a cross-platform prebuilt download or production upgrade test.
It also rehearses replacing an installed package with external SQLite/media:
the failed candidate leaves the original available, while the valid candidate
preserves a real uploaded image after archive compatibility checks and restart.
The separate `smoke-cross-version.ps1` CI gate proves `0.0.1` backup producer
compatibility with the `0.0.2` maintenance reader. The
`smoke-package-cutover.ps1` gate then starts a real `0.0.1` package, stops it
after verifying a backup, and starts a real `0.0.2` package over the same
external data and unchanged private configuration. Both use disposable CI
fixtures; this is not a supported production upgrade/rollback procedure.
