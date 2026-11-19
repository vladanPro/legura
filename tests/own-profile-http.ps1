# Isolated fixture only: exercise self-service without granting user administration.
$selfServicePath = "/__axonyx/action?path=%2Fadmin%2Fprofile&name=ChangeProfilePassword"
$selfServiceLogin = "/__axonyx/action?path=%2Flogin&name=SignIn"
$selfPreviousCookie = $null
$ownerHashBefore = & $python.Source -c 'import sqlite3,sys;d=sqlite3.connect(sys.argv[1]);print(d.execute("SELECT password_hash FROM legura_credentials WHERE user_id=?",(sys.argv[2],)).fetchone()[0]);d.close()' $db $ownerId
if ($LASTEXITCODE -ne 0) { throw "Cannot inspect fixture owner credential" }
foreach ($selfRole in @("editor", "subscriber")) {
  $selfEmail = "own-$selfRole-" + [guid]::NewGuid().ToString("N") + "@example.test"
  $selfEmailEncoded = [Uri]::EscapeDataString($selfEmail)
  $selfPassword = "own-profile-" + [guid]::NewGuid().ToString("N")
  $selfReplacement = "replacement-" + [guid]::NewGuid().ToString("N")
  $selfLanding = if ($selfRole -eq "editor") { "/admin/posts" } else { "/admin/profile" }
  Invoke-Request -Path "/__axonyx/action?path=%2Fadmin%2Fusers&name=CreateUser" -Method POST -Body "email=$selfEmailEncoded&password=$selfPassword&role=$selfRole" -Headers $sessionProof -Status 303 | Out-Null
  $selfLookup = Invoke-Request -Path "/admin/users?q=$selfEmailEncoded" -Headers @{ Cookie = $cookie }
  $selfDetails = [regex]::Match($selfLookup.Body, 'href="(/admin/users/[a-f0-9-]{36})"')
  if (!$selfDetails.Success) { throw "Cannot find fixture self-service account" }
  $selfId = Split-Path $selfDetails.Groups[1].Value -Leaf
  $selfCookies = @()
  foreach ($attempt in 1..2) {
    $selfLogin = Invoke-Request -Path $selfServiceLogin -Method POST -Body "email=$selfEmailEncoded&password=$selfPassword" -Headers (Get-Proof) -Status 303
    if ($selfLogin.Headers.Location -ne $selfLanding) { throw "Wrong self-service landing" }
    $selfCookies += ([string]$selfLogin.Headers["Set-Cookie"]).Split(';')[0]
  }
  $selfPage = Invoke-Request -Path "/admin/profile?id=$ownerId" -Headers @{ Cookie = $selfCookies[0] }
  if (!$selfPage.Body.Contains($selfEmail) -or $selfPage.Body.Contains($identity.email) -or
      $selfPage.Body -match 'password_hash|session_generation|argon2' -or
      $selfPage.Body -notmatch '<a\b(?=[^>]*href="/admin/profile")(?=[^>]*data-active="true")' -or
      $selfPage.Body -match 'href="/admin/(users|settings|language|media)"') {
    throw "Self profile lost selection, leaked data or offered forbidden navigation"
  }
  if ($selfRole -eq "subscriber" -and $selfPage.Body -match 'href="/admin/(posts|pages)"') {
    throw "Subscriber profile offered content navigation"
  }
  foreach ($path in @("/admin", "/admin/users", "/admin/settings", "/admin/language", "/admin/media", "/admin/users/$ownerId")) {
    Invoke-Request -Path $path -Headers @{ Cookie = $selfCookies[0] } -Status 403 | Out-Null
  }
  $selfProof = Get-Proof -Cookie $selfCookies[0]
  $selfForm = "currentPassword=$selfPassword&password=$selfReplacement&confirmPassword=$selfReplacement"
  Invoke-Request -Path $selfServicePath -Method POST -Body $selfForm -Headers @{ Cookie = $selfCookies[0]; Origin = $baseUrl } -Status 403 | Out-Null
  $selfWrong = Invoke-Request -Path $selfServicePath -Method POST -Body $selfForm.Replace("currentPassword=$selfPassword", "currentPassword=$password") -Headers $selfProof -Status 422
  if ($selfWrong.Body -notmatch 'Current password is incorrect' -or $selfWrong.Body.Contains($selfPassword) -or $selfWrong.Body.Contains($selfReplacement)) {
    throw "Self-service accepted another account's password or echoed credentials"
  }
  $selfChanged = Invoke-Request -Path $selfServicePath -Method POST -Body "$selfForm&id=$ownerId&account_id=$ownerId&administrator_id=$ownerId&role=admin" -Headers $selfProof -Status 303
  if ($selfChanged.Headers.Location -ne "/login") { throw "Self-service must require a fresh login" }
  foreach ($revoked in $selfCookies) {
    Invoke-Request -Path "/admin/profile" -Headers @{ Cookie = $revoked } -Status 403 | Out-Null
  }
  Invoke-Request -Path $selfServicePath -Method POST -Body $selfForm -Headers $selfProof -Status 403 | Out-Null
  Invoke-Request -Path $selfServiceLogin -Method POST -Body "email=$selfEmailEncoded&password=$selfPassword" -Headers (Get-Proof) -Status 422 | Out-Null
  $selfNewLogin = Invoke-Request -Path $selfServiceLogin -Method POST -Body "email=$selfEmailEncoded&password=$selfReplacement" -Headers (Get-Proof) -Status 303
  if ($selfNewLogin.Headers.Location -ne $selfLanding) { throw "Self-service changed the role or landing" }
  $selfNewCookie = ([string]$selfNewLogin.Headers["Set-Cookie"]).Split(';')[0]
  Invoke-Request -Path "/admin/profile" -Headers @{ Cookie = $selfNewCookie } | Out-Null
  Invoke-Request -Path "/admin" -Headers @{ Cookie = $cookie } | Out-Null
  Invoke-Request -Path "/admin/users" -Headers @{ Cookie = $selfNewCookie } -Status 403 | Out-Null
  $selfContentStatus = if ($selfRole -eq "editor") { 200 } else { 403 }
  Invoke-Request -Path "/admin/posts" -Headers @{ Cookie = $selfNewCookie } -Status $selfContentStatus | Out-Null
  if ($selfPreviousCookie) {
    Invoke-Request -Path "/admin/profile" -Headers @{ Cookie = $selfPreviousCookie } | Out-Null
  }
  $selfPreviousCookie = $selfNewCookie
  if ($selfRole -eq "subscriber") {
    $selfFreshProof = Get-Proof -Cookie $selfNewCookie
    $selfDeactivate = "/__axonyx/action?path=$([Uri]::EscapeDataString($selfDetails.Groups[1].Value))&name=UpdateUserStatus"
    Invoke-Request -Path $selfDeactivate -Method POST -Body "id=$selfId&status=inactive" -Headers $sessionProof -Status 303 | Out-Null
    Invoke-Request -Path "/admin/profile" -Headers @{ Cookie = $selfNewCookie } -Status 403 | Out-Null
    Invoke-Request -Path $selfServicePath -Method POST -Body $selfForm -Headers $selfFreshProof -Status 403 | Out-Null
  }
}
$ownerHashAfter = & $python.Source -c 'import sqlite3,sys;d=sqlite3.connect(sys.argv[1]);print(d.execute("SELECT password_hash FROM legura_credentials WHERE user_id=?",(sys.argv[2],)).fetchone()[0]);d.close()' $db $ownerId
if ($LASTEXITCODE -ne 0 -or $ownerHashBefore -cne $ownerHashAfter) { throw "Self-service changed the owner credential" }
Write-Host "Legura self-profile smoke passed: Editor/Subscriber own profiles, scoped navigation, password verification, forged IDs ignored, session revocation, unchanged owner and deactivated-account denial."
