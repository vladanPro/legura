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

Create a private .env in the package root with:

```dotenv
AX_SECRET_DB_URL=sqlite://data/legura.db
AX_SECRET_DB_DIALECT=sqlite
AX_SECRET_SESSION_KEY=<independent random 32-byte hex value>
AX_SECRET_SETUP_TOKEN=<another independent random 32-byte hex value>
AX_SECRET_SESSION_COOKIE_SECURE=false
```

Replace placeholders with independently generated cryptographic random values,
not words or example passwords. Configuration/secrets are still operator-managed;
no setup script generates them in this package yet. The false cookie setting is
for local HTTP only. Start the server, open /setup and supply the owner token to
create the first administrator. Setup locks after success. Repeating init cannot
reset the installation; this is not a migration/upgrade command.

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

## Build From Source

Build prerequisites are Git, Rust/native toolchain, CLI 0.6.10 and PowerShell
7.4+. Stop servers using the same Cargo target, or use an independent target.
From the initialized source checkout:

```sh
pwsh -File scripts/package.ps1 -OutputDirectory ../legura-native-pilot
```

Output directory must be new and its parent must exist. Partial output without
package.json is not a completed package. Automatic secret/configuration setup,
signed archives, supported platform matrix, service installers and upgrade
acceptance remain subsequent gates. Do not advertise this as a finished CMS
installer.

## Acceptance

```sh
pwsh -File scripts/smoke-local.ps1 -Port 3943 -Package
```

The disposable fixture packages its own initialized installation, validates the
inventory/hashes, restores a trusted snapshot and rotates the test session key.
The packaged launcher runs from another working directory with Cargo/Git/Node
absent from PATH. HTTP checks prove readiness, login, restored published/private
posts, private admin/data guards, setup lock, CSS and denied configuration/source
paths. No developer database, secrets or server are used. Windows passed locally;
Linux acceptance is a separate required CI gate, not assumed from that result.
The gate also initializes a second empty DB with the packaged maintenance binary,
refuses repeat init without changes, creates an administrator through browser
setup, publishes a post and verifies logout/login, without Cargo in runtime PATH.
