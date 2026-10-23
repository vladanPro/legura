# Runs after archive/recovery acceptance, only in the disposable media fixture.
$deletePath = '/__axonyx/action?path=%2Fadmin%2Fmedia&name=DeleteMedia'
$retryPath = '/__axonyx/action?path=%2Fadmin%2Fmedia&name=RetryMediaCleanup'
$used = $first[0]
Invoke-Request -Path "/admin/media/$used/delete" -Status 403 | Out-Null
Invoke-Request -Path $deletePath -Method POST -Body "id=$used&confirmation=delete" -Status 403 | Out-Null
Invoke-Request -Path $deletePath -Method POST -Body "id=$used&confirmation=delete" -Headers @{Cookie=$cookie} -Status 403 | Out-Null
$confirmation = Invoke-Request -Path $deletePath -Method POST -Body "id=$used" -Headers $sessionProof -Status 422
if (!$confirmation.Body.Contains('Confirm permanent deletion')) { throw 'Missing confirmation feedback' }
$rejected = Invoke-Request -Path $deletePath -Method POST -Body "id=$used&confirmation=delete&administrator_id=forged" -Headers $sessionProof -Status 422
if (!$rejected.Body.Contains('including drafts')) { throw 'Referenced media feedback missing' }
Assert-PrivateMediaRead $used $cookie 200

$fresh = ((Send-Media -Bytes $validImage -Headers $sessionProof -Status 200).Body | ConvertFrom-Json).value.id
$confirm = Invoke-Request -Path "/admin/media/$fresh/delete" -Headers @{Cookie=$cookie}
if (!$confirm.Body.Contains('Cancel deletion')) { throw 'No native confirmation screen' }
Assert-PrivateMediaRead $fresh $cookie 200
$deleted = Invoke-Request -Path $deletePath -Method POST -Body "id=$fresh&confirmation=delete" -Headers $sessionProof -Status 303
if ($deleted.Headers.Location -ne '/admin/media') { throw "Incorrect deletion redirect: $($deleted.Headers.Location); object remains: $(Test-Path -LiteralPath (Join-Path $mediaRoot $fresh))" }
Assert-PrivateMediaRead $fresh $cookie 404
if (Test-Path -LiteralPath (Join-Path $mediaRoot $fresh)) { throw 'Deleted file remains' }
Invoke-Request -Path $deletePath -Method POST -Body "id=$fresh&confirmation=delete" -Headers $sessionProof -Status 404 | Out-Null
Invoke-Request -Path "/admin/media/$fresh/delete" -Headers @{Cookie=$cookie} -Status 404 | Out-Null

# A directory occupying the fixture object key forces portable cleanup refusal.
$failed = ((Send-Media -Bytes $validImage -Headers $sessionProof -Status 200).Body | ConvertFrom-Json).value.id
$failedObject = Join-Path $mediaRoot $failed
[IO.File]::Delete($failedObject)
[IO.Directory]::CreateDirectory($failedObject) | Out-Null
try {
  $partial = Invoke-Request -Path $deletePath -Method POST -Body "id=$failed&confirmation=delete" -Headers $sessionProof -Status 422
  if (!$partial.Body.Contains('private file could not be cleaned up')) { throw 'Cleanup failure was reported as full success' }
  Assert-PrivateMediaRead $failed $cookie 404
  $retryPending = Invoke-Request -Path $retryPath -Method POST -Body "id=$failed" -Headers $sessionProof -Status 422
  if (!$retryPending.Body.Contains('Private file cleanup is still pending')) { throw 'Retry failure did not render operation feedback' }
} finally { [IO.Directory]::Delete($failedObject) }
$pending = & $python.Source -c 'import sqlite3,sys; d=sqlite3.connect(sys.argv[1]); print(d.execute("SELECT count(*) FROM legura_media_cleanup WHERE id=?", (sys.argv[2],)).fetchone()[0]); d.close()' $db $failed
if ($LASTEXITCODE -ne 0 -or $pending -ne '1') { throw 'Cleanup failure lost its durable receipt' }
$pendingPage = Invoke-Request -Path '/admin/media' -Headers @{ Cookie = $cookie }
if (!$pendingPage.Body.Contains('Storage cleanup needed') -or !$pendingPage.Body.Contains($failed)) { throw 'Administrator cannot see pending cleanup' }
Invoke-Request -Path $retryPath -Method POST -Body "id=$failed" -Status 403 | Out-Null
Invoke-Request -Path $retryPath -Method POST -Body "id=$failed" -Headers @{ Cookie = $cookie } -Status 403 | Out-Null

if ($MediaBrowser) {
  $browserIds = @(1..2 | ForEach-Object { ((Send-Media -Bytes $validImage -Headers $sessionProof -Status 200).Body | ConvertFrom-Json).value.id })
  @{baseUrl=$baseUrl; cookie=$cookie; used=$used; ids=$browserIds} | ConvertTo-Json -Compress | & node (Join-Path $root 'tests/media-delete-browser.mjs')
  if ($LASTEXITCODE -ne 0) { throw 'Media delete browser proof failed' }
  foreach ($id in $browserIds) { Assert-PrivateMediaRead $id $cookie 404 }
}
Stop-Fixture
Start-Fixture
Assert-PrivateMediaRead $fresh $cookie 404
Assert-PrivateMediaRead $used $cookie 200
$stillPending = Invoke-Request -Path '/admin/media' -Headers @{ Cookie = $cookie }
if (!$stillPending.Body.Contains('Storage cleanup needed')) { throw 'Cleanup receipt was lost across restart' }
$retryProof = Get-Proof -Cookie $cookie
$cleaned = Invoke-Request -Path $retryPath -Method POST -Body "id=$failed" -Headers $retryProof -Status 303
if ($cleaned.Headers.Location -ne '/admin/media') { throw 'Cleanup retry did not return to the library' }
$cleared = & $python.Source -c 'import sqlite3,sys; d=sqlite3.connect(sys.argv[1]); print(d.execute("SELECT count(*) FROM legura_media_cleanup WHERE id=?", (sys.argv[2],)).fetchone()[0]); d.close()' $db $failed
if ($LASTEXITCODE -ne 0 -or $cleared -ne '0') { throw 'Successful retry left a cleanup receipt' }
Invoke-Request -Path $retryPath -Method POST -Body "id=$failed" -Headers $retryProof -Status 404 | Out-Null
Write-Host 'Media deletion HTTP passed: native confirmation, session/CSRF, referenced refusal, durable cleanup retry and restart.'
