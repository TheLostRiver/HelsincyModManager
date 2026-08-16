use anyhow::Result;
use hmm_app::{
    SaveBackupCenterError, SaveBackupCenterQuery, SaveBackupCenterService, SaveBackupService,
    SaveBackupTaskScopeRegistry, SaveBackupTaskService, StartSaveBackupTaskRequest, TaskManager,
};
use hmm_core::{
    GameId, Profile, ProfileBackupRetention, ProfileBackupSchedule, ProfileDirectoryMode,
    ProfileDirectorySelection, ProfileDirectoryStatus, ProfileId, ProfileSaveSettings,
    SaveBackupRetentionOutcome, SaveBackupStatus, SaveBackupSummary, SaveBackupTrigger,
    SteamAccountDisplaySummary,
};
use hmm_ports::{
    AppClock, AuditLogEvent, AuditLogWriter, ProfileRepository, ProfileSaveDirectoryValidator,
    ProfileSaveSettingsRepository, SaveBackupRepository, SaveBackupWriteRequest,
    SaveBackupWriteResult, SaveBackupWriter,
};
use std::sync::{Arc, Mutex};

#[test]
fn backup_center_queries_cross_profile_filters_pagination_and_remaining_space() {
    let harness = Harness::new();
    harness.insert_profile("alpha", "Alpha", true);
    harness.insert_profile("beta", "Beta Hunters", false);
    harness.insert_settings("alpha", None, None);
    harness.insert_settings(
        "beta",
        Some(80),
        Some(SteamAccountDisplaySummary {
            account_name: Some("Beta Hunter".to_owned()),
            avatar_url: Some("https://avatars.steamstatic.com/avatar.jpg".to_owned()),
            account_label: "Steam 12****34".to_owned(),
        }),
    );
    harness.insert_backup(sample_backup(
        "alpha-new",
        "alpha",
        SaveBackupTrigger::Manual,
        SaveBackupStatus::Completed,
        30,
        100,
        0,
        Some("Fatalis clear"),
    ));
    harness.insert_backup(sample_backup(
        "alpha-protected",
        "alpha",
        SaveBackupTrigger::PreRestore,
        SaveBackupStatus::Completed,
        20,
        50,
        0,
        None,
    ));
    harness.insert_backup(sample_backup(
        "beta-partial",
        "beta",
        SaveBackupTrigger::Auto,
        SaveBackupStatus::RetentionPartial,
        40,
        200,
        100,
        Some("retry cleanup"),
    ));

    let page = harness
        .center
        .query(SaveBackupCenterQuery {
            game_id: GameId::mhw(),
            profile_id: None,
            trigger: None,
            status: None,
            search: None,
            offset: 0,
            limit: 2,
        })
        .expect("query backup center");

    assert_eq!(page.total_count, 3);
    assert_eq!(page.items.len(), 2);
    assert_eq!(page.items[0].backup.backup_id, "beta-partial");
    assert_eq!(page.summary.backup_count, 3);
    assert_eq!(page.summary.archive_bytes, 250);
    assert_eq!(page.summary.protected_count, 1);
    assert_eq!(page.summary.attention_count, 1);
    let beta = page
        .profiles
        .iter()
        .find(|profile| profile.profile_id.as_str() == "beta")
        .expect("beta profile summary");
    assert_eq!(beta.archive_bytes, 100);
    assert!(!beta.budget_satisfied);
    assert_eq!(
        beta.steam_account
            .as_ref()
            .and_then(|account| account.account_name.as_deref()),
        Some("Beta Hunter")
    );
    assert_eq!(page.profiles.len(), 2);

    let searched = harness
        .center
        .query(SaveBackupCenterQuery {
            game_id: GameId::mhw(),
            profile_id: None,
            trigger: None,
            status: None,
            search: Some("beta hunters".to_owned()),
            offset: 0,
            limit: 30,
        })
        .expect("search profile name");
    assert_eq!(searched.total_count, 1);
    assert_eq!(searched.items[0].backup.backup_id, "beta-partial");

    let protected = harness
        .center
        .query(SaveBackupCenterQuery {
            game_id: GameId::mhw(),
            profile_id: Some(ProfileId::new("alpha")),
            trigger: Some(SaveBackupTrigger::PreRestore),
            status: Some(SaveBackupStatus::Completed),
            search: None,
            offset: 0,
            limit: 30,
        })
        .expect("filter protected backups");
    assert_eq!(protected.total_count, 1);
    assert_eq!(protected.items[0].backup.backup_id, "alpha-protected");
    assert_eq!(protected.profiles.len(), 2);
    let alpha = protected
        .profiles
        .iter()
        .find(|profile| profile.profile_id.as_str() == "alpha")
        .expect("alpha profile summary remains available");
    assert_eq!(alpha.backup_count, 2);
    assert_eq!(alpha.archive_bytes, 150);

    assert_eq!(
        harness.center.query(SaveBackupCenterQuery {
            game_id: GameId::mhw(),
            profile_id: None,
            trigger: None,
            status: None,
            search: None,
            offset: 0,
            limit: 0,
        }),
        Err(SaveBackupCenterError::QueryInvalid)
    );
    assert_eq!(
        harness.center.query(SaveBackupCenterQuery {
            game_id: GameId::mhw(),
            profile_id: None,
            trigger: None,
            status: None,
            search: Some("x".repeat(101)),
            offset: 0,
            limit: 30,
        }),
        Err(SaveBackupCenterError::QueryInvalid)
    );
    #[cfg(target_pointer_width = "64")]
    assert_eq!(
        harness.center.query(SaveBackupCenterQuery {
            game_id: GameId::mhw(),
            profile_id: None,
            trigger: None,
            status: None,
            search: None,
            offset: usize::MAX,
            limit: 30,
        }),
        Err(SaveBackupCenterError::QueryInvalid)
    );
}

