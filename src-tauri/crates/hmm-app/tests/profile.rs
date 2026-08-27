use anyhow::Result;
use hmm_app::{CreateProfileRequest, ProfileDirectoryKind, ProfileService, UpdateProfileRequest};
use hmm_core::{
    Profile, ProfileBackupRetention, ProfileBackupSchedule, ProfileDirectoryMode,
    ProfileDirectorySelection, ProfileDirectoryStatus, ProfileSaveSettings,
    SteamAccountDisplaySummary,
};
use hmm_ports::{
    AppClock, ProfileRepository, ProfileSaveDirectoryValidator, ProfileSaveSettingsRepository,
};
use std::sync::{Arc, Mutex};

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
        let mut profiles = self.profiles.lock().unwrap().clone();
        profiles.sort_by(|a, b| a.created_at.cmp(&b.created_at).then(a.name.cmp(&b.name)));
        Ok(profiles)
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

/// 记录被请求打开的路径,不真的启动文件管理器——测试断言的是「选了哪个目录」,
/// 不是系统行为。
#[derive(Default)]
struct RecordingDirectoryOpener {
    opened: Mutex<Vec<String>>,
}

impl hmm_ports::SystemDirectoryOpener for RecordingDirectoryOpener {
    fn open_directory(&self, path: &std::path::Path) -> Result<()> {
        self.opened
            .lock()
            .unwrap()
            .push(path.to_string_lossy().into_owned());
        Ok(())
    }
}

/// 记录 locator 收到的 (game_id, profile_id) 并返回固定路径——测试断言的是
/// 「defaulted 走 locator、custom 不走」,真实布局与补建在 hmm-infra 单测覆盖。
struct RecordingBackupDirectoryLocator {
    calls: Mutex<Vec<(String, String)>>,
    directory: std::path::PathBuf,
}

impl RecordingBackupDirectoryLocator {
    fn returning(directory: &str) -> Self {
        Self {
            calls: Mutex::new(Vec::new()),
            directory: std::path::PathBuf::from(directory),
        }
    }
}

impl hmm_ports::SaveBackupDirectoryLocator for RecordingBackupDirectoryLocator {
    fn backup_directory_for_profile(
        &self,
        _selection: &ProfileDirectorySelection,
        game_id: &str,
        profile_id: &str,
    ) -> Result<std::path::PathBuf> {
        self.calls
            .lock()
            .unwrap()
            .push((game_id.to_owned(), profile_id.to_owned()));
        Ok(self.directory.clone())
    }
}

struct FixedClock(u128);

impl AppClock for FixedClock {
    fn now_unix_millis(&self) -> Result<u128> {
        Ok(self.0)
    }
}

fn make_service() -> (ProfileService, Arc<FakeProfileRepository>) {
    let (service, repo, _) = make_service_with_settings();
    (service, repo)
}

fn make_service_with_settings() -> (
    ProfileService,
    Arc<FakeProfileRepository>,
    Arc<FakeProfileSaveSettingsRepository>,
) {
    let (service, repo, settings, _, _) = make_service_with_opener();
    (service, repo, settings)
}

fn make_service_with_opener() -> (
    ProfileService,
    Arc<FakeProfileRepository>,
    Arc<FakeProfileSaveSettingsRepository>,
    Arc<RecordingBackupDirectoryLocator>,
    Arc<RecordingDirectoryOpener>,
) {
    let repo = Arc::new(FakeProfileRepository::default());
    let settings_repo = Arc::new(FakeProfileSaveSettingsRepository::default());
    let validator = Arc::new(FakeProfileSaveDirectoryValidator);
    let locator = Arc::new(RecordingBackupDirectoryLocator::returning(
        "D:/managed/saves/mhw/profile-1",
    ));
    let opener = Arc::new(RecordingDirectoryOpener::default());
    let service = ProfileService::new(
        Arc::clone(&repo) as _,
        Arc::clone(&settings_repo) as _,
        validator,
        Arc::clone(&locator) as _,
        Arc::clone(&opener) as _,
        Arc::new(FixedClock(7000)),
    );
    (service, repo, settings_repo, locator, opener)
}

