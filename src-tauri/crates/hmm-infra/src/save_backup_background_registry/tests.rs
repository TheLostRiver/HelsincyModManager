use super::task_spec::{ScheduledTaskReadback, ScheduledTaskSpec, ScheduledTaskSpecMatch};
use super::{ScheduledTaskCommand, ScheduledTaskCommandOutcome, ScheduledTaskCommandRunner};
use hmm_ports::SaveBackupBackgroundRegistryError;
use std::path::PathBuf;

#[cfg(windows)]
use super::powershell::{
    build_command, parse_script_output, system_powershell_runtime,
    PowerShellScheduledTaskCommandRunner, COMMAND_TIMEOUT, MAX_OUTPUT_BYTES, SCRIPT,
};

#[cfg(windows)]
use std::collections::BTreeMap;

#[test]
fn task_name_is_stable_per_sid_without_exposing_the_sid() {
    let path = std::env::temp_dir().join("hmm-save-backup-worker.exe");
    let first = ScheduledTaskSpec::new("S-1-5-21-100-200-300-400", path.clone()).expect("spec");
    let second = ScheduledTaskSpec::new("S-1-5-21-100-200-300-400", path).expect("spec");

    assert_eq!(first.task_name, second.task_name);
    assert!(first
        .task_name
        .starts_with("HelsincyModManager.SaveBackup."));
    assert!(!first.task_name.contains("S-1-5-21"));
    assert_eq!(
        first.task_name.rsplit('.').next().expect("digest").len(),
        16
    );
}

#[test]
fn invalid_sid_and_relative_worker_path_are_rejected() {
    let worker_path = std::env::temp_dir().join("hmm-save-backup-worker.exe");
    for sid in ["", "S-", "S--1", "s-1-5", "1-5-21", "S-1-x"] {
        assert!(ScheduledTaskSpec::new(sid, worker_path.clone()).is_err());
    }
    assert!(ScheduledTaskSpec::new(
        "S-1-5-21-100-200-300-400",
        PathBuf::from("hmm-save-backup-worker.exe"),
    )
    .is_err());
}

#[test]
fn exact_readback_matches_and_each_security_field_can_drift() {
    let spec = sample_spec();
    assert_eq!(
        spec.compare(&exact_readback(&spec)),
        ScheduledTaskSpecMatch::Exact
    );

    let mut cases = Vec::new();
    cases.push({
        let mut value = exact_readback(&spec);
        value.task_path = "\\Other\\".into();
        value
    });
    cases.push({
        let mut value = exact_readback(&spec);
        value.action_count = 2;
        value
    });
    cases.push({
        let mut value = exact_readback(&spec);
        value.action_arguments = "--once --profile default".into();
        value
    });
    cases.push({
        let mut value = exact_readback(&spec);
        value.action_execute = PathBuf::from(r"C:\other.exe");
        value
    });
    cases.push({
        let mut value = exact_readback(&spec);
        value.action_working_directory = r"C:\Temp".into();
        value
    });
    cases.push({
        let mut value = exact_readback(&spec);
        value.user_sid = "S-1-5-21-9".into();
        value
    });
    cases.push({
        let mut value = exact_readback(&spec);
        value.logon_trigger_count = 0;
        value
    });
    cases.push({
        let mut value = exact_readback(&spec);
        value.time_trigger_count = 2;
        value
    });
    cases.push({
        let mut value = exact_readback(&spec);
        value.logon_trigger_user_sid = "S-1-5-21-9".into();
        value
    });
    cases.push({
        let mut value = exact_readback(&spec);
        value.logon_trigger_enabled = false;
        value
    });
    cases.push({
        let mut value = exact_readback(&spec);
        value.time_trigger_enabled = false;
        value
    });
    cases.push({
        let mut value = exact_readback(&spec);
        value.logon_type = "Password".into();
        value
    });
    cases.push({
        let mut value = exact_readback(&spec);
        value.run_level = "Highest".into();
        value
    });
    cases.push({
        let mut value = exact_readback(&spec);
        value.logon_delay = "PT0M".into();
        value
    });
    cases.push({
        let mut value = exact_readback(&spec);
        value.periodic_interval = "PT30M".into();
        value
    });
    cases.push({
        let mut value = exact_readback(&spec);
        value.periodic_duration = "PT1H".into();
        value
    });
    cases.push({
        let mut value = exact_readback(&spec);
        value.multiple_instances = "Parallel".into();
        value
    });
    cases.push({
        let mut value = exact_readback(&spec);
        value.start_when_available = false;
        value
    });
    cases.push({
        let mut value = exact_readback(&spec);
        value.allow_start_on_batteries = false;
        value
    });
    cases.push({
        let mut value = exact_readback(&spec);
        value.dont_stop_on_batteries = false;
        value
    });
    cases.push({
        let mut value = exact_readback(&spec);
        value.wake_to_run = true;
        value
    });
    cases.push({
        let mut value = exact_readback(&spec);
        value.run_only_if_network_available = true;
        value
    });
    cases.push({
        let mut value = exact_readback(&spec);
        value.execution_time_limit = "PT2H".into();
        value
    });
    cases.push({
        let mut value = exact_readback(&spec);
        value.enabled = false;
        value
    });

    for value in cases {
        assert_eq!(spec.compare(&value), ScheduledTaskSpecMatch::OwnedDrift);
    }
}

