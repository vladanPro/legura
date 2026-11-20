# Post Categories V0

## Scope And Delivery

This feature is integrated into dev and included in the 0.0.4 Pilot 1 candidate,
not the published 0.0.3 Pilot 1 package. Work was delivered in separate steps:

1. Schema and integrity contract (implemented).
2. Administrator category management with native form validation (implemented).
3. Editor/Admin post assignments and administration post filtering (implemented).
4. Public category routes showing only published, non-trashed posts (implemented).
5. Compiled HTTP and Aegis browser acceptance of the complete flow (passed).

[Full registry CI](https://github.com/vladanPro/legura/actions/runs/44705590774)
passed for PR #172, including anonymous public archives and both browser modes.
Release preparation adds a separate 0.0.3-to-0.0.4 native-package upgrade gate;
that gate is not implied by source/browser acceptance.

The administrator's Content sidebar now includes Categories. The list supports
literal name/slug search, stable ten-row pagination, assignment counts and links
to edit or confirm deletion. The create/edit forms share Foundry fields and
server validation; invalid fields are retained for retry. Display names are
escaped, not treated as HTML. Slug editing changes the public category address;
the edit screen warns that old URLs return 404 and no redirect alias is created.

Deletion requires typing the current slug on a separate confirmation screen.
The delete predicate also matches that slug, so a concurrent rename cannot
silently confirm a different value. Cancellation is a link, not a mutation.
Administrator checks protect every loader/action, independently of navigation;
Editors, Subscribers and anonymous visitors cannot manage categories.
Names and slugs are checked before writes, with database uniqueness as the
final concurrent-write boundary. Category editing is last-write-wins; there is
no multi-editor revision/conflict UI.

## Saved Post Assignments

After saving a post, open **Edit post > Categories > Choose categories**.
Administrators and Editors can add several existing categories or remove an
individual assignment. Each Add/Remove submits and saves immediately; this is
not a batch of unsaved checkboxes. The separate page avoids submitting an
unfinished rich-text document or coupling content saves to category saves.
Save unsaved post edits before leaving the editor. A new post must be saved
before its categories can be assigned.

Tooling gate: CLI 0.8.9 quotes a redirect URL returned by `Native.call`.
Published CLI 0.8.10 adopts compiler core 0.8.4 and fixes this issue through
[runtime PR #297](https://github.com/vladanPro/axonyx-runtime/pull/297).
CI pins 0.8.10. The isolated registry HTTP and Aegis gates must pass on this
published toolchain before merging; the earlier source-CLI proof alone is not
registry acceptance.

The picker has literal category name/slug search and ten-row pagination. Search
and page are preserved after each mutation. Its assigned-total includes links
outside the current search/page. Editors do not receive category-management
links. Only administrators can create, edit or delete categories themselves.

A Rust native service checks the actor's current active content capability,
the non-trashed post and the category in an immediate SQLite transaction.
Actor identity comes from the server guard, not an action parameter. A repeated
Add/Remove is idempotent and affects only that `(post, category)` pair. Separate
choices are not overwritten by another writer. Missing categories produce a
validation error; missing/trashed posts are not editable. No content,
publication status or other post's assignments change.

## Post List Filter

Admin and Editor can choose one category in **Posts > Filter category**.
The default **All categories** also includes uncategorized posts. A selected
category is combined with title search and collection/status before both
counting and ten-row pagination. The query uses `EXISTS`, so assigning additional
categories to a post does not duplicate it in the list or inflate counts.

Next/Previous retain category, status and title search. Submitting the filter
form starts at the first matching page; Reset filters clears all three. The
existing Posts/Trash collection tabs remain shortcuts that reset other filters;
choose Trash in the status selector to combine it with a category.

An unknown or deleted category stays selected as **Unavailable category**, with
zero results and a reset hint. It never silently falls back to all posts.
Category IDs longer than the schema's 128-character limit return not-found.
Search remains literal and parameter-bound; SQLite's built-in case folding is
ASCII-only, not locale-aware Unicode search. The native dropdown lists all
categories; remote searchable category selection is not included in V0.

## Public Catalog And Archives

`/categories` lists only categories with at least one published, non-trashed
post. `/categories/:slug` serves that category's published post titles/links,
with ten-row stable pagination. Counts and rows use the same publication
predicates; no draft/Trash titles, bodies, category names, internal IDs or
account metadata are included in the public loader projection.

Empty, draft-only, Trash-only, missing and deleted categories return the same
public 404. A category disappears from the catalog when its last published
post is unpublished or trashed. Restoring a post to Draft does not republish it.
Requests are served dynamically with no-store headers, not persisted as static
catalog HTML. Separate reads do not promise a transactionally frozen snapshot
during concurrent editorial changes.

The published-post list links to Browse categories. Published stories expose
their current category links; saved private previews do not gain public links
from this change. Successful catalog/archive pages reuse the selected public
theme, color mode, palette, navigation and identity. English/French interface
labels and visitor locale survive archive links and native GET pagination;
content/category names keep the separately configured content language.
Fallback pages follow the existing public-story pattern: a static English,
default-public frame without database access or private error details.

The shared `PublicPagination` component uses native GET forms. No new client
JavaScript or automatic navigation-menu insertion is introduced. Slug aliases,
SEO metadata, category-specific templates and APIs remain outside V0. No
installed database is migrated by this PR, and no package/release is published.

## Data Model

Migration `20261119000000_020` adds:

- `legura_categories`: stable ID, display name and unique URL slug.
- `legura_post_categories`: unique `(post_id, category_id)` assignments.

A post can have zero, one or many categories. Categories can be shared across
posts. Existing posts stay uncategorized; this migration does not create a
default category or change their content, publication state or authorship.
Tags, nested categories, category templates and automatic menu insertion are
outside V0.

Names have 1-80 characters, no outer ASCII spaces, NUL or line breaks. Slugs
have 1-120 lowercase ASCII letters/digits/hyphens, begin and end with a letter
or digit, and are unique across categories. Display names need not be unique.
Forms report these constraints before writing; the SQL
constraints also protect non-UI writers.

## Deletion And Trash

Deleting a category removes its assignments only. It must not delete, unpublish
or edit any post. Permanently deleting a post removes its assignments but keeps
the categories and assignments belonging to other posts.

Trashing a post retains assignments for a possible restore. Restoring does not
publish the post. The later public query must explicitly require both
`status = 'published'` and `trashed = 0`; an assignment alone is not a visibility
grant.

Foreign keys and fallback triggers protect parent references. The triggers are
needed because not every framework connection enables SQLite foreign keys.
They reject missing parents and changes to an attached parent's ID, and clean
assignments on deletion with foreign keys either on or off.

## Migration And Recovery

The embedded maintenance contract includes migration 020. Historical migration
files are unchanged. Current-schema verification expects the new tables and
triggers; an older installation must follow the normal backup and migration
rehearsal procedure before activation with this source build.

Rollback refuses to discard any categories or assignments, including unused
categories. An empty category schema can be rolled back transactionally without
removing posts. Do not manually delete editorial data to force a downgrade.
The 0.0.4 release candidate adds an explicit historical-backup reader policy;
package compatibility and upgrade rehearsal remain gates before publication.

## Acceptance

Run `cargo test --locked --test post-categories`. It covers many-to-many
assignments, duplicates, constraints, missing references, parent-ID changes,
delete behavior with foreign keys on/off, Trash preservation, legacy post
preservation and lossless/refused rollback. Linux and Windows CI run this test.
Library and package/recovery suites must also pass before this schema foundation
is merged. Schema tests do not prove HTTP authorization or public visibility;
those require the later route and browser gates.

`pwsh -File scripts/smoke-auth.ps1 -Users` now also checks category CRUD, 403/404,
CSRF, invalid/duplicate slugs, escaped labels, literal search, page clamping and
deletion of an attached category without changing the post. Direct SQL seeding
is limited to disposable fixtures; separate HTTP checks exercise assignment actions.
The Aegis `-PostsTableAegis` gate includes category validation retry, search,
editing, slug-confirmed deletion and 320/390-pixel captures, in JS and no-JS modes.
Browser execution must pass in CI before merging this management step.

The same `-Users` fixture now proves Admin/Editor assignment, repeated Add/Remove,
independent links/posts, retained search/page, CSRF, missing/trashed resources and
denial after an Editor is demoted. Rust tests check those transactional service
boundaries and SQL-failure rollback. The Aegis category flow opens the saved
post picker, adds/removes a category, reloads saved state, checks search retention
and captures 320/390-pixel layouts with page scripts on and off. These browser
checks must pass before merging the assignment step.

The `-Users` fixture also proves post filtering for Admin and Editor with more
than ten matching posts, multiple assignments, draft/published/Trash combinations,
literal percent/underscore/SQL-looking input, final-page clamping and retained
category/status/search. Unknown and deleted category filters must remain empty.
Subscriber/anonymous access is denied. Aegis exercises the combined category,
status and title controls, reset and 320/390-pixel layouts in both browser modes.

The public HTTP fixture checks anonymous catalog/archive reads, more than ten
visible categories/posts, safe projections and escaped names, no static catalog,
no-store caching, French controls and language retention, private/empty 404s,
live unpublish/Trash/restore, slug edits and deletion without deleting posts.
Its temporary appearance/language settings are restored afterward. Aegis uses
a separate anonymous browser session to prove discovery from published posts,
draft-only 404, publish/read/story links, unpublish/404 and 320/390-pixel public
layouts with JavaScript on and off. Browser execution must pass before merging.
