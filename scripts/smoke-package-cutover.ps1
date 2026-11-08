param(
  [Parameter(Mandatory)][string] $PriorPackage,
  [Parameter(Mandatory)][string] $CurrentPackage,
  [ValidateRange(1024, 65535)][int] $Port = 3947
)
$ErrorActionPreference = 'Stop'
if ($PSVersionTable.PSVersion -lt [version]'7.4') { throw 'Package cutover smoke requires PowerShell 7.4 or later' }
$prior = [IO.Path]::GetFullPath($PriorPackage)
$current = [IO.Path]::GetFullPath($CurrentPackage)
if ($prior -eq $current) { throw 'Cutover requires two distinct package directories' }
$extension = if ($IsWindows) { '.exe' } else { '' }
foreach ($package in @(@{ path = $prior; version = '0.0.1' }, @{ path = $current; version = '0.0.2' })) {
  $manifest = Get-Content -LiteralPath (Join-Path $package.path 'package.json') -Raw | ConvertFrom-Json
  if ($manifest.product -cne 'legura' -or $manifest.version -cne $package.version) {
    throw "Expected a genuine Legura $($package.version) package at $($package.path)"
  }
  if (Test-Path -LiteralPath (Join-Path $package.path '.env')) { throw 'Package must not contain private configuration' }
  $maintenance = Join-Path $package.path "legura-maintenance$extension"
  & $maintenance verify-package $package.path | Out-Null
  if ($LASTEXITCODE -ne 0) { throw "Package integrity failed: $($package.version)" }
}

$temporaryRoot = [IO.Path]::GetFullPath([IO.Path]::GetTempPath())
$fixture = Join-Path $temporaryRoot ('legura-package-cutover-' + [Guid]::NewGuid().ToString('N'))
$baseUrl = "http://127.0.0.1:$Port"
$process = $null
$client = $null
$environment = @{}
$keys = @(Get-ChildItem Env: | Where-Object Name -Match '^(AX_SECRET_|AX_PUBLIC_|AXONYX_|DB_|DATABASE_|DATA_|SESSION_|SETUP_TOKEN$)' | Select-Object -ExpandProperty Name)
foreach ($key in $keys) {
  $environment[$key] = [Environment]::GetEnvironmentVariable($key)
  [Environment]::SetEnvironmentVariable($key, $null)
}

function New-Client {
  $handler = [Net.Http.HttpClientHandler]::new()
  $handler.UseCookies = $true
  $handler.CookieContainer = [Net.CookieContainer]::new()
  $handler.AllowAutoRedirect = $false
  return [Net.Http.HttpClient]::new($handler)
}

function Request {
  param([string] $Path, [int] $Status, [string] $Body = '', [hashtable] $Headers = @{})
  $method = if ($Body) { [Net.Http.HttpMethod]::Post } else { [Net.Http.HttpMethod]::Get }
  $request = [Net.Http.HttpRequestMessage]::new($method, "$baseUrl$Path")
  foreach ($key in $Headers.Keys) { $request.Headers.Add($key, [string]$Headers[$key]) }
  if ($Body) { $request.Content = [Net.Http.StringContent]::new($Body, [Text.Encoding]::UTF8, 'application/x-www-form-urlencoded') }
  $response = $null
  try {
    $response = $client.SendAsync($request).GetAwaiter().GetResult()
    if ([int]$response.StatusCode -ne $Status) { throw "Expected HTTP $Status for $Path, got $([int]$response.StatusCode)" }
    return $response.Content.ReadAsStringAsync().GetAwaiter().GetResult()
  } finally {
    if ($response) { $response.Dispose() }
    $request.Dispose()
  }
}

function Csrf {
  return (Request -Path '/__axonyx/csrf' -Status 200 -Headers @{ Origin = $baseUrl } | ConvertFrom-Json).token
}