#[test]
fn backup_center_updates_only_exact_backup_note_with_normalization() {
    let harness = Harness::new();
    harness.insert_profile("alpha", "Alpha", true);
    harness.insert_settings("alpha", None, None);
    harness.insert_backup(sample_backup(
        "alpha-new",
        "alpha",
        SaveBackupTrigger::Manual,
        SaveBackupStatus::Completed,
        30,
        100,
        0,
        None,
    ));

    assert_eq!(
        harness
            .center
            .update_note(
                &GameId::mhw(),
                &ProfileId::new("alpha"),
                "alpha-new",
                Some("  before fatalis  ".to_owned()),
            )
            .expect("update note")
            .as_deref(),
        Some("before fatalis")
    );
    assert_eq!(
        harness
            .center
            .update_note(
                &GameId::mhw(),
                &ProfileId::new("alpha"),
                "alpha-new",
                Some("   ".to_owned()),
            )
            .expect("clear note"),
        None
    );
    assert_eq!(
        harness.center.update_note(
            &GameId::mhw(),
            &ProfileId::new("alpha"),
            "missing",
            Some("note".to_owned()),
        ),
        Err(SaveBackupCenterError::BackupMissing)
    );
    assert_eq!(
        SaveBackupCenterError::BackupMissing.code(),
        "save_backup_center_backup_missing"
    );
    assert_eq!(
        harness.center.update_note(
            &GameId::mhw(),
            &ProfileId::new("missing"),
            "alpha-new",
            Some("note".to_owned()),
        ),
        Err(SaveBackupCenterError::ProfileMissing)
    );
    assert_eq!(
        harness.center.update_note(
            &GameId::mhw(),
            &ProfileId::new("alpha"),
            "alpha-new",
            Some("x".repeat(201)),
        ),
        Err(SaveBackupCenterError::NoteInvalid)
    );
}

#[test]
fn backup_center_retention_preserves_missing_profile_error() {
    let harness = Harness::new();

    assert_eq!(
        harness
            .center
            .run_retention(&GameId::mhw(), &ProfileId::new("missing")),
        Err(SaveBackupCenterError::ProfileMissing)
    );
    assert_eq!(
        SaveBackupCenterError::ProfileMissing.code(),
        "save_backup_center_profile_missing"
    );
    assert!(harness.audit.take_events().is_empty());
}

