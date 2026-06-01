$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$repoRoot = git rev-parse --show-toplevel 2>$null
if ($LASTEXITCODE -ne 0 -or [string]::IsNullOrWhiteSpace($repoRoot)) {
    Write-Host "Current directory is not inside a Git repository." -ForegroundColor Red
    exit 1
}

$repoRoot = $repoRoot.Trim()
$errors = New-Object System.Collections.Generic.List[string]

$packageJsonPath = Join-Path $repoRoot "package.json"
$srcRoot = Join-Path $repoRoot "src"
if (
    -not (Test-Path -LiteralPath $packageJsonPath -PathType Leaf) -or
    -not (Test-Path -LiteralPath $srcRoot -PathType Container)
) {
    Write-Host "Frontend boundary check skipped: frontend scaffold not found."
    exit 0
}

function Get-RepoRelativePath {
    param(
        [Parameter(Mandatory = $true)]
        [string] $FullName
    )

    $fullPath = [System.IO.Path]::GetFullPath($FullName)
    if ($fullPath.StartsWith($repoRoot, [System.StringComparison]::OrdinalIgnoreCase)) {
        return $fullPath.Substring($repoRoot.Length).TrimStart('\', '/') -replace '\\', '/'
    }

    return $fullPath -replace '\\', '/'
}

$dashboardRoot = Join-Path $repoRoot "src/features/dashboard"
if (Test-Path -LiteralPath $dashboardRoot -PathType Container) {
    $dashboardFiles = @(
        Get-ChildItem -LiteralPath $dashboardRoot -Recurse -File |
            Where-Object { $_.Name -like "*.ts" -or $_.Name -like "*.tsx" }
    )

    foreach ($file in $dashboardFiles) {
        $content = Get-Content -LiteralPath $file.FullName -Raw -Encoding UTF8
        if ($content -match "sidebarMode" -or $content -match "useSidebarMode") {
            $relative = Get-RepoRelativePath -FullName $file.FullName
            $errors.Add("Dashboard file must not read sidebar mode: $relative")
        }
    }

    $dashboardCssFiles = @(
        Get-ChildItem -LiteralPath $dashboardRoot -Recurse -File |
            Where-Object { $_.Name -like "*.css" }
    )

    foreach ($file in $dashboardCssFiles) {
        $content = Get-Content -LiteralPath $file.FullName -Raw -Encoding UTF8
        if ($content -match "\[data-sidebar-mode") {
            $relative = Get-RepoRelativePath -FullName $file.FullName
            $errors.Add("Dashboard CSS must not branch by sidebar mode: $relative")
        }
    }
}

$forbiddenDashboardFiles = @(
    "src/features/dashboard/FloatingDashboardPage.tsx",
    "src/features/dashboard/ClassicDashboardPage.tsx"
)

foreach ($relativePath in $forbiddenDashboardFiles) {
    $fullPath = Join-Path $repoRoot ($relativePath -replace '/', [System.IO.Path]::DirectorySeparatorChar)
    if (Test-Path -LiteralPath $fullPath -PathType Leaf) {
        $errors.Add("Do not duplicate dashboard page by sidebar mode: $relativePath")
    }
}

$navDefinitionFiles = @()
$navDefinitionFiles = @(
    Get-ChildItem -LiteralPath $srcRoot -Recurse -File |
        Where-Object { $_.Name -like "*navItems.ts" -or $_.Name -like "*NavItems.ts" }
)

if ($navDefinitionFiles.Count -ne 1) {
    $relativeFiles = @($navDefinitionFiles | ForEach-Object { Get-RepoRelativePath -FullName $_.FullName })
    $message = "Expected exactly one navItems file under src, found $($navDefinitionFiles.Count)."
    if ($relativeFiles.Count -gt 0) {
        $message += " Found: $($relativeFiles -join ', ')"
    }

    $errors.Add($message)
}

if ($errors.Count -gt 0) {
    Write-Host "Frontend boundary check failed:" -ForegroundColor Red
    foreach ($errorMessage in $errors) {
        Write-Host " - $errorMessage" -ForegroundColor Red
    }
    exit 1
}

Write-Host "Frontend boundary check passed."
