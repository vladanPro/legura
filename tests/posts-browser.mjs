import assert from "node:assert/strict";
import { mkdir } from "node:fs/promises";
import { resolve } from "node:path";
import { tmpdir } from "node:os";
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
const searchResults = resolve(tmpdir(), 'legura-post-search');
await mkdir(searchResults, { recursive: true });

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

async function checkAdminFrame() {
  const nav = page.getByRole('navigation', { name: 'Administration', exact: true });
  const active = nav.locator('[data-active="true"]');
  assert.equal(await active.count(), 1, 'Exactly one admin section must be active');
  assert.equal(await active.getAttribute('href'), '/admin/posts');
  assert.equal(await active.getAttribute('aria-current'), 'true');
  assert.equal(await page.getByRole('main').count(), 1);
  for (const width of [320, 390, 1280]) {
    await page.setViewportSize({ width, height: 900 });
    assert.ok(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), `Admin shell overflow at ${width}px`);
    assert.ok(await active.evaluate(node => node.getBoundingClientRect().height >= 44), 'Sidebar link target is shorter than 44px');
  }
}

async function checkPostsTable(title) {
  const table = page.getByRole('table', { name: 'Your posts', exact: true });
  assert.equal(await table.count(), 1);
  assert.deepEqual(await table.getByRole('columnheader').allTextContents(), ['Title and URL slug', 'Status', 'Actions']);
  assert.equal(await table.getByRole('rowheader').filter({ hasText: title }).count(), 1);
  const edit = table.getByRole('link', { name: `Edit post: ${title}`, exact: true });
  assert.ok(await edit.evaluate(node => node.getBoundingClientRect().height >= 44));
  const region = page.getByRole('region', { name: 'Scrollable posts table', exact: true });
  await page.screenshot({ path: resolve(results, `posts-table-${mode}-desktop.png`), fullPage: true });
  if (mode === 'javascript') {
    await page.getByLabel('Appearance', { exact: true }).selectOption('dark');
    assert.equal(await table.evaluate(node => getComputedStyle(node).colorScheme), 'dark');
    await page.screenshot({ path: resolve(results, 'posts-table-dark-desktop.png'), fullPage: true, animations: 'disabled' });
    await page.getByLabel('Appearance', { exact: true }).selectOption('light');
  }
  await page.setViewportSize({ width: 320, height: 844 });
  assert.ok(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), 'Table expands the page on mobile');
  assert.ok(await region.evaluate(node => node.scrollWidth > node.clientWidth), 'Wide table must scroll inside its own region');
  await region.focus();
  await page.keyboard.press('ArrowRight');
  // Poll in the runner so no-JS coverage does not depend on browser timers.
  let scrolled = false;
  for (let attempt = 0; attempt < 20; attempt += 1) {
    scrolled = await region.evaluate(node => node.scrollLeft > 0);
    if (scrolled) break;
    await new Promise(resolve => setTimeout(resolve, 50));
  }
  assert.ok(scrolled, 'Focused table region must scroll with the keyboard');
  await page.screenshot({ path: resolve(results, `posts-table-${mode}-mobile.png`), fullPage: true });
  await page.setViewportSize({ width: 1280, height: 900 });
}

async function checkOverview(total, drafts, published, screenshot = false) {
  await navigate(page, '/admin');
  const overview = page.getByRole('region', { name: 'Content overview', exact: true });
  for (const [title, count] of [['All posts', total], ['Drafts', drafts], ['Published', published]]) {
    const card = overview.getByRole('article').filter({ has: page.getByRole('heading', { name: title, exact: true }) });
    assert.equal(await card.count(), 1);
    assert.equal(await card.locator('.legura-stat-value').textContent(), String(count));
  }
  assert.equal(await page.getByRole('heading', { name: 'Start with your first draft', exact: true }).count(), total === 0 ? 1 : 0);
  const nav = page.getByRole('navigation', { name: 'Administration', exact: true });
  assert.equal(await nav.locator('[data-active="true"]').getAttribute('href'), '/admin');
  assert.equal(await page.getByRole('main').count(), 1);
  assert.equal(await overview.getByRole('link', { name: 'View all posts', exact: true }).getAttribute('href'), '/admin/posts');
  assert.equal(await overview.getByRole('link', { name: 'Review drafts', exact: true }).getAttribute('href'), '/admin/posts?status=draft');
  assert.equal(await overview.getByRole('link', { name: 'View published posts', exact: true }).getAttribute('href'), '/admin/posts?status=published');
  if (screenshot) {
    const directory = resolve(tmpdir(), 'legura-overview-qa');
    await mkdir(directory, { recursive: true });
    for (const width of [1280, 390, 320]) {
      await page.setViewportSize({ width, height: 900 });
      assert.ok(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), `Overview overflow at ${width}px`);
      for (const link of await overview.getByRole('link').all()) {
        assert.ok(await link.evaluate(node => node.getBoundingClientRect().height >= 44), 'Overview link target is shorter than 44px');
      }
      await page.screenshot({ path: resolve(directory, `overview-${mode}-${width}.png`), fullPage: true });
    }
    await page.setViewportSize({ width: 1280, height: 900 });
  }
}

