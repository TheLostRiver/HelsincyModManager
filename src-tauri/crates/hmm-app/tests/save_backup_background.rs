use anyhow::Result;
use hmm_app::{
    SaveBackupBackgroundRegistrationResult, SaveBackupBackgroundService,
    SaveBackupBackgroundServiceError, SAVE_BACKUP_BACKGROUND_HEARTBEAT_TTL_MILLIS,
    SAVE_BACKUP_BACKGROUND_STARTUP_GRACE_MILLIS,
};
use hmm_core::{
    GameId, ProfileId, SaveBackupBackgroundProtectionStatus,
    SaveBackupBackgroundRegistrationStatus, SaveBackupBackgroundSettings,
    SaveBackupSchedulerLeaseRequest, SaveBackupSchedulerState, SaveBackupWorkerHeartbeat,
};
use hmm_ports::{
    AppClock, AuditLogEvent, AuditLogWriter, SaveBackupBackgroundRegistry,
    SaveBackupBackgroundRegistryError, SaveBackupBackgroundRegistryResult,
    SaveBackupBackgroundSettingsRepository, SaveBackupSchedulerStateRepository,
};
use std::collections::VecDeque;
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::Duration;

#[test]
fn exact_registration_waits_for_current_enable_heartbeat() {
    let enabled_at = 1_000_000;
    const {
        assert!(SAVE_BACKUP_BACKGROUND_STARTUP_GRACE_MILLIS > 15 * 60_000);
    }
    let harness = ControlHarness::with_global_settings(
        enabled_at + SAVE_BACKUP_BACKGROUND_STARTUP_GRACE_MILLIS,
        SaveBackupBackgroundSettings {
            desired_enabled: true,
            enabled_at: Some(enabled_at),
            last_worker_heartbeat_at: None,
            updated_at: enabled_at,
        },
        SaveBackupBackgroundRegistrationStatus::Registered,
    );

    assert_eq!(
        harness
            .service
            .control_status()
            .expect("starting status")
            .status,
        SaveBackupBackgroundProtectionStatus::Starting
    );

    harness
        .clock
        .set(enabled_at + SAVE_BACKUP_BACKGROUND_STARTUP_GRACE_MILLIS + 1);
    assert_eq!(
        harness
            .service
            .control_status()
            .expect("unhealthy status")
            .status,
        SaveBackupBackgroundProtectionStatus::WorkerUnhealthy
    );
}

#[test]
fn global_heartbeat_health_uses_enable_time_ttl_and_future_boundaries() {
    let enabled_at = 1_000_000;
    let grace = SAVE_BACKUP_BACKGROUND_STARTUP_GRACE_MILLIS;
    let ttl = SAVE_BACKUP_BACKGROUND_HEARTBEAT_TTL_MILLIS;
    let cases = [
        (
            enabled_at + grace,
            Some(enabled_at - 1),
            SaveBackupBackgroundProtectionStatus::Starting,
        ),
        (
            enabled_at + grace + 1,
            Some(enabled_at - 1),
            SaveBackupBackgroundProtectionStatus::WorkerUnhealthy,
        ),
        (
            enabled_at + ttl,
            Some(enabled_at),
            SaveBackupBackgroundProtectionStatus::Protected,
        ),
        (
            enabled_at + ttl + 1,
            Some(enabled_at),
            SaveBackupBackgroundProtectionStatus::WorkerUnhealthy,
        ),
        (
            enabled_at + 10,
            Some(enabled_at + 11),
            SaveBackupBackgroundProtectionStatus::WorkerUnhealthy,
        ),
    ];

    for (now, heartbeat, expected) in cases {
        let harness = ControlHarness::with_global_settings(
            now,
            SaveBackupBackgroundSettings {
                desired_enabled: true,
                enabled_at: Some(enabled_at),
                last_worker_heartbeat_at: heartbeat,
                updated_at: heartbeat.unwrap_or(enabled_at),
            },
            SaveBackupBackgroundRegistrationStatus::Registered,
        );
        let status = harness.service.control_status().expect("control status");
        assert_eq!(status.status, expected);
        assert!(status.desired_enabled);
        assert_eq!(status.enabled_at, Some(enabled_at));
        assert_eq!(status.last_heartbeat_at, heartbeat);
        assert_eq!(
            status.last_error_code.as_deref(),
            (expected == SaveBackupBackgroundProtectionStatus::WorkerUnhealthy)
                .then_some("save_backup_background_worker_unhealthy")
        );
    }

    let future_enable = ControlHarness::with_global_settings(
        enabled_at - 1,
        SaveBackupBackgroundSettings {
            desired_enabled: true,
            enabled_at: Some(enabled_at),
            last_worker_heartbeat_at: None,
            updated_at: enabled_at,
        },
        SaveBackupBackgroundRegistrationStatus::Registered,
    );
    assert_eq!(
        future_enable
            .service
            .control_status()
            .expect("future enable status")
            .status,
        SaveBackupBackgroundProtectionStatus::WorkerUnhealthy
    );
}