#[test]
fn backup_center_retention_uses_shared_scope_and_records_sanitized_audit() {
    let harness = Harness::new();
    harness.insert_profile("alpha", "Alpha", true);
    harness.insert_settings("alpha", None, None);
    harness.insert_backup(sample_backup(
        "alpha-new",
        "alpha",
        SaveBackupTrigger::Manual,
        SaveBackupStatus::Completed,
        30,
        100,
        0,
        None,
    ));
    harness.insert_backup(sample_backup(
        "alpha-old",
        "alpha",
        SaveBackupTrigger::Manual,
        SaveBackupStatus::Completed,
        10,
        100,
        0,
        None,
    ));

    let task_service = SaveBackupTaskService::with_scope_registry(
        Arc::new(TaskManager::new()),
        Arc::clone(&harness.scope_registry),
    );
    let _reserved = task_service
        .start_save_backup_task(StartSaveBackupTaskRequest {
            game_id: GameId::mhw(),
            profile_id: ProfileId::new("alpha"),
            trigger: SaveBackupTrigger::Manual,
            note: None,
            scheduler_lease_owner: None,
        })
        .expect("reserve backup scope");
    assert_eq!(
        harness
            .center
            .run_retention(&GameId::mhw(), &ProfileId::new("alpha")),
        Err(SaveBackupCenterError::TaskConflict)
    );

    let independent = Harness::new();
    independent.insert_profile("alpha", "Alpha", true);
    independent.insert_settings("alpha", None, None);
    independent.insert_backup(sample_backup(
        "alpha-new",
        "alpha",
        SaveBackupTrigger::Manual,
        SaveBackupStatus::Completed,
        30,
        100,
        0,
        None,
    ));
    independent.insert_backup(sample_backup(
        "alpha-old",
        "alpha",
        SaveBackupTrigger::Manual,
        SaveBackupStatus::Completed,
        10,
        100,
        0,
        None,
    ));
    let report = independent
        .center
        .run_retention(&GameId::mhw(), &ProfileId::new("alpha"))
        .expect("explicit retention succeeds");
    assert_eq!(report.outcome, SaveBackupRetentionOutcome::Completed);
    assert!(!report.evidence_degraded);
    assert_eq!(report.deleted_count, 1);
    assert_eq!(independent.writer.take_deleted(), vec!["alpha-old"]);

    let events = independent.audit.take_events();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].operation, "retention_pruning");
    assert_eq!(events[0].result, "success");
    assert_eq!(
        events[0]
            .fields
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        vec![
            "archive_bytes_after",
            "archive_bytes_before",
            "blocked_count",
            "budget_satisfied",
            "candidate_count",
            "deleted_count",
            "game_id",
            "outcome",
            "partial_count",
            "problem_count",
            "profile_id",
            "protected_count",
            "released_bytes",
            "scanned_count",
        ]
    );
    assert_eq!(events[0].fields["outcome"], "completed");
    assert_eq!(events[0].fields["protected_count"], "0");
    assert_eq!(events[0].fields["problem_count"], "0");
    assert_eq!(events[0].fields["candidate_count"], "1");
    assert_eq!(events[0].fields["deleted_count"], "1");
    let serialized = serde_json::to_string(&events[0]).expect("serialize audit");
    assert!(!serialized.contains("Alpha"));
    assert!(!serialized.contains("C:/"));
    assert!(!serialized.contains("alpha-old.zip"));
    assert!(!serialized.contains("sha256"));
}

#[test]
fn backup_center_retention_reports_audit_evidence_degradation_after_cleanup() {
    let harness = Harness::new();
    harness.insert_profile("alpha", "Alpha", true);
    harness.insert_settings("alpha", None, None);
    harness.insert_backup(sample_backup(
        "alpha-new",
        "alpha",
        SaveBackupTrigger::Manual,
        SaveBackupStatus::Completed,
        30,
        100,
        0,
        None,
    ));
    harness.insert_backup(sample_backup(
        "alpha-old",
        "alpha",
        SaveBackupTrigger::Manual,
        SaveBackupStatus::Completed,
        10,
        100,
        0,
        None,
    ));
    harness.audit.set_fail(true);

    let report = harness
        .center
        .run_retention(&GameId::mhw(), &ProfileId::new("alpha"))
        .expect("audit failure cannot reclassify completed cleanup");

    assert_eq!(report.outcome, SaveBackupRetentionOutcome::Completed);
    assert!(report.evidence_degraded);
    assert_eq!(report.deleted_count, 1);
    assert_eq!(harness.writer.take_deleted(), vec!["alpha-old"]);
    assert!(harness.audit.take_events().is_empty());
}

struct Harness {
    center: SaveBackupCenterService,
    profiles: Arc<FakeProfileRepository>,
    settings: Arc<FakeSettingsRepository>,
    backups: Arc<FakeBackupRepository>,
    writer: Arc<FakeBackupWriter>,
    audit: Arc<RecordingAuditLog>,
    scope_registry: Arc<SaveBackupTaskScopeRegistry>,
}

