param(
  [string] $ToolManifest = "",
  [int] $Port = 3941
)
$ErrorActionPreference = "Stop"
$root = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot ".."))
$fixture = Join-Path ([IO.Path]::GetTempPath()) ("legura-auth-" + [Guid]::NewGuid().ToString("N"))
$baseUrl = "http://127.0.0.1:$Port"
$python = Get-Command python -ErrorAction Stop
$process = $null
$client = $null
$handler = $null
$environment = @{}
$keys = @("AX_SECRET_DB_URL", "AX_SECRET_DB_DIALECT", "AX_SECRET_SESSION_KEY", "AX_SECRET_SETUP_TOKEN", "AX_SECRET_SESSION_COOKIE_SECURE", "AXONYX_HOST", "AXONYX_PORT")
$keys = @($keys + @(Get-ChildItem Env: | Where-Object Name -Match '^(AX_SECRET_|AX_PUBLIC_|DB_|DATABASE_|DATA_|SESSION_|SETUP_TOKEN$)' | Select-Object -ExpandProperty Name) | Select-Object -Unique)
foreach ($key in $keys) { $environment[$key] = [Environment]::GetEnvironmentVariable($key) }
if ($ToolManifest) { $ToolManifest = [IO.Path]::GetFullPath($ToolManifest) }

function Invoke-Ax {
  param([string[]] $Arguments)
  if ($ToolManifest) { & cargo run --manifest-path $ToolManifest -p cargo-axonyx --bin cargo-axonyx -- @Arguments }
  else { & cargo ax @Arguments }
  if ($LASTEXITCODE -ne 0) { throw "Fixture Axonyx command failed" }
}

function New-Request {
  param([string] $Path, [string] $Method = "GET", [string] $Body = "", [hashtable] $Headers = @{})
  $request = [Net.Http.HttpRequestMessage]::new([Net.Http.HttpMethod]::new($Method), "$baseUrl$Path")
  if ($Method -eq "POST") { $request.Content = [Net.Http.StringContent]::new($Body, [Text.Encoding]::UTF8, "application/x-www-form-urlencoded") }
  foreach ($key in $Headers.Keys) { $request.Headers.Add($key, [string]$Headers[$key]) }
  return $request
}

function Read-Response {
  param($Response)
  try {
    $headers = @{}
    foreach ($header in $Response.Headers) { $headers[$header.Key] = $header.Value -join ', ' }
    return @{ Status = [int]$Response.StatusCode; Body = $Response.Content.ReadAsStringAsync().GetAwaiter().GetResult(); Headers = $headers }
  } finally { $Response.Dispose() }
}

function Invoke-Request {
  param([string] $Path, [string] $Method = "GET", [string] $Body = "", [hashtable] $Headers = @{}, [int] $Status = 200)
  $request = New-Request -Path $Path -Method $Method -Body $Body -Headers $Headers
  try { $response = Read-Response ($client.SendAsync($request).GetAwaiter().GetResult()) }
  finally { $request.Dispose() }
  if ($response.Status -ne $Status) { throw "Expected HTTP $Status for $Path, got $($response.Status)" }
  return $response
}

function Get-Proof {
  param([string] $Cookie = "")
  $headers = @{ Origin = $baseUrl }
  if ($Cookie) { $headers.Cookie = $Cookie }
  $response = Invoke-Request -Path "/__axonyx/csrf" -Headers $headers
  $token = ($response.Body | ConvertFrom-Json).token
  if (!$Cookie) { $Cookie = ([string]$response.Headers["Set-Cookie"]).Split(';')[0] }
  return @{ Origin = $baseUrl; Cookie = $Cookie; "X-Axonyx-CSRF" = $token; Accept = "text/html" }
}

function Start-Fixture {
  $options = @{ FilePath = $script:binary; WorkingDirectory = $fixture; PassThru = $true; RedirectStandardOutput = (Join-Path $fixture "server.out"); RedirectStandardError = (Join-Path $fixture "server.err") }
  if ($IsWindows) { $options.WindowStyle = "Hidden" }
  $script:process = Start-Process @options
  for ($attempt = 0; $attempt -lt 60; $attempt++) {
    if ($process.HasExited) { throw "Fixture server exited before readiness" }
    try { Invoke-Request -Path "/login" | Out-Null; return } catch { Start-Sleep -Milliseconds 250 }
  }
  throw "Fixture server did not become ready"
}

function Stop-Fixture {
  if ($null -ne $script:process -and !$script:process.HasExited) {
    Stop-Process -Id $script:process.Id
    $script:process.WaitForExit(5000) | Out-Null
  }
  $script:process = $null
}