#[derive(Default)]
struct FakeProfileSaveSettingsRepository {
    settings: Mutex<Vec<ProfileSaveSettings>>,
}

impl FakeProfileSaveSettingsRepository {
    /// 直接塞入已配置好两个目录的 settings,跳过校验器——本测试关心的是
    /// 「按 kind 取对目录」,不是目录校验本身。
    fn seed_directories(&self, profile_id: &str, save: &str, backup: &str) {
        let selection = |directory: &str| ProfileDirectorySelection {
            mode: ProfileDirectoryMode::Custom,
            status: ProfileDirectoryStatus::Valid,
            directory: Some(directory.to_owned()),
            path_label: None,
            messages: Vec::new(),
        };
        self.settings.lock().unwrap().push(ProfileSaveSettings {
            profile_id: profile_id.to_owned(),
            save_directory: selection(save),
            backup_directory: selection(backup),
            schedule: ProfileBackupSchedule::manual(),
            retention: ProfileBackupRetention::default(),
            steam_account: None,
            pre_restore_backup_enabled: true,
            updated_at: 0,
        });
    }

    /// 塞入任意组合的两个目录 selection,给 defaulted/数据破损分支的测试用。
    fn seed_selections(
        &self,
        profile_id: &str,
        save: ProfileDirectorySelection,
        backup: ProfileDirectorySelection,
    ) {
        self.settings.lock().unwrap().push(ProfileSaveSettings {
            profile_id: profile_id.to_owned(),
            save_directory: save,
            backup_directory: backup,
            schedule: ProfileBackupSchedule::manual(),
            retention: ProfileBackupRetention::default(),
            steam_account: None,
            pre_restore_backup_enabled: true,
            updated_at: 0,
        });
    }
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

    fn default_backup_directory(&self, game_id: &str) -> Result<ProfileDirectorySelection> {
        Ok(ProfileDirectorySelection {
            mode: ProfileDirectoryMode::Default,
            status: ProfileDirectoryStatus::Defaulted,
            directory: None,
            path_label: Some(format!("{game_id}/HelsincyModManager/Backups")),
            messages: vec!["使用默认备份目录".to_owned()],
        })
    }
}

fn custom_directory_selection(directory: &str) -> ProfileDirectorySelection {
    ProfileDirectorySelection {
        mode: ProfileDirectoryMode::Custom,
        status: ProfileDirectoryStatus::Valid,
        directory: Some(directory.to_owned()),
        path_label: Some(path_label(directory)),
        messages: Vec::new(),
    }
}

fn defaulted_directory_selection() -> ProfileDirectorySelection {
    ProfileDirectorySelection {
        mode: ProfileDirectoryMode::Default,
        status: ProfileDirectoryStatus::Defaulted,
        directory: None,
        path_label: None,
        messages: Vec::new(),
    }
}

fn path_label(path: &str) -> String {
    path.replace('\\', "/")
        .rsplit('/')
        .next()
        .unwrap_or(path)
        .to_owned()
}

#[test]
fn create_profile_trims_fields_and_saves_inactive_profile() {
    let (service, repo) = make_service();

    let id = service
        .create_profile(CreateProfileRequest {
            name: "  Hunt Loadout  ".to_owned(),
            description: Some("  Iceborne testing  ".to_owned()),
        })
        .unwrap();

    let saved = repo.get(&id).unwrap().expect("profile should exist");
    assert_eq!(saved.name, "Hunt Loadout");
    assert_eq!(saved.description.as_deref(), Some("Iceborne testing"));
    assert!(!saved.is_active);
    assert_eq!(saved.created_at, 7000);
    assert_eq!(saved.updated_at, 7000);
}

#[test]
fn create_profile_rejects_empty_name() {
    let (service, _) = make_service();

    let result = service.create_profile(CreateProfileRequest {
        name: "   ".to_owned(),
        description: None,
    });

    assert!(result.is_err());
}

