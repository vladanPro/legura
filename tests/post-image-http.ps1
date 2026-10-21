# Runs only inside smoke-auth's disposable, authenticated Media fixture.
$createPostPath = '/__axonyx/action?path=%2Fadmin%2Fposts%2Fnew&name=CreatePost'
Invoke-Request -Path $createPostPath -Method POST -Body 'title=Image+fixture&slug=image-fixture&body=Unchanged+body&status=draft' -Headers $sessionProof -Status 303 | Out-Null
$postImagePostId = & $python.Source -c 'import sqlite3,sys;d=sqlite3.connect(sys.argv[1]);print(d.execute("select id from legura_posts where slug=?",("image-fixture",)).fetchone()[0]);d.close()' $db
if ($LASTEXITCODE -ne 0 -or !$postImagePostId) { throw 'Cannot locate image fixture post' }
$pickerPath = "/admin/posts/$postImagePostId/image"
$encodedRoute = [Uri]::EscapeDataString($pickerPath)
$postImageSetPath = "/__axonyx/action?path=$encodedRoute&name=SetPostImage"
$postImageRemovePath = "/__axonyx/action?path=$encodedRoute&name=RemovePostImage"
$postImageForm = "id=$postImagePostId&media_id=$($first[0])&alt=Fixture+image"

function Assert-PostImage([string]$ExpectedId = '', [string]$ExpectedAlt = '') {
  $value = & $python.Source -c 'import sqlite3,sys,json;d=sqlite3.connect(sys.argv[1]);r=d.execute("select media_id,alt from legura_post_images where post_id=?",(sys.argv[2],)).fetchone();p=d.execute("select body,status from legura_posts where id=?",(sys.argv[2],)).fetchone();print(json.dumps({"image":r,"post":p}));d.close()' $db $postImagePostId | ConvertFrom-Json
  if ($LASTEXITCODE -ne 0 -or ($value.post -join ',') -ne 'Unchanged body,draft') { throw 'Image mutation changed post content/status' }
  if (!$ExpectedId) {
    if ($null -ne $value.image) { throw 'Image selection was not cleared' }
  } elseif (($value.image -join '|') -ne "$ExpectedId|$ExpectedAlt") { throw 'Image selection/alt mismatch' }
}

