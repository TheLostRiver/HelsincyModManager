#requires -Version 5.0
param(
    [string] $PlanId = ""
)

$ErrorActionPreference = "Stop"
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$PlanPy = Join-Path $ScriptDir "plan.py"

$PlanArgs = @("--root", (Get-Location).Path, "switch")
if ($PlanId -ne "") { $PlanArgs += $PlanId }

& python $PlanPy @PlanArgs
exit $LASTEXITCODE
