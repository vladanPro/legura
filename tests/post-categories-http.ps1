# Runs inside the Categories fixture before its linked category is deleted.
$selectionRoute = "/admin/posts/category-proof/categories"
$selectionAction = "/__axonyx/action?path=$([Uri]::EscapeDataString($selectionRoute))&name=SetPostCategory"
Invoke-Request -Path $selectionRoute -Status 403 | Out-Null
$selection = Invoke-Request -Path ($selectionRoute + '?q=renamed-pilot') -Headers @{ Cookie = $cookie }
if (!$selection.Body.Contains('1 categories assigned to this post.') -or !$selection.Body.Contains('Category assignment: Renamed')) {
  throw "Saved category assignment was not displayed"
}
Invoke-Request -Path $selectionAction -Method POST -Body "id=category-proof&category_id=$categoryId&assigned=0" -Headers @{ Cookie = $cookie; Origin = $baseUrl } -Status 403 | Out-Null
& $python.Source -c 'import sqlite3,sys;d=sqlite3.connect(sys.argv[1]);d.execute("INSERT INTO legura_posts(id,title,slug,body,status) VALUES (?,?,?,?,?)",("category-proof-other","Other","category-proof-other","Other body","draft"));d.execute("INSERT INTO legura_post_categories VALUES (?,?)",("category-proof-other",sys.argv[2]));d.commit();d.close()' $db $categoryId
if ($LASTEXITCODE -ne 0) { throw "Cannot prepare independent assignment" }
foreach ($selectionRole in @('editor','subscriber')) {
  $selectionEmail = "categories-$selectionRole@example.test"
  $selectionPassword = 'categories-' + [guid]::NewGuid().ToString('N')
  Invoke-Request -Path "/__axonyx/action?path=%2Fadmin%2Fusers&name=CreateUser" -Method POST -Body "email=$([Uri]::EscapeDataString($selectionEmail))&password=$selectionPassword&role=$selectionRole" -Headers $sessionProof -Status 303 | Out-Null
  $selectionLogin = Invoke-Request -Path "/__axonyx/action?path=%2Flogin&name=SignIn" -Method POST -Body "email=$([Uri]::EscapeDataString($selectionEmail))&password=$selectionPassword" -Headers (Get-Proof) -Status 303
  $selectionCookie = ([string]$selectionLogin.Headers['Set-Cookie']).Split(';')[0]
  $selectionProof = Get-Proof -Cookie $selectionCookie
  if ($selectionRole -eq 'subscriber') {
    Invoke-Request -Path $selectionRoute -Headers @{ Cookie = $selectionCookie } -Status 403 | Out-Null
    Invoke-Request -Path $selectionAction -Method POST -Body "id=category-proof&category_id=$categoryId&assigned=0&actor_id=$ownerId" -Headers $selectionProof -Status 403 | Out-Null
    continue
  }
  $editorSelection = Invoke-Request -Path $selectionRoute -Headers @{ Cookie = $selectionCookie }
  if ($editorSelection.Body -match 'href="/admin/categories"') { throw "Editor was offered category administration" }
  foreach ($attempt in 1..2) {
    Invoke-Request -Path $selectionAction -Method POST -Body "id=category-proof&category_id=category-page-0&assigned=1" -Headers $selectionProof -Status 303 | Out-Null
  }
  $multiple = Invoke-Request -Path $selectionRoute -Headers @{ Cookie = $selectionCookie }
  if (!$multiple.Body.Contains('2 categories assigned to this post.')) { throw "Add duplicated an assignment or replaced an existing choice" }
  foreach ($attempt in 1..2) {
    Invoke-Request -Path $selectionAction -Method POST -Body "id=category-proof&category_id=category-page-0&assigned=0" -Headers $selectionProof -Status 303 | Out-Null
  }
  $pagedSelection = Invoke-Request -Path ($selectionRoute + '?q=page-fixture&page=999') -Headers @{ Cookie = $selectionCookie }
  if (!$pagedSelection.Body.Contains('Page 2')) { throw "Selection pagination did not clamp" }
  $preservedSelection = Invoke-Request -Path $selectionAction -Method POST -Body "id=category-proof&category_id=category-page-10&assigned=1&page=2&search=page-fixture" -Headers $selectionProof -Status 303
  if ($preservedSelection.Headers.Location -ne ($selectionRoute + '?page=2&q=page-fixture')) { throw "Assignment lost category search or page: $($preservedSelection.Headers.Location)" }
  Invoke-Request -Path $selectionAction -Method POST -Body "id=category-proof&category_id=category-page-10&assigned=0&page=2&search=page-fixture" -Headers $selectionProof -Status 303 | Out-Null
  Invoke-Request -Path $selectionAction -Method POST -Body "id=category-proof&category_id=missing&assigned=1" -Headers $selectionProof -Status 422 | Out-Null
  Invoke-Request -Path $selectionAction -Method POST -Body "id=category-proof&category_id=$categoryId&assigned=2" -Headers $selectionProof -Status 422 | Out-Null
  Invoke-Request -Path $selectionAction -Method POST -Body "id=missing&category_id=$categoryId&assigned=1" -Headers $selectionProof -Status 404 | Out-Null
  & $python.Source -c 'import sqlite3,sys;d=sqlite3.connect(sys.argv[1]);d.execute("UPDATE legura_posts SET status=?,trashed=1 WHERE id=?",("draft","category-proof-other"));d.commit();d.close()' $db
  if ($LASTEXITCODE -ne 0) { throw "Cannot trash fixture post" }
  Invoke-Request -Path "/admin/posts/category-proof-other/categories" -Headers @{ Cookie = $selectionCookie } -Status 404 | Out-Null
  Invoke-Request -Path $selectionAction -Method POST -Body "id=category-proof-other&category_id=$categoryId&assigned=0" -Headers $selectionProof -Status 404 | Out-Null
  & $python.Source -c 'import sqlite3,sys;d=sqlite3.connect(sys.argv[1]);d.execute("UPDATE legura_users SET role=? WHERE email=?",("subscriber",sys.argv[2]));d.commit();d.close()' $db $selectionEmail
  if ($LASTEXITCODE -ne 0) { throw "Cannot demote fixture Editor" }
  Invoke-Request -Path $selectionRoute -Headers @{ Cookie = $selectionCookie } -Status 403 | Out-Null
  Invoke-Request -Path $selectionAction -Method POST -Body "id=category-proof&category_id=$categoryId&assigned=0" -Headers $selectionProof -Status 403 | Out-Null
}
& $python.Source -c 'import sqlite3,sys;d=sqlite3.connect(sys.argv[1]);assert d.execute("SELECT COUNT(*) FROM legura_post_categories WHERE post_id=?",("category-proof",)).fetchone()[0]==1;assert d.execute("SELECT category_id FROM legura_post_categories WHERE post_id=?",("category-proof-other",)).fetchone()[0]==sys.argv[2];assert d.execute("SELECT body,status FROM legura_posts WHERE id=?",("category-proof",)).fetchone()==("Original body","published");d.execute("DELETE FROM legura_posts WHERE id=?",("category-proof-other",));d.commit();d.close()' $db $categoryId
if ($LASTEXITCODE -ne 0) { throw "Category assignment changed independent links or post content/status" }
Write-Host "Legura post-category HTTP passed: Admin/Editor selection, multiple/idempotent links, scoped removal, preserved search/page, CSRF, Subscriber/demoted denial and Trash safety."
