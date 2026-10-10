import assert from "node:assert/strict";
import { mkdir } from "node:fs/promises";
import { resolve } from "node:path";
import { randomUUID } from "node:crypto";
import { tmpdir } from "node:os";
import { chromium } from "playwright";

const [baseUrl, mode] = process.argv.slice(2);
const url = new URL(baseUrl);
assert.ok(["127.0.0.1", "localhost"].includes(url.hostname), "Use an isolated local fixture");
assert.ok(["javascript", "native"].includes(mode), "Choose a browser mode");
const setupToken = process.env.AX_SECRET_SETUP_TOKEN;
assert.ok(setupToken, "Fixture owner token is required");
const email = "browser-owner@example.com";
const password = `fixture-password-${randomUUID()}`;
const browser = await chromium.launch();
const context = await browser.newContext({
  javaScriptEnabled: mode === "javascript",
  viewport: { width: 1280, height: 900 },
});
const page = await context.newPage();
const errors = [];
const responses = [];
page.on("response", (response) => {
  const route = new URL(response.url());
  responses.push({ path: route.pathname, status: response.status(), type: response.headers()["content-type"] });
});
page.on("pageerror", (error) => errors.push(error.message));
page.on("console", (message) => {
  // HTTP 422/403 responses are expected validation/access-control evidence.
  if (message.type() === "error" && !message.text().startsWith("Failed to load resource:")) {
    errors.push(message.text());
  }
});
const results = resolve(tmpdir(), "legura-auth-qa", mode);
await mkdir(results, { recursive: true });

async function navigate(path, status = 200) {
  const response = await page.goto(`${baseUrl}${path}`);
  assert.equal(response.status(), status, `Unexpected response for ${path}`);
}

async function submitValidation(button, field, expected) {
  const responsePromise = page.waitForResponse((response) =>
    response.url().includes("/__axonyx/action?") && response.request().method() === "POST"
  );
  await page.getByRole("button", { name: button, exact: true }).focus();
  await page.keyboard.press("Enter");
  const response = await responsePromise;
  assert.equal(response.status(), 422);
  const body = await response.text();
  assert.ok(!body.includes(password) && !body.includes(setupToken), "Response echoed a credential");
  const error = page.locator(`[data-ax-field-error="${field}"]`);
  await error.filter({ hasText: expected }).waitFor({ state: "visible" });
  assert.equal(await error.getAttribute("aria-live"), "polite");
  const input = page.locator(`input[name="${field}"]`);
  assert.equal(await input.getAttribute("aria-invalid"), "true");
  assert.ok((await input.getAttribute("aria-describedby")).split(/\s+/).includes(await error.getAttribute("id")), "Error is not associated with its input");
}

async function submitSuccess(button) {
  const responsePromise = page.waitForResponse((response) =>
    response.url().includes("/__axonyx/action?") && response.request().method() === "POST"
  );
  await page.getByRole("button", { name: button, exact: true }).focus();
  await page.keyboard.press("Enter");
  const response = await responsePromise;
  assert.ok([200, 303].includes(response.status()), `Successful ${button} returned HTTP ${response.status()}`);
}

