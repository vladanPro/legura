# Dot-sourced by the isolated compiled fixture after administrator setup.
$createUserPath = "/__axonyx/action?path=%2Fadmin%2Fusers&name=CreateUser"
$newUserPassword = "account-fixture-" + [guid]::NewGuid().ToString("N")
$editorForm = "email=editor-account%40example.com&role=editor&password=$newUserPassword"
$usersList = Invoke-Request -Path "/admin/users" -Headers @{ Cookie = $cookie }
if ($usersList.Body -notmatch 'href="#add-user"' -or $usersList.Body -notmatch 'id="add-user"') { throw "Users list is missing the create section" }
$createUserMarkup = [regex]::Match($usersList.Body, '<form\b(?=[^>]*name=CreateUser)[^>]*>[\s\S]*?</form>').Value
if ($createUserMarkup -notmatch 'autocomplete="new-password"' -or $createUserMarkup -match '<option[^>]+value="admin"') {
  throw "Create user form lost password semantics or offered an administrator role"
}
Invoke-Request -Path "/admin/users" -Status 403 | Out-Null
Invoke-Request -Path $createUserPath -Method POST -Body $editorForm -Headers (Get-Proof) -Status 403 | Out-Null
Invoke-Request -Path $createUserPath -Method POST -Body $editorForm -Headers @{ Cookie = $cookie; Origin = $baseUrl } -Status 403 | Out-Null

$invalidUsers = @(
  @{ Body = $editorForm.Replace("role=editor", "role=admin"); Field = "role"; Message = "Choose Editor or Subscriber." },
  @{ Body = $editorForm.Replace($newUserPassword, "too-short"); Field = "password"; Message = "Use at least 10 characters." },
  @{ Body = $editorForm.Replace("editor-account%40example.com", "malformed"); Field = "email"; Message = "Enter a valid email address." }
)
foreach ($invalidUser in $invalidUsers) {
  $failure = Invoke-Request -Path $createUserPath -Method POST -Body $invalidUser.Body -Headers $sessionProof -Status 422
  if ($failure.Body -notmatch ('data-ax-field-error="' + $invalidUser.Field + '"[^>]*>\s*' + [regex]::Escape($invalidUser.Message)) -or
      $failure.Body.Contains($newUserPassword) -or $failure.Body.Contains("too-short") -or $failure.Headers["Set-Cookie"]) {
    throw "Create user field error was missing, leaked a password or changed the session"
  }
}

$editor = Invoke-Request -Path $createUserPath -Method POST -Body $editorForm -Headers $sessionProof -Status 303
if ($editor.Headers.Location -ne "/admin/users" -or $editor.Headers["Set-Cookie"]) {
  throw "Create user did not return to Users or changed the administrator session"
}
$updatedUsers = Invoke-Request -Path "/admin/users" -Headers @{ Cookie = $cookie }
$editorRow = [regex]::Matches($updatedUsers.Body, '<tr\b[^>]*>[\s\S]*?</tr>') |
  Where-Object { $_.Value.Contains("editor-account@example.com") } |
  Select-Object -First 1
if (!$editorRow) { throw "Users did not list the created Editor" }
$editorLink = [regex]::Match($editorRow.Value, 'href="(/admin/users/[a-f0-9-]{36})"')
if (!$editorLink.Success) { throw "Users did not list the created Editor" }
$editorDetailsPath = $editorLink.Groups[1].Value
$details = Invoke-Request -Path $editorDetailsPath -Headers @{ Cookie = $cookie }
if (!$details.Body.Contains("editor-account@example.com") -or $details.Body.Contains($newUserPassword) -or
    $details.Body -notmatch 'name=UpdateUserRole') {
  throw "Created account details were not safe or complete"
}
$duplicate = Invoke-Request -Path $createUserPath -Method POST -Body $editorForm.Replace("editor-account", "EDITOR-ACCOUNT") -Headers $sessionProof -Status 422
if (!$duplicate.Body.Contains("This email address is already in use.") -or $duplicate.Body.Contains($newUserPassword)) {
  throw "Case-insensitive duplicate email did not return safe field feedback"
}

