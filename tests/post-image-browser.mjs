// Browser plugin not available; reuse existing test-only Playwright.
import assert from 'node:assert/strict';
import { mkdir } from 'node:fs/promises';
import path from 'node:path';
import { chromium } from 'playwright';
import { waitForDocumentNavigation } from './document-navigation.mjs';

let input = '';
for await (const chunk of process.stdin) input += chunk;
const { baseUrl, cookie, postId } = JSON.parse(input);
assert.equal(new URL(baseUrl).hostname, '127.0.0.1');
assert.equal(new URL(baseUrl).protocol, 'http:');
const separator = cookie.indexOf('=');
assert.ok(separator > 0);
const browser = await chromium.launch({ headless: true });
try {
  for (const javaScriptEnabled of [false, true]) {
    const context = await browser.newContext({ javaScriptEnabled });
    try {
      await context.addCookies([{ name: cookie.slice(0, separator), value: cookie.slice(separator + 1), url: baseUrl }]);
      const page = await context.newPage();
      const errors = [];
      page.on('pageerror', error => errors.push(error.message));
      page.on('console', message => {
        if (message.type() === 'error' && !message.text().startsWith('Failed to load resource:')) errors.push(message.text());
      });
      const picker = `${baseUrl}/admin/posts/${postId}/image`;
      async function submit(name) {
        const response = page.waitForResponse(value => value.url().includes('/__axonyx/action?') && value.request().method() === 'POST');
        const navigation = waitForDocumentNavigation(page, picker);
        await page.getByRole('button', { name, exact: true }).click();
        assert.ok([200, 303].includes((await response).status()));
        await navigation;
      }
      assert.equal((await page.goto(picker)).status(), 200);
      assert.equal(await page.title(), 'Legura CMS');
      assert.ok(await page.getByRole('heading', { name: 'Featured image', exact: true }).isVisible());
      assert.equal(await page.locator('a[href="/admin/posts"][aria-current="true"]').count(), 1);
      assert.equal(await page.locator('.legura-image-choice input').count(), 10);
      const choices = page.locator('details').filter({ has: page.locator('.legura-image-options') });
      if (await choices.getAttribute('open') === null) await choices.locator('summary').click();
      const first = page.locator('.legura-image-choice input').first();
      const id = await first.getAttribute('value');
      await first.check();
      await page.getByLabel('Alternative text', { exact: true }).fill('Browser-selected image');
      await submit('Save featured image');
      await page.locator('[data-post-image-preview="true"]').waitFor();
      await page.reload();
      assert.equal(await page.locator('details').filter({ has: page.locator('.legura-image-options') }).getAttribute('open'), null);
      assert.equal(await page.getByLabel('Alternative text', { exact: true }).inputValue(), 'Browser-selected image');
      const preview = page.locator('[data-post-image-preview="true"]');
      assert.equal(await preview.getAttribute('src'), `/api/media/${id}`);
      await preview.evaluate(node => node.decode());
      assert.ok(await preview.evaluate(node => node.naturalWidth > 0));
      for (const width of [1440, 390]) {
        await page.setViewportSize({ width, height: 960 });
        assert.ok(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), 'Picker overflow');
        if (process.env.LEGURA_MEDIA_SCREENSHOTS) {
          const directory = path.resolve(process.env.LEGURA_MEDIA_SCREENSHOTS);
          await mkdir(directory, { recursive: true });
          await page.screenshot({ path: path.join(directory, `post-image-${width}-js-${javaScriptEnabled}.png`), fullPage: true });
        }
      }
      await page.getByLabel('Alternative text', { exact: true }).fill('Updated description');
      await page.getByLabel('Keep current image', { exact: true }).check();
      await submit('Save featured image');
      await page.reload();
      assert.equal(await page.getByLabel('Alternative text', { exact: true }).inputValue(), 'Updated description');
      await page.getByRole('link', { name: 'Next page', exact: true }).click();
      assert.ok((await page.locator('.legura-image-choice input').count()) < 10);
      await submit('Remove featured image');
      await page.getByRole('heading', { name: 'No featured image selected', exact: true }).waitFor();
      await page.reload();
      assert.equal(await page.locator('[data-post-image-preview="true"]').count(), 0);
      assert.deepEqual(errors, []);
    } finally { await context.close(); }
  }
} finally { await browser.close(); }
console.log('Post image browser passed: JS/no-JS select, alt edit, pagination, detach, decoded preview and desktop/mobile layout.');
