$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

. "$PSScriptRoot/lib/Policy.ps1"

$repoRoot = Get-RepoRoot
$policy = Read-ProjectPolicy -RepoRoot $repoRoot
$errors = New-Object System.Collections.Generic.List[string]

foreach ($file in @($policy.requiredFiles)) {
    if (-not (Test-ExactPathCase -RepoRoot $repoRoot -RelativePath $file)) {
        $errors.Add("Required file is missing or has wrong case: $file")
    }
}

foreach ($file in @($policy.caseSensitiveFiles)) {
    if (-not (Test-ExactPathCase -RepoRoot $repoRoot -RelativePath $file)) {
        $errors.Add("Case-sensitive file mismatch: $file")
    }
}

foreach ($script in @($policy.requiredScripts)) {
    if (-not (Test-ExactPathCase -RepoRoot $repoRoot -RelativePath $script)) {
        $errors.Add("Required script is missing or has wrong case: $script")
    }
}

$rootEntries = Get-ChildItem -LiteralPath $repoRoot -Force
$wrongAgentFiles = @($rootEntries | Where-Object { $_.Name -ieq "agents.md" -and $_.Name -cne "AGENTS.md" })
foreach ($file in $wrongAgentFiles) {
    $errors.Add("Agent guide must be named AGENTS.md; found: $($file.Name)")
}

if ($errors.Count -gt 0) {
    Write-PolicyErrors -Title "Policy check failed:" -Errors $errors
    exit 1
}

Write-Host "Policy check passed."