#[test]
fn global_control_status_maps_registration_and_dependency_failures() {
    let now = 1_000_000;
    let settings = SaveBackupBackgroundSettings {
        desired_enabled: true,
        enabled_at: Some(now),
        last_worker_heartbeat_at: None,
        updated_at: now,
    };
    for (registration, expected, expected_error) in [
        (
            SaveBackupBackgroundRegistrationStatus::ConfigurationDrift,
            SaveBackupBackgroundProtectionStatus::RegistrationFailed,
            "save_backup_background_configuration_drift",
        ),
        (
            SaveBackupBackgroundRegistrationStatus::PermissionRequired,
            SaveBackupBackgroundProtectionStatus::PermissionRequired,
            "save_backup_background_permission_required",
        ),
        (
            SaveBackupBackgroundRegistrationStatus::UnsupportedPlatform,
            SaveBackupBackgroundProtectionStatus::UnsupportedPlatform,
            "save_backup_background_unsupported_platform",
        ),
    ] {
        let harness = ControlHarness::with_global_settings(now, settings.clone(), registration);
        let status = harness.service.control_status().expect("control status");
        assert_eq!(status.status, expected);
        assert_eq!(status.last_error_code.as_deref(), Some(expected_error));
    }

    let registry = Arc::new(FakeRegistry::for_inspect(Err(
        SaveBackupBackgroundRegistryError::CommandTimeout,
    )));
    let service = service_with_global_settings(
        registry,
        Arc::new(FakeBackgroundSettingsRepository::with_state(
            settings.clone(),
        )),
        Arc::new(RecordingAuditLog::default()),
        Arc::new(FixedClock::new(now)),
    );
    let status = service.control_status().expect("registry error status");
    assert_eq!(
        status.status,
        SaveBackupBackgroundProtectionStatus::RegistrationFailed
    );
    assert_eq!(
        status.last_error_code.as_deref(),
        Some("save_backup_background_command_timeout")
    );

    let registry = Arc::new(FakeRegistry::for_inspect(Ok(
        SaveBackupBackgroundRegistrationStatus::Registered,
    )));
    let service = service_with_global_settings(
        Arc::clone(&registry),
        Arc::new(FakeBackgroundSettingsRepository::failing_load()),
        Arc::new(RecordingAuditLog::default()),
        Arc::new(FixedClock::new(now)),
    );
    let error = service.control_status().expect_err("settings load failure");
    assert_eq!(error, SaveBackupBackgroundServiceError::SettingsUnavailable);
    assert_eq!(error.code(), "save_backup_background_settings_unavailable");
    assert!(registry.calls().is_empty());

    let registry = Arc::new(FakeRegistry::new(
        vec![Ok(SaveBackupBackgroundRegistrationStatus::Registered)],
        Vec::new(),
        Vec::new(),
    ));
    let service = service_with_global_settings(
        Arc::clone(&registry),
        Arc::new(FakeBackgroundSettingsRepository::with_state(settings)),
        Arc::new(RecordingAuditLog::default()),
        Arc::new(FixedClock::failing()),
    );
    assert_eq!(
        service.control_status().expect_err("clock failure"),
        SaveBackupBackgroundServiceError::ClockUnavailable
    );
    assert_eq!(registry.calls(), vec!["inspect"]);

    let registry = Arc::new(FakeRegistry::for_inspect(Ok(
        SaveBackupBackgroundRegistrationStatus::Registered,
    )));
    let service = service_with_global_settings(
        Arc::clone(&registry),
        Arc::new(FakeBackgroundSettingsRepository::with_state(
            SaveBackupBackgroundSettings::disabled(),
        )),
        Arc::new(RecordingAuditLog::default()),
        Arc::new(FixedClock::failing()),
    );
    assert_eq!(
        service.control_status().expect("disabled status").status,
        SaveBackupBackgroundProtectionStatus::NotEnabled
    );
    assert!(registry.calls().is_empty());
}

