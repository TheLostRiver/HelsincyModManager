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

/// 在系统文件管理器中打开一个已配置的目录。
///
/// 刻意只接受 `&Path` 而不是前端传入的字符串:调用方(app 层)必须先从后端持久化事实里
/// 解析出路径,前端全程不经手路径,也就无法借这个能力打开任意位置。
/// 实现方负责在打开前拒绝 symlink、重解析点和非目录。
pub trait SystemDirectoryOpener: Send + Sync {
    fn open_directory(&self, path: &std::path::Path) -> Result<()>;
}
