param(
    [string]$Scope = "verify"
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

. "$PSScriptRoot/lib/Policy.ps1"

$repoRoot = Get-RepoRoot
$policy = Read-ProjectPolicy -RepoRoot $repoRoot
$files = Select-PolicyIncludedFiles -Files (Get-GitCandidateFiles -RepoRoot $repoRoot) -ExcludePathPatterns (Get-PolicyCheckScopeExcludePathPatterns -Policy $policy -Scope $Scope)
$markdownFiles = @($files | Where-Object { [System.IO.Path]::GetExtension($_).ToLowerInvariant() -eq ".md" })
$errors = New-Object System.Collections.Generic.List[string]
$linkPattern = [regex]'\[[^\]]+\]\(([^)]+)\)'

foreach ($file in $markdownFiles) {
    $fullPath = Join-RepoPath -RepoRoot $repoRoot -RelativePath $file
    if (-not (Test-Path -LiteralPath $fullPath -PathType Leaf)) {
        continue
    }

    $content = Get-Content -LiteralPath $fullPath -Raw -Encoding UTF8
    $matches = $linkPattern.Matches($content)
    $baseDir = Split-Path -Path ($file -replace '\\', '/') -Parent

    foreach ($match in $matches) {
        $target = $match.Groups[1].Value.Trim()
        if ([string]::IsNullOrWhiteSpace($target)) {
            continue
        }

        $target = $target.Trim('<', '>')
        if ($target.StartsWith("#")) {
            continue
        }

        if ($target -match '^[a-zA-Z][a-zA-Z0-9+.-]*:') {
            continue
        }

        $targetWithoutAnchor = ($target -split '#', 2)[0]
        if ([string]::IsNullOrWhiteSpace($targetWithoutAnchor)) {
            continue
        }

        $decodedTarget = [uri]::UnescapeDataString($targetWithoutAnchor)
        if ([string]::IsNullOrWhiteSpace($baseDir)) {
            $candidate = $decodedTarget
        }
        else {
            $candidate = "$baseDir/$decodedTarget"
        }

        try {
            $normalizedCandidate = Normalize-RepoRelativePath -RelativePath $candidate
        }
        catch {
            $errors.Add("$file link escapes repository root: $target")
            continue
        }

        if (-not (Test-ExactPathCase -RepoRoot $repoRoot -RelativePath $normalizedCandidate)) {
            $errors.Add("$file contains invalid link: $target")
        }
    }
}

if ($errors.Count -gt 0) {
    Write-PolicyErrors -Title "Markdown link check failed:" -Errors $errors
    exit 1
}

Write-Host "Markdown link check passed."