#[test]
fn enable_persists_intent_before_register_and_returns_starting() {
    let now = 2_000_000;
    let harness = OperationHarness::new(
        now,
        SaveBackupBackgroundSettings {
            desired_enabled: true,
            enabled_at: Some(1_000_000),
            last_worker_heartbeat_at: Some(1_100_000),
            updated_at: 1_100_000,
        },
        Vec::new(),
        vec![Ok(SaveBackupBackgroundRegistrationStatus::Registered)],
        Vec::new(),
    );

    let result = harness.service.enable().expect("enable");

    assert!(result.desired_enabled);
    assert_eq!(
        result.status,
        SaveBackupBackgroundProtectionStatus::Starting
    );
    assert_eq!(result.enabled_at, Some(now));
    assert_eq!(result.last_heartbeat_at, None);
    assert_eq!(result.last_error_code, None);
    assert_eq!(
        harness.calls(),
        vec!["settings.begin_enable", "registry.register", "audit.record",]
    );
    assert_eq!(
        harness.settings.state(),
        SaveBackupBackgroundSettings {
            desired_enabled: true,
            enabled_at: Some(now),
            last_worker_heartbeat_at: None,
            updated_at: now,
        }
    );
    assert_registration_audit(&harness.audit.events()[0], "registered", "success", None);
}

#[test]
fn disable_confirms_task_missing_before_persisting_disabled() {
    let now = 2_000_000;
    let harness = OperationHarness::new(
        now,
        enabled_settings(1_000_000, Some(1_100_000)),
        Vec::new(),
        Vec::new(),
        vec![Ok(SaveBackupBackgroundRegistrationStatus::NotRegistered)],
    );

    let result = harness.service.disable().expect("disable");

    assert_eq!(
        result,
        hmm_app::SaveBackupBackgroundControlStatus {
            desired_enabled: false,
            status: SaveBackupBackgroundProtectionStatus::NotEnabled,
            enabled_at: None,
            last_heartbeat_at: None,
            last_error_code: None,
        }
    );
    assert_eq!(
        harness.calls(),
        vec![
            "registry.unregister",
            "settings.finish_disable",
            "audit.record",
        ]
    );
    assert_eq!(
        harness.settings.state(),
        SaveBackupBackgroundSettings {
            desired_enabled: false,
            enabled_at: None,
            last_worker_heartbeat_at: None,
            updated_at: now,
        }
    );
    assert_registration_audit(
        &harness.audit.events()[0],
        "not_registered",
        "success",
        None,
    );
}

#[test]
fn enable_waits_for_in_flight_disable_transition() {
    let now = 2_000_000;
    let shared_calls = Arc::new(Mutex::new(Vec::new()));
    let registry = Arc::new(FakeRegistry::with_shared_calls(
        Vec::new(),
        vec![Ok(SaveBackupBackgroundRegistrationStatus::Registered)],
        vec![Ok(SaveBackupBackgroundRegistrationStatus::NotRegistered)],
        Arc::clone(&shared_calls),
    ));
    let (finish_entered_tx, finish_entered_rx) = mpsc::channel();
    let (finish_release_tx, finish_release_rx) = mpsc::channel();
    let (begin_entered_tx, begin_entered_rx) = mpsc::channel();
    let settings = Arc::new(
        FakeBackgroundSettingsRepository::with_shared_calls_and_transition_gate(
            enabled_settings(1_000_000, Some(1_100_000)),
            Arc::clone(&shared_calls),
            finish_entered_tx,
            finish_release_rx,
            begin_entered_tx,
        ),
    );
    let service = Arc::new(SaveBackupBackgroundService::new_with_settings_repository(
        registry,
        Arc::new(FakeSchedulerRepository::with_state(None)),
        settings.clone(),
        Arc::new(RecordingAuditLog::with_shared_calls(Arc::clone(
            &shared_calls,
        ))),
        Arc::new(FixedClock::new(now)),
    ));

    let disable_service = Arc::clone(&service);
    let disable_thread = thread::spawn(move || disable_service.disable());
    finish_entered_rx
        .recv_timeout(Duration::from_secs(1))
        .expect("disable reaches final settings write");

    let enable_service = Arc::clone(&service);
    let enable_thread = thread::spawn(move || enable_service.enable());
    let enable_entered_before_disable_finished = begin_entered_rx
        .recv_timeout(Duration::from_millis(100))
        .is_ok();

    finish_release_tx
        .send(())
        .expect("release final disable write");
    let disabled = disable_thread
        .join()
        .expect("disable thread")
        .expect("disable transition");
    if !enable_entered_before_disable_finished {
        begin_entered_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("enable starts after disable finishes");
    }
    let enabled = enable_thread
        .join()
        .expect("enable thread")
        .expect("enable transition");

    assert!(
        !enable_entered_before_disable_finished,
        "enable must not enter while disable owns the transition"
    );
    assert_eq!(
        disabled.status,
        SaveBackupBackgroundProtectionStatus::NotEnabled
    );
    assert_eq!(
        enabled.status,
        SaveBackupBackgroundProtectionStatus::Starting
    );
    assert!(settings.state().desired_enabled);
    assert_eq!(
        shared_calls.lock().expect("shared calls lock").as_slice(),
        [
            "registry.unregister",
            "settings.finish_disable",
            "audit.record",
            "settings.begin_enable",
            "registry.register",
            "audit.record",
        ]
    );
}

