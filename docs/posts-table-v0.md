# Posts Table V0

The administration posts list composes registry Foundry Table, TableCaption,
TableHead, TableBody, TableRow, TableHeaderCell and TableCell components.
Title/slug, status and an Edit link are rendered from the existing guarded
`loadAdminPosts` result. Public posts retain their existing card list.

The caption names the table. Column headers have column scope; titles are row
headers. Each Edit link has the post title in its accessible name. Empty
installations show the existing No posts yet card without an empty table.

Small viewports scroll inside a named, keyboard-focusable table region, not
the entire page. Native browser scrolling works without JS. Application CSS
owns minimum table width and readable title wrapping; package CSS owns colors
and borders. No new client script, database schema or query is introduced.

The list retains its existing title ordering and loads all posts. Search,
filtering, pagination and bulk operations are not part of this slice. This is
not a scalability or cross-browser accessibility claim.

Acceptance repeats isolated compiled posts flows in JS and native modes and
asserts semantic headers, edit labels, empty state, 44px action height, 320px
overflow containment and keyboard scrolling. Required Linux registry CI is
the dev merge gate. Browser plugin skill is absent; repository Playwright
fixtures are used. No production database is used.
