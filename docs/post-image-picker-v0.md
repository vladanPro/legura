# Private Post Image Picker V0

Open a saved post, then choose **Choose featured image**. Save unsaved content
first: image settings have their own screen and independent mutations.
The picker at `/admin/posts/:id/image` uses Foundry Card/Grid/Button/Input and
native labeled radio inputs. It adds no handwritten client-side JavaScript and
works with ordinary HTML forms when JavaScript is disabled.

## Behavior

- Ten library choices per page, using existing bounded media pagination.
- A private current-image preview and post-specific alternative text.
- Keep the current image to edit its alt text, or select a thumbnail and save.
- A separate remove action clears only the association, never library bytes.
- Content, slug and publication status are not part of either image mutation.
- Empty library links to upload. Disabled media hides editor controls and denies
  the picker and mutations; it never delegates to raw filesystem storage.

The image is NOT public yet, even on a published post. Its preview uses the
existing current-administrator `/api/media/:id` boundary. Public delivery through
a current published-post association is the next independent vertical slice.
No inline rich-text images, galleries, thumbnails, deletion UI or CDN are added.

## Authority

SetPostImage and RemovePostImage call the existing current-administrator loader
after normal action CSRF/session admission. They derive administrator_id on the
server; neither action declares a subject/administrator parameter. Internal
native callbacks are not directly exposed as HTTP endpoints. They additionally
recheck current DB authority in the transactional foundation service.

The target post is the submitted ID, as in existing UpdatePost/DeletePost:
administrators can manage all posts. The route URL is not a separate ownership
grant or a promised route/body-ID equality constraint. Missing targets return
404, denied authority/disabled media return 403, invalid selection/alt return
422, and unavailable/corrupt storage fails safely without replacing the choice.

Alternative text is literal, escaped data, not trusted markup. The 300-scalar
server bound remains authoritative; a browser maxlength is only an input aid.
Selection does not change global media visibility or cache authorization.

## Acceptance

```sh
pwsh -File scripts/smoke-auth.ps1 -Media -ScopedMedia -MediaBrowser -Port 3973
```

The existing required media CI job runs the new HTTP/browser proof. It exercises
published CLI/runtime 0.7.1. Supply the installed CLI executable through
`-ToolPath` when it is not the global `cargo ax` version. It exercises
anonymous/forged/old/revoked sessions, missing/forged CSRF and wrong origins,
selection/replace/alt/remove, escaped text, missing/corrupt objects, restart
persistence, disabled media and unchanged post content/status. Browser checks
cover JS/no-JS forms, pagination, decoded previews and desktop/mobile overflow.
The archive proof includes legura_post_images in its exact row comparison.

Use a disposable migrated database, never an operator installation. No global
CLI, .env, database, process or production deployment is changed by acceptance.
Required CI, screenshots and actual HTTP results must pass before merge; source
presence is not certification.

## Framework Follow-Ups

Published 0.7.1 does not correctly compile action params references or optional
members of untyped DB records in the attempted forms. Reproducers are tracked in
framework issues 374 and 375. This product uses the existing body-ID admin
action model and explicit SQL COALESCE columns. No generated Rust is hand-edited
and no source-only framework dependency is introduced to hide these defects.
