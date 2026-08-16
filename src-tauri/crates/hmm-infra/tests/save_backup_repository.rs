use hmm_core::{
    GameId, ProfileDirectoryMode, ProfileDirectorySelection, ProfileDirectoryStatus, ProfileId,
    SaveBackupRetentionReason, SaveBackupStatus, SaveBackupSummary, SaveBackupTrigger,
};
use hmm_infra::{open_database, SqliteSaveBackupRepository};
use hmm_ports::{SaveBackupCenterRepositoryQuery, SaveBackupRepository};
use std::sync::{Arc, Mutex};

#[test]
fn sqlite_save_backup_repository_round_trips_latest_first_and_updates_status() {
    let temp = tempfile::tempdir().expect("temp dir");
    let conn = open_database(&temp.path().join("test.db")).expect("open db");
    let repo = SqliteSaveBackupRepository::new(Arc::new(Mutex::new(conn)));

    repo.save(&sample_summary("backup-old", 10))
        .expect("save old");
    repo.save(&sample_summary("backup-new", 20))
        .expect("save new");

    let latest = repo
        .list_for_profile(&GameId::mhw(), &ProfileId::new("default"), Some(1))
        .expect("list backups");

    assert_eq!(latest.len(), 1);
    assert_eq!(latest[0].backup_id, "backup-new");
    assert_eq!(latest[0].source_path_label.as_deref(), Some("remote"));
    assert_eq!(latest[0].source_path_hash, "sha256:source");
    assert_eq!(
        latest[0].backup_directory.mode,
        ProfileDirectoryMode::Custom
    );
    assert_eq!(
        latest[0].backup_directory.directory.as_deref(),
        Some("D:/Backups")
    );

    repo.mark_status("backup-new", SaveBackupStatus::DeletedByRetention)
        .expect("status update");
    let backups = repo
        .list_for_profile(&GameId::mhw(), &ProfileId::new("default"), None)
        .expect("list backups");

    assert_eq!(backups[0].backup_id, "backup-new");
    assert_eq!(backups[0].status, SaveBackupStatus::DeletedByRetention);

    let exact = repo
        .get_for_restore(&GameId::mhw(), &ProfileId::new("default"), "backup-old")
        .expect("get exact backup")
        .expect("backup exists");
    assert_eq!(exact.backup_id, "backup-old");
    assert!(repo
        .get_for_restore(&GameId::mhw(), &ProfileId::new("other"), "backup-old",)
        .expect("query other profile")
        .is_none());
}

#[test]
fn sqlite_save_backup_repository_rejects_negative_size_facts_instead_of_wrapping() {
    let temp = tempfile::tempdir().expect("temp dir");
    let conn = open_database(&temp.path().join("test.db")).expect("open db");
    let db = Arc::new(Mutex::new(conn));
    let repo = SqliteSaveBackupRepository::new(db.clone());
    repo.save(&sample_summary("backup-corrupt", 10))
        .expect("save backup");

    db.lock()
        .expect("db lock")
        .execute(
            "UPDATE save_backups SET archive_size_bytes = -1
             WHERE backup_id = 'backup-corrupt'",
            [],
        )
        .expect("seed corrupt archive size");
    assert!(repo
        .list_for_profile(&GameId::mhw(), &ProfileId::new("default"), None)
        .is_err());

    db.lock()
        .expect("db lock")
        .execute_batch(
            "PRAGMA ignore_check_constraints = ON;
             UPDATE save_backups
             SET archive_size_bytes = 100, retention_released_bytes = -1
             WHERE backup_id = 'backup-corrupt';
             PRAGMA ignore_check_constraints = OFF;",
        )
        .expect("seed corrupt released size");
    assert!(repo
        .query_for_center(&SaveBackupCenterRepositoryQuery {
            game_id: GameId::mhw(),
            profile_id: None,
            trigger: None,
            status: None,
            search: None,
            offset: 0,
            limit: 30,
        })
        .is_err());
}