$subscriberForm = $editorForm.Replace("editor-account", "subscriber-account").Replace("role=editor", "role=subscriber")
Invoke-Request -Path $createUserPath -Method POST -Body $subscriberForm -Headers $sessionProof -Status 303 | Out-Null
foreach ($account in @(
  @{ Email = "editor-account"; Role = "editor"; Landing = "/admin/posts"; Content = 200 },
  @{ Email = "subscriber-account"; Role = "subscriber"; Landing = "/"; Content = 403 }
)) {
  $login = Invoke-Request -Path "/__axonyx/action?path=%2Flogin&name=SignIn" -Method POST -Body "email=$($account.Email)%40example.com&password=$newUserPassword" -Headers (Get-Proof) -Status 303
  if ($login.Headers.Location -ne $account.Landing) { throw "Wrong landing for $($account.Role)" }
  $memberCookie = ([string]$login.Headers["Set-Cookie"]).Split(';')[0]
  if ($account.Role -eq "editor") { $editorCookie = $memberCookie }
  foreach ($path in "/admin/posts", "/admin/pages", "/admin/posts/new", "/admin/pages/new") {
    Invoke-Request -Path $path -Headers @{ Cookie = $memberCookie } -Status $account.Content | Out-Null
  }
  foreach ($path in "/admin/users", $editorDetailsPath) {
    Invoke-Request -Path $path -Headers @{ Cookie = $memberCookie } -Status 403 | Out-Null
  }
  $memberProof = Get-Proof -Cookie $memberCookie
  Invoke-Request -Path $createUserPath -Method POST -Body $editorForm -Headers $memberProof -Status 403 | Out-Null
  # Invalid content is field-rejected only after passing the content capability check.
  $writeStatus = if ($account.Role -eq "editor") { 422 } else { 403 }
  Invoke-Request -Path "/__axonyx/action?path=%2Fadmin%2Fposts%2Fnew&name=CreatePost" -Method POST -Body "title=&slug=account-test&body=Fixture&status=draft" -Headers $memberProof -Status $writeStatus | Out-Null
  Invoke-Request -Path "/__axonyx/action?path=%2Fadmin%2Fpages%2Fnew&name=CreatePage" -Method POST -Body "title=&slug=account-test&body=Fixture" -Headers $memberProof -Status $writeStatus | Out-Null
}

$editorId = Split-Path $editorDetailsPath -Leaf
$roleAction = "/__axonyx/action?path=$([Uri]::EscapeDataString($editorDetailsPath))&name=UpdateUserRole"
$roleForm = "id=$editorId&role=subscriber"
$editorProof = Get-Proof -Cookie $editorCookie
Invoke-Request -Path $roleAction -Method POST -Body $roleForm -Headers $editorProof -Status 403 | Out-Null
Invoke-Request -Path $roleAction -Method POST -Body $roleForm -Headers @{ Cookie = $cookie; Origin = $baseUrl } -Status 403 | Out-Null
$badRole = Invoke-Request -Path $roleAction -Method POST -Body "id=$editorId&role=admin" -Headers $sessionProof -Status 422
if ($badRole.Body -notmatch 'data-ax-field-error="role"[^>]*>\s*Choose Editor or Subscriber') {
  throw "Invalid role did not preserve field-level feedback"
}

$ownerId = & $python.Source -c 'import sqlite3,sys;d=sqlite3.connect(sys.argv[1]);print(d.execute("SELECT admin_id FROM legura_installation WHERE id=1").fetchone()[0]);d.close()' $db
if ($LASTEXITCODE -ne 0 -or !$ownerId) { throw "Cannot find fixture owner" }
$ownerPath = "/admin/users/$ownerId"
$ownerDetails = Invoke-Request -Path $ownerPath -Headers @{ Cookie = $cookie }
if ($ownerDetails.Body -match 'name=UpdateUserRole') { throw "Administrator was offered a role change form" }
$ownerAction = "/__axonyx/action?path=$([Uri]::EscapeDataString($ownerPath))&name=UpdateUserRole"
Invoke-Request -Path $ownerAction -Method POST -Body "id=$ownerId&role=subscriber" -Headers $sessionProof -Status 403 | Out-Null
Invoke-Request -Path $roleAction -Method POST -Body "id=$ownerId&role=subscriber" -Headers $sessionProof -Status 403 | Out-Null
Invoke-Request -Path $roleAction -Method POST -Body "id=missing&role=subscriber" -Headers $sessionProof -Status 404 | Out-Null

