param(
    [string]$Scope = "verify"
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

. "$PSScriptRoot/lib/Policy.ps1"

$repoRoot = Get-RepoRoot
$policy = Read-ProjectPolicy -RepoRoot $repoRoot
$files = Select-PolicyIncludedFiles -Files (Get-GitCandidateFiles -RepoRoot $repoRoot) -ExcludePathPatterns (Get-PolicyCheckScopeExcludePathPatterns -Policy $policy -Scope $Scope)
$errors = New-Object System.Collections.Generic.List[string]
$textExtensions = @(".md", ".txt", ".json", ".toml", ".yml", ".yaml", ".ps1", ".psm1", ".rs", ".ts", ".tsx", ".js", ".jsx", ".css", ".html")

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
