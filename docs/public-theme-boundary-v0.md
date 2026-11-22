# Public Theme Boundary V0

Legura is one Axonyx application with separate presentation surfaces. The
public home page and `/posts` subtree render inside `PublicFrame`; setup,
login and `/admin` render inside `BackofficeFrame`. Both frames share the root
head/assets, database and route graph, but own separate Foundry boundaries.

The public frame ships with **Classic** (Silver, centered reading layout),
**Alloy** (Bronze, wider editorial layout) and **Legura** (Bronze, paper
surfaces in Light mode, serif headings, and public navigation). Classic and
Alloy keep the historical `default` and `editorial` IDs. The selected theme is stored in the
singleton `legura_public_theme` database row. An administrator selects it at
`/admin/appearance` using **Activate**; the choice takes effect on the next
public request and survives a restart. Explicit public mode and palette
settings stay in place. Before downgrading to a release without Legura,
activate Classic or Alloy; the downgrade rejects an active Legura bundle.
The backoffice keeps its Silver + Classic frame and
its independent Light/Dark preference. The appearance script and picker target
only `.legura-app`, never `.legura-public`. A saved backoffice Dark preference
must not change the public frame, even in the same browser session.

This is one operator-facing theme choice that bundles layout and Foundry
tokens, not merely a palette switch. It is not a theme installer: there is no
upload, marketplace, arbitrary CSS execution or public theme switcher. Public
not-found and error pages use the safe default public frame rather than loading
the database from an error boundary. The global fallbacks still use the stable
backoffice frame. Future installable themes must preserve posts, authentication,
admin actions and their data contracts.

The isolated compiled `-Theme` HTTP proof covers authorization, CSRF,
allowlisted selection, public render, restart persistence and restoration.
The isolated compiled posts HTTP proof still covers create/edit/publish,
private drafts, deletion and restart. Chromium posts/browser and auth/browser
proofs run with JavaScript enabled and disabled. The posts browser also asserts
the public boundary on `/` and `/posts`, checks it has no backoffice controls,
then switches admin Dark in the same browser context and verifies public Light
is unchanged. An unpublished story must return a branded public 404 without
backoffice controls. This is not an operator theme-selection or visual-design audit.
