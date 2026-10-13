# Native Content HTTP Proof V0

This is a disposable CMS acceptance fixture, not active rich-text integration.
`scripts/smoke-auth.ps1 -Content` copies tracked CMS sources into a fresh GUID-named
temporary directory, migrates its own SQLite database, adds the prototype content
columns, and installs test-only native registration, page, loader, action and API
route. The real compiled application handles requests on loopback port 3946.
Operator environment values are restored and the child server is stopped before
the generated directory is removed. The operator's database is never copied.

Application native registration and member lowering are now published. CI uses
CLI 0.6.12 and runtime 0.6.8 from crates.io without source overrides. The editor
remains a pinned Git dependency. Optional source overrides apply only inside
the fixture, never to the operator application.

```powershell
cargo install cargo-axonyx --version 0.6.12 --locked
pwsh -File scripts/smoke-auth.ps1 -Content -Port 3946
```

The fixture uses the existing CMS setup, password hashing, signed sessions,
administrator database lookup and runtime CSRF boundary, not a bearer test marker.
Its action validates through `Native.call` before a single parameterized insert.
Rich body is derived from the document; plain text is literal and has no document.
The prototype database constraints remain a second boundary. This does not prove
a multi-operation transaction or concurrent rich editing.

Acceptance checks cover rejected anonymous requests, missing/forged CSRF proofs,
foreign origin, recoverable invalid input, duplicate-write rejection, stored columns,
restart persistence and revoked identity. Test-only readback rehydrates `PostContent`
from stored values and returns its escaped/fixed-markup HTML inside JSON; it is
not a public raw-HTML rendering API. No editor JavaScript lifecycle, native retry
retention, active migration, installable release or browser editing is implied.

The independent Registry Compiled Auth CI job keeps verifying the normal published
application path. Content mode cannot be combined with schema export, browser
mode or Posts mode, so its experimental schema cannot overwrite source contracts.

Local Windows acceptance passed on 2026-10-11, including revoked-identity denial
after the normal auth lifecycle. The nine product library tests also passed.
Linux CI remains required before merging this proof into dev.
