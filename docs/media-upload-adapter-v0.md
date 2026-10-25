# Media Upload Adapter V0

This slice bridges the image boundary to Axonyx's existing `AxFileStorage` trait.
It is not an activated HTTP upload route, media library, metadata schema or CMS
release. It depends on the local storage and still-image validation foundations.

`MediaUploadStorage<S: ObjectStorage>` accepts only the `legura-media` capability.
It fully prepares an image before calling the store. Submitted filenames,
extensions and Content-Type never determine object paths or response metadata.
The returned FileRef has the logical store ID, opaque key, canonical MIME/size
and application-generated key.jpg/key.png display filename. `save_image` also
returns the processed digest and dimensions for a later metadata transaction.

Construct once per process and share the adapter. A nonblocking mutex permits
one decode/write per instance and immediately refuses concurrent calls; poison
fails closed. This is not a global upload rate limit or an async scheduler.
Transport still needs limits before buffering multipart bodies and before queuing
blocking work. No upload bytes pass through Native.call's JSON envelope.

The bridge grants no authority. Real routes must check the current administrator
and CSRF before calling it. Unknown capabilities fail forbidden. Invalid images
become typed InvalidInput for the image field; storage failures use fixed public
messages without paths or codec details. Busy and error-to-HTTP/field mappings
still need acceptance at the future HTTP boundary, not assumed 422/429 behavior.

## Compiled proof

The existing native-content proof now compiles a real UploadMedia(image: File)
action using Storage.save, with a fixture-only bearer guard. The same source is
executed through compiled and preview dispatch against a disposable LocalStorage.
Anonymous/forged requests and invalid bytes leave no object; valid content uses
the canonical bytes and trusted metadata and can be reopened after storage.
This is not a session/CSRF, multipart transport or public-server proof. The bearer
is a disposable test constant, never a CMS authentication mechanism.

Local commands:

```text
cargo test --locked --lib
cargo clippy --locked --lib --tests -- -D warnings
cargo run --locked --manifest-path tests/native-content/Cargo.toml
```

The proof uses existing CI; its image encoder is test-fixture tooling, not npm.
No local operator DB/config/server is touched.
Library/tests strict Clippy passes. Additional Clippy on the generated consumer
reports four codegen-only style warnings (owned string comparisons, redundant
closure, match-single-binding); its ordinary check succeeds. No generated Rust
is manually rewritten or lint allowance added to hide those upstream warnings.

## Integration gap and next boundary

The current compiled startup builds a concrete AxStorageRegistry from configured
raw file capabilities. src/native.rs registers JSON callbacks only; it cannot
install this custom AxFileStorage. Do not bypass image validation by saving raw
uploads, raising Native.call's limits or introducing a second CMS server.

Next add an explicit application storage-registration seam to the generated
startup, with a compiled consumer proof. Then wire administrator/CSRF-protected
bounded upload, SQLite metadata and library UI in one isolated HTTP flow.
File write plus DB insert is not atomic: failure/orphan policy and media-aware
backup/restore must be covered before exposing the flow to operators.
Framework tracking: https://github.com/vladanPro/axonyx-framework/issues/356
