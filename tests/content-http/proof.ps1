# Runs only inside the disposable setup/auth fixture with its real owner session.
$contentAction = "/__axonyx/action?path=%2Fadmin%2Fcontent-proof&name=PrepareContentPost"
$contentForm = "slug=rich-proof&content_format=legura-richtext&body=forged&content_document="
$document = '{"format":"legura-richtext","version":1,"blocks":[{"type":"paragraph","children":[{"text":"Rich proof","marks":["bold"]}]}]}'
$contentForm += [Uri]::EscapeDataString($document)

function Assert-ContentCount {
  param([int] $Expected)
  $count = & $python.Source -c 'import sqlite3,sys;d=sqlite3.connect(sys.argv[1]);print(d.execute("select count(*) from legura_posts").fetchone()[0]);d.close()' $db
  if ($LASTEXITCODE -ne 0 -or $count -ne "$Expected") { throw "Content rejection changed the database ($count != $Expected)" }
}

function Assert-StoredContent {
  & $python.Source -c 'import sqlite3,sys,json;d=sqlite3.connect(sys.argv[1]);r=d.execute("select body,content_format,content_document from legura_posts where slug=?",("rich-proof",)).fetchone();p=d.execute("select body,content_format,content_document from legura_posts where slug=?",("plain-proof",)).fetchone();assert r is not None and r[0]=="Rich proof" and r[1]=="legura-richtext";assert json.loads(r[2])==json.loads(sys.argv[2]);assert p==("<script>literal</script>","plain-text",None);d.close()' $db $document
  if ($LASTEXITCODE -ne 0) { throw "Content storage projection/document mismatch" }
  $rich = (Invoke-Request -Path "/api/content-proof?slug=rich-proof" -Headers @{ Cookie = $cookie }).Body | ConvertFrom-Json
  if (!$rich.html.Contains("<strong>Rich proof</strong>")) { throw "Stored rich document did not rehydrate/render" }
  $plain = (Invoke-Request -Path "/api/content-proof?slug=plain-proof" -Headers @{ Cookie = $cookie }).Body | ConvertFrom-Json
  if (!$plain.html.Contains("&lt;script&gt;literal&lt;/script&gt;") -or $plain.html.Contains("<script>")) { throw "Stored plain content was not safely escaped" }
}

$anonymous = Get-Proof
Invoke-Request -Path $contentAction -Method POST -Body $contentForm -Headers $anonymous -Status 403 | Out-Null
Invoke-Request -Path $contentAction -Method POST -Body $contentForm -Headers @{ Cookie = $cookie; Origin = $baseUrl } -Status 403 | Out-Null
$wrongProof = @{ Cookie = $cookie; Origin = $baseUrl; "X-Axonyx-CSRF" = "forged" }
Invoke-Request -Path $contentAction -Method POST -Body $contentForm -Headers $wrongProof -Status 403 | Out-Null
$wrongProof = @{ Cookie = $cookie; Origin = "https://invalid.example"; "X-Axonyx-CSRF" = $sessionProof["X-Axonyx-CSRF"] }
Invoke-Request -Path $contentAction -Method POST -Body $contentForm -Headers $wrongProof -Status 403 | Out-Null
Assert-ContentCount -Expected 0

