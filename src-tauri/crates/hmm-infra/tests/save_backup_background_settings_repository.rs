use hmm_core::SaveBackupBackgroundSettings;
use hmm_infra::{open_database, SqliteSaveBackupBackgroundSettingsRepository};
use hmm_ports::SaveBackupBackgroundSettingsRepository;
use std::path::Path;
use std::sync::{Arc, Mutex};

#[test]
fn missing_singleton_defaults_to_disabled() {
    let temp = tempfile::tempdir().expect("temp dir");
    let repo = settings_repo_at(&temp.path().join("test.db"));

    assert_eq!(
        repo.load().expect("load default settings"),
        SaveBackupBackgroundSettings::disabled()
    );
}

#[test]
fn begin_enable_round_trips_and_reenable_clears_old_heartbeat() {
    let temp = tempfile::tempdir().expect("temp dir");
    let repo = settings_repo_at(&temp.path().join("test.db"));

    repo.begin_enable(1_000).expect("begin enable");
    assert_eq!(
        repo.load().expect("load enabled settings"),
        SaveBackupBackgroundSettings {
            desired_enabled: true,
            enabled_at: Some(1_000),
            last_worker_heartbeat_at: None,
            updated_at: 1_000,
        }
    );

    repo.record_worker_heartbeat(1_100)
        .expect("record worker heartbeat");
    assert_eq!(
        repo.load().expect("load heartbeat"),
        SaveBackupBackgroundSettings {
            desired_enabled: true,
            enabled_at: Some(1_000),
            last_worker_heartbeat_at: Some(1_100),
            updated_at: 1_100,
        }
    );

    repo.begin_enable(2_000).expect("begin re-enable");
    assert_eq!(
        repo.load().expect("load re-enabled settings"),
        SaveBackupBackgroundSettings {
            desired_enabled: true,
            enabled_at: Some(2_000),
            last_worker_heartbeat_at: None,
            updated_at: 2_000,
        }
    );
}

#[test]
fn finish_disable_clears_enabled_state_and_heartbeat() {
    let temp = tempfile::tempdir().expect("temp dir");
    let repo = settings_repo_at(&temp.path().join("test.db"));
    repo.begin_enable(1_000).expect("begin enable");
    repo.record_worker_heartbeat(1_100)
        .expect("record worker heartbeat");

    repo.finish_disable(1_200).expect("finish disable");

    assert_eq!(
        repo.load().expect("load disabled settings"),
        SaveBackupBackgroundSettings {
            desired_enabled: false,
            enabled_at: None,
            last_worker_heartbeat_at: None,
            updated_at: 1_200,
        }
    );
}

#[test]
fn worker_heartbeat_rejects_missing_and_disabled_settings() {
    let temp = tempfile::tempdir().expect("temp dir");
    let repo = settings_repo_at(&temp.path().join("test.db"));

    let missing_error = repo
        .record_worker_heartbeat(1_000)
        .expect_err("missing settings must reject heartbeat");
    assert!(missing_error
        .to_string()
        .contains("background protection must be enabled before recording worker heartbeat"));

    repo.begin_enable(1_100).expect("begin enable");
    repo.finish_disable(1_200).expect("finish disable");
    let disabled_error = repo
        .record_worker_heartbeat(1_300)
        .expect_err("disabled settings must reject heartbeat");
    assert!(disabled_error
        .to_string()
        .contains("background protection must be enabled before recording worker heartbeat"));
}

#[test]
fn settings_persist_across_database_reopen() {
    let temp = tempfile::tempdir().expect("temp dir");
    let database_path = temp.path().join("test.db");
    {
        let repo = settings_repo_at(&database_path);
        repo.begin_enable(1_000).expect("begin enable");
        repo.record_worker_heartbeat(1_100)
            .expect("record worker heartbeat");
    }

    let reopened = settings_repo_at(&database_path);
    assert_eq!(
        reopened.load().expect("load reopened settings"),
        SaveBackupBackgroundSettings {
            desired_enabled: true,
            enabled_at: Some(1_000),
            last_worker_heartbeat_at: Some(1_100),
            updated_at: 1_100,
        }
    );
}

#[test]
fn timestamps_saturate_on_write_and_negative_values_clamp_on_read() {
    let temp = tempfile::tempdir().expect("temp dir");
    let database_path = temp.path().join("test.db");
    let db = Arc::new(Mutex::new(
        open_database(&database_path).expect("open database"),
    ));
    let repo = SqliteSaveBackupBackgroundSettingsRepository::new(Arc::clone(&db));

    repo.begin_enable(u128::MAX).expect("begin enable");
    repo.record_worker_heartbeat(u128::MAX)
        .expect("record worker heartbeat");
    let saturated = i64::MAX as u128;
    assert_eq!(
        repo.load().expect("load saturated settings"),
        SaveBackupBackgroundSettings {
            desired_enabled: true,
            enabled_at: Some(saturated),
            last_worker_heartbeat_at: Some(saturated),
            updated_at: saturated,
        }
    );

    db.lock()
        .expect("database lock")
        .execute(
            "UPDATE save_backup_background_settings
             SET enabled_at = -1, last_worker_heartbeat_at = -2, updated_at = -3
             WHERE singleton_id = 1",
            [],
        )
        .expect("seed legacy negative timestamps");
    assert_eq!(
        repo.load().expect("load clamped settings"),
        SaveBackupBackgroundSettings {
            desired_enabled: true,
            enabled_at: Some(0),
            last_worker_heartbeat_at: Some(0),
            updated_at: 0,
        }
    );
}

fn settings_repo_at(path: &Path) -> SqliteSaveBackupBackgroundSettingsRepository {
    let conn = open_database(path).expect("open database");
    SqliteSaveBackupBackgroundSettingsRepository::new(Arc::new(Mutex::new(conn)))
}