New-Item -ItemType Directory -Path $fixture | Out-Null
Push-Location $root
try {
  $reservation = [Net.Sockets.TcpListener]::new([Net.IPAddress]::Loopback, $Port)
  try { $reservation.Start() } finally { $reservation.Stop() }
  # Copy tracked source only: never copy the developer's .env or database.
  $files = & git ls-files
  if ($LASTEXITCODE -ne 0) { throw "Cannot enumerate fixture sources" }
  foreach ($file in $files) {
    if ($file -match '^(\.env$|data/|dist/|target/|src/generated/)') { continue }
    $target = Join-Path $fixture $file
    New-Item -ItemType Directory -Path (Split-Path $target) -Force | Out-Null
    Copy-Item -LiteralPath (Join-Path $root $file) -Destination $target
  }
  Set-Location $fixture
  foreach ($key in $keys) { [Environment]::SetEnvironmentVariable($key, $null) }
  New-Item -ItemType Directory -Path "data" | Out-Null
  $db = Join-Path $fixture "data/legura.db"
  $setup = [Convert]::ToHexString([Security.Cryptography.RandomNumberGenerator]::GetBytes(32))
  $password = "fixture-password-" + [Guid]::NewGuid().ToString("N")
  $env:AX_SECRET_DB_URL = "sqlite://" + $db.Replace('\', '/')
  $env:AX_SECRET_DB_DIALECT = "sqlite"
  $env:AX_SECRET_SESSION_KEY = [Convert]::ToHexString([Security.Cryptography.RandomNumberGenerator]::GetBytes(32))
  $env:AX_SECRET_SETUP_TOKEN = $setup
  $env:AX_SECRET_SESSION_COOKIE_SECURE = "false"
  $env:AXONYX_HOST = "127.0.0.1"
  $env:AXONYX_PORT = "$Port"
  Invoke-Ax -Arguments @("db", "migrate")
  Invoke-Ax -Arguments @("check")
  Invoke-Ax -Arguments @("build", "--clean", "--compiled")
  $metadata = & cargo metadata --format-version 1 --no-deps | ConvertFrom-Json
  if ($LASTEXITCODE -ne 0) { throw "Cannot find compiled server" }
  $binary = Join-Path $metadata.target_directory "release/axonyx-production"
  if ($IsWindows) { $binary += ".exe" }
  $handler = [Net.Http.HttpClientHandler]::new()
  $handler.AllowAutoRedirect = $false
  $handler.UseCookies = $false
  $client = [Net.Http.HttpClient]::new($handler)
  $client.Timeout = [TimeSpan]::FromSeconds(20)
  Start-Fixture
  Invoke-Request -Path "/admin" -Status 403 | Out-Null
  $page = Invoke-Request -Path "/setup"
  if ($page.Body.Contains($setup)) { throw "Setup page exposed the owner token" }
  $proof = Get-Proof
  $installPath = "/__axonyx/action?path=%2Fsetup&name=Install"
  $body = "siteName=Fixture&email=owner%40example.com&password=$password&setupToken=$setup"
  Invoke-Request -Path $installPath -Method POST -Body $body -Headers @{ Origin = $baseUrl; Accept = "text/html" } -Status 403 | Out-Null
  Invoke-Request -Path $installPath -Method POST -Body $body.Replace($setup, "wrong") -Headers $proof -Status 403 | Out-Null
  $invalid = Invoke-Request -Path $installPath -Method POST -Body $body.Replace("siteName=Fixture", "siteName=Retained+site").Replace("owner%40example.com", "not-an-email") -Headers $proof -Status 422
  if ($invalid.Body -notmatch 'value="Retained site"' -or $invalid.Body.Contains($password) -or $invalid.Body.Contains($setup)) { throw "Native validation did not retain public fields safely" }
  $requests = @()
  try {
    $tasks = foreach ($email in @("owner", "racer")) {
      $request = New-Request -Path $installPath -Method POST -Body $body.Replace("owner%40", "$email%40") -Headers $proof
      $requests += $request
      $client.SendAsync($request)
    }
    $results = @($tasks | ForEach-Object { Read-Response ($_.GetAwaiter().GetResult()) })
  } finally { foreach ($request in $requests) { $request.Dispose() } }
  if (@($results | Where-Object Status -eq 303).Count -ne 1 -or @($results | Where-Object { $_.Status -in @(403,409) }).Count -ne 1) {
    throw "Setup race did not elect exactly one owner (HTTP $($results.Status -join ', '))"
  }
  $loser = $results | Where-Object Status -ne 303
  if ($loser.Headers["Set-Cookie"] -or $loser.Headers["Cache-Control"] -ne "no-store" -or $loser.Body -match 'unique_violation|legura_installation|argon2|INSERT') { throw "Losing setup response exposed internals or issued a session" }
  $winner = $results | Where-Object Status -eq 303
  $cookie = ([string]$winner.Headers["Set-Cookie"]).Split(';')[0]
  if ($winner.Headers["Set-Cookie"] -notmatch "HttpOnly") { throw "Session cookie is not private" }
  $inspection = & $python.Source -c 'import sqlite3,sys,json;d=sqlite3.connect(sys.argv[1]);row=d.execute("select email,password_hash from legura_credentials").fetchone();print(json.dumps({"counts":[d.execute("select count(*) from "+t).fetchone()[0] for t in ["legura_users","legura_credentials","legura_installation"]],"email":row[0],"hashed":row[1].startswith("$argon2id$"),"plaintext":row[1]==sys.argv[2]}));d.close()' $db $password
  if ($LASTEXITCODE -ne 0) { throw "Cannot inspect fixture identity" }
  $identity = $inspection | ConvertFrom-Json
  if (($identity.counts -join ',') -ne '1,1,1' -or !$identity.hashed -or $identity.plaintext) { throw "Setup persistence/rollback/password policy failed" }
  Invoke-Request -Path $installPath -Method POST -Body $body -Headers $proof -Status 403 | Out-Null
  Invoke-Request -Path "/setup" -Status 403 | Out-Null
  $admin = Invoke-Request -Path "/admin" -Headers @{ Cookie = $cookie }
  if (!$admin.Body.Contains($identity.email) -or $admin.Body.Contains($password)) { throw "Protected admin render failed" }
  Stop-Fixture
  Start-Fixture
  Invoke-Request -Path "/admin" -Headers @{ Cookie = $cookie } | Out-Null
  $sessionProof = Get-Proof -Cookie $cookie
  $logoutPath = "/__axonyx/action?path=%2Fadmin&name=SignOut"
  Invoke-Request -Path $logoutPath -Method POST -Headers @{ Cookie = $cookie; Origin = $baseUrl } -Status 403 | Out-Null
  Invoke-Request -Path "/admin" -Headers @{ Cookie = $cookie } | Out-Null
  Invoke-Request -Path $logoutPath -Method POST -Headers $sessionProof -Status 303 | Out-Null
  Invoke-Request -Path "/admin" -Headers @{ Cookie = $cookie } -Status 403 | Out-Null
  $proof = Get-Proof
  $loginPath = "/__axonyx/action?path=%2Flogin&name=SignIn"
  $email = [Uri]::EscapeDataString($identity.email)
  $wrong = Invoke-Request -Path $loginPath -Method POST -Body "email=$email&password=wrong-password" -Headers $proof -Status 422
  $missing = Invoke-Request -Path $loginPath -Method POST -Body "email=missing%40example.com&password=wrong-password" -Headers $proof -Status 422
  foreach ($response in @($wrong, $missing)) {
    if ($response.Body -notmatch "Email or password is incorrect" -or $response.Headers["Set-Cookie"]) { throw "Login exposed account existence or created an invalid session" }
  }
  $login = Invoke-Request -Path $loginPath -Method POST -Body "email=$email&password=$password" -Headers $proof -Status 303
  $cookie = ([string]$login.Headers["Set-Cookie"]).Split(';')[0]
  Invoke-Request -Path "/admin" -Headers @{ Cookie = $cookie } | Out-Null
  # The owner's wrong and successful logins consumed two slots; unknown email has its own key.
  foreach ($attempt in 1..3) {
    Invoke-Request -Path $loginPath -Method POST -Body "email=$email&password=wrong-password" -Headers $proof -Status 422 | Out-Null
  }
  $limited = Invoke-Request -Path $loginPath -Method POST -Body "email=$email&password=$password" -Headers $proof -Status 429
  if ([int]$limited.Headers["Retry-After"] -lt 1 -or $limited.Headers["Cache-Control"] -ne "no-store" -or $limited.Headers["Set-Cookie"]) { throw "Login throttle failed its admission boundary" }
  & $python.Source -c 'import sqlite3,sys;d=sqlite3.connect(sys.argv[1]);d.execute("delete from legura_credentials");d.execute("delete from legura_installation");d.execute("delete from legura_users");d.commit();d.close()' $db
  if ($LASTEXITCODE -ne 0) { throw "Cannot revoke fixture identity" }
  Invoke-Request -Path "/admin" -Headers @{ Cookie = $cookie } -Status 403 | Out-Null
  Write-Host "Legura compiled auth smoke passed: setup race, CSRF, password hash, login/logout, restart persistence, authorization."
} finally {
  Stop-Fixture
  if ($client) { $client.Dispose() }
  if ($handler) { $handler.Dispose() }
  foreach ($key in $keys) { [Environment]::SetEnvironmentVariable($key, $environment[$key]) }
  Pop-Location
  $resolved = [IO.Path]::GetFullPath($fixture)
  $temp = [IO.Path]::GetFullPath([IO.Path]::GetTempPath())
  if ($resolved.StartsWith($temp) -and (Split-Path $resolved -Leaf) -like 'legura-auth-*') { Remove-Item -LiteralPath $resolved -Recurse -Force }
}
