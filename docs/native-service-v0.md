# Native Service V0 (Linux example)

The extracted Legura package runs without Cargo, Node or PowerShell. The
`legura-server` executable reads `.env` and `dist/` relative to its working
directory. The archive does not install a service, reverse proxy or TLS.

For a **new, disposable or operator-prepared** installation, keep the package
at `/opt/legura/release` and SQLite/media at `/var/lib/legura`. Before starting,
run `legura-maintenance verify-package` on the fresh extraction, create the
database and HTTPS-mode configuration as described in
[Native Package V0](native-package-v0.md), and give the dedicated `legura`
service identity read access to the package and `.env` plus write access to the
private data directory. The `.env` must remain private. Do not apply this
procedure to an existing installation or overwrite its configuration/data.

An operator can adapt this example for `/etc/systemd/system/legura.service`:

```ini
[Unit]
Description=Legura CMS
After=network.target

[Service]
Type=simple
User=legura
Group=legura
WorkingDirectory=/opt/legura/release
ExecStart=/opt/legura/release/legura-server
Environment=AXONYX_HOST=127.0.0.1
Environment=AXONYX_PORT=3940
Restart=on-failure
RestartSec=5s
NoNewPrivileges=true
ProtectSystem=strict
ReadWritePaths=/var/lib/legura

[Install]
WantedBy=multi-user.target
```

Change the absolute paths, identity and port together if the installation uses
different values. `ReadWritePaths` is not a substitute for filesystem ownership
and must include every writable data/media directory in `.env`. Keep the
listener on loopback; HTTPS and forwarding are the reverse proxy's job. The
operator must validate the unit (`systemd-analyze verify`), check readiness at
`/__axonyx/ready`, inspect service logs and test setup/login through the actual
proxy before admitting traffic. Do not put secrets in `Environment=` lines.

CI currently proves direct native binary startup, setup and Secure/HttpOnly
cookies on an isolated package. It does **not** install this unit, exercise
systemd restart behavior, prove TLS or certify production hosting. Upgrades,
rollback and migration/service coordination remain separate work.