async function checkPostListing() {
  for (let index = 1; index <= 11; index += 1) {
    await navigate(page, '/admin/posts/new');
    const suffix = String(index).padStart(2, '0');
    const title = index === 11 ? `Listing ${suffix} & + % _ " Č` : `Listing ${suffix}`;
    await fillPost(title, `listing-${suffix}`, 'Pagination fixture.', index <= 6 ? 'draft' : 'published');
    await submit('Create post', '/admin/posts');
  }
  await checkOverview(11, 6, 5, true);
  await page.getByRole('link', { name: 'Review drafts', exact: true }).click();
  await page.waitForURL(`${baseUrl}/admin/posts?status=draft`);
  assert.equal(await page.getByRole('rowheader').count(), 6);
  await navigate(page, '/admin');
  await page.getByRole('link', { name: 'View published posts', exact: true }).click();
  await page.waitForURL(`${baseUrl}/admin/posts?status=published`);
  assert.equal(await page.getByRole('rowheader').count(), 5);
  await navigate(page, '/admin');
  await page.getByRole('link', { name: 'View all posts', exact: true }).click();
  await page.waitForURL(`${baseUrl}/admin/posts`);
  const table = page.getByRole('table', { name: 'Your posts', exact: true });
  const rows = table.getByRole('rowheader');
  const navigation = page.getByRole('navigation', { name: 'Posts pages', exact: true });
  assert.equal(await rows.count(), 10);
  assert.equal(await rows.first().textContent(), 'Listing 01listing-01');
  assert.equal(await navigation.getByRole('button', { name: 'Previous page' }).count(), 0);
  await navigation.getByRole('button', { name: 'Next page' }).click();
  await page.waitForURL(`${baseUrl}/admin/posts?q=&status=all&page=2`);
  assert.equal(await rows.count(), 1);
  assert.match(await rows.first().textContent(), /Listing 11/);
  assert.equal(await navigation.getByRole('button', { name: 'Next page' }).count(), 0);
  await page.reload();
  assert.equal(await rows.count(), 1);
  await navigation.getByRole('button', { name: 'Previous page' }).click();
  await page.waitForURL(`${baseUrl}/admin/posts?q=&status=all&page=1`);
  assert.equal(await rows.count(), 10);

  await navigate(page, '/admin/posts?status=all&page=2');
  await page.getByLabel('Filter status', { exact: true }).selectOption('draft');
  await page.getByRole('button', { name: 'Apply filters', exact: true }).click();
  await page.waitForURL(`${baseUrl}/admin/posts?q=&status=draft`);
  assert.equal(await rows.count(), 6);
  assert.deepEqual(await table.locator('tbody .ax-badge').allTextContents(), Array(6).fill('draft'));
  await page.getByLabel('Filter status', { exact: true }).selectOption('published');
  await page.getByRole('button', { name: 'Apply filters', exact: true }).click();
  await page.waitForURL(`${baseUrl}/admin/posts?q=&status=published`);
  assert.equal(await rows.count(), 5);
  assert.deepEqual(await table.locator('tbody .ax-badge').allTextContents(), Array(5).fill('published'));

  for (const value of ['', '0', '-1', 'abc', '1.5', "1; DROP TABLE legura_posts"]) {
    await navigate(page, `/admin/posts?status=all&page=${encodeURIComponent(value)}`);
    assert.equal(await navigation.textContent(), 'Page 1Next page', `Invalid page ${value}`);
    assert.equal(await rows.count(), 10);
  }
  await navigate(page, '/admin/posts?status=all&page=999999999999999999999999999');
  assert.equal(await rows.count(), 1);
  await navigate(page, '/admin/posts?status=published&page=99');
  assert.equal(await rows.count(), 5);
  assert.equal(await navigation.textContent(), 'Page 1');
  await navigate(page, `/admin/posts?status=${encodeURIComponent("draft' OR 1=1 --")}`);
  assert.equal(await page.getByLabel('Filter status').inputValue(), 'all');
  assert.equal(await rows.count(), 10);
  await navigate(page, '/admin/posts?status=draft&status=published&pa%67e=1');
  assert.equal(await rows.count(), 5, 'Decoded keys and last duplicate value must match rendering');
  await navigate(page, '/admin/posts?status=all&page=2');
  await page.getByLabel('Search titles', { exact: true }).fill('  lIsTiNg  ');
  await page.getByRole('button', { name: 'Apply filters', exact: true }).click();
  await page.waitForURL(url => url.searchParams.get('q') === '  lIsTiNg  ' && !url.searchParams.has('page'));
  assert.equal(await page.getByLabel('Search titles').inputValue(), 'lIsTiNg');
  assert.equal(await rows.count(), 10);
  await navigation.getByRole('button', { name: 'Next page' }).click();
  await page.waitForURL(url => url.searchParams.get('q') === 'lIsTiNg' && url.searchParams.get('page') === '2');
  assert.equal(await rows.count(), 1);
  await page.reload();
  assert.equal(await page.getByLabel('Search titles').inputValue(), 'lIsTiNg');
  await navigation.getByRole('button', { name: 'Previous page' }).click();
  await page.waitForURL(url => url.searchParams.get('q') === 'lIsTiNg' && url.searchParams.get('page') === '1');
  await page.getByLabel('Filter status').selectOption('published');
  await page.getByRole('button', { name: 'Apply filters', exact: true }).click();
  await page.waitForURL(url => url.searchParams.get('status') === 'published' && !url.searchParams.has('page'));
  assert.equal(await rows.count(), 5);
  await page.getByLabel('Search titles').fill('11');
  await page.getByRole('button', { name: 'Apply filters', exact: true }).click();
  await page.waitForURL(url => url.searchParams.get('q') === '11');
  assert.equal(await rows.count(), 1);
  assert.match(await rows.first().textContent(), /Listing 11/);
  for (const search of ['%', '_', '& +', '" Č']) {
    await page.getByLabel('Search titles').fill(search);
    await page.getByRole('button', { name: 'Apply filters', exact: true }).click();
    await page.waitForURL(url => url.searchParams.get('q') === search);
    assert.equal(await rows.count(), 1, 'Search metacharacters must match literal text');
    assert.match(await rows.first().textContent(), /Listing 11/);
    assert.equal(await page.getByLabel('Search titles').inputValue(), search);
    await page.reload();
    assert.equal(await rows.count(), 1);
  }
  for (const search of ["' OR 1=1 --", '<script>window.__searchInjected=true</script>', 'Listing & + ? # / "']) {
    await navigate(page, `/admin/posts?q=${encodeURIComponent(search)}`);
    await page.getByRole('heading', { name: 'No matching posts', exact: true }).waitFor();
    assert.equal(await rows.count(), 0);
    assert.equal(await page.getByLabel('Search titles').inputValue(), search);
    assert.equal(await page.evaluate(() => window.__searchInjected), undefined);
  }
  await navigate(page, '/admin/posts?q=' + 'x'.repeat(250));
  assert.equal(await page.getByLabel('Search titles').inputValue(), 'x'.repeat(200));
  await page.getByRole('link', { name: 'Reset filters', exact: true }).click();
  await page.waitForURL(`${baseUrl}/admin/posts`);
  assert.equal(await rows.count(), 10);
  assert.equal(await page.getByLabel('Search titles').inputValue(), '');
  assert.equal(await page.getByRole('link', { name: 'Reset filters', exact: true }).count(), 0);
  await navigate(page, '/admin/posts?q=Listing+11&q=Listing+07&status=published&page=999');
  assert.equal(await rows.count(), 1);
  assert.match(await rows.first().textContent(), /Listing 07/);
  if (mode === 'javascript') {
    const refreshed = await page.evaluate(async () => {
      const response = await fetch('/__axonyx/data?path=' + encodeURIComponent('/admin/posts?status=published&page=1&q=Listing%2011') + '&name=listing');
      return { status: response.status, payload: await response.json() };
    });
    assert.equal(refreshed.status, 200);
    assert.equal(refreshed.payload.value.posts.length, 1);
    assert.equal(refreshed.payload.value.status, 'published');
    assert.equal(refreshed.payload.value.search, 'Listing 11');
    assert.ok(refreshed.payload.html.includes('Listing 11'));
  }
  await navigate(page, '/admin/posts?q=Listing&status=all');
  await checkAdminFrame();
  for (const width of [320, 390]) {
    await page.setViewportSize({ width, height: 844 });
    assert.ok(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), 'Search controls overflow');
    await page.screenshot({ path: resolve(searchResults, `posts-search-${mode}-${width}.png`), fullPage: true });
    for (const [name, control] of [['Search titles', page.getByLabel('Search titles')], ['Filter status', page.getByLabel('Filter status')], ['Apply filters', page.getByRole('button', { name: 'Apply filters', exact: true })], ['Next page', navigation.getByRole('button', { name: 'Next page' })]]) {
      const height = await control.evaluate(node => node.getBoundingClientRect().height);
      assert.ok(height >= 44, `${name} target is shorter than 44px: ${height}`);
    }
  }
  await page.setViewportSize({ width: 1280, height: 900 });
  await page.screenshot({ path: resolve(searchResults, `posts-search-${mode}-desktop.png`), fullPage: true });
  await page.screenshot({ path: resolve(results, `posts-filter-${mode}-desktop.png`), fullPage: true, animations: 'disabled' });
  await navigate(publicPage, '/admin/posts?status=published&page=2&q=Listing', 403);
  await navigate(publicPage, '/posts/listing-01', 404);
}

