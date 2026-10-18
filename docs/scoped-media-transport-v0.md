# Scoped Media Transport V0

This is an isolated acceptance mode, not product Media activation. Legura now
selects published runtime 0.6.11, with CLI 0.6.16 in executable acceptance jobs.
Operator settings, database and running CMS are not changed. UI and the pinned
editor are unchanged. The native proof uses core 0.6.9 to match this runtime.

The existing Media job now runs ScopedMedia plus its browser checks against
registry packages. The separate framework-source build job is retired rather
than duplicating the same upload/recovery proof. Native editor retry also uses
the published runtime and the product's pinned Git editor, without path patches.
The source-format job retains its independent formatter pin.

## Historical Source Gate

Framework PR #362 is merged into dev at `be84f76`. The source CI job checks out
that exact framework revision and its runtime submodule, builds a private test
CLI and applies a runtime path override only to the disposable application.
The framework repository does not track Cargo.lock: CI first generates a test
lockfile and then builds with `--locked`. Framework source is pinned; transitive
dependency resolution is not claimed to be identical across future CI runs.
No developer global CLI is replaced. This must not be described as registry-only
acceptance. Compatible packages have since been published after green main CI.

## Reproduce

```text
pwsh -File scripts/smoke-auth.ps1 -Media -ScopedMedia -ToolPath <source-CLI-binary> -RuntimeSource <runtime-crate-directory> -Port 3964
```

Current registry reproduction (install the CLI in an isolated root if a global
toolchain must remain unchanged):

```text
cargo install cargo-axonyx --version '=0.6.16' --locked --debug
pwsh -File scripts/smoke-auth.ps1 -Media -ScopedMedia -ToolPath <installed-CLI-binary> -Port 3964
```

Do not pass RuntimeSource, ToolManifest, UiSource or EditorSource in this gate.

`ScopedMedia` requires `Media`. The fixture retains the product's 64 KiB default
and grants 9 MiB only to `UploadLibraryImage` and `UploadLibraryImageAndReturn`
at `/admin/media-proof`. The harness verifies that the selected toolchain actually
emits those policies into the generated host, so an older CLI ignoring an unknown
config key cannot accidentally pass. Without `ScopedMedia`, existing registry
acceptance keeps its original fixture-only global allowance and expectations.

## Boundaries Proved

- Both known-length and chunked login/unknown/ambiguous action requests over
  64 KiB receive 413 and create no media rows or objects.
- The selected Media endpoint still requires current administrator authorization,
  same-origin checks and CSRF. A larger anonymous or tokenless upload is denied.
- An authenticated chunked upload with multipart padding above 64 KiB succeeds
  under the selected policy. Padding is a separate file part: the independent
  64 KiB text-field cap remains intact. Stored output still contains only the
  canonical re-encoded image, with matching metadata and checksum.
- A selected upload over 9 MiB receives 413, including with chunked transport.
  The separate 8 MiB image limit and invalid-image 422 checks remain unchanged.
- Native upload returns 303 to the library with a larger request. Existing
  pagination, restart, failure/orphan policy, archive, no-clobber recovery,
  recovered-session rotation and permission-revocation checks still execute.

This is bounded request buffering, not storage streaming or a total-memory cap.
No private image delivery, product route, metadata migration or plugin API is
introduced. Optional existing browser acceptance may be added with `MediaBrowser`;
the source CI job here is HTTP/native acceptance, not a new browser certification.

Before operator activation, pass this proof without source overrides, then
implement the separate explicit Media opt-in
and authenticated delivery gates from the activation plan in axonyx-notes.

## Local Evidence

On 2026-10-18 the full source acceptance command passed on Windows, port 3964,
using framework `be84f76` and its runtime submodule. This includes both scoped
upload actions, ordinary-request bounds, image validation, persistence, archive
verification, new-only restore, isolated recovery activation and auth revocation.
The refused second restore into an existing directory is an expected negative
test, not a failed overall run. Generated host compilation reported the known
unused legacy host import; this is not a warning-free claim.

Fixture sources, SQLite/media storage and Cargo target were created under a
randomized temporary root and removed afterward. Operator checkout changes,
global CLI, environment and running CMS were not modified. Linux CI and future
registry acceptance are separate evidence gates, not inferred from this run.

### Published Registry Acceptance

On 2026-10-18 the current registry command passed on Windows, port 3964, using
the isolated crates.io installation of CLI 0.6.16 and runtime 0.6.11. No source
overrides were supplied. Both tracked lockfiles select registry core 0.6.9 and
runtime 0.6.11. This passed all scoped HTTP/native checks above, archive/recovery,
isolated recovery activation and revoked-administrator denial. The expected
second-restore refusal is a negative assertion, not a failed overall command.

The randomized fixture legura-auth-2305a9d571c449dd9cfea6d217ba05f6 and its private
Cargo target were removed. The 63 library tests and separate locked native-content
proof also passed. Local browser certification is not claimed by this HTTP run;
the seven Linux/Windows CI gates, including existing browser checks, remain the
merge boundary for the adoption PR.
