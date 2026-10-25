# Posts table: published Foundry UI

The admin posts list uses the native Table, TableCaption, TableHead, TableBody,
TableRow, TableHeaderCell and TableCell components from Cargo axonyx-ui 0.0.87.
It does not use npm assets or a local copy of the package's table markup.

## Server-owned dataset

The authenticated loader counts matching posts and loads at most ten rows.
Title search, exact status filtering, stable title/id ordering and page clamping
remain on the server. GET forms preserve query parameters and work without JS.
Sorting is currently fixed by the loader, not an interactive column control.

Do not wrap this page in the client DataTable component: that component searches
and paginates a complete bounded dataset already present in the HTML. Applying
it to one server page would make search incomplete and add a second paginator.
The existing browser acceptance proves a search can find Listing 11 even when
it is not part of the first ten-row slice, and rejects a client DataTable wrapper.

## Acceptance and scope

Use disposable SQLite fixtures, never an operator installation:

```powershell
./scripts/smoke-auth.ps1 -Posts -Mode javascript -Port 3961
./scripts/smoke-auth.ps1 -Posts -Mode native -Port 3961
```

These tests cover CRUD, administrator guards, title/status filters, page limits,
query normalization and responsive table scrolling. They create their own
temporary configuration and database. Browser QA uses the existing test-only
Playwright dependency; no new production dependency is added.

This is a package adoption and regression-proof step, not a new grid engine,
interactive server sorting, operator upgrade or CMS production release.
