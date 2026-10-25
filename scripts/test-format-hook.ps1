param([Parameter(Mandatory = $true)][string]$FormatterPath)
$ErrorActionPreference = 'Stop'
$formatter = (Resolve-Path -LiteralPath $FormatterPath).Path
$hook = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '../.githooks')).Path
$temporary = [IO.Path]::GetFullPath([IO.Path]::GetTempPath())
$fixture = Join-Path $temporary ('axonyx-hook-proof-' + [Guid]::NewGuid().ToString('N'))
$null = New-Item -ItemType Directory -Path $fixture
$file = Join-Path $fixture 'with space.asx'
$bad = "page Test() {`nreturn ASX {`n<Card><Copy>Hello</Copy></Card>`n}`n}`n"

function Invoke-Git {
    & git @args
    if ($LASTEXITCODE -ne 0) { throw "Git failed: $args" }
}

function Assert-Rejected {
    $index = Invoke-Git write-tree
    $hash = (Get-FileHash -LiteralPath $file).Hash
    & git commit -m 'Must reject unformatted staged content'
    if ($LASTEXITCODE -eq 0) { throw 'Hook accepted unformatted staged content' }
    if ((Invoke-Git write-tree) -ne $index) { throw 'Hook changed the index' }
    if ((Get-FileHash -LiteralPath $file).Hash -ne $hash) { throw 'Hook changed the working file' }
}

Push-Location $fixture
try {
    Invoke-Git init --quiet
    Invoke-Git config user.name 'Formatter Proof'
    Invoke-Git config user.email 'formatter@example.invalid'
    Invoke-Git config core.hooksPath $hook
    Invoke-Git config axonyx.formatterPath $formatter
    [IO.File]::WriteAllText($file, $bad)
    Invoke-Git add -- 'with space.asx'
    Assert-Rejected
    & $formatter fmt --file $file
    if ($LASTEXITCODE -ne 0) { throw 'Formatter failed' }
    Assert-Rejected
    Invoke-Git add -- 'with space.asx'
    [IO.File]::WriteAllText($file, $bad.Replace('Hello', 'Unstaged work'))
    $hash = (Get-FileHash -LiteralPath $file).Hash
    Invoke-Git commit -m 'Formatted staged version only'
    if ((Get-FileHash -LiteralPath $file).Hash -ne $hash) { throw 'Hook rewrote unstaged work' }
    if ((Invoke-Git show 'HEAD:with space.asx') -match 'Unstaged work') { throw 'Hook staged unrelated work' }
    Write-Host 'PASS: rejection, staged-vs-working separation, spaced paths and unchanged index/files'
} finally {
    Pop-Location
    $resolved = [IO.Path]::GetFullPath($fixture)
    if ($resolved.StartsWith($temporary, [StringComparison]::OrdinalIgnoreCase) -and
        [IO.Path]::GetFileName($resolved).StartsWith('axonyx-hook-proof-')) {
        Remove-Item -LiteralPath $resolved -Recurse -Force
    }
}
