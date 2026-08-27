use anyhow::{bail, ensure, Result};
use hmm_core::{
    BackupCadence, Profile, ProfileBackupRetention, ProfileBackupSchedule, ProfileDirectoryMode,
    ProfileDirectorySelection, ProfileDirectoryStatus, ProfileSaveSettings, DEFAULT_PROFILE_ID,
};
use hmm_ports::{
    AppClock, ProfileRepository, ProfileSaveDirectoryValidator, ProfileSaveSettingsRepository,
    SaveBackupDirectoryLocator, SystemDirectoryOpener,
};
use std::sync::Arc;
use uuid::Uuid;

/// 可被「打开文件夹」入口消费的 profile 目录种类。刻意用枚举而非字符串:
/// 前端只能在这两个值里选,无法表达任意目录。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfileDirectoryKind {
    Save,
    Backup,
}

pub struct CreateProfileRequest {
    pub name: String,
    pub description: Option<String>,
}

pub struct UpdateProfileRequest {
    pub profile_id: String,
    pub name: Option<String>,
    pub description: Option<Option<String>>,
}

pub struct SetProfileSaveSettingsRequest {
    pub profile_id: String,
    pub game_id: String,
    pub save_directory: Option<String>,
    pub backup_directory: Option<String>,
    pub schedule: ProfileBackupSchedule,
    pub retention: ProfileBackupRetention,
    pub pre_restore_backup_enabled: bool,
}

pub struct ProfileService {
    profile_repository: Arc<dyn ProfileRepository>,
    save_settings_repository: Arc<dyn ProfileSaveSettingsRepository>,
    save_directory_validator: Arc<dyn ProfileSaveDirectoryValidator>,
    backup_directory_locator: Arc<dyn SaveBackupDirectoryLocator>,
    directory_opener: Arc<dyn SystemDirectoryOpener>,
    clock: Arc<dyn AppClock>,
}

impl ProfileService {
    pub fn new(
        profile_repository: Arc<dyn ProfileRepository>,
        save_settings_repository: Arc<dyn ProfileSaveSettingsRepository>,
        save_directory_validator: Arc<dyn ProfileSaveDirectoryValidator>,
        backup_directory_locator: Arc<dyn SaveBackupDirectoryLocator>,
        directory_opener: Arc<dyn SystemDirectoryOpener>,
        clock: Arc<dyn AppClock>,
    ) -> Self {
        Self {
            profile_repository,
            save_settings_repository,
            save_directory_validator,
            backup_directory_locator,
            directory_opener,
            clock,
        }
    }

    pub fn create_profile(&self, request: CreateProfileRequest) -> Result<String> {
        let name = normalize_required_name(request.name)?;
        let description = normalize_optional_string(request.description);
        let now = self.clock.now_unix_millis()?;
        let id = Uuid::new_v4().to_string();

        let profile = Profile {
            id: id.clone(),
            name,
            description,
            is_active: false,
            created_at: now,
            updated_at: now,
        };

        self.profile_repository.save(&profile)?;
        Ok(id)
    }

    pub fn update_profile(&self, request: UpdateProfileRequest) -> Result<()> {
        let existing = self
            .profile_repository
            .get(&request.profile_id)?
            .ok_or_else(|| anyhow::anyhow!("profile not found: {}", request.profile_id))?;

        let name = match request.name {
            Some(name) => normalize_required_name(name)?,
            None => existing.name,
        };
        let description = match request.description {
            Some(description) => normalize_optional_string(description),
            None => existing.description,
        };
        let now = self.clock.now_unix_millis()?;

        self.profile_repository.save(&Profile {
            id: existing.id,
            name,
            description,
            is_active: existing.is_active,
            created_at: existing.created_at,
            updated_at: now,
        })
    }

    pub fn delete_profile(&self, profile_id: &str) -> Result<()> {
        if profile_id == DEFAULT_PROFILE_ID {
            bail!("default profile cannot be deleted");
        }

        let existing = self
            .profile_repository
            .get(profile_id)?
            .ok_or_else(|| anyhow::anyhow!("profile not found: {profile_id}"))?;
        if existing.is_active {
            bail!("active profile cannot be deleted");
        }

        self.profile_repository.delete(profile_id)
    }

