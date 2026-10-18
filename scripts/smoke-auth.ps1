param(
  [string] $ToolManifest = "",
  [string] $ToolPath = "",
  [string] $RuntimeSource = "",
  [string] $UiSource = "",
  [string] $EditorSource = "",
  [int] $Port = 3941,
  [ValidateSet("http", "javascript", "native")][string] $Mode = "http",
  [switch] $Posts,
  [switch] $BackupRestore,
  [switch] $RefreshSchema,
  [switch] $Content,
  [switch] $ContentEditor,
  [switch] $PostEditor,
  [switch] $ActiveEditor,
  [switch] $NativeRetry,
  [switch] $Media,
  [switch] $MediaBrowser
)
$ErrorActionPreference = "Stop"
if ($MediaBrowser -and !$Media) { throw "MediaBrowser requires the isolated Media fixture" }
if ($Media -and ($Mode -ne "http" -or $Posts -or $Content -or $ContentEditor -or $PostEditor -or $ActiveEditor -or $RefreshSchema)) { throw "Media requires isolated HTTP mode without other content/browser switches" }
if ($ToolPath -and $ToolManifest) { throw "Choose ToolPath or ToolManifest, not both" }
if ($ActiveEditor) {
  $PostEditor = $true
  if ($RuntimeSource -or $EditorSource -or $UiSource) { throw "ActiveEditor must prove the pinned product dependencies without source overrides" }
}
if ($NativeRetry -and (!$PostEditor -or $Mode -ne "javascript")) { throw "NativeRetry requires PostEditor javascript mode" }
if ($NativeRetry -and ([bool]$RuntimeSource -ne [bool]$EditorSource)) { throw "NativeRetry source proof requires both RuntimeSource and EditorSource, or neither for registry acceptance" }
if ($BackupRestore -and (!$Posts -or $Mode -ne "http")) { throw "BackupRestore requires Posts HTTP mode" }
if ($PostEditor) { $ContentEditor = $true }
if ($PostEditor -and $Mode -eq "http") { throw "PostEditor requires javascript or native browser mode" }
if ($ContentEditor) { $Content = $true }
if ($UiSource -and !$ContentEditor) { throw "UiSource is only for the isolated ContentEditor proof" }
if ($EditorSource -and !$ContentEditor) { throw "EditorSource is only for the isolated ContentEditor proof" }
if ($Content -and (($Mode -ne "http" -and !$ContentEditor) -or $Posts -or $RefreshSchema)) { throw "Content requires HTTP or ContentEditor mode and no Posts/RefreshSchema" }
$root = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot ".."))
$fixture = Join-Path ([IO.Path]::GetTempPath()) ("legura-auth-" + [Guid]::NewGuid().ToString("N"))
$baseUrl = "http://127.0.0.1:$Port"
$python = Get-Command python -ErrorAction Stop
$process = $null
$client = $null
$handler = $null
$environment = @{}
$keys = @("AX_SECRET_DB_URL", "AX_SECRET_DB_DIALECT", "AX_SECRET_SESSION_KEY", "AX_SECRET_SETUP_TOKEN", "AX_SECRET_SESSION_COOKIE_SECURE", "AXONYX_HOST", "AXONYX_PORT")
$keys += @('AX_SECRET_MEDIA_DRIVER', 'AX_SECRET_MEDIA_ROOT', 'AX_SECRET_MEDIA_STORE_ID')
if ($Media) { $keys += "CARGO_TARGET_DIR" }
$keys = @($keys + @(Get-ChildItem Env: | Where-Object Name -Match '^(AX_SECRET_|AX_PUBLIC_|DB_|DATABASE_|DATA_|SESSION_|SETUP_TOKEN$)' | Select-Object -ExpandProperty Name) | Select-Object -Unique)
foreach ($key in $keys) { $environment[$key] = [Environment]::GetEnvironmentVariable($key) }
if ($ToolManifest) { $ToolManifest = [IO.Path]::GetFullPath($ToolManifest) }
if ($ToolPath) {
  $ToolPath = [IO.Path]::GetFullPath($ToolPath)
  if (!(Test-Path -LiteralPath $ToolPath -PathType Leaf)) { throw "ToolPath must be an existing CLI binary" }
}
if ($RuntimeSource) {
  $RuntimeSource = [IO.Path]::GetFullPath($RuntimeSource)
  if (!(Test-Path -LiteralPath (Join-Path $RuntimeSource "Cargo.toml"))) { throw "Runtime source manifest is missing" }
}
if ($UiSource) {
  $UiSource = [IO.Path]::GetFullPath($UiSource)
  if (!(Test-Path -LiteralPath (Join-Path $UiSource "src/foundry/icons/LinkIcon.asx"))) { throw "UI source is missing editor icons" }
}
if ($EditorSource) {
  $EditorSource = [IO.Path]::GetFullPath($EditorSource)
  if (!(Test-Path -LiteralPath (Join-Path $EditorSource "Cargo.toml"))) { throw "Editor source manifest is missing" }
}

