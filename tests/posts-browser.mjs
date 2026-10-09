import assert from "node:assert/strict";
import { mkdir } from "node:fs/promises";
import { resolve } from "node:path";
import { randomUUID } from "node:crypto";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright";

const [baseUrl, mode] = process.argv.slice(2);
assert.ok(["127.0.0.1", "localhost"].includes(new URL(baseUrl).hostname), "Use an isolated local fixture");
assert.ok(["javascript", "native"].includes(mode), "Choose a browser mode");
const setupToken = process.env.AX_SECRET_SETUP_TOKEN;
assert.ok(setupToken, "Fixture owner token is required");
const browser = await chromium.launch();
const context = await browser.newContext({ javaScriptEnabled: mode === "javascript", viewport: { width: 1280, height: 900 } });
const reader = await browser.newContext({ javaScriptEnabled: mode === "javascript" });
const page = await context.newPage();
const publicPage = await reader.newPage();
const errors = [];
for (const target of [page, publicPage]) {
  target.on("pageerror", (error) => errors.push(error.message));
  target.on("console", (message) => {
    if (message.type() === "error" && !message.text().startsWith("Failed to load resource:")) errors.push(message.text());
  });
}
const results = fileURLToPath(new URL("../test-results/", import.meta.url));
await mkdir(results, { recursive: true });

async function navigate(target, path, expected = 200) {
  assert.equal((await target.goto(`${baseUrl}${path}`)).status(), expected, path);
}

async function submit(button, destination) {
  const responsePromise = page.waitForResponse((response) => response.url().includes("/__axonyx/action?") && response.request().method() === "POST");
  await page.getByRole("button", { name: button, exact: true }).click();
  const response = await responsePromise;
  assert.ok([200, 303].includes(response.status()), `Unexpected ${button} status ${response.status()}`);
  await page.waitForURL(`${baseUrl}${destination}`);
}

async function fillPost(title, slug, body, status) {
  await page.getByLabel("Title", { exact: true }).fill(title);
  await page.getByLabel("URL slug", { exact: true }).fill(slug);
  await page.getByLabel("Content", { exact: true }).fill(body);
  await page.getByLabel("Status", { exact: true }).selectOption(status);
}

