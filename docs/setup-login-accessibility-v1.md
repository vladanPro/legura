# Setup / Login Accessibility V1

Scoped Chromium proof, not a WCAG certification, security audit or mobile-device
compatibility guarantee. The pilot remains English-only and development-only.

## Implemented

- One level-one page heading and named form per setup/login page.
- One main landmark supplied by Axonyx; the layout uses a non-landmark content
  wrapper instead of nesting another main. The keyboard skip link targets that
  focusable content wrapper and appears only when focused.
- Associated labels, required fields, credential autocomplete and an email
  keyboard hint. Password/setup-token controls remain masked.
- Existing field errors have unique IDs, polite live regions and explicit
  aria-describedby associations. Published runtime behavior supplies
  aria-invalid after field validation; no application JavaScript was added.
- Setup explains password length and distinguishes the server-owner setup token
  from the administrator password. Server validation remains authoritative;
  the browser is not the authentication boundary.
- Visible link/button keyboard focus, at least 44px form-control height and
  16px input text, keeping the existing Foundry look and palette.

## Acceptance

```powershell
pwsh ./scripts/smoke-auth.ps1 -Mode javascript -Port 3941
pwsh ./scripts/smoke-auth.ps1 -Mode native -Port 3941
```

Both modes use disposable migrated SQLite fixtures and published CLI 0.6.9,
runtime/core 0.6.6 and UI 0.0.84. Keyboard tests prove skip navigation, logical
field tab order, visible submit focus, Enter submission, required-input and
malformed-email browser rejection without requests, and corrected server 422
submission. Native retries retain only allowlisted site/email values, never
password or setup token. Tests retain setup/login/logout, private cookie,
reload/persistence, page identity and console-health checks.

Each setup/login initial and error state is captured at 320px and 390px, with
no horizontal overflow. Screenshots are saved outside the repository under the
system temporary directory, legura-auth-qa/javascript and legura-auth-qa/native.
Desktop setup and mobile admin screenshots remain part of that same test.
Browser plugin not available; the existing Playwright fixture is used.

## Remaining Boundaries

No manual NVDA/VoiceOver testing, contrast audit, real iOS/Android soft-keyboard
or password-manager validation, high-zoom acceptance, Firefox or WebKit proof.
Automatic focus transfer to server validation errors is not implemented; errors
are associated with fields and announced through polite live regions in the JS
path, but a no-JS full navigation is not proof of a spoken announcement.
HTTP guard failures and 429/413 recovery remain separate product UX work.

Framework follow-up: runtime issue #242 records a compiled slot lexical-binding
collision. With caller data named title, Card's default title parameter shadows
the h1 slot expression. Setup uses setupTitle to avoid it. This slice does not
claim to fix the renderer or prove preview parity for that bug.
