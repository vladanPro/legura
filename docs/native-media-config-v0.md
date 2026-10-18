# Native Media Configuration V0

Legura now owns `src/native_storage.rs` and `src/media_config.rs`. The compiled
server startup hook uses this product adapter; tests no longer substitute a
different adapter or supply `AX_TEST_MEDIA_*` database/storage settings.

The storage startup hook is published in `cargo-axonyx 0.6.15`. The media CI job
installs that exact registry version with `--locked`, without a framework Git
checkout. Preview mode is not a supported native-storage host. Do not assume an
older global CLI wires this hook. This remains an isolated product pilot, not
operator media activation.

For an already verified inactive archive recovery, the maintenance tool can
generate NEW private settings instead of copying old secrets. See
[Private Recovery Configuration V0](recovery-private-config-v0.md); service
activation remains separate from preparing configuration.

## Disabled By Default

With no media settings, or only the following setting, media writes are disabled:

```dotenv
AX_SECRET_MEDIA_DRIVER=disabled
```

Do not leave active root/store entries beside `disabled`; partial/conflicting
configuration is an error. A request for `legura-media` never passes through to
the framework's raw fallback store, whether disabled or enabled. Other capability
names retain their existing fallback behavior and request authorization rules.

## Explicit Local Adapter

Provision a dedicated private directory first. Then configure the same process
environment or private `.env` used by the rest of the compiled application:

```dotenv
AX_SECRET_DB_URL=sqlite://data/legura.db
AX_SECRET_DB_DIALECT=sqlite
AX_SECRET_MEDIA_DRIVER=local
AX_SECRET_MEDIA_ROOT=/private/legura/media
AX_SECRET_MEDIA_STORE_ID=local-media
```

Use an actual existing directory path on your OS. Relative DB/media paths resolve
against the process working directory, not the executable location. Keep that
directory fixed when starting the service. Unix directories should be 0700;
restrict Windows ACLs to the service identity. No directory permissions are
automatically changed. These settings use the normal AxEnv `.env` loading and
process-environment precedence, not another configuration parser.

There is deliberately no second media DB URL/path setting. The adapter uses
`AxEnv.database_config()` and the same application DB. Only direct SQLite is
implemented; memory, API transport, Postgres/MySQL and S3 are not media drivers
in this version. Supported file URL prefixes are `sqlite:`, `sqlite://` and
`sqlite:///`; URI options/fragments and `file:`/memory forms are refused.
Canonical Windows verbatim paths are supported.

## Startup Checks

Enabled local storage requires an existing regular SQLite file, the current
product migration/schema/content contract and an existing directory. The adapter
does not create or migrate them. Final symlinks are refused, as are a media root
equal to/above the working directory or a DB/media path under this application's
`public`/`dist` trees (including a symlinked web root). Parent directories remain
trusted operator configuration, not a sandbox against the host owner.

Every stored media row must use the configured logical store ID; changing the ID
is not a provider migration. Unknown media setting names/drivers, incomplete
settings and public media environment settings fail startup rather than falling
back or silently changing stores. Errors do not print private paths or URLs.

Startup checks schema and store identity, not every image's bytes. Run the native
`media-check` command against existing data before enabling/rebinding storage;
for recovered data run `archive-check-recovery` before starting. An existing empty
directory with the same store ID is not proof that all old objects are present.
The generic runtime has its own DB/session initialization; this adapter's refusal
to create missing files is not a guarantee about every other startup component.

Configuration is read once per process startup, not hot-reloaded. A wrong root or
schema stops the reviewed compiled server before readiness. There is no new HTTP
file-serving route, authorization exemption or JSON upload protocol.

## Not An Operator Activation Yet

The product still defaults to the 64 KiB request budget, and media navigation and
upload pages remain inactive. The HTTP fixture alone raises its budget and
installs its protected media proof route. Explicit operator activation, upload
budget/quotas, safe media delivery, thumbnails, attachments and service/traffic
management remain separate gates. New-only private recovery configuration is
available through `config-recovery`; it does not activate the service.

The full isolated HTTP/browser/recovery test now exercises the product hook,
including recovered-only writes, disabled authorized uploads, invalid-root startup
refusal and return to the preserved source. Unit tests cover scoped fallback,
same-DB persistence, invalid/partial/public configuration, schema/store mismatch,
static roots and Unix symlink refusal. No npm dependency was added.