impl Harness {
    fn new() -> Self {
        let profiles = Arc::new(FakeProfileRepository::default());
        let settings = Arc::new(FakeSettingsRepository::default());
        let backups = Arc::new(FakeBackupRepository::default());
        let writer = Arc::new(FakeBackupWriter::default());
        let audit = Arc::new(RecordingAuditLog::default());
        let scope_registry = Arc::new(SaveBackupTaskScopeRegistry::default());
        let save_backup = Arc::new(SaveBackupService::new(
            profiles.clone(),
            settings.clone(),
            Arc::new(FakeDirectoryValidator),
            backups.clone(),
            writer.clone(),
            Arc::new(FixedClock),
        ));
        let center = SaveBackupCenterService::new(
            profiles.clone(),
            settings.clone(),
            backups.clone(),
            save_backup,
            Arc::clone(&scope_registry),
            audit.clone(),
            Arc::new(FixedClock),
        );
        Self {
            center,
            profiles,
            settings,
            backups,
            writer,
            audit,
            scope_registry,
        }
    }

    fn insert_profile(&self, id: &str, name: &str, is_active: bool) {
        self.profiles
            .save(&Profile {
                id: id.to_owned(),
                name: name.to_owned(),
                description: None,
                is_active,
                created_at: 1,
                updated_at: 1,
            })
            .expect("save profile");
    }

    fn insert_settings(
        &self,
        profile_id: &str,
        max_total_bytes: Option<u64>,
        steam_account: Option<SteamAccountDisplaySummary>,
    ) {
        self.settings
            .save_settings(&ProfileSaveSettings {
                profile_id: profile_id.to_owned(),
                save_directory: custom_directory(),
                backup_directory: default_backup_directory(),
                schedule: ProfileBackupSchedule::manual(),
                retention: ProfileBackupRetention {
                    max_count: 1,
                    max_age_days: None,
                    max_total_bytes,
                },
                steam_account,
                pre_restore_backup_enabled: true,
                updated_at: 1,
            })
            .expect("save settings");
    }

    fn insert_backup(&self, backup: SaveBackupSummary) {
        self.backups.save(&backup).expect("save backup");
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

    fn set_active(&self, profile_id: &str, _updated_at: u128) -> Result<()> {
        for profile in self.profiles.lock().unwrap().iter_mut() {
            profile.is_active = profile.id == profile_id;
        }
        Ok(())
    }
}

#[derive(Default)]
struct FakeSettingsRepository {
    settings: Mutex<Vec<ProfileSaveSettings>>,
}

impl ProfileSaveSettingsRepository for FakeSettingsRepository {
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
        let mut all = self.settings.lock().unwrap();
        all.retain(|existing| existing.profile_id != settings.profile_id);
        all.push(settings.clone());
        Ok(())
    }
}

#[derive(Default)]
struct FakeBackupRepository {
    backups: Mutex<Vec<SaveBackupSummary>>,
}

impl SaveBackupRepository for FakeBackupRepository {
    fn save(&self, summary: &SaveBackupSummary) -> Result<()> {
        let mut backups = self.backups.lock().unwrap();
        backups.retain(|existing| existing.backup_id != summary.backup_id);
        backups.push(summary.clone());
        Ok(())
    }

    fn list_for_profile(
        &self,
        game_id: &GameId,
        profile_id: &ProfileId,
        limit: Option<usize>,
    ) -> Result<Vec<SaveBackupSummary>> {
        let mut backups = self
            .backups
            .lock()
            .unwrap()
            .iter()
            .filter(|backup| &backup.game_id == game_id && &backup.profile_id == profile_id)
            .cloned()
            .collect::<Vec<_>>();
        sort_latest_first(&mut backups);
        if let Some(limit) = limit {
            backups.truncate(limit);
        }
        Ok(backups)
    }

    fn list_for_game(&self, game_id: &GameId) -> Result<Vec<SaveBackupSummary>> {
        let mut backups = self
            .backups
            .lock()
            .unwrap()
            .iter()
            .filter(|backup| &backup.game_id == game_id)
            .cloned()
            .collect::<Vec<_>>();
        sort_latest_first(&mut backups);
        Ok(backups)
    }

    fn mark_status(&self, backup_id: &str, status: SaveBackupStatus) -> Result<()> {
        if let Some(backup) = self
            .backups
            .lock()
            .unwrap()
            .iter_mut()
            .find(|backup| backup.backup_id == backup_id)
        {
            backup.status = status;
        }
        Ok(())
    }

    fn update_note(
        &self,
        game_id: &GameId,
        profile_id: &ProfileId,
        backup_id: &str,
        note: Option<&str>,
    ) -> Result<bool> {
        let mut backups = self.backups.lock().unwrap();
        let Some(backup) = backups.iter_mut().find(|backup| {
            &backup.game_id == game_id
                && &backup.profile_id == profile_id
                && backup.backup_id == backup_id
        }) else {
            return Ok(false);
        };
        backup.notes = note.map(str::to_owned);
        Ok(true)
    }
}

