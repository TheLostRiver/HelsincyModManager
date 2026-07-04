use anyhow::Result;
use hmm_app::{CreateSaveBackupRequest, SaveBackupError, SaveBackupService};
use hmm_core::{
    GameId, Profile, ProfileBackupRetention, ProfileBackupSchedule, ProfileDirectoryMode,
    ProfileDirectorySelection, ProfileDirectoryStatus, ProfileId, ProfileSaveSettings,
    SaveBackupStatus, SaveBackupSummary, SaveBackupTrigger,
};
use hmm_ports::{
    AppClock, ProfileRepository, ProfileSaveDirectoryValidator, ProfileSaveSettingsRepository,
    SaveBackupRepository, SaveBackupWriteRequest, SaveBackupWriteResult, SaveBackupWriter,
};
use std::collections::BTreeSet;
use std::sync::{Arc, Mutex};

#[test]
fn manual_backup_uses_default_backup_directory_when_custom_backup_root_is_unset() {
    let harness = Harness::new();
    harness.insert_profile("default");
    harness.insert_settings(ProfileSaveSettings {
        profile_id: "default".to_owned(),
        save_directory: custom_directory_selection("C:/Users/Test/Saves"),
        backup_directory: default_backup_directory_selection(),
        schedule: ProfileBackupSchedule::manual(),
        retention: ProfileBackupRetention {
            max_count: 3,
            max_age_days: None,
        },
        updated_at: 10,
    });

    let summary = harness
        .service
        .create_manual_backup(CreateSaveBackupRequest {
            game_id: GameId::mhw(),
            profile_id: ProfileId::new("default"),
            note: Some("after fatalis".to_owned()),
        })
        .expect("manual backup should use default backup directory")
        .summary;

    assert_eq!(summary.backup_id, "backup-1");
    assert_eq!(summary.trigger, SaveBackupTrigger::Manual);
    assert_eq!(summary.status, SaveBackupStatus::Completed);

    let writer_requests = harness.writer.take_requests();
    assert_eq!(writer_requests.len(), 1);
    assert_eq!(writer_requests[0].game_id.as_str(), "mhw");
    assert_eq!(writer_requests[0].profile_id.as_str(), "default");
    assert_eq!(
        writer_requests[0].source_directory.as_deref(),
        Some("C:/Users/Test/Saves")
    );
    assert_eq!(
        writer_requests[0].source_directory_selection.mode,
        ProfileDirectoryMode::Custom
    );
    assert_eq!(
        writer_requests[0].backup_directory.mode,
        ProfileDirectoryMode::Default
    );
    assert_eq!(writer_requests[0].note.as_deref(), Some("after fatalis"));
    assert_eq!(writer_requests[0].created_at_unix_millis, 42);

    let saved = harness.repository.take_saved();
    assert_eq!(saved.len(), 1);
    assert_eq!(saved[0].backup_id, "backup-1");
}

#[test]
fn manual_backup_rejects_unset_save_directory_before_writer_runs() {
    let harness = Harness::new();
    harness.insert_profile("default");
    harness.insert_settings(ProfileSaveSettings {
        profile_id: "default".to_owned(),
        save_directory: unset_save_directory(),
        backup_directory: default_backup_directory_selection(),
        schedule: ProfileBackupSchedule::manual(),
        retention: ProfileBackupRetention::default(),
        updated_at: 10,
    });

    let error = harness
        .service
        .create_manual_backup(CreateSaveBackupRequest {
            game_id: GameId::mhw(),
            profile_id: ProfileId::new("default"),
            note: None,
        })
        .expect_err("unset source must be rejected");

    assert_eq!(error, SaveBackupError::SourceUnset);
    assert_eq!(error.code(), "save_backup_source_unset");
    assert!(harness.writer.take_requests().is_empty());
    assert!(harness.repository.take_saved().is_empty());
}