    pub fn list_profiles(&self) -> Result<Vec<Profile>> {
        self.profile_repository.list_all()
    }

    pub fn get_active_profile(&self) -> Result<Profile> {
        self.profile_repository
            .get_active()?
            .ok_or_else(|| anyhow::anyhow!("active profile not found"))
    }

    pub fn set_active_profile(&self, profile_id: &str) -> Result<()> {
        self.profile_repository
            .get(profile_id)?
            .ok_or_else(|| anyhow::anyhow!("profile not found: {profile_id}"))?;
        let now = self.clock.now_unix_millis()?;
        self.profile_repository.set_active(profile_id, now)
    }

    pub fn get_profile_save_settings(
        &self,
        game_id: &str,
        profile_id: &str,
    ) -> Result<ProfileSaveSettings> {
        self.profile_repository
            .get(profile_id)?
            .ok_or_else(|| anyhow::anyhow!("profile not found: {profile_id}"))?;

        if let Some(settings) = self.save_settings_repository.get_settings(profile_id)? {
            return Ok(settings);
        }

        Ok(ProfileSaveSettings {
            profile_id: profile_id.to_owned(),
            save_directory: unset_save_directory(),
            backup_directory: self
                .save_directory_validator
                .default_backup_directory(game_id)?,
            schedule: ProfileBackupSchedule::manual(),
            retention: ProfileBackupRetention::default(),
            steam_account: None,
            pre_restore_backup_enabled: true,
            updated_at: 0,
        })
    }

    /// 在系统文件管理器中打开该 profile 已配置的存档或备份目录。
    ///
    /// 路径只从后端持久化事实解析,调用方(Tauri command)只传 profile 与目录种类。
    /// 只有应用自有的托管默认备份目录会被按需补建;玩家自选的 Custom 目录一律
    /// 按原样打开,绝不因为一次打开动作向玩家目录写入。未配置时返回稳定错误,
    /// 而不是退化成打开某个默认位置。
    pub fn open_profile_directory(
        &self,
        game_id: &str,
        profile_id: &str,
        kind: ProfileDirectoryKind,
    ) -> Result<()> {
        let settings = self.get_profile_save_settings(game_id, profile_id)?;
        let selection = match kind {
            ProfileDirectoryKind::Save => settings.save_directory,
            ProfileDirectoryKind::Backup => settings.backup_directory,
        };
        // 按 mode 穷尽分支:directory 的 Some/None 只是持久化编码(NULL=托管默认),
        // 语义由 mode 承担。若按 Option 分支,理论上的 Unset 备份目录会被布局函数
        // 静默解析到默认根并补建目录,穷尽 match 封死此路。
        let directory = match selection.mode {
            ProfileDirectoryMode::Custom => selection
                .directory
                .clone()
                .map(std::path::PathBuf::from)
                .ok_or_else(|| anyhow::anyhow!("profile directory is not configured"))?,
            ProfileDirectoryMode::Default => match kind {
                ProfileDirectoryKind::Backup => self
                    .backup_directory_locator
                    .backup_directory_for_profile(&selection, game_id, profile_id)?,
                // save 目录没有托管默认;当前不可构造,显式封死。
                ProfileDirectoryKind::Save => {
                    bail!("save directory has no managed default location")
                }
            },
            ProfileDirectoryMode::Unset => {
                bail!("profile directory is not configured")
            }
        };
        self.directory_opener.open_directory(&directory)
    }

    pub fn validate_profile_save_directory(
        &self,
        game_id: &str,
        directory: &str,
    ) -> Result<ProfileDirectorySelection> {
        self.save_directory_validator
            .validate_save_directory(game_id, directory)
    }

    pub fn validate_profile_backup_directory(
        &self,
        game_id: &str,
        directory: &str,
    ) -> Result<ProfileDirectorySelection> {
        self.save_directory_validator
            .validate_backup_directory(game_id, directory)
    }

