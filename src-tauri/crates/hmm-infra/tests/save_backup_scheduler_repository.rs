use hmm_core::{
    GameId, ProfileId, SaveBackupBackgroundProtectionStatus, SaveBackupSchedulerLeaseRequest,
    SaveBackupSchedulerPendingReason, SaveBackupSchedulerState, SaveBackupWorkerHeartbeat,
};
use hmm_infra::{open_database, SqliteSaveBackupSchedulerStateRepository};
use hmm_ports::SaveBackupSchedulerStateRepository;
use std::sync::{Arc, Mutex};

#[test]
fn sqlite_scheduler_state_round_trips_without_path_fields() {
    let (_temp, repo) = scheduler_repo();
    let state = sample_state();

    repo.upsert_state(&state).expect("state can be saved");

    let loaded = repo
        .get_state(&GameId::mhw(), &ProfileId::new("default"))
        .expect("state can be loaded")
        .expect("state exists");

    assert_eq!(loaded, state);
}

#[test]
fn acquire_due_lease_allows_one_owner_until_expired() {
    let (_temp, repo) = scheduler_repo();
    repo.upsert_state(&sample_state()).expect("seed state");

    let first = repo
        .acquire_due_lease(SaveBackupSchedulerLeaseRequest {
            game_id: GameId::mhw(),
            profile_id: ProfileId::new("default"),
            lease_owner: "worker-a".to_owned(),
            lease_expires_at: 1_500,
            now_unix_millis: 1_000,
            last_checked_at: Some(1_000),
            next_due_at: Some(1_200),
        })
        .expect("first lease succeeds");
    assert!(first.is_some());

    let second = repo
        .acquire_due_lease(SaveBackupSchedulerLeaseRequest {
            game_id: GameId::mhw(),
            profile_id: ProfileId::new("default"),
            lease_owner: "worker-b".to_owned(),
            lease_expires_at: 1_700,
            now_unix_millis: 1_100,
            last_checked_at: Some(1_100),
            next_due_at: Some(1_200),
        })
        .expect("busy lease is not fatal");
    assert!(second.is_none());

    let loaded = repo
        .get_state(&GameId::mhw(), &ProfileId::new("default"))
        .expect("load state")
        .expect("state exists");
    assert_eq!(loaded.lease_owner.as_deref(), Some("worker-a"));
    assert_eq!(loaded.lease_expires_at, Some(1_500));
}

#[test]
fn expired_lease_can_be_taken_over_and_release_is_owner_scoped() {
    let (_temp, repo) = scheduler_repo();
    repo.upsert_state(&SaveBackupSchedulerState {
        lease_owner: Some("worker-a".to_owned()),
        lease_expires_at: Some(900),
        ..sample_state()
    })
    .expect("seed expired lease");

    let acquired = repo
        .acquire_due_lease(SaveBackupSchedulerLeaseRequest {
            game_id: GameId::mhw(),
            profile_id: ProfileId::new("default"),
            lease_owner: "worker-b".to_owned(),
            lease_expires_at: 2_000,
            now_unix_millis: 1_000,
            last_checked_at: Some(1_000),
            next_due_at: Some(1_800),
        })
        .expect("expired lease can be acquired")
        .expect("lease acquired");
    assert_eq!(acquired.lease_owner.as_deref(), Some("worker-b"));

    repo.release_lease(&GameId::mhw(), &ProfileId::new("default"), "worker-a")
        .expect("wrong owner release is harmless");
    let still_owned = repo
        .get_state(&GameId::mhw(), &ProfileId::new("default"))
        .expect("load state")
        .expect("state exists");
    assert_eq!(still_owned.lease_owner.as_deref(), Some("worker-b"));

    repo.release_lease(&GameId::mhw(), &ProfileId::new("default"), "worker-b")
        .expect("owner can release");
    let released = repo
        .get_state(&GameId::mhw(), &ProfileId::new("default"))
        .expect("load state")
        .expect("state exists");
    assert_eq!(released.lease_owner, None);
    assert_eq!(released.lease_expires_at, None);
}

#[test]
fn worker_heartbeat_updates_worker_health_without_leaking_paths() {
    let (_temp, repo) = scheduler_repo();
    repo.upsert_state(&sample_state()).expect("seed state");

    repo.record_worker_heartbeat(SaveBackupWorkerHeartbeat {
        game_id: GameId::mhw(),
        profile_id: ProfileId::new("default"),
        worker_instance_id: "worker-a".to_owned(),
        checked_at: 1_234,
        status: SaveBackupBackgroundProtectionStatus::Protected,
    })
    .expect("heartbeat can be saved");

    let loaded = repo
        .get_state(&GameId::mhw(), &ProfileId::new("default"))
        .expect("load state")
        .expect("state exists");
    assert_eq!(loaded.worker_instance_id.as_deref(), Some("worker-a"));
    assert_eq!(loaded.last_checked_at, Some(1_234));
    assert_eq!(
        loaded.background_status,
        SaveBackupBackgroundProtectionStatus::Protected
    );
}

fn scheduler_repo() -> (tempfile::TempDir, SqliteSaveBackupSchedulerStateRepository) {
    let temp = tempfile::tempdir().expect("temp dir");
    let conn = open_database(&temp.path().join("test.db")).expect("open db");
    (
        temp,
        SqliteSaveBackupSchedulerStateRepository::new(Arc::new(Mutex::new(conn))),
    )
}

fn sample_state() -> SaveBackupSchedulerState {
    SaveBackupSchedulerState {
        game_id: GameId::mhw(),
        profile_id: ProfileId::new("default"),
        enabled: true,
        background_protection_enabled: true,
        background_status: SaveBackupBackgroundProtectionStatus::TrayOnly,
        last_checked_at: Some(10),
        last_attempt_at: Some(20),
        last_success_at: Some(30),
        next_due_at: Some(40),
        pending_reason: Some(SaveBackupSchedulerPendingReason::GameRunning),
        last_error_code: Some("save_backup_auto_skipped_game_running".to_owned()),
        worker_instance_id: Some("worker-a".to_owned()),
        lease_owner: None,
        lease_expires_at: None,
        updated_at: 50,
    }
}