#[test]
fn update_profile_merges_optional_fields_and_refreshes_timestamp() {
    let (service, repo) = make_service();
    let id = service
        .create_profile(CreateProfileRequest {
            name: "Old".to_owned(),
            description: Some("Original".to_owned()),
        })
        .unwrap();

    service
        .update_profile(UpdateProfileRequest {
            profile_id: id.clone(),
            name: Some("  New  ".to_owned()),
            description: Some(None),
        })
        .unwrap();

    let saved = repo.get(&id).unwrap().expect("profile should exist");
    assert_eq!(saved.name, "New");
    assert!(saved.description.is_none());
    assert_eq!(saved.updated_at, 7000);
}

#[test]
fn set_active_profile_deactivates_previous_active_profile() {
    let (service, repo) = make_service();
    repo.save(&Profile {
        id: "default".to_owned(),
        name: "Default".to_owned(),
        description: None,
        is_active: true,
        created_at: 1,
        updated_at: 1,
    })
    .unwrap();
    repo.save(&Profile {
        id: "profile-2".to_owned(),
        name: "Second".to_owned(),
        description: None,
        is_active: false,
        created_at: 2,
        updated_at: 2,
    })
    .unwrap();

    service.set_active_profile("profile-2").unwrap();

    let default = repo.get("default").unwrap().unwrap();
    let second = repo.get("profile-2").unwrap().unwrap();
    assert!(!default.is_active);
    assert!(second.is_active);
    assert_eq!(second.updated_at, 7000);
}

#[test]
fn delete_rejects_default_and_active_profiles() {
    let (service, repo) = make_service();
    repo.save(&Profile {
        id: "default".to_owned(),
        name: "Default".to_owned(),
        description: None,
        is_active: false,
        created_at: 1,
        updated_at: 1,
    })
    .unwrap();
    repo.save(&Profile {
        id: "active".to_owned(),
        name: "Active".to_owned(),
        description: None,
        is_active: true,
        created_at: 2,
        updated_at: 2,
    })
    .unwrap();

    assert!(service.delete_profile("default").is_err());
    assert!(service.delete_profile("active").is_err());
}

#[test]
fn profile_save_settings_rejects_unknown_profile() {
    let (service, _repo) = make_service();

    let result = service.set_profile_save_settings(hmm_app::SetProfileSaveSettingsRequest {
        profile_id: "missing".to_owned(),
        game_id: "mhw".to_owned(),
        save_directory: Some("C:/Users/Test/Saves".to_owned()),
        backup_directory: None,
        schedule: hmm_core::ProfileBackupSchedule::manual(),
        retention: hmm_core::ProfileBackupRetention::default(),
        pre_restore_backup_enabled: true,
    });

    assert!(result.is_err());
}

#[test]
fn get_profile_save_settings_uses_requested_game_id_for_default_backup() {
    let (service, repo) = make_service();
    repo.save(&Profile {
        id: "profile-game".to_owned(),
        name: "Profile".to_owned(),
        description: None,
        is_active: false,
        created_at: 1,
        updated_at: 1,
    })
    .unwrap();

    let settings = service
        .get_profile_save_settings("mhw-test", "profile-game")
        .expect("settings should be available");

    assert_eq!(
        settings.backup_directory.path_label.as_deref(),
        Some("mhw-test/HelsincyModManager/Backups")
    );
    assert_eq!(settings.retention.max_count, 0);
    assert_eq!(settings.retention.max_age_days, None);
    assert_eq!(settings.retention.max_total_bytes, None);
}

