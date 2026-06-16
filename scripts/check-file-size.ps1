param(
    [string]$Scope = "verify"
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

. "$PSScriptRoot/lib/Policy.ps1"

$repoRoot = Get-RepoRoot
$policy = Read-ProjectPolicy -RepoRoot $repoRoot
$files = Get-GitCandidateFiles -RepoRoot $repoRoot
$errors = New-Object System.Collections.Generic.List[string]
$allowlist = @($policy.fileSize.allowlist)
$fileSizeExcludePathPatterns = @()
if ($null -ne $policy.fileSize.PSObject.Properties["excludePathPatterns"]) {
    $fileSizeExcludePathPatterns += @($policy.fileSize.excludePathPatterns)
}
$fileSizeExcludePathPatterns += @(Get-PolicyCheckScopeExcludePathPatterns -Policy $policy -Scope $Scope)
$excludePathRegexes = @(Convert-PolicyGlobListToRegexes -Patterns $fileSizeExcludePathPatterns)

function Test-ExcludedPath {
    param([string]$Path)

    return Test-PolicyPathExcluded -Path $Path -Regexes $excludePathRegexes
}

function Get-PolicyCategory {
    param([string]$Path)

    $extension = [System.IO.Path]::GetExtension($Path).ToLowerInvariant()
    foreach ($property in $policy.fileSize.extensions.PSObject.Properties) {
        foreach ($candidate in @($property.Value)) {
            if ($extension -eq $candidate.ToLowerInvariant()) {
                return $property.Name
            }
        }
    }

    return $null
}

foreach ($file in $files) {
    $normalized = $file -replace '\\', '/'
    if (Test-ExcludedPath -Path $normalized) {
        continue
    }

    if ($allowlist -contains $normalized) {
        continue
    }

    $category = Get-PolicyCategory -Path $normalized
    if ($null -eq $category) {
        continue
    }

    $limitProperty = $policy.fileSize.block.PSObject.Properties[$category]
    if ($null -eq $limitProperty) {
        continue
    }

    $fullPath = Join-RepoPath -RepoRoot $repoRoot -RelativePath $normalized
    if (-not (Test-Path -LiteralPath $fullPath -PathType Leaf)) {
        continue
    }

    $lineCount = 0
    foreach ($line in [System.IO.File]::ReadLines($fullPath)) {
        $lineCount++
    }

    if ($lineCount -gt [int]$limitProperty.Value) {
        $errors.Add("$normalized exceeds hard line limit: $lineCount / $($limitProperty.Value)")
    }
}

if ($errors.Count -gt 0) {
    Write-PolicyErrors -Title "File size check failed:" -Errors $errors
    exit 1
}

Write-Host "File size check passed."
