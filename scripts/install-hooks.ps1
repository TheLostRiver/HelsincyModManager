$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

. "$PSScriptRoot/lib/Policy.ps1"

$repoRoot = Get-RepoRoot
$hooksPath = Join-RepoPath -RepoRoot $repoRoot -RelativePath ".githooks"

if (-not (Test-Path -LiteralPath $hooksPath -PathType Container)) {
    throw "Missing .githooks directory."
}

Push-Location $repoRoot
try {
    git config core.hooksPath .githooks
    if ($LASTEXITCODE -ne 0) {
        exit $LASTEXITCODE
    }

    Write-Host "Git hooks installed. core.hooksPath=.githooks"
}
finally {
    Pop-Location
}