fn sort_latest_first(backups: &mut [SaveBackupSummary]) {
    backups.sort_by(|left, right| {
        right
            .created_at
            .cmp(&left.created_at)
            .then_with(|| right.backup_id.cmp(&left.backup_id))
    });
}

#[derive(Default)]
struct FakeBackupWriter {
    deleted: Mutex<Vec<String>>,
}

impl FakeBackupWriter {
    fn take_deleted(&self) -> Vec<String> {
        std::mem::take(&mut *self.deleted.lock().unwrap())
    }
}

impl SaveBackupWriter for FakeBackupWriter {
    fn write_backup(&self, _request: SaveBackupWriteRequest) -> Result<SaveBackupWriteResult> {
        anyhow::bail!("write not used by backup center tests")
    }

    fn delete_backup_files(
        &self,
        _backup_directory: &ProfileDirectorySelection,
        summary: &SaveBackupSummary,
    ) -> Result<()> {
        self.deleted.lock().unwrap().push(summary.backup_id.clone());
        Ok(())
    }
}

struct FakeDirectoryValidator;

impl ProfileSaveDirectoryValidator for FakeDirectoryValidator {
    fn validate_save_directory(
        &self,
        _game_id: &str,
        _directory: &str,
    ) -> Result<ProfileDirectorySelection> {
        Ok(custom_directory())
    }

    fn validate_backup_directory(
        &self,
        _game_id: &str,
        _directory: &str,
    ) -> Result<ProfileDirectorySelection> {
        Ok(default_backup_directory())
    }

    fn default_backup_directory(&self, _game_id: &str) -> Result<ProfileDirectorySelection> {
        Ok(default_backup_directory())
    }
}

#[derive(Default)]
struct RecordingAuditLog {
    events: Mutex<Vec<AuditLogEvent>>,
    fail: Mutex<bool>,
}

impl RecordingAuditLog {
    fn set_fail(&self, fail: bool) {
        *self.fail.lock().unwrap() = fail;
    }

    fn take_events(&self) -> Vec<AuditLogEvent> {
        std::mem::take(&mut *self.events.lock().unwrap())
    }
}

impl AuditLogWriter for RecordingAuditLog {
    fn record(&self, event: AuditLogEvent) -> Result<()> {
        if *self.fail.lock().unwrap() {
            anyhow::bail!("injected audit failure");
        }
        self.events.lock().unwrap().push(event);
        Ok(())
    }
}

struct FixedClock;

impl AppClock for FixedClock {
    fn now_unix_millis(&self) -> Result<u128> {
        Ok(50)
    }
}

#[allow(clippy::too_many_arguments)]
fn sample_backup(
    backup_id: &str,
    profile_id: &str,
    trigger: SaveBackupTrigger,
    status: SaveBackupStatus,
    created_at: u128,
    archive_size_bytes: u64,
    retention_released_bytes: u64,
    notes: Option<&str>,
) -> SaveBackupSummary {
    SaveBackupSummary {
        backup_id: backup_id.to_owned(),
        game_id: GameId::mhw(),
        profile_id: ProfileId::new(profile_id),
        trigger,
        status,
        archive_file_name: format!("{backup_id}.zip"),
        manifest_file_name: format!("{backup_id}.manifest.json"),
        archive_size_bytes,
        retention_released_bytes,
        archive_sha256: "sha256:fixture".to_owned(),
        file_count: 1,
        created_at,
        source_path_label: Some("fixture".to_owned()),
        source_path_hash: "sha256:source".to_owned(),
        backup_directory: default_backup_directory(),
        notes: notes.map(str::to_owned),
    }
}

fn custom_directory() -> ProfileDirectorySelection {
    ProfileDirectorySelection {
        mode: ProfileDirectoryMode::Custom,
        status: ProfileDirectoryStatus::Valid,
        directory: Some("C:/synthetic/save".to_owned()),
        path_label: Some("save".to_owned()),
        messages: Vec::new(),
    }
}

fn default_backup_directory() -> ProfileDirectorySelection {
    ProfileDirectorySelection {
        mode: ProfileDirectoryMode::Default,
        status: ProfileDirectoryStatus::Defaulted,
        directory: None,
        path_label: Some("HelsincyModManager/backups/saves/mhw/profile".to_owned()),
        messages: Vec::new(),
    }
}
