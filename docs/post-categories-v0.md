# Post Categories V0

## Scope And Delivery

This is a development-source feature, not part of the published 0.0.3 Pilot 1
package. Work is delivered in separate reviewed steps:

1. Schema and integrity contract (implemented).
2. Administrator category management with native form validation (this step).
3. Editor/Admin post assignments and administration filtering.
4. Public category routes showing only published, non-trashed posts.
5. Compiled HTTP and Aegis browser acceptance of the complete flow.

The administrator's Content sidebar now includes Categories. The list supports
literal name/slug search, stable ten-row pagination, assignment counts and links
to edit or confirm deletion. The create/edit forms share Foundry fields and
server validation; invalid fields are retained for retry. Display names are
escaped, not treated as HTML. Slugs can be edited before public category URLs
are introduced; URL stability must be decided with that later step.

Deletion requires typing the current slug on a separate confirmation screen.
The delete predicate also matches that slug, so a concurrent rename cannot
silently confirm a different value. Cancellation is a link, not a mutation.
Administrator checks protect every loader/action, independently of navigation;
Editors, Subscribers and anonymous visitors cannot manage categories.
Names and slugs are checked before writes, with database uniqueness as the
final concurrent-write boundary. Category editing is last-write-wins; there is
no multi-editor revision/conflict UI.

There is no post-assignment UI, post category filter or public category endpoint
yet. No installed database is migrated by this PR, and no new package or release
is published.

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
Later form validation must report these constraints before writing; the SQL
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
The product version remains unchanged during development; versioning and
compatibility must be reviewed before the next native-package release.

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
deletion of an attached category without changing the post. Only that disposable
fixture seeds assignments directly until post assignment actions exist.
The Aegis `-PostsTableAegis` gate includes category validation retry, search,
editing, slug-confirmed deletion and 320/390-pixel captures, in JS and no-JS modes.
Browser execution must pass in CI before merging this management step.
