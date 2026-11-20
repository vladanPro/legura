# Read-only public routes, tested only in the disposable Categories fixture.
if (Test-Path -LiteralPath (Join-Path $fixture 'dist/categories/index.html')) { throw 'Database-backed category catalog was emitted as static HTML' }
$publicCategorySeed = @'
import json, sqlite3, sys
d = sqlite3.connect(sys.argv[1])
saved = {"appearance":d.execute("SELECT theme,color_mode,palette FROM legura_public_theme WHERE id=1").fetchone(),"language":d.execute("SELECT interface_language,interface_switcher,site_language FROM legura_languages WHERE id=1").fetchone()}
d.execute("UPDATE legura_public_theme SET theme='editorial',color_mode='dark',palette='gold' WHERE id=1")
d.execute("UPDATE legura_languages SET interface_language='en',interface_switcher=1,site_language='sr-Latn' WHERE id=1")
categories = [("public-cat-main","AAA Public <b>Main</b>","public-main"),("public-cat-shared","AAA Public Shared","public-shared"),("public-cat-draft","PRIVATE draft-only category","private-draft-category"),("public-cat-trash","PRIVATE trash-only category","private-trash-category"),("public-cat-empty","PRIVATE empty category","private-empty-category")]
categories += [(f"public-cat-index-{i}",f"AAA Public index {i:02}",f"public-index-{i:02}") for i in range(10)]
d.executemany("INSERT INTO legura_categories VALUES (?,?,?)",categories)
for i in range(12):
    id = f"public-post-{i}"
    d.execute("INSERT INTO legura_posts(id,title,slug,body,status) VALUES (?,?,?,?,?)",(id,f"Public archive {i:02}",f"public-archive-{i:02}",f"PUBLIC body {i:02}","published"))
    d.execute("INSERT INTO legura_post_categories VALUES (?,?)",(id,"public-cat-main"))
for category in ["public-cat-shared"] + [f"public-cat-index-{i}" for i in range(10)]:
    d.execute("INSERT INTO legura_post_categories VALUES (?,?)",("public-post-0",category))
for suffix, trash in [("draft",0),("trash",1)]:
    id = f"public-post-{suffix}"
    d.execute("INSERT INTO legura_posts(id,title,slug,body,status,trashed) VALUES (?,?,?,?,?,?)",(id,f"PRIVATE {suffix} title",f"private-{suffix}-post",f"PRIVATE {suffix} body","draft",trash))
    d.executemany("INSERT INTO legura_post_categories VALUES (?,?)",[(id,"public-cat-main"),(id,f"public-cat-{suffix}")])
d.commit()
d.close()
print(json.dumps(saved))
'@
$publicSaved = & $python.Source -c $publicCategorySeed $db
if ($LASTEXITCODE -ne 0) { throw 'Cannot seed public category fixture' }
function Assert-PublicCategoryResponse {
  param($Response)
  if ($Response.Body -match 'PRIVATE|password_hash|session_generation|legura-admin-shell' -or
      !$Response.Body.Contains('data-legura-public-theme="editorial"') -or
      !$Response.Body.Contains('data-foundry-mode="dark"') -or !$Response.Body.Contains('data-foundry="gold"') -or
      $Response.Headers['Cache-Control'] -notmatch 'no-store') {
    throw 'Public category response exposed private data, lost appearance or allowed stale caching'
  }
}
$publicIndex = Invoke-Request -Path '/categories'
Assert-PublicCategoryResponse $publicIndex
if (!$publicIndex.Body.Contains('&lt;b&gt;Main&lt;/b&gt;') -or $publicIndex.Body.Contains('AAA Public <b>Main</b>') -or
    [regex]::Matches($publicIndex.Body, 'href="/categories/public-[^"]+"').Count -ne 10 -or !$publicIndex.Body.Contains('Page 1 / 2')) {
  throw 'Public catalog did not escape names or paginate visible categories'
}
$publicIndexLast = Invoke-Request -Path '/categories?page=999&lang=fr'
Assert-PublicCategoryResponse $publicIndexLast
if (!$publicIndexLast.Body.Contains('Toutes les catégories') -and !$publicIndexLast.Body.Contains('Catégories')) { throw 'Public catalog did not resolve French labels' }
if (!$publicIndexLast.Body.Contains('Page 2 / 2') -or $publicIndexLast.Body -notmatch '<input\b(?=[^>]*name="lang")(?=[^>]*value="fr")') { throw 'Catalog final page or language retention failed' }
$publicArchive = Invoke-Request -Path '/categories/public-main?lang=fr'
Assert-PublicCategoryResponse $publicArchive
if (!$publicArchive.Body.Contains('Articles publiés: 12') -or !$publicArchive.Body.Contains('Page suivante') -or
    [regex]::Matches($publicArchive.Body, 'href="/posts/public-archive-[^"]+"').Count -ne 10 -or
    !$publicArchive.Body.Contains('/posts/public-archive-00?lang=fr') -or
    $publicArchive.Body -notmatch '<h2\b[^>]*lang="sr-Latn"') { throw 'Archive count, locale, content language or ten-row limit failed' }
$publicArchiveLast = Invoke-Request -Path '/categories/public-main?page=999&lang=fr'
Assert-PublicCategoryResponse $publicArchiveLast
if (!$publicArchiveLast.Body.Contains('Page 2 / 2') -or [regex]::Matches($publicArchiveLast.Body, 'href="/posts/public-archive-[^"]+"').Count -ne 2 -or
    !$publicArchiveLast.Body.Contains('/posts/public-archive-11?lang=fr') -or $publicArchiveLast.Body -notmatch '<input\b(?=[^>]*name="lang")(?=[^>]*value="fr")') { throw 'Archive final page or language retention failed' }
