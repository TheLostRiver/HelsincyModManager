use anyhow::Result;
use hmm_app::{
    SaveBackupAutoCheckRequest, SaveBackupAutoCheckStatus, SaveBackupAutoSchedulerService,
};
use hmm_core::{
    BackupCadence, GameId, Profile, ProfileBackupRetention, ProfileBackupSchedule,
    ProfileDirectoryMode, ProfileDirectorySelection, ProfileDirectoryStatus, ProfileId,
    ProfileSaveSettings, SaveBackupStatus, SaveBackupSummary, SaveBackupTrigger,
};
use hmm_ports::{AppClock, ProfileRepository, ProfileSaveSettingsRepository, SaveBackupRepository};
use std::sync::{Arc, Mutex};

const DAY_MS: u128 = 86_400_000;
const HOUR_MS: u128 = 3_600_000;

#[test]
fn manual_schedule_is_reported_without_starting_auto_backup() {
    let harness = Harness::new(DAY_MS + 4 * HOUR_MS);
    harness.insert_profile("default");
    harness.insert_settings(settings_with_schedule(ProfileBackupSchedule::manual()));

    let result = harness
        .scheduler
        .check_profile(SaveBackupAutoCheckRequest {
            game_id: GameId::mhw(),
            profile_id: ProfileId::new("default"),
        })
        .expect("manual schedule should be checked");

    assert_eq!(result.status, SaveBackupAutoCheckStatus::ManualOnly);
    assert!(result.due_task.is_none());
}

#[test]
fn daily_schedule_due_without_prior_auto_backup_returns_auto_task_request() {
    let harness = Harness::new(2 * DAY_MS + 4 * HOUR_MS);
    harness.insert_profile("default");
    harness.insert_settings(settings_with_schedule(ProfileBackupSchedule {
        cadence: BackupCadence::Daily,
        hour: Some(3),
        minute: Some(0),
        weekdays: Vec::new(),
    }));

    let result = harness
        .scheduler
        .check_profile(SaveBackupAutoCheckRequest {
            game_id: GameId::mhw(),
            profile_id: ProfileId::new("default"),
        })
        .expect("daily schedule should be due");

    assert_eq!(result.status, SaveBackupAutoCheckStatus::Due);
    assert_eq!(result.next_due_at, Some(3 * DAY_MS + 3 * HOUR_MS));
    let task = result.due_task.expect("due schedule returns task request");
    assert_eq!(task.game_id, GameId::mhw());
    assert_eq!(task.profile_id, ProfileId::new("default"));
    assert_eq!(task.trigger, SaveBackupTrigger::Auto);
    assert!(task.note.is_none());
}

#[test]
fn daily_schedule_not_due_after_auto_backup_in_current_slot() {
    let harness = Harness::new(2 * DAY_MS + 4 * HOUR_MS);
    harness.insert_profile("default");
    harness.insert_settings(settings_with_schedule(ProfileBackupSchedule {
        cadence: BackupCadence::Daily,
        hour: Some(3),
        minute: Some(0),
        weekdays: Vec::new(),
    }));
    harness.insert_backup(auto_summary("auto-current", 2 * DAY_MS + 3 * HOUR_MS + 1));

    let result = harness
        .scheduler
        .check_profile(SaveBackupAutoCheckRequest {
            game_id: GameId::mhw(),
            profile_id: ProfileId::new("default"),
        })
        .expect("daily schedule should be checked");

    assert_eq!(result.status, SaveBackupAutoCheckStatus::NotDue);
    assert!(result.due_task.is_none());
    assert_eq!(result.next_due_at, Some(3 * DAY_MS + 3 * HOUR_MS));
}

