Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"
$ProgressPreference = "SilentlyContinue"
$WarningPreference = "SilentlyContinue"
$VerbosePreference = "SilentlyContinue"
$DebugPreference = "SilentlyContinue"
$InformationPreference = "SilentlyContinue"
[Console]::OutputEncoding = New-Object System.Text.UTF8Encoding($false)

function Write-Result([hashtable]$Value) {
    $Value.schemaVersion = 1
    [Console]::Out.WriteLine(($Value | ConvertTo-Json -Compress -Depth 6))
    exit 0
}

function Get-TaskOrStatus([string]$TaskName) {
    try {
        return Get-ScheduledTask -TaskPath "\" -TaskName $TaskName -ErrorAction Stop
    } catch {
        Write-TaskLookupFailure $_
    }
}

function Test-PermissionFailure($ErrorRecord) {
    $category = [string]$ErrorRecord.CategoryInfo.Category
    return $category -eq "PermissionDenied" -or $category -eq "SecurityError"
}

function Write-TaskLookupFailure($ErrorRecord) {
    $category = [string]$ErrorRecord.CategoryInfo.Category
    $errorId = [string]$ErrorRecord.FullyQualifiedErrorId
    if ($category -eq "ObjectNotFound" -and $errorId.StartsWith("CmdletizationQuery_NotFound")) {
        Write-Result @{ status = "not_found" }
    }
    if (Test-PermissionFailure $ErrorRecord) {
        Write-Result @{ status = "permission_required" }
    }
    Write-Result @{ status = "operation_failed" }
}

function Write-OperationFailure($ErrorRecord) {
    if (Test-PermissionFailure $ErrorRecord) {
        Write-Result @{ status = "permission_required" }
    }
    Write-Result @{ status = "operation_failed" }
}

function Resolve-Sid([string]$Identity) {
    if ([string]::IsNullOrWhiteSpace($Identity)) { return "" }
    if ($Identity -match "^S-[0-9-]+$") { return $Identity }
    return (New-Object System.Security.Principal.NTAccount($Identity)).Translate(
        [System.Security.Principal.SecurityIdentifier]
    ).Value
}

function Write-TaskFoundResult($Task) {
    $actions = @($Task.Actions)
    $triggers = @($Task.Triggers)
    $logon = @($triggers | Where-Object { $_.CimClass.CimClassName -eq "MSFT_TaskLogonTrigger" })
    $time = @($triggers | Where-Object { $_.CimClass.CimClassName -eq "MSFT_TaskTimeTrigger" })
    $action = if ($actions.Count -eq 1) { $actions[0] } else { $null }
    Write-Result @{ status = "found"; task = @{
        taskPath = [string]$Task.TaskPath
        ownerMarker = [string]$Task.Description
        userSid = Resolve-Sid ([string]$Task.Principal.UserId)
        actionCount = $actions.Count
        actionExecute = if ($null -eq $action) { "" } else { [string]$action.Execute }
        actionArguments = if ($null -eq $action) { "" } else { [string]$action.Arguments }
        actionWorkingDirectory = if ($null -eq $action) { "" } else { [string]$action.WorkingDirectory }
        logonTriggerCount = $logon.Count
        timeTriggerCount = $time.Count
        logonTriggerUserSid = if ($logon.Count -eq 1) { Resolve-Sid ([string]$logon[0].UserId) } else { "" }
        logonTriggerEnabled = if ($logon.Count -eq 1) { [bool]$logon[0].Enabled } else { $false }
        timeTriggerEnabled = if ($time.Count -eq 1) { [bool]$time[0].Enabled } else { $false }
        logonDelay = if ($logon.Count -eq 1) { [string]$logon[0].Delay } else { "" }
        periodicInterval = if ($time.Count -eq 1) { [string]$time[0].Repetition.Interval } else { "" }
        periodicDuration = if ($time.Count -eq 1) { [string]$time[0].Repetition.Duration } else { "" }
        logonType = [string]$Task.Principal.LogonType
        runLevel = [string]$Task.Principal.RunLevel
        multipleInstances = [string]$Task.Settings.MultipleInstances
        startWhenAvailable = [bool]$Task.Settings.StartWhenAvailable
        allowStartOnBatteries = -not [bool]$Task.Settings.DisallowStartIfOnBatteries
        dontStopOnBatteries = -not [bool]$Task.Settings.StopIfGoingOnBatteries
        wakeToRun = [bool]$Task.Settings.WakeToRun
        runOnlyIfNetworkAvailable = [bool]$Task.Settings.RunOnlyIfNetworkAvailable
        executionTimeLimit = [string]$Task.Settings.ExecutionTimeLimit
        enabled = [string]$Task.State -ne "Disabled"
        state = [string]$Task.State
    }}
}

function Assert-InstallerCleanupTaskIsQuiescent($Task, [string]$OwnerMarker) {
    if ([string]$Task.Description -ne $OwnerMarker) {
        Write-Result @{ status = "ownership_conflict" }
    }
    $state = [string]$Task.State
    if ($state -eq "Running" -or $state -eq "Queued") {
        Write-Result @{ status = "task_busy" }
    }
    if ($state -ne "Ready" -and $state -ne "Disabled") {
        Write-Result @{ status = "state_unverified" }
    }
}