#[test]
fn profile_save_settings_validates_selected_directories_before_persisting() {
    let (service, repo) = make_service();
    repo.save(&Profile {
        id: "profile-1".to_owned(),
        name: "Profile".to_owned(),
        description: None,
        is_active: false,
        created_at: 1,
        updated_at: 1,
    })
    .unwrap();

    let result = service.set_profile_save_settings(hmm_app::SetProfileSaveSettingsRequest {
        profile_id: "profile-1".to_owned(),
        game_id: "mhw".to_owned(),
        save_directory: Some("C:/Users/Test/Saves".to_owned()),
        backup_directory: Some("D:/HMM/Backups".to_owned()),
        schedule: hmm_core::ProfileBackupSchedule {
            cadence: hmm_core::BackupCadence::Daily,
            hour: Some(3),
            minute: Some(0),
            weekdays: Vec::new(),
        },
        retention: hmm_core::ProfileBackupRetention {
            max_count: 20,
            max_age_days: Some(30),
            max_total_bytes: None,
        },
        pre_restore_backup_enabled: false,
    });

    let settings = result.expect("settings saved");
    assert_eq!(settings.profile_id, "profile-1");
    assert_eq!(settings.save_directory.path_label.as_deref(), Some("Saves"));
    assert_eq!(
        settings.backup_directory.path_label.as_deref(),
        Some("Backups")
    );
    assert_eq!(settings.schedule.cadence, hmm_core::BackupCadence::Daily);
    assert_eq!(settings.retention.max_count, 20);
    assert!(!settings.pre_restore_backup_enabled);
}

#[test]
fn profile_save_settings_rejects_out_of_range_schedule_time() {
    let (service, repo) = make_service();
    repo.save(&Profile {
        id: "profile-1".to_owned(),
        name: "Profile".to_owned(),
        description: None,
        is_active: false,
        created_at: 1,
        updated_at: 1,
    })
    .unwrap();

    let invalid_hour = service.set_profile_save_settings(hmm_app::SetProfileSaveSettingsRequest {
        profile_id: "profile-1".to_owned(),
        game_id: "mhw".to_owned(),
        save_directory: None,
        backup_directory: None,
        schedule: hmm_core::ProfileBackupSchedule {
            cadence: hmm_core::BackupCadence::Daily,
            hour: Some(24),
            minute: Some(0),
            weekdays: Vec::new(),
        },
        retention: hmm_core::ProfileBackupRetention::default(),
        pre_restore_backup_enabled: true,
    });
    assert!(invalid_hour.is_err());

    let invalid_minute =
        service.set_profile_save_settings(hmm_app::SetProfileSaveSettingsRequest {
            profile_id: "profile-1".to_owned(),
            game_id: "mhw".to_owned(),
            save_directory: None,
            backup_directory: None,
            schedule: hmm_core::ProfileBackupSchedule {
                cadence: hmm_core::BackupCadence::Weekly,
                hour: Some(23),
                minute: Some(60),
                weekdays: vec![1],
            },
            retention: hmm_core::ProfileBackupRetention::default(),
            pre_restore_backup_enabled: true,
        });
    assert!(invalid_minute.is_err());
}

#[test]
fn profile_save_settings_accepts_only_the_supported_space_budget_range() {
    let (service, repo) = make_service();
    repo.save(&Profile {
        id: "profile-1".to_owned(),
        name: "Profile".to_owned(),
        description: None,
        is_active: false,
        created_at: 1,
        updated_at: 1,
    })
    .unwrap();

    let request = |max_total_bytes| hmm_app::SetProfileSaveSettingsRequest {
        profile_id: "profile-1".to_owned(),
        game_id: "mhw".to_owned(),
        save_directory: None,
        backup_directory: None,
        schedule: hmm_core::ProfileBackupSchedule::manual(),
        retention: hmm_core::ProfileBackupRetention {
            max_count: 20,
            max_age_days: Some(30),
            max_total_bytes,
        },
        pre_restore_backup_enabled: true,
    };

    assert!(service
        .set_profile_save_settings(request(Some(16 * 1024 * 1024)))
        .is_ok());
    assert!(service
        .set_profile_save_settings(request(Some(1024 * 1024 * 1024 * 1024)))
        .is_ok());
    assert!(service
        .set_profile_save_settings(request(Some(16 * 1024 * 1024 - 1)))
        .is_err());
    assert!(service
        .set_profile_save_settings(request(Some(1024 * 1024 * 1024 * 1024 + 1)))
        .is_err());
}

