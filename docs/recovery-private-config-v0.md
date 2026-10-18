# Private Recovery Configuration V0

The native maintenance tool can prepare private settings for a verified,
inactive recovery. It does not switch a running CMS or install a service.

```text
legura-maintenance config-recovery <archive.legura> <recovery-dir> <package-dir> https
```

Both directories must already exist. Use a trusted, private package directory
containing the application assets, outside `public` and `dist`. The command runs
the same read-only archive/recovery compatibility check as `archive-check-recovery`.
It then validates the recovered DB/media paths through the product storage adapter.
An existing `.env`, including an empty file or symlink, is never replaced.

## Generated Settings

The NEW `<package-dir>/.env` contains absolute SQLite/media paths, the verified
logical store ID, and independent random session/setup secrets. Old configuration
and ambient process secrets are not copied. Secret values are not printed.
The recovered database, archive and receipt remain unchanged.

Unix creation requests mode 0600; on Windows restrict the package directory's
ACLs to the service identity before running the command. Paths containing quotes
or control characters are refused rather than relying on unsupported dotenv
escaping. Parent directories must remain trusted throughout generation/startup.

## Startup Remains Explicit

Use `https` for a service behind correctly configured HTTPS. This sets secure
cookies; it does not provision TLS, configure a proxy or bind the listener.
`local-http` disables secure cookies and is only for a deliberately loopback-bound
local proof. The operator must configure that host binding separately.

Start the compatible compiled application with `<package-dir>` as its working
directory. Its normal AxEnv loader reads `.env`; process variables take precedence,
so clear conflicting overrides first. Keep the old installation stopped before
an actual cutover, retain its configuration for rollback, and expect existing
sessions to be invalidated by the new key. Rollback does not merge later writes.

The required startup hook is published in `cargo-axonyx 0.6.15`; the isolated
Media CI uses that exact registry CLI. Legura itself is still a product pilot.
No operator activation, automatic update, public media delivery or historical
schema upgrade is implied.

## Acceptance

Rust tests cover independent keys, both cookie modes, no-clobber behavior,
invalid recoveries, web-root rejection and dotenv injection refusal. The isolated
Media HTTP smoke boots a recovered package from the actual generated `.env`,
rejects old sessions/CSRF, logs in afresh, uploads/restarts and returns to the
unchanged source. It does not read or modify the developer's running CMS.
