# Media Deletion Service V0

This is an unregistered Rust service, not a shipped administrator button or HTTP
mutation. It introduces no schema migration and changes no running installation.

## Contract

- The caller supplies the verified session subject and startup-owned store/DB.
- Current administrator authority, store ownership and all featured-image
  references are checked inside one SQLite IMMEDIATE transaction.
- Draft and published associations both prevent deletion. No force/cascade option.
- Invalid keys, missing rows and other-store rows never reach object removal.
- Foreign keys remain enabled; failed SQL or commit leaves bytes untouched.
- After successful metadata commit, the separate ObjectRemoval capability removes
  one immutable object. Local storage rejects links/directories and never recurses.
- An already absent file is successful removal. Other filesystem failures return
  CleanupRequired with the object reference, not a claim of complete deletion.

## Crash and Concurrency Boundary

SQLite and the filesystem do not share an atomic transaction. Commit metadata
first: a crash afterwards can leave unreferenced private bytes, not a live DB
reference to bytes deleted by this operation. Such bytes are no longer reachable
through the application read routes. Already admitted reads can finish.

There is no durable cleanup queue or automatic orphan collection in this slice.
The returned cleanup reference is not a persistent retry ticket: a process crash
can lose it. A later explicit operator cleanup policy must recheck references
before removal. Never roll metadata back after a filesystem error and never
reuse an old object key. This follows the existing upload orphan-on-DB-failure
policy rather than pretending both stores can commit together.

All current media references are featured-image associations. Before supporting
inline editor images or other consumers, extend the reference check and tests.
Unsupported direct SQL writers are outside the product service contract.

## Next Integration Gate

Add an administrator-only confirmation flow and POST action with normal session
and CSRF guards. Surface InUse as a useful error and CleanupRequired as an honest
partial-cleanup warning. Prove no-JS/JS behavior, repeated submits, stale forms,
public/private read revocation and restart behavior before exposing deletion.

Tests cover denied identities/keys/stores, draft/published references, SQL and
deferred-commit failure, missing files, cleanup failure and local leaf safety.
CI runs the service tests on Linux and Windows; Unix also checks symlink refusal.
