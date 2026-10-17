# Dot-sourced only inside smoke-auth's disposable, session-backed HTTP fixture.
$mediaAction = "/__axonyx/action?path=%2Fadmin%2Fmedia-proof&name=UploadLibraryImage"
$encoded = & $python.Source -c 'import struct,zlib,base64;chunk=lambda k,b:struct.pack(">I",len(b))+k+b+struct.pack(">I",zlib.crc32(k+b));data=b"\x89PNG\r\n\x1a\n"+chunk(b"IHDR",struct.pack(">IIBBBBB",1,1,8,6,0,0,0))+chunk(b"IDAT",zlib.compress(bytes([0,70,120,200,255])))+chunk(b"IEND",b"")+b"<script>not-an-image-payload</script>";print(base64.b64encode(data).decode())'
if ($LASTEXITCODE -ne 0) { throw "Cannot create isolated image bytes" }
$validImage = [Convert]::FromBase64String($encoded)

function Assert-MediaCounts([int]$Rows, [int]$Files) {
  $count = & $python.Source -c 'import sqlite3,sys;d=sqlite3.connect(sys.argv[1]);print(d.execute("select count(*) from legura_media").fetchone()[0]);d.close()' $db
  if ($LASTEXITCODE -ne 0 -or $count -ne "$Rows") { throw "Unexpected media row count: $count != $Rows" }
  if (@(Get-ChildItem -LiteralPath $mediaRoot -File).Count -ne $Files) { throw "Unexpected media object count" }
}

function Send-Media([byte[]]$Bytes, [hashtable]$Headers, [int]$Status, [switch]$Native) {
  $action = if ($Native) { $mediaAction + 'AndReturn' } else { $mediaAction }
  $request = [Net.Http.HttpRequestMessage]::new([Net.Http.HttpMethod]::Post, "$baseUrl$action")
  $multipart = [Net.Http.MultipartFormDataContent]::new()
  $part = [Net.Http.ByteArrayContent]::new($Bytes)
  $part.Headers.ContentType = [Net.Http.Headers.MediaTypeHeaderValue]::new("text/html")
  $multipart.Add($part, "image", "../../untrusted.svg")
  $request.Content = $multipart
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

Invoke-Request -Path "/admin/media-proof" -Status 403 | Out-Null
$anonymous = Get-Proof
Send-Media -Bytes $validImage -Headers $anonymous -Status 403 | Out-Null
Send-Media -Bytes $validImage -Headers $anonymous -Status 403 -Native | Out-Null
Send-Media -Bytes $validImage -Headers @{ Cookie = $cookie; Origin = $baseUrl } -Status 403 -Native | Out-Null
Send-Media -Bytes $validImage -Headers @{ Cookie = $cookie; Origin = $baseUrl } -Status 403 | Out-Null
Send-Media -Bytes $validImage -Headers @{ Cookie = $cookie; Origin = $baseUrl; 'X-Axonyx-CSRF' = 'forged' } -Status 403 | Out-Null
Send-Media -Bytes $validImage -Headers @{ Cookie = $cookie; Origin = 'https://attacker.example'; 'X-Axonyx-CSRF' = $sessionProof['X-Axonyx-CSRF'] } -Status 403 | Out-Null
Assert-MediaCounts -Rows 0 -Files 0
$empty = Invoke-Request -Path '/admin/media-proof?page=999' -Headers @{ Cookie = $cookie }
if (!$empty.Body.Contains('No images yet') -or !$empty.Body.Contains('data-media-page="1"') -or !$empty.Body.Contains('data-media-total="0"')) { throw "Empty media library is not bounded" }
if (!$empty.Body.Contains('aria-label="No image selected"')) { throw "Empty media library has no accessible image placeholder" }
if ($empty.Body -notmatch 'href="/admin/media-proof"[^>]*aria-current="true"|aria-current="true"[^>]*href="/admin/media-proof"') { throw "Media sidebar item is not selected" }

foreach ($bytes in @([Text.Encoding]::UTF8.GetBytes('<svg onload="alert(1)"/>'), [byte[]]::new(0), [byte[]]::new(8388609))) {
  $response = Send-Media -Bytes $bytes -Headers $sessionProof -Status 422
  if ($response.Body -match 'untrusted.svg|onload|private-media') { throw "Invalid upload echoed untrusted or private input" }
  Assert-MediaCounts -Rows 0 -Files 0
}
Send-Media -Bytes ([byte[]]::new(9437185)) -Headers $sessionProof -Status 400 | Out-Null
Assert-MediaCounts -Rows 0 -Files 0

$result = (Send-Media -Bytes $validImage -Headers $sessionProof -Status 200).Body | ConvertFrom-Json
if (!$result.ok -or $result.value.storage -ne 'local-media' -or $result.value.content_type -ne 'image/png' -or $result.value.file_name -ne "$($result.value.id).png") { throw "Uploaded FileRef is not canonical" }
$mediaId = $result.value.id
Assert-MediaCounts -Rows 1 -Files 1
Assert-StoredMedia
$page = Invoke-Request -Path '/admin/media-proof' -Headers @{ Cookie = $cookie }
if (!$page.Body.Contains($mediaId) -or $page.Headers['Cache-Control'] -notmatch 'no-store') { throw "Guarded library rendering/caching failed" }
Invoke-Request -Path "/private-media/$mediaId" -Status 404 | Out-Null
Stop-Fixture
Start-Fixture
Assert-StoredMedia
$page = Invoke-Request -Path '/admin/media-proof' -Headers @{ Cookie = $cookie }
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
$native = Send-Media -Bytes $validImage -Headers $sessionProof -Status 303 -Native
if ($native.Headers['Location'] -ne '/admin/media-proof') { throw "Native upload did not return to the library" }
for ($index = 0; $index -lt 8; $index++) { Send-Media -Bytes $validImage -Headers $sessionProof -Status 200 | Out-Null }
Assert-MediaCounts -Rows 11 -Files 12
Assert-StoredMedia

function Get-MediaPageIds([string]$Query, [int]$Page, [int]$Count) {
  $response = Invoke-Request -Path "/admin/media-proof$Query" -Headers @{ Cookie = $cookie }
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
