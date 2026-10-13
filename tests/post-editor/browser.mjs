import assert from 'node:assert/strict';
import { randomUUID } from 'node:crypto';
import { mkdir } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';
import { chromium } from 'playwright';

const [baseUrl, mode] = process.argv.slice(2);
assert.ok(['127.0.0.1', 'localhost'].includes(new URL(baseUrl).hostname));
assert.ok(['javascript', 'native'].includes(mode));
const browser = await chromium.launch();
const context = await browser.newContext({ javaScriptEnabled: mode === 'javascript' });
const page = await context.newPage();
const errors = [];
page.on('pageerror', error => errors.push(error.message));
try {
  await page.goto(baseUrl + '/setup');
  await page.getByLabel('Site name', { exact: true }).fill('Post editor fixture');
  await page.getByLabel('Administrator email', { exact: true }).fill('posts@example.com');
  await page.getByLabel('Password', { exact: true }).fill('Post-editor-1-' + randomUUID());
  await page.getByLabel('Setup token', { exact: true }).fill(process.env.AX_SECRET_SETUP_TOKEN);
  await page.getByRole('button', { name: 'Create installation', exact: true }).click();
  await page.waitForURL(baseUrl + '/admin');
  await page.goto(baseUrl + '/admin/posts/new');
  let richId;
  if (mode === 'javascript') {
    const surface = page.locator('[data-editor-preview]');
    await page.waitForFunction(() => document.querySelector('[data-editor-preview]').contentEditable === 'true');
    await surface.click();
    await page.keyboard.insertText('Real post');
    await page.keyboard.press('Control+a');
    await page.getByRole('button', { name: 'Bold', exact: true }).click();
    await page.getByRole('button', { name: 'Insert link', exact: true }).click();
    await page.getByRole('textbox', { name: 'Link address', exact: true }).fill('https://example.com/post');
    await page.getByRole('button', { name: 'Apply link', exact: true }).click();
    await page.getByLabel('URL slug', { exact: true }).fill('rich-post');
    const doc = await page.locator('[data-editor-document]').inputValue();
    const invalid = page.waitForResponse(r => r.url().includes('/__axonyx/action?') && r.request().method() === 'POST');
    await page.getByRole('button', { name: 'Create post', exact: true }).click();
    assert.equal((await invalid).status(), 422);
    assert.equal(await page.locator('[data-editor-document]').inputValue(), doc, '422 lost rich content');
    await page.getByLabel('Title', { exact: true }).fill('Rich post');
    await page.getByRole('button', { name: 'Create post', exact: true }).click();
    await page.waitForURL(baseUrl + '/admin/posts');
    await page.locator('a[href^="/admin/posts/"][href$="/edit"]').first().click();
    await page.waitForURL(/\/admin\/posts\/[^/]+\/edit$/);
    richId = new URL(page.url()).pathname.split('/')[3];
    await page.waitForFunction(() => document.querySelector('[data-editor-preview]').contentEditable === 'true');
    assert.equal(await surface.locator('a strong').textContent(), 'Real post');
    assert.equal(await surface.locator('a').getAttribute('href'), 'https://example.com/post');
    assert.equal((await context.request.get(baseUrl + '/posts/rich-post')).status(), 404, 'Draft must stay private');
    await page.getByLabel('Title', { exact: true }).fill('Updated rich post');
    await page.getByLabel('Status', { exact: true }).selectOption('published');
    const navigation = page.waitForNavigation({ waitUntil: 'load' });
    await page.getByRole('button', { name: 'Save changes', exact: true }).click();
    await navigation;
    await page.reload();
    await page.waitForFunction(() => document.querySelector('[data-editor-preview]').contentEditable === 'true');
    assert.equal(await page.getByLabel('Title', { exact: true }).inputValue(), 'Updated rich post');
    assert.equal(await surface.locator('a strong').textContent(), 'Real post');
    const readerContext = await browser.newContext({ javaScriptEnabled: false });
    try {
      const reader = await readerContext.newPage();
      const response = await reader.goto(baseUrl + '/posts/rich-post');
      assert.equal(response.status(), 200);
      assert.equal(await reader.locator('.legura-story h1').textContent(), 'Updated rich post');
      assert.equal(await reader.locator('.legura-story__body a strong').textContent(), 'Real post');
      assert.equal(await reader.locator('.legura-story__body a').getAttribute('href'), 'https://example.com/post');
      assert.equal(await reader.locator('script[src*="content-editor"]').count(), 0);
      for (const width of [1280, 390]) {
        await reader.setViewportSize({ width, height: 900 });
        assert.ok(await reader.evaluate(() => document.documentElement.scrollWidth <= innerWidth));
        const directory = resolve(tmpdir(), 'legura-public-richtext-qa');
        await mkdir(directory, { recursive: true });
        await reader.screenshot({ path: resolve(directory, 'public-' + width + '.png'), fullPage: true });
      }
    } finally {
      await readerContext.close();
    }
  } else {
    assert.equal(await page.getByRole('button', { name: 'Create post', exact: true }).isDisabled(), true);
    assert.equal(await page.locator('[data-editor-document]').isDisabled(), true);
  }

  // Normal plain-text authoring remains available in both browser modes.
  await page.goto(baseUrl + '/admin/posts/new?format=plain-text');
  await page.getByLabel('Title', { exact: true }).fill('Plain post');
  await page.getByLabel('URL slug', { exact: true }).fill('plain-post');
  await page.getByLabel('Content', { exact: true }).fill('<script>literal</script>');
  await page.getByRole('button', { name: 'Create post', exact: true }).click();
  await page.waitForURL(baseUrl + '/admin/posts');
  await page.locator('tr').filter({ hasText: 'Plain post' }).locator('a[href$="/edit"]').first().click();
  await page.waitForURL(/\/admin\/posts\/[^/]+\/edit$/);
  assert.equal(await page.locator('textarea[name="body"]').count(), 1, await page.locator('body').innerText());
  assert.equal(await page.getByLabel('Content', { exact: true }).inputValue(), '<script>literal</script>');
  assert.equal(await page.locator('[data-editor-preview]').count(), 0, 'Plain posts must not be converted');
  const plainId = new URL(page.url()).pathname.split('/')[3];
  await page.getByLabel('Content', { exact: true }).fill('<script>literal</script> Updated');
  const plainNavigation = page.waitForNavigation({ waitUntil: 'load' });
  await page.getByRole('button', { name: 'Save changes', exact: true }).click();
  await plainNavigation;
  assert.equal(await page.getByLabel('Content', { exact: true }).inputValue(), '<script>literal</script> Updated');
  const tokenResponse = await context.request.get(baseUrl + '/__axonyx/csrf', { headers: { Origin: baseUrl } });
  const { token } = await tokenResponse.json();
  const read = await context.request.get(baseUrl + '/api/content-proof?slug=plain-post');
  assert.equal(read.status(), 200);
  assert.ok((await read.json()).html.includes('&lt;script&gt;literal&lt;/script&gt; Updated'));
  const converted = await context.request.post(baseUrl + '/__axonyx/action?path=' + encodeURIComponent('/admin/posts/' + plainId + '/edit') + '&name=UpdatePost', {
    headers: { Origin: baseUrl, 'X-Axonyx-CSRF': token },
    form: { id: plainId, title: 'Changed', slug: 'plain-post', status: 'draft', body: '', content_format: 'legura-richtext', content_document: '{}' },
  });
  assert.equal(converted.status(), 422, 'Implicit plain/rich conversion must be refused');
  const after = await context.request.get(baseUrl + '/api/content-proof?slug=plain-post');
  assert.deepEqual(await after.json(), await read.json());
  if (richId) {
    const flattened = await context.request.post(baseUrl + '/__axonyx/action?path=' + encodeURIComponent('/admin/posts/' + richId + '/edit') + '&name=UpdatePost', {
      headers: { Origin: baseUrl, 'X-Axonyx-CSRF': token },
      form: { id: richId, title: 'Flattened', slug: 'rich-post', status: 'draft', body: 'Forged plain body', content_format: 'plain-text' },
    });
    assert.equal(flattened.status(), 422);
    const saved = await context.request.get(baseUrl + '/api/content-proof?slug=rich-post');
    const { html } = await saved.json();
    assert.ok(html.includes('<strong>Real post</strong>'));
    assert.ok(html.includes('href="https://example.com/post"'));
    const noJs = await browser.newContext({ javaScriptEnabled: false, storageState: await context.storageState() });
    const fallback = await noJs.newPage();
    await fallback.goto(baseUrl + '/admin/posts/' + richId + '/edit');
    assert.equal(await fallback.locator('[data-editor-input]').inputValue(), 'Real post');
    assert.equal(await fallback.getByRole('button', { name: 'Save changes', exact: true }).isDisabled(), true);
    assert.equal(await fallback.locator('[data-editor-document]').isDisabled(), true);
    await noJs.close();
  }
  for (const width of [1280, 390]) {
    await page.setViewportSize({ width, height: 900 });
    assert.ok(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth));
    const directory = resolve(tmpdir(), 'legura-post-editor-qa');
    await mkdir(directory, { recursive: true });
    await page.screenshot({ path: resolve(directory, 'posts-' + mode + '-' + width + '.png'), fullPage: true });
  }
  assert.deepEqual(errors, []);
  console.log('Regular Post editor proof passed (' + mode + '): create/edit/reload, 422 recovery, plain/rich boundaries, no-JS safety and safe stored content.');
} finally {
  await context.close();
  await browser.close();
}