try {
  await navigate(publicPage, "/admin/posts", 403);
  await navigate(publicPage, "/posts");
  await publicPage.getByText("Nothing published yet", { exact: true }).waitFor();
  await navigate(page, "/setup");
  await page.getByLabel("Site name", { exact: true }).fill("Posts fixture");
  await page.getByLabel("Administrator email", { exact: true }).fill("editor@example.com");
  await page.getByLabel("Password", { exact: true }).fill(`fixture-password-${randomUUID()}`);
  await page.getByLabel("Setup token", { exact: true }).fill(setupToken);
  await submit("Create installation", "/admin");
  await page.getByRole("link", { name: "Manage posts", exact: true }).click();
  await page.getByText("No posts yet", { exact: true }).waitFor();
  await page.getByRole("link", { name: "New post", exact: true }).click();
  await fillPost("Private browser draft", "browser-draft", "Only the administrator should see this.", "draft");
  await submit("Create post", "/admin/posts");
  await page.getByRole("link", { name: "Private browser draft", exact: true }).waitFor();
  await navigate(publicPage, "/posts");
  assert.ok(!(await publicPage.content()).includes("Private browser draft"));
  await navigate(publicPage, "/posts/browser-draft", 404);
  await page.getByRole("link", { name: "Edit post", exact: true }).click();
  const editPath = new URL(page.url()).pathname;
  assert.match(editPath, /^\/admin\/posts\/[^/]+\/edit$/);

  const longText = `${"Editor text ".repeat(1000)}\nČuvanje </textarea><script>unsafe</script>`;
  await navigate(page, "/admin/posts/new");
  await fillPost("Duplicate draft", "browser-draft", longText, "draft");
  const duplicateResponse = page.waitForResponse((response) => response.url().includes("/__axonyx/action?") && response.request().method() === "POST");
  await page.getByRole("button", { name: "Create post", exact: true }).click();
  assert.equal((await duplicateResponse).status(), 422);
  await page.locator('[data-ax-field-error="slug"]').filter({ hasText: "already in use" }).waitFor({ state: "visible" });
  assert.equal(await page.getByLabel("Title", { exact: true }).inputValue(), "Duplicate draft");
  assert.equal(await page.getByLabel("Content", { exact: true }).inputValue(), longText);
  assert.equal(await page.getByLabel("URL slug", { exact: true }).getAttribute("aria-invalid"), "true");
  assert.equal(await page.locator("textarea script").count(), 0);
  await navigate(page, editPath);
  assert.equal(await page.getByLabel("Title", { exact: true }).inputValue(), "Private browser draft");
  assert.equal(await page.getByLabel("Content", { exact: true }).inputValue(), "Only the administrator should see this.");
  assert.equal(await page.getByLabel("Status", { exact: true }).inputValue(), "draft");

  // Bypass browser required validation to exercise the server's actual 422 retry.
  await page.locator("form").evaluate((form) => { form.noValidate = true; });
  await page.getByLabel("Title", { exact: true }).fill("");
  await page.getByLabel("Content", { exact: true }).fill(longText);
  const invalidResponse = page.waitForResponse((response) => response.url().includes("/__axonyx/action?") && response.request().method() === "POST");
  await page.getByRole("button", { name: "Save changes", exact: true }).click();
  assert.equal((await invalidResponse).status(), 422);
  await page.locator('[data-ax-field-error="title"]').filter({ hasText: "Enter a title" }).waitFor({ state: "visible" });
  assert.equal(await page.getByLabel("URL slug", { exact: true }).inputValue(), "browser-draft");
  assert.equal(await page.getByLabel("Content", { exact: true }).inputValue(), longText);

  const content = '<script>window.__leguraInjected = true</script>\nSecond line, safely shown.';
  await fillPost("Published browser story", "browser-story", content, "published");
  await page.setViewportSize({ width: 390, height: 844 });
  assert.ok(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), "Mobile editor overflow");
  await page.screenshot({ path: resolve(results, `editor-${mode}-mobile.png`), fullPage: true });
  await submit("Save changes", "/admin/posts");
  await navigate(publicPage, "/posts");
  await publicPage.getByRole("link", { name: "Read post", exact: true }).click();
  await publicPage.getByRole("heading", { name: "Published browser story", exact: true }).waitFor();
  assert.equal(await publicPage.locator(".legura-story__body").textContent(), content);
  assert.equal(await publicPage.locator(".legura-story__body script").count(), 0);
  assert.equal(await publicPage.evaluate(() => window.__leguraInjected), undefined);
  await publicPage.screenshot({ path: resolve(results, `story-${mode}-desktop.png`), fullPage: true });
  await navigate(page, editPath);
  assert.equal(await page.getByLabel("Status", { exact: true }).inputValue(), "published");
  await page.getByLabel("Status", { exact: true }).selectOption("draft");
  await submit("Save changes", "/admin/posts");
  await navigate(publicPage, "/posts/browser-story", 404);
  await navigate(publicPage, "/posts");
  await publicPage.getByText("Nothing published yet", { exact: true }).waitFor();
  await navigate(page, editPath);
  await page.getByRole("link", { name: "Delete post", exact: true }).click();
  const deletePath = new URL(page.url()).pathname;
  assert.ok(deletePath.endsWith("/delete"));
  await page.getByRole("link", { name: "Cancel deletion", exact: true }).click();
  await page.waitForURL(`${baseUrl}${editPath}`);
  assert.equal(await page.getByLabel("Content", { exact: true }).inputValue(), content);
  await page.getByLabel("Status", { exact: true }).selectOption("published");
  await submit("Save changes", "/admin/posts");
  await navigate(publicPage, "/posts/browser-story");
  await navigate(page, deletePath);
  await navigate(publicPage, deletePath, 403);
  await page.getByText("This permanently deletes the post.", { exact: true }).waitFor();
  assert.ok(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), "Mobile deletion form overflow");
  await page.screenshot({ path: resolve(results, `delete-${mode}-mobile.png`), fullPage: true });
  await page.getByLabel("Type the URL slug to confirm", { exact: true }).fill("wrong");
  const wrongConfirmation = page.waitForResponse((response) => response.url().includes("/__axonyx/action?") && response.request().method() === "POST");
  await page.getByRole("button", { name: "Delete post permanently", exact: true }).click();
  assert.equal((await wrongConfirmation).status(), 422);
  await page.locator('[data-ax-field-error="confirmSlug"]').filter({ hasText: "URL slug to confirm deletion" }).waitFor({ state: "visible" });
  await navigate(publicPage, "/posts/browser-story");
  await page.getByLabel("Type the URL slug to confirm", { exact: true }).fill("browser-story");
  await submit("Delete post permanently", "/admin/posts");
  await page.getByText("No posts yet", { exact: true }).waitFor();
  await navigate(publicPage, "/posts/browser-story", 404);
  await navigate(publicPage, "/posts");
  await publicPage.getByText("Nothing published yet", { exact: true }).waitFor();
  await navigate(page, editPath, 404);
  await navigate(page, deletePath, 404);
  assert.deepEqual(errors, [], "Unexpected browser errors");
  console.log(`Legura posts browser passed (${mode}): create, validation retry, publish/unpublish, safe text, cancellation and confirmed deletion.`);
} finally {
  await reader.close();
  await context.close();
  await browser.close();
}
