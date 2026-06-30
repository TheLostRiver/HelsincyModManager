use anyhow::Result;
use hmm_core::{Profile, ProfileDirectorySelection, ProfileSaveSettings};

pub trait ProfileRepository: Send + Sync {
    fn get(&self, profile_id: &str) -> Result<Option<Profile>>;
    fn save(&self, profile: &Profile) -> Result<()>;
    fn delete(&self, profile_id: &str) -> Result<()>;
    fn list_all(&self) -> Result<Vec<Profile>>;
    fn get_active(&self) -> Result<Option<Profile>>;
    fn set_active(&self, profile_id: &str, updated_at: u128) -> Result<()>;
}

pub trait ProfileSaveSettingsRepository: Send + Sync {
    fn get_settings(&self, profile_id: &str) -> Result<Option<ProfileSaveSettings>>;
    fn save_settings(&self, settings: &ProfileSaveSettings) -> Result<()>;
}

pub trait ProfileSaveDirectoryValidator: Send + Sync {
    fn validate_save_directory(
        &self,
        game_id: &str,
        directory: &str,
    ) -> Result<ProfileDirectorySelection>;

    fn validate_backup_directory(
        &self,
        game_id: &str,
        directory: &str,
    ) -> Result<ProfileDirectorySelection>;

    fn default_backup_directory(&self, game_id: &str) -> Result<ProfileDirectorySelection>;
}
