use hmm_core::{
    GameId, ProfileDirectoryMode, ProfileDirectorySelection, ProfileDirectoryStatus, ProfileId,
    SaveBackupStatus, SaveBackupSummary, SaveBackupTrigger,
};
use hmm_infra::{open_database, SqliteSaveBackupRepository};
use hmm_ports::SaveBackupRepository;
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