try {
  await navigate(publicPage, "/admin", 403);
  await navigate(publicPage, "/__axonyx/data?path=%2Fadmin&name=overview", 403);
  await navigate(publicPage, "/admin/posts", 403);
  await navigate(publicPage, "/posts");
  await publicPage.getByText("Nothing published yet", { exact: true }).waitFor();
  await navigate(page, "/setup");
  await page.getByLabel("Site name", { exact: true }).fill("Posts fixture");
  await page.getByLabel("Administrator email", { exact: true }).fill("editor@example.com");
  await page.getByLabel("Password", { exact: true }).fill(`fixture-password-${randomUUID()}`);
  await page.getByLabel("Setup token", { exact: true }).fill(setupToken);
  await submit("Create installation", "/admin");
  await checkOverview(0, 0, 0);
  await page.getByRole("link", { name: "Manage posts", exact: true }).click();
  await page.getByText("No posts yet", { exact: true }).waitFor();
  assert.equal(await page.getByRole('table', { name: 'Your posts', exact: true }).count(), 0);
  await navigate(page, '/admin/posts?status=published&page=2');
  await page.getByText('No matching posts', { exact: true }).waitFor();
  assert.equal(await page.getByRole('navigation', { name: 'Posts pages' }).textContent(), 'Page 1');
  await page.getByRole('link', { name: 'Show all posts', exact: true }).click();
  await page.getByText('No posts yet', { exact: true }).waitFor();
  await checkAdminFrame();
  await page.getByRole("link", { name: "New post", exact: true }).click();
  await checkAdminFrame();
  await fillPost("Private browser draft", "browser-draft", "Only the administrator should see this.", "draft");
  await submit("Create post", "/admin/posts");
  await checkOverview(1, 1, 0);
  await navigate(page, '/admin/posts');
  await page.getByRole("link", { name: "Private browser draft", exact: true }).waitFor();
  await checkPostsTable('Private browser draft');
  await navigate(publicPage, "/posts");
  assert.ok(!(await publicPage.content()).includes("Private browser draft"));
  await navigate(publicPage, "/posts/browser-draft", 404);
  await page.getByRole("link", { name: "Edit post: Private browser draft", exact: true }).click();
  const editPath = new URL(page.url()).pathname;
  assert.match(editPath, /^\/admin\/posts\/[^/]+\/edit$/);
  await checkAdminFrame();

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
  await checkOverview(1, 0, 1);
  await navigate(page, '/admin/posts');
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
  await checkOverview(1, 1, 0);
  await navigate(page, '/admin/posts');
  await navigate(publicPage, "/posts/browser-story", 404);
  await navigate(publicPage, "/posts");
  await publicPage.getByText("Nothing published yet", { exact: true }).waitFor();
  await navigate(page, editPath);
  await page.getByRole("link", { name: "Delete post", exact: true }).click();
  const deletePath = new URL(page.url()).pathname;
  assert.ok(deletePath.endsWith("/delete"));
  await checkAdminFrame();
  await page.getByRole("link", { name: "Cancel deletion", exact: true }).click();
  await page.waitForURL(`${baseUrl}${editPath}`);
  assert.equal(await page.getByLabel("Content", { exact: true }).inputValue(), content);
  await page.getByLabel("Status", { exact: true }).selectOption("published");
  await submit("Save changes", "/admin/posts");
  await navigate(publicPage, "/posts/browser-story");
  await navigate(page, deletePath);
  await navigate(publicPage, deletePath, 403);
  await page.getByText("This permanently deletes the post.", { exact: true }).waitFor();
  assert.equal(await page.locator("#confirmation-hint").textContent(), "Enter exactly: browser-story");
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
  await checkOverview(0, 0, 0);
  await navigate(page, '/admin/posts');
  await page.getByText("No posts yet", { exact: true }).waitFor();
  await navigate(publicPage, "/posts/browser-story", 404);
  await navigate(publicPage, "/posts");
  await publicPage.getByText("Nothing published yet", { exact: true }).waitFor();
  await navigate(page, editPath, 404);
  await navigate(page, deletePath, 404);
  await checkPostListing();
  assert.deepEqual(errors, [], "Unexpected browser errors");
  console.log(`Legura posts browser passed (${mode}): CRUD, guarded overview counts, authorization, literal title search, filters, bounded server pagination and URL normalization.`);
} finally {
  await reader.close();
  await context.close();
  await browser.close();
}
