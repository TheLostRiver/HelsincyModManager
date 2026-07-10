use anyhow::Result;
use hmm_app::{
    SaveBackupBackgroundRegistrationResult, SaveBackupBackgroundService,
    SaveBackupBackgroundServiceError, SAVE_BACKUP_BACKGROUND_HEARTBEAT_TTL_MILLIS,
};
use hmm_core::{
    GameId, ProfileId, SaveBackupBackgroundProtectionStatus,
    SaveBackupBackgroundRegistrationStatus, SaveBackupSchedulerLeaseRequest,
    SaveBackupSchedulerState, SaveBackupWorkerHeartbeat,
};
use hmm_ports::{
    AppClock, AuditLogEvent, AuditLogWriter, SaveBackupBackgroundRegistry,
    SaveBackupBackgroundRegistryError, SaveBackupBackgroundRegistryResult,
    SaveBackupSchedulerStateRepository,
};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

#[test]
fn protection_status_requires_enabled_exact_registration_and_fresh_heartbeat() {
    let now = 3_000_000_u128;
    let missing = Harness::new(
        now,
        SaveBackupBackgroundRegistrationStatus::Registered,
        None,
    );
    assert_eq!(
        missing
            .service
            .status(&GameId::mhw(), &ProfileId::new("default"))
            .expect("status")
            .status,
        SaveBackupBackgroundProtectionStatus::NotEnabled
    );
    assert!(missing.registry.calls().is_empty());

    let cases = [
        (
            false,
            false,
            SaveBackupBackgroundRegistrationStatus::Registered,
            Some(now),
            SaveBackupBackgroundProtectionStatus::NotEnabled,
            None,
        ),
        (
            true,
            false,
            SaveBackupBackgroundRegistrationStatus::Registered,
            Some(now),
            SaveBackupBackgroundProtectionStatus::TrayOnly,
            None,
        ),
        (
            true,
            true,
            SaveBackupBackgroundRegistrationStatus::UnsupportedPlatform,
            Some(now),
            SaveBackupBackgroundProtectionStatus::UnsupportedPlatform,
            Some("save_backup_background_unsupported_platform"),
        ),
        (
            true,
            true,
            SaveBackupBackgroundRegistrationStatus::PermissionRequired,
            Some(now),
            SaveBackupBackgroundProtectionStatus::PermissionRequired,
            Some("save_backup_background_permission_required"),
        ),
        (
            true,
            true,
            SaveBackupBackgroundRegistrationStatus::NotRegistered,
            Some(now),
            SaveBackupBackgroundProtectionStatus::RegistrationFailed,
            Some("save_backup_background_not_registered"),
        ),
        (
            true,
            true,
            SaveBackupBackgroundRegistrationStatus::ConfigurationDrift,
            Some(now),
            SaveBackupBackgroundProtectionStatus::RegistrationFailed,
            Some("save_backup_background_configuration_drift"),
        ),
        (
            true,
            true,
            SaveBackupBackgroundRegistrationStatus::RegistrationFailed,
            Some(now),
            SaveBackupBackgroundProtectionStatus::RegistrationFailed,
            Some("save_backup_background_registration_failed"),
        ),
        (
            true,
            true,
            SaveBackupBackgroundRegistrationStatus::Registered,
            None,
            SaveBackupBackgroundProtectionStatus::WorkerUnhealthy,
            Some("save_backup_background_worker_unhealthy"),
        ),
        (
            true,
            true,
            SaveBackupBackgroundRegistrationStatus::Registered,
            Some(now + 1),
            SaveBackupBackgroundProtectionStatus::WorkerUnhealthy,
            Some("save_backup_background_worker_unhealthy"),
        ),
        (
            true,
            true,
            SaveBackupBackgroundRegistrationStatus::Registered,
            Some(now - SAVE_BACKUP_BACKGROUND_HEARTBEAT_TTL_MILLIS - 1),
            SaveBackupBackgroundProtectionStatus::WorkerUnhealthy,
            Some("save_backup_background_worker_unhealthy"),
        ),
        (
            true,
            true,
            SaveBackupBackgroundRegistrationStatus::Registered,
            Some(now - SAVE_BACKUP_BACKGROUND_HEARTBEAT_TTL_MILLIS),
            SaveBackupBackgroundProtectionStatus::Protected,
            None,
        ),
    ];

    for (enabled, protection_enabled, registration, heartbeat, expected, expected_error) in cases {
        let harness = Harness::new(
            now,
            registration,
            Some(sample_state(enabled, protection_enabled, heartbeat)),
        );
        let status = harness
            .service
            .status(&GameId::mhw(), &ProfileId::new("default"))
            .expect("status");
        assert_eq!(status.status, expected);
        assert_eq!(status.last_error_code.as_deref(), expected_error);
    }
}