$changed = Invoke-Request -Path $roleAction -Method POST -Body $roleForm -Headers $sessionProof -Status 303
if ($changed.Headers.Location -ne $editorDetailsPath) { throw "Role change did not return to the account" }
foreach ($path in "/admin/posts", "/admin/pages") {
  Invoke-Request -Path $path -Headers @{ Cookie = $editorCookie } -Status 403 | Out-Null
}
Invoke-Request -Path "/__axonyx/action?path=%2Fadmin%2Fposts%2Fnew&name=CreatePost" -Method POST -Body "title=&slug=account-test&body=Fixture&status=draft" -Headers $editorProof -Status 403 | Out-Null
Invoke-Request -Path $roleAction -Method POST -Body "id=$editorId&role=editor" -Headers $sessionProof -Status 303 | Out-Null
Invoke-Request -Path "/admin/posts" -Headers @{ Cookie = $editorCookie } | Out-Null
Invoke-Request -Path "/admin" -Headers @{ Cookie = $cookie } | Out-Null

$accountRows = & $python.Source -c 'import sqlite3,sys;d=sqlite3.connect(sys.argv[1]);print(d.execute("SELECT COUNT(*) FROM legura_users u JOIN legura_credentials c ON c.user_id=u.id AND c.email=u.email WHERE u.email IN (?,?) AND c.password_hash LIKE ?", ("editor-account@example.com","subscriber-account@example.com","$argon2id$%")).fetchone()[0]);d.close()' $db
if ($LASTEXITCODE -ne 0 -or $accountRows -ne "2") { throw "New accounts must have matching Argon2id credentials" }

& $python.Source -c 'import sqlite3,sys;d=sqlite3.connect(sys.argv[1]);rows=[(f"filters-{i}",f"qa_user{i:02d}@example.com","editor" if i<=12 else "subscriber") for i in range(1,14)];d.executemany("INSERT INTO legura_users(id,email,role) VALUES (?,?,?)",rows);d.executemany("INSERT INTO legura_credentials(user_id,email,password_hash) SELECT ?,?,password_hash FROM legura_credentials WHERE email=?",[(id,email,"editor-account@example.com") for id,email,_ in rows]);d.commit();d.close()' $db
if ($LASTEXITCODE -ne 0) { throw "Cannot seed accounts for filtered pagination" }
$managerHeaders = @{ Cookie = $cookie }
$allEditors = Invoke-Request -Path "/admin/users?role=editor" -Headers $managerHeaders
if (!$allEditors.Body.Contains("13 matching accounts") -or $allEditors.Body.Contains("subscriber-account@example.com")) {
  throw "Role filter did not isolate Editor accounts"
}
$firstPage = Invoke-Request -Path "/admin/users?q=%20QA_%20&role=editor" -Headers $managerHeaders
if (!$firstPage.Body.Contains("12 matching accounts") -or !$firstPage.Body.Contains("qa_user10@example.com") -or
    $firstPage.Body.Contains("qa_user11@example.com") -or
    $firstPage.Body -notmatch '<input(?=[^>]*\bname="q")(?=[^>]*\bvalue="QA_")(?=[^>]*\btype="hidden")' -or
    $firstPage.Body -notmatch '<input(?=[^>]*\bname="role")(?=[^>]*\bvalue="editor")(?=[^>]*\btype="hidden")' -or
    $firstPage.Body -notmatch '<input(?=[^>]*\bname="page")(?=[^>]*\bvalue="2")(?=[^>]*\btype="hidden")') {
  throw "First page did not apply filters before pagination or preserve the query"
}
$lastPage = Invoke-Request -Path "/admin/users?q=QA_&role=editor&page=9999" -Headers $managerHeaders
if (!$lastPage.Body.Contains("Page 2") -or !$lastPage.Body.Contains("qa_user11@example.com") -or
    !$lastPage.Body.Contains("qa_user12@example.com") -or $lastPage.Body.Contains("qa_user13@example.com") -or
    $lastPage.Body.Contains("Next page")) {
  throw "Filtered page was not clamped to the final matching page"
}
$emptyUsers = Invoke-Request -Path "/admin/users?q=%25" -Headers $managerHeaders
if (!$emptyUsers.Body.Contains("No matching accounts") -or $emptyUsers.Body.Contains("qa_user01@example.com")) {
  throw "A literal percent sign must not act as an email wildcard"
}
Invoke-Request -Path "/admin/users?role=editor&q=qa_" -Headers @{ Cookie = $editorCookie } -Status 403 | Out-Null
Write-Host "Legura users smoke passed: guarded creation, validation, hashed credentials, login, role changes, search and filtered pagination."
