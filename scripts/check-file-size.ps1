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
$warnings = New-Object System.Collections.Generic.List[string]
$allowlist = @($policy.fileSize.allowlist)
$byteLimit = $null
if ($null -ne $policy.fileSize.PSObject.Properties["blockBytes"]) {
    $byteLimit = [long]$policy.fileSize.blockBytes
}
$maxLineLength = $null
$maxLineLengthProperty = $policy.fileSize.PSObject.Properties["maxLineLength"]
if ($null -ne $maxLineLengthProperty -and $null -ne $maxLineLengthProperty.Value) {
    $maxLineLength = [int]$maxLineLengthProperty.Value
}
$maxLineLengthExcludePathPatterns = @()
if ($null -ne $policy.fileSize.PSObject.Properties["maxLineLengthExcludePathPatterns"]) {
    $maxLineLengthExcludePathPatterns = @($policy.fileSize.maxLineLengthExcludePathPatterns)
}
$maxLineLengthExcludePathRegexes = @(Convert-PolicyGlobListToRegexes -Patterns $maxLineLengthExcludePathPatterns)
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

    $fullPath = Join-RepoPath -RepoRoot $repoRoot -RelativePath $normalized
    if (-not (Test-Path -LiteralPath $fullPath -PathType Leaf)) {
        continue
    }

    $fileInfo = Get-Item -LiteralPath $fullPath
    if ($null -ne $byteLimit -and $fileInfo.Length -gt $byteLimit) {
        $errors.Add("$normalized exceeds hard byte limit: $($fileInfo.Length) / $byteLimit")
    }

    $category = Get-PolicyCategory -Path $normalized
    if ($null -eq $category) {
        continue
    }

    $limitProperty = $policy.fileSize.block.PSObject.Properties[$category]
    $reviewLimitProperty = $null
    if ($null -ne $policy.fileSize.PSObject.Properties["review"]) {
        $reviewLimitProperty = $policy.fileSize.review.PSObject.Properties[$category]
    }
    $checkMaxLineLength = $null -ne $maxLineLength -and -not (
        Test-PolicyPathExcluded -Path $normalized -Regexes $maxLineLengthExcludePathRegexes
    )
    if (
        $null -eq $limitProperty -and
        $null -eq $reviewLimitProperty -and
        -not $checkMaxLineLength
    ) {
        continue
    }

    $lineCount = 0
    $longestLineLength = 0
    $longestLineNumber = 0
    foreach ($line in [System.IO.File]::ReadLines($fullPath)) {
        $lineCount++
        if ($line.Length -gt $longestLineLength) {
            $longestLineLength = $line.Length
            $longestLineNumber = $lineCount
        }
    }

    if ($null -ne $limitProperty -and $lineCount -gt [int]$limitProperty.Value) {
        $errors.Add("$normalized exceeds hard line limit: $lineCount / $($limitProperty.Value)")
    } elseif ($null -ne $reviewLimitProperty -and $lineCount -gt [int]$reviewLimitProperty.Value) {
        $warnings.Add("$normalized exceeds review line threshold: $lineCount / $($reviewLimitProperty.Value)")
    }

    if ($checkMaxLineLength -and $longestLineLength -gt $maxLineLength) {
        $errors.Add("$normalized exceeds hard line length: $longestLineLength at line $longestLineNumber / $maxLineLength")
    }
}

if ($warnings.Count -gt 0) {
    Write-Host "File size review warnings:"
    foreach ($warning in $warnings) {
        Write-Host "  - $warning"
    }
}

if ($errors.Count -gt 0) {
    Write-PolicyErrors -Title "File size check failed:" -Errors $errors
    exit 1
}

Write-Host "File size check passed."