function Invoke-Ax {
  param([string[]] $Arguments)
  if ($ToolPath) { & $ToolPath @Arguments }
  elseif ($ToolManifest) { & cargo run --manifest-path $ToolManifest -p cargo-axonyx --bin cargo-axonyx -- @Arguments }
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
  param([string]$WorkingDirectory = $fixture)
  $options = @{ FilePath = $script:binary; WorkingDirectory = $WorkingDirectory; PassThru = $true; RedirectStandardOutput = (Join-Path $fixture "server.out"); RedirectStandardError = (Join-Path $fixture "server.err") }
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
  if ($Media) {
    $mediaRoute = Join-Path $fixture "app/admin/media-proof"
    New-Item -ItemType Directory -Path $mediaRoute -Force | Out-Null
    foreach ($name in @("actions.ax", "loader.ax", "page.asx")) { Copy-Item -LiteralPath (Join-Path $root "tests/media-http/$name") -Destination $mediaRoute }
    # Raise only this fixture's transport budget; the product's 64 KiB setting stays.
    $configPath = Join-Path $fixture "Axonyx.toml"
    [IO.File]::WriteAllText($configPath, ([IO.File]::ReadAllText($configPath)).Replace('max_body_bytes = "64kb"', 'max_body_bytes = "9mb"'))
    $mediaRoot = Join-Path $fixture "private-media"
    New-Item -ItemType Directory -Path $mediaRoot | Out-Null
  }
  & ./scripts/editor-assets.ps1 -Task install
  if ($LASTEXITCODE -ne 0) { throw "Cannot prepare product editor assets" }
  if ($Content) {
    $proofRoute = Join-Path $fixture "app/admin/content-proof"
    New-Item -ItemType Directory -Path $proofRoute -Force | Out-Null
    Copy-Item -LiteralPath (Join-Path $root "tests/content-http/actions.ax") -Destination $proofRoute
    Copy-Item -LiteralPath (Join-Path $root "tests/content-http/page.asx") -Destination $proofRoute
    Copy-Item -LiteralPath (Join-Path $root "tests/content-http/loader.ax") -Destination $proofRoute
    if (!$ActiveEditor) { Copy-Item -LiteralPath (Join-Path $root "tests/content-http/native.rs") -Destination (Join-Path $fixture "src/native.rs") }
    New-Item -ItemType Directory -Path (Join-Path $fixture "routes/api") -Force | Out-Null
    Copy-Item -LiteralPath (Join-Path $root "tests/content-http/content-proof.ax") -Destination (Join-Path $fixture "routes/api/content-proof.ax")
  }
  if ($RuntimeSource) {
    # Source overrides are confined to the disposable fixture, never the app manifest.
    $path = $RuntimeSource.Replace('\', '/') | ConvertTo-Json -Compress
    [IO.File]::AppendAllText((Join-Path $fixture "Cargo.toml"), "`n[patch.crates-io]`naxonyx-runtime = { path = $path }`n")
    & cargo update -p axonyx-runtime
    if ($LASTEXITCODE -ne 0) { throw "Cannot activate fixture runtime source override" }
  }
  if ($UiSource) {
    if (!$RuntimeSource) {
      [IO.File]::AppendAllText((Join-Path $fixture "Cargo.toml"), "`n[patch.crates-io]`n")
    }
    $path = $UiSource.Replace('\', '/') | ConvertTo-Json -Compress
    [IO.File]::AppendAllText((Join-Path $fixture "Cargo.toml"), "axonyx-ui = { path = $path }`n")
    & cargo update -p axonyx-ui
    if ($LASTEXITCODE -ne 0) { throw "Cannot activate isolated UI source override" }
  }
  if ($EditorSource) {
    $path = $EditorSource.Replace('\', '/') | ConvertTo-Json -Compress
    [IO.File]::AppendAllText((Join-Path $fixture "Cargo.toml"), "`n[patch.`"https://github.com/vladanPro/legura-editor`"]`nlegura-editor = { path = $path }`n")
    & cargo update -p legura-editor
    if ($LASTEXITCODE -ne 0) { throw "Cannot activate isolated editor source override" }
  }
  if ($ContentEditor) {
    $editorInstall = & cargo run --locked --bin legura-editor-assets -- install $fixture --json | ConvertFrom-Json
    if ($LASTEXITCODE -ne 0 -or !$editorInstall.installed) { throw "Cannot install pinned editor generation" }
    $editorAssets = Join-Path $fixture "public/content-editor"
    $hostModule = [IO.File]::ReadAllText((Join-Path $root "tests/content-editor/host.js"))
    $hostModule = $hostModule.Replace("'./rich-text-editor.js'", "'$($editorInstall.module)'")
    if (!$ActiveEditor) { [IO.File]::WriteAllText((Join-Path $editorAssets "host.js"), $hostModule) }
    Copy-Item -Path (Join-Path $root "tests/content-editor/app/*") -Destination (Join-Path $fixture "app/admin") -Recurse
    if ($PostEditor -and !$ActiveEditor) {
      Copy-Item -LiteralPath (Join-Path $root "tests/post-editor/PostFields.asx") -Destination (Join-Path $fixture "app/components/PostFields.asx")
      foreach ($file in @("page.asx", "actions.ax")) {
        Copy-Item -LiteralPath (Join-Path $root "tests/post-editor/new/$file") -Destination (Join-Path $fixture "app/admin/posts/new/$file")
      }
      Copy-Item -LiteralPath (Join-Path $root "tests/post-editor/new/loader.ax") -Destination (Join-Path $fixture "app/admin/posts/new/editor-loader.ax")
      foreach ($file in @("page.asx", "actions.ax")) {
        Copy-Item -LiteralPath (Join-Path $root "tests/post-editor/edit/$file") -Destination (Join-Path $fixture "app/admin/posts/[id]/edit/$file")
      }
      Copy-Item -LiteralPath (Join-Path $root "tests/post-editor/edit/loader.ax") -Destination (Join-Path $fixture "app/admin/posts/[id]/edit/loader.ax")
    }
    $editorPages = @("app/admin/content-editor/page.asx", "app/admin/content-editor/edit/page.asx")
    if ($PostEditor -and !$ActiveEditor) { $editorPages += @("app/components/PostFields.asx", "app/admin/posts/new/page.asx", "app/admin/posts/[id]/edit/page.asx") }
    foreach ($relative in $editorPages) {
      $pagePath = Join-Path $fixture $relative
      $pageSource = [IO.File]::ReadAllText($pagePath)
      $componentImport = "@/" + $editorInstall.component.Substring(4).Replace(".asx", "")
      $pageSource = $pageSource.Replace("@/components/RichTextEditor", $componentImport).Replace("/content-editor/editor.css", $editorInstall.stylesheet)
      [IO.File]::WriteAllText($pagePath, $pageSource)
    }
    & cargo run --locked --bin legura-editor-assets -- check $fixture
    if ($LASTEXITCODE -ne 0) { throw "Installed editor generation verification failed" }
  }
  foreach ($key in $keys) { Remove-Item -LiteralPath "Env:$key" -ErrorAction SilentlyContinue }
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
  if ($Media) {
    $env:AX_SECRET_MEDIA_DRIVER = 'local'
    $env:AX_SECRET_MEDIA_ROOT = $mediaRoot
    $env:AX_SECRET_MEDIA_STORE_ID = 'local-media'
    $env:CARGO_TARGET_DIR = Join-Path $fixture 'target'
  }
  Invoke-Ax -Arguments @("db", "migrate")
  Invoke-Ax -Arguments @("db", "pull")
  if ($RefreshSchema) {
    # Generate a portable contract from the isolated migrated database, not user data.
    $databaseUrl = $env:AX_SECRET_DB_URL
    try {
      $env:AX_SECRET_DB_URL = "sqlite://data/legura.db"
      Invoke-Ax -Arguments @("db", "pull")
      Copy-Item -LiteralPath (Join-Path $fixture ".axonyx/db/schema.json") -Destination (Join-Path $root ".axonyx/db/schema.json")
      Copy-Item -LiteralPath (Join-Path $fixture "app/generated/db.ax") -Destination (Join-Path $root "app/generated/db.ax")
    } finally { $env:AX_SECRET_DB_URL = $databaseUrl }
  }
  Invoke-Ax -Arguments @("check")
  Invoke-Ax -Arguments @("build", "--clean", "--compiled")
  $metadata = & cargo metadata --format-version 1 --no-deps | ConvertFrom-Json
  if ($LASTEXITCODE -ne 0) { throw "Cannot find compiled server" }
  $binary = Join-Path $metadata.target_directory "release/axonyx-production"
  if ($IsWindows) { $binary += ".exe" }
  if ($BackupRestore) {
    & cargo build --locked --release --bin legura-maintenance
    if ($LASTEXITCODE -ne 0) { throw "Cannot build fixture maintenance tool" }
    $maintenance = Join-Path $metadata.target_directory "release/legura-maintenance"
    if ($IsWindows) { $maintenance += ".exe" }
  }
  $handler = [Net.Http.HttpClientHandler]::new()
  $handler.AllowAutoRedirect = $false
  $handler.UseCookies = $false
  $client = [Net.Http.HttpClient]::new($handler)
  $client.Timeout = [TimeSpan]::FromSeconds(20)
  Start-Fixture
  if ($Mode -ne "http") {
    $browserTest = if ($PostEditor) { "tests/post-editor/browser.mjs" } elseif ($ContentEditor) { "tests/content-editor/browser.mjs" } elseif ($Posts) { "tests/posts-browser.mjs" } else { "tests/auth-browser.mjs" }
    $retryMode = if ($NativeRetry) { "native-retry" } else { "standard" }
    $editorMode = if ($ActiveEditor) { 'active' } else { 'fixture' }
    & node (Join-Path $root $browserTest) $baseUrl $Mode $db $retryMode $editorMode
    if ($LASTEXITCODE -ne 0) { throw "Browser auth acceptance failed ($Mode)" }
    return
  }
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
  if ($Content) { . (Join-Path $root "tests/content-http/proof.ps1") }
  if ($Media) { . (Join-Path $root "tests/media-http/proof.ps1") }
  if ($Posts) { . (Join-Path $root "tests/posts-http.ps1") }
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
  $revokedProof = Get-Proof -Cookie $cookie
  & $python.Source -c 'import sqlite3,sys;d=sqlite3.connect(sys.argv[1]);d.execute("delete from legura_credentials");d.execute("delete from legura_installation");d.execute("delete from legura_users");d.commit();d.close()' $db
  if ($LASTEXITCODE -ne 0) { throw "Cannot revoke fixture identity" }
  Invoke-Request -Path "/admin" -Headers @{ Cookie = $cookie } -Status 403 | Out-Null
  if ($Content) {
    Invoke-Request -Path $contentAction -Method POST -Body $contentForm -Headers $revokedProof -Status 403 | Out-Null
    Assert-ContentCount -Expected 2
  }
  if ($Media) {
    Send-Media -Bytes $validImage -Headers $revokedProof -Status 403 | Out-Null
    Invoke-Request -Path "/admin/media-proof" -Headers @{ Cookie = $cookie } -Status 403 | Out-Null
    Send-Media -Bytes $validImage -Headers $revokedProof -Status 403 -Native | Out-Null
    Assert-MediaCounts -Rows $mediaExpectedRows -Files $mediaExpectedFiles
  }
  if ($Posts) {
    Invoke-Request -Path "/admin/posts" -Headers @{ Cookie = $cookie } -Status 403 | Out-Null
    Invoke-Request -Path "/__axonyx/action?path=%2Fadmin%2Fposts%2Fnew&name=CreatePost" -Method POST -Body "title=Revoked&slug=revoked&body=Denied&status=draft" -Headers $revokedProof -Status 403 | Out-Null
    Invoke-Request -Path "/admin/posts/$revocablePostId/delete" -Headers @{ Cookie = $cookie } -Status 403 | Out-Null
    Invoke-Request -Path "/__axonyx/action?path=%2Fadmin%2Fposts%2F$revocablePostId%2Fdelete&name=DeletePost" -Method POST -Body "id=$revocablePostId&confirmSlug=public-story" -Headers $revokedProof -Status 403 | Out-Null
    $count = & $python.Source -c 'import sqlite3,sys;d=sqlite3.connect(sys.argv[1]);print(d.execute("select count(*) from legura_posts").fetchone()[0]);d.close()' $db
    if ($LASTEXITCODE -ne 0 -or $count -ne "1") { throw "Revoked action changed the remaining post" }
  }
  Write-Host "Legura compiled auth smoke passed: setup race, CSRF, password hash, login/logout, restart persistence, authorization."
} catch {
  foreach ($log in @("server.out", "server.err")) {
    $path = Join-Path $fixture $log
    if (Test-Path -LiteralPath $path) { Get-Content -LiteralPath $path -Tail 40 | Write-Host }
  }
  throw
} finally {
  Stop-Fixture
  if ($client) { $client.Dispose() }
  if ($handler) { $handler.Dispose() }
  foreach ($key in $keys) {
    if ($null -eq $environment[$key]) { Remove-Item -LiteralPath "Env:$key" -ErrorAction SilentlyContinue }
    else { [Environment]::SetEnvironmentVariable($key, $environment[$key]) }
  }
  Pop-Location
  $resolved = [IO.Path]::GetFullPath($fixture)
  $temp = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd([IO.Path]::DirectorySeparatorChar) + [IO.Path]::DirectorySeparatorChar
  if (!$resolved.StartsWith($temp, [StringComparison]::OrdinalIgnoreCase) -or (Split-Path $resolved -Leaf) -notmatch '^legura-auth-[a-f0-9]{32}$') { throw "Refusing cleanup outside generated auth fixture" }
  if (Test-Path -LiteralPath $resolved) { Remove-Item -LiteralPath $resolved -Recurse -Force }
}
