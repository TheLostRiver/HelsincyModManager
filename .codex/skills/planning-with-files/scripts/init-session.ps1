#requires -Version 5.0
param(
    [string] $ProjectName = "project",
    [string] $Template = "default",
    [switch] $Legacy,
    [switch] $Force
)

$ErrorActionPreference = "Stop"
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$PlanPy = Join-Path $ScriptDir "plan.py"

if ($Template -ne "default") {
    Write-Output "[planning-with-files] Template '$Template' is not used by the unified init command; using default templates."
}

$PlanArgs = @("--root", (Get-Location).Path, "init", $ProjectName)
if ($Legacy) { $PlanArgs += "--legacy" }
if ($Force) { $PlanArgs += "--force" }

& python $PlanPy @PlanArgs
exit $LASTEXITCODE
