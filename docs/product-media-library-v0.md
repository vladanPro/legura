# Product Media Library V0

The protected product route is `/admin/media`. It composes the existing Foundry
administration shell, empty state, image upload and ten-item paginated grid.
Technical IDs stay in collapsed details. The
[private HTTP extension](private-media-http-v0.md) supplies authenticated image
previews. Post attachments, image deletion and public URLs remain separate.

## Explicit Opt-In

Media is disabled when settings are absent or `AX_SECRET_MEDIA_DRIVER=disabled`.
In that state administration navigation omits the link, and even an authenticated
administrator receives 403 from the library and both upload actions. Hiding the
link is not the authorization boundary.

Local storage requires all three private settings:

```dotenv
AX_SECRET_MEDIA_DRIVER=local
AX_SECRET_MEDIA_ROOT=/private/legura/media
AX_SECRET_MEDIA_STORE_ID=local-media
```

This uses the application SQLite database, not a second Media DB. The private
directory and current migrations, including migration 004, must already exist.
Do not put storage in `public`, `dist` or the project root. Partial, unsupported,
public or invalid settings refuse startup; they never enable raw fallback.

For an existing installation, follow the explicit backup, stopped-service,
migration/configuration validation and restart procedure in
[native media configuration](native-media-config-v0.md) and
[recovery compatibility](recovery-compatibility-v0.md). Opening the page does not
provision a directory, migrate a DB or change settings. Configuration changes
require a restart. This work does not change any operator installation.

## Request And Authorization Boundaries

The product config retains the 64 KiB ordinary-request budget and gives 9 MiB
only to the two known upload actions at `/admin/media`. Request limits apply
before body collection even when storage is disabled: this is bounded buffering,
not streaming, pre-body authorization or a total process-memory guarantee.

Each loader/action checks current administrator identity. Upload actions then
check server-owned availability and traverse the configured image-storage
adapter. CSRF/origin, separate 8 MiB image limit, pixel bounds, re-encoding,
metadata persistence and no-raw-fallback policy remain in force. Client props or
forged availability flags cannot enable storage. Native upload redirects to the
library; existing validation failures remain safe errors, not a new toast UX.

The administration pages obtain the boolean from a guarded server query and pass
it to the shared AdminFrame. New-post forms forward the same prop. No paths,
database settings or secrets are included. This is an explicit page binding,
not a new global client state or duplicated browser configuration.

## Acceptance

The Media harness now runs the product files directly rather than copying a
second route from tests. `-Media` always exercises scoped product limits;
`-ScopedMedia` remains accepted for existing scripts. There is no fixture-only
global 9 MiB override. Existing HTTP, browser, archive and recovery assertions
target `/admin/media`.

Acceptance covers disabled navigation/read/write denial, enabled navigation from
overview/posts/new-post pages, selected library navigation, guarded uploads,
canonical bytes/metadata, pagination, restart, orphan-on-DB-failure behavior and
backup/recovery including disabled recovered settings. Existing browser CI covers
both JavaScript modes and desktop/mobile layouts. All required CI gates must pass
before merging; local HTTP results do not imply local browser certification.

Local Windows registry HTTP/native acceptance passed on 2026-10-18, port 3964,
using CLI 0.6.16/runtime 0.6.11 without source overrides. It tested the actual
product route/config, default-off navigation/read/write denial, enabled
navigation, uploads and the full recovery flow above. The randomized fixture
legura-auth-ffa7d94bac0d442c975c33a122278423 and its private Cargo target were
removed. All 66 library tests, 21 content/media integration tests, Clippy with
warnings denied, Rust/ASX formatting and PowerShell parse checks passed. Required
Linux/Windows and browser CI remains a separate gate before merge.