    pub fn set_profile_save_settings(
        &self,
        request: SetProfileSaveSettingsRequest,
    ) -> Result<ProfileSaveSettings> {
        self.profile_repository
            .get(&request.profile_id)?
            .ok_or_else(|| anyhow::anyhow!("profile not found: {}", request.profile_id))?;
        let existing_settings = self
            .save_settings_repository
            .get_settings(&request.profile_id)?;

        let save_directory = match request.save_directory {
            Some(directory) => self
                .save_directory_validator
                .validate_save_directory(&request.game_id, &directory)?,
            None => existing_settings
                .as_ref()
                .map(|settings| settings.save_directory.clone())
                .unwrap_or_else(unset_save_directory),
        };

        let backup_directory = match request.backup_directory {
            Some(directory) => self
                .save_directory_validator
                .validate_backup_directory(&request.game_id, &directory)?,
            None => {
                if let Some(settings) = existing_settings.as_ref() {
                    settings.backup_directory.clone()
                } else {
                    self.save_directory_validator
                        .default_backup_directory(&request.game_id)?
                }
            }
        };

        validate_schedule(&request.schedule)?;
        validate_retention(&request.retention)?;

        let steam_account = existing_settings.as_ref().and_then(|settings| {
            directories_equivalent(
                settings.save_directory.directory.as_deref(),
                save_directory.directory.as_deref(),
            )
            .then(|| settings.steam_account.clone())
            .flatten()
        });

        let settings = ProfileSaveSettings {
            profile_id: request.profile_id,
            save_directory,
            backup_directory,
            schedule: request.schedule,
            retention: request.retention,
            steam_account,
            pre_restore_backup_enabled: request.pre_restore_backup_enabled,
            updated_at: self.clock.now_unix_millis()?,
        };

        self.save_settings_repository.save_settings(&settings)?;
        Ok(settings)
    }
}

fn directories_equivalent(left: Option<&str>, right: Option<&str>) -> bool {
    match (left, right) {
        (None, None) => true,
        (Some(left), Some(right)) if cfg!(windows) => left
            .replace('\\', "/")
            .eq_ignore_ascii_case(&right.replace('\\', "/")),
        (Some(left), Some(right)) => left == right,
        _ => false,
    }
}

fn normalize_required_name(value: String) -> Result<String> {
    let value = value.trim().to_owned();
    if value.is_empty() {
        bail!("profile name must not be empty");
    }
    Ok(value)
}

fn normalize_optional_string(value: Option<String>) -> Option<String> {
    value.map(|v| v.trim().to_owned()).filter(|v| !v.is_empty())
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

fn validate_schedule(schedule: &ProfileBackupSchedule) -> Result<()> {
    match schedule.cadence {
        BackupCadence::Manual => Ok(()),
        BackupCadence::Daily => {
            validate_schedule_time(schedule)?;
            Ok(())
        }
        BackupCadence::Weekly => {
            validate_schedule_time(schedule)?;
            ensure!(
                !schedule.weekdays.is_empty(),
                "weekly backup days are required"
            );
            ensure!(
                schedule.weekdays.iter().all(|day| *day <= 6),
                "weekly backup day must be between 0 and 6"
            );
            Ok(())
        }
    }
}

fn validate_schedule_time(schedule: &ProfileBackupSchedule) -> Result<()> {
    let hour = schedule
        .hour
        .ok_or_else(|| anyhow::anyhow!("backup hour is required"))?;
    let minute = schedule
        .minute
        .ok_or_else(|| anyhow::anyhow!("backup minute is required"))?;
    ensure!(hour <= 23, "backup hour must be between 0 and 23");
    ensure!(minute <= 59, "backup minute must be between 0 and 59");
    Ok(())
}

fn validate_retention(retention: &ProfileBackupRetention) -> Result<()> {
    ensure!(
        retention.max_count <= 999,
        "backup retention max count must be between 0 and 999"
    );
    if let Some(max_age_days) = retention.max_age_days {
        ensure!(
            (1..=3650).contains(&max_age_days),
            "backup retention max age days must be between 1 and 3650"
        );
    }
    if let Some(max_total_bytes) = retention.max_total_bytes {
        ensure!(
            (16 * 1024 * 1024..=1024 * 1024 * 1024 * 1024).contains(&max_total_bytes),
            "backup retention max total bytes must be between 16 MiB and 1 TiB"
        );
    }
    Ok(())
}
