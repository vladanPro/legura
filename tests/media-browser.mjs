// Browser plugin not available in CI; use the existing test-only Playwright.
import assert from 'node:assert/strict';
import { mkdir } from 'node:fs/promises';
import path from 'node:path';
import { chromium } from 'playwright';
import { waitForDocumentNavigation } from './document-navigation.mjs';

let input = '';
for await (const chunk of process.stdin) input += chunk;
const { baseUrl, cookie, image } = JSON.parse(input);
const url = new URL(baseUrl);
assert.equal(url.protocol, 'http:');
assert.equal(url.hostname, '127.0.0.1');
const separator = cookie.indexOf('=');
assert.ok(separator > 0);
const browser = await chromium.launch({ headless: true });
const sessionCookie = { name: cookie.slice(0, separator), value: cookie.slice(separator + 1), url: baseUrl };
try {
  for (const javaScriptEnabled of [false, true]) {
    const context = await browser.newContext({ javaScriptEnabled });
    try {
      await context.addCookies([sessionCookie]);
      const page = await context.newPage();
      const errors = [];
      page.on('pageerror', error => errors.push(error.message));
      page.on('console', message => { if (message.type() === 'error') errors.push(message.text()); });
      for (const width of [1440, 390]) {
        await page.setViewportSize({ width, height: 960 });
        const response = await page.goto(`${baseUrl}/admin/media`);
        assert.equal(response.status(), 200);
        assert.match(response.headers()['cache-control'], /no-store/);
        assert.ok(await page.getByRole('heading', { name: 'Media library', exact: true }).isVisible());
        assert.equal(await page.locator('[data-media-id]').count(), 10);
        const card = page.locator('[data-media-id]').first();
        const id = await card.getAttribute('data-media-id');
        const details = card.locator('details');
        const code = details.locator('code');
        assert.equal(await details.getAttribute('open'), null);
        assert.equal(await code.isVisible(), false, 'Media ID must be hidden by default');
        assert.ok(!(await card.innerText()).includes(id), 'Media ID leaked into the visible card');
        await details.locator('summary').click();
        assert.ok(await code.isVisible(), 'Technical details must open without JavaScript too');
        assert.equal((await code.innerText()).trim(), id);
        const copy = details.getByRole('button', { name: 'Copy', exact: true });
        if (javaScriptEnabled) {
          await context.grantPermissions(['clipboard-read', 'clipboard-write']);
          await copy.click();
          await details.getByRole('button', { name: 'Copied', exact: true }).waitFor();
          assert.equal((await page.evaluate(() => navigator.clipboard.readText())).trim(), id);
        } else {
          assert.equal(await copy.isVisible(), false, 'No-JS mode must not show a dead Copy button');
        }
        assert.ok(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), 'Expanded technical details overflow');
        await details.locator('summary').click();
        assert.equal(await code.isVisible(), false);
        const previews = page.getByRole('img', { name: 'Stored image preview', exact: true });
        assert.equal(await previews.count(), 10);
        const preview = card.locator('img');
        await preview.scrollIntoViewIfNeeded();
        await preview.evaluate(node => node.decode());
        assert.equal(await preview.getAttribute('src'), `/api/media/${id}`);
        assert.ok(await preview.evaluate(node => node.complete && node.naturalWidth === 1 && node.naturalHeight === 1));
        assert.ok(await preview.evaluate(node => node.getBoundingClientRect().height >= 128));
        const selected = page.locator('a[href="/admin/media"][aria-current="true"]');
        assert.equal(await selected.count(), 1);
        assert.ok(await page.getByLabel('Image', { exact: true }).isVisible());
        assert.ok(await page.getByRole('button', { name: 'Upload image', exact: true }).isVisible());
        assert.ok(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), `Library overflow at ${width}px`);
        const first = await page.locator('[data-media-id]').evaluateAll(nodes => nodes.map(node => node.dataset.mediaId));
        await page.getByRole('link', { name: 'Next page', exact: true }).click();
        await page.waitForURL(`${baseUrl}/admin/media?page=2`);
        assert.equal(await page.locator('[data-media-id]').count(), 1);
        assert.equal(await page.getByRole('link', { name: 'Next page', exact: true }).count(), 0);
        const last = await page.locator('[data-media-id]').getAttribute('data-media-id');
        assert.ok(!first.includes(last));
        await page.getByRole('link', { name: 'Previous page', exact: true }).click();
        await page.waitForURL(`${baseUrl}/admin/media?page=1`);
        assert.equal(await page.locator('[data-media-id]').count(), 10);
        assert.equal(await page.getByRole('link', { name: 'Previous page', exact: true }).count(), 0);
        assert.ok(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth));
        assert.equal(await page.locator('text=Application error').count(), 0);
        if (process.env.LEGURA_MEDIA_SCREENSHOTS) {
          const directory = path.resolve(process.env.LEGURA_MEDIA_SCREENSHOTS);
          await mkdir(directory, { recursive: true });
          await page.screenshot({ path: path.join(directory, `media-${width}-js-${javaScriptEnabled}.png`), fullPage: true });
        }
      }
      assert.deepEqual(errors, []);
    } finally { await context.close(); }
  }
  let total = 11;
  for (const javaScriptEnabled of [false, true]) {
    const context = await browser.newContext({ javaScriptEnabled });
    try {
      await context.addCookies([sessionCookie]);
      const page = await context.newPage();
      await page.goto(`${baseUrl}/admin/media`);
      await page.getByLabel('Image', { exact: true }).setInputFiles({ name: 'image.png', mimeType: 'image/png', buffer: Buffer.from(image, 'base64') });
      const response = page.waitForResponse(value => value.url().includes('name=UploadLibraryImageAndReturn') && value.request().method() === 'POST');
      const navigation = waitForDocumentNavigation(page, `${baseUrl}/admin/media`);
      await page.getByRole('button', { name: 'Upload image', exact: true }).click();
      assert.ok([200, 303].includes((await response).status()));
      await navigation;
      total += 1;
      assert.equal(await page.locator('[data-media-total]').getAttribute('data-media-total'), String(total));
    } finally { await context.close(); }
  }
} finally { await browser.close(); }
console.log('Media library Chromium acceptance passed: JS/no-JS upload/redirect, desktop/mobile, selected navigation, pagination and no overflow/console errors.');
