param(
    [ValidateSet("staged", "head", "working", "range")]
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
    elseif ($Mode -eq "range") {
        $branch = (& git branch --show-current)
        if ($LASTEXITCODE -ne 0 -or $null -eq $branch -or [string]::IsNullOrWhiteSpace(($branch | Select-Object -First 1))) {
            Write-Host "No upstream branch configured; skipping governance range check."
            $changed = @()
        }
        else {
            $branchName = ($branch | Select-Object -First 1).Trim()
            $remoteRef = "refs/remotes/origin/$branchName"
            $remoteExists = (& git show-ref --verify --quiet $remoteRef)
            if ($LASTEXITCODE -ne 0) {
                Write-Host "No origin/$branchName branch found; skipping governance range check."
                $changed = @()
            }
            else {
                $changed = @(git -c core.quotePath=false diff --name-only "origin/$branchName..HEAD")
            }
        }
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
