# Posts Listing V0

The administration list has All / Draft / Published filtering and server-side
pagination, ten rows per page. Filtering submits a native GET form and resets
the page. Previous/Next links preserve the normalized status. No new client
script or browser-side full-list cache is used.

The guarded loader returns `AdminPostPage`: posts, normalized status, page and
matching total. Existing administrator checks run before database reads.
Queries use bound parameters, with title/id ordering for deterministic ties.
Invalid, empty, negative or non-integer pages become page one; out-of-range
pages clamp to the last available page. Unknown statuses become All.

This is a SQLite pilot. The parameterized `db.query()` escape hatch is used
because fluent limit/offset currently accept literals only. SQLite GLOB and
scalar MIN/MAX are not a portable Postgres/MySQL paginator. Count and row reads
are separate statements, not a guaranteed transactional snapshot.

## Tooling Dependency

This pilot requires cargo-axonyx 0.6.10 or later, with the framework
compiled-query-loader-arguments fix. Older CLIs cannot compile the
`query.status ?? "all"` loader argument into the page route. The workflow pins
0.6.10; registry CI must pass before this pilot is merged.

Local source acceptance uses registry runtime/UI and the source CLI:

```powershell
./scripts/smoke-auth.ps1 -ToolManifest ../axonyx-framework/Cargo.toml -Posts -Mode javascript
./scripts/smoke-auth.ps1 -ToolManifest ../axonyx-framework/Cargo.toml -Posts -Mode native
```

Browser fixtures create eleven posts through the actual editor. They cover
page boundaries, reloads, filtering, invalid and oversized URL values, encoded
keys, duplicate parameters, unauthorized access and private drafts. JS mode
also checks the compiled data refresh endpoint with a preserved query target.
The existing CRUD and validation retry checks remain in both modes.
