use anyhow::{bail, ensure, Result};
use hmm_core::{
    BackupCadence, Profile, ProfileBackupRetention, ProfileBackupSchedule, ProfileDirectoryMode,
    ProfileDirectorySelection, ProfileDirectoryStatus, ProfileSaveSettings, DEFAULT_PROFILE_ID,
};
use hmm_ports::{
    AppClock, ProfileRepository, ProfileSaveDirectoryValidator, ProfileSaveSettingsRepository,
};
use std::sync::Arc;
use uuid::Uuid;

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
    clock: Arc<dyn AppClock>,
}

impl ProfileService {
    pub fn new(
        profile_repository: Arc<dyn ProfileRepository>,
        save_settings_repository: Arc<dyn ProfileSaveSettingsRepository>,
        save_directory_validator: Arc<dyn ProfileSaveDirectoryValidator>,
        clock: Arc<dyn AppClock>,
    ) -> Self {
        Self {
            profile_repository,
            save_settings_repository,
            save_directory_validator,
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
            pre_restore_backup_enabled: true,
            updated_at: 0,
        })
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

        let settings = ProfileSaveSettings {
            profile_id: request.profile_id,
            save_directory,
            backup_directory,
            schedule: request.schedule,
            retention: request.retention,
            pre_restore_backup_enabled: request.pre_restore_backup_enabled,
            updated_at: self.clock.now_unix_millis()?,
        };

        self.save_settings_repository.save_settings(&settings)?;
        Ok(settings)
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
        retention.max_count > 0,
        "backup retention max count must be greater than zero"
    );
    if let Some(max_age_days) = retention.max_age_days {
        ensure!(
            max_age_days > 0,
            "backup retention max age days must be greater than zero"
        );
    }
    Ok(())
}