#[test]
fn platform_errors_are_cleared_but_scheduler_errors_are_retained() {
    let now = 3_000_000;
    for (protection_enabled, heartbeat, expected_status) in [
        (false, None, SaveBackupBackgroundProtectionStatus::TrayOnly),
        (
            true,
            Some(now),
            SaveBackupBackgroundProtectionStatus::Protected,
        ),
    ] {
        let mut stale_platform = sample_state(true, protection_enabled, heartbeat);
        stale_platform.last_error_code =
            Some("save_backup_background_configuration_drift".to_owned());
        let harness = Harness::new(
            now,
            SaveBackupBackgroundRegistrationStatus::Registered,
            Some(stale_platform),
        );
        let status = harness
            .service
            .status(&GameId::mhw(), &ProfileId::new("default"))
            .expect("status");
        assert_eq!(status.status, expected_status);
        assert_eq!(status.last_error_code, None);

        let mut scheduler_error = sample_state(true, protection_enabled, heartbeat);
        scheduler_error.last_error_code = Some("save_backup_auto_skipped_game_running".to_owned());
        let harness = Harness::new(
            now,
            SaveBackupBackgroundRegistrationStatus::Registered,
            Some(scheduler_error),
        );
        let status = harness
            .service
            .status(&GameId::mhw(), &ProfileId::new("default"))
            .expect("status");
        assert_eq!(status.status, expected_status);
        assert_eq!(
            status.last_error_code.as_deref(),
            Some("save_backup_auto_skipped_game_running")
        );
    }
}

#[test]
fn registry_errors_map_to_registration_failed_without_raw_details() {
    for error in [
        SaveBackupBackgroundRegistryError::TaskOwnershipConflict,
        SaveBackupBackgroundRegistryError::WorkerBinaryUnavailable,
        SaveBackupBackgroundRegistryError::CommandTimeout,
        SaveBackupBackgroundRegistryError::CommandInvalidOutput,
        SaveBackupBackgroundRegistryError::OperationFailed,
    ] {
        let registry = Arc::new(FakeRegistry::for_inspect(Err(error)));
        let service = service_with(
            registry,
            Some(sample_state(true, true, Some(3_000_000))),
            Arc::new(RecordingAuditLog::default()),
            Arc::new(FixedClock::new(3_000_000)),
        );

        let status = service
            .status(&GameId::mhw(), &ProfileId::new("default"))
            .expect("status");

        assert_eq!(
            status.status,
            SaveBackupBackgroundProtectionStatus::RegistrationFailed
        );
        assert_eq!(status.last_error_code.as_deref(), Some(error.code()));
    }
}

#[test]
fn status_maps_repository_and_clock_failures_to_stable_service_errors() {
    let registry = Arc::new(FakeRegistry::for_inspect(Ok(
        SaveBackupBackgroundRegistrationStatus::Registered,
    )));
    let repository = Arc::new(FakeSchedulerRepository::failing());
    let service = SaveBackupBackgroundService::new(
        registry.clone(),
        repository,
        Arc::new(RecordingAuditLog::default()),
        Arc::new(FixedClock::new(3_000_000)),
    );
    let error = service
        .status(&GameId::mhw(), &ProfileId::new("default"))
        .expect_err("repository failure");
    assert_eq!(
        error,
        SaveBackupBackgroundServiceError::SchedulerStateUnavailable
    );
    assert_eq!(error.code(), "save_backup_scheduler_unavailable");
    assert!(registry.calls().is_empty());

    let service = service_with(
        registry,
        Some(sample_state(true, true, Some(3_000_000))),
        Arc::new(RecordingAuditLog::default()),
        Arc::new(FixedClock::failing()),
    );
    let error = service
        .status(&GameId::mhw(), &ProfileId::new("default"))
        .expect_err("clock failure");
    assert_eq!(error, SaveBackupBackgroundServiceError::ClockUnavailable);
    assert_eq!(error.code(), "save_backup_clock_unavailable");
}

