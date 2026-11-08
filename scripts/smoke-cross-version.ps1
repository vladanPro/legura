param(
  [Parameter(Mandatory)][string] $PriorTool,
  [Parameter(Mandatory)][string] $CurrentTool
)
$ErrorActionPreference = 'Stop'
if ($PSVersionTable.PSVersion -lt [version]'7.4') { throw 'Cross-version smoke requires PowerShell 7.4 or later' }
$prior = [IO.Path]::GetFullPath($PriorTool)
$current = [IO.Path]::GetFullPath($CurrentTool)
if (!(Test-Path -LiteralPath $prior -PathType Leaf) -or !(Test-Path -LiteralPath $current -PathType Leaf) -or $prior -eq $current) {
  throw 'Provide distinct existing 0.0.1 and 0.0.2 maintenance binaries'
}
$temporaryRoot = [IO.Path]::GetFullPath([IO.Path]::GetTempPath())
$fixture = Join-Path $temporaryRoot ('legura-cross-version-' + [Guid]::NewGuid().ToString('N'))
try {
  New-Item -ItemType Directory -Path $fixture | Out-Null
  $database = Join-Path $fixture 'prior.db'
  $media = Join-Path $fixture 'media'
  New-Item -ItemType Directory -Path $media | Out-Null
  & $prior init $database | Out-Null
  if ($LASTEXITCODE -ne 0) { throw '0.0.1 could not initialize its database' }

  $priorBundle = Join-Path $fixture 'prior-db-only'
  & $prior backup $database $priorBundle | Out-Null
  if ($LASTEXITCODE -ne 0) { throw '0.0.1 could not create its database-only backup' }
  $manifest = Get-Content -LiteralPath (Join-Path $priorBundle 'manifest.json') -Raw | ConvertFrom-Json
  if ($manifest.version -cne '0.0.1') { throw 'Prior binary did not produce a 0.0.1 backup' }
  & $current verify $priorBundle | Out-Null
  if ($LASTEXITCODE -ne 0) { throw '0.0.2 cannot verify the 0.0.1 database-only backup' }
  $restoredDatabase = Join-Path $fixture 'db-only-restored.db'
  & $current restore $priorBundle $restoredDatabase | Out-Null
  if ($LASTEXITCODE -ne 0) { throw '0.0.2 cannot restore the 0.0.1 database-only backup' }

  $image = [Convert]::FromBase64String('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGNwqzjxHwAFFAKG7uYx/AAAAABJRU5ErkJggg==')
  $key = 'a' * 64
  $digest = [Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($image)).ToLowerInvariant()
  [IO.File]::WriteAllBytes((Join-Path $media $key), $image)
  $seed = @'
import sqlite3, sys
with sqlite3.connect(sys.argv[1]) as db:
    db.execute('INSERT INTO legura_media (id, store_id, content_type, byte_size, width, height, sha256) VALUES (?, ?, ?, ?, ?, ?, ?)', (sys.argv[2], 'local-media', 'image/png', int(sys.argv[3]), 1, 1, sys.argv[4]))
'@
  & python -c $seed $database $key $image.Length $digest
  if ($LASTEXITCODE -ne 0) { throw 'Could not seed an isolated 0.0.1 media row' }
  $priorArchive = Join-Path $fixture 'prior.legura'
  & $prior archive-backup $database local-media $media $priorArchive | Out-Null
  if ($LASTEXITCODE -ne 0) { throw '0.0.1 could not create its media archive' }
  $report = & $current archive-verify $priorArchive | ConvertFrom-Json
  if ($LASTEXITCODE -ne 0 -or $report.files -ne 2) { throw '0.0.2 cannot verify the 0.0.1 image archive' }
  $recovery = Join-Path $fixture 'inactive-recovery'
  & $current archive-restore $priorArchive $recovery | Out-Null
  if ($LASTEXITCODE -ne 0) { throw '0.0.2 cannot recover the 0.0.1 image archive' }
  & $current archive-check-recovery $priorArchive $recovery | Out-Null
  if ($LASTEXITCODE -ne 0) { throw '0.0.2 rejects the 0.0.1 schema or media' }
  $recoveredImage = Join-Path $recovery "media/local-media/$key"
  if ((Get-FileHash -LiteralPath $recoveredImage -Algorithm SHA256).Hash.ToLowerInvariant() -cne $digest) {
    throw 'Recovered 0.0.1 image bytes changed'
  }

  $newBundle = Join-Path $fixture 'new-db-only'
  & $current backup $restoredDatabase $newBundle | Out-Null
  if ($LASTEXITCODE -ne 0) { throw '0.0.2 could not create its database-only backup' }
  $manifest = Get-Content -LiteralPath (Join-Path $newBundle 'manifest.json') -Raw | ConvertFrom-Json
  if ($manifest.version -cne '0.0.2') { throw 'Current binary did not produce a 0.0.2 backup' }
  & $prior verify $newBundle *> $null
  if ($LASTEXITCODE -eq 0) { throw '0.0.1 incorrectly accepted a future database-only backup' }
  $newArchive = Join-Path $fixture 'new.legura'
  & $current archive-backup (Join-Path $recovery 'database.sqlite') local-media (Join-Path $recovery 'media/local-media') $newArchive | Out-Null
  if ($LASTEXITCODE -ne 0) { throw '0.0.2 could not create its media archive' }
  & $prior archive-verify $newArchive *> $null
  if ($LASTEXITCODE -eq 0) { throw '0.0.1 incorrectly accepted a future media archive' }
  Write-Host 'Cross-version backup proof passed: 0.0.2 reads real 0.0.1 database/media backups; 0.0.1 rejects 0.0.2 backups.'
} finally {
  $resolved = [IO.Path]::GetFullPath($fixture)
  if ($resolved.StartsWith($temporaryRoot, [StringComparison]::OrdinalIgnoreCase) -and
      (Split-Path $resolved -Leaf) -match '^legura-cross-version-[0-9a-f]{32}$') {
    if (Test-Path -LiteralPath $resolved) { Remove-Item -LiteralPath $resolved -Recurse -Force }
  } else { throw 'Refusing cleanup outside the isolated cross-version fixture' }
}
exit 0
