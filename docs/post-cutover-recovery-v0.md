# Post-Cutover Recovery V0

This is a fail-closed policy and an isolated acceptance test, not an automatic
rollback command or a production upgrade procedure. The test uses independently
built Legura `0.0.1` and `0.0.2` packages with one disposable external SQLite
database and media directory.

## Why rollback is not automatic

After `0.0.2` accepts a new post, restoring the pre-cutover `0.0.1` archive
would remove that post from the active database. The `0.0.1` maintenance reader
also rejects archives produced by `0.0.2`. Starting the old binary against the
live database after new writes is not a safe shortcut: later versions may have
changed schema or content contracts. A successful pre-cutover backup check does
not make new writes disappear safely.

## Isolated proof

`scripts/smoke-package-cutover.ps1` now:

1. Publishes a post and image under `0.0.1`, stops the test server, confirms
   its HTTP port is free, then verifies a media-inclusive pre-cutover archive
   before starting `0.0.2`.
2. Publishes a second post under `0.0.2`, stops that test server and confirms
   the port is free before creating the new archive. The old maintenance reader
   must refuse this newer archive.
3. Restores both archives into different **inactive, new** directories and
   checks their schema, content and media without switching the live server.
4. Confirms the older recovery has only the first post, the newer recovery has
   both posts, image bytes match, and the restarted `0.0.2` server still serves
   the second post and accepts a fresh administrator login.

CI uses test-only Python to compare recovered SQLite rows. No Python dependency
is added to the native Legura package or runtime.

## Operator boundary

Before any real cutover, arrange a maintenance/write-freeze window and retain a
verified final pre-cutover archive made **after** all writers stop. Checking
that one HTTP port is free does not prove other instances or workers are gone.
If the new version has accepted writes, stop all new-version writers and
preserve a separate verified post-cutover archive **before** making
any recovery decision. Do not point the old package at the new live database,
overwrite either archive, or silently activate the older recovery. Reconcile
new posts/media and any other accepted writes explicitly, or keep the newer
version running while investigating. Legura currently has no supported
cross-version reverse migration, archive merge, automatic write reconciliation
or unattended rollback. Existing session secrets must not be treated as
restored login authority; see recovery-activation-proof-v0.md.

The test demonstrates why this decision is necessary; it does not provide a
general tool for detecting all writes, make an operator decision, or claim a
production-safe rollback.
