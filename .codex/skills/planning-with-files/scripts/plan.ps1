#requires -Version 5.0
$ErrorActionPreference = "Stop"

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$PlanPy = Join-Path $ScriptDir "plan.py"

& python $PlanPy @args
exit $LASTEXITCODE
