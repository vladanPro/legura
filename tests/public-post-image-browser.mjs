import assert from 'node:assert/strict';
import { chromium } from 'playwright';

let input = '';
for await (const chunk of process.stdin) input += chunk;
const { baseUrl, imageUrl, alt } = JSON.parse(input);
assert.equal(new URL(baseUrl).hostname, '127.0.0.1');
const browser = await chromium.launch({ headless: true });
try {
  for (const javaScriptEnabled of [false, true]) {
    const context = await browser.newContext({ javaScriptEnabled });
    try {
      const page = await context.newPage();
      const errors = [];
      page.on('pageerror', error => errors.push(error.message));
      for (const width of [1440, 390]) {
        await page.setViewportSize({ width, height: 960 });
        assert.equal((await page.goto(`${baseUrl}/posts/image-fixture`)).status(), 200);
        const image = page.locator('[data-post-featured-image="true"]');
        assert.equal(await image.getAttribute('src'), imageUrl);
        assert.equal(await image.getAttribute('alt'), alt);
        await image.evaluate(node => node.decode());
        assert.ok(await image.evaluate(node => node.naturalWidth > 0));
        assert.ok(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth));
        assert.equal(await context.cookies().then(cookies => cookies.length), 0);
      }
      assert.deepEqual(errors, []);
    } finally { await context.close(); }
  }
} finally { await browser.close(); }
console.log('Anonymous public post image renders and decodes on desktop/mobile with JS on/off.');
