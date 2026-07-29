param(
    [string]$Scope = "verify"
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

. "$PSScriptRoot/lib/Policy.ps1"

$repoRoot = Get-RepoRoot
$policy = Read-ProjectPolicy -RepoRoot $repoRoot
$candidateFiles = Get-GitCandidateFiles -RepoRoot $repoRoot
$scopeFiles = Select-PolicyIncludedFiles -Files $candidateFiles -ExcludePathPatterns (Get-PolicyCheckScopeExcludePathPatterns -Policy $policy -Scope $Scope)
$forceIncludePathPatterns = @()
if ($null -ne $policy.PSObject.Properties["secretScan"] -and $null -ne $policy.secretScan.PSObject.Properties["forceIncludePathPatterns"]) {
    $forceIncludePathPatterns = @($policy.secretScan.forceIncludePathPatterns)
}
$forceIncludedFiles = Select-PolicyMatchingFiles -Files $candidateFiles -IncludePathPatterns $forceIncludePathPatterns
$files = Merge-PolicyFileLists -PrimaryFiles $scopeFiles -AdditionalFiles $forceIncludedFiles
$errors = New-Object System.Collections.Generic.List[string]
$textExtensions = @(".md", ".txt", ".json", ".toml", ".yml", ".yaml", ".ps1", ".psm1", ".py", ".sql", ".rs", ".ts", ".tsx", ".js", ".jsx", ".mjs", ".cjs", ".css", ".html", ".sh", ".bash", ".zsh")

foreach ($file in $files) {
    $normalized = $file -replace '\\', '/'
    $extension = [System.IO.Path]::GetExtension($normalized).ToLowerInvariant()
    if ($textExtensions -notcontains $extension) {
        continue
    }

    $fullPath = Join-RepoPath -RepoRoot $repoRoot -RelativePath $normalized
    if (-not (Test-Path -LiteralPath $fullPath -PathType Leaf)) {
        continue
    }

    $lineNumber = 0
    foreach ($line in [System.IO.File]::ReadLines($fullPath)) {
        $lineNumber++
        foreach ($pattern in @($policy.secretPatterns)) {
            if ($line -match $pattern.regex) {
                $errors.Add("${normalized}:$lineNumber matches secret pattern: $($pattern.name)")
            }
        }
    }
}

if ($errors.Count -gt 0) {
    Write-PolicyErrors -Title "Secret scan failed:" -Errors $errors
    exit 1
}

Write-Host "Secret scan passed."