#[test]
fn foreign_owner_is_not_treated_as_repairable_drift() {
    let spec = sample_spec();
    let mut readback = exact_readback(&spec);
    readback.owner_marker = "another.application/task/v1".to_owned();

    assert_eq!(
        spec.compare(&readback),
        ScheduledTaskSpecMatch::OwnershipConflict
    );
}

#[cfg(windows)]
#[test]
fn parses_versioned_inspect_output_without_exposing_raw_output() {
    let output = br#"{"schemaVersion":1,"status":"found","task":{"taskPath":"\\","ownerMarker":"dev.helsincy.modmanager/save-backup","userSid":"S-1-5-21-1","actionCount":1,"actionExecute":"C:\\HMM\\hmm-save-backup-worker.exe","actionArguments":"--once","actionWorkingDirectory":"","logonTriggerCount":1,"timeTriggerCount":1,"logonTriggerUserSid":"S-1-5-21-1","logonTriggerEnabled":true,"timeTriggerEnabled":true,"logonDelay":"PT1M","periodicInterval":"PT15M","periodicDuration":"","logonType":"Interactive","runLevel":"Limited","multipleInstances":"IgnoreNew","startWhenAvailable":true,"allowStartOnBatteries":true,"dontStopOnBatteries":true,"wakeToRun":false,"runOnlyIfNetworkAvailable":false,"executionTimeLimit":"PT1H","enabled":true}}"#;

    let parsed = parse_script_output(output).expect("valid output");

    assert!(matches!(parsed, ScheduledTaskCommandOutcome::Found(_)));
}

