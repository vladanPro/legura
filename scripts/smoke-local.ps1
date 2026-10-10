param([ValidateRange(1024, 65535)][int] $Port = 3943)
$ErrorActionPreference = "Stop"
$root = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot ".."))
$temporaryRoot = [IO.Path]::GetFullPath([IO.Path]::GetTempPath())
$fixture = Join-Path $temporaryRoot ("legura-local-" + [Guid]::NewGuid().ToString("N"))
$baseUrl = "http://127.0.0.1:$Port"
$process = $null
$client = $null
$handler = $null
$environment = @{}
$keys = @(Get-ChildItem Env: | Where-Object Name -Match '^(AX_SECRET_|AX_PUBLIC_|AXONYX_|DB_|DATABASE_|DATA_|SESSION_|SETUP_TOKEN$)' | Select-Object -ExpandProperty Name)
foreach ($key in $keys) {
  $environment[$key] = [Environment]::GetEnvironmentVariable($key)
  [Environment]::SetEnvironmentVariable($key, $null)
}

function Request {
  param([string] $Path, [int] $Status, [string] $Body = "", [hashtable] $Headers = @{})
  $method = if ($Body) { [Net.Http.HttpMethod]::Post } else { [Net.Http.HttpMethod]::Get }
  $request = [Net.Http.HttpRequestMessage]::new($method, "$baseUrl$Path")
  foreach ($key in $Headers.Keys) { $request.Headers.Add($key, [string]$Headers[$key]) }
  if ($Body) { $request.Content = [Net.Http.StringContent]::new($Body, [Text.Encoding]::UTF8, "application/x-www-form-urlencoded") }
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

function Assert-InitRefused {
  param([string] $Message)
  $refused = $false
  try { & ./scripts/local.ps1 -Task init | Out-Null } catch {
    if ($_.Exception.Message -notmatch $Message) { throw }
    $refused = $true
  }
  if (!$refused) { throw "Unsafe local initialization was accepted" }
}

try {
  $reservation = [Net.Sockets.TcpListener]::new([Net.IPAddress]::Loopback, $Port)
  try { $reservation.Start() } finally { $reservation.Stop() }
  New-Item -ItemType Directory -Path $fixture | Out-Null
  Push-Location $root
  try {
    $files = & git ls-files
    if ($LASTEXITCODE -ne 0) { throw "Cannot enumerate fixture source" }
    foreach ($file in $files) {
      if ($file -match '^(\.env$|data/|dist/|target/|src/generated/)') { continue }
      $target = Join-Path $fixture $file
      New-Item -ItemType Directory -Path (Split-Path $target) -Force | Out-Null
      Copy-Item -LiteralPath (Join-Path $root $file) -Destination $target
    }
  } finally { Pop-Location }
  Push-Location $fixture
  try {
    & ./scripts/local.ps1 -Task prepare -Port $Port
    $envPath = Join-Path $fixture ".env"
    $original = [IO.File]::ReadAllText($envPath)
    if ($IsWindows) { [IO.File]::SetAttributes($envPath, [IO.File]::GetAttributes($envPath) -bor [IO.FileAttributes]::Hidden) }
    $config = ConvertFrom-StringData $original
    if ($config.AX_SECRET_SESSION_KEY -notmatch '^[0-9A-F]{64}$' -or $config.AX_SECRET_SETUP_TOKEN -notmatch '^[0-9A-F]{64}$' -or $config.AX_SECRET_SESSION_KEY -eq $config.AX_SECRET_SETUP_TOKEN) {
      throw "Local preparation did not generate independent random secrets"
    }
    if (!$IsWindows -and [IO.File]::GetUnixFileMode($envPath) -ne ([IO.UnixFileMode]::UserRead -bor [IO.UnixFileMode]::UserWrite)) { throw "Generated .env is not owner-only" }
    & ./scripts/local.ps1 -Task init
    if ([IO.File]::ReadAllText($envPath) -cne $original) { throw "Repeated initialization replaced existing configuration" }
    & ./scripts/local.ps1 -Task check
    if ($IsWindows) { [IO.File]::SetAttributes($envPath, [IO.File]::GetAttributes($envPath) -band (-bnot [IO.FileAttributes]::Hidden)) }
    $database = Join-Path $fixture "data/legura.db"
    $before = (Get-FileHash -LiteralPath $database).Hash
    $env:AX_SECRET_DB_URL = "sqlite://outside.db"
    try { Assert-InitRefused -Message 'Process configuration overrides' } finally { Remove-Item Env:AX_SECRET_DB_URL }
    if (Test-Path -LiteralPath (Join-Path $fixture "outside.db")) { throw "Process override created an unexpected database" }
    try {
      [IO.File]::WriteAllText($envPath, $original.Replace('sqlite://data/legura.db', 'postgresql://invalid.example/legura'))
      Assert-InitRefused -Message 'only supports sqlite'
      [IO.File]::WriteAllText($envPath, $original.Replace('sqlite://data/legura.db', 'sqlite://DATA/legura.db'))
      Assert-InitRefused -Message 'only supports sqlite'
      [IO.File]::WriteAllText($envPath, $original.Replace($config.AX_SECRET_SETUP_TOKEN, $config.AX_SECRET_SESSION_KEY))
      Assert-InitRefused -Message 'must be independent'
      [IO.File]::WriteAllText($envPath, $original.Replace($config.AX_SECRET_SESSION_KEY, ''))
      Assert-InitRefused -Message 'Local secrets must'
      [IO.File]::WriteAllText($envPath, $original + "AX_SECRET_DB_URL=sqlite://outside.db`n")
      Assert-InitRefused -Message 'duplicate keys'
    } finally { [IO.File]::WriteAllText($envPath, $original) }
    if (!$IsWindows) {
      foreach ($path in @($envPath, $database)) {
        $saved = "$path.fixture-original"
        [IO.File]::Move($path, $saved)
        try {
          [IO.File]::CreateSymbolicLink($path, $saved) | Out-Null
          Assert-InitRefused -Message 'must not be (a symlink|symlinks)'
        } finally {
          if ([IO.File]::Exists($path)) { [IO.File]::Delete($path) }
          [IO.File]::Move($saved, $path)
        }
      }
    }
    if ((Get-FileHash -LiteralPath $database).Hash -ne $before) { throw "Refused preparation changed the local database" }
    $handler = [Net.Http.HttpClientHandler]::new()
    $handler.AllowAutoRedirect = $false
    $handler.CookieContainer = [Net.CookieContainer]::new()
    $client = [Net.Http.HttpClient]::new($handler)
    $client.Timeout = [TimeSpan]::FromSeconds(20)
    $options = @{
      FilePath = (Get-Command pwsh -ErrorAction Stop).Source
      ArgumentList = @('-NoProfile', '-File', 'scripts/local.ps1', '-Task', 'start', '-Port', "$Port")
      WorkingDirectory = $fixture
      PassThru = $true
      RedirectStandardOutput = (Join-Path $fixture "server.out")
      RedirectStandardError = (Join-Path $fixture "server.err")
    }
    if ($IsWindows) { $options.WindowStyle = "Hidden" }
    $process = Start-Process @options
    $ready = $false
    for ($attempt = 0; $attempt -lt 120; $attempt++) {
      if ($process.HasExited) { throw "Local start exited before readiness" }
      try { Request -Path "/setup" -Status 200 | Out-Null; $ready = $true; break } catch { Start-Sleep -Milliseconds 250 }
    }
    if (!$ready) { throw "Local start did not become ready" }
    $setupPage = Request -Path "/setup" -Status 200
    if ($setupPage.Contains($config.AX_SECRET_SETUP_TOKEN) -or $setupPage.Contains($config.AX_SECRET_SESSION_KEY)) { throw "Setup HTML exposed configuration secrets" }
    Request -Path "/admin" -Status 403 | Out-Null
    $proof = (Request -Path "/__axonyx/csrf" -Status 200 -Headers @{ Origin = $baseUrl } | ConvertFrom-Json).token
    $password = [Convert]::ToHexString([Security.Cryptography.RandomNumberGenerator]::GetBytes(5))
    $body = "siteName=Local+bootstrap&email=bootstrap%40example.com&password=$password&setupToken=$($config.AX_SECRET_SETUP_TOKEN)"
    $headers = @{ Origin = $baseUrl; Accept = "text/html"; "X-Axonyx-CSRF" = $proof }
    $invalid = Request -Path "/__axonyx/action?path=%2Fsetup&name=Install" -Status 422 -Body $body.Replace("password=$password", "password=$($password.Substring(0, 9))") -Headers $headers
    if (!$invalid.Contains('Use at least 10 characters.')) { throw "Nine-character password was not rejected with the expected validation" }
    $invalid = Request -Path "/__axonyx/action?path=%2Fsetup&name=Install" -Status 422 -Body $body.Replace("password=$password", "password=$('a' * 257)") -Headers $headers
    if (!$invalid.Contains('Use at most 256 characters.')) { throw "Oversized password was not rejected with the expected validation" }
    Request -Path "/__axonyx/action?path=%2Fsetup&name=Install" -Status 303 -Body $body -Headers @{ Origin = $baseUrl; Accept = "text/html"; "X-Axonyx-CSRF" = $proof } | Out-Null
    $admin = Request -Path "/admin" -Status 200
    if (!$admin.Contains('bootstrap@example.com') -or !$admin.Contains('Start with your first draft')) { throw "Local setup did not reach the empty administration overview" }
    Request -Path "/setup" -Status 403 | Out-Null
    $backups = Join-Path $fixture "backups"
    New-Item -ItemType Directory -Path $backups | Out-Null
    if (!$IsWindows) { [IO.File]::SetUnixFileMode($backups, [IO.UnixFileMode]::UserRead -bor [IO.UnixFileMode]::UserWrite -bor [IO.UnixFileMode]::UserExecute) }
    # Exercise the operator wrapper with a live server, without switching its DB.
    & ./scripts/maintenance.ps1 -Task backup -Bundle backups/first-install
    & ./scripts/maintenance.ps1 -Task verify -Bundle backups/first-install
    & ./scripts/maintenance.ps1 -Task restore -Bundle backups/first-install -Destination data/recovered.db
    if (!(Test-Path -LiteralPath (Join-Path $fixture "data/recovered.db") -PathType Leaf)) { throw "Maintenance wrapper did not create a recovered database" }
    $recoveredHash = (Get-FileHash -LiteralPath (Join-Path $fixture "data/recovered.db")).Hash
    $refused = $false
    try { & ./scripts/maintenance.ps1 -Task restore -Bundle backups/first-install -Destination data/recovered.db } catch {
      if ($_.Exception.Message -notmatch 'Legura maintenance restore failed') { throw }
      $refused = $true
    }
    if (!$refused -or (Get-FileHash -LiteralPath (Join-Path $fixture "data/recovered.db")).Hash -ne $recoveredHash) { throw "Restore overwrote an existing recovery database" }
    if ([IO.File]::ReadAllText($envPath) -cne $original) { throw "Maintenance changed application configuration" }
    Request -Path "/admin" -Status 200 | Out-Null
    Write-Host "Legura local bootstrap passed: prepare/start, safe config, password boundaries, native setup, live backup/verify, no-clobber recovery and unchanged running application."
  } finally { Pop-Location }
} catch {
  foreach ($log in @("server.out", "server.err")) {
    $path = Join-Path $fixture $log
    if (Test-Path -LiteralPath $path) { Get-Content -LiteralPath $path -Tail 20 | Write-Host }
  }
  throw
} finally {
  if ($process -and !$process.HasExited) {
    $process.Kill($true)
    $process.WaitForExit()
  }
  if ($client) { $client.Dispose() }
  if ($handler) { $handler.Dispose() }
  foreach ($key in $environment.Keys) { [Environment]::SetEnvironmentVariable($key, $environment[$key]) }
  $resolved = [IO.Path]::GetFullPath($fixture)
  if ($resolved.StartsWith($temporaryRoot, [StringComparison]::OrdinalIgnoreCase) -and (Split-Path $resolved -Leaf) -match '^legura-local-[0-9a-f]{32}$') {
    if (Test-Path -LiteralPath $resolved) { Remove-Item -LiteralPath $resolved -Recurse -Force }
  } else { throw "Refusing cleanup outside the isolated fixture" }
}