function Start-Site {
  param([string] $Package)
  $options = @{
    FilePath = 'pwsh'
    ArgumentList = @('-NoProfile', '-File', (Join-Path $Package 'start.ps1'), '-Port', "$Port")
    WorkingDirectory = $Package
    PassThru = $true
  }
  if ($IsWindows) { $options.WindowStyle = 'Hidden' }
  $server = Start-Process @options
  $lastReadinessError = ''
  try {
    for ($attempt = 0; $attempt -lt 120; $attempt++) {
      if ($server.HasExited) { throw "Package server exited before readiness: $Package" }
      try { Request -Path '/__axonyx/ready' -Status 200 | Out-Null; return $server } catch {
        $lastReadinessError = $_.Exception.Message
        Start-Sleep -Milliseconds 250
      }
    }
    throw "Package server did not become ready: $Package; last HTTP error: $lastReadinessError"
  } catch {
    if (!$server.HasExited) { $server.Kill($true) }
    $server.WaitForExit()
    $server.Dispose()
    throw
  }
}

function Stop-Site {
  param([Diagnostics.Process] $Server)
  if (!$Server.HasExited) { $Server.Kill($true) }
  $Server.WaitForExit()
  $Server.Dispose()
}

function Assert-Port-Free {
  $listener = [Net.Sockets.TcpListener]::new([Net.IPAddress]::Loopback, $Port)
  try { $listener.Start() } catch { throw "Test server port $Port remains occupied after stopping its process" } finally { $listener.Stop() }
}