function Write-InstallerCleanupPostDeleteStatus([string]$TaskName, [string]$OwnerMarker) {
    try {
        $remaining = Get-ScheduledTask -TaskPath "\" -TaskName $TaskName -ErrorAction Stop
    } catch {
        $category = [string]$_.CategoryInfo.Category
        $errorId = [string]$_.FullyQualifiedErrorId
        if ($category -eq "ObjectNotFound" -and $errorId.StartsWith("CmdletizationQuery_NotFound")) {
            Write-Result @{ status = "completed" }
        }
        Write-OperationFailure $_
    }
    if ([string]$remaining.Description -eq $OwnerMarker) {
        Write-Result @{ status = "post_delete_owned" }
    }
    Write-Result @{ status = "post_delete_foreign" }
}

try {
    $operation = $env:HMM_OPERATION
    if ($operation -eq "identity") {
        $sid = [System.Security.Principal.WindowsIdentity]::GetCurrent().User.Value
        Write-Result @{ status = "identity"; currentUserSid = $sid }
    }

    $modulePath = $env:HMM_SCHEDULED_TASKS_MODULE
    if ([string]::IsNullOrWhiteSpace($modulePath) -or -not (Test-Path -LiteralPath $modulePath -PathType Leaf)) {
        Write-Result @{ status = "module_unavailable" }
    }
    Import-Module -Name $modulePath -Force -ErrorAction Stop

    $taskName = $env:HMM_TASK_NAME
    $ownerMarker = $env:HMM_OWNER_MARKER
    if ([string]::IsNullOrWhiteSpace($taskName) -or [string]::IsNullOrWhiteSpace($ownerMarker)) {
        Write-Result @{ status = "operation_failed" }
    }

    if ($operation -eq "inspect") {
        $task = Get-TaskOrStatus $taskName
        Write-TaskFoundResult $task
    }

    if ($operation -eq "register") {
        $workerPath = $env:HMM_WORKER_PATH
        $userSid = $env:HMM_USER_SID
        if ([string]::IsNullOrWhiteSpace($workerPath) -or [string]::IsNullOrWhiteSpace($userSid)) {
            Write-Result @{ status = "operation_failed" }
        }
        $action = New-ScheduledTaskAction -Execute $workerPath -Argument "--once"
        $logon = New-ScheduledTaskTrigger -AtLogOn -User $userSid
        $logon.Delay = "PT1M"
        $periodic = New-ScheduledTaskTrigger -Once -At (Get-Date).AddMinutes(1) -RepetitionInterval (New-TimeSpan -Minutes 15)
        $principal = New-ScheduledTaskPrincipal -UserId $userSid -LogonType Interactive -RunLevel Limited
        $settings = New-ScheduledTaskSettingsSet -StartWhenAvailable -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries -ExecutionTimeLimit (New-TimeSpan -Hours 1) -MultipleInstances IgnoreNew
        $current = $null
        try {
            $current = Get-ScheduledTask -TaskPath "\" -TaskName $taskName -ErrorAction Stop
        } catch {
            $category = [string]$_.CategoryInfo.Category
            $errorId = [string]$_.FullyQualifiedErrorId
            $missing = $category -eq "ObjectNotFound" -and $errorId.StartsWith("CmdletizationQuery_NotFound")
            if (-not $missing) { Write-OperationFailure $_ }
        }
        if ($null -ne $current -and [string]$current.Description -ne $ownerMarker) {
            Write-Result @{ status = "ownership_conflict" }
        }
        if ($null -eq $current) {
            Register-ScheduledTask -TaskPath "\" -TaskName $taskName -Action $action -Trigger @($logon, $periodic) -Settings $settings -Principal $principal -Description $ownerMarker | Out-Null
        } else {
            $updated = Set-ScheduledTask -TaskPath "\" -TaskName $taskName -Action $action -Trigger @($logon, $periodic) -Settings $settings -Principal $principal
            Enable-ScheduledTask -InputObject $updated | Out-Null
        }
        Write-TaskFoundResult (Get-TaskOrStatus $taskName)
    }

    if ($operation -eq "unregister") {
        $current = Get-TaskOrStatus $taskName
        if ([string]$current.Description -ne $ownerMarker) {
            Write-Result @{ status = "ownership_conflict" }
        }
        Unregister-ScheduledTask -InputObject $current -Confirm:$false -ErrorAction Stop
        Write-InstallerCleanupPostDeleteStatus $taskName $ownerMarker
    }

    if ($operation -eq "installer_cleanup") {
        $current = Get-TaskOrStatus $taskName
        Assert-InstallerCleanupTaskIsQuiescent $current $ownerMarker
        $current = Get-TaskOrStatus $taskName
        Assert-InstallerCleanupTaskIsQuiescent $current $ownerMarker
        Unregister-ScheduledTask -InputObject $current -Confirm:$false -ErrorAction Stop
        Write-InstallerCleanupPostDeleteStatus $taskName $ownerMarker
    }

    Write-Result @{ status = "operation_failed" }
} catch {
    Write-OperationFailure $_
}
