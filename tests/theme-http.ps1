$themeAction = '/__axonyx/action?path=%2Fadmin%2Fappearance&name=SelectPublicTheme'
$modeAction = '/__axonyx/action?path=%2Fadmin%2Fappearance&name=SetPublicColorMode'

function Assert-PublicTheme {
  param([string]$Expected, [string]$Mode = 'light')
  foreach ($path in @('/', '/posts')) {
    $response = Invoke-Request -Path $path
    if (!$response.Body.Contains('data-legura-public-theme="' + $Expected + '"')) {
      throw "Public theme $Expected was not rendered at $path"
    }
    if ($response.Body -notmatch ('<div(?=[^>]*data-legura-public-theme="' + $Expected + '")[^>]*data-foundry-mode="' + $Mode + '"')) {
      throw "Public color mode $Mode was not rendered at $path"
    }
  }
}

Assert-PublicTheme -Expected 'default'
Invoke-Request -Path '/admin/appearance' -Status 403 | Out-Null
Invoke-Request -Path $themeAction -Method POST -Body 'theme=editorial' -Headers (Get-Proof) -Status 403 | Out-Null
Invoke-Request -Path $modeAction -Method POST -Body 'color_mode=dark' -Headers (Get-Proof) -Status 403 | Out-Null
$appearance = Invoke-Request -Path '/admin/appearance' -Headers @{ Cookie = $cookie }
if (!$appearance.Body.Contains('Public theme') -or !$appearance.Body.Contains('Default') -or !$appearance.Body.Contains('Editorial')) {
  throw 'Admin theme selector did not render both installed themes'
}
Invoke-Request -Path $themeAction -Method POST -Body 'theme=editorial' -Headers @{ Cookie = $cookie; Origin = $baseUrl } -Status 403 | Out-Null
Invoke-Request -Path $themeAction -Method POST -Body 'theme=unknown' -Headers $sessionProof -Status 422 | Out-Null
Invoke-Request -Path $modeAction -Method POST -Body 'color_mode=dark' -Headers @{ Cookie = $cookie; Origin = $baseUrl } -Status 403 | Out-Null
Invoke-Request -Path $modeAction -Method POST -Body 'color_mode=system' -Headers $sessionProof -Status 422 | Out-Null
Invoke-Request -Path $modeAction -Method POST -Body 'color_mode=dark' -Headers $sessionProof -Status 303 | Out-Null
Assert-PublicTheme -Expected 'default' -Mode 'dark'
Invoke-Request -Path $themeAction -Method POST -Body 'theme=editorial' -Headers $sessionProof -Status 303 | Out-Null
Assert-PublicTheme -Expected 'editorial' -Mode 'dark'
$editorial = Invoke-Request -Path '/posts'
if (!$editorial.Body.Contains('legura-editorial-nav') -or !$editorial.Body.Contains('data-foundry="bronze"')) {
  throw 'Editorial layout and palette did not render together'
}
if ($Posts) {
  & $python.Source -c 'import sqlite3,sys;d=sqlite3.connect(sys.argv[1]);d.execute("UPDATE legura_posts SET status = ? WHERE slug = ?", ("published", "public-story"));d.commit();d.close()' $db
  if ($LASTEXITCODE -ne 0) { throw 'Cannot prepare the isolated published story' }
  $story = Invoke-Request -Path '/posts/public-story'
  if ($story.Body -notmatch '<div(?=[^>]*data-legura-public-theme="editorial")[^>]*data-foundry-mode="dark"') {
    throw 'Published story did not use the selected public appearance'
  }
}
$missing = Invoke-Request -Path '/posts/unknown' -Status 404
if (!$missing.Body.Contains('data-legura-public-theme="default"')) {
  throw 'Not-found boundary did not keep its safe default public frame'
}
$appearance = Invoke-Request -Path '/admin/appearance' -Headers @{ Cookie = $cookie }
if (!$appearance.Body.Contains('Active theme') -or !$appearance.Body.Contains('legura-admin-shell') -or
    $appearance.Body -notmatch 'class="legura-app"[^>]*data-foundry-mode="light"' -or
    $appearance.Body -notmatch '<option(?=[^>]*\sselected(?:\s|=|>))[^>]*value="dark"') {
  throw 'Admin theme selection was not reflected without replacing the admin shell'
}
Stop-Fixture
Start-Fixture
Assert-PublicTheme -Expected 'editorial' -Mode 'dark'
$sessionProof = Get-Proof -Cookie $cookie
Invoke-Request -Path $modeAction -Method POST -Body 'color_mode=light' -Headers $sessionProof -Status 303 | Out-Null
Assert-PublicTheme -Expected 'editorial'
Invoke-Request -Path $themeAction -Method POST -Body 'theme=default' -Headers $sessionProof -Status 303 | Out-Null
Assert-PublicTheme -Expected 'default'
Write-Host 'Legura public appearance HTTP passed: authorization, CSRF, allowlists, independent layout and mode, and restart persistence.'