#[test]
fn sqlite_save_backup_repository_persists_retention_intent_partial_retry_and_note() {
    let temp = tempfile::tempdir().expect("temp dir");
    let conn = open_database(&temp.path().join("test.db")).expect("open db");
    let db = Arc::new(Mutex::new(conn));
    let repo = SqliteSaveBackupRepository::new(db.clone());
    repo.save(&sample_summary("backup-retention", 10))
        .expect("save backup");

    assert!(!repo
        .begin_retention(
            &GameId::mhw(),
            &ProfileId::new("other"),
            "backup-retention",
            &[SaveBackupRetentionReason::Age],
            20,
        )
        .expect("wrong profile is a stale intent"));
    assert!(repo
        .begin_retention(
            &GameId::mhw(),
            &ProfileId::new("default"),
            "backup-retention",
            &[
                SaveBackupRetentionReason::Age,
                SaveBackupRetentionReason::Space,
            ],
            21,
        )
        .expect("begin retention"));
    repo.finish_retention(
        &GameId::mhw(),
        &ProfileId::new("default"),
        "backup-retention",
        SaveBackupStatus::RetentionPartial,
        Some("save_backup_retention_delete_failed"),
        64,
    )
    .expect("finish partial retention");

    let partial = repo
        .get_for_restore(
            &GameId::mhw(),
            &ProfileId::new("default"),
            "backup-retention",
        )
        .expect("read partial")
        .expect("partial exists");
    assert_eq!(partial.status, SaveBackupStatus::RetentionPartial);
    assert_eq!(partial.retention_released_bytes, 64);

    assert!(repo
        .begin_retention(
            &GameId::mhw(),
            &ProfileId::new("default"),
            "backup-retention",
            &[SaveBackupRetentionReason::Retry],
            22,
        )
        .expect("retry partial retention"));
    repo.finish_retention(
        &GameId::mhw(),
        &ProfileId::new("default"),
        "backup-retention",
        SaveBackupStatus::DeletedByRetention,
        None,
        64,
    )
    .expect("finish retry");
    assert!(repo
        .finish_retention(
            &GameId::mhw(),
            &ProfileId::new("default"),
            "backup-retention",
            SaveBackupStatus::DeletedByRetention,
            None,
            0,
        )
        .is_err());

    assert!(repo
        .update_note(
            &GameId::mhw(),
            &ProfileId::new("default"),
            "backup-retention",
            Some("updated note"),
        )
        .expect("update exact note"));
    assert!(!repo
        .update_note(
            &GameId::mhw(),
            &ProfileId::new("other"),
            "backup-retention",
            Some("wrong profile"),
        )
        .expect("wrong profile does not update"));

    let final_summary = repo
        .list_for_game(&GameId::mhw())
        .expect("list game backups")
        .pop()
        .expect("backup exists");
    assert_eq!(final_summary.status, SaveBackupStatus::DeletedByRetention);
    assert_eq!(final_summary.retention_released_bytes, 128);
    assert_eq!(final_summary.notes.as_deref(), Some("updated note"));

    let facts: (String, i64, Option<String>, i64) = db
        .lock()
        .expect("db lock")
        .query_row(
            "SELECT retention_reasons, retention_attempted_at,
                    retention_error_code, retention_released_bytes
             FROM save_backups WHERE backup_id = 'backup-retention'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .expect("read retention facts");
    assert_eq!(facts.0, "[\"retry\"]");
    assert_eq!(facts.1, 22);
    assert_eq!(facts.2, None);
    assert_eq!(facts.3, 128);
}

#[test]
fn sqlite_save_backup_repository_queries_center_with_database_paging_and_profile_facts() {
    let temp = tempfile::tempdir().expect("temp dir");
    let conn = open_database(&temp.path().join("test.db")).expect("open db");
    let db = Arc::new(Mutex::new(conn));
    let repo = SqliteSaveBackupRepository::new(db.clone());

    db.lock()
        .expect("db lock")
        .execute(
            "INSERT INTO profiles
                (profile_id, name, description, is_active, created_at, updated_at)
             VALUES (?1, ?2, NULL, 0, ?3, ?3)",
            rusqlite::params!["beta", "Beta Hunters", 2_i64],
        )
        .expect("insert second profile");

    repo.save(&sample_summary("default-new", 30))
        .expect("save default newest");
    let mut default_protected = sample_summary("default-protected", 20);
    default_protected.trigger = SaveBackupTrigger::PreRestore;
    default_protected.archive_size_bytes = 50;
    repo.save(&default_protected)
        .expect("save default protected");

    let mut beta_backup = sample_summary("beta-backup", 40);
    beta_backup.profile_id = ProfileId::new("beta");
    beta_backup.archive_size_bytes = 200;
    beta_backup.notes = Some("Beta cleanup".to_owned());
    repo.save(&beta_backup).expect("save beta backup");

    let first_page = repo
        .query_for_center(&SaveBackupCenterRepositoryQuery {
            game_id: GameId::mhw(),
            profile_id: None,
            trigger: None,
            status: None,
            search: None,
            offset: 0,
            limit: 2,
        })
        .expect("query center")
        .expect("sqlite repository supports center query");
    assert_eq!(first_page.total_count, 3);
    assert_eq!(first_page.items.len(), 2);
    assert_eq!(first_page.items[0].backup.backup_id, "beta-backup");
    assert_eq!(first_page.items[1].backup.backup_id, "default-new");
    assert_eq!(first_page.summary.backup_count, 3);
    assert_eq!(first_page.summary.archive_bytes, 378);
    assert_eq!(first_page.summary.protected_count, 1);
    assert_eq!(first_page.summary.attention_count, 0);
    assert_eq!(first_page.profiles.len(), 2);
    let default_facts = first_page
        .profiles
        .iter()
        .find(|profile| profile.profile_id.as_str() == "default")
        .expect("default profile facts");
    assert_eq!(default_facts.facts.backup_count, 2);
    assert_eq!(default_facts.facts.archive_bytes, 178);
    let beta_facts = first_page
        .profiles
        .iter()
        .find(|profile| profile.profile_id.as_str() == "beta")
        .expect("beta profile facts");
    assert_eq!(beta_facts.facts.backup_count, 1);
    assert_eq!(beta_facts.facts.archive_bytes, 200);

    let second_page = repo
        .query_for_center(&SaveBackupCenterRepositoryQuery {
            game_id: GameId::mhw(),
            profile_id: None,
            trigger: None,
            status: None,
            search: None,
            offset: 2,
            limit: 2,
        })
        .expect("query second page")
        .expect("sqlite repository supports center query");
    assert_eq!(second_page.total_count, 3);
    assert_eq!(second_page.items.len(), 1);
    assert_eq!(second_page.items[0].backup.backup_id, "default-protected");

    let searched = repo
        .query_for_center(&SaveBackupCenterRepositoryQuery {
            game_id: GameId::mhw(),
            profile_id: None,
            trigger: None,
            status: None,
            search: Some("beta hunters".to_owned()),
            offset: 0,
            limit: 30,
        })
        .expect("search profile name")
        .expect("sqlite repository supports center query");
    assert_eq!(searched.total_count, 1);
    assert_eq!(searched.summary.archive_bytes, 200);
    assert_eq!(searched.items[0].profile_name, "Beta Hunters");

    let filtered = repo
        .query_for_center(&SaveBackupCenterRepositoryQuery {
            game_id: GameId::mhw(),
            profile_id: Some(ProfileId::new("default")),
            trigger: Some(SaveBackupTrigger::PreRestore),
            status: Some(SaveBackupStatus::Completed),
            search: None,
            offset: 0,
            limit: 30,
        })
        .expect("filter protected backup")
        .expect("sqlite repository supports center query");
    assert_eq!(filtered.total_count, 1);
    assert_eq!(filtered.items[0].backup.backup_id, "default-protected");
    let default_facts = filtered
        .profiles
        .iter()
        .find(|profile| profile.profile_id.as_str() == "default")
        .expect("default facts remain unfiltered");
    assert_eq!(default_facts.facts.backup_count, 2);
    assert_eq!(default_facts.facts.archive_bytes, 178);
}

fn sample_summary(backup_id: &str, created_at: u128) -> SaveBackupSummary {
    SaveBackupSummary {
        backup_id: backup_id.to_owned(),
        game_id: GameId::mhw(),
        profile_id: ProfileId::new("default"),
        trigger: SaveBackupTrigger::Manual,
        status: SaveBackupStatus::Completed,
        archive_file_name: format!("{backup_id}.zip"),
        manifest_file_name: format!("{backup_id}.manifest.json"),
        archive_size_bytes: 128,
        retention_released_bytes: 0,
        archive_sha256: "sha256:archive".to_owned(),
        file_count: 1,
        created_at,
        source_path_label: Some("remote".to_owned()),
        source_path_hash: "sha256:source".to_owned(),
        backup_directory: custom_backup_directory_selection("D:/Backups"),
        notes: Some("note".to_owned()),
    }
}

fn custom_backup_directory_selection(directory: &str) -> ProfileDirectorySelection {
    ProfileDirectorySelection {
        mode: ProfileDirectoryMode::Custom,
        status: ProfileDirectoryStatus::Valid,
        directory: Some(directory.to_owned()),
        path_label: Some("Backups".to_owned()),
        messages: Vec::new(),
    }
}
