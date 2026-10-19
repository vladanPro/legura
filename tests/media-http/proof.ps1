# Dot-sourced only inside smoke-auth's disposable, session-backed HTTP fixture.
$mediaAction = "/__axonyx/action?path=%2Fadmin%2Fmedia&name=UploadLibraryImage"
$encoded = & $python.Source -c 'import struct,zlib,base64;chunk=lambda k,b:struct.pack(">I",len(b))+k+b+struct.pack(">I",zlib.crc32(k+b));data=b"\x89PNG\r\n\x1a\n"+chunk(b"IHDR",struct.pack(">IIBBBBB",1,1,8,6,0,0,0))+chunk(b"IDAT",zlib.compress(bytes([0,70,120,200,255])))+chunk(b"IEND",b"")+b"<script>not-an-image-payload</script>";print(base64.b64encode(data).decode())'
if ($LASTEXITCODE -ne 0) { throw "Cannot create isolated image bytes" }
$validImage = [Convert]::FromBase64String($encoded)

function Assert-PrivateMediaRead([string]$Id, [string]$Cookie, [int]$Status = 200, [string]$Query = '') {
  $request = [Net.Http.HttpRequestMessage]::new([Net.Http.HttpMethod]::Get, "$baseUrl/api/media/$Id$Query")
  if ($Cookie) { $request.Headers.Add('Cookie', $Cookie) }
  try {
    $response = $client.SendAsync($request).GetAwaiter().GetResult()
    try {
      if ([int]$response.StatusCode -ne $Status) { throw "Private media expected $Status, got $([int]$response.StatusCode)" }
      if ($response.Headers.CacheControl.ToString() -notmatch 'private' -or $response.Headers.CacheControl.ToString() -notmatch 'no-store') { throw 'Private image is cacheable' }
      if (($response.Headers.GetValues('X-Content-Type-Options') -join '') -ne 'nosniff' -or $response.Headers.Contains('Set-Cookie')) { throw 'Private image header policy failed' }
      if (($response.Headers.GetValues('Cross-Origin-Resource-Policy') -join '') -ne 'same-origin') { throw 'Private image cross-origin policy failed' }
      $bytes = $response.Content.ReadAsByteArrayAsync().GetAwaiter().GetResult()
      if ($Status -eq 200) {
        $expected = [IO.File]::ReadAllBytes((Join-Path $mediaRoot $Id))
        if ($response.Content.Headers.ContentType.MediaType -ne 'image/png' -or [Convert]::ToBase64String($bytes) -ne [Convert]::ToBase64String($expected)) { throw 'Private image bytes or MIME changed' }
      } else {
        $text = [Text.Encoding]::UTF8.GetString($bytes)
        $expected = switch ($Status) { 403 { 'forbidden' }; 404 { 'not found' }; 500 { 'binary response unavailable' } }
        if ($text -ne $expected) { throw 'Private image error exposed implementation details' }
      }
    } finally { $response.Dispose() }
  } finally { $request.Dispose() }
}

function Assert-MediaCounts([int]$Rows, [int]$Files) {
  $count = & $python.Source -c 'import sqlite3,sys;d=sqlite3.connect(sys.argv[1]);print(d.execute("select count(*) from legura_media").fetchone()[0]);d.close()' $db
  if ($LASTEXITCODE -ne 0 -or $count -ne "$Rows") { throw "Unexpected media row count: $count != $Rows" }
  if (@(Get-ChildItem -LiteralPath $mediaRoot -File).Count -ne $Files) { throw "Unexpected media object count" }
}

