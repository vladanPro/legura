# Posts Listing V0

The administration list has title search, All / Draft / Published filtering and
server-side pagination, ten rows per page. Filtering submits a native GET form
and resets the page. Previous/Next GET forms preserve the normalized status and
search; the browser encodes their values without custom JavaScript. No new client
script or browser-side full-list cache is used.

The guarded loader returns `AdminPostPage`: posts, normalized status/search, page and
matching total. Existing administrator checks run before database reads.
Queries use bound parameters, with title/id ordering for deterministic ties.
Invalid, empty, negative or non-integer pages become page one; out-of-range
pages clamp to the last available page. Unknown statuses become All.

`q` searches for a literal substring of the title. Leading/trailing ASCII spaces
are trimmed and the server bounds it to 200 SQLite characters, including for
direct URL requests. `%` and `_` are ordinary characters, not SQL wildcards.
SQLite `lower()` provides ASCII case-insensitivity only; this is not full Unicode
case folding or accent-insensitive search. Non-ASCII characters match literally.
Reset filters clears search/status/page. An empty filtered result is distinct
from a new installation with no posts.

This is a SQLite pilot. The parameterized `db.query()` escape hatch is used
because fluent limit/offset currently accept literals only. Normalization and
pagination now use typed Rust helpers registered in `src/native.rs`; the loader
calls them through Native.call after its administrator checks. SQL only counts
and fetches matching rows, with bound limit/offset. The search SQL still uses
SQLite instr/lower, so this is not a Postgres/MySQL adapter. Count and row reads
are separate statements, not a guaranteed transactional snapshot.

## Tooling Dependency

This pilot requires cargo-axonyx 0.6.10 or later, with the framework
compiled-query-loader-arguments fix. Older CLIs cannot compile the
`query.status ?? "all"` loader argument into the page route. The workflow pins
0.6.12; core/runtime 0.6.8 provide the compiled native helper startup. The
listing cleanup requires compiled mode with the registered helpers; an
unconfigured preview fails closed rather than approximating their behavior.

Local source acceptance uses registry runtime/UI and the source CLI:

```powershell
./scripts/smoke-auth.ps1 -ToolManifest ../axonyx-framework/Cargo.toml -Posts -Mode javascript
./scripts/smoke-auth.ps1 -ToolManifest ../axonyx-framework/Cargo.toml -Posts -Mode native
```

Browser fixtures create eleven posts through the actual editor. They cover
page boundaries, reloads, combined search/filtering, literal metacharacters,
escaping, invalid and oversized URL values, encoded keys, duplicate parameters,
unauthorized access and private drafts. JS mode
also checks the compiled data refresh endpoint with a preserved query target.
The existing CRUD and validation retry checks remain in both modes.

Title-search acceptance passed on Windows on 2026-10-10 using published CLI
0.6.10 and registry runtime/UI, without source overrides. Both Chromium modes
cover eleven editor-created posts, search plus status, pagination preservation,
reload, reset, literal `%`/`_` matching, encoded punctuation, duplicate `q`,
server-side length bounds and anonymous rejection. Desktop and 320/390px
screenshots were inspected; form/pagination controls meet the 44px target check.
Browser plugin not available; the repository Playwright runner was used.
These tests use disposable SQLite fixtures, never the developer/live database.
Linux registry CI is the integration gate; local passing tests are not a
production deployment or cross-browser/accessibility certification.

## Typed Pagination Cleanup

The post filters preserve ASCII-space trimming, a 200-Unicode-scalar search
bound and exact draft/published matching. Page parsing accepts only the old
SQLite-compatible ASCII digit prefix, saturates oversized values, then clamps
against the count. SQLite NUL-prefix behavior is covered by differential tests.
The generic paginator uses integer ceil division without `total + size - 1`
overflow, rejecting negative totals or nonpositive page sizes.

`src/pagination.rs` has no database/request access and no new dependency.
Callbacks are statically named, typed and checked by AxNativeRegistry; these
names are not public HTTP endpoints. Protected entrypoints keep authorization.
The fixture native registration composes these helpers with content-proof
callbacks rather than replacing product registrations accidentally.

Local Windows registry acceptance passed: 14 library tests, strict library
Clippy, compiled HTTP/backup/restore, and Chromium JS/no-JS listing flows with
11 real editor-created posts and 320/390/1280px overflow/target checks.
Browser plugin not available; the existing test-only Playwright runner was used.
One initial JS run hit a transient sidebar selection assertion; a repeat passed
without product UI changes. The test now waits for the expected active navigation
element and still requires exactly one active section, with richer diagnostics.
This is not a separate hydration/flicker audit. Remote CI gates integration.
