param(
  [ValidateSet("build", "backup", "verify", "restore")][string] $Task = "build",
  [string] $Bundle = "",
  [string] $Destination = ""
)
$ErrorActionPreference = "Stop"
if ($PSVersionTable.PSVersion -lt [version]"7.4") { throw "Maintenance requires PowerShell 7.4 or later" }
$root = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot ".."))
if ($Task -ne "build" -and !$Bundle) { throw "Specify -Bundle as a backup directory" }
if ($Task -eq "restore" -and !$Destination) { throw "Restore requires -Destination as a NEW database file" }
if ($Task -ne "restore" -and $Destination) { throw "-Destination is only supported for restore" }
if ($Task -eq "build" -and $Bundle) { throw "-Bundle is not supported for build" }
if ($Bundle) { $Bundle = [IO.Path]::GetFullPath($Bundle, $root) }
if ($Destination) { $Destination = [IO.Path]::GetFullPath($Destination, $root) }

Push-Location $root
try {
  # Build the tool from this checkout so embedded migration checks stay current.
  & cargo build --locked --release --bin legura-maintenance
  if ($LASTEXITCODE -ne 0) { throw "Cannot build Legura maintenance tool" }
  $metadata = & cargo metadata --no-deps --format-version 1
  if ($LASTEXITCODE -ne 0) { throw "Cannot resolve the Cargo target directory" }
  $target = ($metadata | ConvertFrom-Json).target_directory
  $binary = Join-Path $target "release/legura-maintenance"
  if ($IsWindows) { $binary += ".exe" }
  switch ($Task) {
    "build" { Write-Host "Maintenance tool built. Use backup, verify or restore with an explicit bundle path." }
    "backup" { & $binary backup (Join-Path $root "data/legura.db") $Bundle }
    "verify" { & $binary verify $Bundle }
    "restore" { & $binary restore $Bundle $Destination }
  }
  if ($LASTEXITCODE -ne 0) { throw "Legura maintenance $Task failed; original database and configuration were not switched" }
} finally { Pop-Location }
