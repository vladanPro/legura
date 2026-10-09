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
Invoke-Request -Path $createPath -Method POST -Body $form -Headers $sessionProof -Status 409 | Out-Null
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
Invoke-Request -Path $updatePath -Method POST -Body $update.Replace("public-story", "second-draft") -Headers $sessionProof -Status 409 | Out-Null
Invoke-Request -Path "/posts/public-story" | Out-Null
Invoke-Request -Path $updatePath -Method POST -Body $update.Replace("status=published", "status=draft") -Headers $sessionProof -Status 303 | Out-Null
Invoke-Request -Path "/posts/public-story" -Status 404 | Out-Null
$public = Invoke-Request -Path "/posts"
if ($public.Body.Contains("Published story") -or $public.Body.Contains("Next line")) { throw "Unpublished content remained public" }
$count = & $python.Source -c 'import sqlite3,sys;d=sqlite3.connect(sys.argv[1]);print(d.execute("select count(*) from legura_posts").fetchone()[0]);d.close()' $db
if ($LASTEXITCODE -ne 0 -or $count -ne "2") { throw "Rejected writes changed the stored post set" }
Write-Host "Legura posts HTTP passed: private drafts, admin/CSRF guards, constraints, editing, publishing, unpublishing, escaping and restart persistence."
