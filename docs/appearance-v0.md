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

The header now offers an accessible Light/Dark select when JavaScript is enabled.
It stores only `light` or `dark` under `legura-appearance-mode`, independently
from palette and style. A blocking external head script applies the saved mode
as the parser inserts the boundary, before its first paint. It also synchronizes
other tabs and survives fragment replacement. Storage failures fall back to Light
and allow an in-memory selection. Without JS, Light remains usable and the inert
picker stays hidden. This is not an OS-theme detector or a CMS theme installer.
The existing Theme head directive has a Silver fallback, but saved head palette
metadata does not change this explicit Foundry boundary. A unified preference
picker keeps this separate mode contract rather than competing palette state.

Auth browser acceptance verifies the effective light color scheme and loaded
logo alongside setup/login flows with JavaScript both enabled and disabled.
UI package checks cover representative menus, dialogs, drawers, popovers,
tooltips, toasts and native form controls at desktop/mobile sizes. This is not
a WCAG certification or a full specialized-industrial-component audit.
