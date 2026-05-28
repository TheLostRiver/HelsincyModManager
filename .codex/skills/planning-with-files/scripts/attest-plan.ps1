#requires -Version 5.0
param(
    [Parameter(ParameterSetName = "Show")]
    [switch] $Show,

    [Parameter(ParameterSetName = "Clear")]
    [switch] $Clear
)

$ErrorActionPreference = "Stop"
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$PlanPy = Join-Path $ScriptDir "plan.py"

$PlanArgs = @("--root", (Get-Location).Path, "attest")
if ($Show) { $PlanArgs += "--show" }
if ($Clear) { $PlanArgs += "--clear" }

& python $PlanPy @PlanArgs
exit $LASTEXITCODE
