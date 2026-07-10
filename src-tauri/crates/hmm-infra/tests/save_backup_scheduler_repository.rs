use hmm_core::{
    GameId, ProfileId, SaveBackupBackgroundProtectionStatus,
    SaveBackupSchedulerLeaseRenewalRequest, SaveBackupSchedulerLeaseRequest,
    SaveBackupSchedulerPendingReason, SaveBackupSchedulerState, SaveBackupWorkerHeartbeat,
};
use hmm_infra::{open_database, SqliteSaveBackupSchedulerStateRepository};
use hmm_ports::SaveBackupSchedulerStateRepository;
use std::sync::{Arc, Mutex};

#[cfg(target_os = "windows")]
use std::path::PathBuf;

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
fn upsert_state_preserves_existing_lease_while_updating_stale_check_fields() {
    let (_temp, repo) = scheduler_repo();
    repo.upsert_state(&SaveBackupSchedulerState {
        lease_owner: Some("worker-a".to_owned()),
        lease_expires_at: Some(1_500),
        ..sample_state()
    })
    .expect("seed leased state");

    let stale_state = SaveBackupSchedulerState {
        last_checked_at: Some(1_100),
        next_due_at: Some(1_200),
        pending_reason: None,
        updated_at: 1_100,
        lease_owner: None,
        lease_expires_at: None,
        ..sample_state()
    };
    repo.upsert_state(&stale_state)
        .expect("stale check state can be saved");

    let loaded = repo
        .get_state(&GameId::mhw(), &ProfileId::new("default"))
        .expect("load state")
        .expect("state exists");
    assert_eq!(loaded.lease_owner.as_deref(), Some("worker-a"));
    assert_eq!(loaded.lease_expires_at, Some(1_500));
    assert_eq!(loaded.last_checked_at, Some(1_100));
    assert_eq!(loaded.next_due_at, Some(1_200));
    assert_eq!(loaded.pending_reason, None);
    assert_eq!(loaded.updated_at, 1_100);
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
fn acquire_due_lease_rejects_the_same_owner_while_unexpired() {
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
            lease_owner: "worker-a".to_owned(),
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
fn renew_lease_extends_only_the_unexpired_owner_lease_across_repositories() {
    let (_temp, first, second) = independent_scheduler_repositories();
    first.upsert_state(&sample_state()).expect("seed state");

    first
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

    assert!(second
        .renew_lease(SaveBackupSchedulerLeaseRenewalRequest {
            game_id: GameId::mhw(),
            profile_id: ProfileId::new("default"),
            lease_owner: "worker-a".to_owned(),
            lease_expires_at: 2_000,
            now_unix_millis: 1_400,
        })
        .expect("owner renewal succeeds"));

    let competing = first
        .acquire_due_lease(SaveBackupSchedulerLeaseRequest {
            game_id: GameId::mhw(),
            profile_id: ProfileId::new("default"),
            lease_owner: "worker-b".to_owned(),
            lease_expires_at: 2_500,
            now_unix_millis: 1_600,
            last_checked_at: Some(1_600),
            next_due_at: Some(1_800),
        })
        .expect("competing due check is not fatal");
    assert!(competing.is_none());

    assert!(!second
        .renew_lease(SaveBackupSchedulerLeaseRenewalRequest {
            game_id: GameId::mhw(),
            profile_id: ProfileId::new("default"),
            lease_owner: "worker-b".to_owned(),
            lease_expires_at: 2_500,
            now_unix_millis: 1_700,
        })
        .expect("wrong owner renewal fails closed"));
    assert!(!second
        .renew_lease(SaveBackupSchedulerLeaseRenewalRequest {
            game_id: GameId::mhw(),
            profile_id: ProfileId::new("default"),
            lease_owner: "worker-a".to_owned(),
            lease_expires_at: 2_500,
            now_unix_millis: 2_000,
        })
        .expect("expired owner renewal fails closed"));

    let state = first
        .get_state(&GameId::mhw(), &ProfileId::new("default"))
        .expect("load state")
        .expect("state exists");
    assert_eq!(state.lease_owner.as_deref(), Some("worker-a"));
    assert_eq!(state.lease_expires_at, Some(2_000));
}

#[test]
fn worker_heartbeat_updates_only_worker_health_fields() {
    let (_temp, repo) = scheduler_repo();
    repo.upsert_state(&SaveBackupSchedulerState {
        lease_owner: Some("lease-owner".to_owned()),
        lease_expires_at: Some(2_000),
        ..sample_state()
    })
    .expect("seed state");

    repo.record_worker_heartbeat(SaveBackupWorkerHeartbeat {
        game_id: GameId::mhw(),
        profile_id: ProfileId::new("default"),
        worker_instance_id: "worker-b".to_owned(),
        heartbeat_at: 1_234,
    })
    .expect("heartbeat can be saved");

    let loaded = repo
        .get_state(&GameId::mhw(), &ProfileId::new("default"))
        .expect("load state")
        .expect("state exists");
    assert_eq!(loaded.worker_instance_id.as_deref(), Some("worker-b"));
    assert_eq!(loaded.worker_heartbeat_at, Some(1_234));
    assert_eq!(loaded.last_checked_at, Some(10));
    assert_eq!(
        loaded.background_status,
        SaveBackupBackgroundProtectionStatus::TrayOnly
    );
    assert_eq!(loaded.lease_owner.as_deref(), Some("lease-owner"));
    assert_eq!(loaded.lease_expires_at, Some(2_000));
    assert_eq!(loaded.updated_at, 1_234);
}

#[cfg(target_os = "windows")]
#[test]
#[ignore = "reads disposable smoke AppData after a real Scheduled Task trigger"]
fn windows_smoke_probe_sees_fresh_worker_heartbeat() {
    assert_eq!(
        std::env::var("HMM_RUN_WINDOWS_SCHEDULED_TASK_SMOKE").as_deref(),
        Ok("1"),
        "explicit smoke authorization is required",
    );
    let database_path = PathBuf::from(
        std::env::var_os("HMM_WINDOWS_SMOKE_DATABASE_PATH")
            .expect("disposable smoke database path is required"),
    );
    let profile_id = std::env::var("HMM_WINDOWS_SMOKE_PROFILE_ID")
        .expect("synthetic smoke profile id is required");
    let conn = rusqlite::Connection::open_with_flags(
        database_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .expect("open disposable database read-only");
    let heartbeat: Option<i64> = conn
        .query_row(
            "SELECT worker_heartbeat_at FROM save_backup_scheduler_state
             WHERE game_id = 'mhw' AND profile_id = ?1",
            [&profile_id],
            |row| row.get(0),
        )
        .expect("synthetic scheduler state exists");
    let heartbeat = heartbeat.expect("worker heartbeat exists") as u128;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock is after epoch")
        .as_millis();
    assert!(heartbeat <= now);
    assert!(now - heartbeat <= 45 * 60_000, "worker heartbeat is stale");
}

fn scheduler_repo() -> (tempfile::TempDir, SqliteSaveBackupSchedulerStateRepository) {
    let temp = tempfile::tempdir().expect("temp dir");
    let conn = open_database(&temp.path().join("test.db")).expect("open db");
    (
        temp,
        SqliteSaveBackupSchedulerStateRepository::new(Arc::new(Mutex::new(conn))),
    )
}

fn independent_scheduler_repositories() -> (
    tempfile::TempDir,
    SqliteSaveBackupSchedulerStateRepository,
    SqliteSaveBackupSchedulerStateRepository,
) {
    let temp = tempfile::tempdir().expect("temp dir");
    let database_path = temp.path().join("test.db");
    let first = open_database(&database_path).expect("open first database connection");
    let second = open_database(&database_path).expect("open second database connection");
    (
        temp,
        SqliteSaveBackupSchedulerStateRepository::new(Arc::new(Mutex::new(first))),
        SqliteSaveBackupSchedulerStateRepository::new(Arc::new(Mutex::new(second))),
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
        worker_heartbeat_at: Some(5),
        lease_owner: None,
        lease_expires_at: None,
        updated_at: 50,
    }
}