#[test]
fn manual_backup_prunes_old_completed_backups_for_same_profile_only() {
    let harness = Harness::new();
    harness.insert_profile("default");
    harness.insert_profile("alt");
    harness.insert_settings(ProfileSaveSettings {
        profile_id: "default".to_owned(),
        save_directory: custom_directory_selection("C:/Users/Test/Saves"),
        backup_directory: default_backup_directory_selection(),
        schedule: ProfileBackupSchedule::manual(),
        retention: ProfileBackupRetention {
            max_count: 1,
            max_age_days: None,
        },
        updated_at: 10,
    });
    harness
        .repository
        .save(&sample_summary("backup-old", "default", 1))
        .expect("old summary saved");
    harness
        .repository
        .save(&sample_summary("backup-other-profile", "alt", 1))
        .expect("other profile summary saved");

    let summary = harness
        .service
        .create_manual_backup(CreateSaveBackupRequest {
            game_id: GameId::mhw(),
            profile_id: ProfileId::new("default"),
            note: None,
        })
        .expect("manual backup should prune old backup")
        .summary;

    assert_eq!(summary.backup_id, "backup-1");
    assert_eq!(harness.writer.take_deleted_ids(), vec!["backup-old"]);

    let saved = harness.repository.take_saved();
    assert_eq!(
        saved
            .iter()
            .find(|item| item.backup_id == "backup-old")
            .expect("old backup retained as history")
            .status,
        SaveBackupStatus::DeletedByRetention
    );
    assert_eq!(
        saved
            .iter()
            .find(|item| item.backup_id == "backup-other-profile")
            .expect("other profile untouched")
            .status,
        SaveBackupStatus::Completed
    );
    assert!(saved.iter().any(|item| item.backup_id == "backup-1"));
}

#[test]
fn manual_backup_returns_created_summary_when_retention_delete_fails() {
    let harness = Harness::new();
    harness.insert_profile("default");
    harness.insert_settings(ProfileSaveSettings {
        profile_id: "default".to_owned(),
        save_directory: custom_directory_selection("C:/Users/Test/Saves"),
        backup_directory: default_backup_directory_selection(),
        schedule: ProfileBackupSchedule::manual(),
        retention: ProfileBackupRetention {
            max_count: 1,
            max_age_days: None,
        },
        updated_at: 10,
    });
    harness
        .repository
        .save(&sample_summary("backup-old", "default", 1))
        .expect("old summary saved");
    harness.writer.fail_delete_for("backup-old");

    let summary = harness
        .service
        .create_manual_backup(CreateSaveBackupRequest {
            game_id: GameId::mhw(),
            profile_id: ProfileId::new("default"),
            note: None,
        })
        .expect("created backup should still be returned when retention fails")
        .summary;

    assert_eq!(summary.backup_id, "backup-1");
    assert_eq!(harness.writer.take_deleted_ids(), vec!["backup-old"]);

    let saved = harness.repository.take_saved();
    assert!(saved.iter().any(|item| item.backup_id == "backup-1"));
    assert_eq!(
        saved
            .iter()
            .find(|item| item.backup_id == "backup-old")
            .expect("old backup remains visible when delete fails")
            .status,
        SaveBackupStatus::Completed
    );
}

#[test]
fn manual_backup_continues_retention_after_individual_delete_failure() {
    let harness = Harness::new();
    harness.insert_profile("default");
    harness.insert_settings(ProfileSaveSettings {
        profile_id: "default".to_owned(),
        save_directory: custom_directory_selection("C:/Users/Test/Saves"),
        backup_directory: default_backup_directory_selection(),
        schedule: ProfileBackupSchedule::manual(),
        retention: ProfileBackupRetention {
            max_count: 1,
            max_age_days: None,
        },
        updated_at: 10,
    });
    harness
        .repository
        .save(&sample_summary("backup-failing", "default", 3))
        .expect("failing summary saved");
    harness
        .repository
        .save(&sample_summary("backup-pruned", "default", 2))
        .expect("pruned summary saved");
    harness.writer.fail_delete_for("backup-failing");

    let summary = harness
        .service
        .create_manual_backup(CreateSaveBackupRequest {
            game_id: GameId::mhw(),
            profile_id: ProfileId::new("default"),
            note: None,
        })
        .expect("retention failure should not fail created backup")
        .summary;

    assert_eq!(summary.backup_id, "backup-1");
    assert_eq!(
        harness.writer.take_deleted_ids(),
        vec!["backup-failing", "backup-pruned"]
    );

    let saved = harness.repository.take_saved();
    assert_eq!(
        saved
            .iter()
            .find(|item| item.backup_id == "backup-failing")
            .expect("failed delete remains completed")
            .status,
        SaveBackupStatus::Completed
    );
    assert_eq!(
        saved
            .iter()
            .find(|item| item.backup_id == "backup-pruned")
            .expect("later old backup still pruned")
            .status,
        SaveBackupStatus::DeletedByRetention
    );
}

