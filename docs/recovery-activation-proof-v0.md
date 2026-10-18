# Recovery Activation Proof V0

This is an executable acceptance test, not a product activation command or an
automatic CMS updater. It switches only the environment of a disposable test
server and then switches back. It does not stop the developer's running CMS,
change its `.env`, migrate its database or register an operating-system service.

## Reproduce

Use published CLI 0.6.15 containing the native storage startup hook. Install it
in a separate test directory if the developer's global CLI must stay unchanged:

```powershell
cargo install cargo-axonyx --version '=0.6.15' --locked --debug --root <test-tool-dir>
pwsh -File scripts/smoke-auth.ps1 -Media -MediaBrowser -ToolPath <test-tool-dir>/bin/cargo-axonyx -Port 3963
```

The normal media proof creates an isolated SQLite installation, administrator,
real uploaded images, live archive and a new private recovery directory. It
already proves archive integrity and read-only recovery compatibility. The new
step then exercises the same compiled binary against the recovered database and
object directory. It is not a second independently deployed service.

## Stop, Check, Switch, Start

The script checks that every DB/storage/archive/binary path belongs to the
generated fixture. It records the current test environment and stops only its
own process. After it has stopped, it fingerprints source DB/sidecar files and
all source objects, including the intentionally retained orphan. It does not open
or checkpoint the stopped source database while testing the recovery.

The native `archive-check-recovery` command must pass offline before switching
anything. The proof generates independent cryptographic session/setup keys in
memory, selects the recovered DB and media directory together and starts its
own compiled process on the same test port. No secrets are logged or persisted
to a configuration file. Other settings remain confined to the existing local
HTTP fixture; they are not production defaults.

Acceptance checks:

- Setup remains locked; the restored administrator is not recreated.
- Anonymous requests and original session cookies cannot open protected pages.
- Original CSRF/session proof cannot upload an image after key rotation.
- A fresh login works using the recovered password hash and current credentials.
- A new upload writes only to the recovered DB and recovered object directory.
- Fresh recovered-session access and media persist across process restart.
- An intentionally missing storage directory prevents startup before readiness.
- The failed start does not create that directory or fall back to another store.

The new recovery DB changes legitimately after login/upload. The original archive
and historical receipt remain untouched; `archive-check-recovery` is a pre-start
gate and must not be expected to match a database after new writes.

## Return To Source

A `finally` path stops the test recovery process, restores the original test
environment and starts the source again, including when activation/startup fails.
Before source restart, its DB/sidecars and all objects must be byte-for-byte
unchanged. Then the original cookie must work again, the recovered cookie must
fail against the source key, and source media counts must remain unchanged.
The enclosing smoke continues its existing logout/login/throttle/revocation tests.

This rollback is switching back to a preserved source installation, not merging
new writes. The test's recovered-only upload is deliberately left behind in the
disposable recovery directory. A real operator must account for writes accepted
after cutover before any rollback; blindly discarding them is not safe recovery.

## Remaining Product Work

Operator media activation, portable runtime storage configuration, durable private
configuration generation, process/service management, maintenance-mode traffic
admission and an explicit cutover/rollback policy are still separate gates.
There is no `legura activate` command, installer or production upgrade support.
The proof now uses the product native storage adapter with explicit private
media settings and the same application DB URL. Its route, transport budget and
activation orchestration remain test-only; it must not be copied as an unattended
production deployment script. See native-media-config-v0.md.
