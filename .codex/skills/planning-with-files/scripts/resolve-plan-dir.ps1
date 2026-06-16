$ErrorActionPreference = "Stop"

# planning-with-files: deprecated compatibility resolver.
#
# The authoritative resolver lives in .codex/hooks/planning_state.py and is
# exposed through plan.py status. Keep this wrapper thin so session bindings,
# PLAN_ID precedence, and workspace fallback do not drift across implementations.

$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$rootDir = Resolve-Path (Join-Path $scriptDir "..\..\..\..")
$planCli = Join-Path $rootDir ".codex\skills\planning-with-files\scripts\plan.py"

$previousLang = $env:PWF_LANG
$env:PWF_LANG = ""
try {
    $output = & python $planCli --root $rootDir status 2>$null
    foreach ($line in $output) {
        if ($line -like "path: *") {
            $line.Substring(6)
            break
        }
    }
}
finally {
    $env:PWF_LANG = $previousLang
}
