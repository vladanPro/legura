param([ValidateRange(1024, 65535)][int] $Port = 3940)
$ErrorActionPreference = "Stop"
if ($PSVersionTable.PSVersion -lt [version]"7.4") { throw "This convenience launcher requires PowerShell 7.4 or later" }
$root = [IO.Path]::GetFullPath($PSScriptRoot)
$extension = if ($IsWindows) { ".exe" } else { "" }
$binary = Join-Path $root "legura-server$extension"
if (!(Test-Path -LiteralPath (Join-Path $root ".env") -PathType Leaf)) { throw "Configure a private .env first; this package does not initialize or migrate a database" }
foreach ($path in @($binary, (Join-Path $root ".env"), (Join-Path $root "dist"))) {
  if ((Get-Item -LiteralPath $path -Force).Attributes -band [IO.FileAttributes]::ReparsePoint) { throw "Package program/configuration/assets must not be linked" }
}
$hostBefore = $env:AXONYX_HOST
$portBefore = $env:AXONYX_PORT
Push-Location $root
try {
  $env:AXONYX_HOST = "127.0.0.1"
  $env:AXONYX_PORT = "$Port"
  & $binary
  if ($LASTEXITCODE -ne 0) { throw "Packaged server exited unsuccessfully" }
} finally {
  $env:AXONYX_HOST = $hostBefore
  $env:AXONYX_PORT = $portBefore
  Pop-Location
}
