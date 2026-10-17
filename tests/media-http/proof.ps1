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

function Send-Media([byte[]]$Bytes, [hashtable]$Headers, [int]$Status) {
  $request = [Net.Http.HttpRequestMessage]::new([Net.Http.HttpMethod]::Post, "$baseUrl$mediaAction")
  $multipart = [Net.Http.MultipartFormDataContent]::new()
  $part = [Net.Http.ByteArrayContent]::new($Bytes)
  $part.Headers.ContentType = [Net.Http.Headers.MediaTypeHeaderValue]::new("text/html")
  $multipart.Add($part, "image", "../../untrusted.svg")
  $request.Content = $multipart
  foreach ($key in $Headers.Keys) {
    if ($key -ne 'Accept') { $request.Headers.Add($key, [string]$Headers[$key]) }
  }
  $request.Headers.Add('Accept', 'application/ax-patch+json')
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
Send-Media -Bytes $validImage -Headers @{ Cookie = $cookie; Origin = $baseUrl } -Status 403 | Out-Null
Send-Media -Bytes $validImage -Headers @{ Cookie = $cookie; Origin = $baseUrl; 'X-Axonyx-CSRF' = 'forged' } -Status 403 | Out-Null
Send-Media -Bytes $validImage -Headers @{ Cookie = $cookie; Origin = 'https://attacker.example'; 'X-Axonyx-CSRF' = $sessionProof['X-Axonyx-CSRF'] } -Status 403 | Out-Null
Assert-MediaCounts -Rows 0 -Files 0

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
Write-Host 'Media HTTP proof passed: real admin/session/CSRF, bounded multipart, canonical image bytes, SQLite metadata, restart and explicit orphan-on-DB-failure policy.'
