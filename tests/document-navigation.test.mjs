import assert from 'node:assert/strict';
import { createServer } from 'node:http';
import { once } from 'node:events';
import { test } from 'node:test';
import { chromium } from 'playwright';
import { waitForDocumentNavigation } from './document-navigation.mjs';

test('same-URL JS save waits for a new document before the next navigation', async () => {
  let generation = 0;
  const server = createServer((request, response) => {
    if (request.method === 'POST' && request.url === '/save') {
      generation += 1;
      response.writeHead(200, { 'Content-Type': 'application/json' });
      return response.end('{}');
    }
    response.writeHead(200, { 'Content-Type': 'text/html' });
    response.end(`<p id="generation">${generation}</p><button>Save</button><script>
      document.querySelector('button').onclick = async () => {
        await fetch('/save', {method: 'POST'});
        window.releaseRedirect = () => location.assign('/editor');
      };
    </script>`);
  });
  server.listen(0, '127.0.0.1');
  await once(server, 'listening');
  let browser;
  try {
    browser = await chromium.launch();
    const page = await browser.newPage();
    const url = `http://127.0.0.1:${server.address().port}/editor`;
    await page.goto(url);
    let committed = false;
    const navigation = waitForDocumentNavigation(page, url).then(() => { committed = true; });
    await page.getByRole('button', { name: 'Save' }).click();
    await page.waitForFunction(() => typeof window.releaseRedirect === 'function');
    assert.equal(committed, false, 'The old matching URL must not complete the save');
    assert.equal(await page.locator('#generation').textContent(), '0');
    await page.evaluate(() => window.releaseRedirect());
    await navigation;
    assert.equal(await page.locator('#generation').textContent(), '1');
    assert.equal(committed, true);
    // The next page transition now cannot race the delayed save redirect.
    assert.equal((await page.goto(url.replace('/editor', '/admin'))).status(), 200);
  } finally {
    await browser?.close();
    await new Promise(resolve => server.close(resolve));
  }
});

test('native form save also waits for the redirected document', async () => {
  const server = createServer((request, response) => {
    if (request.method === 'POST') {
      response.writeHead(303, { Location: '/done' });
      return response.end();
    }
    response.writeHead(200, { 'Content-Type': 'text/html' });
    response.end(request.url === '/done' ? '<h1>Saved</h1>' : '<form method="post" action="/save"><button>Save</button></form>');
  });
  server.listen(0, '127.0.0.1');
  await once(server, 'listening');
  let browser;
  try {
    browser = await chromium.launch();
    const page = await browser.newPage({ javaScriptEnabled: false });
    const base = `http://127.0.0.1:${server.address().port}`;
    await page.goto(base + '/editor');
    const navigation = waitForDocumentNavigation(page, base + '/done');
    await page.getByRole('button', { name: 'Save' }).click();
    await navigation;
    assert.equal(await page.getByRole('heading').textContent(), 'Saved');
  } finally {
    await browser?.close();
    await new Promise(resolve => server.close(resolve));
  }
});
