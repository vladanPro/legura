# Setup And Identity Pilot

This document describes the first setup/identity slice, not an installable
production CMS release. The broader source pilot now includes posts and an
editor; recovery emails, social login and plugin loading are still out of scope.

Current registry pins are core/runtime `0.8.3` and CLI `0.8.8`. Registry-only
compiled HTTP/browser acceptance passed in CI on 2026-11-03, including the
account-key login throttle. The original 2026-10-09 acceptance used earlier
`0.6.x` packages; those are historical, not the versions to install today.
Do not weaken optional credential verification or remove guards to pass build.

## Boundaries

- Owner configures the database, session key and independent setup token on the
  server. The token is entered as a password field; it is never embedded in HTML.
- Setup requires that token and an unclaimed installation. The user, credential
  and singleton installation row are committed in one transaction. A losing
  concurrent setup must roll back all of its writes.
- Passwords use the framework Password API; only hashes enter the database.
- Legura setup uses an explicit product policy of 10 to 256 UTF-16 code units,
  matching the current Axonyx String.length semantics. It does not trim passwords
  or require specific character classes. This deliberately differs from the
  framework's 15-code-point password-only registration baseline. Use a unique
  password; this policy is not a claim of NIST password-policy compliance.
- Admin reads require a trusted session and a current database admin role.
- Anonymous admin reads and claimed setup reads return 403. Query guard redirects
  are not supported in this slice; framework issue #326 tracks that UX boundary.
- `env.SETUP_TOKEN` is the logical secret key; the process environment stores it
  as `AX_SECRET_SETUP_TOKEN`. The prefix is not repeated in the DSL lookup.
- Mutations use framework same-origin/CSRF checks. Logout destroys the session.
- Native validation retains site name/email only. Passwords and setup tokens
  must be entered again. Unknown-account and wrong-password errors are identical.
- Sign-in is limited to five attempts per normalized account key per minute,
  plus a shared cap of 60 attempts per minute. A limited request returns 429
  with `Retry-After` and no session cookie. Both counters are process-local;
  a restart clears them and multiple server instances do not share a budget.
  The shared cap can also block legitimate users under abuse. This is a
  single-administrator pilot mitigation, not a final multi-user rate-limit policy.

## Local Development

Install the tested CLI, then initialize the local development pilot:

```powershell
cargo install cargo-axonyx --version 0.8.8 --locked --force
pwsh -File scripts/local.ps1 -Task prepare
pwsh -File scripts/local.ps1 -Task start
```

Requires PowerShell 7.4 or later. Open http://127.0.0.1:3940/setup. The local prepare script generates independent
random secrets into the ignored .env file, runs migrations and pulls schema.
Read the setup token from that local file; do not commit or share it.
Database and credentials remain in ignored data/ storage. Never point this
development pilot at a production database.

See installation.md for the exact source workflow and local-only safeguards.

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
The original 2026-10-09 proof used source and then-published registry tooling.
The 2026-11-03 CI proof uses published runtime `0.8.3` and CLI `0.8.7`:
migration/check/compiled build, token
rejection, invalid-input retention without credential replay, exactly one owner
from parallel setup, transaction rollback, Argon2id persistence, protected admin
reads, restart/session persistence, CSRF rejection, login/logout, identical
unknown/wrong-password errors, per-account rate admission and revocation after
deleting the database identity. The independent-account assertion additionally
passed the isolated local compiled smoke with source CLI on 2026-11-04.
Run `pwsh -File scripts/smoke-auth.ps1` to repeat the registry proof. For
upstream changes only, add `-ToolManifest ../axonyx-framework/Cargo.toml`.

This is HTTP acceptance, not browser UX/accessibility testing or a security
audit. Pilot PR #1 passed Linux CI and merged into dev. Browser acceptance is
documented separately in browser-auth-v0.md; it found historical redirect/CSRF
gaps that were fixed in core 0.6.5/runtime 0.6.4. Current CI installs CLI
0.8.8 and repeats isolated HTTP and browser acceptance on Linux. No
production-ready CMS or installer is claimed.

Prove token rejection, invalid-input 422 retention, first administrator creation,
setup locking (including parallel requests), password hashing, private admin
reads, login/logout, CSRF rejection, persistence after restart and secret-free
responses before merging this pilot. Presence of the code is not proof.