#[cfg(windows)]
#[test]
fn rejects_non_whitelisted_script_envelopes_and_oversized_output() {
    assert_eq!(
        parse_script_output(br#"{"schemaVersion":2,"status":"completed"}"#),
        Err(SaveBackupBackgroundRegistryError::CommandInvalidOutput)
    );
    assert_eq!(
        parse_script_output(br#"{"schemaVersion":1,"status":"surprise"}"#),
        Err(SaveBackupBackgroundRegistryError::CommandInvalidOutput)
    );
    assert_eq!(
        parse_script_output(br#"{"schemaVersion":1,"status":"completed","unexpected":true}"#,),
        Err(SaveBackupBackgroundRegistryError::CommandInvalidOutput)
    );
    assert_eq!(
        parse_script_output(br#"{"schemaVersion":1,"status":"found"}"#),
        Err(SaveBackupBackgroundRegistryError::CommandInvalidOutput)
    );
    assert_eq!(
        parse_script_output(
            br#"{"schemaVersion":1,"status":"identity","currentUserSid":"S-1-5-21-1","task":{"taskPath":"\\","ownerMarker":"x","userSid":"S-1-5-21-1","actionCount":0,"actionExecute":"","actionArguments":"","actionWorkingDirectory":"","logonTriggerCount":0,"timeTriggerCount":0,"logonTriggerUserSid":"","logonTriggerEnabled":false,"timeTriggerEnabled":false,"logonDelay":"","periodicInterval":"","periodicDuration":"","logonType":"","runLevel":"","multipleInstances":"","startWhenAvailable":false,"allowStartOnBatteries":false,"dontStopOnBatteries":false,"wakeToRun":false,"runOnlyIfNetworkAvailable":false,"executionTimeLimit":"","enabled":false}}"#,
        ),
        Err(SaveBackupBackgroundRegistryError::CommandInvalidOutput)
    );
    assert_eq!(
        parse_script_output(&vec![b'x'; 65_537]),
        Err(SaveBackupBackgroundRegistryError::CommandInvalidOutput)
    );
}

#[cfg(windows)]
#[test]
fn operation_failed_maps_to_typed_error_without_raw_output() {
    assert_eq!(
        parse_script_output(br#"{"schemaVersion":1,"status":"operation_failed"}"#),
        Err(SaveBackupBackgroundRegistryError::OperationFailed)
    );
}

#[cfg(windows)]
#[test]
fn runner_limits_and_system_runtime_are_fixed() {
    fn assert_runner<T: ScheduledTaskCommandRunner>() {}

    let runtime = system_powershell_runtime().expect("system PowerShell runtime");

    assert_runner::<PowerShellScheduledTaskCommandRunner>();
    let _runner = PowerShellScheduledTaskCommandRunner;
    let _run = <PowerShellScheduledTaskCommandRunner as ScheduledTaskCommandRunner>::run;
    assert_eq!(COMMAND_TIMEOUT, std::time::Duration::from_secs(15));
    assert_eq!(MAX_OUTPUT_BYTES, 64 * 1024);
    assert!(runtime.executable.is_absolute());
    assert!(runtime.executable.is_file());
    assert_eq!(
        runtime
            .executable
            .file_name()
            .and_then(|value| value.to_str()),
        Some("powershell.exe")
    );
    assert!(runtime.scheduled_tasks_module.is_absolute());
    assert!(runtime.scheduled_tasks_module.is_file());
    assert_eq!(
        runtime
            .scheduled_tasks_module
            .file_name()
            .and_then(|value| value.to_str()),
        Some("ScheduledTasks.psd1")
    );
}

#[cfg(windows)]
#[test]
fn command_builder_uses_only_fixed_executable_script_and_internal_env_keys() {
    let runtime = system_powershell_runtime().expect("system PowerShell runtime");
    let identity = build_command(&ScheduledTaskCommand::Identity).expect("identity command");

    assert_eq!(identity.get_program(), runtime.executable.as_os_str());
    assert_eq!(
        identity
            .get_args()
            .map(|value| value.to_string_lossy().into_owned())
            .collect::<Vec<_>>(),
        vec![
            "-NoLogo".to_owned(),
            "-NoProfile".to_owned(),
            "-NonInteractive".to_owned(),
            "-Command".to_owned(),
            SCRIPT.to_owned(),
        ]
    );
    assert_eq!(
        hmm_environment(&identity),
        BTreeMap::from([("HMM_OPERATION".to_owned(), "identity".to_owned())])
    );

    let inspect = build_command(&ScheduledTaskCommand::Inspect {
        task_name: "task-name".to_owned(),
        owner_marker: "owner-marker".to_owned(),
    })
    .expect("inspect command");
    assert_eq!(
        hmm_environment(&inspect),
        BTreeMap::from([
            ("HMM_OPERATION".to_owned(), "inspect".to_owned()),
            ("HMM_OWNER_MARKER".to_owned(), "owner-marker".to_owned()),
            (
                "HMM_SCHEDULED_TASKS_MODULE".to_owned(),
                runtime
                    .scheduled_tasks_module
                    .to_string_lossy()
                    .into_owned(),
            ),
            ("HMM_TASK_NAME".to_owned(), "task-name".to_owned()),
        ])
    );

    let spec = sample_spec();
    let register =
        build_command(&ScheduledTaskCommand::Register(spec.clone())).expect("register command");
    assert_eq!(
        hmm_environment(&register),
        BTreeMap::from([
            ("HMM_OPERATION".to_owned(), "register".to_owned()),
            ("HMM_OWNER_MARKER".to_owned(), spec.owner_marker),
            (
                "HMM_SCHEDULED_TASKS_MODULE".to_owned(),
                runtime
                    .scheduled_tasks_module
                    .to_string_lossy()
                    .into_owned(),
            ),
            ("HMM_TASK_NAME".to_owned(), spec.task_name),
            ("HMM_USER_SID".to_owned(), spec.user_sid),
            (
                "HMM_WORKER_PATH".to_owned(),
                spec.worker_path.to_string_lossy().into_owned(),
            ),
        ])
    );

    let unregister = build_command(&ScheduledTaskCommand::Unregister {
        task_name: "task-name".to_owned(),
        owner_marker: "owner-marker".to_owned(),
    })
    .expect("unregister command");
    assert_eq!(
        hmm_environment(&unregister),
        BTreeMap::from([
            ("HMM_OPERATION".to_owned(), "unregister".to_owned()),
            ("HMM_OWNER_MARKER".to_owned(), "owner-marker".to_owned()),
            (
                "HMM_SCHEDULED_TASKS_MODULE".to_owned(),
                runtime
                    .scheduled_tasks_module
                    .to_string_lossy()
                    .into_owned(),
            ),
            ("HMM_TASK_NAME".to_owned(), "task-name".to_owned()),
        ])
    );
}

#[cfg(windows)]
#[test]
fn scheduled_task_script_keeps_fail_closed_security_boundaries() {
    let script = include_str!("scheduled_task.ps1");

    assert!(script.contains("-TaskPath \"\\\""));
    assert!(script.contains("CategoryInfo.Category"));
    assert!(script.contains("CmdletizationQuery_NotFound"));
    assert!(script.contains("HMM_SCHEDULED_TASKS_MODULE"));
    assert!(script.contains("Import-Module -Name $modulePath"));
    assert!(script.contains("$Value.schemaVersion = 1"));
    assert!(!script.contains("NativeErrorCode"));
    assert!(!script.contains("Get-Module -ListAvailable"));
    assert!(!script.contains("ExecutionPolicy"));
    assert!(!script.contains("Invoke-Expression"));
    assert!(!script
        .lines()
        .any(|line| { line.contains("Register-ScheduledTask") && line.contains("-Force") }));
}

#[cfg(windows)]
fn hmm_environment(command: &std::process::Command) -> BTreeMap<String, String> {
    command
        .get_envs()
        .filter_map(|(key, value)| {
            let key = key.to_string_lossy();
            if !key.starts_with("HMM_") {
                return None;
            }
            value.map(|value| (key.into_owned(), value.to_string_lossy().into_owned()))
        })
        .collect()
}

fn sample_spec() -> ScheduledTaskSpec {
    ScheduledTaskSpec::new(
        "S-1-5-21-100-200-300-400",
        std::env::temp_dir().join("hmm-save-backup-worker.exe"),
    )
    .expect("sample spec")
}

fn exact_readback(spec: &ScheduledTaskSpec) -> ScheduledTaskReadback {
    ScheduledTaskReadback {
        task_path: spec.task_path.clone(),
        owner_marker: spec.owner_marker.clone(),
        user_sid: spec.user_sid.clone(),
        action_count: 1,
        action_execute: spec.worker_path.clone(),
        action_arguments: spec.action_arguments.clone(),
        action_working_directory: String::new(),
        logon_trigger_count: 1,
        time_trigger_count: 1,
        logon_trigger_user_sid: spec.user_sid.clone(),
        logon_trigger_enabled: true,
        time_trigger_enabled: true,
        logon_delay: spec.logon_delay.clone(),
        periodic_interval: spec.periodic_interval.clone(),
        periodic_duration: String::new(),
        logon_type: "Interactive".to_owned(),
        run_level: "Limited".to_owned(),
        multiple_instances: "IgnoreNew".to_owned(),
        start_when_available: true,
        allow_start_on_batteries: true,
        dont_stop_on_batteries: true,
        wake_to_run: false,
        run_only_if_network_available: false,
        execution_time_limit: spec.execution_time_limit.clone(),
        enabled: true,
    }
}