#[test]
fn register_and_unregister_require_expected_operation_and_readback() {
    let registry = Arc::new(FakeRegistry::new(
        vec![
            Ok(SaveBackupBackgroundRegistrationStatus::Registered),
            Ok(SaveBackupBackgroundRegistrationStatus::NotRegistered),
        ],
        vec![Ok(SaveBackupBackgroundRegistrationStatus::Registered)],
        vec![Ok(SaveBackupBackgroundRegistrationStatus::NotRegistered)],
    ));
    let audit = Arc::new(RecordingAuditLog::default());
    let service = service_with(
        registry.clone(),
        None,
        audit.clone(),
        Arc::new(FixedClock::new(3_000_000)),
    );

    assert_eq!(
        service.register().expect("register"),
        SaveBackupBackgroundRegistrationResult {
            status: SaveBackupBackgroundRegistrationStatus::Registered,
            error_code: None,
        }
    );
    assert_eq!(
        service.unregister().expect("unregister"),
        SaveBackupBackgroundRegistrationResult {
            status: SaveBackupBackgroundRegistrationStatus::NotRegistered,
            error_code: None,
        }
    );
    assert_eq!(
        registry.calls(),
        vec!["register", "inspect", "unregister", "inspect"]
    );
    let events = audit.events();
    assert_eq!(events.len(), 2);
    assert!(events.iter().all(|event| event.result == "success"));
}

#[test]
fn register_preserves_stable_operation_and_readback_failures() {
    let cases = [
        (
            SaveBackupBackgroundRegistrationStatus::ConfigurationDrift,
            "save_backup_background_configuration_drift",
        ),
        (
            SaveBackupBackgroundRegistrationStatus::PermissionRequired,
            "save_backup_background_permission_required",
        ),
        (
            SaveBackupBackgroundRegistrationStatus::UnsupportedPlatform,
            "save_backup_background_unsupported_platform",
        ),
    ];

    for (status, expected_code) in cases {
        let operation_registry =
            Arc::new(FakeRegistry::new(Vec::new(), vec![Ok(status)], Vec::new()));
        let operation = service_with(
            operation_registry.clone(),
            None,
            Arc::new(RecordingAuditLog::default()),
            Arc::new(FixedClock::new(3_000_000)),
        )
        .register()
        .expect("operation result");
        assert_eq!(operation.status, status);
        assert_eq!(operation.error_code.as_deref(), Some(expected_code));
        assert_eq!(operation_registry.calls(), vec!["register"]);

        let readback_registry = Arc::new(FakeRegistry::new(
            vec![Ok(status)],
            vec![Ok(SaveBackupBackgroundRegistrationStatus::Registered)],
            Vec::new(),
        ));
        let readback = service_with(
            readback_registry.clone(),
            None,
            Arc::new(RecordingAuditLog::default()),
            Arc::new(FixedClock::new(3_000_000)),
        )
        .register()
        .expect("readback result");
        assert_eq!(readback.status, status);
        assert_eq!(readback.error_code.as_deref(), Some(expected_code));
        assert_eq!(readback_registry.calls(), vec!["register", "inspect"]);
    }
}

#[test]
fn unregister_readback_mismatch_is_always_generic_failure() {
    for readback in [
        SaveBackupBackgroundRegistrationStatus::Registered,
        SaveBackupBackgroundRegistrationStatus::ConfigurationDrift,
        SaveBackupBackgroundRegistrationStatus::PermissionRequired,
        SaveBackupBackgroundRegistrationStatus::UnsupportedPlatform,
    ] {
        let registry = Arc::new(FakeRegistry::new(
            vec![Ok(readback)],
            Vec::new(),
            vec![Ok(SaveBackupBackgroundRegistrationStatus::NotRegistered)],
        ));
        let result = service_with(
            registry,
            None,
            Arc::new(RecordingAuditLog::default()),
            Arc::new(FixedClock::new(3_000_000)),
        )
        .unregister()
        .expect("unregister result");

        assert_eq!(
            result.status,
            SaveBackupBackgroundRegistrationStatus::RegistrationFailed
        );
        assert_eq!(
            result.error_code.as_deref(),
            Some("save_backup_background_registration_failed")
        );
    }
}

#[test]
fn lifecycle_typed_errors_keep_codes_and_audit_only_whitelisted_fields() {
    for error in [
        SaveBackupBackgroundRegistryError::TaskOwnershipConflict,
        SaveBackupBackgroundRegistryError::WorkerBinaryUnavailable,
        SaveBackupBackgroundRegistryError::CommandTimeout,
        SaveBackupBackgroundRegistryError::CommandInvalidOutput,
        SaveBackupBackgroundRegistryError::OperationFailed,
    ] {
        let registry = Arc::new(FakeRegistry::new(Vec::new(), vec![Err(error)], Vec::new()));
        let audit = Arc::new(RecordingAuditLog::default());
        let result = service_with(
            registry,
            None,
            audit.clone(),
            Arc::new(FixedClock::new(3_000_000)),
        )
        .register()
        .expect("typed error result");

        assert_eq!(
            result.status,
            SaveBackupBackgroundRegistrationStatus::RegistrationFailed
        );
        assert_eq!(result.error_code.as_deref(), Some(error.code()));
        let events = audit.events();
        assert_eq!(events.len(), 1);
        let event = &events[0];
        assert_eq!(event.category, "save_backup");
        assert_eq!(event.operation, "background_registration");
        assert_eq!(event.result, "failure");
        assert_eq!(
            event.fields.keys().map(String::as_str).collect::<Vec<_>>(),
            vec!["error_code", "registration_status", "task_schema_version"]
        );
        let serialized = format!("{event:?}");
        for forbidden in ["C:\\", "S-1-5-", "powershell.exe", "<Task"] {
            assert!(!serialized.contains(forbidden));
        }
    }
}

