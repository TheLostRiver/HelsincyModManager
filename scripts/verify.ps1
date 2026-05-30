$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$repoRoot = git rev-parse --show-toplevel 2>$null
if ($LASTEXITCODE -ne 0 -or [string]::IsNullOrWhiteSpace($repoRoot)) {
    Write-Host "Current directory is not inside a Git repository." -ForegroundColor Red
    exit 1
}

$repoRoot = $repoRoot.Trim()

function Invoke-Pnpm {
    param(
        [Parameter(Mandatory = $true)]
        [string[]] $Arguments
    )

    if ($env:OS -eq "Windows_NT") {
        & cmd /c corepack pnpm @Arguments
    }
    else {
        & corepack pnpm @Arguments
    }

    if ($LASTEXITCODE -ne 0) {
        exit $LASTEXITCODE
    }
}

function Assert-RequiredFile {
    param(
        [Parameter(Mandatory = $true)]
        [string] $RelativePath
    )

    $fullPath = Join-Path $repoRoot ($RelativePath -replace '/', [System.IO.Path]::DirectorySeparatorChar)
    if (-not (Test-Path -LiteralPath $fullPath -PathType Leaf)) {
        Write-Host "Required file is missing: $RelativePath" -ForegroundColor Red
        exit 1
    }
}

$checks = @(
    "scripts/check-policy.ps1",
    "scripts/check-file-size.ps1",
    "scripts/check-forbidden-files.ps1",
    "scripts/check-doc-links.ps1",
    "scripts/check-secrets.ps1"
)

Push-Location $repoRoot
try {
    Write-Host "Running Git whitespace check..."
    git diff --check
    if ($LASTEXITCODE -ne 0) {
        exit 1
    }

    foreach ($check in $checks) {
        Write-Host "Running $check ..."
        & (Join-Path $repoRoot ($check -replace '/', [System.IO.Path]::DirectorySeparatorChar))
        if ($LASTEXITCODE -ne 0) {
            exit $LASTEXITCODE
        }
    }

    if (Test-Path -LiteralPath (Join-Path $repoRoot "src-tauri/tauri.conf.json")) {
        Write-Host "Checking Tauri icon assets..."
        Assert-RequiredFile -RelativePath "src-tauri/icons/icon.ico"
        Assert-RequiredFile -RelativePath "src-tauri/icons/icon.png"
    }

    if (Test-Path -LiteralPath (Join-Path $repoRoot "package.json")) {
        if (-not (Test-Path -LiteralPath (Join-Path $repoRoot "node_modules"))) {
            Write-Host "node_modules is missing. Run: cmd /c corepack pnpm install --frozen-lockfile" -ForegroundColor Red
            exit 1
        }

        Write-Host "Running frontend typecheck..."
        Invoke-Pnpm -Arguments @("run", "typecheck")

        Write-Host "Running frontend lint..."
        Invoke-Pnpm -Arguments @("run", "lint")

        Write-Host "Running frontend build..."
        Invoke-Pnpm -Arguments @("run", "build")
    }
    else {
        Write-Host "Skipping frontend checks: package.json does not exist yet."
    }

    if (Test-Path -LiteralPath (Join-Path $repoRoot "Cargo.toml")) {
        Write-Host "Running Rust tests..."
        cargo test --workspace
        if ($LASTEXITCODE -ne 0) {
            exit $LASTEXITCODE
        }

        Write-Host "Running Rust check..."
        cargo check --workspace
        if ($LASTEXITCODE -ne 0) {
            exit $LASTEXITCODE
        }
    }
    else {
        Write-Host "Skipping Rust checks: Cargo.toml does not exist yet."
    }

    Write-Host "Verification passed."
}
finally {
    Pop-Location
}