function Send-Media([byte[]]$Bytes, [hashtable]$Headers, [int]$Status, [switch]$Native, [switch]$Chunked, [int]$Padding = 0, [string]$Target = '') {
  $action = if ($Target) { $Target } elseif ($Native) { $mediaAction + 'AndReturn' } else { $mediaAction }
  $request = [Net.Http.HttpRequestMessage]::new([Net.Http.HttpMethod]::Post, "$baseUrl$action")
  $multipart = [Net.Http.MultipartFormDataContent]::new()
  $part = [Net.Http.ByteArrayContent]::new($Bytes)
  $part.Headers.ContentType = [Net.Http.Headers.MediaTypeHeaderValue]::new("text/html")
  $multipart.Add($part, "image", "../../untrusted.svg")
  if ($Padding -gt 0) { $multipart.Add([Net.Http.ByteArrayContent]::new([byte[]]::new($Padding)), 'padding', 'padding.bin') }
  $request.Content = $multipart
  if ($ScopedMedia) { $request.Headers.ExpectContinue = $true }
  if ($Chunked) { $request.Headers.TransferEncodingChunked = $true }
  foreach ($key in $Headers.Keys) {
    if ($key -ne 'Accept') { $request.Headers.Add($key, [string]$Headers[$key]) }
  }
  $request.Headers.Add('Accept', $(if ($Native) { 'text/html' } else { 'application/ax-patch+json' }))
  try { $response = Read-Response ($client.SendAsync($request).GetAwaiter().GetResult()) }
  finally { $request.Dispose() }
  if ($response.Status -ne $Status) { throw "Media upload expected $Status, got $($response.Status): $($response.Body)" }
  return $response
}

function Assert-StoredMedia {
  $inspection = @'
import hashlib, pathlib, re, sqlite3, struct, sys
with sqlite3.connect(sys.argv[1]) as database:
    rows = database.execute("SELECT id,store_id,content_type,byte_size,width,height,sha256 FROM legura_media").fetchall()
assert rows
root = pathlib.Path(sys.argv[2])
for key, store, mime, size, width, height, digest in rows:
    assert re.fullmatch("[0-9a-f]{64}", key)
    data = (root / key).read_bytes()
    assert store == "local-media" and mime == "image/png"
    assert size == len(data) and (width, height) == (1, 1)
    assert digest == hashlib.sha256(data).hexdigest()
    assert data[:8] == b"\x89PNG\r\n\x1a\n"
    assert struct.unpack(">II", data[16:24]) == (1, 1)
    assert b"<script>" not in data
'@
  & $python.Source -c $inspection $db $mediaRoot
  if ($LASTEXITCODE -ne 0) { throw "Stored media metadata/hash/canonical bytes mismatch" }
}

