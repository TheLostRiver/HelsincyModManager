Set-StrictMode -Version Latest

function Get-RepoRoot {
    $root = git rev-parse --show-toplevel 2>$null
    if ($LASTEXITCODE -ne 0 -or [string]::IsNullOrWhiteSpace($root)) {
        throw "Current directory is not inside a Git repository."
    }

    return $root.Trim()
}

function Join-RepoPath {
    param(
        [Parameter(Mandatory = $true)][string]$RepoRoot,
        [Parameter(Mandatory = $true)][string]$RelativePath
    )

    $normalized = $RelativePath -replace '/', [System.IO.Path]::DirectorySeparatorChar
    return Join-Path $RepoRoot $normalized
}

function Normalize-RepoRelativePath {
    param(
        [Parameter(Mandatory = $true)][string]$RelativePath
    )

    $segments = New-Object System.Collections.Generic.List[string]
    foreach ($part in ($RelativePath -split '[\\/]')) {
        if ([string]::IsNullOrWhiteSpace($part) -or $part -eq '.') {
            continue
        }

        if ($part -eq '..') {
            if ($segments.Count -eq 0) {
                throw "Path escapes repository root: $RelativePath"
            }

            $segments.RemoveAt($segments.Count - 1)
            continue
        }

        $segments.Add($part)
    }

    return ($segments -join '/')
}

function Read-ProjectPolicy {
    param(
        [Parameter(Mandatory = $true)][string]$RepoRoot
    )

    $policyPath = Join-RepoPath -RepoRoot $RepoRoot -RelativePath "policy/project-policy.json"
    if (-not (Test-Path -LiteralPath $policyPath -PathType Leaf)) {
        throw "Missing policy file: policy/project-policy.json"
    }

    return Get-Content -LiteralPath $policyPath -Raw -Encoding UTF8 | ConvertFrom-Json
}

function Get-GitCandidateFiles {
    param(
        [Parameter(Mandatory = $true)][string]$RepoRoot
    )

    Push-Location $RepoRoot
    try {
        $files = git -c core.quotePath=false ls-files --cached --others --exclude-standard
        if ($LASTEXITCODE -ne 0) {
            throw "Unable to read Git file list."
        }

        return @($files | Where-Object { -not [string]::IsNullOrWhiteSpace($_) })
    }
    finally {
        Pop-Location
    }
}

function Test-ExactPathCase {
    param(
        [Parameter(Mandatory = $true)][string]$RepoRoot,
        [Parameter(Mandatory = $true)][string]$RelativePath
    )

    $parts = (Normalize-RepoRelativePath -RelativePath $RelativePath) -split '/'
    $current = $RepoRoot

    foreach ($part in $parts) {
        $match = Get-ChildItem -LiteralPath $current -Force | Where-Object { $_.Name -ceq $part } | Select-Object -First 1
        if ($null -eq $match) {
            return $false
        }

        $current = $match.FullName
    }

    return $true
}

function Convert-PolicyGlobToRegex {
    param(
        [Parameter(Mandatory = $true)][string]$Pattern
    )

    $normalized = $Pattern -replace '\\', '/'
    $escaped = [regex]::Escape($normalized)
    $escaped = $escaped -replace '\\\*\\\*', '.*'
    $escaped = $escaped -replace '\\\*', '[^/]*'
    return "^$escaped$"
}

function Write-PolicyErrors {
    param(
        [Parameter(Mandatory = $true)][string]$Title,
        [Parameter(Mandatory = $true)][System.Collections.IEnumerable]$Errors
    )

    Write-Host $Title -ForegroundColor Red
    foreach ($item in $Errors) {
        Write-Host "  - $item" -ForegroundColor Red
    }
}
