$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$PSNativeCommandUseErrorActionPreference = $false
$repoRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot "..")).Path
$scenarioPackage = "hmm-runtime"
$scenarioFilter = "headless_composition_"
$expectedScenarios = @(
    "composition::core_mod_lifecycle_tests::headless_composition_imports_v1_and_rebuilds_plan_after_restart",
    "composition::core_mod_lifecycle_tests::headless_composition_installs_restarts_uninstalls_and_restores_baseline",
    "composition::core_mod_lifecycle_tests::headless_composition_reinstalls_v1_to_v2_and_restores_baseline",
    "composition::core_mod_lifecycle_tests::headless_composition_retargets_staging_commits_and_persists_binding_snapshot",
    "composition::core_mod_lifecycle_tests::headless_composition_switches_retarget_with_true_reinstall_and_uninstalls_to_baseline",
    "composition::core_mod_lifecycle_tests::headless_composition_rolls_back_v1_when_reinstall_manifest_save_fails"
)

function Assert-CommandAvailable {
    param(
        [Parameter(Mandatory = $true)]
        [string] $Name
    )

    if ($null -eq (Get-Command $Name -ErrorAction SilentlyContinue)) {
        Write-Host "Required command is missing: $Name" -ForegroundColor Red
        exit 1
    }
}

function Invoke-NativeCapture {
    param(
        [Parameter(Mandatory = $true)]
        [string] $FilePath,

        [Parameter(Mandatory = $true)]
        [string[]] $Arguments
    )

    $output = @(& $FilePath @Arguments 2>&1)
    $exitCode = $LASTEXITCODE

    [PSCustomObject]@{
        Output = @($output)
        ExitCode = $exitCode
    }
}

function ConvertTo-SafeOutputLine {
    param(
        [Parameter(Mandatory = $true)]
        [object] $Value
    )

    $line = $Value.ToString()
    $privateRoots = @(
        $repoRoot,
        [Environment]::GetFolderPath([Environment+SpecialFolder]::UserProfile)
    )

    foreach ($root in $privateRoots) {
        if (-not [string]::IsNullOrWhiteSpace($root)) {
            $line = [Regex]::Replace(
                $line,
                [Regex]::Escape($root),
                "<redacted-root>",
                [Text.RegularExpressions.RegexOptions]::IgnoreCase
            )
        }
    }

    return $line
}

function Write-SafeOutput {
    param(
        [Parameter(Mandatory = $true)]
        [object[]] $Output
    )

    foreach ($item in $Output) {
        Write-Host (ConvertTo-SafeOutputLine -Value $item)
    }
}

function Stop-Acceptance {
    param(
        [Parameter(Mandatory = $true)]
        [string] $Message,

        [int] $ExitCode = 1
    )

    Write-Host $Message -ForegroundColor Red
    exit $ExitCode
}

$env:CARGO_TERM_COLOR = "never"
$env:CARGO_TERM_QUIET = "true"

Assert-CommandAvailable -Name "cargo"

Push-Location $repoRoot
try {
    if ($env:OS -eq "Windows_NT") {
        Assert-CommandAvailable -Name "node"
        Write-Host "Preparing the Windows development sidecar..."
        $sidecar = Invoke-NativeCapture -FilePath "node" -Arguments @(
            "scripts/prepare-save-backup-worker-sidecar.mjs",
            "--debug"
        )
        if ($sidecar.ExitCode -ne 0) {
            Write-SafeOutput -Output $sidecar.Output
            Stop-Acceptance -Message "Development sidecar preparation failed." -ExitCode $sidecar.ExitCode
        }
    }

    Write-Host "Discovering core Mod lifecycle scenarios..."
    $discovery = Invoke-NativeCapture -FilePath "cargo" -Arguments @(
        "test",
        "--quiet",
        "-p",
        $scenarioPackage,
        $scenarioFilter,
        "--",
        "--list"
    )
    if ($discovery.ExitCode -ne 0) {
        Write-SafeOutput -Output $discovery.Output
        Stop-Acceptance -Message "Core Mod lifecycle scenario discovery failed." -ExitCode $discovery.ExitCode
    }

    $scenarioPattern = "^(?<name>composition::core_mod_lifecycle_tests::headless_composition_[A-Za-z0-9_]+): test$"
    $discoveredScenarios = @(@(
        foreach ($item in $discovery.Output) {
            $line = $item.ToString().Trim()
            if ($line -match $scenarioPattern) {
                $Matches["name"]
            }
        }
    ) | Sort-Object -Unique)

    if ($discoveredScenarios.Count -eq 0) {
        Stop-Acceptance -Message "No core Mod lifecycle scenarios were discovered. Refusing a green 0-test result."
    }
    if ($discoveredScenarios.Count -lt $expectedScenarios.Count) {
        Stop-Acceptance -Message (
            "Discovered {0} core Mod lifecycle scenarios; expected at least {1}." -f
                $discoveredScenarios.Count,
                $expectedScenarios.Count
        )
    }

    $missingScenarios = @(
        $expectedScenarios | Where-Object { $discoveredScenarios -notcontains $_ }
    )
    if ($missingScenarios.Count -gt 0) {
        Stop-Acceptance -Message (
            "Required core Mod lifecycle scenarios are missing: {0}" -f
                ($missingScenarios -join ", ")
        )
    }

    Write-Host ("Discovered {0} core Mod lifecycle scenarios:" -f $discoveredScenarios.Count)
    foreach ($scenario in $discoveredScenarios) {
        Write-Host "  $scenario"
    }

    Write-Host "Executing all core Mod lifecycle scenarios once..."
    $execution = Invoke-NativeCapture -FilePath "cargo" -Arguments @(
        "test",
        "--quiet",
        "-p",
        $scenarioPackage,
        $scenarioFilter
    )
    Write-SafeOutput -Output $execution.Output
    if ($execution.ExitCode -ne 0) {
        Stop-Acceptance -Message "Core Mod lifecycle scenario execution failed." -ExitCode $execution.ExitCode
    }

    $summaries = @(
        foreach ($item in $execution.Output) {
            $line = $item.ToString().Trim()
            if ($line -match "^test result: ok\. (?<passed>[0-9]+) passed; 0 failed; (?<ignored>[0-9]+) ignored;") {
                [PSCustomObject]@{
                    Passed = [int] $Matches["passed"]
                    Ignored = [int] $Matches["ignored"]
                }
            }
        }
    )
    if ($summaries.Count -eq 0) {
        Stop-Acceptance -Message "Cargo succeeded without a parseable core Mod lifecycle test summary."
    }

    $executedCount = ($summaries | Measure-Object -Property Passed -Maximum).Maximum
    $ignoredCount = ($summaries | Measure-Object -Property Ignored -Sum).Sum
    if ($ignoredCount -ne 0) {
        Stop-Acceptance -Message (
            "Core Mod lifecycle acceptance cannot skip scenarios; Cargo reported {0} ignored." -f
                $ignoredCount
        )
    }
    if ($executedCount -lt $discoveredScenarios.Count) {
        Stop-Acceptance -Message (
            "Discovered {0} scenarios but Cargo reported only {1} passed." -f
                $discoveredScenarios.Count,
                $executedCount
        )
    }

    Write-Host ("Core Mod lifecycle acceptance passed: {0} scenarios executed." -f $executedCount) -ForegroundColor Green
}
finally {
    Pop-Location
}
