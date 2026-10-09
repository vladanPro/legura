param(
  [ValidateSet("init", "check", "build", "start")][string] $Task = "check",
  [string] $ToolManifest = "",
  [int] $Port = 3940
)
$ErrorActionPreference = "Stop"
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

Push-Location $root
try {
  switch ($Task) {
    "init" {
      New-Item -ItemType Directory -Path (Join-Path $root "data") -Force | Out-Null
      $envPath = Join-Path $root ".env"
      if (!(Test-Path -LiteralPath $envPath)) {
        $session = [Convert]::ToHexString([Security.Cryptography.RandomNumberGenerator]::GetBytes(32))
        $setup = [Convert]::ToHexString([Security.Cryptography.RandomNumberGenerator]::GetBytes(32))
        $config = "AX_SECRET_DB_URL=sqlite://data/legura.db`nAX_SECRET_DB_DIALECT=sqlite`nAX_SECRET_SESSION_KEY=$session`nAX_SECRET_SETUP_TOKEN=$setup`nAX_SECRET_SESSION_COOKIE_SECURE=false`n"
        [IO.File]::WriteAllText($envPath, $config)
      }
      Invoke-Ax -Arguments @("db", "migrate")
      Invoke-Ax -Arguments @("db", "pull")
      Write-Host "Local database initialized. Setup token is in the ignored .env file."
    }
    "check" { Invoke-Ax -Arguments @("check") }
    "build" { Invoke-Ax -Arguments @("build", "--clean", "--compiled") }
    "start" { Invoke-Ax -Arguments @("run", "start", "--compiled", "--host", "127.0.0.1", "--port", "$Port") }
  }
} finally { Pop-Location }
