# Dot-sourced only by the isolated fixture runner after administrator setup.
$createPath = "/__axonyx/action?path=%2Fadmin%2Fposts%2Fnew&name=CreatePost"
$form = "title=Private+draft&slug=private-draft&body=Secret+draft+body&status=draft"
$anonymousProof = Get-Proof
foreach ($path in @("/admin/posts", "/admin/posts/new", "/admin/posts/missing/edit")) {
  Invoke-Request -Path $path -Status 403 | Out-Null
}
Invoke-Request -Path $createPath -Method POST -Body $form -Headers $anonymousProof -Status 403 | Out-Null
Invoke-Request -Path $createPath -Method POST -Body $form -Headers @{ Cookie = $cookie; Origin = $baseUrl } -Status 403 | Out-Null
Invoke-Request -Path $createPath -Method POST -Body $form.Replace("title=Private+draft", "title=") -Headers $sessionProof -Status 422 | Out-Null
Invoke-Request -Path $createPath -Method POST -Body $form.Replace("status=draft", "status=unknown") -Headers $sessionProof -Status 422 | Out-Null
foreach ($slug in @("unsafe%2Fpath", "Uppercase", "-invalid", "invalid-")) {
  Invoke-Request -Path $createPath -Method POST -Body $form.Replace("private-draft", $slug) -Headers $sessionProof -Status 409 | Out-Null
}
Invoke-Request -Path $createPath -Method POST -Body $form.Replace("title=Private+draft", "title=+++") -Headers $sessionProof -Status 409 | Out-Null
$created = Invoke-Request -Path $createPath -Method POST -Body $form -Headers $sessionProof -Status 303
if ($created.Headers.Location -ne "/admin/posts") { throw "Create redirect lost its target" }
$id = & $python.Source -c 'import sqlite3,sys;d=sqlite3.connect(sys.argv[1]);print(d.execute("select id from legura_posts where slug=?",("private-draft",)).fetchone()[0]);d.close()' $db
if ($LASTEXITCODE -ne 0 -or !$id) { throw "Created post did not persist" }
$conflict = Invoke-Request -Path $createPath -Method POST -Body $form -Headers $sessionProof -Status 422
if (!$conflict.Body.Contains("This URL slug is already in use.")) { throw "Duplicate create lost field feedback" }
$public = Invoke-Request -Path "/posts"
if ($public.Body.Contains("Private draft") -or $public.Body.Contains("Secret draft body")) { throw "Public list leaked a draft" }
Invoke-Request -Path "/posts/private-draft" -Status 404 | Out-Null
Invoke-Request -Path "/posts/unknown" -Status 404 | Out-Null
$edit = Invoke-Request -Path "/admin/posts/$id/edit" -Headers @{ Cookie = $cookie }
if (!$edit.Body.Contains("Secret draft body")) { throw "Private editor failed to load post" }
Invoke-Request -Path "/admin/posts/missing/edit" -Headers @{ Cookie = $cookie } -Status 404 | Out-Null
$updatePath = "/__axonyx/action?path=%2Fadmin%2Fposts%2F$id%2Fedit&name=UpdatePost"
$update = "id=$id&title=Published+story&slug=public-story&body=%3Cscript%3Ealert%281%29%3C%2Fscript%3E%0ANext+line&status=published"
Invoke-Request -Path $updatePath -Method POST -Body $update -Headers $anonymousProof -Status 403 | Out-Null
Invoke-Request -Path $updatePath -Method POST -Body $update -Headers @{ Cookie = $cookie; Origin = $baseUrl } -Status 403 | Out-Null
Invoke-Request -Path $updatePath -Method POST -Body $update.Replace("id=$id", "id=missing") -Headers $sessionProof -Status 404 | Out-Null
Invoke-Request -Path $updatePath -Method POST -Body $update -Headers $sessionProof -Status 303 | Out-Null
$story = Invoke-Request -Path "/posts/public-story"
if (!$story.Body.Contains("Published story") -or !$story.Body.Contains("&lt;script&gt;") -or $story.Body.Contains("<script>alert(1)</script>")) { throw "Published text was missing or not escaped" }
Stop-Fixture
Start-Fixture
$story = Invoke-Request -Path "/posts/public-story"
if (!$story.Body.Contains("Published story")) { throw "Published post did not survive restart" }
$duplicate = $form.Replace("private-draft", "second-draft")
Invoke-Request -Path $createPath -Method POST -Body $duplicate -Headers $sessionProof -Status 303 | Out-Null
$conflict = Invoke-Request -Path $updatePath -Method POST -Body $update.Replace("public-story", "second-draft") -Headers $sessionProof -Status 422
if (!$conflict.Body.Contains("This URL slug is already in use.")) { throw "Duplicate update lost field feedback" }
Invoke-Request -Path "/posts/public-story" | Out-Null
Invoke-Request -Path $updatePath -Method POST -Body $update.Replace("status=published", "status=draft") -Headers $sessionProof -Status 303 | Out-Null
Invoke-Request -Path "/posts/public-story" -Status 404 | Out-Null
$public = Invoke-Request -Path "/posts"
if ($public.Body.Contains("Published story") -or $public.Body.Contains("Next line")) { throw "Unpublished content remained public" }
$count = & $python.Source -c 'import sqlite3,sys;d=sqlite3.connect(sys.argv[1]);print(d.execute("select count(*) from legura_posts").fetchone()[0]);d.close()' $db
if ($LASTEXITCODE -ne 0 -or $count -ne "2") { throw "Rejected writes changed the stored post set" }

