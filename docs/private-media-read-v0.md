# Private Media Read V0 - Rust Boundary Only

This document describes the tested product read service and response builder.
The subsequent [private HTTP extension](private-media-http-v0.md) connects it to
compiled routes using registry 0.7.0. No public storage directory, JSON/base64 image
payload or generated-host edit is introduced.

`media_read::read_for_administrator` takes an explicit database connection,
startup-validated object-storage capability/store, a server-verified session
subject and a logical media ID. The caller must obtain the subject from real
session verification, never a query/body parameter. There is deliberately no
native JSON registry callback accepting a purported user ID.

Each read checks the current database administrator before reading media metadata
or touching object bytes. There is no cached role grant. Invalid/unknown IDs or
unreferenced orphan objects cannot become arbitrary filesystem reads. Metadata
must select the configured store, canonical key, bounded size/dimensions,
allowlisted PNG/JPEG type and SHA-256. LocalStorage uses a bounded, no-final-symlink
read and refuses directories/nonregular files. Size, hash, detected format and
image-header dimensions must match before a response can be constructed.

The private result is not serializable. It can become an `AxHttpResponse` with
exact bytes, trusted MIME, `Cache-Control: private, no-store`, `nosniff`, inline
disposition and same-origin resource policy. Errors expose only a safe generic
message; HTTP error mapping is not implemented by this Rust-only slice.

This verifies stored canonical images without re-decoding/re-encoding their pixel
buffers on every read. It does not protect against a malicious host/DB owner,
revoke already downloaded bytes or interrupt an authorized in-flight read.
Per-object bounds are not an aggregate/concurrent process-memory guarantee.
The caller still owns disabled-configuration rejection and service lifecycle.

## Evidence

Local Windows checks pass: 70 library tests and two filesystem integration tests.
Tests cover real canonical PNG/JPEG uploads followed by reopened private reads,
exact response bytes/headers, unchanged DB/object bytes, revoked or absent users,
forged subject strings, invalid IDs, wrong store, missing/orphan metadata,
corruption, mismatched MIME/dimensions/size, oversized and nonregular objects.
A Unix-only test rejects a final symlink even when its target bytes match the
metadata. Linux CI must execute that test before it is claimed passed.

The filesystem tests create randomized disposable roots and remove only those
roots. No operator DB, media, environment or process is involved. Rust formatting,
Clippy with warnings denied and diff checks pass; all PR CI gates remain required.

## Compiler/Runtime HTTP Bridge

Existing `Native.call` deliberately uses bounded JSON. Image bytes must not be
smuggled through it. The HTTP extension uses a separate typed private
binary channel with verified request context, safe errors and response validation.
Registry adoption follows the coordinated 0.7 release and isolated acceptance.
Streaming, range/conditional responses and public post images remain separate contracts.