#[test]
fn manual_backup_retention_uses_each_backup_original_directory() {
    let harness = Harness::new();
    harness.insert_profile("default");
    harness.insert_settings(ProfileSaveSettings {
        profile_id: "default".to_owned(),
        save_directory: custom_directory_selection("C:/Users/Test/Saves"),
        backup_directory: custom_directory_selection("D:/CurrentBackups"),
        schedule: ProfileBackupSchedule::manual(),
        retention: ProfileBackupRetention {
            max_count: 1,
            max_age_days: None,
        },
        updated_at: 10,
    });
    let mut old_summary = sample_summary("backup-old", "default", 1);
    old_summary.backup_directory = custom_directory_selection("E:/OriginalBackups");
    harness
        .repository
        .save(&old_summary)
        .expect("old summary saved");

    harness
        .service
        .create_manual_backup(CreateSaveBackupRequest {
            game_id: GameId::mhw(),
            profile_id: ProfileId::new("default"),
            note: None,
        })
        .expect("manual backup should prune using original directory");

    let deleted = harness.writer.take_deleted();
    assert_eq!(deleted.len(), 1);
    assert_eq!(deleted[0].0, "backup-old");
    assert_eq!(deleted[0].1.as_deref(), Some("E:/OriginalBackups"));
}

struct Harness {
    service: SaveBackupService,
    profile_repository: Arc<FakeProfileRepository>,
    settings_repository: Arc<FakeProfileSaveSettingsRepository>,
    repository: Arc<FakeSaveBackupRepository>,
    writer: Arc<FakeSaveBackupWriter>,
}

impl Harness {
    fn new() -> Self {
        let profile_repository = Arc::new(FakeProfileRepository::default());
        let settings_repository = Arc::new(FakeProfileSaveSettingsRepository::default());
        let validator = Arc::new(FakeProfileSaveDirectoryValidator);
        let repository = Arc::new(FakeSaveBackupRepository::default());
        let writer = Arc::new(FakeSaveBackupWriter::default());
        let service = SaveBackupService::new(
            profile_repository.clone(),
            settings_repository.clone(),
            validator,
            repository.clone(),
            writer.clone(),
            Arc::new(FixedClock),
        );

        Self {
            service,
            profile_repository,
            settings_repository,
            repository,
            writer,
        }
    }

    fn insert_profile(&self, profile_id: &str) {
        self.profile_repository
            .save(&Profile {
                id: profile_id.to_owned(),
                name: "Profile".to_owned(),
                description: None,
                is_active: profile_id == "default",
                created_at: 1,
                updated_at: 1,
            })
            .expect("profile saved");
    }

    fn insert_settings(&self, settings: ProfileSaveSettings) {
        self.settings_repository
            .save_settings(&settings)
            .expect("settings saved");
    }
}

#[derive(Default)]
struct FakeProfileRepository {
    profiles: Mutex<Vec<Profile>>,
}

impl ProfileRepository for FakeProfileRepository {
    fn get(&self, profile_id: &str) -> Result<Option<Profile>> {
        Ok(self
            .profiles
            .lock()
            .unwrap()
            .iter()
            .find(|profile| profile.id == profile_id)
            .cloned())
    }

    fn save(&self, profile: &Profile) -> Result<()> {
        let mut profiles = self.profiles.lock().unwrap();
        profiles.retain(|existing| existing.id != profile.id);
        profiles.push(profile.clone());
        Ok(())
    }

