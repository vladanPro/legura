# Disposable Categories fixture only; categoryId and the two role cookies are server-test values.
$filterSetup = @'
import sqlite3, sys
d = sqlite3.connect(sys.argv[1])
category = sys.argv[2]
for i in range(12):
    id = f"category-filter-{i}"
    title = f"Filter proof {i:02}" + ("%" if i == 11 else "")
    d.execute("INSERT INTO legura_posts(id,title,slug,body,status) VALUES (?,?,?,?,?)", (id,title,id,"Body","published"))
    d.executemany("INSERT INTO legura_post_categories VALUES (?,?)", [(id,category),(id,"category-page-0")])
for suffix, status, trash, assigned in [("draft","draft",0,category),("trash","draft",1,category),("uncategorized","published",0,None),("other","published",0,"category-page-0")]:
    id = f"category-filter-{suffix}"
    d.execute("INSERT INTO legura_posts(id,title,slug,body,status,trashed) VALUES (?,?,?,?,?,?)", (id,f"Filter proof {suffix}",id,"Body",status,trash))
    if assigned:
        d.execute("INSERT INTO legura_post_categories VALUES (?,?)", (id,assigned))
d.commit()
d.close()
'@
& $python.Source -c $filterSetup $db $categoryId
if ($LASTEXITCODE -ne 0) { throw "Cannot seed category post-filter fixture" }
$filterQuery = "/admin/posts?category=$categoryId&q=Filter%20proof"
Invoke-Request -Path $filterQuery -Status 403 | Out-Null
Invoke-Request -Path $filterQuery -Headers @{ Cookie = $selectionCookie } -Status 403 | Out-Null
$filterEmail = 'category-filter-editor@example.test'
$filterPassword = 'filter-editor-' + [guid]::NewGuid().ToString('N')
Invoke-Request -Path '/__axonyx/action?path=%2Fadmin%2Fusers&name=CreateUser' -Method POST -Body "email=$filterEmail&password=$filterPassword&role=editor" -Headers $sessionProof -Status 303 | Out-Null
$filterLogin = Invoke-Request -Path '/__axonyx/action?path=%2Flogin&name=SignIn' -Method POST -Body "email=$filterEmail&password=$filterPassword" -Headers (Get-Proof) -Status 303
$filterEditorCookie = ([string]$filterLogin.Headers['Set-Cookie']).Split(';')[0]
foreach ($filterCookie in @($cookie, $filterEditorCookie)) {
  $headers = @{ Cookie = $filterCookie }
  $first = Invoke-Request -Path ($filterQuery + '&status=published') -Headers $headers
  $firstRows = [regex]::Match($first.Body, '(?s)<tbody\b[^>]*>(.*?)</tbody>').Groups[1].Value
  if (!$first.Body.Contains('12 matching posts') -or [regex]::Matches($firstRows, '<tr\b').Count -ne 10 -or
      !$firstRows.Contains('Filter proof 00') -or $firstRows.Contains('Filter proof draft') -or $firstRows.Contains('Filter proof other') -or
      $first.Body -notmatch ('<option\b(?=[^>]*value="' + [regex]::Escape($categoryId) + '")(?=[^>]*selected)') -or
      $first.Body -notmatch ('<input\b(?=[^>]*name="category")(?=[^>]*value="' + [regex]::Escape($categoryId) + '")')) {
    throw "Category/status/title filtering lost selection, duplicated rows or paginated before filtering"
  }
  $last = Invoke-Request -Path ($filterQuery + '&status=published&page=999') -Headers $headers
  $lastRows = [regex]::Match($last.Body, '(?s)<tbody\b[^>]*>(.*?)</tbody>').Groups[1].Value
  if (!$last.Body.Contains('Page 2') -or [regex]::Matches($lastRows, '<tr\b').Count -ne 2 -or !$lastRows.Contains('Filter proof 11%') -or
      $last.Body -notmatch '<input\b(?=[^>]*name="q")(?=[^>]*value="Filter proof")' -or
      $last.Body -notmatch '<input\b(?=[^>]*name="status")(?=[^>]*value="published")' -or
      $last.Body -notmatch ('<input\b(?=[^>]*name="category")(?=[^>]*value="' + [regex]::Escape($categoryId) + '")')) {
    throw "Category-filtered last page lost filters or final-page clamping"
  }
  foreach ($case in @(@{ Status='all'; Count=13 }, @{ Status='draft'; Count=1 }, @{ Status='trash'; Count=1 })) {
    $result = Invoke-Request -Path ($filterQuery + '&status=' + $case.Status) -Headers $headers
    if (!$result.Body.Contains("$($case.Count) matching posts")) { throw "Category was not combined with collection/status" }
  }
  $literal = Invoke-Request -Path "/admin/posts?category=$categoryId&q=%25" -Headers $headers
  if (!$literal.Body.Contains('1 matching posts') -or !$literal.Body.Contains('Filter proof 11%')) { throw "Category search interpreted percent as a wildcard" }
  foreach ($query in @('_', "' OR 1=1 --")) {
    $empty = Invoke-Request -Path ("/admin/posts?category=$categoryId&q=" + [Uri]::EscapeDataString($query)) -Headers $headers
    if (!$empty.Body.Contains('0 matching posts') -or !$empty.Body.Contains('No matching posts')) { throw "Category search stopped being literal/bound" }
  }
  $unknown = Invoke-Request -Path '/admin/posts?category=missing-category' -Headers $headers
  if (!$unknown.Body.Contains('0 matching posts') -or !$unknown.Body.Contains('Unavailable category') -or !$unknown.Body.Contains('No matching posts') -or $unknown.Body.Contains('No posts yet')) {
    throw "Unknown category silently fell back to unfiltered results or wrong empty state"
  }
  Invoke-Request -Path ('/admin/posts?category=' + ('x' * 129)) -Headers $headers -Status 404 | Out-Null
}
& $python.Source -c 'import sqlite3,sys;d=sqlite3.connect(sys.argv[1]);d.execute("DELETE FROM legura_posts WHERE id LIKE ?",("category-filter-%",));d.commit();d.close()' $db
if ($LASTEXITCODE -ne 0) { throw "Cannot clean disposable category-filter rows" }
Write-Host "Legura category post filter HTTP passed: Admin/Editor access, combined status/title/category, unique rows, bounded pagination, retained filters and unknown-category safety."