Invoke-Request -Path "/admin/media" -Status 403 | Out-Null
# Fresh migrated installation remains unavailable until explicit local opt-in.
Stop-Fixture
$env:AX_SECRET_MEDIA_DRIVER = 'disabled'
Remove-Item -LiteralPath 'Env:AX_SECRET_MEDIA_ROOT' -ErrorAction SilentlyContinue
Remove-Item -LiteralPath 'Env:AX_SECRET_MEDIA_STORE_ID' -ErrorAction SilentlyContinue
try {
  Start-Fixture
  $disabled = Invoke-Request -Path '/admin' -Headers @{ Cookie = $cookie }
  if ($disabled.Body.Contains('href="/admin/media"')) { throw 'Disabled Media leaked into product navigation' }
  Invoke-Request -Path '/admin/media' -Headers @{ Cookie = $cookie } -Status 403 | Out-Null
  Assert-PrivateMediaRead ('a' * 64) $cookie 403
  $disabledProof = Get-Proof -Cookie $cookie
  Send-Media -Bytes $validImage -Headers $disabledProof -Status 403 | Out-Null
  Send-Media -Bytes $validImage -Headers $disabledProof -Status 403 -Native | Out-Null
  Assert-MediaCounts -Rows 0 -Files 0
} finally {
  Stop-Fixture
  $env:AX_SECRET_MEDIA_DRIVER = 'local'
  $env:AX_SECRET_MEDIA_ROOT = $mediaRoot
  $env:AX_SECRET_MEDIA_STORE_ID = 'local-media'
  Start-Fixture
}
foreach ($path in @('/admin', '/admin/posts', '/admin/posts/new', '/admin/posts/new/plain')) {
  $navigation = Invoke-Request -Path $path -Headers @{ Cookie = $cookie }
  if (!$navigation.Body.Contains('href="/admin/media"')) { throw "Enabled Media missing from $path navigation" }
}
$anonymous = Get-Proof
Send-Media -Bytes $validImage -Headers $anonymous -Status 403 | Out-Null
Send-Media -Bytes $validImage -Headers $anonymous -Status 403 -Native | Out-Null
Send-Media -Bytes $validImage -Headers @{ Cookie = $cookie; Origin = $baseUrl } -Status 403 -Native | Out-Null
Send-Media -Bytes $validImage -Headers @{ Cookie = $cookie; Origin = $baseUrl } -Status 403 | Out-Null
Send-Media -Bytes $validImage -Headers @{ Cookie = $cookie; Origin = $baseUrl; 'X-Axonyx-CSRF' = 'forged' } -Status 403 | Out-Null
Send-Media -Bytes $validImage -Headers @{ Cookie = $cookie; Origin = 'https://attacker.example'; 'X-Axonyx-CSRF' = $sessionProof['X-Axonyx-CSRF'] } -Status 403 | Out-Null
Assert-MediaCounts -Rows 0 -Files 0
$empty = Invoke-Request -Path '/admin/media?page=999' -Headers @{ Cookie = $cookie }
if (!$empty.Body.Contains('No images yet') -or !$empty.Body.Contains('data-media-page="1"') -or !$empty.Body.Contains('data-media-total="0"')) { throw "Empty media library is not bounded" }
if (!$empty.Body.Contains('data-media-available="true"')) { throw 'Validated Media availability did not reach the guarded compiled page' }
foreach ($private in @($mediaRoot, $db, $env:AX_SECRET_SESSION_KEY, $env:AX_SECRET_SETUP_TOKEN)) {
  if ($private -and ($empty.Body.Contains($private) -or $empty.Body.Contains($private.Replace('\', '/')))) { throw 'Media availability leaked private configuration' }
}
if (!$empty.Body.Contains('aria-label="No image selected"')) { throw "Empty media library has no accessible image placeholder" }
if ($empty.Body -notmatch 'href="/admin/media"[^>]*aria-current="true"|aria-current="true"[^>]*href="/admin/media"') { throw "Media sidebar item is not selected" }

foreach ($bytes in @([Text.Encoding]::UTF8.GetBytes('<svg onload="alert(1)"/>'), [byte[]]::new(0), [byte[]]::new(8388609))) {
  $response = Send-Media -Bytes $bytes -Headers $sessionProof -Status 422
  if ($response.Body -match 'untrusted.svg|onload|private-media') { throw "Invalid upload echoed untrusted or private input" }
  Assert-MediaCounts -Rows 0 -Files 0
}
$oversizeStatus = if ($ScopedMedia) { 413 } else { 400 }
Send-Media -Bytes ([byte[]]::new(9437185)) -Headers $sessionProof -Status $oversizeStatus | Out-Null
Assert-MediaCounts -Rows 0 -Files 0

$padding = 0
if ($ScopedMedia) {
  foreach ($target in @(
    '/__axonyx/action?path=%2Flogin&name=SignIn',
    '/__axonyx/action?path=%2Fadmin%2Fmedia&name=Missing',
    "$mediaAction&name=SignIn",
    "$mediaAction&%6Eame=SignIn",
    "$mediaAction&path=%2Flogin"
  )) {
    Send-Media -Bytes $validImage -Headers $sessionProof -Status 413 -Padding 70000 -Target $target | Out-Null
    Send-Media -Bytes $validImage -Headers $sessionProof -Status 413 -Padding 70000 -Target $target -Chunked | Out-Null
    Assert-MediaCounts -Rows 0 -Files 0
  }
  Send-Media -Bytes ([byte[]]::new(9437185)) -Headers $sessionProof -Status 413 -Chunked | Out-Null
  Send-Media -Bytes $validImage -Headers $anonymous -Status 403 -Padding 70000 | Out-Null
  Send-Media -Bytes $validImage -Headers @{ Cookie = $cookie; Origin = $baseUrl } -Status 403 -Padding 70000 | Out-Null
  Assert-MediaCounts -Rows 0 -Files 0
  $padding = 70000
}

$result = (Send-Media -Bytes $validImage -Headers $sessionProof -Status 200 -Padding $padding -Chunked:$ScopedMedia).Body | ConvertFrom-Json
if (!$result.ok -or $result.value.storage -ne 'local-media' -or $result.value.content_type -ne 'image/png' -or $result.value.file_name -ne "$($result.value.id).png") { throw "Uploaded FileRef is not canonical" }
$mediaId = $result.value.id
Assert-MediaCounts -Rows 1 -Files 1
Assert-StoredMedia
Assert-PrivateMediaRead $mediaId '' 403
Assert-PrivateMediaRead $mediaId 'session=forged' 403 '?subject=owner&role=admin'
Assert-PrivateMediaRead $mediaId $cookie
Assert-PrivateMediaRead ('0' * 64) $cookie 404
Assert-PrivateMediaRead 'not-an-object-key' $cookie 404
$objectPath = Join-Path $mediaRoot $mediaId
$canonical = [IO.File]::ReadAllBytes($objectPath)
try {
  [IO.File]::WriteAllBytes($objectPath, [Text.Encoding]::UTF8.GetBytes('corrupted-private-image'))
  Assert-PrivateMediaRead $mediaId $cookie 500
} finally { [IO.File]::WriteAllBytes($objectPath, $canonical) }
Assert-PrivateMediaRead $mediaId $cookie
$page = Invoke-Request -Path '/admin/media' -Headers @{ Cookie = $cookie }
if (!$page.Body.Contains($mediaId) -or $page.Headers['Cache-Control'] -notmatch 'no-store') { throw "Guarded library rendering/caching failed" }
Invoke-Request -Path "/private-media/$mediaId" -Status 404 | Out-Null
Stop-Fixture
Start-Fixture
Assert-StoredMedia
$page = Invoke-Request -Path '/admin/media' -Headers @{ Cookie = $cookie }
if (!$page.Body.Contains($mediaId)) { throw "Library state did not survive server restart" }
Send-Media -Bytes $validImage -Headers $sessionProof -Status 200 | Out-Null
Assert-MediaCounts -Rows 2 -Files 2
Assert-StoredMedia

# An explicit abort proves file/DB failure policy: retain an unreferenced object,
# return no successful FileRef, and never delete a possibly committed object.
$injectFailure = @'
import sqlite3, sys
with sqlite3.connect(sys.argv[1]) as database:
    database.execute("CREATE TRIGGER reject_media BEFORE INSERT ON legura_media BEGIN SELECT RAISE(ABORT, 'private-db-marker'); END")
'@
& $python.Source -c $injectFailure $db
if ($LASTEXITCODE -ne 0) { throw "Cannot inject isolated metadata failure" }
try {
  $failed = Send-Media -Bytes $validImage -Headers $sessionProof -Status 500
  if ($failed.Body -match 'private-db-marker|INSERT|legura_media|private-media') { throw "Metadata failure leaked implementation details" }
  Assert-MediaCounts -Rows 2 -Files 3
  Assert-StoredMedia
} finally {
  & $python.Source -c 'import sqlite3,sys;d=sqlite3.connect(sys.argv[1]);d.execute("DROP TRIGGER reject_media");d.commit();d.close()' $db
  if ($LASTEXITCODE -ne 0) { throw "Cannot remove isolated metadata failure" }
}

# Ten items per page, populated through real uploads rather than fake metadata.
$native = Send-Media -Bytes $validImage -Headers $sessionProof -Status 303 -Native -Padding $padding
if ($native.Headers['Location'] -ne '/admin/media') { throw "Native upload did not return to the library" }
for ($index = 0; $index -lt 8; $index++) { Send-Media -Bytes $validImage -Headers $sessionProof -Status 200 | Out-Null }
Assert-MediaCounts -Rows 11 -Files 12
Assert-StoredMedia

function Get-MediaPageIds([string]$Query, [int]$Page, [int]$Count) {
  $response = Invoke-Request -Path "/admin/media$Query" -Headers @{ Cookie = $cookie }
  if ($response.Headers['Cache-Control'] -notmatch 'no-store') { throw "Media pagination response was cacheable" }
  if (!$response.Body.Contains("data-media-page=`"$Page`"") -or !$response.Body.Contains('data-media-total="11"')) { throw "Incorrect media page/total" }
  $ids = @([regex]::Matches($response.Body, 'data-media-id="([0-9a-f]{64})"') | ForEach-Object { $_.Groups[1].Value })
  if ($ids.Count -ne $Count) { throw "Expected $Count media entries on page $Page, got $($ids.Count)" }
  return $ids
}
$first = @(Get-MediaPageIds '' 1 10)
$second = @(Get-MediaPageIds '?page=2' 2 1)
$all = @($first + $second)
if (@($all | Select-Object -Unique).Count -ne 11 -or ($all -join ',') -ne (($all | Sort-Object) -join ',')) { throw "Media pages duplicate, omit or reorder entries" }
foreach ($invalid in @('-1', 'text', '1%20OR%201%3D1', '%3Cscript%3E')) {
  $actual = @(Get-MediaPageIds "?page=$invalid" 1 10)
  if (($actual -join ',') -ne ($first -join ',')) { throw "Invalid media page altered listing" }
}
$last = @(Get-MediaPageIds '?page=999999999999999999999999' 2 1)
if (($last -join ',') -ne ($second -join ',')) { throw "Large media page was not clamped" }
Stop-Fixture
Start-Fixture
$restarted = @(Get-MediaPageIds '?page=2' 2 1)
if (($restarted -join ',') -ne ($second -join ',')) { throw "Media pagination changed after restart" }
$mediaExpectedRows = 11
$mediaExpectedFiles = 12
if ($MediaBrowser) {
  # Session is passed on stdin, not command-line arguments or a persistent file.
  @{ baseUrl = $baseUrl; cookie = $cookie; image = [Convert]::ToBase64String($validImage) } | ConvertTo-Json -Compress | & node (Join-Path $root 'tests/media-browser.mjs')
  if ($LASTEXITCODE -ne 0) { throw "Media browser acceptance failed" }
  $mediaExpectedRows += 2
  $mediaExpectedFiles += 2
  Assert-MediaCounts -Rows $mediaExpectedRows -Files $mediaExpectedFiles
  Assert-StoredMedia
}
Write-Host 'Media HTTP proof passed: protected uploads, native redirect, Foundry library, bounded pagination, persistence and orphan-on-DB-failure policy.'
. (Join-Path $root 'tests/post-image-http.ps1')

# Run the operator boundary against the real uploaded objects and live SQLite.
# Media's target is disposable; this never replaces the operator maintenance tool.
$inventoryJson = & cargo run --locked --release --bin legura-maintenance -- media-check $db local-media $mediaRoot
if ($LASTEXITCODE -ne 0) { throw "Operator media integrity check rejected valid uploaded objects" }
$inventory = $inventoryJson | ConvertFrom-Json
if ($inventory.objects -ne $mediaExpectedRows -or $inventory.bytes -ne ($result.value.size * $mediaExpectedRows)) { throw "Operator media inventory does not match persisted uploads" }
Assert-MediaCounts -Rows $mediaExpectedRows -Files $mediaExpectedFiles

# Source-pilot archive creation while the disposable CMS is still listening.
# This uses SQLite's online backup API, never a raw copy of its live DB file.
$archivePath = Join-Path $fixture 'media-backup.legura'
$archiveJson = & cargo run --locked --release --bin legura-maintenance -- archive-backup $db local-media $mediaRoot $archivePath
if ($LASTEXITCODE -ne 0) { throw "Live media archive creation failed" }
$archiveInventory = $archiveJson | ConvertFrom-Json
if ($archiveInventory.files -ne ($mediaExpectedRows + 1)) { throw "Live archive did not contain DB plus all referenced images" }
$checkedJson = & cargo run --locked --release --bin legura-maintenance -- archive-verify $archivePath
if ($LASTEXITCODE -ne 0) { throw "Live media archive verification failed" }
$checked = $checkedJson | ConvertFrom-Json
if ($checked.files -ne $archiveInventory.files -or $checked.bytes -ne $archiveInventory.bytes) { throw "Archive verification inventory changed" }
$recoveryRoot = Join-Path $fixture 'recovered-media'
$restoredJson = & cargo run --locked --release --bin legura-maintenance -- archive-restore $archivePath $recoveryRoot
if ($LASTEXITCODE -ne 0) { throw "Inactive media recovery failed" }
$restored = $restoredJson | ConvertFrom-Json
if ($restored.files -ne $checked.files -or $restored.bytes -ne $checked.bytes) { throw "Recovery inventory changed" }
$recoveredDb = Join-Path $recoveryRoot 'database.sqlite'
$recoveredMedia = Join-Path $recoveryRoot 'media/local-media'
$recoveredInventoryJson = & cargo run --locked --release --bin legura-maintenance -- media-check $recoveredDb local-media $recoveredMedia
if ($LASTEXITCODE -ne 0) { throw "Recovered media failed independent audit" }
$recoveredInventory = $recoveredInventoryJson | ConvertFrom-Json
if ($recoveredInventory.objects -ne $inventory.objects -or $recoveredInventory.bytes -ne $inventory.bytes) { throw "Recovered objects do not match live snapshot" }
$comparison = @'
import json, pathlib, sqlite3, sys
with sqlite3.connect(sys.argv[1]) as source, sqlite3.connect(sys.argv[2]) as recovered:
    for table in ("legura_users", "legura_credentials", "legura_installation", "legura_posts", "legura_media", "legura_post_images", "_axonyx_migrations"):
        assert source.execute(f'SELECT * FROM "{table}" ORDER BY 1').fetchall() == recovered.execute(f'SELECT * FROM "{table}" ORDER BY 1').fetchall(), table
root = pathlib.Path(sys.argv[3])
receipt = json.loads((root / "recovery.json").read_text())
assert receipt["status"] == "recovered-not-activated"
assert receipt["cms_compatibility"] == "not-checked"
assert not (root / ".env").exists()
'@
& $python.Source -c $comparison $db $recoveredDb $recoveryRoot
if ($LASTEXITCODE -ne 0) { throw "Recovery altered identity, posts, media or migration rows" }
$compatibleJson = & cargo run --locked --release --bin legura-maintenance -- archive-check-recovery $archivePath $recoveryRoot
if ($LASTEXITCODE -ne 0) { throw "Recovered current product failed schema/migration/content/media compatibility" }
$compatible = $compatibleJson | ConvertFrom-Json
if ($compatible.files -ne $checked.files -or $compatible.bytes -ne $checked.bytes) { throw "Compatibility check changed inventory" }
# Certification is read-only: it does not rewrite receipts, apply migrations or
# change the live configuration. The original payloads remain authoritative.
& $python.Source -c $comparison $db $recoveredDb $recoveryRoot
if ($LASTEXITCODE -ne 0) { throw "Read-only recovery check changed product data or receipt" }
# Repeated recovery must fail, leaving both source and recovered data intact.
& cargo run --locked --release --bin legura-maintenance -- archive-restore $archivePath $recoveryRoot
if ($LASTEXITCODE -eq 0) { throw "Repeated recovery overwrote an existing destination" }
& $python.Source -c $comparison $db $recoveredDb $recoveryRoot
if ($LASTEXITCODE -ne 0) { throw "Refused overwrite damaged recovered data" }
Assert-MediaCounts -Rows $mediaExpectedRows -Files $mediaExpectedFiles
Invoke-Request -Path '/admin/media' -Headers @{ Cookie = $cookie } | Out-Null
Write-Host 'Live media archive/recovery proof passed without stopping or changing the running disposable CMS; recovery remains inactive.'
. (Join-Path $root 'tests/media-http/recovery-activation.ps1')
