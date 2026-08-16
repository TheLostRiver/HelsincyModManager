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
$forbiddenExtensions = @($policy.forbiddenFiles.extensions | ForEach-Object { $_.ToLowerInvariant() })
$pathRegexes = @($policy.forbiddenFiles.pathPatterns | ForEach-Object { Convert-PolicyGlobToRegex -Pattern $_ })
$allowedPathRegexes = @($policy.forbiddenFiles.allowedPathPatterns | ForEach-Object { Convert-PolicyGlobToRegex -Pattern $_ })

foreach ($file in $files) {
    $normalized = $file -replace '\\', '/'
    $extension = [System.IO.Path]::GetExtension($normalized).ToLowerInvariant()

    if ($forbiddenExtensions -contains $extension) {
        $errors.Add("Forbidden file type: $normalized")
        continue
    }

    if (@($allowedPathRegexes | Where-Object { $normalized -match $_ }).Count -gt 0) {
        continue
    }

    foreach ($regex in $pathRegexes) {
        if ($normalized -match $regex) {
            $errors.Add("Forbidden path: $normalized")
            break
        }
    }
}

if ($errors.Count -gt 0) {
    Write-PolicyErrors -Title "Forbidden files check failed:" -Errors $errors
    exit 1
}

Write-Host "Forbidden files check passed."
