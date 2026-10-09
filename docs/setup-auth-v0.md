# Setup And Identity Pilot

This is the first product slice, not an installable production CMS release.
SQLite only. No posts, editor, recovery emails, social login or plugin loading.

Current status: migrations/schema pull and `cargo ax check` pass. The required
typed action query, backend expression and action-conflict fixes (#323/#324)
are published in core 0.6.4, runtime 0.6.3 and CLI 0.6.7 after green upstream CI
and dev-to-main release PRs. Registry-only compiled HTTP acceptance passed on
2026-10-09 with the published CLI and registry runtime, without ToolManifest.
Do not weaken optional credential verification or remove guards to pass build.

## Boundaries

- Owner configures the database, session key and independent setup token on the
  server. The token is entered as a password field; it is never embedded in HTML.
- Setup requires that token and an unclaimed installation. The user, credential
  and singleton installation row are committed in one transaction. A losing
  concurrent setup must roll back all of its writes.
- Passwords use the framework Password API; only hashes enter the database.
- Admin reads require a trusted session and a current database admin role.
- Anonymous admin reads and claimed setup reads return 403. Query guard redirects
  are not supported in this slice; framework issue #326 tracks that UX boundary.
- `env.SETUP_TOKEN` is the logical secret key; the process environment stores it
  as `AX_SECRET_SETUP_TOKEN`. The prefix is not repeated in the DSL lookup.
- Mutations use framework same-origin/CSRF checks. Logout destroys the session.
- Native validation retains site name/email only. Passwords and setup tokens
  must be entered again. Unknown-account and wrong-password errors are identical.

## Local Development

Install the tested CLI, then initialize the local development pilot:

```powershell
cargo install cargo-axonyx --version 0.6.8 --locked
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

The executable acceptance runner is `pwsh -File scripts/smoke-auth.ps1`.
It copies tracked source into a disposable directory, ignores the developer's
`.env`/database, uses its own SQLite file and random secrets, and cleans up its
own process and files. Source tooling can be selected with `-ToolManifest`.
Verified on 2026-10-09 with source and published registry tooling:
migration/check/compiled build, token
rejection, invalid-input retention without credential replay, exactly one owner
from parallel setup, transaction rollback, Argon2id persistence, protected admin
reads, restart/session persistence, CSRF rejection, login/logout, identical
unknown/wrong-password errors, rate admission and revocation after deleting the
database identity. Run `pwsh -File scripts/smoke-auth.ps1` to repeat the registry
proof. For upstream changes only, add `-ToolManifest ../axonyx-framework/Cargo.toml`.

This is HTTP acceptance, not browser UX/accessibility testing or a security
audit. Pilot PR #1 passed Linux CI and merged into dev. Browser acceptance is
documented separately in browser-auth-v0.md; it found redirect/CSRF gaps now
fixed in core 0.6.5/runtime 0.6.4. CI installs CLI 0.6.8 and repeats isolated
HTTP and browser acceptance on Linux. No production-ready CMS or installer
is claimed.

Prove token rejection, invalid-input 422 retention, first administrator creation,
setup locking (including parallel requests), password hashing, private admin
reads, login/logout, CSRF rejection, persistence after restart and secret-free
responses before merging this pilot. Presence of the code is not proof.