foreach ($invalid in @(
  "slug=invalid&content_format=legura-richtext&body=private&content_document=private",
  "slug=invalid&content_format=unknown&body=private",
  "slug=invalid&content_format=plain-text&body=+"
)) {
  $jsonHeaders = @{ Cookie = $cookie; Origin = $baseUrl; "X-Axonyx-CSRF" = $sessionProof["X-Axonyx-CSRF"]; Accept = "application/json" }
  $response = Invoke-Request -Path $contentAction -Method POST -Body $invalid -Headers $jsonHeaders -Status 422
  if ($response.Body.Contains("private")) { throw "Invalid content leaked submitted values" }
  $payload = $response.Body | ConvertFrom-Json
  if ($payload.ok -ne $false -or !$payload.error.value.fields) { throw "Missing content field error contract" }
  Assert-ContentCount -Expected 0
}
$created = Invoke-Request -Path $contentAction -Method POST -Body $contentForm -Headers $sessionProof -Status 303
if ($created.Headers.Location -ne "/admin/content-proof") { throw "Native content redirect was lost" }
Invoke-Request -Path $contentAction -Method POST -Body "slug=plain-proof&content_format=plain-text&body=%3Cscript%3Eliteral%3C%2Fscript%3E" -Headers $sessionProof -Status 303 | Out-Null
Assert-ContentCount -Expected 2
Assert-StoredContent
$duplicate = Invoke-Request -Path $contentAction -Method POST -Body $contentForm.Replace("body=forged", "body=changed") -Headers $sessionProof -Status 409
if ($duplicate.Body -match 'INSERT|UNIQUE constraint|legura_posts') { throw "Content write failure leaked database details" }
Assert-ContentCount -Expected 2
Assert-StoredContent
Stop-Fixture
Start-Fixture
Invoke-Request -Path "/admin/content-proof" -Headers @{ Cookie = $cookie } | Out-Null
Assert-StoredContent
Invoke-Request -Path "/posts/rich-proof" -Status 404 | Out-Null
Invoke-Request -Path "/posts/plain-proof" -Status 404 | Out-Null
$richId = & $python.Source -c 'import sqlite3,sys;d=sqlite3.connect(sys.argv[1]);print(d.execute("select id from legura_posts where slug=?",("rich-proof",)).fetchone()[0]);d.close()' $db
if ($LASTEXITCODE -ne 0) { throw "Cannot find isolated rich post" }
$previewPath = "/admin/posts/$richId/preview"
$preview = Invoke-Request -Path $previewPath -Headers @{ Cookie = $cookie }
if (!$preview.Body.Contains('<strong>Rich proof</strong>') -or $preview.Headers['Cache-Control'] -notmatch 'no-store') { throw "Private preview lost formatting or cache protection" }
Invoke-Request -Path $previewPath -Status 403 | Out-Null
Invoke-Request -Path ("/__axonyx/data?path=" + [Uri]::EscapeDataString($previewPath) + "&name=post") -Status 403 | Out-Null
Invoke-Request -Path "/posts/rich-proof" -Status 404 | Out-Null
& $python.Source -c 'import sqlite3,sys;d=sqlite3.connect(sys.argv[1]);d.execute("update legura_posts set status=?",("published",));d.commit();d.close()' $db
if ($LASTEXITCODE -ne 0) { throw "Cannot publish isolated content fixtures" }
$publicRich = Invoke-Request -Path "/posts/rich-proof"
if (!$publicRich.Body.Contains("<strong>Rich proof</strong>") -or $publicRich.Body.Contains("&lt;strong&gt;")) { throw "Public rich content was flattened or double escaped" }
$publicPlain = Invoke-Request -Path "/posts/plain-proof"
if (!$publicPlain.Body.Contains("&lt;script&gt;literal&lt;/script&gt;") -or $publicPlain.Body.Contains("&amp;lt;script&amp;gt;")) { throw "Public plain content was unescaped or double escaped" }
# Deliberately corrupt only the disposable DB, then restore the original record.
try {
  $formatted = '{"format":"legura-richtext","version":1,"blocks":[{"type":"heading","level":2,"children":[{"text":"<Title>","marks":["bold"]}]},{"type":"list-item","list":"unordered","children":[{"text":"List one","marks":["italic"],"link":"https://example.com/?a=1&b=2"}]},{"type":"list-item","list":"ordered","children":[{"text":"List two","marks":[]}]},{"type":"paragraph","children":[{"text":"<svg onload=alert(1)>","marks":[]}]}]}'
  & $python.Source -c 'import sqlite3,sys;d=sqlite3.connect(sys.argv[1]);d.execute("update legura_posts set body=?,content_document=? where slug=?",("<Title>\nList one\nList two\n<svg onload=alert(1)>",sys.argv[2],"rich-proof"));d.commit();d.close()' $db $formatted
  if ($LASTEXITCODE -ne 0) { throw "Cannot seed isolated formatted storage" }
  $formattedPage = Invoke-Request -Path "/posts/rich-proof"
  $formattedPreview = Invoke-Request -Path $previewPath -Headers @{ Cookie = $cookie }
  foreach ($markup in @('<h2><strong>&lt;Title&gt;</strong></h2>', '<ul><li>', '<ol><li>', '<em>List one</em>', 'href="https://example.com/?a=1&amp;b=2"', '&lt;svg onload=alert(1)&gt;')) {
    if (!$formattedPage.Body.Contains($markup)) { throw "Public formatted markup missing: $markup" }
    if (!$formattedPreview.Body.Contains($markup)) { throw "Private formatted markup missing: $markup" }
  }
  if ($formattedPage.Body.Contains('<svg onload=alert(1)>')) { throw "Public renderer trusted text as markup" }
  foreach ($corrupt in @('{}', '{"format":"legura-richtext","version":1,"blocks":[{"type":"paragraph","children":[{"text":"private-marker","marks":[],"link":"javascript:alert(1)"}]}]}')) {
    & $python.Source -c 'import sqlite3,sys;d=sqlite3.connect(sys.argv[1]);d.execute("update legura_posts set content_document=? where slug=?",(sys.argv[2],"rich-proof"));d.commit();d.close()' $db $corrupt
    if ($LASTEXITCODE -ne 0) { throw "Cannot inject isolated invalid document" }
    $failed = Invoke-Request -Path "/posts/rich-proof" -Status 500
    if (!$failed.Body.Contains("We could not load this page.")) { throw "Corrupt public content lost the safe Legura 500 boundary" }
    if ($failed.Body -match 'private-marker|javascript:alert|legura_posts|(?i:\bselect\s+\*\s+from\b)') { throw "Invalid public document leaked storage or executable content" }
    $failedPreview = Invoke-Request -Path $previewPath -Headers @{ Cookie = $cookie } -Status 500
    if (!$failedPreview.Body.Contains("We could not load this page.")) { throw "Corrupt private preview lost the safe Legura 500 boundary" }
    if ($failedPreview.Body -match 'private-marker|javascript:alert|legura_posts|(?i:\bselect\s+\*\s+from\b)') { throw "Invalid private document leaked content" }
    Invoke-Request -Path $previewPath -Status 403 | Out-Null
  }
  & $python.Source -c 'import sqlite3,sys;d=sqlite3.connect(sys.argv[1]);d.execute("update legura_posts set body=?,content_document=? where slug=?",("mismatched",sys.argv[2],"rich-proof"));d.commit();d.close()' $db $document
  if ($LASTEXITCODE -ne 0) { throw "Cannot inject isolated projection mismatch" }
  Invoke-Request -Path "/posts/rich-proof" -Status 500 | Out-Null
  Invoke-Request -Path $previewPath -Headers @{ Cookie = $cookie } -Status 500 | Out-Null
} finally {
  & $python.Source -c 'import sqlite3,sys;d=sqlite3.connect(sys.argv[1]);d.execute("update legura_posts set body=?,content_document=? where slug=?",("Rich proof",sys.argv[2],"rich-proof"));d.execute("update legura_posts set status=?",("draft",));d.commit();d.close()' $db $document
  if ($LASTEXITCODE -ne 0) { throw "Cannot restore isolated content fixture" }
}
Assert-StoredContent
Write-Host "Isolated native content HTTP proof passed: administrator/session, CSRF/origin, 422 errors, derived SQLite storage and restart persistence."