#[test]
fn lifecycle_failures_preserve_recoverable_global_intent() {
    let now = 2_000_000;
    let enable = OperationHarness::new(
        now,
        SaveBackupBackgroundSettings::disabled(),
        Vec::new(),
        vec![Err(SaveBackupBackgroundRegistryError::CommandTimeout)],
        Vec::new(),
    );
    let enabled = enable.service.enable().expect("typed enable failure");
    assert!(enabled.desired_enabled);
    assert_eq!(
        enabled.status,
        SaveBackupBackgroundProtectionStatus::RegistrationFailed
    );
    assert_eq!(
        enabled.last_error_code.as_deref(),
        Some("save_backup_background_command_timeout")
    );
    assert!(enable.settings.state().desired_enabled);
    assert_eq!(
        enable.calls(),
        vec!["settings.begin_enable", "registry.register", "audit.record",]
    );

    let disable = OperationHarness::new(
        now,
        enabled_settings(1_000_000, Some(1_100_000)),
        Vec::new(),
        Vec::new(),
        vec![Ok(SaveBackupBackgroundRegistrationStatus::Registered)],
    );
    let still_enabled = disable
        .service
        .disable()
        .expect("read-back mismatch is recoverable");
    assert!(still_enabled.desired_enabled);
    assert_eq!(
        still_enabled.status,
        SaveBackupBackgroundProtectionStatus::RegistrationFailed
    );
    assert_eq!(
        still_enabled.last_error_code.as_deref(),
        Some("save_backup_background_registration_failed")
    );
    assert!(disable.settings.state().desired_enabled);
    assert_eq!(
        disable.calls(),
        vec!["registry.unregister", "audit.record",]
    );
}

#[test]
fn per_profile_status_combines_auto_plan_with_global_control() {
    let now = 3_000_000;
    let mut scheduler_state = sample_state(true, false, None);
    scheduler_state.last_error_code = Some("save_backup_auto_skipped_game_running".to_owned());
    let disabled_registry = Arc::new(FakeRegistry::for_inspect(Ok(
        SaveBackupBackgroundRegistrationStatus::Registered,
    )));
    let service = service_with_global_settings_and_scheduler_state(
        Arc::clone(&disabled_registry),
        Arc::new(FakeBackgroundSettingsRepository::with_state(
            SaveBackupBackgroundSettings::disabled(),
        )),
        Arc::new(RecordingAuditLog::default()),
        Arc::new(FixedClock::new(now)),
        Some(scheduler_state.clone()),
    );
    let tray_only = service
        .status(&GameId::mhw(), &ProfileId::new("default"))
        .expect("tray-only profile status");
    assert_eq!(
        tray_only.status,
        SaveBackupBackgroundProtectionStatus::TrayOnly
    );
    assert_eq!(
        tray_only.last_error_code.as_deref(),
        Some("save_backup_auto_skipped_game_running")
    );
    assert!(disabled_registry.calls().is_empty());

    let protected_registry = Arc::new(FakeRegistry::for_inspect(Ok(
        SaveBackupBackgroundRegistrationStatus::Registered,
    )));
    let service = service_with_global_settings_and_scheduler_state(
        protected_registry,
        Arc::new(FakeBackgroundSettingsRepository::with_state(
            enabled_settings(1_000_000, Some(now)),
        )),
        Arc::new(RecordingAuditLog::default()),
        Arc::new(FixedClock::new(now)),
        Some(scheduler_state),
    );
    let protected = service
        .status(&GameId::mhw(), &ProfileId::new("default"))
        .expect("protected profile status");
    assert_eq!(
        protected.status,
        SaveBackupBackgroundProtectionStatus::Protected
    );
    assert_eq!(
        protected.last_error_code.as_deref(),
        Some("save_backup_auto_skipped_game_running")
    );
}

