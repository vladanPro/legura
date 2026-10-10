param(
  [ValidateSet("init", "check", "build", "start", "prepare")][string] $Task = "check",
  [string] $ToolManifest = "",
  [ValidateRange(1024, 65535)][int] $Port = 3940
)
$ErrorActionPreference = "Stop"
if ($PSVersionTable.PSVersion -lt [version]"7.4") { throw "Local preparation requires PowerShell 7.4 or later" }
$root = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot ".."))
if ($ToolManifest) { $ToolManifest = [IO.Path]::GetFullPath($ToolManifest) }

function Invoke-Ax {
  param([string[]] $Arguments)
  if ($ToolManifest) {
    & cargo run --manifest-path $ToolManifest -p cargo-axonyx --bin cargo-axonyx -- @Arguments
  } else {
    & cargo ax @Arguments
  }
  if ($LASTEXITCODE -ne 0) { throw "Axonyx command failed: $($Arguments -join ' ')" }
}

function Assert-LocalConfiguration {
  $path = Join-Path $root ".env"
  if (!(Test-Path -LiteralPath $path -PathType Leaf)) { throw "Run local.ps1 -Task init first to create local configuration" }
  if ((Get-Item -LiteralPath $path -Force).Attributes -band [IO.FileAttributes]::ReparsePoint) { throw "Local configuration must not be a symlink" }
  $values = @{}
  foreach ($line in [IO.File]::ReadAllLines($path)) {
    if (!$line.Trim() -or $line.Trim().StartsWith('#')) { continue }
    $parts = $line.Split('=', 2)
    if ($parts.Length -ne 2) { throw "Local .env requires plain KEY=value entries" }
    $key = $parts[0].Trim()
    if ($values.ContainsKey($key)) { throw "Local .env contains duplicate keys" }
    $values[$key] = $parts[1].Trim()
  }
  if ($values["AX_SECRET_DB_URL"] -cne "sqlite://data/legura.db" -or $values["AX_SECRET_DB_DIALECT"] -cne "sqlite") {
    throw "This local helper only supports sqlite://data/legura.db; use a separate operator procedure for other databases"
  }
  foreach ($key in @("AX_SECRET_SESSION_KEY", "AX_SECRET_SETUP_TOKEN")) {
    if ($values[$key] -cnotmatch '^[0-9A-Fa-f]{64}$') { throw "Local secrets must be independent 32-byte hexadecimal values; existing configuration was not changed" }
  }
  if ($values["AX_SECRET_SESSION_KEY"] -eq $values["AX_SECRET_SETUP_TOKEN"]) { throw "Session key and setup token must be independent" }
  if ($values["AX_SECRET_SESSION_COOKIE_SECURE"] -cne "false") { throw "Local HTTP preparation requires secure cookies disabled; this helper is not for production" }
  foreach ($key in @("AX_SECRET_DB_URL", "AX_SECRET_DB_DIALECT", "AX_SECRET_SESSION_KEY", "AX_SECRET_SETUP_TOKEN", "AX_SECRET_SESSION_COOKIE_SECURE")) {
    $override = [Environment]::GetEnvironmentVariable($key)
    if ($override -and $override -cne $values[$key]) { throw "Process configuration overrides local .env; clear the conflicting variables before using this helper" }
  }
  $data = Join-Path $root "data"
  if (!(Test-Path -LiteralPath $data -PathType Container)) { throw "Local data directory is missing; run init" }
  foreach ($candidate in @($data, (Join-Path $data "legura.db"), (Join-Path $data "legura.db-wal"), (Join-Path $data "legura.db-shm"))) {
    if ((Test-Path -LiteralPath $candidate) -and ((Get-Item -LiteralPath $candidate -Force).Attributes -band [IO.FileAttributes]::ReparsePoint)) {
      throw "Local data paths must not be symlinks"
    }
  }
}

function Initialize-Local {
  $data = Join-Path $root "data"
  if (!(Test-Path -LiteralPath $data)) {
    New-Item -ItemType Directory -Path $data | Out-Null
    if (!$IsWindows) { [IO.File]::SetUnixFileMode($data, [IO.UnixFileMode]::UserRead -bor [IO.UnixFileMode]::UserWrite -bor [IO.UnixFileMode]::UserExecute) }
  }
  $envPath = Join-Path $root ".env"
  if (!(Test-Path -LiteralPath $envPath)) {
    $session = [Convert]::ToHexString([Security.Cryptography.RandomNumberGenerator]::GetBytes(32))
    $setup = [Convert]::ToHexString([Security.Cryptography.RandomNumberGenerator]::GetBytes(32))
    $config = "AX_SECRET_DB_URL=sqlite://data/legura.db`nAX_SECRET_DB_DIALECT=sqlite`nAX_SECRET_SESSION_KEY=$session`nAX_SECRET_SETUP_TOKEN=$setup`nAX_SECRET_SESSION_COOKIE_SECURE=false`n"
    # CreateNew prevents replacing configuration created by another preparation.
    $options = [IO.FileStreamOptions]::new()
    $options.Mode = [IO.FileMode]::CreateNew
    $options.Access = [IO.FileAccess]::Write
    $options.Share = [IO.FileShare]::None
    if (!$IsWindows) { $options.UnixCreateMode = [IO.UnixFileMode]::UserRead -bor [IO.UnixFileMode]::UserWrite }
    $stream = [IO.FileStream]::new($envPath, $options)
    try {
      $bytes = [Text.UTF8Encoding]::new($false).GetBytes($config)
      $stream.Write($bytes, 0, $bytes.Length)
    } finally { $stream.Dispose() }
  }
  Assert-LocalConfiguration
  Invoke-Ax -Arguments @("db", "migrate")
  Invoke-Ax -Arguments @("db", "pull")
  Write-Host "Local database initialized. Setup token is in the ignored .env file; it was not printed."
}

Push-Location $root
try {
  switch ($Task) {
    "init" { Initialize-Local }
    "prepare" {
      Initialize-Local
      Invoke-Ax -Arguments @("check")
      Invoke-Ax -Arguments @("build", "--clean", "--compiled")
      Write-Host "Prepared. Next: pwsh -File scripts/local.ps1 -Task start -Port $Port"
      Write-Host "First-run setup: http://127.0.0.1:$Port/setup"
    }
    "check" { Assert-LocalConfiguration; Invoke-Ax -Arguments @("check") }
    "build" { Assert-LocalConfiguration; Invoke-Ax -Arguments @("build", "--clean", "--compiled") }
    "start" { Assert-LocalConfiguration; Invoke-Ax -Arguments @("run", "start", "--compiled", "--host", "127.0.0.1", "--port", "$Port") }
  }
} finally { Pop-Location }