#[test]
fn clock_failure_prevents_registry_change_and_audit_failure_requires_reinspection() {
    let registry = Arc::new(FakeRegistry::new(
        Vec::new(),
        vec![Ok(SaveBackupBackgroundRegistrationStatus::Registered)],
        Vec::new(),
    ));
    let service = service_with(
        registry.clone(),
        None,
        Arc::new(RecordingAuditLog::default()),
        Arc::new(FixedClock::failing()),
    );
    assert_eq!(
        service.register().expect_err("clock failure"),
        SaveBackupBackgroundServiceError::ClockUnavailable
    );
    assert!(registry.calls().is_empty());

    let registry = Arc::new(FakeRegistry::new(
        vec![Ok(SaveBackupBackgroundRegistrationStatus::Registered)],
        vec![Ok(SaveBackupBackgroundRegistrationStatus::Registered)],
        Vec::new(),
    ));
    let service = service_with(
        registry.clone(),
        None,
        Arc::new(RecordingAuditLog::failing()),
        Arc::new(FixedClock::new(3_000_000)),
    );
    let error = service.register().expect_err("audit failure");
    assert_eq!(error, SaveBackupBackgroundServiceError::AuditUnavailable);
    assert_eq!(error.code(), "save_backup_background_audit_unavailable");
    assert_eq!(registry.calls(), vec!["register", "inspect"]);
}

struct Harness {
    service: SaveBackupBackgroundService,
    registry: Arc<FakeRegistry>,
}

impl Harness {
    fn new(
        now: u128,
        registration: SaveBackupBackgroundRegistrationStatus,
        state: Option<SaveBackupSchedulerState>,
    ) -> Self {
        let registry = Arc::new(FakeRegistry::for_inspect(Ok(registration)));
        Self {
            service: service_with(
                registry.clone(),
                state,
                Arc::new(RecordingAuditLog::default()),
                Arc::new(FixedClock::new(now)),
            ),
            registry,
        }
    }
}

fn service_with(
    registry: Arc<FakeRegistry>,
    state: Option<SaveBackupSchedulerState>,
    audit: Arc<RecordingAuditLog>,
    clock: Arc<FixedClock>,
) -> SaveBackupBackgroundService {
    SaveBackupBackgroundService::new(
        registry,
        Arc::new(FakeSchedulerRepository::with_state(state)),
        audit,
        clock,
    )
}

fn sample_state(
    enabled: bool,
    background_protection_enabled: bool,
    worker_heartbeat_at: Option<u128>,
) -> SaveBackupSchedulerState {
    SaveBackupSchedulerState {
        game_id: GameId::mhw(),
        profile_id: ProfileId::new("default"),
        enabled,
        background_protection_enabled,
        background_status: SaveBackupBackgroundProtectionStatus::TrayOnly,
        last_checked_at: Some(1_000),
        last_attempt_at: None,
        last_success_at: None,
        next_due_at: Some(4_000_000),
        pending_reason: None,
        last_error_code: None,
        worker_instance_id: worker_heartbeat_at.map(|_| "worker-a".to_owned()),
        worker_heartbeat_at,
        lease_owner: None,
        lease_expires_at: None,
        updated_at: 1_000,
    }
}

struct FixedClock {
    now: u128,
    fail: bool,
}

impl FixedClock {
    fn new(now: u128) -> Self {
        Self { now, fail: false }
    }

    fn failing() -> Self {
        Self { now: 0, fail: true }
    }
}

impl AppClock for FixedClock {
    fn now_unix_millis(&self) -> Result<u128> {
        if self.fail {
            anyhow::bail!("clock unavailable");
        }
        Ok(self.now)
    }
}