#[test]
fn lifecycle_dependency_failures_do_not_claim_success() {
    let now = 2_000_000;

    let calls = Arc::new(Mutex::new(Vec::new()));
    let registry = Arc::new(FakeRegistry::with_shared_calls(
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Arc::clone(&calls),
    ));
    let settings = Arc::new(
        FakeBackgroundSettingsRepository::with_shared_calls_and_failures(
            SaveBackupBackgroundSettings::disabled(),
            Arc::clone(&calls),
            true,
            false,
        ),
    );
    let audit = Arc::new(RecordingAuditLog::with_shared_calls(Arc::clone(&calls)));
    let service = service_with_global_settings(
        Arc::clone(&registry),
        Arc::clone(&settings),
        Arc::clone(&audit),
        Arc::new(FixedClock::new(now)),
    );
    assert_eq!(
        service.enable().expect_err("begin-enable failure"),
        SaveBackupBackgroundServiceError::SettingsUnavailable
    );
    assert_eq!(
        calls.lock().expect("shared calls lock").as_slice(),
        ["settings.begin_enable"]
    );
    assert_eq!(settings.state(), SaveBackupBackgroundSettings::disabled());
    assert!(registry.calls().is_empty());
    assert!(audit.events().is_empty());

    let calls = Arc::new(Mutex::new(Vec::new()));
    let registry = Arc::new(FakeRegistry::with_shared_calls(
        Vec::new(),
        Vec::new(),
        vec![Ok(SaveBackupBackgroundRegistrationStatus::NotRegistered)],
        Arc::clone(&calls),
    ));
    let settings = Arc::new(
        FakeBackgroundSettingsRepository::with_shared_calls_and_failures(
            enabled_settings(1_000_000, Some(1_100_000)),
            Arc::clone(&calls),
            false,
            true,
        ),
    );
    let audit = Arc::new(RecordingAuditLog::with_shared_calls(Arc::clone(&calls)));
    let service = service_with_global_settings(
        registry,
        Arc::clone(&settings),
        Arc::clone(&audit),
        Arc::new(FixedClock::new(now)),
    );
    assert_eq!(
        service.disable().expect_err("finish-disable failure"),
        SaveBackupBackgroundServiceError::SettingsUnavailable
    );
    assert!(settings.state().desired_enabled);
    assert_eq!(
        calls.lock().expect("shared calls lock").as_slice(),
        [
            "registry.unregister",
            "settings.finish_disable",
            "audit.record",
        ]
    );
    assert_registration_audit(
        &audit.events()[0],
        "not_registered",
        "failure",
        Some("save_backup_background_settings_unavailable"),
    );
}

#[test]
fn lifecycle_clock_and_audit_failures_preserve_observable_state() {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let registry = Arc::new(FakeRegistry::with_shared_calls(
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Arc::clone(&calls),
    ));
    let settings = Arc::new(FakeBackgroundSettingsRepository::with_shared_calls(
        SaveBackupBackgroundSettings::disabled(),
        Arc::clone(&calls),
    ));
    let service = service_with_global_settings(
        Arc::clone(&registry),
        Arc::clone(&settings),
        Arc::new(RecordingAuditLog::with_shared_calls(Arc::clone(&calls))),
        Arc::new(FixedClock::failing()),
    );
    assert_eq!(
        service.enable().expect_err("clock failure"),
        SaveBackupBackgroundServiceError::ClockUnavailable
    );
    assert!(calls.lock().expect("shared calls lock").is_empty());
    assert_eq!(settings.state(), SaveBackupBackgroundSettings::disabled());
    assert!(registry.calls().is_empty());

    let calls = Arc::new(Mutex::new(Vec::new()));
    let registry = Arc::new(FakeRegistry::with_shared_calls(
        Vec::new(),
        vec![Ok(SaveBackupBackgroundRegistrationStatus::Registered)],
        Vec::new(),
        Arc::clone(&calls),
    ));
    let settings = Arc::new(FakeBackgroundSettingsRepository::with_shared_calls(
        SaveBackupBackgroundSettings::disabled(),
        Arc::clone(&calls),
    ));
    let audit = Arc::new(RecordingAuditLog::with_shared_calls_and_failure(
        Arc::clone(&calls),
    ));
    let service = service_with_global_settings(
        registry,
        Arc::clone(&settings),
        audit,
        Arc::new(FixedClock::new(2_000_000)),
    );
    assert_eq!(
        service.enable().expect_err("audit failure"),
        SaveBackupBackgroundServiceError::AuditUnavailable
    );
    assert!(settings.state().desired_enabled);
    assert_eq!(
        calls.lock().expect("shared calls lock").as_slice(),
        ["settings.begin_enable", "registry.register", "audit.record",]
    );
}

fn assert_registration_audit(
    event: &AuditLogEvent,
    registration_status: &str,
    result: &str,
    error_code: Option<&str>,
) {
    assert_eq!(event.category, "save_backup");
    assert_eq!(event.operation, "background_registration");
    assert_eq!(event.result, result);
    assert_eq!(
        event.fields.keys().map(String::as_str).collect::<Vec<_>>(),
        vec!["error_code", "registration_status", "task_schema_version"]
    );
    assert_eq!(
        event.fields.get("registration_status").map(String::as_str),
        Some(registration_status)
    );
    assert_eq!(
        event.fields.get("error_code").map(String::as_str),
        Some(error_code.unwrap_or_default())
    );
}