#[test]
fn profile_save_settings_accepts_unbounded_count_and_rejects_zero_optional_domain_limits() {
    let (service, repo) = make_service();
    repo.save(&Profile {
        id: "profile-1".to_owned(),
        name: "Profile".to_owned(),
        description: None,
        is_active: false,
        created_at: 1,
        updated_at: 1,
    })
    .unwrap();

    let request =
        |max_count, max_age_days, max_total_bytes| hmm_app::SetProfileSaveSettingsRequest {
            profile_id: "profile-1".to_owned(),
            game_id: "mhw".to_owned(),
            save_directory: None,
            backup_directory: None,
            schedule: hmm_core::ProfileBackupSchedule::manual(),
            retention: hmm_core::ProfileBackupRetention {
                max_count,
                max_age_days,
                max_total_bytes,
            },
            pre_restore_backup_enabled: true,
        };

    assert!(service
        .set_profile_save_settings(request(0, None, None))
        .is_ok());
    assert!(service
        .set_profile_save_settings(request(1_000, None, None))
        .is_err());
    assert!(service
        .set_profile_save_settings(request(1, Some(0), None))
        .is_err());
    assert!(service
        .set_profile_save_settings(request(1, None, Some(0)))
        .is_err());
}

#[test]
fn profile_save_settings_preserve_account_for_same_directory_and_clear_it_for_a_new_one() {
    let (service, repo, settings_repo) = make_service_with_settings();
    repo.save(&Profile {
        id: "profile-1".to_owned(),
        name: "Profile".to_owned(),
        description: None,
        is_active: false,
        created_at: 1,
        updated_at: 1,
    })
    .unwrap();

    let original_directory = if cfg!(windows) {
        "C:\\Fixture\\Saves"
    } else {
        "/fixture/saves"
    };
    let equivalent_directory = if cfg!(windows) {
        "c:/fixture/saves"
    } else {
        "/fixture/saves"
    };
    let different_directory = if cfg!(windows) {
        "C:/Fixture/OtherSaves"
    } else {
        "/fixture/other-saves"
    };
    let steam_account = SteamAccountDisplaySummary {
        account_name: Some("Synthetic Hunter".to_owned()),
        avatar_url: Some("https://avatars.steamstatic.com/fixture.jpg".to_owned()),
        account_label: "Steam 12****34".to_owned(),
    };
    settings_repo
        .save_settings(&ProfileSaveSettings {
            profile_id: "profile-1".to_owned(),
            save_directory: custom_directory_selection(original_directory),
            backup_directory: custom_directory_selection("D:/Fixture/Backups"),
            schedule: hmm_core::ProfileBackupSchedule::manual(),
            retention: hmm_core::ProfileBackupRetention::default(),
            steam_account: Some(steam_account.clone()),
            pre_restore_backup_enabled: true,
            updated_at: 1,
        })
        .unwrap();

    let request = |save_directory: &str| hmm_app::SetProfileSaveSettingsRequest {
        profile_id: "profile-1".to_owned(),
        game_id: "mhw".to_owned(),
        save_directory: Some(save_directory.to_owned()),
        backup_directory: None,
        schedule: hmm_core::ProfileBackupSchedule::manual(),
        retention: hmm_core::ProfileBackupRetention::default(),
        pre_restore_backup_enabled: true,
    };

    let preserved = service
        .set_profile_save_settings(request(equivalent_directory))
        .expect("save equivalent directory");
    assert_eq!(preserved.steam_account, Some(steam_account));

    let cleared = service
        .set_profile_save_settings(request(different_directory))
        .expect("save different directory");
    assert_eq!(cleared.steam_account, None);
}

