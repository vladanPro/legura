# Public Theme Boundary V0

Legura is one Axonyx application with separate presentation surfaces. The
public home page and `/posts` subtree render inside `PublicFrame`; setup,
login and `/admin` render inside `BackofficeFrame`. Both frames share the root
head/assets, database and route graph, but own separate Foundry boundaries.

The public frame currently selects the fixed `default` theme, implemented as
Silver + Light + Classic. The backoffice keeps its Silver + Classic frame and
its independent Light/Dark preference. The appearance script and picker target
only `.legura-app`, never `.legura-public`. A saved backoffice Dark preference
must not change the public frame, even in the same browser session.

This is a layout and token boundary, not a theme installer or a CMS setting.
There is no active-theme database row, theme upload, marketplace, user CSS
execution or public theme switcher. The global error and not-found pages still
use the stable backoffice frame; route-specific public fallbacks are follow-up
work. A future public theme may change the public layout/recipes without
changing posts, authentication, admin actions or their data contracts.

The isolated compiled posts HTTP proof still covers create/edit/publish,
private drafts, deletion and restart. Chromium posts/browser and auth/browser
proofs run with JavaScript enabled and disabled. The posts browser also asserts
the public boundary on `/` and `/posts`, checks it has no backoffice controls,
then switches admin Dark in the same browser context and verifies public Light
is unchanged. This is not an operator theme-selection or visual-design audit.
