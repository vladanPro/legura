# Dot-sourced inside the disposable media proof only. Never changes an operator
# service, .env file, database path or process outside smoke-auth's fixture.
function Invoke-RecoveryActivationProof {
  param([string]$Email, [string]$Password)
  $fixturePrefix = [IO.Path]::GetFullPath($fixture).TrimEnd([IO.Path]::DirectorySeparatorChar) + [IO.Path]::DirectorySeparatorChar
  foreach ($path in @($db, $mediaRoot, $recoveryRoot, $recoveredDb, $recoveredMedia, $archivePath, $script:binary)) {
    if (![IO.Path]::GetFullPath($path).StartsWith($fixturePrefix, [StringComparison]::OrdinalIgnoreCase)) {
      throw 'Recovery activation proof requires exclusively disposable fixture paths'
    }
  }
  if (!$Media -or !$script:process -or $script:process.HasExited) { throw 'Recovery proof requires its running isolated media server' }
  $maintenanceName = if ($IsWindows) { 'legura-maintenance.exe' } else { 'legura-maintenance' }
  $maintenance = Join-Path (Split-Path $script:binary) $maintenanceName
  if (!(Test-Path -LiteralPath $maintenance -PathType Leaf)) { throw 'Fixture native maintenance binary is missing' }
  $saved = @{}
  foreach ($name in @('AX_SECRET_DB_URL', 'AX_SECRET_DB_DIALECT', 'AX_SECRET_SESSION_KEY', 'AX_SECRET_SETUP_TOKEN', 'AX_SECRET_SESSION_COOKIE_SECURE', 'AX_SECRET_MEDIA_DRIVER', 'AX_SECRET_MEDIA_ROOT', 'AX_SECRET_MEDIA_STORE_ID')) {
    $saved[$name] = [Environment]::GetEnvironmentVariable($name)
  }
  $sourceFingerprint = $null
  $sourceUnchanged = $false
  $archiveHash = (Get-FileHash -LiteralPath $archivePath -Algorithm SHA256).Hash
  $receiptHash = (Get-FileHash -LiteralPath (Join-Path $recoveryRoot 'recovery.json') -Algorithm SHA256).Hash

  function Get-SourceFingerprint {
    # No SQLite connection here: inspect closed source files without checkpointing
    # its WAL. Include the orphan object as well as referenced images.
    $hashes = [ordered]@{}
    foreach ($suffix in @('', '-wal', '-shm', '-journal')) {
      $path = $db + $suffix
      if (Test-Path -LiteralPath $path -PathType Leaf) { $hashes["database$suffix"] = (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash }
    }
    foreach ($file in @(Get-ChildItem -LiteralPath $mediaRoot -File | Sort-Object Name)) {
      $hashes["media/$($file.Name)"] = (Get-FileHash -LiteralPath $file.FullName -Algorithm SHA256).Hash
    }
    return ($hashes | ConvertTo-Json -Compress)
  }

  function Assert-RecoveredMedia([int]$Expected) {
    $count = & $python.Source -c 'import sqlite3,sys;d=sqlite3.connect(sys.argv[1]);print(d.execute("select count(*) from legura_media").fetchone()[0]);d.close()' $recoveredDb
    if ($LASTEXITCODE -ne 0 -or $count -ne "$Expected") { throw 'Unexpected recovered media row count' }
    if (@(Get-ChildItem -LiteralPath $recoveredMedia -File).Count -ne $Expected) { throw 'Unexpected recovered object count' }
    & $maintenance media-check $recoveredDb local-media $recoveredMedia | Out-Null
    if ($LASTEXITCODE -ne 0) { throw 'Activated recovery media audit failed' }
  }

  try {
    Stop-Fixture
    $sourceFingerprint = Get-SourceFingerprint
    & $maintenance archive-check-recovery $archivePath $recoveryRoot | Out-Null
    if ($LASTEXITCODE -ne 0) { throw 'Offline compatibility gate refused recovery activation' }

    $configPackage = Join-Path $fixture 'recovered-package'
    New-Item -ItemType Directory -Path $configPackage | Out-Null
    Copy-Item -LiteralPath (Join-Path $fixture 'dist') -Destination (Join-Path $configPackage 'dist') -Recurse
    & $maintenance config-recovery $archivePath $recoveryRoot $configPackage local-http | Out-Null
    if ($LASTEXITCODE -ne 0) { throw 'Native private recovery configuration generation failed' }
    $configPath = Join-Path $configPackage '.env'
    $configHash = (Get-FileHash -LiteralPath $configPath -Algorithm SHA256).Hash
    $generated = @{}
    foreach ($line in [IO.File]::ReadAllLines($configPath)) {
      $parts = $line.Split('=', 2)
      if ($parts.Length -ne 2 -or $generated.ContainsKey($parts[0])) { throw 'Generated dotenv structure is invalid' }
      $generated[$parts[0]] = $parts[1]
    }
    if ($generated.Count -ne 8) { throw 'Generated configuration has unexpected settings' }
    $newKey = $generated['AX_SECRET_SESSION_KEY']
    $newSetup = $generated['AX_SECRET_SETUP_TOKEN']
    if ($newKey -eq $saved['AX_SECRET_SESSION_KEY'] -or $newSetup -eq $saved['AX_SECRET_SETUP_TOKEN'] -or $newSetup -eq $newKey) { throw 'Recovery secrets must be independent' }
    # The first recovered startup must read the real generated .env through the
    # runtime parser, not a test-only reconstruction of its secret settings.
    foreach ($name in $saved.Keys) { Remove-Item -LiteralPath "Env:$name" -ErrorAction SilentlyContinue }
    Start-Fixture -WorkingDirectory $configPackage
    foreach ($name in $generated.Keys) { [Environment]::SetEnvironmentVariable($name, $generated[$name]) }
    Invoke-Request -Path '/setup' -Status 403 | Out-Null
    Invoke-Request -Path '/admin' -Status 403 | Out-Null
    Invoke-Request -Path '/admin' -Headers @{ Cookie = $cookie } -Status 403 | Out-Null
    Invoke-Request -Path '/admin/media' -Headers @{ Cookie = $cookie } -Status 403 | Out-Null
    Assert-PrivateMediaRead $mediaId $cookie 403
    Send-Media -Bytes $validImage -Headers $sessionProof -Status 403 | Out-Null
    Assert-RecoveredMedia $mediaExpectedRows

    $anonymous = Get-Proof
    $encodedEmail = [Uri]::EscapeDataString($Email)
    $encodedPassword = [Uri]::EscapeDataString($Password)
    $login = Invoke-Request -Path '/__axonyx/action?path=%2Flogin&name=SignIn' -Method POST -Body "email=$encodedEmail&password=$encodedPassword" -Headers $anonymous -Status 303
    $recoveryCookie = ([string]$login.Headers['Set-Cookie']).Split(';')[0]
    if (!$recoveryCookie) { throw 'Recovered credentials did not establish a fresh session' }
    Invoke-Request -Path '/admin' -Headers @{ Cookie = $recoveryCookie } | Out-Null
    Invoke-Request -Path '/admin/media' -Headers @{ Cookie = $recoveryCookie } | Out-Null
    Assert-PrivateMediaRead $mediaId $recoveryCookie
    $freshProof = Get-Proof -Cookie $recoveryCookie
    Send-Media -Bytes $validImage -Headers $freshProof -Status 200 | Out-Null
    Assert-RecoveredMedia ($mediaExpectedRows + 1)
    Stop-Fixture
    Start-Fixture
    Invoke-Request -Path '/admin/media' -Headers @{ Cookie = $recoveryCookie } | Out-Null
    Assert-RecoveredMedia ($mediaExpectedRows + 1)

    # The shipped adapter must deny even an authorized upload when disabled,
    # rather than passing image bytes to the framework's raw fallback store.
    Stop-Fixture
    $env:AX_SECRET_MEDIA_DRIVER = 'disabled'
    Remove-Item -LiteralPath 'Env:AX_SECRET_MEDIA_ROOT' -ErrorAction SilentlyContinue
    Remove-Item -LiteralPath 'Env:AX_SECRET_MEDIA_STORE_ID' -ErrorAction SilentlyContinue
    Start-Fixture
    $disabled = Invoke-Request -Path '/admin' -Headers @{ Cookie = $recoveryCookie }
    if ($disabled.Body.Contains('href="/admin/media"')) { throw 'Disabled recovered Media remained in navigation' }
    Invoke-Request -Path '/admin/media' -Headers @{ Cookie = $recoveryCookie } -Status 403 | Out-Null
    Assert-PrivateMediaRead $mediaId $recoveryCookie 403
    Send-Media -Bytes $validImage -Headers (Get-Proof -Cookie $recoveryCookie) -Status 403 | Out-Null
    Assert-RecoveredMedia ($mediaExpectedRows + 1)
    Stop-Fixture
    $env:AX_SECRET_MEDIA_DRIVER = 'local'
    $env:AX_SECRET_MEDIA_STORE_ID = $saved['AX_SECRET_MEDIA_STORE_ID']

    # A bad storage configuration must exit before becoming ready. Rollback
    # below restores only this proof's environment and its own server process.
    Stop-Fixture
    $env:AX_SECRET_MEDIA_ROOT = Join-Path $recoveryRoot 'missing-storage-must-not-be-created'
    $rejected = $false
    $exited = $false
    try { Start-Fixture }
    catch {
      $rejected = $true
      $exited = $script:process -and $script:process.HasExited
    }
    finally { Stop-Fixture }
    if (!$rejected -or !$exited) { throw 'Invalid recovered storage did not fail closed at startup' }
    if (Test-Path -LiteralPath $env:AX_SECRET_MEDIA_ROOT) { throw 'Failed startup created a missing storage directory' }
  } finally {
    Stop-Fixture
    if ($null -ne $sourceFingerprint) { $sourceUnchanged = (Get-SourceFingerprint) -ceq $sourceFingerprint }
    foreach ($name in $saved.Keys) {
      if ($null -eq $saved[$name]) { Remove-Item -LiteralPath "Env:$name" -ErrorAction SilentlyContinue }
      else { [Environment]::SetEnvironmentVariable($name, $saved[$name]) }
    }
    Start-Fixture
  }

  if (!$sourceUnchanged) { throw 'Recovery activation altered the stopped source database/sidecars or objects' }
  if ((Get-FileHash -LiteralPath $archivePath -Algorithm SHA256).Hash -ne $archiveHash) { throw 'Recovery activation changed the original archive' }
  if ((Get-FileHash -LiteralPath (Join-Path $recoveryRoot 'recovery.json') -Algorithm SHA256).Hash -ne $receiptHash) { throw 'Activation proof rewrote the historical recovery receipt' }
  if ((Get-FileHash -LiteralPath $configPath -Algorithm SHA256).Hash -ne $configHash) { throw 'Generated private configuration changed during startup/recovery' }
  Invoke-Request -Path '/admin/media' -Headers @{ Cookie = $cookie } | Out-Null
  Invoke-Request -Path '/admin' -Headers @{ Cookie = $recoveryCookie } -Status 403 | Out-Null
  Assert-MediaCounts -Rows $mediaExpectedRows -Files $mediaExpectedFiles
  Write-Host 'Isolated recovery activation passed: offline gate, independent session key, fresh login, separate upload/restart, failed-start refusal and source rollback.'
}

Invoke-RecoveryActivationProof -Email $identity.email -Password $password