# Confirmation is read-only; mutation authorization is checked again on POST.
$deleteRoute = "/admin/posts/$id/delete"
$deletePath = "/__axonyx/action?path=%2Fadmin%2Fposts%2F$id%2Fdelete&name=DeletePost"
$deleteBody = "id=$id&confirmSlug=public-story"
Invoke-Request -Path $updatePath -Method POST -Body $update -Headers $sessionProof -Status 303 | Out-Null
Invoke-Request -Path $deleteRoute -Status 403 | Out-Null
$confirmation = Invoke-Request -Path $deleteRoute -Headers @{ Cookie = $cookie }
if (!$confirmation.Body.Contains("This permanently deletes the post.")) { throw "Deletion warning is missing" }
Invoke-Request -Path "/posts/public-story" | Out-Null
Invoke-Request -Path $deletePath -Headers $sessionProof -Status 405 | Out-Null
if ($BackupRestore) {
  $bundle = Join-Path $fixture "posts-backup"
  & $maintenance backup $db $bundle
  if ($LASTEXITCODE -ne 0) { throw "Live fixture backup failed" }
  & $maintenance verify $bundle
  if ($LASTEXITCODE -ne 0) { throw "Backup verification failed" }
}
Invoke-Request -Path $deletePath -Method POST -Body $deleteBody -Headers $anonymousProof -Status 403 | Out-Null
Invoke-Request -Path $deletePath -Method POST -Body $deleteBody -Headers @{ Cookie = $cookie; Origin = $baseUrl } -Status 403 | Out-Null
$wrong = Invoke-Request -Path $deletePath -Method POST -Body $deleteBody.Replace("confirmSlug=public-story", "confirmSlug=wrong") -Headers $sessionProof -Status 422
if (!$wrong.Body.Contains("URL slug to confirm deletion.")) { throw "Deletion confirmation field feedback is missing" }
Invoke-Request -Path $deletePath -Method POST -Body $deleteBody.Replace("id=$id", "id=missing") -Headers $sessionProof -Status 404 | Out-Null
Invoke-Request -Path "/admin/posts/missing/delete" -Headers @{ Cookie = $cookie } -Status 404 | Out-Null
$otherId = & $python.Source -c 'import sqlite3,sys;d=sqlite3.connect(sys.argv[1]);print(d.execute("select id from legura_posts where slug=?",("second-draft",)).fetchone()[0]);d.close()' $db
if ($LASTEXITCODE -ne 0 -or !$otherId) { throw "Second post inspection failed" }
Invoke-Request -Path $deletePath -Method POST -Body $deleteBody.Replace("id=$id", "id=$otherId") -Headers $sessionProof -Status 422 | Out-Null
Invoke-Request -Path "/posts/public-story" | Out-Null
$deleted = Invoke-Request -Path $deletePath -Method POST -Body $deleteBody -Headers $sessionProof -Status 303
if ($deleted.Headers.Location -ne "/admin/posts") { throw "Delete redirect lost its target" }
Invoke-Request -Path "/posts/public-story" -Status 404 | Out-Null
Invoke-Request -Path "/admin/posts/$id/edit" -Headers @{ Cookie = $cookie } -Status 404 | Out-Null
$afterDelete = Invoke-Request -Path "/posts"
if ($afterDelete.Body.Contains("Published story")) { throw "Deleted post remained in the public list" }
Invoke-Request -Path $deleteRoute -Headers @{ Cookie = $cookie } -Status 404 | Out-Null
Invoke-Request -Path $deletePath -Method POST -Body $deleteBody -Headers $sessionProof -Status 404 | Out-Null
$remaining = & $python.Source -c 'import sqlite3,sys,json;d=sqlite3.connect(sys.argv[1]);print(json.dumps([r[0] for r in d.execute("select id from legura_posts")]));d.close()' $db
$remainingIds = @($remaining | ConvertFrom-Json)
if ($LASTEXITCODE -ne 0 -or $remainingIds.Count -ne 1 -or $remainingIds[0] -ne $otherId) { throw "Delete affected the wrong stored post set" }
Stop-Fixture
Start-Fixture
Invoke-Request -Path "/posts/public-story" -Status 404 | Out-Null
Invoke-Request -Path $deleteRoute -Headers @{ Cookie = $cookie } -Status 404 | Out-Null
if ($BackupRestore) {
  $recovered = Join-Path $fixture "data/recovered.db"
  & $maintenance restore $bundle $recovered
  if ($LASTEXITCODE -ne 0) { throw "Restore to new database failed" }
  Stop-Fixture
  $originalUrl = $env:AX_SECRET_DB_URL
  try {
    $env:AX_SECRET_DB_URL = "sqlite://" + $recovered.Replace('\', '/')
    Start-Fixture
    $restoredStory = Invoke-Request -Path "/posts/public-story"
    if (!$restoredStory.Body.Contains("Published story")) { throw "Restored public post is missing" }
    Invoke-Request -Path "/posts/second-draft" -Status 404 | Out-Null
    Invoke-Request -Path "/admin" -Status 403 | Out-Null
    Invoke-Request -Path "/setup" -Status 403 | Out-Null
    $restoreProof = Get-Proof
    $restoreEmail = [Uri]::EscapeDataString($identity.email)
    $restoreLogin = Invoke-Request -Path "/__axonyx/action?path=%2Flogin&name=SignIn" -Method POST -Body "email=$restoreEmail&password=$password" -Headers $restoreProof -Status 303
    $restoreCookie = ([string]$restoreLogin.Headers["Set-Cookie"]).Split(';')[0]
    Invoke-Request -Path "/admin/posts/$id/edit" -Headers @{ Cookie = $restoreCookie } | Out-Null
    Invoke-Request -Path "/admin/posts/$otherId/edit" -Headers @{ Cookie = $restoreCookie } | Out-Null
    Stop-Fixture
    Start-Fixture
    Invoke-Request -Path "/posts/public-story" | Out-Null
    Write-Host "Legura backup/restore passed: live snapshot, deleted-content recovery, preserved login/drafts/setup lock and restored restart."
  } finally {
    Stop-Fixture
    $env:AX_SECRET_DB_URL = $originalUrl
    Start-Fixture
  }
  Invoke-Request -Path "/posts/public-story" -Status 404 | Out-Null
}
Invoke-Request -Path $createPath -Method POST -Body $form.Replace("private-draft", "public-story") -Headers $sessionProof -Status 303 | Out-Null
$revocablePostId = & $python.Source -c 'import sqlite3,sys;d=sqlite3.connect(sys.argv[1]);print(d.execute("select id from legura_posts where slug=?",("public-story",)).fetchone()[0]);d.close()' $db
if ($LASTEXITCODE -ne 0 -or !$revocablePostId) { throw "Reused slug did not persist a new post" }
$draftDeletePath = "/__axonyx/action?path=%2Fadmin%2Fposts%2F$otherId%2Fdelete&name=DeletePost"
Invoke-Request -Path $draftDeletePath -Method POST -Body "id=$otherId&confirmSlug=second-draft" -Headers $sessionProof -Status 303 | Out-Null
Write-Host "Legura posts HTTP passed: auth/CSRF, create/edit/publish, escaped text, confirmed deletion, repeat-delete rejection and restart persistence."
