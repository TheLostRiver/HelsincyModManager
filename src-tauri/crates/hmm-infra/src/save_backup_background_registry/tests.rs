use super::registry::ScheduledTaskRegistry;
use super::task_spec::{
    ScheduledTaskReadback, ScheduledTaskSpec, ScheduledTaskSpecMatch, ScheduledTaskState,
};
use super::{
    InstallerCleanupOutcome, ScheduledTaskCommand, ScheduledTaskCommandOutcome,
    ScheduledTaskCommandRunner,
};
use hmm_core::SaveBackupBackgroundRegistrationStatus;
use hmm_ports::{
    SaveBackupBackgroundRegistry, SaveBackupBackgroundRegistryError,
    SaveBackupBackgroundRegistryResult,
};
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

#[cfg(windows)]
use super::powershell::{
    build_command, module_preflight_outcome, parse_script_output, system_powershell_runtime,
    PowerShellScheduledTaskCommandRunner, SystemPowerShellRuntime, COMMAND_TIMEOUT,
    MAX_OUTPUT_BYTES, SCRIPT,
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
    let mut running = exact_readback(&spec);
    running.state = ScheduledTaskState::Running;
    assert_eq!(
        spec.compare(&running),
        ScheduledTaskSpecMatch::Exact,
        "runtime state must not participate in registration spec drift",
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

#[derive(Clone, Default)]
struct FakeRunner {
    outcomes: Arc<Mutex<VecDeque<SaveBackupBackgroundRegistryResult<ScheduledTaskCommandOutcome>>>>,
    commands: Arc<Mutex<Vec<ScheduledTaskCommand>>>,
}

impl FakeRunner {
    fn with_outcomes(
        outcomes: Vec<SaveBackupBackgroundRegistryResult<ScheduledTaskCommandOutcome>>,
    ) -> Self {
        Self {
            outcomes: Arc::new(Mutex::new(outcomes.into())),
            commands: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn commands(&self) -> Vec<ScheduledTaskCommand> {
        self.commands.lock().expect("commands lock").clone()
    }
}

impl ScheduledTaskCommandRunner for FakeRunner {
    fn run(
        &self,
        command: ScheduledTaskCommand,
    ) -> SaveBackupBackgroundRegistryResult<ScheduledTaskCommandOutcome> {
        self.commands.lock().expect("commands lock").push(command);
        self.outcomes
            .lock()
            .expect("outcomes lock")
            .pop_front()
            .expect("queued outcome")
    }
}

#[test]
fn save_backup_installer_cleanup_preserves_missing_and_foreign_tasks_without_mutation() {
    let fixture = RegistryFixture::new_without_worker_file();
    let missing = FakeRunner::with_outcomes(vec![
        Ok(ScheduledTaskCommandOutcome::Identity(fixture.sid.clone())),
        Ok(ScheduledTaskCommandOutcome::Missing),
    ]);
    let registry = ScheduledTaskRegistry::with_worker_path(missing.clone(), None);
    assert_eq!(
        registry.cleanup_for_installer(),
        InstallerCleanupOutcome::AlreadyAbsent
    );
    assert!(matches!(
        missing.commands().as_slice(),
        [
            ScheduledTaskCommand::Identity,
            ScheduledTaskCommand::InstallerCleanup { .. }
        ]
    ));

    let foreign_runner = FakeRunner::with_outcomes(vec![
        Ok(ScheduledTaskCommandOutcome::Identity(fixture.sid)),
        Ok(ScheduledTaskCommandOutcome::OwnershipConflict),
    ]);
    let registry = ScheduledTaskRegistry::with_worker_path(foreign_runner.clone(), None);
    assert_eq!(
        registry.cleanup_for_installer(),
        InstallerCleanupOutcome::ForeignPreserved
    );
    assert!(matches!(
        foreign_runner.commands().as_slice(),
        [
            ScheduledTaskCommand::Identity,
            ScheduledTaskCommand::InstallerCleanup { .. }
        ]
    ));
}

#[test]
fn save_backup_installer_cleanup_removes_owned_exact_and_drift_when_quiescent() {
    for _case in ["owned_exact", "owned_drift"] {
        let fixture = RegistryFixture::new_without_worker_file();
        let runner = FakeRunner::with_outcomes(vec![
            Ok(ScheduledTaskCommandOutcome::Identity(fixture.sid)),
            Ok(ScheduledTaskCommandOutcome::Completed),
        ]);
        let registry =
            ScheduledTaskRegistry::with_worker_path(runner.clone(), Some(fixture.worker_path));

        assert_eq!(
            registry.cleanup_for_installer(),
            InstallerCleanupOutcome::Removed
        );
        assert!(matches!(
            runner.commands().as_slice(),
            [
                ScheduledTaskCommand::Identity,
                ScheduledTaskCommand::InstallerCleanup { .. }
            ]
        ));
    }
}

#[test]
fn save_backup_installer_cleanup_blocks_running_and_queued_tasks_without_mutation() {
    for _state in [ScheduledTaskState::Running, ScheduledTaskState::Queued] {
        let fixture = RegistryFixture::new_without_worker_file();
        let runner = FakeRunner::with_outcomes(vec![
            Ok(ScheduledTaskCommandOutcome::Identity(fixture.sid)),
            Ok(ScheduledTaskCommandOutcome::TaskBusy),
        ]);
        let registry = ScheduledTaskRegistry::with_worker_path(runner.clone(), None);

        assert_eq!(
            registry.cleanup_for_installer(),
            InstallerCleanupOutcome::OwnedTaskRunning
        );
        assert!(matches!(
            runner.commands().as_slice(),
            [
                ScheduledTaskCommand::Identity,
                ScheduledTaskCommand::InstallerCleanup { .. }
            ]
        ));
    }
}

#[test]
fn save_backup_installer_cleanup_fails_closed_when_identity_owner_or_state_is_unverified() {
    let identity_error =
        FakeRunner::with_outcomes(vec![Err(SaveBackupBackgroundRegistryError::CommandTimeout)]);
    let registry = ScheduledTaskRegistry::with_worker_path(identity_error.clone(), None);
    assert_eq!(
        registry.cleanup_for_installer(),
        InstallerCleanupOutcome::OwnershipUnverified
    );
    assert_eq!(
        identity_error.commands(),
        vec![ScheduledTaskCommand::Identity]
    );

    let fixture = RegistryFixture::new_without_worker_file();
    for (cleanup, expected) in [
        (
            ScheduledTaskCommandOutcome::PermissionRequired,
            InstallerCleanupOutcome::OwnershipUnverified,
        ),
        (
            ScheduledTaskCommandOutcome::ModuleUnavailable,
            InstallerCleanupOutcome::PlatformUnavailable,
        ),
        (
            ScheduledTaskCommandOutcome::StateUnverified,
            InstallerCleanupOutcome::OwnershipUnverified,
        ),
        (
            ScheduledTaskCommandOutcome::PostDeleteForeign,
            InstallerCleanupOutcome::OwnershipUnverified,
        ),
    ] {
        let runner = FakeRunner::with_outcomes(vec![
            Ok(ScheduledTaskCommandOutcome::Identity(fixture.sid.clone())),
            Ok(cleanup),
        ]);
        let registry = ScheduledTaskRegistry::with_worker_path(runner.clone(), None);
        assert_eq!(registry.cleanup_for_installer(), expected);
        assert_eq!(runner.commands().len(), 2);
    }
}

#[test]
fn save_backup_installer_cleanup_distinguishes_mutation_and_post_delete_failures() {
    let fixture = RegistryFixture::new_without_worker_file();
    for (cleanup, expected) in [
        (
            Ok(ScheduledTaskCommandOutcome::Missing),
            InstallerCleanupOutcome::AlreadyAbsent,
        ),
        (
            Ok(ScheduledTaskCommandOutcome::OwnershipConflict),
            InstallerCleanupOutcome::ForeignPreserved,
        ),
        (
            Ok(ScheduledTaskCommandOutcome::PermissionRequired),
            InstallerCleanupOutcome::OwnershipUnverified,
        ),
        (
            Ok(ScheduledTaskCommandOutcome::StateUnverified),
            InstallerCleanupOutcome::OwnershipUnverified,
        ),
        (
            Ok(ScheduledTaskCommandOutcome::TaskBusy),
            InstallerCleanupOutcome::OwnedTaskRunning,
        ),
        (
            Ok(ScheduledTaskCommandOutcome::PostDeleteOwned),
            InstallerCleanupOutcome::RemovalUnverified,
        ),
        (
            Ok(ScheduledTaskCommandOutcome::PostDeleteForeign),
            InstallerCleanupOutcome::OwnershipUnverified,
        ),
        (
            Err(SaveBackupBackgroundRegistryError::OperationFailed),
            InstallerCleanupOutcome::RemovalUnverified,
        ),
    ] {
        let runner = FakeRunner::with_outcomes(vec![
            Ok(ScheduledTaskCommandOutcome::Identity(fixture.sid.clone())),
            cleanup,
        ]);
        let registry = ScheduledTaskRegistry::with_worker_path(runner.clone(), None);
        assert_eq!(registry.cleanup_for_installer(), expected);
        assert_eq!(runner.commands().len(), 2);
    }
}

struct RegistryFixture {
    _temp: tempfile::TempDir,
    sid: String,
    worker_path: PathBuf,
    exact_readback: ScheduledTaskReadback,
}

impl RegistryFixture {
    fn new() -> Self {
        Self::build(true)
    }

    fn new_without_worker_file() -> Self {
        Self::build(false)
    }

    fn build(create_worker: bool) -> Self {
        let temp = tempfile::tempdir().expect("temp dir");
        let worker_path = temp.path().join("hmm-save-backup-worker.exe");
        if create_worker {
            std::fs::write(&worker_path, b"fixture").expect("write worker fixture");
        }
        let sid = "S-1-5-21-100-200-300-400".to_owned();
        let spec_path = if create_worker {
            std::fs::canonicalize(&worker_path).expect("canonical worker fixture")
        } else {
            worker_path.clone()
        };
        let spec = ScheduledTaskSpec::new(&sid, spec_path).expect("fixture spec");
        let exact_readback = exact_readback(&spec);
        Self {
            _temp: temp,
            sid,
            worker_path,
            exact_readback,
        }
    }
}

#[test]
fn register_creates_missing_task_then_requires_exact_readback() {
    let fixture = RegistryFixture::new();
    let runner = FakeRunner::with_outcomes(vec![
        Ok(ScheduledTaskCommandOutcome::Identity(fixture.sid.clone())),
        Ok(ScheduledTaskCommandOutcome::Found(Box::new(
            fixture.exact_readback.clone(),
        ))),
    ]);
    let registry = ScheduledTaskRegistry::new(runner.clone(), fixture.worker_path);

    assert_eq!(
        registry.register().expect("register"),
        SaveBackupBackgroundRegistrationStatus::Registered
    );
    assert!(matches!(
        runner.commands().as_slice(),
        [
            ScheduledTaskCommand::Identity,
            ScheduledTaskCommand::Register(_)
        ]
    ));
}

#[test]
fn exact_registration_is_reconciled_in_one_command() {
    let fixture = RegistryFixture::new();
    let runner = FakeRunner::with_outcomes(vec![
        Ok(ScheduledTaskCommandOutcome::Identity(fixture.sid.clone())),
        Ok(ScheduledTaskCommandOutcome::Found(Box::new(
            fixture.exact_readback,
        ))),
    ]);
    let registry = ScheduledTaskRegistry::new(runner.clone(), fixture.worker_path);

    assert_eq!(
        registry.register().expect("register"),
        SaveBackupBackgroundRegistrationStatus::Registered
    );
    assert!(matches!(
        runner.commands().as_slice(),
        [
            ScheduledTaskCommand::Identity,
            ScheduledTaskCommand::Register(_)
        ]
    ));
}

#[test]
fn current_user_identity_is_cached_across_registry_operations() {
    let fixture = RegistryFixture::new();
    let runner = FakeRunner::with_outcomes(vec![
        Ok(ScheduledTaskCommandOutcome::Identity(fixture.sid)),
        Ok(ScheduledTaskCommandOutcome::Found(Box::new(
            fixture.exact_readback.clone(),
        ))),
        Ok(ScheduledTaskCommandOutcome::Found(Box::new(
            fixture.exact_readback,
        ))),
        Ok(ScheduledTaskCommandOutcome::Completed),
    ]);
    let registry = ScheduledTaskRegistry::new(runner.clone(), fixture.worker_path);

    assert_eq!(
        registry.inspect().expect("inspect"),
        SaveBackupBackgroundRegistrationStatus::Registered
    );
    assert_eq!(
        registry.register().expect("register"),
        SaveBackupBackgroundRegistrationStatus::Registered
    );
    assert_eq!(
        registry.unregister().expect("unregister"),
        SaveBackupBackgroundRegistrationStatus::NotRegistered
    );
    assert!(matches!(
        runner.commands().as_slice(),
        [
            ScheduledTaskCommand::Identity,
            ScheduledTaskCommand::Inspect { .. },
            ScheduledTaskCommand::Register(_),
            ScheduledTaskCommand::Unregister { .. }
        ]
    ));
}

#[test]
fn register_repairs_owned_drift_and_blocks_foreign_owner_observed_before_mutation() {
    let fixture = RegistryFixture::new();
    let repair = FakeRunner::with_outcomes(vec![
        Ok(ScheduledTaskCommandOutcome::Identity(fixture.sid.clone())),
        Ok(ScheduledTaskCommandOutcome::Found(Box::new(
            fixture.exact_readback.clone(),
        ))),
    ]);
    let registry = ScheduledTaskRegistry::new(repair.clone(), fixture.worker_path.clone());
    assert_eq!(
        registry.register().expect("repair"),
        SaveBackupBackgroundRegistrationStatus::Registered
    );
    assert!(repair
        .commands()
        .iter()
        .any(|command| matches!(command, ScheduledTaskCommand::Register(_))));

    let conflict = FakeRunner::with_outcomes(vec![
        Ok(ScheduledTaskCommandOutcome::Identity(fixture.sid)),
        Ok(ScheduledTaskCommandOutcome::OwnershipConflict),
    ]);
    let registry = ScheduledTaskRegistry::new(conflict.clone(), fixture.worker_path);
    assert_eq!(
        registry.register().expect_err("foreign owner blocked"),
        SaveBackupBackgroundRegistryError::TaskOwnershipConflict
    );
    assert!(conflict
        .commands()
        .iter()
        .any(|command| matches!(command, ScheduledTaskCommand::Register(_))));
}

#[test]
fn register_reports_post_write_drift_instead_of_claiming_success() {
    let fixture = RegistryFixture::new();
    let mut drift = fixture.exact_readback;
    drift.enabled = false;
    let runner = FakeRunner::with_outcomes(vec![
        Ok(ScheduledTaskCommandOutcome::Identity(fixture.sid)),
        Ok(ScheduledTaskCommandOutcome::Found(Box::new(drift))),
    ]);
    let registry = ScheduledTaskRegistry::new(runner, fixture.worker_path);

    assert_eq!(
        registry.register().expect("register"),
        SaveBackupBackgroundRegistrationStatus::ConfigurationDrift
    );
}

#[test]
fn inspect_maps_missing_drift_permission_and_module_statuses() {
    let fixture = RegistryFixture::new();
    let cases = [
        (
            ScheduledTaskCommandOutcome::Missing,
            SaveBackupBackgroundRegistrationStatus::NotRegistered,
        ),
        (
            ScheduledTaskCommandOutcome::PermissionRequired,
            SaveBackupBackgroundRegistrationStatus::PermissionRequired,
        ),
        (
            ScheduledTaskCommandOutcome::ModuleUnavailable,
            SaveBackupBackgroundRegistrationStatus::UnsupportedPlatform,
        ),
    ];
    for (outcome, expected) in cases {
        let runner = FakeRunner::with_outcomes(vec![
            Ok(ScheduledTaskCommandOutcome::Identity(fixture.sid.clone())),
            Ok(outcome),
        ]);
        let registry = ScheduledTaskRegistry::new(runner, fixture.worker_path.clone());
        assert_eq!(registry.inspect().expect("inspect"), expected);
    }

    let mut drift = fixture.exact_readback;
    drift.run_level = "Highest".to_owned();
    let runner = FakeRunner::with_outcomes(vec![
        Ok(ScheduledTaskCommandOutcome::Identity(fixture.sid)),
        Ok(ScheduledTaskCommandOutcome::Found(Box::new(drift))),
    ]);
    let registry = ScheduledTaskRegistry::new(runner, fixture.worker_path);
    assert_eq!(
        registry.inspect().expect("inspect drift"),
        SaveBackupBackgroundRegistrationStatus::ConfigurationDrift
    );
}

#[test]
fn inspect_maps_runner_ownership_conflict_to_safe_error() {
    let fixture = RegistryFixture::new();
    let runner = FakeRunner::with_outcomes(vec![
        Ok(ScheduledTaskCommandOutcome::Identity(fixture.sid)),
        Ok(ScheduledTaskCommandOutcome::OwnershipConflict),
    ]);
    let registry = ScheduledTaskRegistry::new(runner, fixture.worker_path);

    assert_eq!(
        registry.inspect().expect_err("ownership conflict blocked"),
        SaveBackupBackgroundRegistryError::TaskOwnershipConflict
    );
}

#[test]
fn register_maps_write_permission_and_module_outcomes_without_readback() {
    for (outcome, expected) in [
        (
            ScheduledTaskCommandOutcome::PermissionRequired,
            SaveBackupBackgroundRegistrationStatus::PermissionRequired,
        ),
        (
            ScheduledTaskCommandOutcome::ModuleUnavailable,
            SaveBackupBackgroundRegistrationStatus::UnsupportedPlatform,
        ),
    ] {
        let fixture = RegistryFixture::new();
        let runner = FakeRunner::with_outcomes(vec![
            Ok(ScheduledTaskCommandOutcome::Identity(fixture.sid)),
            Ok(outcome),
        ]);
        let registry = ScheduledTaskRegistry::new(runner.clone(), fixture.worker_path);

        assert_eq!(registry.register().expect("register status"), expected);
        assert_eq!(runner.commands().len(), 2);
    }
}

#[test]
fn worker_path_must_resolve_to_a_real_non_symlink_file_before_inspect() {
    let fixture = RegistryFixture::new_without_worker_file();
    let runner = FakeRunner::default();
    let registry = ScheduledTaskRegistry::new(runner.clone(), fixture.worker_path);
    assert_eq!(
        registry.inspect().expect_err("missing worker"),
        SaveBackupBackgroundRegistryError::WorkerBinaryUnavailable
    );
    assert!(runner.commands().is_empty());

    let temp = tempfile::tempdir().expect("temp dir");
    let registry = ScheduledTaskRegistry::new(runner.clone(), temp.path().to_path_buf());
    assert_eq!(
        registry.inspect().expect_err("directory is not worker"),
        SaveBackupBackgroundRegistryError::WorkerBinaryUnavailable
    );
    assert!(runner.commands().is_empty());

    let registry = ScheduledTaskRegistry::new(runner.clone(), PathBuf::from("worker.exe"));
    assert_eq!(
        registry.inspect().expect_err("relative worker"),
        SaveBackupBackgroundRegistryError::WorkerBinaryUnavailable
    );
    assert!(runner.commands().is_empty());
}

#[test]
fn worker_symlink_is_rejected_before_inspect_when_platform_allows_fixture() {
    let temp = tempfile::tempdir().expect("temp dir");
    let target = temp.path().join("worker-target.exe");
    let link = temp.path().join("hmm-save-backup-worker.exe");
    std::fs::write(&target, b"fixture").expect("write worker target");
    if let Err(error) = create_file_symlink(&target, &link) {
        if error.kind() == std::io::ErrorKind::PermissionDenied
            || error.raw_os_error() == Some(1314)
        {
            return;
        }
        panic!("create symlink fixture: {error}");
    }
    let runner = FakeRunner::default();
    let registry = ScheduledTaskRegistry::new(runner.clone(), link);

    assert_eq!(
        registry.inspect().expect_err("symlink worker"),
        SaveBackupBackgroundRegistryError::WorkerBinaryUnavailable
    );
    assert!(runner.commands().is_empty());
}

#[test]
fn unregister_is_idempotent_rechecks_and_does_not_require_worker_file() {
    let fixture = RegistryFixture::new_without_worker_file();
    let runner = FakeRunner::with_outcomes(vec![
        Ok(ScheduledTaskCommandOutcome::Identity(fixture.sid)),
        Ok(ScheduledTaskCommandOutcome::Completed),
    ]);
    let registry = ScheduledTaskRegistry::new(runner.clone(), fixture.worker_path);

    assert_eq!(
        registry.unregister().expect("unregister"),
        SaveBackupBackgroundRegistrationStatus::NotRegistered
    );
    assert!(matches!(
        runner.commands().as_slice(),
        [
            ScheduledTaskCommand::Identity,
            ScheduledTaskCommand::Unregister { .. }
        ]
    ));
}

#[test]
fn unregister_missing_task_is_a_noop_and_foreign_task_is_blocked() {
    let fixture = RegistryFixture::new_without_worker_file();
    let missing = FakeRunner::with_outcomes(vec![
        Ok(ScheduledTaskCommandOutcome::Identity(fixture.sid.clone())),
        Ok(ScheduledTaskCommandOutcome::Missing),
    ]);
    let registry = ScheduledTaskRegistry::new(missing.clone(), fixture.worker_path.clone());
    assert_eq!(
        registry.unregister().expect("missing unregister"),
        SaveBackupBackgroundRegistrationStatus::NotRegistered
    );
    assert!(matches!(
        missing.commands().as_slice(),
        [
            ScheduledTaskCommand::Identity,
            ScheduledTaskCommand::Unregister { .. }
        ]
    ));

    let conflict = FakeRunner::with_outcomes(vec![
        Ok(ScheduledTaskCommandOutcome::Identity(fixture.sid)),
        Ok(ScheduledTaskCommandOutcome::OwnershipConflict),
    ]);
    let registry = ScheduledTaskRegistry::new(conflict.clone(), fixture.worker_path);
    assert_eq!(
        registry
            .unregister()
            .expect_err("foreign unregister blocked"),
        SaveBackupBackgroundRegistryError::TaskOwnershipConflict
    );
    assert!(matches!(
        conflict.commands().as_slice(),
        [
            ScheduledTaskCommand::Identity,
            ScheduledTaskCommand::Unregister { .. }
        ]
    ));
}

#[test]
fn unregister_requires_missing_post_delete_readback() {
    let fixture = RegistryFixture::new_without_worker_file();
    let runner = FakeRunner::with_outcomes(vec![
        Ok(ScheduledTaskCommandOutcome::Identity(fixture.sid)),
        Ok(ScheduledTaskCommandOutcome::PostDeleteOwned),
    ]);
    let registry = ScheduledTaskRegistry::new(runner, fixture.worker_path);

    assert_eq!(
        registry.unregister().expect("unregister readback"),
        SaveBackupBackgroundRegistrationStatus::RegistrationFailed
    );
}

#[test]
fn missing_worker_locator_blocks_inspect_and_register_but_not_unregister() {
    let blocked = FakeRunner::default();
    let registry = ScheduledTaskRegistry::with_worker_path(blocked.clone(), None);
    assert_eq!(
        registry.inspect().expect_err("inspect blocked"),
        SaveBackupBackgroundRegistryError::WorkerBinaryUnavailable
    );
    assert_eq!(
        registry.register().expect_err("register blocked"),
        SaveBackupBackgroundRegistryError::WorkerBinaryUnavailable
    );
    assert!(blocked.commands().is_empty());

    let allowed = FakeRunner::with_outcomes(vec![
        Ok(ScheduledTaskCommandOutcome::Identity(
            "S-1-5-21-100-200-300-400".to_owned(),
        )),
        Ok(ScheduledTaskCommandOutcome::Missing),
    ]);
    let registry = ScheduledTaskRegistry::with_worker_path(allowed.clone(), None);
    assert_eq!(
        registry.unregister().expect("unregister without worker"),
        SaveBackupBackgroundRegistrationStatus::NotRegistered
    );
    assert_eq!(allowed.commands().len(), 2);
}

#[test]
fn runner_errors_remain_typed_and_fail_closed() {
    for expected in [
        SaveBackupBackgroundRegistryError::CommandTimeout,
        SaveBackupBackgroundRegistryError::CommandInvalidOutput,
        SaveBackupBackgroundRegistryError::OperationFailed,
    ] {
        let fixture = RegistryFixture::new();
        let runner = FakeRunner::with_outcomes(vec![Err(expected)]);
        let registry = ScheduledTaskRegistry::new(runner, fixture.worker_path);
        assert_eq!(registry.inspect().expect_err("typed failure"), expected);
    }
}

#[test]
fn invalid_identity_and_unexpected_phase_outcomes_fail_closed() {
    let fixture = RegistryFixture::new();
    let invalid_identity = FakeRunner::with_outcomes(vec![Ok(
        ScheduledTaskCommandOutcome::Identity("not-a-sid".to_owned()),
    )]);
    let registry = ScheduledTaskRegistry::new(invalid_identity, fixture.worker_path.clone());
    assert_eq!(
        registry.inspect().expect_err("invalid identity"),
        SaveBackupBackgroundRegistryError::CommandInvalidOutput
    );

    let unexpected_inspect = FakeRunner::with_outcomes(vec![
        Ok(ScheduledTaskCommandOutcome::Identity(fixture.sid.clone())),
        Ok(ScheduledTaskCommandOutcome::Completed),
    ]);
    let registry = ScheduledTaskRegistry::new(unexpected_inspect, fixture.worker_path.clone());
    assert_eq!(
        registry.inspect().expect_err("unexpected inspect"),
        SaveBackupBackgroundRegistryError::CommandInvalidOutput
    );

    let unexpected_register = FakeRunner::with_outcomes(vec![
        Ok(ScheduledTaskCommandOutcome::Identity(fixture.sid.clone())),
        Ok(ScheduledTaskCommandOutcome::Missing),
        Ok(ScheduledTaskCommandOutcome::Missing),
    ]);
    let registry = ScheduledTaskRegistry::new(unexpected_register, fixture.worker_path.clone());
    assert_eq!(
        registry.register().expect_err("unexpected register"),
        SaveBackupBackgroundRegistryError::CommandInvalidOutput
    );

    let unexpected_unregister = FakeRunner::with_outcomes(vec![
        Ok(ScheduledTaskCommandOutcome::Identity(fixture.sid)),
        Ok(ScheduledTaskCommandOutcome::Found(Box::new(
            fixture.exact_readback,
        ))),
        Ok(ScheduledTaskCommandOutcome::Missing),
    ]);
    let registry = ScheduledTaskRegistry::new(unexpected_unregister, fixture.worker_path);
    assert_eq!(
        registry.unregister().expect_err("unexpected unregister"),
        SaveBackupBackgroundRegistryError::CommandInvalidOutput
    );
}

#[cfg(target_os = "windows")]
#[test]
#[ignore = "creates a real user Scheduled Task; disposable Windows account/VM only"]
fn windows_scheduled_task_registry_smoke() {
    assert_eq!(
        std::env::var("HMM_RUN_WINDOWS_SCHEDULED_TASK_SMOKE").as_deref(),
        Ok("1"),
        "explicit smoke authorization is required",
    );
    let worker_path = std::env::var_os("HMM_WINDOWS_SMOKE_WORKER_PATH")
        .map(PathBuf::from)
        .expect("test-only worker path is required");
    let registry = ScheduledTaskRegistry::new(PowerShellScheduledTaskCommandRunner, worker_path);
    assert_eq!(
        registry.inspect().expect("initial inspect"),
        SaveBackupBackgroundRegistrationStatus::NotRegistered,
        "smoke refuses to overwrite a pre-existing task",
    );

    let body = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        assert_eq!(
            registry.register().expect("register"),
            SaveBackupBackgroundRegistrationStatus::Registered
        );
        assert_eq!(
            registry.inspect().expect("inspect"),
            SaveBackupBackgroundRegistrationStatus::Registered
        );
        assert_eq!(
            registry.register().expect("idempotent register"),
            SaveBackupBackgroundRegistrationStatus::Registered
        );
        if std::env::var("HMM_WINDOWS_SMOKE_WAIT_FOR_TRIGGER").as_deref() == Ok("1") {
            println!(
                "Run the registered task in Task Scheduler, verify the heartbeat in the second terminal, then press Enter."
            );
            let mut acknowledgement = String::new();
            std::io::stdin()
                .read_line(&mut acknowledgement)
                .expect("read smoke acknowledgement");
            assert_eq!(
                registry.inspect().expect("post-trigger inspect"),
                SaveBackupBackgroundRegistrationStatus::Registered
            );
        }
    }));

    let first_cleanup = registry.unregister();
    let second_cleanup = registry.unregister();
    let final_inspect = registry.inspect();
    assert_eq!(
        first_cleanup.expect("first cleanup"),
        SaveBackupBackgroundRegistrationStatus::NotRegistered
    );
    assert_eq!(
        second_cleanup.expect("idempotent cleanup"),
        SaveBackupBackgroundRegistrationStatus::NotRegistered
    );
    assert_eq!(
        final_inspect.expect("cleanup read-back"),
        SaveBackupBackgroundRegistrationStatus::NotRegistered
    );
    if let Err(payload) = body {
        std::panic::resume_unwind(payload);
    }
}

#[cfg(target_os = "windows")]
#[test]
#[ignore = "cleanup for an explicitly authorized disposable Scheduled Task smoke"]
fn windows_scheduled_task_registry_cleanup_smoke() {
    assert_eq!(
        std::env::var("HMM_RUN_WINDOWS_SCHEDULED_TASK_SMOKE").as_deref(),
        Ok("1"),
        "explicit smoke authorization is required",
    );
    let worker_path = std::env::var_os("HMM_WINDOWS_SMOKE_WORKER_PATH")
        .map(PathBuf::from)
        .expect("test-only worker path is required");
    let registry = ScheduledTaskRegistry::new(PowerShellScheduledTaskCommandRunner, worker_path);
    assert_eq!(
        registry.unregister().expect("first cleanup"),
        SaveBackupBackgroundRegistrationStatus::NotRegistered
    );
    assert_eq!(
        registry.unregister().expect("idempotent cleanup"),
        SaveBackupBackgroundRegistrationStatus::NotRegistered
    );
}

#[cfg(windows)]
fn create_file_symlink(target: &std::path::Path, link: &std::path::Path) -> std::io::Result<()> {
    std::os::windows::fs::symlink_file(target, link)
}

#[cfg(unix)]
fn create_file_symlink(target: &std::path::Path, link: &std::path::Path) -> std::io::Result<()> {
    std::os::unix::fs::symlink(target, link)
}

#[cfg(windows)]
#[test]
fn parses_versioned_inspect_output_without_exposing_raw_output() {
    let output = br#"{"schemaVersion":1,"status":"found","task":{"taskPath":"\\","ownerMarker":"dev.helsincy.modmanager/save-backup","userSid":"S-1-5-21-1","actionCount":1,"actionExecute":"C:\\HMM\\hmm-save-backup-worker.exe","actionArguments":"--once","actionWorkingDirectory":"","logonTriggerCount":1,"timeTriggerCount":1,"logonTriggerUserSid":"S-1-5-21-1","logonTriggerEnabled":true,"timeTriggerEnabled":true,"logonDelay":"PT1M","periodicInterval":"PT15M","periodicDuration":"","logonType":"Interactive","runLevel":"Limited","multipleInstances":"IgnoreNew","startWhenAvailable":true,"allowStartOnBatteries":true,"dontStopOnBatteries":true,"wakeToRun":false,"runOnlyIfNetworkAvailable":false,"executionTimeLimit":"PT1H","enabled":true,"state":"Ready"}}"#;

    let parsed = parse_script_output(output).expect("valid output");

    assert!(matches!(parsed, ScheduledTaskCommandOutcome::Found(_)));
    assert_eq!(
        parse_script_output(br#"{"schemaVersion":1,"status":"task_busy"}"#),
        Ok(ScheduledTaskCommandOutcome::TaskBusy)
    );
    assert_eq!(
        parse_script_output(br#"{"schemaVersion":1,"status":"state_unverified"}"#),
        Ok(ScheduledTaskCommandOutcome::StateUnverified)
    );
    assert_eq!(
        parse_script_output(br#"{"schemaVersion":1,"status":"post_delete_owned"}"#),
        Ok(ScheduledTaskCommandOutcome::PostDeleteOwned)
    );
    assert_eq!(
        parse_script_output(br#"{"schemaVersion":1,"status":"post_delete_foreign"}"#),
        Ok(ScheduledTaskCommandOutcome::PostDeleteForeign)
    );

    let invalid_state = String::from_utf8(output.to_vec())
        .expect("readback fixture is UTF-8")
        .replace("\"state\":\"Ready\"", "\"state\":\"Unexpected\"");
    assert_eq!(
        parse_script_output(invalid_state.as_bytes()),
        Err(SaveBackupBackgroundRegistryError::CommandInvalidOutput)
    );
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
fn missing_scheduled_tasks_module_is_classified_without_spawning() {
    let directory = tempfile::tempdir().expect("temporary runtime directory");
    let runtime = SystemPowerShellRuntime {
        executable: directory.path().join("powershell.exe"),
        scheduled_tasks_module: directory.path().join("ScheduledTasks.psd1"),
    };

    assert_eq!(
        module_preflight_outcome(&ScheduledTaskCommand::Identity, &runtime),
        None
    );
    assert_eq!(
        module_preflight_outcome(
            &ScheduledTaskCommand::Inspect {
                task_name: "task-name".to_owned(),
                owner_marker: "owner-marker".to_owned(),
            },
            &runtime,
        ),
        Some(ScheduledTaskCommandOutcome::ModuleUnavailable)
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

    let installer_cleanup = build_command(&ScheduledTaskCommand::InstallerCleanup {
        task_name: "task-name".to_owned(),
        owner_marker: "owner-marker".to_owned(),
    })
    .expect("installer cleanup command");
    assert_eq!(
        hmm_environment(&installer_cleanup),
        BTreeMap::from([
            ("HMM_OPERATION".to_owned(), "installer_cleanup".to_owned(),),
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
    assert!(!script.contains("Stop-ScheduledTask"));
    assert!(!script.contains("Stop-Process"));
    assert!(!script.contains("schtasks"));
    assert!(!script
        .lines()
        .any(|line| { line.contains("Register-ScheduledTask") && line.contains("-Force") }));

    let quiescence_guard = script
        .split("function Assert-InstallerCleanupTaskIsQuiescent")
        .nth(1)
        .and_then(|value| {
            value
                .split("function Write-InstallerCleanupPostDeleteStatus")
                .next()
        })
        .expect("installer cleanup quiescence guard exists");
    let owner_check = quiescence_guard
        .find("$Task.Description")
        .expect("owner check");
    let state_check = quiescence_guard.find("$Task.State").expect("state check");
    assert!(owner_check < state_check);

    let cleanup = script
        .split("if ($operation -eq \"installer_cleanup\")")
        .nth(1)
        .expect("installer cleanup operation exists");
    assert_eq!(
        cleanup
            .matches("Assert-InstallerCleanupTaskIsQuiescent")
            .count(),
        2
    );
    assert_eq!(cleanup.matches("Get-TaskOrStatus $taskName").count(), 2);
    let delete = cleanup
        .find("Unregister-ScheduledTask")
        .expect("owned delete");
    let post_delete = cleanup
        .find("Write-InstallerCleanupPostDeleteStatus")
        .expect("post-delete read-back");
    assert!(delete < post_delete);
    assert!(!cleanup.contains(".Actions"));
    assert!(!cleanup.contains(".Triggers"));
    assert!(!cleanup.contains(".Settings"));
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
        state: ScheduledTaskState::Ready,
    }
}