async function checkForm(heading, fields) {
  const appearance = await page.locator('.legura-app').evaluate(node => {
    const css = getComputedStyle(node);
    return { palette: node.dataset.foundry, style: node.dataset.foundryStyle,
      mode: node.dataset.foundryMode, scheme: css.colorScheme, background: css.backgroundColor };
  });
  assert.deepEqual(appearance, { palette: 'silver', style: 'classic', mode: 'light',
    scheme: 'light', background: 'rgb(243, 245, 247)' });
  assert.equal(await page.locator('.legura-app').evaluate(node => {
    const rect = node.getBoundingClientRect();
    return rect.top === 0 && rect.left === 0 && rect.right === innerWidth;
  }), true, 'Application canvas must cover the viewport without dark scaffold gutters');
  const logo = page.locator('.legura-brand img');
  assert.equal(await logo.getAttribute('src'), '/legura-mark.svg');
  assert.equal(await logo.evaluate(node => node.complete && node.naturalWidth > 0), true);
  assert.equal(await page.getByRole("main").count(), 1, "Duplicate main landmarks");
  assert.equal(await page.locator("html").getAttribute("lang"), "en");
  assert.equal(await page.getByRole("heading", { level: 1, name: heading, exact: true }).count(), 1);
  const form = page.getByRole("form", { name: heading, exact: true });
  assert.equal(await form.count(), 1);
  for (const field of fields) {
    const control = page.locator(`#${field}`);
    assert.equal(await control.getAttribute("name"), field);
    assert.equal(await control.evaluate((input) => input.required), true);
    assert.equal(await control.evaluate((input) => input.labels.length), 1);
    const describedBy = await control.getAttribute("aria-describedby");
    for (const id of (describedBy || "").split(/\s+/).filter(Boolean)) {
      assert.equal(await page.locator(`[id="${id}"]`).count(), 1, `Missing or duplicate description ${id}`);
    }
  }
  assert.equal(await page.locator("#email").getAttribute("inputmode"), "email");
  assert.equal(await page.locator("#email").getAttribute("autocomplete"), "username");
  assert.equal(await page.locator("#password").getAttribute("type"), "password");
}

async function checkKeyboardOrder(fields, button) {
  await page.locator(`#${fields[0]}`).focus();
  for (const field of fields.slice(1)) {
    await page.keyboard.press("Tab");
    assert.equal(await page.locator(`#${field}`).evaluate((node) => node === document.activeElement), true, `Tab did not reach ${field}`);
  }
  await page.keyboard.press("Tab");
  const submit = page.getByRole("button", { name: button, exact: true });
  assert.equal(await submit.evaluate((node) => node === document.activeElement), true);
  assert.ok(await submit.evaluate((node) => {
    const style = getComputedStyle(node);
    return style.outlineStyle !== "none" && Number.parseFloat(style.outlineWidth) >= 2;
  }), "Keyboard submit focus is not visible");
}

async function checkMobile(name) {
  for (const width of [320, 390]) {
    await page.setViewportSize({ width, height: 844 });
    assert.ok(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), `Horizontal overflow at ${width}px`);
    for (const control of await page.locator("form input:not([type=hidden]), form button[type=submit]").all()) {
      assert.ok(await control.evaluate((node) => node.getBoundingClientRect().height >= 44), "Form target is shorter than 44px");
    }
    assert.ok(await page.locator("#email").evaluate((node) => Number.parseFloat(getComputedStyle(node).fontSize) >= 16), "Mobile input text is smaller than 16px");
    await page.screenshot({ path: resolve(results, `${name}-${width}.png`), fullPage: true });
  }
}

function actionCount() {
  return responses.filter((response) => response.path === "/__axonyx/action").length;
}

