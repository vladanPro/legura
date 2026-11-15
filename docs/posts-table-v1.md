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

These Playwright tests cover CRUD, administrator guards, title/status filters,
page limits, query normalization and table screenshots. They create their own
temporary configuration and database. No production dependency is added.

To run the sign-in, mobile keyboard-scroll and sign-out smoke without Node, start a local
ChromeDriver or EdgeDriver that matches the installed browser:

```powershell
$env:AEGIS_WEBDRIVER_URL = "http://127.0.0.1:9515"
# For EdgeDriver only: $env:AEGIS_TEST_BROWSER = "edge"
./scripts/smoke-auth.ps1 -PostsTableAegis -Port 3961
```

The smoke runner compiles the actual CMS and seeds a draft in a fresh SQLite
database. In both scripting modes, Aegis signs in through the form, verifies
that the table fits the page at 320 px and scrolls within its focusable region,
then signs out and verifies that the session can no longer access the dashboard.
Playwright still covers broader auth behavior, the remaining Posts scenarios
and desktop and mobile screenshots.

This is a package adoption and regression-proof step, not a new grid engine,
interactive server sorting, operator upgrade or CMS production release.
