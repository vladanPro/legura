# Navigation V0

An administrator can create multiple named menus at `/admin/navigation`. Assign
a menu to Header, Footer, both, or neither. Each location holds at most one menu.
An unassigned header retains the current theme's built-in header; an assigned
empty menu does not fall back to built-in links. An empty footer has no menu.

Publishing a page does not add a link. Choose a published page manually, set
its position, and use Visible or Hidden to control that one menu item. Smaller
positions appear first; ties are ordered by the stored page ID. Hidden items
remain editable, and removing an item does not modify its page. A page that
returns to draft is excluded from every public menu without losing its saved
membership or visibility setting. Republishing restores only its visible links.

Menus link to `/pages/<slug>` and use the page's current title. Menu positions
are shared by both built-in public themes and do not affect the admin sidebar.
Navigation V0 has no custom URLs, submenus, menu deletion, or drag-and-drop.

The native package smoke exercises guarded edits, explicit membership, menu
assignment, visibility, order, unpublishing, and persisted links after archive
recovery. This does not migrate a live installation or change the release version.