try {
  await navigate("/admin", 403);
  await navigate("/setup");
  assert.equal(await page.title(), "Legura CMS");
  await page.getByRole("button", { name: "Create installation", exact: true }).waitFor();
  await checkForm("Create your first administrator", ["siteName", "email", "password", "setupToken"]);
  assert.equal(await page.locator("#password").getAttribute("autocomplete"), "new-password");
  await page.keyboard.press("Tab");
  const skip = page.getByRole("link", { name: "Skip to content", exact: true });
  assert.equal(await skip.evaluate((node) => node === document.activeElement), true);
  assert.ok(await skip.evaluate((node) => node.getBoundingClientRect().top >= 0), "Focused skip link is offscreen");
  assert.equal(await skip.evaluate((node) => getComputedStyle(node).clipPath), "none", "Focused skip link is clipped");
  await page.keyboard.press("Enter");
  assert.equal(await page.locator("#main-content").evaluate((node) => node === document.activeElement), true);
  await checkKeyboardOrder(["siteName", "email", "password", "setupToken"], "Create installation");
  const beforeMissing = actionCount();
  await page.keyboard.press("Enter");
  assert.equal(await page.locator("#siteName").evaluate((node) => node === document.activeElement && node.validity.valueMissing), true);
  assert.equal(actionCount(), beforeMissing, "Empty setup sent a request");
  await checkMobile("setup");
  await page.setViewportSize({ width: 1280, height: 900 });
  assert.ok(!(await page.content()).includes(setupToken), "Owner token exposed in markup");
  await page.screenshot({ path: resolve(results, `setup-${mode}-desktop.png`), fullPage: true });

  await page.getByLabel("Site name", { exact: true }).fill("Browser fixture");
  await page.getByLabel("Administrator email", { exact: true }).fill(email);
  await page.getByLabel("Password", { exact: true }).fill("short");
  await page.getByLabel("Setup token", { exact: true }).fill(setupToken);
  await submitValidation("Create installation", "password", "Use at least 15 characters.");
  assert.equal(await page.getByLabel("Site name", { exact: true }).inputValue(), "Browser fixture");
  assert.equal(await page.getByLabel("Administrator email", { exact: true }).inputValue(), email);
  if (mode === "native") {
    assert.equal(await page.getByLabel("Password", { exact: true }).inputValue(), "");
    assert.equal(await page.getByLabel("Setup token", { exact: true }).inputValue(), "");
  }
  await checkMobile("setup-validation");
  await page.getByLabel("Password", { exact: true }).fill(password);
  await page.getByLabel("Setup token", { exact: true }).fill(setupToken);
  await submitSuccess("Create installation");
  await page.waitForURL(`${baseUrl}/admin`);
  await page.locator("p").filter({ hasText: email }).waitFor();
  await page.reload();
  await page.locator("p").filter({ hasText: email }).waitFor();
  const session = (await context.cookies()).filter((cookie) => cookie.httpOnly);
  assert.ok(session.length > 0, "No private session cookie after setup");

  await page.setViewportSize({ width: 390, height: 844 });
  if (mode === "javascript") {
    assert.ok(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), "Mobile horizontal overflow");
  }
  await page.getByRole("button", { name: "Sign out", exact: true }).waitFor({ state: "visible" });
  await page.screenshot({ path: resolve(results, `admin-${mode}-mobile.png`), fullPage: true });
  await submitSuccess("Sign out");
  await page.waitForURL(`${baseUrl}/login`);
  await navigate("/admin", 403);
  await navigate("/setup", 403);
  await navigate("/login");
  await checkForm("Sign in to Legura", ["email", "password"]);
  assert.equal(await page.locator("#password").getAttribute("autocomplete"), "current-password");
  await checkKeyboardOrder(["email", "password"], "Sign in");
  const beforeMissingLogin = actionCount();
  await page.keyboard.press("Enter");
  assert.equal(await page.locator("#email").evaluate((node) => node === document.activeElement && node.validity.valueMissing), true);
  assert.equal(actionCount(), beforeMissingLogin, "Empty login sent a request");
  await checkMobile("login");
  await page.getByLabel("Email", { exact: true }).fill("not-an-email");
  await page.getByLabel("Password", { exact: true }).fill("wrong-password");
  const beforeBadEmail = actionCount();
  await page.getByRole("button", { name: "Sign in", exact: true }).focus();
  await page.keyboard.press("Enter");
  assert.equal(await page.locator("#email").evaluate((node) => node === document.activeElement && node.validity.typeMismatch), true);
  assert.equal(actionCount(), beforeBadEmail, "Malformed login email bypassed browser validation");
  await page.getByLabel("Email", { exact: true }).fill(email);
  await page.getByLabel("Password", { exact: true }).fill("wrong-password");
  await submitValidation("Sign in", "email", "Email or password is incorrect.");
  assert.equal(await page.getByLabel("Email", { exact: true }).inputValue(), email);
  await checkMobile("login-validation");
  await page.getByLabel("Password", { exact: true }).fill(password);
  await submitSuccess("Sign in");
  await page.waitForURL(`${baseUrl}/admin`);
  await page.locator("p").filter({ hasText: email }).waitFor();
  assert.deepEqual(errors, [], "Unexpected browser errors");
  console.log(`Legura browser auth passed (${mode}): keyboard setup/login, linked validation, skip navigation, 320/390px forms, private session, reload and logout.`);
  console.log(`QA screenshots: ${results}`);
} catch (error) {
  console.error({ mode, path: new URL(page.url()).pathname, headings: await page.locator("h1").allTextContents(), responses, errors });
  throw error;
} finally {
  await context.close();
  await browser.close();
}