try {
  Assert-Port-Free
  New-Item -ItemType Directory -Path $fixture | Out-Null
  $data = Join-Path $fixture 'private-data'
  $media = Join-Path $data 'media'
  New-Item -ItemType Directory -Path $data, $media | Out-Null
  if (!$IsWindows) {
    $mode = [IO.UnixFileMode]::UserRead -bor [IO.UnixFileMode]::UserWrite -bor [IO.UnixFileMode]::UserExecute
    [IO.File]::SetUnixFileMode($data, $mode)
    [IO.File]::SetUnixFileMode($media, $mode)
  }
  $database = Join-Path $data 'legura.db'
  $oldMaintenance = Join-Path $prior "legura-maintenance$extension"
  $newMaintenance = Join-Path $current "legura-maintenance$extension"
  & $oldMaintenance init $database | Out-Null
  if ($LASTEXITCODE -ne 0) { throw '0.0.1 could not initialize its private database' }

  $session = [Convert]::ToHexString([Security.Cryptography.RandomNumberGenerator]::GetBytes(32)).ToLowerInvariant()
  $setup = [Convert]::ToHexString([Security.Cryptography.RandomNumberGenerator]::GetBytes(32)).ToLowerInvariant()
  $password = 'NativeCutover9!'
  $config = "AX_SECRET_DB_URL=sqlite:$($database.Replace('\', '/'))`nAX_SECRET_DB_DIALECT=sqlite`nAX_SECRET_SESSION_KEY=$session`nAX_SECRET_SETUP_TOKEN=$setup`nAX_SECRET_SESSION_COOKIE_SECURE=false`nAX_SECRET_MEDIA_DRIVER=local`nAX_SECRET_MEDIA_ROOT=$($media.Replace('\', '/'))`nAX_SECRET_MEDIA_STORE_ID=local-media`n"
  $oldConfig = Join-Path $prior '.env'
  $newConfig = Join-Path $current '.env'
  [IO.File]::WriteAllText($oldConfig, $config, [Text.UTF8Encoding]::new($false))
  if (!$IsWindows) { [IO.File]::SetUnixFileMode($oldConfig, [IO.UnixFileMode]::UserRead -bor [IO.UnixFileMode]::UserWrite) }
  $configHash = (Get-FileHash -LiteralPath $oldConfig -Algorithm SHA256).Hash
  $client = New-Client
  $process = Start-Site -Package $prior
  Request -Path '/admin' -Status 403 | Out-Null
  Request -Path '/__axonyx/action?path=%2Fsetup&name=Install' -Status 303 -Body "siteName=Cutover+fixture&email=cutover%40example.com&password=$password&setupToken=$setup" -Headers @{ Origin = $baseUrl; Accept = 'text/html'; 'X-Axonyx-CSRF' = (Csrf) } | Out-Null
  Request -Path '/admin' -Status 200 | Out-Null
  Request -Path '/__axonyx/action?path=%2Fadmin%2Fposts%2Fnew&name=CreatePost' -Status 303 -Body 'title=Cross+version+post&slug=cross-version&body=Persisted+across+the+cutover&status=published' -Headers @{ Origin = $baseUrl; Accept = 'text/html'; 'X-Axonyx-CSRF' = (Csrf) } | Out-Null
  if (!(Request -Path '/posts/cross-version' -Status 200).Contains('Persisted across the cutover')) { throw '0.0.1 could not publish its post' }

  $image = [Convert]::FromBase64String('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGNwqzjxHwAFFAKG7uYx/AAAAABJRU5ErkJggg==')
  $upload = [Net.Http.HttpRequestMessage]::new([Net.Http.HttpMethod]::Post, "$baseUrl/__axonyx/action?path=%2Fadmin%2Fmedia&name=UploadLibraryImage")
  $part = [Net.Http.ByteArrayContent]::new($image)
  $part.Headers.ContentType = [Net.Http.Headers.MediaTypeHeaderValue]::new('image/png')
  $multipart = [Net.Http.MultipartFormDataContent]::new()
  $multipart.Add($part, 'image', 'fixture.png')
  $upload.Content = $multipart
  $upload.Headers.Add('Origin', $baseUrl)
  $upload.Headers.Add('Accept', 'application/ax-patch+json')
  $upload.Headers.Add('X-Axonyx-CSRF', (Csrf))
  try {
    $response = $client.SendAsync($upload).GetAwaiter().GetResult()
    try {
      if ([int]$response.StatusCode -ne 200) { throw "0.0.1 media upload returned HTTP $([int]$response.StatusCode)" }
      $uploaded = $response.Content.ReadAsStringAsync().GetAwaiter().GetResult() | ConvertFrom-Json
      if (!$uploaded.ok -or $uploaded.value.storage -cne 'local-media') { throw '0.0.1 upload returned an invalid media reference' }
      $mediaId = [string]$uploaded.value.id
    } finally { $response.Dispose() }
  } finally { $upload.Dispose() }
  $mediaObject = Join-Path $media $mediaId
  $mediaHash = (Get-FileHash -LiteralPath $mediaObject -Algorithm SHA256).Hash

  Stop-Site -Server $process
  $process = $null
  Assert-Port-Free
  $client.Dispose()
  $client = New-Client
  $backup = Join-Path $fixture 'before-cutover.legura'
  & $oldMaintenance archive-backup $database local-media $media $backup | Out-Null
  if ($LASTEXITCODE -ne 0) { throw '0.0.1 could not back up its stopped database and image' }
  & $newMaintenance archive-verify $backup | Out-Null
  if ($LASTEXITCODE -ne 0) { throw '0.0.2 could not verify the pre-cutover archive' }
  $recovery = Join-Path $fixture 'inactive-recovery'
  & $newMaintenance archive-restore $backup $recovery | Out-Null
  if ($LASTEXITCODE -ne 0) { throw '0.0.2 could not restore the pre-cutover archive' }
  & $newMaintenance archive-check-recovery $backup $recovery | Out-Null
  if ($LASTEXITCODE -ne 0) { throw '0.0.2 rejected pre-cutover data or media' }

  Copy-Item -LiteralPath $oldConfig -Destination $newConfig
  if (!$IsWindows) { [IO.File]::SetUnixFileMode($newConfig, [IO.UnixFileMode]::UserRead -bor [IO.UnixFileMode]::UserWrite) }
  if ((Get-FileHash -LiteralPath $newConfig -Algorithm SHA256).Hash -ne $configHash) { throw 'Cutover changed the private configuration' }
  $process = Start-Site -Package $current
  if (!(Request -Path '/posts/cross-version' -Status 200).Contains('Persisted across the cutover')) { throw '0.0.2 lost the public post' }
  Request -Path '/admin' -Status 403 | Out-Null
  Request -Path '/setup' -Status 403 | Out-Null
  Request -Path '/__axonyx/action?path=%2Flogin&name=SignIn' -Status 303 -Body "email=cutover%40example.com&password=$password" -Headers @{ Origin = $baseUrl; Accept = 'text/html'; 'X-Axonyx-CSRF' = (Csrf) } | Out-Null
  Request -Path '/admin' -Status 200 | Out-Null
  $library = Request -Path '/admin/media' -Status 200
  if (!$library.Contains($mediaId)) { throw '0.0.2 lost the private media library entry' }
  $privateImage = [Net.Http.HttpRequestMessage]::new([Net.Http.HttpMethod]::Get, "$baseUrl/api/media/$mediaId")
  try {
    $response = $client.SendAsync($privateImage).GetAwaiter().GetResult()
    try {
      if ([int]$response.StatusCode -ne 200 -or $response.Content.Headers.ContentType.MediaType -ne 'image/png') { throw '0.0.2 could not serve the private image' }
      $bytes = $response.Content.ReadAsByteArrayAsync().GetAwaiter().GetResult()
      if ([Convert]::ToBase64String($bytes) -cne [Convert]::ToBase64String([IO.File]::ReadAllBytes($mediaObject))) { throw 'Private image bytes changed across cutover' }
    } finally { $response.Dispose() }
  } finally { $privateImage.Dispose() }
  & $newMaintenance media-check $database local-media $media | Out-Null
  if ($LASTEXITCODE -ne 0 -or (Get-FileHash -LiteralPath $mediaObject -Algorithm SHA256).Hash -ne $mediaHash) { throw '0.0.2 media audit failed or original image changed' }
  if ((Get-FileHash -LiteralPath $oldConfig -Algorithm SHA256).Hash -ne $configHash -or
      (Get-FileHash -LiteralPath $newConfig -Algorithm SHA256).Hash -ne $configHash) { throw 'Server cutover modified private configuration' }

  Request -Path '/__axonyx/action?path=%2Fadmin%2Fposts%2Fnew&name=CreatePost' -Status 303 -Body 'title=Post+after+cutover&slug=after-cutover&body=Do+not+discard+this+write&status=published' -Headers @{ Origin = $baseUrl; Accept = 'text/html'; 'X-Axonyx-CSRF' = (Csrf) } | Out-Null
  if (!(Request -Path '/posts/after-cutover' -Status 200).Contains('Do not discard this write')) { throw '0.0.2 did not persist its new post' }
  Stop-Site -Server $process
  $process = $null
  Assert-Port-Free
  $client.Dispose()
  $client = New-Client
  $afterBackup = Join-Path $fixture 'after-cutover.legura'
  & $newMaintenance archive-backup $database local-media $media $afterBackup | Out-Null
  if ($LASTEXITCODE -ne 0) { throw '0.0.2 could not preserve post-cutover writes' }
  & $newMaintenance archive-verify $afterBackup | Out-Null
  if ($LASTEXITCODE -ne 0) { throw '0.0.2 could not verify post-cutover writes' }
  & $oldMaintenance archive-verify $afterBackup *> $null
  if ($LASTEXITCODE -eq 0) { throw '0.0.1 incorrectly accepted a 0.0.2 archive' }

  $afterRecovery = Join-Path $fixture 'after-cutover-recovery'
  & $newMaintenance archive-restore $afterBackup $afterRecovery | Out-Null
  if ($LASTEXITCODE -ne 0) { throw 'Could not recover the post-cutover archive to a new location' }
  & $newMaintenance archive-check-recovery $afterBackup $afterRecovery | Out-Null
  if ($LASTEXITCODE -ne 0) { throw 'Post-cutover recovery failed its content/media check' }
  $beforeRecovery = Join-Path $fixture 'before-cutover-recovery'
  & $oldMaintenance archive-restore $backup $beforeRecovery | Out-Null
  if ($LASTEXITCODE -ne 0) { throw 'Could not recover the 0.0.1 archive to a new location' }
  & $oldMaintenance archive-check-recovery $backup $beforeRecovery | Out-Null
  if ($LASTEXITCODE -ne 0) { throw 'Pre-cutover recovery failed its content/media check' }
  $compare = @'
import pathlib, sqlite3, sys
def slugs(path):
    uri = pathlib.Path(path).resolve(strict=True).as_uri() + '?mode=ro'
    with sqlite3.connect(uri, uri=True) as db:
        return {row[0] for row in db.execute('SELECT slug FROM legura_posts')}
before, after = (slugs(path) for path in sys.argv[1:3])
if before != {'cross-version'} or after != {'cross-version', 'after-cutover'}:
    raise SystemExit('Post-cutover write was not isolated in the newer archive')
'@
  & python -c $compare (Join-Path $beforeRecovery 'database.sqlite') (Join-Path $afterRecovery 'database.sqlite')
  if ($LASTEXITCODE -ne 0) { throw 'Could not prove the two recovered databases have different posts' }
  foreach ($location in @($beforeRecovery, $afterRecovery)) {
    $restoredImage = Join-Path $location "media/local-media/$mediaId"
    if ((Get-FileHash -LiteralPath $restoredImage -Algorithm SHA256).Hash -ne $mediaHash) { throw 'A recovered archive lost the original image' }
  }
  $process = Start-Site -Package $current
  if (!(Request -Path '/posts/after-cutover' -Status 200).Contains('Do not discard this write')) { throw 'Offline recovery changed the restarted 0.0.2 installation' }
  Request -Path '/admin' -Status 403 | Out-Null
  Request -Path '/__axonyx/action?path=%2Flogin&name=SignIn' -Status 303 -Body "email=cutover%40example.com&password=$password" -Headers @{ Origin = $baseUrl; Accept = 'text/html'; 'X-Axonyx-CSRF' = (Csrf) } | Out-Null
  Request -Path '/admin' -Status 200 | Out-Null
  if ((Get-FileHash -LiteralPath $oldConfig -Algorithm SHA256).Hash -ne $configHash -or
      (Get-FileHash -LiteralPath $newConfig -Algorithm SHA256).Hash -ne $configHash) { throw 'Offline recovery changed the private configuration' }
  Write-Host 'Native package cutover passed: genuine 0.0.1 to 0.0.2 server, external SQLite/media, stopped-writer backups, public post, fresh admin login, locked setup and exact private image bytes.'
  Write-Host 'Post-cutover safety gate passed: both snapshots were taken with the test server stopped and verified in separate inactive locations; the older archive lacks the new post, so automatic rollback remains unsupported.'
} finally {
  if ($process) { Stop-Site -Server $process }
  if ($client) { $client.Dispose() }
  foreach ($key in $keys) { [Environment]::SetEnvironmentVariable($key, $environment[$key]) }
  $resolved = [IO.Path]::GetFullPath($fixture)
  if ($resolved.StartsWith($temporaryRoot, [StringComparison]::OrdinalIgnoreCase) -and
      (Split-Path $resolved -Leaf) -match '^legura-package-cutover-[0-9a-f]{32}$') {
    if (Test-Path -LiteralPath $resolved) { Remove-Item -LiteralPath $resolved -Recurse -Force }
  } else { throw 'Refusing cleanup outside the isolated cutover fixture' }
  foreach ($path in @((Join-Path $prior '.env'), (Join-Path $current '.env'))) {
    if (Test-Path -LiteralPath $path) { Remove-Item -LiteralPath $path -Force }
  }
}
exit 0
