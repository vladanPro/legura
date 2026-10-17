# Media Image Validation V0

This is the next storage prerequisite, not an activated upload feature. The CMS
does not yet have an HTTP media endpoint, library page, metadata migration,
post attachment or public image-serving route. Operator data and configuration
are unchanged.

## Boundary

`media_image::MediaImage::prepare(&[u8])` accepts untrusted bytes, not a client
filename, extension or declared Content-Type. JPEG, PNG and WebP signatures choose
one of three enabled Rust decoders. The complete selected image must decode;
a signature alone is insufficient. SVG, GIF and other formats are refused.
APNG and animated WebP are refused, not silently reduced to the first frame.

The output is a private-constructor value exposing bytes, dimensions, trusted
Content-Type and extension. A later authorized service can pass its bytes to
`ObjectStorage::put`, persisting metadata for those processed bytes and the logical
store/key reference. This module has no filesystem, SQL, HTTP or auth authority.
The generic storage adapter still accepts arbitrary bytes; an upload service must
explicitly enforce this image boundary before calling it.

## Limits and normalization

- Encoded input and output: at most 8 MiB each.
- Width/height: at most 8192 each, nonzero; product at most 16 million pixels.
- Dimension/product checks occur before full pixel decoding.
- Decoder allocation budget: 64 MiB, best-effort as documented by `image`, not a
  hard process-RSS limit or timeout. Decoder, orientation, conversion and encoder
  buffers can coexist. CPU/concurrency isolation remains a transport requirement.
- EXIF orientation is applied to pixels; original metadata is not copied.
- JPEG is re-encoded as JPEG at quality 85. PNG/WebP become 8-bit RGBA PNG,
  retaining transparency. This intentionally discards original metadata, high
  bit depth and original compression; it is not archival preservation or a
  color-managed ICC workflow. A valid compact WebP may exceed the output limit
  as PNG and be refused. WebP output/variants are later capabilities.
- Encoding writes through a bounded sink; output overflow returns an error and
  no `MediaImage`. Original bytes and trailing input payloads are never returned.

Use the returned Content-Type for future serving, with nosniff and a fixed
application-controlled route. Do not expose the private storage directory as
static files. This is defense in depth, not antivirus or proof that a codec has
no vulnerabilities. Keep decoder dependencies maintained.

## Verification

`cargo test --locked --lib` includes all three real encoded formats, alpha
preservation, canonical metadata, EXIF rotation, stripped trailing payload,
malformed/truncated input, unsupported active content, APNG/animated WebP,
dimension/product rejection, bounded real encoders and an isolated local-store
roundtrip. No fixtures are sourced from user content and no CMS DB is opened.

Both root and native-content proof lockfiles include the same image dependency.
Existing Linux/Windows library CI runs the boundary; broad auth/content regression
gates remain mandatory before integration into dev.

The first broad CI attempt exposed an existing post-save test race, not an image
decode failure: waitForURL resolved against the current editor URL before the
action's same-URL redirect committed, allowing the next /admin navigation to be
aborted. The submit helper now arms a new main-frame commit/load wait before
clicking. Two isolated browser regressions cover delayed same-URL JS redirects
and native 303 form redirects; no sleep/retry masks the failure. The compiled-auth
job budget is 30 minutes, matching the other full-flow gates, to accommodate its
multiple fresh source builds with the image codecs. Required checks are retained.

## Next

Add an administrator/CSRF-protected transport with a bounded body reader before
allocation, concurrency limits and blocking decoder work off the async executor.
Then add SQLite media metadata and the admin library as one isolated HTTP proof.
Media-aware backup/restore remains a release gate before calling uploads complete.

API references:
- https://docs.rs/image/0.25.10/image/trait.ImageDecoder.html
- https://docs.rs/image/0.25.10/image/struct.Limits.html