fn enabled_settings(enabled_at: u128, heartbeat: Option<u128>) -> SaveBackupBackgroundSettings {
    SaveBackupBackgroundSettings {
        desired_enabled: true,
        enabled_at: Some(enabled_at),
        last_worker_heartbeat_at: heartbeat,
        updated_at: heartbeat.unwrap_or(enabled_at),
    }
}

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
fn register_and_unregister_require_verified_postconditions() {
    let registry = Arc::new(FakeRegistry::new(
        Vec::new(),
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
    assert_eq!(registry.calls(), vec!["register", "unregister"]);
    let events = audit.events();
    assert_eq!(events.len(), 2);
    assert!(events.iter().all(|event| event.result == "success"));
}

#[test]
fn register_preserves_stable_postcondition_failures() {
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
    }
}

#[test]
fn unregister_maps_each_non_absent_postcondition_to_a_stable_failure() {
    for (postcondition, expected_status, expected_code) in [
        (
            SaveBackupBackgroundRegistrationStatus::Registered,
            SaveBackupBackgroundRegistrationStatus::RegistrationFailed,
            "save_backup_background_registration_failed",
        ),
        (
            SaveBackupBackgroundRegistrationStatus::ConfigurationDrift,
            SaveBackupBackgroundRegistrationStatus::ConfigurationDrift,
            "save_backup_background_configuration_drift",
        ),
        (
            SaveBackupBackgroundRegistrationStatus::RegistrationFailed,
            SaveBackupBackgroundRegistrationStatus::RegistrationFailed,
            "save_backup_background_registration_failed",
        ),
        (
            SaveBackupBackgroundRegistrationStatus::PermissionRequired,
            SaveBackupBackgroundRegistrationStatus::PermissionRequired,
            "save_backup_background_permission_required",
        ),
        (
            SaveBackupBackgroundRegistrationStatus::UnsupportedPlatform,
            SaveBackupBackgroundRegistrationStatus::UnsupportedPlatform,
            "save_backup_background_unsupported_platform",
        ),
    ] {
        let registry = Arc::new(FakeRegistry::new(
            Vec::new(),
            Vec::new(),
            vec![Ok(postcondition)],
        ));
        let result = service_with(
            registry,
            None,
            Arc::new(RecordingAuditLog::default()),
            Arc::new(FixedClock::new(3_000_000)),
        )
        .unregister()
        .expect("unregister result");

        assert_eq!(result.status, expected_status);
        assert_eq!(result.error_code.as_deref(), Some(expected_code));
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
fn clock_failure_prevents_registry_change_and_audit_failure_does_not_reinspect() {
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
        Vec::new(),
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
    assert_eq!(registry.calls(), vec!["register"]);
}

struct ControlHarness {
    service: SaveBackupBackgroundService,
    clock: Arc<FixedClock>,
}

impl ControlHarness {
    fn with_global_settings(
        now: u128,
        settings: SaveBackupBackgroundSettings,
        registration: SaveBackupBackgroundRegistrationStatus,
    ) -> Self {
        let registry = Arc::new(FakeRegistry::new(
            vec![Ok(registration), Ok(registration)],
            Vec::new(),
            Vec::new(),
        ));
        let clock = Arc::new(FixedClock::new(now));
        Self {
            service: service_with_global_settings(
                registry,
                Arc::new(FakeBackgroundSettingsRepository::with_state(settings)),
                Arc::new(RecordingAuditLog::default()),
                Arc::clone(&clock),
            ),
            clock,
        }
    }
}

struct OperationHarness {
    service: SaveBackupBackgroundService,
    settings: Arc<FakeBackgroundSettingsRepository>,
    audit: Arc<RecordingAuditLog>,
    shared_calls: Arc<Mutex<Vec<&'static str>>>,
}

impl OperationHarness {
    fn new(
        now: u128,
        settings_state: SaveBackupBackgroundSettings,
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
        let shared_calls = Arc::new(Mutex::new(Vec::new()));
        let registry = Arc::new(FakeRegistry::with_shared_calls(
            inspect_results,
            register_results,
            unregister_results,
            Arc::clone(&shared_calls),
        ));
        let settings = Arc::new(FakeBackgroundSettingsRepository::with_shared_calls(
            settings_state,
            Arc::clone(&shared_calls),
        ));
        let audit = Arc::new(RecordingAuditLog::with_shared_calls(Arc::clone(
            &shared_calls,
        )));
        Self {
            service: service_with_global_settings(
                registry,
                Arc::clone(&settings),
                Arc::clone(&audit),
                Arc::new(FixedClock::new(now)),
            ),
            settings,
            audit,
            shared_calls,
        }
    }

    fn calls(&self) -> Vec<&'static str> {
        self.shared_calls.lock().expect("shared calls lock").clone()
    }
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

fn service_with_global_settings(
    registry: Arc<FakeRegistry>,
    settings: Arc<FakeBackgroundSettingsRepository>,
    audit: Arc<RecordingAuditLog>,
    clock: Arc<FixedClock>,
) -> SaveBackupBackgroundService {
    service_with_global_settings_and_scheduler_state(registry, settings, audit, clock, None)
}

fn service_with_global_settings_and_scheduler_state(
    registry: Arc<FakeRegistry>,
    settings: Arc<FakeBackgroundSettingsRepository>,
    audit: Arc<RecordingAuditLog>,
    clock: Arc<FixedClock>,
    scheduler_state: Option<SaveBackupSchedulerState>,
) -> SaveBackupBackgroundService {
    SaveBackupBackgroundService::new_with_settings_repository(
        registry,
        Arc::new(FakeSchedulerRepository::with_state(scheduler_state)),
        settings,
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
    now: Mutex<u128>,
    fail: bool,
}

impl FixedClock {
    fn new(now: u128) -> Self {
        Self {
            now: Mutex::new(now),
            fail: false,
        }
    }

    fn failing() -> Self {
        Self {
            now: Mutex::new(0),
            fail: true,
        }
    }

    fn set(&self, now: u128) {
        *self.now.lock().expect("clock lock") = now;
    }
}

impl AppClock for FixedClock {
    fn now_unix_millis(&self) -> Result<u128> {
        if self.fail {
            anyhow::bail!("clock unavailable");
        }
        Ok(*self.now.lock().expect("clock lock"))
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
    shared_calls: Option<Arc<Mutex<Vec<&'static str>>>>,
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
            shared_calls: None,
        }
    }

    fn with_shared_calls(
        inspect_results: Vec<
            SaveBackupBackgroundRegistryResult<SaveBackupBackgroundRegistrationStatus>,
        >,
        register_results: Vec<
            SaveBackupBackgroundRegistryResult<SaveBackupBackgroundRegistrationStatus>,
        >,
        unregister_results: Vec<
            SaveBackupBackgroundRegistryResult<SaveBackupBackgroundRegistrationStatus>,
        >,
        shared_calls: Arc<Mutex<Vec<&'static str>>>,
    ) -> Self {
        Self {
            inspect_results: Mutex::new(inspect_results.into()),
            register_results: Mutex::new(register_results.into()),
            unregister_results: Mutex::new(unregister_results.into()),
            calls: Mutex::new(Vec::new()),
            shared_calls: Some(shared_calls),
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

    fn record_call(&self, call: &'static str, shared_call: &'static str) {
        self.calls.lock().expect("calls lock").push(call);
        if let Some(shared_calls) = &self.shared_calls {
            shared_calls
                .lock()
                .expect("shared calls lock")
                .push(shared_call);
        }
    }
}

impl SaveBackupBackgroundRegistry for FakeRegistry {
    fn inspect(
        &self,
    ) -> SaveBackupBackgroundRegistryResult<SaveBackupBackgroundRegistrationStatus> {
        self.record_call("inspect", "registry.inspect");
        self.inspect_results
            .lock()
            .expect("inspect lock")
            .pop_front()
            .expect("inspect result")
    }

    fn register(
        &self,
    ) -> SaveBackupBackgroundRegistryResult<SaveBackupBackgroundRegistrationStatus> {
        self.record_call("register", "registry.register");
        self.register_results
            .lock()
            .expect("register lock")
            .pop_front()
            .expect("register result")
    }

    fn unregister(
        &self,
    ) -> SaveBackupBackgroundRegistryResult<SaveBackupBackgroundRegistrationStatus> {
        self.record_call("unregister", "registry.unregister");
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
    shared_calls: Option<Arc<Mutex<Vec<&'static str>>>>,
}

impl RecordingAuditLog {
    fn failing() -> Self {
        Self {
            events: Mutex::new(Vec::new()),
            fail: true,
            shared_calls: None,
        }
    }

    fn with_shared_calls(shared_calls: Arc<Mutex<Vec<&'static str>>>) -> Self {
        Self {
            events: Mutex::new(Vec::new()),
            fail: false,
            shared_calls: Some(shared_calls),
        }
    }

    fn with_shared_calls_and_failure(shared_calls: Arc<Mutex<Vec<&'static str>>>) -> Self {
        Self {
            events: Mutex::new(Vec::new()),
            fail: true,
            shared_calls: Some(shared_calls),
        }
    }

    fn events(&self) -> Vec<AuditLogEvent> {
        self.events.lock().expect("audit lock").clone()
    }
}

impl AuditLogWriter for RecordingAuditLog {
    fn record(&self, event: AuditLogEvent) -> Result<()> {
        if let Some(shared_calls) = &self.shared_calls {
            shared_calls
                .lock()
                .expect("shared calls lock")
                .push("audit.record");
        }
        if self.fail {
            anyhow::bail!("audit unavailable");
        }
        self.events.lock().expect("audit lock").push(event);
        Ok(())
    }
}

struct FakeBackgroundSettingsRepository {
    state: Mutex<SaveBackupBackgroundSettings>,
    fail_load: bool,
    fail_begin: bool,
    fail_finish: bool,
    shared_calls: Option<Arc<Mutex<Vec<&'static str>>>>,
    transition_gate: Option<TransitionGate>,
}

struct TransitionGate {
    finish_entered: mpsc::Sender<()>,
    finish_release: Mutex<mpsc::Receiver<()>>,
    begin_entered: mpsc::Sender<()>,
}

impl FakeBackgroundSettingsRepository {
    fn with_state(state: SaveBackupBackgroundSettings) -> Self {
        Self {
            state: Mutex::new(state),
            fail_load: false,
            fail_begin: false,
            fail_finish: false,
            shared_calls: None,
            transition_gate: None,
        }
    }

    fn with_shared_calls(
        state: SaveBackupBackgroundSettings,
        shared_calls: Arc<Mutex<Vec<&'static str>>>,
    ) -> Self {
        Self {
            state: Mutex::new(state),
            fail_load: false,
            fail_begin: false,
            fail_finish: false,
            shared_calls: Some(shared_calls),
            transition_gate: None,
        }
    }

    fn with_shared_calls_and_transition_gate(
        state: SaveBackupBackgroundSettings,
        shared_calls: Arc<Mutex<Vec<&'static str>>>,
        finish_entered: mpsc::Sender<()>,
        finish_release: mpsc::Receiver<()>,
        begin_entered: mpsc::Sender<()>,
    ) -> Self {
        Self {
            state: Mutex::new(state),
            fail_load: false,
            fail_begin: false,
            fail_finish: false,
            shared_calls: Some(shared_calls),
            transition_gate: Some(TransitionGate {
                finish_entered,
                finish_release: Mutex::new(finish_release),
                begin_entered,
            }),
        }
    }

    fn with_shared_calls_and_failures(
        state: SaveBackupBackgroundSettings,
        shared_calls: Arc<Mutex<Vec<&'static str>>>,
        fail_begin: bool,
        fail_finish: bool,
    ) -> Self {
        Self {
            state: Mutex::new(state),
            fail_load: false,
            fail_begin,
            fail_finish,
            shared_calls: Some(shared_calls),
            transition_gate: None,
        }
    }

    fn failing_load() -> Self {
        Self {
            state: Mutex::new(SaveBackupBackgroundSettings::disabled()),
            fail_load: true,
            fail_begin: false,
            fail_finish: false,
            shared_calls: None,
            transition_gate: None,
        }
    }

    fn state(&self) -> SaveBackupBackgroundSettings {
        self.state.lock().expect("settings lock").clone()
    }

    fn record_call(&self, call: &'static str) {
        if let Some(shared_calls) = &self.shared_calls {
            shared_calls.lock().expect("shared calls lock").push(call);
        }
    }
}

impl SaveBackupBackgroundSettingsRepository for FakeBackgroundSettingsRepository {
    fn load(&self) -> Result<SaveBackupBackgroundSettings> {
        if self.fail_load {
            anyhow::bail!("settings unavailable");
        }
        Ok(self.state.lock().expect("settings lock").clone())
    }

    fn begin_enable(&self, enabled_at: u128) -> Result<()> {
        self.record_call("settings.begin_enable");
        if let Some(gate) = &self.transition_gate {
            gate.begin_entered
                .send(())
                .expect("signal begin enable entry");
        }
        if self.fail_begin {
            anyhow::bail!("begin enable unavailable");
        }
        *self.state.lock().expect("settings lock") = SaveBackupBackgroundSettings {
            desired_enabled: true,
            enabled_at: Some(enabled_at),
            last_worker_heartbeat_at: None,
            updated_at: enabled_at,
        };
        Ok(())
    }

    fn finish_disable(&self, updated_at: u128) -> Result<()> {
        self.record_call("settings.finish_disable");
        if let Some(gate) = &self.transition_gate {
            gate.finish_entered
                .send(())
                .expect("signal finish disable entry");
            gate.finish_release
                .lock()
                .expect("finish release lock")
                .recv()
                .expect("wait for finish disable release");
        }
        if self.fail_finish {
            anyhow::bail!("finish disable unavailable");
        }
        *self.state.lock().expect("settings lock") = SaveBackupBackgroundSettings {
            desired_enabled: false,
            enabled_at: None,
            last_worker_heartbeat_at: None,
            updated_at,
        };
        Ok(())
    }

    fn record_worker_heartbeat(&self, _heartbeat_at: u128) -> Result<()> {
        panic!("unused")
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
