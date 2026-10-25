param([ValidateSet('install', 'check')][string] $Task = 'check')
$ErrorActionPreference = 'Stop'
$root = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
Push-Location $root
try {
    $report = & cargo run --locked --bin legura-editor-assets -- check $root --json | ConvertFrom-Json
    if ($LASTEXITCODE -ne 0) { throw 'Editor package generation check failed; no repair attempted' }
    $componentImport = '@/'+ $report.component.Substring(4).Replace('.asx', '')
    $fields = [IO.File]::ReadAllText((Join-Path $root 'app/components/PostFields.asx'))
    if (!$fields.Contains('"' + $componentImport + '"')) { throw 'Editor package and source import differ; explicit activation is required' }
    foreach ($page in @('app/admin/posts/new/page.asx', 'app/admin/posts/new/plain/page.asx', 'app/admin/posts/[id]/edit/page.asx')) {
        if (![IO.File]::ReadAllText((Join-Path $root $page)).Contains('href="' + $report.stylesheet + '"')) { throw 'Editor stylesheet binding differs from the pinned package' }
    }
    if (![IO.File]::ReadAllText((Join-Path $root 'public/content-editor/host.js')).Contains("from '$($report.module)'")) { throw 'Editor module binding differs from the pinned package' }
    if ($Task -eq 'install') {
        & cargo run --locked --bin legura-editor-assets -- install $root
        if ($LASTEXITCODE -ne 0) { throw 'Editor generation installation failed; no repair attempted' }
    } elseif (!$report.installed) { throw 'Editor generation is missing; run editor-assets.ps1 -Task install before building' }
} finally { Pop-Location }
