# Editor Package Assets V0

The pinned editor Cargo package now embeds its component, stylesheet and three
browser modules. `legura_editor::assets` exposes bundle layout version 1. Build
tooling no longer locates a checkout with Cargo metadata or reads src/ internals.
ASX and browser code come from the same dependency build as Rust validation.

```powershell
cargo run --locked --bin legura-editor-assets -- /path/to/new-editor-bundle
```

The destination parent must exist and the destination itself must not exist.
The tool exports components/ and public/, then writes bundle.json with layout
version, package/version, byte counts and SHA-256 hashes. It refuses unsupported
layout/path traversal, case-insensitive duplicate paths and existing output.
Files use create_new; Unix bundle roots are private to the user. Incomplete
exports remain incomplete rather than claiming success or overwriting a retry.
There is no --force, deletion, managed upgrade or modification of app/public/data.

The integration proof now uses the side-by-side installer described in
editor-managed-install-v0.md under app/components and public/content-editor. The lifecycle
host remains application-owned, not part of the editing engine. The bundle hash
inventory describes exact exported bytes, not an authenticity signature or a
cross-platform newline-independent hash. Cargo.lock selects the immutable Git pin.

The editor is still Git-pinned and publish=false; no Cargo/npm release occurred.
JS editing and native full-page 422 recovery use registry runtime 0.6.9. Native Retry CI
uses the matching editor source commit, not an older package without the asset API.

## Gates

Unit tests verify every exported byte/hash, reject a second export without
changing inventory/unknown files, and reject unsafe paths. The editor crate's
packaged archive was built and verified with all five embedded assets. Existing
license/repository metadata warnings are intentional unresolved publication gates.

PostEditor JS/no-JS proofs use installed assets rather than checkout file copies.
The native-retry source proof remains separate from registry acceptance. The
operator DB, environment, source checkout and server are untouched. This is a
safe export/build integration step, not automatic installation into an existing
CMS or activation of rich editing. Managed updates/conflicts/rollback and explicit
operator activation remain next gates; licensing precedes editor publication.

Local acceptance: 21 CMS library tests, strict Clippy, formatting, the native
content proof, registry ContentEditor JS and PostEditor JS/no-JS, and the separate
source NativeRetry browser proof passed. Linux CI remains the merge gate.
