param(
    [ValidateSet("working", "staged")]
    [string]$Mode = "working",
    [string]$Scope = "verify"
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

. "$PSScriptRoot/lib/Policy.ps1"

$repoRoot = Get-RepoRoot
$policy = Read-ProjectPolicy -RepoRoot $repoRoot
$excludePathPatterns = Get-PolicyCheckScopeExcludePathPatterns -Policy $policy -Scope $Scope

$pathspecs = @("--", ".")
foreach ($pattern in $excludePathPatterns) {
    $pathspecs += ":(exclude)$pattern"
}

Push-Location $repoRoot
try {
    if ($Mode -eq "staged") {
        & git diff --check --cached @pathspecs
    }
    else {
        & git diff --check @pathspecs
    }

    if ($LASTEXITCODE -ne 0) {
        exit $LASTEXITCODE
    }
}
finally {
    Pop-Location
}

Write-Host "Whitespace check passed."