struct FakeRegistry {
    inspect_results:
        Mutex<VecDeque<SaveBackupBackgroundRegistryResult<SaveBackupBackgroundRegistrationStatus>>>,
    register_results:
        Mutex<VecDeque<SaveBackupBackgroundRegistryResult<SaveBackupBackgroundRegistrationStatus>>>,
    unregister_results:
        Mutex<VecDeque<SaveBackupBackgroundRegistryResult<SaveBackupBackgroundRegistrationStatus>>>,
    calls: Mutex<Vec<&'static str>>,
}

impl FakeRegistry {
    fn new(
        inspect_results: Vec<
            SaveBackupBackgroundRegistryResult<SaveBackupBackgroundRegistrationStatus>,
        >,
        register_results: Vec<
            SaveBackupBackgroundRegistryResult<SaveBackupBackgroundRegistrationStatus>,
        >,
        unregister_results: Vec<
            SaveBackupBackgroundRegistryResult<SaveBackupBackgroundRegistrationStatus>,
        >,
    ) -> Self {
        Self {
            inspect_results: Mutex::new(inspect_results.into()),
            register_results: Mutex::new(register_results.into()),
            unregister_results: Mutex::new(unregister_results.into()),
            calls: Mutex::new(Vec::new()),
        }
    }

    fn for_inspect(
        result: SaveBackupBackgroundRegistryResult<SaveBackupBackgroundRegistrationStatus>,
    ) -> Self {
        Self::new(vec![result], Vec::new(), Vec::new())
    }

    fn calls(&self) -> Vec<&'static str> {
        self.calls.lock().expect("calls lock").clone()
    }
}

impl SaveBackupBackgroundRegistry for FakeRegistry {
    fn inspect(
        &self,
    ) -> SaveBackupBackgroundRegistryResult<SaveBackupBackgroundRegistrationStatus> {
        self.calls.lock().expect("calls lock").push("inspect");
        self.inspect_results
            .lock()
            .expect("inspect lock")
            .pop_front()
            .expect("inspect result")
    }

    fn register(
        &self,
    ) -> SaveBackupBackgroundRegistryResult<SaveBackupBackgroundRegistrationStatus> {
        self.calls.lock().expect("calls lock").push("register");
        self.register_results
            .lock()
            .expect("register lock")
            .pop_front()
            .expect("register result")
    }

    fn unregister(
        &self,
    ) -> SaveBackupBackgroundRegistryResult<SaveBackupBackgroundRegistrationStatus> {
        self.calls.lock().expect("calls lock").push("unregister");
        self.unregister_results
            .lock()
            .expect("unregister lock")
            .pop_front()
            .expect("unregister result")
    }
}

#[derive(Default)]
struct RecordingAuditLog {
    events: Mutex<Vec<AuditLogEvent>>,
    fail: bool,
}

impl RecordingAuditLog {
    fn failing() -> Self {
        Self {
            events: Mutex::new(Vec::new()),
            fail: true,
        }
    }

    fn events(&self) -> Vec<AuditLogEvent> {
        self.events.lock().expect("audit lock").clone()
    }
}

impl AuditLogWriter for RecordingAuditLog {
    fn record(&self, event: AuditLogEvent) -> Result<()> {
        if self.fail {
            anyhow::bail!("audit unavailable");
        }
        self.events.lock().expect("audit lock").push(event);
        Ok(())
    }
}

struct FakeSchedulerRepository {
    state: Mutex<Option<SaveBackupSchedulerState>>,
    fail_get: bool,
}

impl FakeSchedulerRepository {
    fn with_state(state: Option<SaveBackupSchedulerState>) -> Self {
        Self {
            state: Mutex::new(state),
            fail_get: false,
        }
    }

    fn failing() -> Self {
        Self {
            state: Mutex::new(None),
            fail_get: true,
        }
    }
}

impl SaveBackupSchedulerStateRepository for FakeSchedulerRepository {
    fn get_state(
        &self,
        _game_id: &GameId,
        _profile_id: &ProfileId,
    ) -> Result<Option<SaveBackupSchedulerState>> {
        if self.fail_get {
            anyhow::bail!("state unavailable");
        }
        Ok(self.state.lock().expect("state lock").clone())
    }

    fn upsert_state(&self, _state: &SaveBackupSchedulerState) -> Result<()> {
        panic!("unused")
    }

    fn acquire_due_lease(
        &self,
        _request: SaveBackupSchedulerLeaseRequest,
    ) -> Result<Option<SaveBackupSchedulerState>> {
        panic!("unused")
    }

    fn release_lease(
        &self,
        _game_id: &GameId,
        _profile_id: &ProfileId,
        _lease_owner: &str,
    ) -> Result<()> {
        panic!("unused")
    }

    fn record_worker_heartbeat(&self, _heartbeat: SaveBackupWorkerHeartbeat) -> Result<()> {
        panic!("unused")
    }
}
