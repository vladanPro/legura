# Media Availability V0

This is a server-side foundation for the later product Media opt-in. It does not
add a product route, sidebar link, upload grant or private image response.

The product native registry registers `legura.media.availability` at startup.
It accepts only `null` (the Rust unit input) and returns `{ available: boolean }`.
Objects, including a client-supplied `available` or storage root, are rejected.
There is no new HTTP endpoint for this function.

The compiled call is `Native.call("legura.media.availability", null)`. An empty
object currently fails native payload code generation in core 0.6.9; fixing that
general compiler edge case is tracked in
[framework issue 365](https://github.com/vladanPro/axonyx-framework/issues/365),
separate from this parameterless product helper.

The startup snapshot uses the same configuration validation as the file-storage
hook. No configuration or explicit `disabled` yields false, without opening the
database or storage. Valid `local` configuration requires an existing private
directory, application SQLite database, current migration/schema contract and
compatible store references. Partial, public, missing or invalid settings abort
registration with a safe error; they do not become a usable library or raw-file
fallback. Validation does not create directories/databases or apply migrations.

Only the boolean is captured by the callback. Paths, store identifiers, database
URLs and secrets are not returned. Settings are a startup snapshot: changing
configuration requires the explicit operator stop/check/start procedure.

Availability is presentation information, not authorization or a guarantee that
disk/DB operations will succeed later. Future product loaders/actions must still
check the current administrator. Uploads still traverse the configured storage,
independent image validation, CSRF/origin and bounded request policies. A true
snapshot does not bypass any of those checks.

## Acceptance

- Unit tests cover disabled configuration, exact result shape, forged inputs,
  valid migrated storage and missing/unmigrated/partial/public configuration.
- Failed registration leaves no callback, database or media directory behind.
- The isolated Media loader authorizes the administrator before calling the
  helper; its rendered page proves the boolean reaches compiled ASX and checks
  that configured private paths and session/setup secrets are absent.
- The existing registry scoped HTTP/recovery and browser CI gates remain required.

Local Windows registry HTTP/native acceptance passed on 2026-10-18, port 3964,
with CLI 0.6.16/runtime 0.6.11 and no source overrides. It exercised the helper
through the guarded compiled loader/page and retained the full scoped upload,
archive, new-only recovery, isolated activation and revoked-session checks.
The randomized fixture and its private target were removed. Rust unit tests,
Clippy with warnings denied and source formatting passed. Local browser execution
is not claimed; PR CI is a separate merge gate.

No operator configuration, database, running CMS or global toolchain is changed.
The actual Media page/navigation and authenticated binary delivery remain separate
steps from this foundation.
