# Browser Auth Acceptance

This test runs the real compiled server with isolated SQLite and random secrets.
It uses Chromium, not mocked API responses. Node/Playwright are test tools only;
Legura still runs as a native Axonyx server.

## Discovered Release Gaps

CLI 0.6.7/core 0.6.4 loses an action's explicit redirect in compiled responses.
Runtime 0.6.3 omits hidden CSRF proof when rerendering a native POST 422 form.
Earlier HTTP acceptance did not assert redirect Location or retry the actual
browser-rendered form. Neither failure is a reason to remove authorization or
CSRF checks.

Runtime PR #233 fixes both boundaries; framework PR #330 adds native and patch
HTTP redirect regressions. Browser JS and no-JS acceptance passed locally with
source CLI/runtime. Core 0.6.5, runtime 0.6.4, CLI 0.6.8 and scaffold 0.6.6 are
now published after green preparation and dev-to-main release CI. This branch
pins the fixed runtime and installs CLI 0.6.8 in CI. Isolated HTTP and Chromium
JS/no-JS acceptance passed on Windows on 2026-10-09 with registry packages only,
without ToolManifest or RuntimeSource. Linux CI must pass before dev integration.

Tested browser URL: http://127.0.0.1:3941 (separate disposable fixtures).
Viewports: desktop 1280x900 setup and mobile 390x844 admin/login.
Browser plugin was not available; the repository Playwright tests exercised
real headless Chromium. Page identity, rendered content, validation/navigation,
session persistence and unexpected browser errors are asserted. Mobile admin
screenshots were inspected. No full browser matrix or accessibility audit is
claimed.

## Run

```powershell
npm ci
npx playwright install chromium
pwsh -File scripts/smoke-auth.ps1 -Mode javascript
pwsh -File scripts/smoke-auth.ps1 -Mode native
```

For upstream regression work only, optional source overrides are available:

```powershell
pwsh -File scripts/smoke-auth.ps1 -Mode javascript -ToolManifest ../axonyx-framework/Cargo.toml -RuntimeSource ../axonyx-framework/vendor/axonyx-runtime/crates/axonyx-runtime
pwsh -File scripts/smoke-auth.ps1 -Mode native -ToolManifest ../axonyx-framework/Cargo.toml -RuntimeSource ../axonyx-framework/vendor/axonyx-runtime/crates/axonyx-runtime
```

Overrides modify only a disposable fixture. Each run migrates and pulls its own
schema, starts its own process on port 3941, and restores environment variables.
Run sequentially: compiled fixtures share the Cargo target binary. Never use
the test against a public server or production database.

## Coverage

- Anonymous admin and claimed setup return 403.
- Setup fields are label-addressable; invalid passwords show field feedback.
- Native validation retains only public values and clears password/setup token.
- Correct setup navigates to admin; reload preserves the trusted session.
- Logout navigates to login; protected reads reject the revoked session.
- Wrong passwords show a generic error; correct credentials navigate to admin.
- Desktop setup and mobile admin screenshots are stored in ignored test-results.
- Unexpected page errors and non-network console errors fail JS acceptance.

Screenshots contain only disposable fixture identities, never real users or
configuration. This is Chromium flow acceptance, not a complete accessibility
audit, browser compatibility matrix or production security certification.
