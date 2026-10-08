# Setup And Identity Pilot

This is the first product slice, not an installable production CMS release.
SQLite only. No posts, editor, recovery emails, social login or plugin loading.

Current status: migrations/schema pull and `cargo ax check` pass. Compiled build
currently stops because `before Login.throttle(...)` is supported on API routes
but not actions. Upstream task: https://github.com/vladanPro/axonyx-framework/issues/319.
Do not remove throttling merely to pass build; the auth acceptance gate is open.

## Boundaries

- Owner configures the database, session key and independent setup token on the
  server. The token is entered as a password field; it is never embedded in HTML.
- Setup requires that token and an unclaimed installation. The user, credential
  and singleton installation row are committed in one transaction. A losing
  concurrent setup must roll back all of its writes.
- Passwords use the framework Password API; only hashes enter the database.
- Admin reads require a trusted session and a current database admin role.
- Mutations use framework same-origin/CSRF checks. Logout destroys the session.
- Native validation retains site name/email only. Passwords and setup tokens
  must be entered again. Unknown-account and wrong-password errors are identical.

## Local Development

After installing the validated CLI release:

```powershell
pwsh -File scripts/local.ps1 -Task init
pwsh -File scripts/local.ps1 -Task check
pwsh -File scripts/local.ps1 -Task build
pwsh -File scripts/local.ps1 -Task start
```

Open http://127.0.0.1:3940/setup. The local init script generates independent
random secrets into the ignored .env file, runs migrations and pulls schema.
Read the setup token from that local file; do not commit or share it.
Database and credentials remain in ignored data/ storage. Never point this
development pilot at a production database.

For upstream work only, pass `-ToolManifest ../axonyx-framework/Cargo.toml` to
use source CLI tooling. Legura dependencies remain registry packages.

Production needs HTTPS, secure session cookies, supervised processes, restricted
configuration permissions, backup/restore and a demonstrated upgrade path. The
local PowerShell helper is not a cross-platform installer.

## Acceptance Gate

Prove token rejection, invalid-input 422 retention, first administrator creation,
setup locking (including parallel requests), password hashing, private admin
reads, login/logout, CSRF rejection, persistence after restart and secret-free
responses before merging this pilot. Presence of the code is not proof.
