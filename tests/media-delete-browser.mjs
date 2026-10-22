import assert from 'node:assert/strict';
import { chromium } from 'playwright';
import { waitForDocumentNavigation } from './document-navigation.mjs';

let input = '';
for await (const chunk of process.stdin) input += chunk;
const { baseUrl, cookie, used, ids } = JSON.parse(input);
assert.equal(new URL(baseUrl).hostname, '127.0.0.1');
const separator = cookie.indexOf('=');
assert.ok(separator > 0);
const browser = await chromium.launch({ headless: true });
try {
  for (const [index, javaScriptEnabled] of [false, true].entries()) {
    const context = await browser.newContext({ javaScriptEnabled, viewport: { width: 390, height: 844 } });
    try {
      await context.addCookies([{ name: cookie.slice(0, separator), value: cookie.slice(separator + 1), url: baseUrl }]);
      const page = await context.newPage();
      const errors = [];
      page.on('pageerror', error => errors.push(error.message));
      await page.goto(`${baseUrl}/admin/media/${used}/delete`);
      await page.getByRole('checkbox').check();
      const rejected = page.waitForResponse(r => r.url().includes('/__axonyx/action?') && r.status() === 422);
      await page.getByRole('button', { name: 'Delete image permanently' }).click();
      await rejected;
      await page.locator('[data-ax-field-error="operation"]').filter({ hasText: 'including drafts' }).waitFor();
      assert.ok(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth));
      await page.goto(`${baseUrl}/admin/media/${ids[index]}/delete`);
      await page.getByRole('link', { name: 'Cancel deletion' }).click();
      await page.waitForURL(`${baseUrl}/admin/media`);
      assert.equal((await page.goto(`${baseUrl}/admin/media/${ids[index]}/delete`)).status(), 200);
      await page.getByRole('checkbox').check();
      const navigation = waitForDocumentNavigation(page, `${baseUrl}/admin/media`);
      await page.getByRole('button', { name: 'Delete image permanently' }).click();
      await navigation;
      assert.equal((await page.goto(`${baseUrl}/admin/media/${ids[index]}/delete`)).status(), 404);
      assert.deepEqual(errors, []);
    } finally { await context.close(); }
  }
} finally { await browser.close(); }
console.log('Media delete browser passed with JS/no-JS: refusal feedback, cancel, confirmed deletion and stale page.');