#[test]
fn open_profile_directory_picks_the_configured_directory_for_each_kind() {
    let (service, _repo, settings_repo, locator, opener) = make_service_with_opener();
    let profile_id = service
        .create_profile(CreateProfileRequest {
            name: "profile".to_owned(),
            description: None,
        })
        .expect("create profile");
    settings_repo.seed_directories(&profile_id, "D:/saves", "D:/backups");

    service
        .open_profile_directory("mhw", &profile_id, ProfileDirectoryKind::Save)
        .expect("open save directory");
    service
        .open_profile_directory("mhw", &profile_id, ProfileDirectoryKind::Backup)
        .expect("open backup directory");

    // 两个种类必须各自解析到自己的目录,不能串。
    assert_eq!(
        opener.opened.lock().unwrap().as_slice(),
        ["D:/saves".to_owned(), "D:/backups".to_owned()]
    );
    // 自定义目录按玩家选的原样打开,不经过托管布局解析。
    assert!(locator.calls.lock().unwrap().is_empty());
}

#[test]
fn open_profile_directory_resolves_the_managed_directory_for_a_defaulted_backup() {
    let (service, _repo, _settings_repo, locator, opener) = make_service_with_opener();
    let profile_id = service
        .create_profile(CreateProfileRequest {
            name: "profile".to_owned(),
            description: None,
        })
        .expect("create profile");

    // 刻意不 seed settings:从未配置过的 profile 走 validator 兜底,
    // 备份目录即 defaulted——这正是「从未备份过就点按钮」的场景。
    service
        .open_profile_directory("mhw", &profile_id, ProfileDirectoryKind::Backup)
        .expect("open defaulted backup directory");

    // 打开的必须是 locator 解析出的托管路径,且 locator 收到正确的上下文。
    assert_eq!(
        opener.opened.lock().unwrap().as_slice(),
        ["D:/managed/saves/mhw/profile-1".to_owned()]
    );
    assert_eq!(
        locator.calls.lock().unwrap().as_slice(),
        [("mhw".to_owned(), profile_id.clone())]
    );
}

#[test]
fn open_profile_directory_refuses_a_defaulted_save_directory() {
    let (service, _repo, settings_repo, locator, opener) = make_service_with_opener();
    let profile_id = service
        .create_profile(CreateProfileRequest {
            name: "profile".to_owned(),
            description: None,
        })
        .expect("create profile");
    settings_repo.seed_selections(
        &profile_id,
        defaulted_directory_selection(),
        custom_directory_selection("D:/backups"),
    );

    // save 目录没有托管默认;即使出现 defaulted 的 save selection 也不解析、不打开。
    assert!(service
        .open_profile_directory("mhw", &profile_id, ProfileDirectoryKind::Save)
        .is_err());
    assert!(locator.calls.lock().unwrap().is_empty());
    assert!(opener.opened.lock().unwrap().is_empty());
}

#[test]
fn open_profile_directory_refuses_a_custom_selection_without_a_directory() {
    let (service, _repo, settings_repo, locator, opener) = make_service_with_opener();
    let profile_id = service
        .create_profile(CreateProfileRequest {
            name: "profile".to_owned(),
            description: None,
        })
        .expect("create profile");
    let mut broken = custom_directory_selection("D:/backups");
    broken.directory = None;
    settings_repo.seed_selections(&profile_id, custom_directory_selection("D:/saves"), broken);

    // Custom 缺 directory 是数据破损,不得误当托管默认去解析。
    assert!(service
        .open_profile_directory("mhw", &profile_id, ProfileDirectoryKind::Backup)
        .is_err());
    assert!(locator.calls.lock().unwrap().is_empty());
    assert!(opener.opened.lock().unwrap().is_empty());
}

#[test]
fn open_profile_directory_refuses_when_the_directory_is_unset() {
    let (service, _repo, _settings_repo, locator, opener) = make_service_with_opener();
    let profile_id = service
        .create_profile(CreateProfileRequest {
            name: "profile".to_owned(),
            description: None,
        })
        .expect("create profile");

    // 未配置时报稳定错误,绝不退化成打开某个默认位置。
    assert!(service
        .open_profile_directory("mhw", &profile_id, ProfileDirectoryKind::Save)
        .is_err());
    assert!(locator.calls.lock().unwrap().is_empty());
    assert!(opener.opened.lock().unwrap().is_empty());
}
