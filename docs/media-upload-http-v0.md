# Isolated Media Upload HTTP V0

This source pilot integrates the reviewed framework startup hook and Legura image
adapter using a disposable CMS installation with real setup, administrator
session, CSRF and database. It does not activate uploads in the operator CMS or
publish a product media schema/UI, public media route, installer or backup upgrade.

## Service and stored data

MediaLibraryStorage opens an existing private LocalStorage and an existing SQLite
database with a prepared media schema. It does not create a database, migrate
schema, read environment settings or create the directory. Test startup supplies
explicit fixture paths; the product gets no automatic storage activation.

save_file delegates validation/re-encoding to MediaUploadStorage. Only afterward
does a parameter-bound INSERT persist the opaque key, logical store ID, canonical
MIME, processed byte length, dimensions and SHA-256 of the processed bytes.
Filename, request MIME and payload metadata are not used as paths or trusted data.
Unknown capabilities fail closed. Errors omit filesystem paths, SQL and codec
details. The prototype has no uploader audit column, alt-text editing or deletion.

File and SQLite writes are NOT a cross-resource transaction. If metadata insertion
fails after a successful object write, return no successful FileRef and retain an
orphan. Never delete a possibly committed object during ambiguous DB failures.
The test injects a deterministic abort and proves existing rows/objects unchanged.
Repair/GC, quota accounting and media-inclusive backups remain release gates.
Do not expose this directory directly as public static storage.

## Isolated route

The test overlays /admin/media-proof, its guarded loader/action, and a fixture-only
004 migration. The page demonstrates listing stored metadata and a native
multipart form, not the designed final Media Library. The upload action calls
the existing current-administrator query before Storage.save. The loader checks
the current subject/user/admin role directly: nested query-to-query calls are not
currently supported by the compiler. No compiler behavior is changed here.

Product max_body_bytes remains 64 KiB. Only the disposable proof uses a 9 MiB
transport budget for the 8 MiB image payload budget plus multipart overhead.
The native registration hook is copied into that fixture only, never src/ in
the operator checkout. All product Cargo dependencies remain registry/pinned.
The CLI hook is published in cargo-axonyx 0.6.15; CI now installs that exact
registry release. No global developer CLI installation is replaced locally.

```text
pwsh -File scripts/smoke-auth.ps1 -Media -ToolPath <registry-0.6.15-cargo-axonyx-binary> -Port 3963
```

The source CLI may also be selected by ToolManifest; prefer a built ToolPath so
the app-only isolated target does not rebuild the CLI. ToolPath and ToolManifest
are mutually exclusive. CI uses `cargo install cargo-axonyx --version '=0.6.15'
--locked --debug --force` in its disposable runner.

## Acceptance

The proof covers anonymous and revoked users, missing/forged CSRF, hostile origin,
unsupported/empty/oversized images, transport oversize, canonical output metadata
and bytes, SQLite hashes/rows, guarded no-store metadata listing, private object
non-serving, server restart/session reuse and explicit DB-insert failure policy.
It also retains the full existing setup-race/login/logout/password/auth smoke.

Library unit tests cover bound metadata inserts/constraints and startup refusal
without existing DB/schema. New CI Isolated Media Upload HTTP runs the real server.
Windows local acceptance passed the complete HTTP proof, all 52 library tests,
strict library/test Clippy, Rust and Axonyx fixture format checks. The isolated
server was stopped and its temporary fixture cleaned. Linux and the existing
broader gates passed before the original source-pilot integration. Registry CLI
adoption is checked separately by the current Media CI; neither is operator activation.

## Remaining before activation

The startup CLI is now published, the versioned product migration exists and
isolated media-aware archive/recovery tests are available. Before activation,
finish explicit operator opt-in, upload budget and delivery policy. Public media
serving, alt text, post image selection, cleanup and independent provider migration
remain open. See native-media-config-v0.md and legura-archive-v0.md for current scope.
The initial implementation is local SQLite/disk only, not cloud storage or hosting.
