# Foundry Appearance V0

Legura's first appearance pilot uses Axonyx UI 0.0.85 with Silver + Light +
Classic. The application layout owns the boundary:

```html
<div class="legura-app" data-foundry="silver"
     data-foundry-style="classic" data-foundry-mode="light">
  <!-- Application content -->
</div>
```

Package CSS owns surfaces, contrast, controls and overlays. Legura CSS keeps
application layout, readable content widths and accessible focus/target sizes;
it does not duplicate palette recipes. The existing SVG logo/favicon are kept.
The application opts out of scaffold page padding so its own light canvas
covers the viewport. Form height tokens preserve 44px targets and the scoped
input typography override preserves 16px mobile text.

This pilot deliberately fixes the appearance. It is not a user theme preference,
an OS-theme detector, a persistent light/dark picker or a CMS theme installer.
The existing Theme head directive has a Silver fallback, but saved head palette
metadata does not change this explicit Foundry boundary. A unified preference
picker needs a separate contract rather than two competing sources of truth.

Auth browser acceptance verifies the effective light color scheme and loaded
logo alongside setup/login flows with JavaScript both enabled and disabled.
UI package checks cover representative menus, dialogs, drawers, popovers,
tooltips, toasts and native form controls at desktop/mobile sizes. This is not
a WCAG certification or a full specialized-industrial-component audit.
