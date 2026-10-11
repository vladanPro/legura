import assert from 'node:assert/strict';
import { randomUUID } from 'node:crypto';
import { mkdir } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';
import { chromium } from 'playwright';

const [baseUrl, mode] = process.argv.slice(2);
assert.ok(['127.0.0.1', 'localhost'].includes(new URL(baseUrl).hostname));
assert.ok(['javascript', 'native'].includes(mode));
assert.ok(process.env.AX_SECRET_SETUP_TOKEN);
const browser = await chromium.launch();
const context = await browser.newContext({ javaScriptEnabled: mode === 'javascript' });
const page = await context.newPage();
const errors = [];
page.on('pageerror', error => errors.push(error.message));
try {
  await page.goto(`${baseUrl}/setup`);
  await page.getByLabel('Site name', { exact: true }).fill('Editor fixture');
  await page.getByLabel('Administrator email', { exact: true }).fill('editor@example.com');
  await page.getByLabel('Password', { exact: true }).fill(`Editor-fixture-1-${randomUUID()}`);
  await page.getByLabel('Setup token', { exact: true }).fill(process.env.AX_SECRET_SETUP_TOKEN);
  await page.getByRole('button', { name: 'Create installation', exact: true }).click();
  await page.waitForURL(`${baseUrl}/admin`);
  const editUrl = `${baseUrl}/admin/content-editor/edit?slug=editor-proof`;

  if (mode === 'javascript') {
    assert.equal((await page.goto(`${baseUrl}/admin/content-editor`)).status(), 200);
    const surface = page.locator('[data-editor-preview]');
    await surface.waitFor();
    await page.waitForFunction(() => document.querySelector('[data-editor-preview]').contentEditable === 'true');
    await surface.click();
    await page.keyboard.insertText('Browser text');
    await page.keyboard.press('Control+a');
    await page.getByRole('button', { name: 'Bold', exact: true }).click();
    assert.equal(await surface.locator('strong').textContent(), 'Browser text');
    const before = await page.locator('[data-editor-document]').inputValue();
    const failed = page.waitForResponse(r => r.url().includes('/__axonyx/action?') && r.request().method() === 'POST');
    await page.getByRole('button', { name: 'Save content', exact: true }).click();
    assert.equal((await failed).status(), 422);
    await page.locator('[data-ax-field-error="slug"]').getByText('Enter a URL slug.', { exact: true }).waitFor();
    assert.equal(await page.locator('[data-editor-document]').inputValue(), before, '422 lost the editor document');
    assert.equal(await surface.locator('strong').textContent(), 'Browser text');
    await page.getByLabel('URL slug', { exact: true }).fill('editor-proof');
    await page.getByRole('button', { name: 'Save content', exact: true }).click();
    await page.waitForURL(editUrl);
    await page.waitForFunction(() => document.querySelector('[data-editor-preview]').contentEditable === 'true');
    assert.equal(await surface.locator('strong').textContent(), 'Browser text', 'Reload lost formatting');
    await surface.click();
    await page.keyboard.press('Control+End');
    await page.keyboard.insertText(' updated');
    const updated = page.waitForResponse(r => r.url().includes('/__axonyx/action?') && r.request().method() === 'POST');
    const navigation = page.waitForNavigation({ waitUntil: 'load' });
    await page.getByRole('button', { name: 'Save content', exact: true }).click();
    assert.ok([200, 303].includes((await updated).status()));
    await navigation;
    await page.reload();
    await page.waitForFunction(() => document.querySelector('[data-editor-preview]').contentEditable === 'true');
    assert.equal(await surface.textContent(), 'Browser text updated');
    await page.evaluate(() => {
      const root = document.querySelector('[data-legura-editor]');
      window.detachedEditorProbe = root;
      root.replaceWith(root.cloneNode(true));
    });
    await page.waitForFunction(() => window.detachedEditorProbe.querySelector('[data-editor-document]').disabled
      && document.querySelector('[data-editor-preview]').contentEditable === 'true');
    assert.equal(await surface.textContent(), 'Browser text updated', 'Remount lost stored content');
  } else {
    // Seed through the actual authenticated/CSRF-protected action, not direct SQL.
    const proof = await context.request.get(`${baseUrl}/__axonyx/csrf`, { headers: { Origin: baseUrl } });
    const { token } = await proof.json();
    const document = JSON.stringify({ format: 'legura-richtext', version: 1, blocks: [{ type: 'paragraph', children: [{ text: 'Browser text', marks: ['bold'] }] }] });
    const created = await context.request.post(`${baseUrl}/__axonyx/action?path=%2Fadmin%2Fcontent-editor&name=CreateEditorPost`, {
      headers: { Origin: baseUrl, 'X-Axonyx-CSRF': token, Accept: 'text/html' },
      form: { slug: 'editor-proof', content_format: 'legura-richtext', body: 'forged', content_document: document },
      maxRedirects: 0,
    });
    assert.equal(created.status(), 303);
    assert.equal((await page.goto(editUrl)).status(), 200);
    assert.equal(await page.locator('[data-editor-input]').inputValue(), 'Browser text');
    assert.equal(await page.locator('[data-editor-input]').getAttribute('readonly'), 'true');
    assert.equal(await page.locator('[data-editor-document]').isDisabled(), true);
    assert.equal(await page.getByRole('button', { name: 'Save content', exact: true }).isDisabled(), true);
  }
  const result = await context.request.get(`${baseUrl}/api/content-proof?slug=editor-proof`);
  assert.equal(result.status(), 200);
  const { html } = await result.json();
  assert.ok(html.includes('<strong>Browser text'), 'Stored rich formatting did not survive');
  if (mode === 'javascript') assert.ok(html.includes('updated'), 'Update did not persist');
  const directory = resolve(tmpdir(), 'legura-content-editor-qa');
  await mkdir(directory, { recursive: true });
  for (const width of [1280, 390]) {
    await page.setViewportSize({ width, height: 900 });
    assert.ok(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), `Overflow at ${width}px`);
    await page.screenshot({ path: resolve(directory, `editor-${mode}-${width}.png`), fullPage: true });
  }
  assert.deepEqual(errors, []);
  console.log(`Content editor browser passed (${mode}): pinned component/assets, authenticated save/readback, formatting and safe lifecycle.`);
} finally {
  await context.close();
  await browser.close();
}
