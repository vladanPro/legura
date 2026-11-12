$themeAction = '/__axonyx/action?path=%2Fadmin%2Fappearance&name=SelectPublicTheme'
$modeAction = '/__axonyx/action?path=%2Fadmin%2Fappearance&name=SetPublicColorMode'
$paletteAction = '/__axonyx/action?path=%2Fadmin%2Fappearance&name=SetPublicPalette'

function Assert-PublicTheme {
  param([string]$Expected, [string]$Mode = 'light', [string]$Palette = '')
  if (!$Palette) { $Palette = if ($Expected -eq 'editorial') { 'bronze' } else { 'silver' } }
  foreach ($path in @('/', '/posts')) {
    $response = Invoke-Request -Path $path
    if (!$response.Body.Contains('data-legura-public-theme="' + $Expected + '"')) {
      throw "Public theme $Expected was not rendered at $path"
    }
    $frame = '<div(?=[^>]*data-legura-public-theme="' + $Expected + '")(?=[^>]*data-foundry="' + $Palette + '")[^>]*data-foundry-mode="' + $Mode + '"'
    if ($response.Body -notmatch $frame) {
      throw "Public color mode $Mode and palette $Palette were not rendered together at $path"
    }
  }
}

Assert-PublicTheme -Expected 'default'
Invoke-Request -Path '/admin/appearance' -Status 403 | Out-Null
Invoke-Request -Path $themeAction -Method POST -Body 'theme=editorial' -Headers (Get-Proof) -Status 403 | Out-Null
Invoke-Request -Path $modeAction -Method POST -Body 'color_mode=dark' -Headers (Get-Proof) -Status 403 | Out-Null
Invoke-Request -Path $paletteAction -Method POST -Body 'palette=gold' -Headers (Get-Proof) -Status 403 | Out-Null
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
Invoke-Request -Path $paletteAction -Method POST -Body 'palette=gold' -Headers @{ Cookie = $cookie; Origin = $baseUrl } -Status 403 | Out-Null
Invoke-Request -Path $paletteAction -Method POST -Body 'palette=unknown' -Headers $sessionProof -Status 422 | Out-Null
Invoke-Request -Path $paletteAction -Method POST -Body 'palette=gold' -Headers $sessionProof -Status 303 | Out-Null
Assert-PublicTheme -Expected 'editorial' -Mode 'dark' -Palette 'gold'
if ($Posts) {
  & $python.Source -c 'import sqlite3,sys;d=sqlite3.connect(sys.argv[1]);d.execute("UPDATE legura_posts SET status = ? WHERE slug = ?", ("published", "public-story"));d.commit();d.close()' $db
  if ($LASTEXITCODE -ne 0) { throw 'Cannot prepare the isolated published story' }
  $story = Invoke-Request -Path '/posts/public-story'
  if ($story.Body -notmatch '<div(?=[^>]*data-legura-public-theme="editorial")(?=[^>]*data-foundry="gold")[^>]*data-foundry-mode="dark"') {
    throw 'Published story did not use the selected public appearance'
  }
}
$missing = Invoke-Request -Path '/posts/unknown' -Status 404
if (!$missing.Body.Contains('data-legura-public-theme="default"')) {
  throw 'Not-found boundary did not keep its safe default public frame'
}
$appearance = Invoke-Request -Path '/admin/appearance' -Headers @{ Cookie = $cookie }
if (!$appearance.Body.Contains('Active theme') -or !$appearance.Body.Contains('legura-admin-shell') -or
    $appearance.Body -notmatch 'class="legura-app"[^>]*data-foundry="silver"[^>]*data-foundry-mode="light"' -or
    $appearance.Body -notmatch '<option(?=[^>]*\sselected(?:\s|=|>))[^>]*value="dark"' -or
    $appearance.Body -notmatch '<option(?=[^>]*\sselected(?:\s|=|>))[^>]*value="gold"') {
  throw 'Admin theme selection was not reflected without replacing the admin shell'
}
Stop-Fixture
Start-Fixture
Assert-PublicTheme -Expected 'editorial' -Mode 'dark' -Palette 'gold'
$sessionProof = Get-Proof -Cookie $cookie
Invoke-Request -Path $modeAction -Method POST -Body 'color_mode=light' -Headers $sessionProof -Status 303 | Out-Null
Assert-PublicTheme -Expected 'editorial' -Palette 'gold'
Invoke-Request -Path $themeAction -Method POST -Body 'theme=default' -Headers $sessionProof -Status 303 | Out-Null
Assert-PublicTheme -Expected 'default' -Palette 'gold'
Invoke-Request -Path $paletteAction -Method POST -Body 'palette=theme' -Headers $sessionProof -Status 303 | Out-Null
Assert-PublicTheme -Expected 'default'
Write-Host 'Legura public appearance HTTP passed: authorization, CSRF, allowlists, independent layout, mode and palette, and restart persistence.'