#[test]
fn weekly_schedule_uses_latest_elapsed_weekday_slot() {
    let wednesday_day = 6;
    let harness = Harness::new(wednesday_day * DAY_MS + 4 * HOUR_MS);
    harness.insert_profile("default");
    harness.insert_settings(settings_with_schedule(ProfileBackupSchedule {
        cadence: BackupCadence::Weekly,
        hour: Some(3),
        minute: Some(0),
        weekdays: vec![1, 3],
    }));
    harness.insert_backup(auto_summary("auto-before-slot", wednesday_day * DAY_MS));

    let result = harness
        .scheduler
        .check_profile(SaveBackupAutoCheckRequest {
            game_id: GameId::mhw(),
            profile_id: ProfileId::new("default"),
        })
        .expect("weekly schedule should be due");

    assert_eq!(result.status, SaveBackupAutoCheckStatus::Due);
    assert_eq!(
        result.last_due_at,
        Some(wednesday_day * DAY_MS + 3 * HOUR_MS)
    );
    assert_eq!(result.next_due_at, Some(11 * DAY_MS + 3 * HOUR_MS));
    assert_eq!(
        result
            .due_task
            .expect("due weekly schedule returns task")
            .trigger,
        SaveBackupTrigger::Auto
    );
}

struct Harness {
    scheduler: SaveBackupAutoSchedulerService,
    profile_repository: Arc<FakeProfileRepository>,
    settings_repository: Arc<FakeProfileSaveSettingsRepository>,
    backup_repository: Arc<FakeSaveBackupRepository>,
}

impl Harness {
    fn new(now_unix_millis: u128) -> Self {
        let profile_repository = Arc::new(FakeProfileRepository::default());
        let settings_repository = Arc::new(FakeProfileSaveSettingsRepository::default());
        let backup_repository = Arc::new(FakeSaveBackupRepository::default());
        let scheduler = SaveBackupAutoSchedulerService::new(
            profile_repository.clone(),
            settings_repository.clone(),
            backup_repository.clone(),
            Arc::new(FixedClock { now_unix_millis }),
        );

        Self {
            scheduler,
            profile_repository,
            settings_repository,
            backup_repository,
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

    fn insert_backup(&self, summary: SaveBackupSummary) {
        self.backup_repository.save(&summary).expect("backup saved");
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

#[derive(Default)]
struct FakeSaveBackupRepository {
    saved: Mutex<Vec<SaveBackupSummary>>,
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

struct FixedClock {
    now_unix_millis: u128,
}

impl AppClock for FixedClock {
    fn now_unix_millis(&self) -> Result<u128> {
        Ok(self.now_unix_millis)
    }
}

fn settings_with_schedule(schedule: ProfileBackupSchedule) -> ProfileSaveSettings {
    ProfileSaveSettings {
        profile_id: "default".to_owned(),
        save_directory: directory_selection("C:/Users/Test/Saves"),
        backup_directory: directory_selection("D:/HMM/Backups"),
        schedule,
        retention: ProfileBackupRetention::default(),
        updated_at: 10,
    }
}

fn auto_summary(backup_id: &str, created_at: u128) -> SaveBackupSummary {
    SaveBackupSummary {
        backup_id: backup_id.to_owned(),
        game_id: GameId::mhw(),
        profile_id: ProfileId::new("default"),
        trigger: SaveBackupTrigger::Auto,
        status: SaveBackupStatus::Completed,
        archive_file_name: format!("{backup_id}.zip"),
        manifest_file_name: format!("{backup_id}.manifest.json"),
        archive_size_bytes: 128,
        archive_sha256: "sha256:test".to_owned(),
        file_count: 1,
        created_at,
        source_path_label: Some("remote".to_owned()),
        source_path_hash: "sha256:source".to_owned(),
        backup_directory: directory_selection("D:/HMM/Backups"),
        notes: None,
    }
}

fn directory_selection(directory: &str) -> ProfileDirectorySelection {
    ProfileDirectorySelection {
        mode: ProfileDirectoryMode::Custom,
        status: ProfileDirectoryStatus::Valid,
        directory: Some(directory.to_owned()),
        path_label: Some("Saves".to_owned()),
        messages: Vec::new(),
    }
}
