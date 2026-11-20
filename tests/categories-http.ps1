# Dot-sourced in the disposable -Users fixture, never an operator database.
$categoryCreate = "/__axonyx/action?path=%2Fadmin%2Fcategories&name=CreateCategory"
Invoke-Request -Path "/admin/categories" -Status 403 | Out-Null
Invoke-Request -Path $categoryCreate -Method POST -Body "name=News&slug=news" -Headers (Get-Proof) -Status 403 | Out-Null
Invoke-Request -Path $categoryCreate -Method POST -Body "name=News&slug=news" -Headers @{ Cookie = $cookie; Origin = $baseUrl } -Status 403 | Out-Null
$categoryIndex = Invoke-Request -Path "/admin/categories" -Headers @{ Cookie = $cookie }
if ($categoryIndex.Body -notmatch '<a\b(?=[^>]*href="/admin/categories")(?=[^>]*data-active="true")') { throw "Categories sidebar selection is missing" }
foreach ($invalidCategory in @("name=&slug=valid", "name=Valid&slug=Bad%2Fslug", "name=%20Leading&slug=valid", "name=Valid&slug=a%00b")) {
  Invoke-Request -Path $categoryCreate -Method POST -Body $invalidCategory -Headers $sessionProof -Status 422 | Out-Null
}
$categoryForm = "name=Pilot%20%3Cb%3ENews%3C%2Fb%3E&slug=pilot-news"
Invoke-Request -Path $categoryCreate -Method POST -Body $categoryForm -Headers $sessionProof -Status 303 | Out-Null
$categoryDuplicate = Invoke-Request -Path $categoryCreate -Method POST -Body $categoryForm -Headers $sessionProof -Status 422
if ($categoryDuplicate.Body -notmatch 'This category slug is already in use' -or $categoryDuplicate.Body -notmatch 'data-ax-field-error="slug"') { throw "Category duplicate feedback is missing" }
$categorySearch = Invoke-Request -Path "/admin/categories?q=pilot-news" -Headers @{ Cookie = $cookie }
$categoryMatch = [regex]::Match($categorySearch.Body, 'href="(/admin/categories/[a-f0-9-]{36})"')
if (!$categoryMatch.Success -or $categorySearch.Body.Contains('Pilot <b>News</b>') -or !$categorySearch.Body.Contains('&lt;b&gt;News&lt;/b&gt;')) { throw "Category was not listed safely" }
$categoryRoute = $categoryMatch.Groups[1].Value
$categoryId = Split-Path $categoryRoute -Leaf
$categoryUpdate = "/__axonyx/action?path=$([Uri]::EscapeDataString($categoryRoute))&name=UpdateCategory"
$categoryDelete = "/__axonyx/action?path=$([Uri]::EscapeDataString($categoryRoute + '/delete'))&name=DeleteCategory"
Invoke-Request -Path "/admin/categories/missing" -Headers @{ Cookie = $cookie } -Status 404 | Out-Null
Invoke-Request -Path $categoryUpdate -Method POST -Body "id=missing&name=Valid&slug=valid" -Headers $sessionProof -Status 404 | Out-Null
Invoke-Request -Path $categoryUpdate -Method POST -Body "id=$categoryId&name=Renamed&slug=renamed-pilot" -Headers $sessionProof -Status 303 | Out-Null
$categoryEdited = Invoke-Request -Path $categoryRoute -Headers @{ Cookie = $cookie }
if (!$categoryEdited.Body.Contains('value="Renamed"') -or !$categoryEdited.Body.Contains('value="renamed-pilot"')) { throw "Category edit was not persisted" }
& $python.Source -c 'import sqlite3,sys;d=sqlite3.connect(sys.argv[1]);d.execute("INSERT INTO legura_posts(id,title,slug,body,status) VALUES (?,?,?,?,?)",("category-proof","Preserved post","category-proof","Original body","published"));d.execute("INSERT INTO legura_post_categories VALUES (?,?)",("category-proof",sys.argv[2]));d.executemany("INSERT INTO legura_categories VALUES (?,?,?)",[(f"category-page-{i}",f"Page fixture {i:02}",f"page-fixture-{i:02}") for i in range(12)]);d.commit();d.close()' $db $categoryId
if ($LASTEXITCODE -ne 0) { throw "Cannot prepare category assignment/pagination fixture" }
. (Join-Path $root "tests/post-categories-http.ps1")
. (Join-Path $root "tests/post-category-filter-http.ps1")
$categoryPage = Invoke-Request -Path "/admin/categories?q=page-fixture&page=999" -Headers @{ Cookie = $cookie }
if (!$categoryPage.Body.Contains('12 matching categories') -or !$categoryPage.Body.Contains('Page 2') -or $categoryPage.Body -notmatch '<input(?=[^>]*name="q")(?=[^>]*value="page-fixture")') { throw "Category search/pagination lost filtering or page clamp" }
$categoryLiteral = Invoke-Request -Path "/admin/categories?q=%25" -Headers @{ Cookie = $cookie }
if (!$categoryLiteral.Body.Contains('No matching categories')) { throw "Category search treated percent as a wildcard" }
$categoryConfirmation = Invoke-Request -Path ($categoryRoute + '/delete') -Headers @{ Cookie = $cookie }
if (!$categoryConfirmation.Body.Contains('1 assigned posts') -or !$categoryConfirmation.Body.Contains('not its posts')) { throw "Category confirmation lost assignment count or safety explanation" }
Invoke-Request -Path $categoryDelete -Method POST -Body "id=$categoryId&confirmSlug=pilot-news" -Headers $sessionProof -Status 422 | Out-Null
Invoke-Request -Path $categoryRoute -Headers @{ Cookie = $cookie } | Out-Null
Invoke-Request -Path $categoryDelete -Method POST -Body "id=$categoryId&confirmSlug=renamed-pilot" -Headers $sessionProof -Status 303 | Out-Null
Invoke-Request -Path $categoryRoute -Headers @{ Cookie = $cookie } -Status 404 | Out-Null
$deletedCategoryFilter = Invoke-Request -Path "/admin/posts?category=$categoryId" -Headers @{ Cookie = $cookie }
if (!$deletedCategoryFilter.Body.Contains('0 matching posts') -or !$deletedCategoryFilter.Body.Contains('Unavailable category')) { throw "Deleted category filter exposed unfiltered posts" }
Invoke-Request -Path $categoryDelete -Method POST -Body "id=$categoryId&confirmSlug=renamed-pilot" -Headers $sessionProof -Status 404 | Out-Null
& $python.Source -c 'import sqlite3,sys;d=sqlite3.connect(sys.argv[1]);assert d.execute("SELECT body,status FROM legura_posts WHERE id=?",("category-proof",)).fetchone()==("Original body","published");assert d.execute("SELECT COUNT(*) FROM legura_post_categories WHERE category_id=?",(sys.argv[2],)).fetchone()[0]==0;d.execute("DELETE FROM legura_posts WHERE id=?",("category-proof",));d.execute("DELETE FROM legura_categories WHERE id LIKE ?",("category-page-%",));d.commit();d.close()' $db $categoryId
if ($LASTEXITCODE -ne 0) { throw "Category deletion changed its post or retained assignments" }
Write-Host "Legura Categories HTTP passed: admin guards, CSRF, validation, escaped listing, search/pagination, edit, confirmed delete and preserved posts."
