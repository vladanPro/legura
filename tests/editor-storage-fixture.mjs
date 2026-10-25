// Deliberate corruption is allowed only inside smoke-auth's disposable database.
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { realpathSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { basename, dirname, sep } from 'node:path';

function storage(db, slug) {
  const path = realpathSync(db);
  const temporary = realpathSync(tmpdir()) + sep;
  assert.ok(path.toLowerCase().startsWith(temporary.toLowerCase()), 'Only temporary fixtures may be changed');
  assert.equal(basename(path), 'legura.db');
  assert.equal(basename(dirname(path)), 'data');
  assert.match(basename(dirname(dirname(path))), /^legura-auth-[a-f0-9]{32}$/);
  const python = `import sqlite3,sys,json
d=sqlite3.connect(sys.argv[1])
if sys.argv[3]=='write':
    row=json.loads(sys.argv[4])
    assert d.execute("update legura_posts set body=?,content_format=?,content_document=? where slug=?",(*row,sys.argv[2])).rowcount==1
    d.commit()
row=d.execute("select body,content_format,content_document from legura_posts where slug=?",(sys.argv[2],)).fetchone()
assert row is not None
print(json.dumps(row))
d.close()`;
  return (operation = 'read', row = null) => {
    const result = spawnSync('python', ['-c', python, path, slug, operation, JSON.stringify(row)], { encoding: 'utf8' });
    assert.equal(result.status, 0, result.stderr || 'Fixture SQLite operation failed');
    return JSON.parse(result.stdout);
  };
}

export async function assertStoredEditorBoundary(page, db, slug, editUrl) {
  const run = storage(db, slug);
  const original = run();
  assert.equal(original[1], 'legura-richtext');
  const document = JSON.parse(original[2]);
  const canonical = JSON.stringify(document);
  const anonymous = await page.context().browser().newContext({ javaScriptEnabled: false });
  try {
    run('write', [original[0], original[1], JSON.stringify(document, null, 2)]);
    const before = run();
    assert.equal((await page.goto(editUrl)).status(), 200);
    assert.equal(await page.locator('[data-legura-editor]').getAttribute('data-document'), canonical, 'Loader did not canonicalize stored JSON');
    assert.deepEqual(run(), before, 'Reading an editor must not rewrite stored content');
    const literal = '</textarea><script>globalThis.editorStorageInjection=1</script>';
    const literalDocument = JSON.stringify({ format: 'legura-richtext', version: 1, blocks: [{ type: 'paragraph', children: [{ text: literal, marks: [] }] }] });
    run('write', [literal, original[1], literalDocument]);
    assert.equal((await page.goto(editUrl)).status(), 200);
    assert.equal(JSON.parse(await page.locator('[data-legura-editor]').getAttribute('data-document')).blocks[0].children[0].text, literal);
    assert.equal(await page.locator('[data-legura-editor] script').count(), 0, 'Stored text escaped the editor field');
    assert.equal(await page.evaluate(() => globalThis.editorStorageInjection), undefined);
    assert.deepEqual(run(), [literal, original[1], literalDocument]);
    const unsafe = JSON.stringify({ format: 'legura-richtext', version: 1, blocks: [{ type: 'paragraph', children: [{ text: 'private-storage-marker', marks: [], link: 'javascript:alert(1)' }] }] });
    for (const row of [
      [original[0], original[1], '{}'],
      [original[0], original[1], JSON.stringify({ ...document, version: 999 })],
      [original[0], original[1], unsafe],
      ['private-storage-marker', original[1], original[2]],
    ]) {
      run('write', row);
      assert.equal((await anonymous.request.get(editUrl)).status(), 403, 'Auth must precede storage validation');
      const response = await page.goto(editUrl);
      assert.equal(response.status(), 500, 'Corrupt storage must fail before editor mount');
      assert.equal(await page.locator('[data-legura-editor]').count(), 0);
      assert.equal(await page.locator('script[src*="content-editor"]').count(), 0);
      assert.ok(!(await response.text()).includes('private-storage-marker'));
      assert.deepEqual(run(), row, 'Rejected storage was silently repaired');
    }
  } finally {
    run('write', original);
    await anonymous.close();
  }
  assert.deepEqual(run(), original);
  assert.equal((await page.goto(editUrl)).status(), 200, 'Restored storage must load again');
}

export async function assertLegacyPlainEditor(page, db, slug, editUrl) {
  const run = storage(db, slug);
  const original = run();
  assert.equal(original[1], 'plain-text');
  try {
    const legacy = ['\t\n ', 'plain-text', null];
    run('write', legacy);
    assert.equal((await page.goto(editUrl)).status(), 200);
    assert.equal(await page.locator('textarea[name="body"]').inputValue(), legacy[0]);
    assert.equal(await page.locator('[data-legura-editor]').count(), 0);
    assert.deepEqual(run(), legacy, 'Reading changed a legacy plain-text post');
  } finally {
    run('write', original);
  }
  assert.equal((await page.goto(editUrl)).status(), 200);
}
