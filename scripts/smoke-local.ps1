param([ValidateRange(1024, 65535)][int] $Port = 3943, [switch] $Package)
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

function Assert-PackageServiceGuide {
  param([string] $PackageRoot)
  $readme = Join-Path $PackageRoot 'README.md'
  $guide = Join-Path $PackageRoot 'native-service-v0.md'
  if (!(Test-Path -LiteralPath $guide -PathType Leaf) -or
      !([IO.File]::ReadAllText($readme).Contains('(native-service-v0.md)'))) {
    throw 'Packaged README has a missing native service guide'
  }
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
    if ($Package) {
      $proof = (Request -Path '/__axonyx/csrf' -Status 200 -Headers @{ Origin = $baseUrl } | ConvertFrom-Json).token
      $headers = @{ Origin = $baseUrl; Accept = 'text/html'; 'X-Axonyx-CSRF' = $proof }
      foreach ($status in @('published', 'draft')) {
        Request -Path '/__axonyx/action?path=%2Fadmin%2Fposts%2Fnew&name=CreatePost' -Status 303 -Body "title=Package+$status&slug=package-$status&body=Package+content+$status&status=$status" -Headers $headers | Out-Null
      }
    }
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
    if ($Package) {
      $process.Kill($true)
      $process.WaitForExit()
      $process.Dispose()
      $process = $null
      & ./scripts/package.ps1 -OutputDirectory native-package
      $packageRoot = Join-Path $fixture "native-package"
      $manifest = Get-Content -LiteralPath (Join-Path $packageRoot "package.json") -Raw | ConvertFrom-Json
      if ($manifest.format -ne 1 -or $manifest.product -ne 'legura') { throw "Package manifest is invalid" }
      $listed = @($manifest.files | ForEach-Object path | Sort-Object)
      $actual = @(Get-ChildItem -LiteralPath $packageRoot -Recurse -File -Force | ForEach-Object { [IO.Path]::GetRelativePath($packageRoot, $_.FullName).Replace('\', '/') } | Where-Object { $_ -ne 'package.json' } | Sort-Object)
      if (($listed -join "`n") -cne ($actual -join "`n")) { throw "Package inventory is incomplete" }
      foreach ($entry in $manifest.files) {
        if ((Get-FileHash -LiteralPath (Join-Path $packageRoot $entry.path)).Hash.ToLowerInvariant() -cne $entry.sha256) { throw "Package hash mismatch" }
        if ($entry.path -match '(^|/)(\.env[^/]*|Cargo\.toml|Cargo\.lock|package-lock\.json)$|^(app|src|data|backups|db)/|\.sqlite$|\.db$|dist/_ax/melt/') { throw "Package contains private/source artifacts" }
      }
      $extension = if ($IsWindows) { '.exe' } else { '' }
      $maintenance = Join-Path $packageRoot "legura-maintenance$extension"
      & $maintenance verify-package $packageRoot
      if ($LASTEXITCODE -ne 0) { throw "Native package integrity check failed" }
      Assert-PackageServiceGuide -PackageRoot $packageRoot
      $artifactRoot = Join-Path $fixture 'artifacts'
      New-Item -ItemType Directory -Path $artifactRoot | Out-Null
      & ./scripts/archive-package.ps1 -PackageDirectory $packageRoot -OutputDirectory $artifactRoot
      $archives = @(Get-ChildItem -LiteralPath $artifactRoot -File -Filter 'legura-*.tar.gz')
      if ($archives.Count -ne 1) { throw 'Expected exactly one candidate archive' }
      $archive = $archives[0]
      $checksum = [IO.File]::ReadAllText("$($archive.FullName).sha256").TrimEnd()
      $archiveHash = (Get-FileHash -LiteralPath $archive.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
      if ($checksum -cne "$archiveHash  $($archive.Name)") { throw 'Candidate archive checksum mismatch' }
      $packageRoot = Join-Path $fixture 'extracted-package'
      New-Item -ItemType Directory -Path $packageRoot | Out-Null
      & tar -xzf $archive.FullName -C $packageRoot
      if ($LASTEXITCODE -ne 0) { throw 'Candidate archive extraction failed' }
      $maintenance = Join-Path $packageRoot "legura-maintenance$extension"
      & $maintenance verify-package $packageRoot
      if ($LASTEXITCODE -ne 0) { throw 'Extracted package integrity check failed' }
      Assert-PackageServiceGuide -PackageRoot $packageRoot
      $packageData = Join-Path $packageRoot "data"
      New-Item -ItemType Directory -Path $packageData | Out-Null
      if (!$IsWindows) { [IO.File]::SetUnixFileMode($packageData, [IO.UnixFileMode]::UserRead -bor [IO.UnixFileMode]::UserWrite -bor [IO.UnixFileMode]::UserExecute) }
      & $maintenance restore (Join-Path $fixture 'backups/first-install') (Join-Path $packageData 'legura.db')
      if ($LASTEXITCODE -ne 0) { throw "Packaged maintenance tool failed" }
      $newKey = [Convert]::ToHexString([Security.Cryptography.RandomNumberGenerator]::GetBytes(32))
      $packageConfig = Join-Path $packageRoot '.env'
      [IO.File]::WriteAllText($packageConfig, $original.Replace($config.AX_SECRET_SESSION_KEY, $newKey))
      if (!$IsWindows) { [IO.File]::SetUnixFileMode($packageConfig, [IO.UnixFileMode]::UserRead -bor [IO.UnixFileMode]::UserWrite) }
      $launcher = (Get-Command pwsh -ErrorAction Stop).Source
      $options.ArgumentList = @('-NoProfile', '-File', (Join-Path $packageRoot 'start.ps1'), '-Port', "$Port")
      $options.WorkingDirectory = $temporaryRoot
      $pathBefore = $env:PATH
      try {
        $env:PATH = Join-Path $fixture 'no-toolchain-path'
        if (Get-Command cargo,git,node,cargo-ax -ErrorAction SilentlyContinue) { throw "Runtime test still exposes developer tools" }
        $options.FilePath = $launcher
        $process = Start-Process @options
      } finally { $env:PATH = $pathBefore }
      $ready = $false
      for ($attempt = 0; $attempt -lt 120; $attempt++) {
        if ($process.HasExited) { throw "Packaged server exited before readiness" }
        try { Request -Path '/__axonyx/ready' -Status 200 | Out-Null; $ready = $true; break } catch { Start-Sleep -Milliseconds 250 }
      }
      if (!$ready) { throw "Packaged server was not ready" }
      Request -Path '/admin' -Status 403 | Out-Null
      Request -Path '/__axonyx/data?path=%2Fadmin&name=overview' -Status 403 | Out-Null
      foreach ($privatePath in @('/.env', '/package.json', '/Cargo.toml', '/src/generated/backend.rs')) { Request -Path $privatePath -Status 404 | Out-Null }
      $story = Request -Path '/posts/package-published' -Status 200
      if (!$story.Contains('Package content published')) { throw "Packaged public content was not recovered" }
      Request -Path '/posts/package-draft' -Status 404 | Out-Null
      $login = Request -Path '/login' -Status 200
      if (!$login.Contains('Password')) { throw "Packaged login page failed" }
      $proof = (Request -Path '/__axonyx/csrf' -Status 200 -Headers @{ Origin = $baseUrl } | ConvertFrom-Json).token
      Request -Path '/__axonyx/action?path=%2Flogin&name=SignIn' -Status 303 -Body "email=bootstrap%40example.com&password=$password" -Headers @{ Origin = $baseUrl; Accept = 'text/html'; 'X-Axonyx-CSRF' = $proof } | Out-Null
      $admin = Request -Path '/admin' -Status 200
      if (!$admin.Contains('bootstrap@example.com')) { throw "Packaged login/admin identity failed" }
      $posts = Request -Path '/admin/posts' -Status 200
      if (!$posts.Contains('Package published') -or !$posts.Contains('Package draft')) { throw "Packaged private content was not recovered" }
      $editor = Request -Path '/admin/posts/new' -Status 200
      if (!$editor.Contains('data-legura-editor="richtext"')) { throw "Packaged rich form is missing" }
      $plainEditor = Request -Path '/admin/posts/new/plain' -Status 200
      if (!$plainEditor.Contains('name="body"') -or $plainEditor.Contains('data-legura-editor="richtext"')) { throw "Packaged plain form is invalid" }
      $editorFiles = @($manifest.files | Where-Object { $_.path -like 'dist/content-editor/*' })
      if ($editorFiles.Count -lt 5) { throw "Packaged editor assets are incomplete" }
      foreach ($entry in $editorFiles) { Request -Path $entry.path.Substring(4) -Status 200 | Out-Null }
      Request -Path '/setup' -Status 403 | Out-Null
      $asset = [regex]::Match($login, 'href="([^"]+\.css)"').Groups[1].Value
      if (!$asset.StartsWith('/')) { throw "Packaged CSS reference missing" }
      Request -Path $asset -Status 200 | Out-Null
      if ([IO.File]::ReadAllText($packageConfig) -cne $original.Replace($config.AX_SECRET_SESSION_KEY, $newKey)) { throw "Packaged startup modified secrets" }
      Write-Host "Native archive passed: checksum, extracted package integrity, no source/secrets, relocated launcher, no developer tools, restored login/admin, private setup and CSS assets."
      $process.Kill($true)
      $process.WaitForExit()
      $process.Dispose()
      $process = $null
      $freshRoot = Join-Path $fixture 'fresh-native-package'
      New-Item -ItemType Directory -Path $freshRoot | Out-Null
      foreach ($entry in Get-ChildItem -LiteralPath $packageRoot) {
        if ($entry.Name -notin @('data', '.env')) { Copy-Item -LiteralPath $entry.FullName -Destination $freshRoot -Recurse }
      }
      $freshData = Join-Path $freshRoot 'data'
      New-Item -ItemType Directory -Path $freshData | Out-Null
      if (!$IsWindows) { [IO.File]::SetUnixFileMode($freshData, [IO.UnixFileMode]::UserRead -bor [IO.UnixFileMode]::UserWrite -bor [IO.UnixFileMode]::UserExecute) }
      $freshPath = Join-Path $freshData 'legura.db'
      $freshMaintenance = Join-Path $freshRoot "legura-maintenance$extension"
      $packageConfig = Join-Path $freshRoot '.env'
      $options.ArgumentList = @('-NoProfile', '-File', (Join-Path $freshRoot 'start.ps1'), '-Port', "$Port")
      $pathBefore = $env:PATH
      try {
        $env:PATH = Join-Path $fixture 'no-toolchain-path'
        & $freshMaintenance init $freshPath
        if ($LASTEXITCODE -ne 0) { throw "Native empty database initialization failed" }
        $freshHash = (Get-FileHash -LiteralPath $freshPath).Hash
        & $freshMaintenance init $freshPath
        if ($LASTEXITCODE -eq 0 -or (Get-FileHash -LiteralPath $freshPath).Hash -ne $freshHash) { throw "Native initialization overwrote an existing database" }
        $configOutput = & $freshMaintenance config-local $freshRoot
        if ($LASTEXITCODE -ne 0) { throw "Native configuration failed" }
        $freshConfig = [IO.File]::ReadAllText($packageConfig)
        $freshKey = [regex]::Match($freshConfig, '(?m)^AX_SECRET_SESSION_KEY=([a-f0-9]{64})$').Groups[1].Value
        $freshToken = [regex]::Match($freshConfig, '(?m)^AX_SECRET_SETUP_TOKEN=([a-f0-9]{64})$').Groups[1].Value
        if (!$freshKey -or !$freshToken -or $freshKey -eq $freshToken -or "$configOutput".Contains($freshKey) -or "$configOutput".Contains($freshToken)) { throw "Native config did not create independent private secrets" }
        & $freshMaintenance config-local $freshRoot
        if ($LASTEXITCODE -eq 0 -or [IO.File]::ReadAllText($packageConfig) -cne $freshConfig) { throw "Native config overwrote existing secrets" }
        $process = Start-Process @options
      } finally { $env:PATH = $pathBefore }
      $ready = $false
      for ($attempt = 0; $attempt -lt 120; $attempt++) {
        if ($process.HasExited) { throw "Fresh packaged server exited before readiness" }
        try { Request -Path '/__axonyx/ready' -Status 200 | Out-Null; $ready = $true; break } catch { Start-Sleep -Milliseconds 250 }
      }
      if (!$ready) { throw "Fresh packaged server was not ready" }
      Request -Path '/admin' -Status 403 | Out-Null
      Request -Path '/posts/package-published' -Status 404 | Out-Null
      $setupPage = Request -Path '/setup' -Status 200
      if ($setupPage.Contains($freshKey) -or $setupPage.Contains($freshToken)) { throw "Fresh setup exposed secrets" }
      $proof = (Request -Path '/__axonyx/csrf' -Status 200 -Headers @{ Origin = $baseUrl } | ConvertFrom-Json).token
      Request -Path '/__axonyx/action?path=%2Fsetup&name=Install' -Status 303 -Body "siteName=Native+installation&email=native%40example.com&password=$password&setupToken=$freshToken" -Headers @{ Origin = $baseUrl; Accept = 'text/html'; 'X-Axonyx-CSRF' = $proof } | Out-Null
      $admin = Request -Path '/admin' -Status 200
      if (!$admin.Contains('native@example.com') -or !$admin.Contains('Start with your first draft')) { throw "Fresh setup did not reach its own empty admin overview" }
      Request -Path '/setup' -Status 403 | Out-Null
      $proof = (Request -Path '/__axonyx/csrf' -Status 200 -Headers @{ Origin = $baseUrl } | ConvertFrom-Json).token
      $headers = @{ Origin = $baseUrl; Accept = 'text/html'; 'X-Axonyx-CSRF' = $proof }
      Request -Path '/__axonyx/action?path=%2Fadmin%2Fposts%2Fnew&name=CreatePost' -Status 303 -Body 'title=Native+first+post&slug=native-first&body=Native+published+content&status=published' -Headers $headers | Out-Null
      $story = Request -Path '/posts/native-first' -Status 200
      if (!$story.Contains('Native published content')) { throw "Fresh native installation could not publish" }
      Request -Path '/__axonyx/action?path=%2Fadmin&name=SignOut' -Status 303 -Body 'submit=1' -Headers $headers | Out-Null
      Request -Path '/admin' -Status 403 | Out-Null
      $proof = (Request -Path '/__axonyx/csrf' -Status 200 -Headers @{ Origin = $baseUrl } | ConvertFrom-Json).token
      Request -Path '/__axonyx/action?path=%2Flogin&name=SignIn' -Status 303 -Body "email=native%40example.com&password=$password" -Headers @{ Origin = $baseUrl; Accept = 'text/html'; 'X-Axonyx-CSRF' = $proof } | Out-Null
      Request -Path '/admin' -Status 200 | Out-Null
      if ([IO.File]::ReadAllText($packageConfig) -cne $freshConfig) { throw "Fresh server modified configuration" }
      $freshBackups = Join-Path $freshRoot 'backups'
      New-Item -ItemType Directory -Path $freshBackups | Out-Null
      if (!$IsWindows) { [IO.File]::SetUnixFileMode($freshBackups, [IO.UnixFileMode]::UserRead -bor [IO.UnixFileMode]::UserWrite -bor [IO.UnixFileMode]::UserExecute) }
      $freshBundle = Join-Path $freshBackups 'first-post'
      $restoredPath = Join-Path $freshData 'restored.db'
      $pathBefore = $env:PATH
      try {
        $env:PATH = Join-Path $fixture 'no-toolchain-path'
        & $freshMaintenance backup $freshPath $freshBundle
        if ($LASTEXITCODE -ne 0) { throw "Fresh native live backup failed" }
        & $freshMaintenance verify $freshBundle
        if ($LASTEXITCODE -ne 0) { throw "Fresh native backup verification failed" }
        & $freshMaintenance restore $freshBundle $restoredPath
        if ($LASTEXITCODE -ne 0) { throw "Fresh native restore failed" }
      } finally { $env:PATH = $pathBefore }
      $process.Kill($true)
      $process.WaitForExit()
      $process.Dispose()
      $process = $null
      $pathBefore = $env:PATH
      try {
        $env:PATH = Join-Path $fixture 'no-toolchain-path'
        $process = Start-Process @options
      } finally { $env:PATH = $pathBefore }
      $ready = $false
      for ($attempt = 0; $attempt -lt 120; $attempt++) {
        if ($process.HasExited) { throw "Restarted fresh server exited before readiness" }
        try { Request -Path '/__axonyx/ready' -Status 200 | Out-Null; $ready = $true; break } catch { Start-Sleep -Milliseconds 250 }
      }
      if (!$ready) { throw "Restarted fresh server was not ready" }
      $story = Request -Path '/posts/native-first' -Status 200
      if (!$story.Contains('Native published content')) { throw "Fresh post did not survive restart" }
      Request -Path '/admin' -Status 200 | Out-Null
      $process.Kill($true)
      $process.WaitForExit()
      $process.Dispose()
      $process = $null
      $restoredKey = [Convert]::ToHexString([Security.Cryptography.RandomNumberGenerator]::GetBytes(32))
      $restoredConfig = $freshConfig.Replace('sqlite://data/legura.db', 'sqlite://data/restored.db').Replace($freshKey, $restoredKey)
      [IO.File]::WriteAllText($packageConfig, $restoredConfig)
      $pathBefore = $env:PATH
      try {
        $env:PATH = Join-Path $fixture 'no-toolchain-path'
        $process = Start-Process @options
      } finally { $env:PATH = $pathBefore }
      $ready = $false
      for ($attempt = 0; $attempt -lt 120; $attempt++) {
        if ($process.HasExited) { throw "Recovered fresh server exited before readiness" }
        try { Request -Path '/__axonyx/ready' -Status 200 | Out-Null; $ready = $true; break } catch { Start-Sleep -Milliseconds 250 }
      }
      if (!$ready) { throw "Recovered fresh server was not ready" }
      $story = Request -Path '/posts/native-first' -Status 200
      if (!$story.Contains('Native published content')) { throw "Fresh post was missing from restored database" }
      Request -Path '/admin' -Status 403 | Out-Null
      Request -Path '/setup' -Status 403 | Out-Null
      $proof = (Request -Path '/__axonyx/csrf' -Status 200 -Headers @{ Origin = $baseUrl } | ConvertFrom-Json).token
      Request -Path '/__axonyx/action?path=%2Flogin&name=SignIn' -Status 303 -Body "email=native%40example.com&password=$password" -Headers @{ Origin = $baseUrl; Accept = 'text/html'; 'X-Axonyx-CSRF' = $proof } | Out-Null
      Request -Path '/admin' -Status 200 | Out-Null
      if ([IO.File]::ReadAllText($packageConfig) -cne $restoredConfig) { throw "Recovered server modified configuration" }
      Write-Host "Fresh native DB passed without CLI: setup, publication, restart persistence, live backup/verify, no-clobber restore, rotated sessions and recovered login."
      $process.Kill($true)
      $process.WaitForExit()
      $process.Dispose()
      $process = $null
      $httpsRoot = Join-Path $fixture 'https-native-package'
      $httpsData = Join-Path $fixture 'https-private-data'
      New-Item -ItemType Directory -Path $httpsRoot, $httpsData | Out-Null
      foreach ($entry in Get-ChildItem -LiteralPath $packageRoot) {
        if ($entry.Name -notin @('data', '.env')) { Copy-Item -LiteralPath $entry.FullName -Destination $httpsRoot -Recurse }
      }
      $httpsMedia = Join-Path $httpsData 'media'
      New-Item -ItemType Directory -Path $httpsMedia | Out-Null
      if (!$IsWindows) {
        [IO.File]::SetUnixFileMode($httpsData, [IO.UnixFileMode]::UserRead -bor [IO.UnixFileMode]::UserWrite -bor [IO.UnixFileMode]::UserExecute)
        [IO.File]::SetUnixFileMode($httpsMedia, [IO.UnixFileMode]::UserRead -bor [IO.UnixFileMode]::UserWrite -bor [IO.UnixFileMode]::UserExecute)
      }
      $httpsMaintenance = Join-Path $httpsRoot "legura-maintenance$extension"
      $httpsDatabase = Join-Path $httpsData 'legura.db'
      $httpsConfig = Join-Path $httpsRoot '.env'
      $pathBefore = $env:PATH
      try {
        $env:PATH = Join-Path $fixture 'no-toolchain-path'
        & $httpsMaintenance init $httpsDatabase
        if ($LASTEXITCODE -ne 0) { throw 'HTTPS-mode fresh database initialization failed' }
        $httpsDatabaseHash = (Get-FileHash -LiteralPath $httpsDatabase).Hash
        $httpsOutput = & $httpsMaintenance config-https $httpsRoot $httpsData
        if ($LASTEXITCODE -ne 0) { throw 'HTTPS-mode fresh configuration failed' }
        $httpsContent = [IO.File]::ReadAllText($httpsConfig)
        if (!$httpsContent.Contains('AX_SECRET_SESSION_COOKIE_SECURE=true') -or
            !$httpsContent.Contains('AX_SECRET_MEDIA_DRIVER=local') -or
            $httpsContent -notmatch '(?m)^AX_SECRET_MEDIA_ROOT=.*https-private-data[/\\]media$' -or
            "$httpsOutput" -match '(?i)[a-f0-9]{64}' -or
            (Get-FileHash -LiteralPath $httpsDatabase).Hash -ne $httpsDatabaseHash) {
          throw 'HTTPS-mode config leaked secrets or changed external data'
        }
        & $httpsMaintenance config-https $httpsRoot $httpsData
        if ($LASTEXITCODE -eq 0 -or [IO.File]::ReadAllText($httpsConfig) -cne $httpsContent) { throw 'HTTPS-mode config replaced an existing file' }
        $options.FilePath = Join-Path $httpsRoot "legura-server$extension"
        $options.Remove('ArgumentList')
        $options.WorkingDirectory = $httpsRoot
        $env:AXONYX_HOST = '127.0.0.1'
        $env:AXONYX_PORT = "$Port"
        if (Get-Command cargo,git,node,cargo-ax,pwsh -ErrorAction SilentlyContinue) { throw 'Direct native runtime still exposes developer tools or PowerShell' }
        $process = Start-Process @options
      } finally {
        $env:PATH = $pathBefore
        Remove-Item Env:AXONYX_HOST,Env:AXONYX_PORT -ErrorAction SilentlyContinue
      }
      $ready = $false
      for ($attempt = 0; $attempt -lt 120; $attempt++) {
        if ($process.HasExited) { throw 'HTTPS-mode native server exited before readiness' }
        try { Request -Path '/__axonyx/ready' -Status 200 | Out-Null; $ready = $true; break } catch { Start-Sleep -Milliseconds 250 }
      }
      if (!$ready) { throw 'HTTPS-mode native server was not ready' }
      Request -Path '/setup' -Status 200 | Out-Null
      $httpsHandler = [Net.Http.HttpClientHandler]::new()
      $httpsHandler.AllowAutoRedirect = $false
      $httpsHandler.CookieContainer = [Net.CookieContainer]::new()
      $httpsClient = [Net.Http.HttpClient]::new($httpsHandler)
      try {
        $csrfRequest = [Net.Http.HttpRequestMessage]::new([Net.Http.HttpMethod]::Get, "$baseUrl/__axonyx/csrf")
        $csrfRequest.Headers.Add('Origin', $baseUrl)
        try {
          $csrfResponse = $httpsClient.SendAsync($csrfRequest).GetAwaiter().GetResult()
          try {
            if ([int]$csrfResponse.StatusCode -ne 200) { throw 'HTTPS-mode CSRF bootstrap failed' }
            $httpsProof = ($csrfResponse.Content.ReadAsStringAsync().GetAwaiter().GetResult() | ConvertFrom-Json).token
          } finally { $csrfResponse.Dispose() }
        } finally { $csrfRequest.Dispose() }
        $httpsToken = [regex]::Match($httpsContent, '(?m)^AX_SECRET_SETUP_TOKEN=([a-f0-9]{64})$').Groups[1].Value
        if (!$httpsProof -or !$httpsToken) { throw 'HTTPS-mode private setup credentials are incomplete' }
        $setupRequest = [Net.Http.HttpRequestMessage]::new([Net.Http.HttpMethod]::Post, "$baseUrl/__axonyx/action?path=%2Fsetup&name=Install")
        $setupRequest.Content = [Net.Http.StringContent]::new("siteName=HTTPS+proof&email=https%40example.com&password=$password&setupToken=$httpsToken", [Text.Encoding]::UTF8, 'application/x-www-form-urlencoded')
        $setupRequest.Headers.Add('Origin', $baseUrl)
        $setupRequest.Headers.Add('Accept', 'text/html')
        $setupRequest.Headers.Add('X-Axonyx-CSRF', $httpsProof)
        try {
          $setupResponse = $httpsClient.SendAsync($setupRequest).GetAwaiter().GetResult()
          try {
            $cookies = [string]::Join(';', $setupResponse.Headers.GetValues('Set-Cookie'))
            if ([int]$setupResponse.StatusCode -ne 303 -or $cookies -notmatch '(?i)(^|;)\s*Secure(\s*;|$)' -or $cookies -notmatch '(?i)(^|;)\s*HttpOnly(\s*;|$)') {
              throw 'HTTPS-mode setup did not issue a Secure, HttpOnly session cookie'
            }
          } finally { $setupResponse.Dispose() }
        } finally { $setupRequest.Dispose() }
      } finally {
        $httpsClient.Dispose()
        $httpsHandler.Dispose()
      }
      if (!$IsWindows) {
        & /bin/kill -TERM $process.Id
        if ($LASTEXITCODE -ne 0 -or !$process.WaitForExit(10000)) { throw 'HTTPS-mode native server did not stop on SIGTERM' }
        $process.Dispose()
        $process = $null
        $pathBefore = $env:PATH
        try {
          $env:PATH = Join-Path $fixture 'no-toolchain-path'
          $env:AXONYX_HOST = '127.0.0.1'
          $env:AXONYX_PORT = "$Port"
          $process = Start-Process @options
        } finally {
          $env:PATH = $pathBefore
          Remove-Item Env:AXONYX_HOST,Env:AXONYX_PORT -ErrorAction SilentlyContinue
        }
        $ready = $false
        for ($attempt = 0; $attempt -lt 120; $attempt++) {
          if ($process.HasExited) { throw 'HTTPS-mode native server exited during restart' }
          try { Request -Path '/__axonyx/ready' -Status 200 | Out-Null; $ready = $true; break } catch { Start-Sleep -Milliseconds 250 }
        }
        if (!$ready) { throw 'HTTPS-mode native server did not restart' }
        Request -Path '/setup' -Status 403 | Out-Null
        Request -Path '/admin' -Status 403 | Out-Null
        $proof = (Request -Path '/__axonyx/csrf' -Status 200 -Headers @{ Origin = $baseUrl } | ConvertFrom-Json).token
        Request -Path '/__axonyx/action?path=%2Flogin&name=SignIn' -Status 303 -Body "email=https%40example.com&password=$password" -Headers @{ Origin = $baseUrl; Accept = 'text/html'; 'X-Axonyx-CSRF' = $proof } | Out-Null
        Write-Host 'Linux native SIGTERM/restart passed: setup stayed locked and administrator credentials were accepted. Secure cookies require HTTPS for subsequent admin requests.'
      }
      $process.Kill($true)
      $process.WaitForExit()
      $process.Dispose()
      $process = $null
      if ([IO.File]::ReadAllText($httpsConfig) -cne $httpsContent) { throw 'HTTPS-mode server modified configuration' }
      Write-Host 'Fresh HTTPS-mode config passed: external data/media, independent private secrets, no clobber and direct native startup without developer tools or PowerShell. TLS/service setup is not covered.'
    }
    # The expected no-clobber failure must not become pwsh's final exit status.
    $global:LASTEXITCODE = 0
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
