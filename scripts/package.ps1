param([Parameter(Mandatory)][string] $OutputDirectory)
$ErrorActionPreference = "Stop"
if ($PSVersionTable.PSVersion -lt [version]"7.4") { throw "Packaging requires PowerShell 7.4 or later" }
$root = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot ".."))
$output = [IO.Path]::GetFullPath($OutputDirectory, $root)
if (Test-Path -LiteralPath $output) { throw "Package destination must be new" }
if (!(Test-Path -LiteralPath (Split-Path $output) -PathType Container)) { throw "Package parent directory must exist" }
if ($env:CARGO_BUILD_TARGET) { throw "Cross-target packaging is not supported by this pilot" }
Push-Location $root
try {
  & cargo ax check
  if ($LASTEXITCODE -ne 0) { throw "Source check failed" }
  & cargo ax build --clean --compiled
  if ($LASTEXITCODE -ne 0) { throw "Compiled server build failed" }
  & ./scripts/maintenance.ps1 -Task build
  $metadata = & cargo metadata --no-deps --format-version 1
  if ($LASTEXITCODE -ne 0) { throw "Cannot resolve Cargo metadata" }
  $metadata = $metadata | ConvertFrom-Json
  $compiler = & rustc -vV
  if ($LASTEXITCODE -ne 0) { throw "Cannot identify the native platform" }
  $hostLine = @($compiler | Where-Object { $_ -like 'host: *' })
  if ($hostLine.Count -ne 1) { throw "Cannot identify the compiler host" }
  $extension = if ($IsWindows) { ".exe" } else { "" }
  $files = @(
    @{ Source = (Join-Path $metadata.target_directory "release/axonyx-production$extension"); Target = "legura-server$extension" },
    @{ Source = (Join-Path $metadata.target_directory "release/legura-maintenance$extension"); Target = "legura-maintenance$extension" },
    @{ Source = (Join-Path $root "scripts/package-start.ps1"); Target = "start.ps1" },
    @{ Source = (Join-Path $root "docs/native-package-v0.md"); Target = "README.md" }
  )
  $dist = Join-Path $root "dist"
  if ((Get-Item -LiteralPath $dist -Force).Attributes -band [IO.FileAttributes]::ReparsePoint) { throw "Package assets must not be linked" }
  foreach ($item in Get-ChildItem -LiteralPath $dist -Recurse -Force) {
    if ($item.Attributes -band [IO.FileAttributes]::ReparsePoint) { throw "Package assets must not contain links" }
    if ($item.PSIsContainer) { continue }
    $relative = [IO.Path]::GetRelativePath($root, $item.FullName).Replace('\', '/')
    # Inspection reports are development artifacts, not runtime assets.
    if ($relative.StartsWith('dist/_ax/melt/')) { continue }
    $files += @{ Source = $item.FullName; Target = $relative }
  }
  foreach ($file in $files) {
    $item = Get-Item -LiteralPath $file.Source -Force
    if ($item.PSIsContainer -or ($item.Attributes -band [IO.FileAttributes]::ReparsePoint)) { throw "Package inputs must be regular files" }
  }
  New-Item -ItemType Directory -Path $output | Out-Null
  if (!$IsWindows) { [IO.File]::SetUnixFileMode($output, [IO.UnixFileMode]::UserRead -bor [IO.UnixFileMode]::UserWrite -bor [IO.UnixFileMode]::UserExecute) }
  $entries = foreach ($file in $files) {
    $destination = Join-Path $output $file.Target
    New-Item -ItemType Directory -Path (Split-Path $destination) -Force | Out-Null
    Copy-Item -LiteralPath $file.Source -Destination $destination
    if (!$IsWindows) { [IO.File]::SetUnixFileMode($destination, [IO.File]::GetUnixFileMode($file.Source)) }
    @{ path = $file.Target; sha256 = (Get-FileHash -LiteralPath $destination -Algorithm SHA256).Hash.ToLowerInvariant() }
  }
  $package = @($metadata.packages | Where-Object name -eq 'legura')
  $manifest = @{ format = 1; product = "legura"; version = $package[0].version; platform = $hostLine[0].Substring(6); files = @($entries | Sort-Object { $_.path }) }
  [IO.File]::WriteAllText((Join-Path $output "package.json"), ($manifest | ConvertTo-Json -Depth 5), [Text.UTF8Encoding]::new($false))
  Write-Host "Native package created. No database or .env included. Read README.md before running."
} finally { Pop-Location }