    fn delete(&self, profile_id: &str) -> Result<()> {
        self.profiles
            .lock()
            .unwrap()
            .retain(|profile| profile.id != profile_id);
        Ok(())
    }

    fn list_all(&self) -> Result<Vec<Profile>> {
        Ok(self.profiles.lock().unwrap().clone())
    }

    fn get_active(&self) -> Result<Option<Profile>> {
        Ok(self
            .profiles
            .lock()
            .unwrap()
            .iter()
            .find(|profile| profile.is_active)
            .cloned())
    }

    fn set_active(&self, profile_id: &str, updated_at: u128) -> Result<()> {
        let mut profiles = self.profiles.lock().unwrap();
        for profile in profiles.iter_mut() {
            profile.is_active = profile.id == profile_id;
            if profile.is_active {
                profile.updated_at = updated_at;
            }
        }
        Ok(())
    }
}

#[derive(Default)]
struct FakeProfileSaveSettingsRepository {
    settings: Mutex<Vec<ProfileSaveSettings>>,
}

impl ProfileSaveSettingsRepository for FakeProfileSaveSettingsRepository {
    fn get_settings(&self, profile_id: &str) -> Result<Option<ProfileSaveSettings>> {
        Ok(self
            .settings
            .lock()
            .unwrap()
            .iter()
            .find(|settings| settings.profile_id == profile_id)
            .cloned())
    }

    fn save_settings(&self, settings: &ProfileSaveSettings) -> Result<()> {
        let mut all_settings = self.settings.lock().unwrap();
        all_settings.retain(|existing| existing.profile_id != settings.profile_id);
        all_settings.push(settings.clone());
        Ok(())
    }
}

struct FakeProfileSaveDirectoryValidator;

impl ProfileSaveDirectoryValidator for FakeProfileSaveDirectoryValidator {
    fn validate_save_directory(
        &self,
        _game_id: &str,
        directory: &str,
    ) -> Result<ProfileDirectorySelection> {
        Ok(custom_directory_selection(directory))
    }

    fn validate_backup_directory(
        &self,
        _game_id: &str,
        directory: &str,
    ) -> Result<ProfileDirectorySelection> {
        Ok(custom_directory_selection(directory))
    }

    fn default_backup_directory(&self, _game_id: &str) -> Result<ProfileDirectorySelection> {
        Ok(default_backup_directory_selection())
    }
}

#[derive(Default)]
struct FakeSaveBackupRepository {
    saved: Mutex<Vec<SaveBackupSummary>>,
}

impl FakeSaveBackupRepository {
    fn take_saved(&self) -> Vec<SaveBackupSummary> {
        std::mem::take(&mut *self.saved.lock().unwrap())
    }
}

impl SaveBackupRepository for FakeSaveBackupRepository {
    fn save(&self, summary: &SaveBackupSummary) -> Result<()> {
        self.saved.lock().unwrap().push(summary.clone());
        Ok(())
    }

    fn list_for_profile(
        &self,
        game_id: &GameId,
        profile_id: &ProfileId,
        _limit: Option<usize>,
    ) -> Result<Vec<SaveBackupSummary>> {
        Ok(self
            .saved
            .lock()
            .unwrap()
            .iter()
            .filter(|summary| &summary.game_id == game_id && &summary.profile_id == profile_id)
            .cloned()
            .collect())
    }

    fn mark_status(&self, backup_id: &str, status: SaveBackupStatus) -> Result<()> {
        for summary in self.saved.lock().unwrap().iter_mut() {
            if summary.backup_id == backup_id {
                summary.status = status;
            }
        }
        Ok(())
    }
}

#[derive(Default)]
struct FakeSaveBackupWriter {
    requests: Mutex<Vec<SaveBackupWriteRequest>>,
    deleted: Mutex<Vec<(String, Option<String>)>>,
    delete_failures: Mutex<BTreeSet<String>>,
}

impl FakeSaveBackupWriter {
    fn take_requests(&self) -> Vec<SaveBackupWriteRequest> {
        std::mem::take(&mut *self.requests.lock().unwrap())
    }