Invoke-Request -Path $pickerPath -Status 403 | Out-Null
$anonymousImageProof = Get-Proof
Invoke-Request -Path $postImageSetPath -Method POST -Body "$postImageForm&administrator_id=owner&subject=owner" -Headers $anonymousImageProof -Status 403 | Out-Null
Invoke-Request -Path $postImageRemovePath -Method POST -Body "id=$postImagePostId" -Headers $anonymousImageProof -Status 403 | Out-Null
foreach ($headers in @(
  @{Cookie=$cookie; Origin=$baseUrl},
  @{Cookie=$cookie; Origin=$baseUrl; 'X-Axonyx-CSRF'='forged'},
  @{Cookie=$cookie; Origin='https://attacker.example'; 'X-Axonyx-CSRF'=$sessionProof['X-Axonyx-CSRF']}
)) {
  Invoke-Request -Path $postImageSetPath -Method POST -Body $postImageForm -Headers $headers -Status 403 | Out-Null
  Invoke-Request -Path $postImageRemovePath -Method POST -Body "id=$postImagePostId" -Headers $headers -Status 403 | Out-Null
}
Assert-PostImage
$emptyPicker = Invoke-Request -Path $pickerPath -Headers @{Cookie=$cookie}
if (!$emptyPicker.Body.Contains('No featured image selected') -or [regex]::Matches($emptyPicker.Body, 'type="radio"').Count -ne 10) { throw 'Image picker did not render bounded library choices' }
$saved = Invoke-Request -Path $postImageSetPath -Method POST -Body "$postImageForm&administrator_id=forged" -Headers $sessionProof -Status 303
if ($saved.Headers.Location -ne $pickerPath) { throw 'Image save redirect is not local to its post' }
Assert-PostImage $first[0] 'Fixture image'
$bad = Invoke-Request -Path $postImageSetPath -Method POST -Body "id=$postImagePostId&media_id=$($first[0])&alt=$('x' * 301)" -Headers $sessionProof -Status 422
if (!$bad.Body.Contains('300 characters')) { throw 'Alt text validation is not visible' }
Assert-PostImage $first[0] 'Fixture image'
Invoke-Request -Path $postImageSetPath -Method POST -Body "id=$postImagePostId&media_id=$('f' * 64)&alt=Rejected" -Headers $sessionProof -Status 422 | Out-Null
Invoke-Request -Path $postImageSetPath -Method POST -Body "id=missing&media_id=$($first[0])&alt=Rejected" -Headers $sessionProof -Status 404 | Out-Null
Assert-PostImage $first[0] 'Fixture image'
$damagedPath = Join-Path $mediaRoot $first[1]
$originalBytes = [IO.File]::ReadAllBytes($damagedPath)
try {
  [IO.File]::WriteAllBytes($damagedPath, [byte[]]@(0, 1, 2))
  $failed = Invoke-Request -Path $postImageSetPath -Method POST -Body "id=$postImagePostId&media_id=$($first[1])&alt=Rejected" -Headers $sessionProof -Status 500
  if ($failed.Body -match 'SELECT |legura_post_images|private-media') { throw 'Image failure exposed storage/SQL details' }
  Assert-PostImage $first[0] 'Fixture image'
} finally { [IO.File]::WriteAllBytes($damagedPath, $originalBytes) }
$literalAlt = '<script>literal</script> "caption"'
Invoke-Request -Path $postImageSetPath -Method POST -Body "id=$postImagePostId&media_id=$($second[0])&alt=$([Uri]::EscapeDataString($literalAlt))" -Headers $sessionProof -Status 303 | Out-Null
Assert-PostImage $second[0] $literalAlt
$chosen = Invoke-Request -Path $pickerPath -Headers @{Cookie=$cookie}
if (!$chosen.Body.Contains('data-post-image-preview="true"') -or $chosen.Body.Contains('alt="<script>')) { throw 'Current image/escaped alt rendering failed' }
$edit = Invoke-Request -Path "/admin/posts/$postImagePostId/edit" -Headers @{Cookie=$cookie}
if (!$edit.Body.Contains("/api/media/$($second[0])") -or !$edit.Body.Contains($pickerPath)) { throw 'Editor does not link/show selected image' }
Stop-Fixture
Start-Fixture
Assert-PostImage $second[0] $literalAlt
Assert-PrivateMediaRead $second[0] '' 403
Invoke-Request -Path $postImageRemovePath -Method POST -Body "id=$postImagePostId" -Headers $sessionProof -Status 303 | Out-Null
Assert-PostImage
Assert-PrivateMediaRead $second[0] $cookie 200
Invoke-Request -Path $postImageSetPath -Method POST -Body $postImageForm -Headers $sessionProof -Status 303 | Out-Null
if ($MediaBrowser) {
  @{baseUrl=$baseUrl; cookie=$cookie; postId=$postImagePostId} | ConvertTo-Json -Compress | & node (Join-Path $root 'tests/post-image-browser.mjs')
  if ($LASTEXITCODE -ne 0) { throw 'Image picker browser acceptance failed' }
}
Stop-Fixture
$env:AX_SECRET_MEDIA_DRIVER = 'disabled'
Remove-Item -LiteralPath 'Env:AX_SECRET_MEDIA_ROOT' -ErrorAction SilentlyContinue
Remove-Item -LiteralPath 'Env:AX_SECRET_MEDIA_STORE_ID' -ErrorAction SilentlyContinue
try {
  Start-Fixture
  Invoke-Request -Path $pickerPath -Headers @{Cookie=$cookie} -Status 403 | Out-Null
  Invoke-Request -Path $postImageSetPath -Method POST -Body $postImageForm -Headers $sessionProof -Status 403 | Out-Null
  Invoke-Request -Path $postImageRemovePath -Method POST -Body "id=$postImagePostId" -Headers $sessionProof -Status 403 | Out-Null
  $disabledEdit = Invoke-Request -Path "/admin/posts/$postImagePostId/edit" -Headers @{Cookie=$cookie}
  if ($disabledEdit.Body.Contains('Choose featured image')) { throw 'Disabled media still exposes the picker' }
  Invoke-Request -Path "/api/post-images/image-fixture/$($first[0])" -Status 404 | Out-Null
} finally {
  Stop-Fixture
  $env:AX_SECRET_MEDIA_DRIVER = 'local'
  $env:AX_SECRET_MEDIA_ROOT = $mediaRoot
  $env:AX_SECRET_MEDIA_STORE_ID = 'local-media'
  Start-Fixture
}
Invoke-Request -Path $postImageSetPath -Method POST -Body $postImageForm -Headers $sessionProof -Status 303 | Out-Null
Assert-PostImage $first[0] 'Fixture image'
Write-Host 'Post image HTTP passed: guarded selection/replace/remove, escaped alt, bounded picker, unchanged content/status, persistence and private bytes.'
. (Join-Path $root 'tests/public-post-image-http.ps1')
