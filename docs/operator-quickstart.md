# Local Operator Quickstart

This is the SQLite development pilot, not a production installation guide.
Run commands from the Legura repository. PowerShell 7.4+, Rust/Cargo and
Axonyx CLI 0.6.10 are required. Git Bash can run the same `pwsh` commands.

## First Start

```sh
pwsh -File scripts/local.ps1 -Task prepare
pwsh -File scripts/local.ps1 -Task start
```

Open http://127.0.0.1:3940/setup. Read `AX_SECRET_SETUP_TOKEN` from the private
`.env` in an editor, then create the administrator. Use a unique password.
Once installed, use `/login` or `/admin`; setup stays locked.

Leave the start terminal open. Stop it with Ctrl+C. On subsequent starts, run
only `-Task start`; prepare is needed when source/build output changes.
If the executable is locked during build, stop your own server before preparing.
Do not delete data/ or .env to resolve a build error.

## Make And Verify A Backup

Use a private directory. On Windows restrict its ACLs to the operator account;
on Unix give the parent directory owner-only access. Never put backups in public/
or commit them. Each bundle name must be new.

Create the parent directory once:

```powershell
New-Item -ItemType Directory -Path backups
if (!$IsWindows) { [IO.File]::SetUnixFileMode((Join-Path $PWD "backups"), [IO.UnixFileMode]::UserRead -bor [IO.UnixFileMode]::UserWrite -bor [IO.UnixFileMode]::UserExecute) }
```

In Git Bash, the equivalent is `mkdir -m 700 backups` (on Windows, restrict ACLs).
Then, while the server can still be running:

```sh
pwsh -File scripts/maintenance.ps1 -Task backup -Bundle backups/before-change
pwsh -File scripts/maintenance.ps1 -Task verify -Bundle backups/before-change
```

The wrapper builds the Rust maintenance tool and locates it with Cargo metadata,
including a shared Cargo target directory. Relative paths resolve from the
repository root. Backup reads data/legura.db, not a URL from .env or process
environment. If using another database, use the underlying tool with explicit
paths and the full [backup/restore procedure](backup-restore-v1.md).

Success means SQLite integrity, expected migrations and bundle checksum passed.
The bundle contains content and password hashes, but not .env or session keys.
Checksum verification is not authentication or encryption. Keep an off-machine
copy and protect configuration separately; neither is automated here.

## Rehearse Recovery Without Switching

```sh
pwsh -File scripts/maintenance.ps1 -Task restore -Bundle backups/before-change -Destination data/recovered.db
```

This creates a new file and refuses existing destinations. It does not change
.env, stop the server, rotate secrets or switch the application to the copy.
The original installation continues running against data/legura.db.

For a real recovery, stop every server instance, preserve the old files,
verify the backup, restore to a new path, rotate the session key and deliberately
switch configuration following [Backup / Restore V1](backup-restore-v1.md).
The local helper intentionally only accepts data/legura.db, so it cannot start
the recovered path; use the explicit operator start command in that procedure.
Never move only a live database file and leave its WAL behind.

## Verify The Procedure

```sh
pwsh -File scripts/smoke-local.ps1 -Port 3943
pwsh -File scripts/smoke-auth.ps1 -Posts -BackupRestore -Port 3943
```

Run serially, with no other test using that port/target binary. The first gate
uses disposable data for preparation, setup, live backup and no-clobber restore.
The second proves actual server recovery and login against restored content.
Neither gate uses your development database. If a development server is using
the shared Cargo target binary, give the tests a separate CARGO_TARGET_DIR.
