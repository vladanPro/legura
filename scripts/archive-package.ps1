param(
  [Parameter(Mandatory)][string] $PackageDirectory,
  [Parameter(Mandatory)][string] $OutputDirectory
)
$ErrorActionPreference = 'Stop'
if ($PSVersionTable.PSVersion -lt [version]'7.4') { throw 'Archiving requires PowerShell 7.4 or later' }
$root = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$package = [IO.Path]::GetFullPath($PackageDirectory, $root)
$output = [IO.Path]::GetFullPath($OutputDirectory, $root)
if (!(Test-Path -LiteralPath $package -PathType Container)) { throw 'Package directory does not exist' }
if (!(Test-Path -LiteralPath $output -PathType Container)) { throw 'Archive output directory does not exist' }
foreach ($path in @($package, $output)) {
  if ((Get-Item -LiteralPath $path -Force).Attributes -band [IO.FileAttributes]::ReparsePoint) { throw 'Package and output directories must not be linked' }
}
$relativeOutput = [IO.Path]::GetRelativePath($package, $output)
if ($relativeOutput -eq '.' -or (!$relativeOutput.StartsWith('..') -and ![IO.Path]::IsPathRooted($relativeOutput))) {
  throw 'Archive output must be outside the package directory'
}
$manifest = Get-Content -LiteralPath (Join-Path $package 'package.json') -Raw | ConvertFrom-Json
if ($manifest.format -ne 1 -or $manifest.product -ne 'legura' -or
    [string]$manifest.version -cnotmatch '^[0-9]+\.[0-9]+\.[0-9]+$' -or
    [string]$manifest.platform -cnotmatch '^[A-Za-z0-9._-]+$') {
  throw 'Package manifest has an invalid product, version or platform'
}
$extension = if ($IsWindows) { '.exe' } else { '' }
$maintenance = Join-Path $package "legura-maintenance$extension"
& $maintenance verify-package $package
if ($LASTEXITCODE -ne 0) { throw 'Package integrity verification failed before archiving' }
$archiveName = "legura-$($manifest.version)-$($manifest.platform).tar.gz"
$archive = Join-Path $output $archiveName
$checksum = "$archive.sha256"
if ((Test-Path -LiteralPath $archive) -or (Test-Path -LiteralPath $checksum)) { throw 'Archive or checksum already exists' }
$tar = (Get-Command tar -CommandType Application -ErrorAction Stop).Source
$temporary = Join-Path $output ('.legura-' + [Guid]::NewGuid().ToString('N') + '.tar.gz')
$archiveCreated = $false
$checksumCreated = $false
$complete = $false
try {
  & $tar -czf $temporary -C $package .
  if ($LASTEXITCODE -ne 0 -or !(Test-Path -LiteralPath $temporary -PathType Leaf)) { throw 'Archive creation failed' }
  $hash = (Get-FileHash -LiteralPath $temporary -Algorithm SHA256).Hash.ToLowerInvariant()
  [IO.File]::Move($temporary, $archive, $false)
  $archiveCreated = $true
  $stream = [IO.File]::Open($checksum, [IO.FileMode]::CreateNew, [IO.FileAccess]::Write, [IO.FileShare]::None)
  $checksumCreated = $true
  try {
    $bytes = [Text.Encoding]::ASCII.GetBytes("$hash  $archiveName`n")
    $stream.Write($bytes, 0, $bytes.Length)
    $stream.Flush($true)
  } finally { $stream.Dispose() }
  $complete = $true
  Write-Host "Release-candidate archive created: $archive"
  Write-Host "Checksum: $checksum"
  Write-Host 'This archive is not signed or published; it is not a production installer.'
} finally {
  if (Test-Path -LiteralPath $temporary) { Remove-Item -LiteralPath $temporary -Force }
  if (!$complete) {
    if ($checksumCreated) { Remove-Item -LiteralPath $checksum -Force }
    if ($archiveCreated) { Remove-Item -LiteralPath $archive -Force }
  }
}
