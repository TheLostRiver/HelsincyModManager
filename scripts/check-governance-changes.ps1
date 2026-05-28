param(
    [ValidateSet("staged", "head", "working")]
    [string]$Mode = "staged"
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

. "$PSScriptRoot/lib/Policy.ps1"

$repoRoot = Get-RepoRoot
$policy = Read-ProjectPolicy -RepoRoot $repoRoot
$patterns = @($policy.governanceFiles | ForEach-Object { Convert-PolicyGlobToRegex -Pattern $_ })

Push-Location $repoRoot
try {
    if ($Mode -eq "staged") {
        $changed = @(git -c core.quotePath=false diff --cached --name-only)
    }
    elseif ($Mode -eq "head") {
        $changed = @(git -c core.quotePath=false diff --name-only HEAD)
    }
    else {
        $changed = @(git -c core.quotePath=false status --short | ForEach-Object {
            $line = $_
            if ($line.Length -ge 4) {
                $line.Substring(3)
            }
        })
    }

    $governanceMatches = New-Object System.Collections.Generic.List[string]
    foreach ($file in $changed) {
        if ([string]::IsNullOrWhiteSpace($file)) {
            continue
        }

        $normalized = $file -replace '\\', '/'
        foreach ($regex in $patterns) {
            if ($normalized -match $regex) {
                $governanceMatches.Add($normalized)
                break
            }
        }
    }

    if ($governanceMatches.Count -gt 0) {
        Write-Host "Governance files changed. Human review is recommended:" -ForegroundColor Yellow
        foreach ($item in ($governanceMatches | Sort-Object -Unique)) {
            Write-Host "  - $item" -ForegroundColor Yellow
        }
    }
    else {
        Write-Host "No governance file changes detected."
    }
}
finally {
    Pop-Location
}