foreach ($path in @('/categories/missing','/categories/private-empty-category','/categories/private-draft-category','/categories/private-trash-category')) {
  $hiddenCategory = Invoke-Request -Path $path -Status 404
  if (!$hiddenCategory.Body.Contains('This category is not available.') -or $hiddenCategory.Body -match 'PRIVATE|legura-admin-shell') { throw 'Category 404 disclosed private metadata or lost its public fallback' }
}
$publicStory = Invoke-Request -Path '/posts/public-archive-00?lang=fr'
if (!$publicStory.Body.Contains('/categories/public-main?lang=fr') -or $publicStory.Body -match 'PRIVATE') { throw 'Published story did not expose safe localized category links' }
$publicPosts = Invoke-Request -Path '/posts?lang=fr'
if (!$publicPosts.Body.Contains('/categories?lang=fr')) { throw 'Published posts did not link to the category catalog' }
& $python.Source -c 'import sqlite3,sys;d=sqlite3.connect(sys.argv[1]);d.execute("UPDATE legura_posts SET status=?,trashed=1 WHERE id=?",("draft","public-post-0"));d.commit();d.close()' $db
if ($LASTEXITCODE -ne 0) { throw 'Cannot trash public fixture post' }
$afterTrash = Invoke-Request -Path '/categories/public-main'
if (!$afterTrash.Body.Contains('Published posts: 11') -or $afterTrash.Body.Contains('Public archive 00')) { throw 'Trash stayed in the public category archive' }
Invoke-Request -Path '/categories/public-shared' -Status 404 | Out-Null
& $python.Source -c 'import sqlite3,sys;d=sqlite3.connect(sys.argv[1]);d.execute("UPDATE legura_posts SET trashed=0 WHERE id=?",("public-post-0",));d.commit();d.close()' $db
if ($LASTEXITCODE -ne 0) { throw 'Cannot restore public fixture draft' }
$afterRestore = Invoke-Request -Path '/categories/public-main'
if (!$afterRestore.Body.Contains('Published posts: 11')) { throw 'Restoring Trash implicitly republished a category post' }
& $python.Source -c 'import sqlite3,sys;d=sqlite3.connect(sys.argv[1]);d.execute("UPDATE legura_posts SET status=? WHERE id=?",("published","public-post-0"));d.commit();d.close()' $db
if ($LASTEXITCODE -ne 0) { throw 'Cannot republish public fixture post' }
$publicRename = '/__axonyx/action?path=%2Fadmin%2Fcategories%2Fpublic-cat-main&name=UpdateCategory'
Invoke-Request -Path $publicRename -Method POST -Body 'id=public-cat-main&name=AAA%20Public%20Main&slug=public-main-renamed' -Headers $sessionProof -Status 303 | Out-Null
Invoke-Request -Path '/categories/public-main' -Status 404 | Out-Null
Invoke-Request -Path '/categories/public-main-renamed' | Out-Null
& $python.Source -c 'import sqlite3,sys;d=sqlite3.connect(sys.argv[1]);d.execute("UPDATE legura_posts SET status=? WHERE id LIKE ?",("draft","public-post-%"));d.commit();d.close()' $db
if ($LASTEXITCODE -ne 0) { throw 'Cannot unpublish public fixture posts' }
Invoke-Request -Path '/categories/public-main-renamed' -Status 404 | Out-Null
$afterUnpublish = Invoke-Request -Path '/categories'
if ($afterUnpublish.Body.Contains('/categories/public-')) { throw 'Unpublished category metadata stayed in the public catalog' }
& $python.Source -c 'import sqlite3,sys;d=sqlite3.connect(sys.argv[1]);d.execute("UPDATE legura_posts SET status=? WHERE id=?",("published","public-post-0"));d.commit();d.close()' $db
if ($LASTEXITCODE -ne 0) { throw 'Cannot republish one public fixture post' }
$publicDelete = '/__axonyx/action?path=%2Fadmin%2Fcategories%2Fpublic-cat-main%2Fdelete&name=DeleteCategory'
Invoke-Request -Path $publicDelete -Method POST -Body 'id=public-cat-main&confirmSlug=public-main-renamed' -Headers $sessionProof -Status 303 | Out-Null
Invoke-Request -Path '/categories/public-main-renamed' -Status 404 | Out-Null
$afterDelete = Invoke-Request -Path '/posts/public-archive-00'
if (!$afterDelete.Body.Contains('PUBLIC body 00') -or $afterDelete.Body.Contains('/categories/public-main-renamed')) { throw 'Deleting a public category deleted the post or retained a dead link' }
$publicCleanup = @'
import json, sqlite3, sys
d = sqlite3.connect(sys.argv[1])
saved = json.loads(sys.argv[2])
d.execute("DELETE FROM legura_posts WHERE id LIKE ?",("public-post-%",))
d.execute("DELETE FROM legura_categories WHERE id LIKE ?",("public-cat-%",))
d.execute("UPDATE legura_public_theme SET theme=?,color_mode=?,palette=? WHERE id=1",saved["appearance"])
d.execute("UPDATE legura_languages SET interface_language=?,interface_switcher=?,site_language=? WHERE id=1",saved["language"])
d.commit()
d.close()
'@
& $python.Source -c $publicCleanup $db $publicSaved
if ($LASTEXITCODE -ne 0) { throw 'Cannot restore disposable public appearance/language fixture' }
Write-Host 'Legura public categories HTTP passed: projection/privacy, published-only counts, paging, locale/theme, safe 404, live unpublish/Trash, slug changes and non-destructive deletion.'
