import assert from "node:assert/strict";
import { mkdir } from "node:fs/promises";
import { resolve } from "node:path";
import { randomUUID } from "node:crypto";
import { fileURLToPath } from "node:url";
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
const results = fileURLToPath(new URL("../test-results/", import.meta.url));
await mkdir(results, { recursive: true });

async function navigate(path, status = 200) {
  const response = await page.goto(`${baseUrl}${path}`);
  assert.equal(response.status(), status, `Unexpected response for ${path}`);
}

async function submitValidation(button, field, expected) {
  const responsePromise = page.waitForResponse((response) =>
    response.url().includes("/__axonyx/action?") && response.request().method() === "POST"
  );
  await page.getByRole("button", { name: button, exact: true }).click();
  const response = await responsePromise;
  assert.equal(response.status(), 422);
  const body = await response.text();
  assert.ok(!body.includes(password) && !body.includes(setupToken), "Response echoed a credential");
  const error = page.locator(`[data-ax-field-error="${field}"]`);
  await error.filter({ hasText: expected }).waitFor({ state: "visible" });
}

async function submitSuccess(button) {
  const responsePromise = page.waitForResponse((response) =>
    response.url().includes("/__axonyx/action?") && response.request().method() === "POST"
  );
  await page.getByRole("button", { name: button, exact: true }).click();
  const response = await responsePromise;
  assert.ok([200, 303].includes(response.status()), `Successful ${button} returned HTTP ${response.status()}`);
}

try {
  await navigate("/admin", 403);
  await navigate("/setup");
  assert.equal(await page.title(), "Legura CMS");
  await page.getByRole("button", { name: "Create installation", exact: true }).waitFor();
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
  await page.getByLabel("Email", { exact: true }).fill(email);
  await page.getByLabel("Password", { exact: true }).fill("wrong-password");
  await submitValidation("Sign in", "email", "Email or password is incorrect.");
  assert.equal(await page.getByLabel("Email", { exact: true }).inputValue(), email);
  await page.getByLabel("Password", { exact: true }).fill(password);
  await submitSuccess("Sign in");
  await page.waitForURL(`${baseUrl}/admin`);
  await page.locator("p").filter({ hasText: email }).waitFor();
  assert.deepEqual(errors, [], "Unexpected browser errors");
  console.log(`Legura browser auth passed (${mode}): setup, validation, private session, reload, logout, login and mobile rendering.`);
} catch (error) {
  console.error({ mode, path: new URL(page.url()).pathname, responses, errors });
  throw error;
} finally {
  await context.close();
  await browser.close();
}
