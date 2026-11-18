# Dot-sourced only by the isolated Users fixture; never targets an installed CMS.
$changePasswordPath = "/__axonyx/action?path=%2Fadmin%2Fprofile&name=ChangeProfilePassword"
$replacementPassword = "profile-fixture-" + [guid]::NewGuid().ToString("N")
$profileForm = "currentPassword=$password&password=$replacementPassword&confirmPassword=$replacementPassword"
$profile = Invoke-Request -Path "/admin/profile" -Headers @{ Cookie = $cookie }
if ($profile.Body -notmatch 'name=ChangeProfilePassword' -or
    $profile.Body -notmatch 'autocomplete="current-password"' -or
    $profile.Body -notmatch 'autocomplete="new-password"') {
  throw "Profile password form is missing or lost password-manager semantics"
}
Invoke-Request -Path $changePasswordPath -Method POST -Body $profileForm -Headers (Get-Proof) -Status 403 | Out-Null
Invoke-Request -Path $changePasswordPath -Method POST -Body $profileForm -Headers @{ Cookie = $cookie; Origin = $baseUrl } -Status 403 | Out-Null
$activeEditorCookie = ([string]$fresh.Headers["Set-Cookie"]).Split(';')[0]
Invoke-Request -Path $changePasswordPath -Method POST -Body $profileForm -Headers (Get-Proof -Cookie $activeEditorCookie) -Status 403 | Out-Null
foreach ($invalidPassword in @(
  @{ Body = $profileForm.Replace("currentPassword=$password", "currentPassword=incorrect"); Field = "currentPassword" },
  @{ Body = $profileForm.Replace("password=$replacementPassword", "password=short"); Field = "password" },
  @{ Body = $profileForm.Replace("confirmPassword=$replacementPassword", "confirmPassword=different"); Field = "confirmPassword" }
)) {
  $failure = Invoke-Request -Path $changePasswordPath -Method POST -Body $invalidPassword.Body -Headers $sessionProof -Status 422
  if ($failure.Body -notmatch ('data-ax-field-error="' + $invalidPassword.Field + '"[^>]*>\s*\S') -or
      $failure.Body.Contains($password) -or $failure.Body.Contains($replacementPassword) -or
      $failure.Headers["Set-Cookie"]) {
    throw "Profile password validation lost field feedback, leaked a secret or changed a session"
  }
  Invoke-Request -Path "/admin" -Headers @{ Cookie = $cookie } | Out-Null
}
$ownerEmail = [Uri]::EscapeDataString($identity.email)
$secondLogin = Invoke-Request -Path "/__axonyx/action?path=%2Flogin&name=SignIn" -Method POST -Body "email=$ownerEmail&password=$password" -Headers (Get-Proof) -Status 303
$secondCookie = ([string]$secondLogin.Headers["Set-Cookie"]).Split(';')[0]
$changedPassword = Invoke-Request -Path $changePasswordPath -Method POST -Body "$profileForm&administrator_id=$editorId" -Headers $sessionProof -Status 303
if ($changedPassword.Headers.Location -ne "/login") { throw "Password change must require a fresh login" }
foreach ($revokedCookie in @($cookie, $secondCookie)) {
  Invoke-Request -Path "/admin" -Headers @{ Cookie = $revokedCookie } -Status 403 | Out-Null
}
Invoke-Request -Path "/admin/posts" -Headers @{ Cookie = $activeEditorCookie } | Out-Null
$oldPasswordLogin = Invoke-Request -Path "/__axonyx/action?path=%2Flogin&name=SignIn" -Method POST -Body "email=$ownerEmail&password=$password" -Headers (Get-Proof) -Status 422
if ($oldPasswordLogin.Body -notmatch "Email or password is incorrect" -or $oldPasswordLogin.Headers["Set-Cookie"]) {
  throw "The old password still works or did not return safe feedback"
}
$newLogin = Invoke-Request -Path "/__axonyx/action?path=%2Flogin&name=SignIn" -Method POST -Body "email=$ownerEmail&password=$replacementPassword" -Headers (Get-Proof) -Status 303
$cookie = ([string]$newLogin.Headers["Set-Cookie"]).Split(';')[0]
$password = $replacementPassword
Invoke-Request -Path "/admin" -Headers @{ Cookie = $cookie } | Out-Null
# Reset only fixture-local admission counters before the existing throttle proof.
Stop-Fixture
Start-Fixture
Invoke-Request -Path "/admin" -Headers @{ Cookie = $cookie } | Out-Null
$sessionProof = Get-Proof -Cookie $cookie
Write-Host "Legura Profile password smoke passed: reauthentication, validation, no secret echoes, all owner sessions revoked, independent Editor preserved and restart login."