    fn take_deleted(&self) -> Vec<(String, Option<String>)> {
        std::mem::take(&mut *self.deleted.lock().unwrap())
    }

    fn take_deleted_ids(&self) -> Vec<String> {
        self.take_deleted()
            .into_iter()
            .map(|(backup_id, _)| backup_id)
            .collect()
    }

    fn fail_delete_for(&self, backup_id: &str) {
        self.delete_failures
            .lock()
            .unwrap()
            .insert(backup_id.to_owned());
    }
}

impl SaveBackupWriter for FakeSaveBackupWriter {
    fn write_backup(&self, request: SaveBackupWriteRequest) -> Result<SaveBackupWriteResult> {
        self.requests.lock().unwrap().push(request.clone());
        Ok(SaveBackupWriteResult {
            summary: SaveBackupSummary {
                backup_id: "backup-1".to_owned(),
                game_id: request.game_id,
                profile_id: request.profile_id,
                trigger: SaveBackupTrigger::Manual,
                status: SaveBackupStatus::Completed,
                archive_file_name: "20260704-221530_mhw_profile-default_manual.zip".to_owned(),
                manifest_file_name: "20260704-221530_mhw_profile-default_manual.manifest.json"
                    .to_owned(),
                archive_size_bytes: 128,
                archive_sha256: "sha256:test".to_owned(),
                file_count: 1,
                created_at: request.created_at_unix_millis,
                source_path_label: Some("Saves".to_owned()),
                source_path_hash: "sha256:source".to_owned(),
                backup_directory: request.backup_directory,
                notes: request.note,
            },
        })
    }

    fn delete_backup_files(
        &self,
        backup_directory: &ProfileDirectorySelection,
        summary: &SaveBackupSummary,
    ) -> Result<()> {
        self.deleted.lock().unwrap().push((
            summary.backup_id.clone(),
            backup_directory.directory.clone(),
        ));
        if self
            .delete_failures
            .lock()
            .unwrap()
            .contains(&summary.backup_id)
        {
            anyhow::bail!("delete failed");
        }
        Ok(())
    }
}

struct FixedClock;

impl AppClock for FixedClock {
    fn now_unix_millis(&self) -> Result<u128> {
        Ok(42)
    }
}

fn custom_directory_selection(directory: &str) -> ProfileDirectorySelection {
    ProfileDirectorySelection {
        mode: ProfileDirectoryMode::Custom,
        status: ProfileDirectoryStatus::Valid,
        directory: Some(directory.to_owned()),
        path_label: Some("Saves".to_owned()),
        messages: Vec::new(),
    }
}

fn default_backup_directory_selection() -> ProfileDirectorySelection {
    ProfileDirectorySelection {
        mode: ProfileDirectoryMode::Default,
        status: ProfileDirectoryStatus::Defaulted,
        directory: None,
        path_label: Some("HelsincyModManager/backups/saves/mhw/profile-default".to_owned()),
        messages: vec!["使用默认备份目录".to_owned()],
    }
}

fn sample_summary(backup_id: &str, profile_id: &str, created_at: u128) -> SaveBackupSummary {
    SaveBackupSummary {
        backup_id: backup_id.to_owned(),
        game_id: GameId::mhw(),
        profile_id: ProfileId::new(profile_id),
        trigger: SaveBackupTrigger::Manual,
        status: SaveBackupStatus::Completed,
        archive_file_name: format!("{backup_id}.zip"),
        manifest_file_name: format!("{backup_id}.manifest.json"),
        archive_size_bytes: 128,
        archive_sha256: "sha256:test".to_owned(),
        file_count: 1,
        created_at,
        source_path_label: Some("Saves".to_owned()),
        source_path_hash: "sha256:source".to_owned(),
        backup_directory: default_backup_directory_selection(),
        notes: None,
    }
}

fn unset_save_directory() -> ProfileDirectorySelection {
    ProfileDirectorySelection {
        mode: ProfileDirectoryMode::Unset,
        status: ProfileDirectoryStatus::Unset,
        directory: None,
        path_label: None,
        messages: vec!["尚未选择游戏存档目录".to_owned()],
    }
}
