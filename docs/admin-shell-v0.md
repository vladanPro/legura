# Administration Shell V0

All five administration screens share `app/components/AdminFrame.asx`:
overview, posts, create, edit and delete confirmation. It composes registry
Axonyx UI 0.0.85 AppShell, Sidebar, SidebarHeader, SidebarSection and SidebarItem.
The default content slot keeps page data/actions in their original route scope.

Pages explicitly select `overview` or `posts`; edit/create/delete belong to the
Posts section. Navigation does not depend on client-side URL inference. No new
query, user permissions, storage schema or package release is introduced.

AppShell supplies desktop columns and mobile stacking. The Sidebar starts open
and uses package JS to collapse on mobile. Without JS it stays open and the
application hides the inert toggle. There is still one generated main landmark.
Application CSS owns content padding and width, not Foundry color recipes.

The header logo and Light/Dark preference remain shared across public and admin
routes. Login/setup/public content keep their narrower single-column layout.
This is a navigation/layout pilot, not a complete CMS dashboard or UI audit.

Acceptance includes registry compiled auth/posts flows, active section on all
admin routes, 320/390/1280px overflow, 44px navigation links, mobile toggle and
JS/no-JS usability. Fixtures never use developer or production databases.

Local Windows compiled auth and posts browser acceptance passes in both modes
with source CLI 0.6.9 and crates.io runtime/UI. Browser plugin skill is absent;
the repository Playwright fixtures were used. Required Linux registry CI is the
merge gate; this document does not claim production or cross-browser readiness.
