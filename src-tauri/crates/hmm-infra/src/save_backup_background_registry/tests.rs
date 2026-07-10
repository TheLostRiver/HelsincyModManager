use super::task_spec::{ScheduledTaskReadback, ScheduledTaskSpec, ScheduledTaskSpecMatch};
use std::path::PathBuf;

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
