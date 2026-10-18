# Scoped Media Transport V0 - Source Acceptance

This is an isolated acceptance mode, not product Media activation or a package
release. Product manifests, operator settings, database and running CMS are not
changed. The existing registry Media job still proves its pinned packages.

Framework PR #362 is merged into dev at `be84f76`. The source CI job checks out
that exact framework revision and its runtime submodule, builds a private test
CLI and applies a runtime path override only to the disposable application.
No developer global CLI is replaced. This must not be described as registry-only
acceptance; compatible runtime/tooling publication is still a separate gate.

## Reproduce

```text
pwsh -File scripts/smoke-auth.ps1 -Media -ScopedMedia -ToolPath <source-CLI-binary> -RuntimeSource <runtime-crate-directory> -Port 3964
```

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

Before operator activation, publish compatible framework packages and repeat this
proof without source overrides, then implement the separate explicit Media opt-in
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
