# Media Storage V0: local adapter foundation

Legura is self-hosted CMS software, not a hosted storage service. The operator
chooses storage. This first slice adds an internal Rust contract and local disk
implementation only; there is no HTTP upload route, admin media page, DB schema,
post image field, public download URL or configured production activation yet.

## Object contract

`ObjectStorage` accepts bounded bytes and returns a `StoredObject`: logical
store ID, opaque random key, byte length and SHA-256. `get` takes the store/key
reference, not a filename, URL, absolute path or provider credential. References
must retain the store ID when persisted so a configuration change cannot silently
reinterpret an old key in another store. IDs are operator-selected logical names,
not a list of implemented providers.
Store IDs must remain stable: do not repoint an existing ID at a different bucket
or directory and assume its data moved. An explicit migration must transfer and
verify objects before changing how existing references resolve.

`LocalStorage::open` receives an existing operator-provisioned directory, a
`StoreId` and a nonzero object limit no greater than 8 MiB. It does not auto-create
directories or read `.env`, and is not mounted under the app's static public root.
The directory and its ancestors must be controlled by the operator, inaccessible
to untrusted local users, with appropriate permissions and quotas. Capabilities
are not protection against the machine's administrator or malicious hard links
introduced by someone already able to write that directory.

The directory handle comes from cap-std. Operations are relative to that handle;
final-component symlinks are not followed. Keys are exactly 64 lowercase hex
characters from 32 random OS bytes. Original names never become storage paths.
Writes use create-new, never overwrite an existing file and sync file contents
before returning success. Failed writes attempt cleanup. Process/OS crashes may
leave unreferenced files: there is no crash-atomic DB/file transaction or garbage
collector in V0. Directory-entry durability across power failure is not promised.

Reads reject wrong stores, missing/empty/nonregular objects and oversized data.
They bound allocation to the configured limit plus one byte even if a file grows
after metadata inspection. The synchronous small-object API is not an async S3
client or HTTP streaming abstraction. A later service must bound concurrent
operations and move blocking work off async request executors.

## Next vertical slices

1. Decode and validate bounded JPG/PNG/WebP data, including pixel/decompression
   limits; do not trust suffixes, declared MIME or a magic prefix alone. The raw
   storage adapter does not certify that its bytes are images.
2. Administrator/CSRF-protected bounded upload transport, media metadata in SQLite,
   safe public response headers, alt text and a library page.
3. Post image selection, full saved/public rendering and consistency/error cases.
4. Media-aware backup/restore acceptance before calling the media flow complete.
   Current SQLite-only backups do not include future media files.

S3-compatible providers and migration between providers are future adapters and
explicit transfer operations, not effects of changing one config value. Retention,
reference-safe deletion and orphan cleanup need their own product policy.

## Verification

`cargo test --locked --lib` exercises object boundaries, restart/reopen reads,
hash/length metadata, trait dispatch, size rejection without writes, no-clobber,
cross-store rejection and concurrent random-key writes. Linux additionally proves
inside/outside symlinks are refused. CI runs library tests and strict Clippy on
Windows and Linux. No operator database/configuration is required or modified.
