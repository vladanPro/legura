# Installation Model

The local source pilot below is implemented and tested. Prebuilt packages,
Docker packaging and production installation/upgrade remain planned. This is
not an installable production CMS release.

[Native Package V0](native-package-v0.md) is an experimental runtime-only
directory for an already initialized compatible SQLite installation. Fresh
database initialization without the CLI is not implemented in that package.

## Prebuilt Native Package

Download a package matching the server OS/architecture and supported system
libraries. Upload it using SFTP or another deployment method. Set execution
permissions, configure storage/database/secrets, and start it as a supervised
service behind the hosting provider's reverse proxy and TLS configuration.

Uploading files alone does not start the service. A Windows executable does not
run on a Linux host. Rust is not required to run a compatible prebuilt binary.

## Source Build

### Local SQLite Pilot

For day-to-day start/stop, backup and recovery, see
[Local Operator Quickstart](operator-quickstart.md).

Prerequisites: Git, Rust/Cargo with a working native linker/C toolchain, and
PowerShell 7.4 or later. These commands use the development branch, not a stable
Legura release. From a terminal:

```powershell
git clone --branch dev https://github.com/vladanPro/legura.git
Set-Location legura
cargo install cargo-axonyx --version 0.6.10 --locked --force
pwsh -File scripts/local.ps1 -Task prepare
pwsh -File scripts/local.ps1 -Task start
```

`prepare` creates data/ and an ignored .env only when absent, generates
independent random session/setup secrets, runs SQLite migrations/schema pull,
checks source and builds the compiled production-server binary. That build mode
does not make the development product production-ready. Cargo resolves the
registry runtime/UI; no Axonyx framework checkout or Node/npm install is needed
for running this pilot. Node/Python are test-tool requirements, not CMS startup
requirements.

`start` binds 127.0.0.1:3940. Open http://127.0.0.1:3940/setup, read
AX_SECRET_SETUP_TOKEN from your private .env in a local editor, then set site
name, administrator email and password. The token is never printed by the
helper or embedded in setup HTML. Do not share or commit that file. After
successful setup, the browser goes to administration and setup locks.
Stop with Ctrl+C in the start terminal. To use another unprivileged port:

```powershell
pwsh -File scripts/local.ps1 -Task start -Port 3945
```

Individual init/check/build tasks still exist. Repeated init preserves existing
configuration and checks pending migrations; it does not reset users/content
or rotate secrets. Prepare updates generated schema/types and rebuilds output.

The helper deliberately accepts only the exact sqlite://data/legura.db path,
SQLite dialect, independent 32-byte hexadecimal secrets and local HTTP cookies.
It refuses conflicting process overrides, duplicate keys and linked .env/data
paths before running database commands. Existing .env is not silently repaired
or replaced. Its supported config uses plain, unquoted KEY=value entries.
Unix newly created .env/data use 0600/0700; Windows inherits directory ACLs,
which the operator must restrict. Use a trusted private working directory.
This helper is not an arbitrary database migration command or a production
setup tool. Other databases/HTTPS need a separate operator procedure.

Compilation uses more resources than serving requests. The binary matches the
build machine's OS/architecture and needs generated dist assets/config/data;
copying just an executable is not yet a supported Legura installation bundle.

### Acceptance

```powershell
pwsh -File scripts/smoke-local.ps1 -Port 3943
```

The runner copies tracked source into a disposable directory, excludes local
secrets/data/generated build output and clears inherited backend configuration.
It invokes the actual prepare/init/check/start commands, verifies configuration
preservation and unsafe-config refusal, then completes native HTTP browser-setup
requests and verifies administration/setup locking. It stops only its own
process tree and deletes only its checked temporary fixture. Linux CI repeats
this gate alongside the existing registry auth/posts/browser acceptance.

Windows registry-only bootstrap passed on 2026-10-10 using CLI 0.6.10, including
case-sensitive database path refusal and unchanged configuration/database after
invalid attempts. Unix owner-only creation and linked-file refusal are checked
by the Linux CI branch, not claimed as locally verified on Windows.

## Docker

An optional alternative with the same application behavior and database/storage
contracts. It is not a prerequisite for running Legura.

## Hosting Boundary

Shared hosting is suitable only when its provider permits and supervises native
application processes and routes web traffic to them. Installing Rust does not
remove provider restrictions. Providers can build a release once and run isolated
instances with separate configuration and persistent data.

## Browser Setup

Choose a supported database, set site details, create the first administrator,
and complete installation. Setup must lock after completion and must not leak
database credentials or allow unauthenticated administrator replacement.

## Upgrades

Program files are replaceable; configuration, secrets, uploads, and databases
are persistent. Back up first, execute checked migrations, verify health, and
document recovery for both application and schema changes. Never overwrite user
data when uploading a new version.
