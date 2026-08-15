pub const DEFAULT_PROFILE_ID: &str = "default";

/// A user-editable mod loadout scope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Profile {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub is_active: bool,
    pub created_at: u128,
    pub updated_at: u128,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackupCadence {
    Manual,
    Daily,
    Weekly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfileDirectoryStatus {
    Unset,
    Valid,
    Invalid,
    Defaulted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfileDirectoryMode {
    Unset,
    Custom,
    Default,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileDirectorySelection {
    pub mode: ProfileDirectoryMode,
    pub status: ProfileDirectoryStatus,
    pub directory: Option<String>,
    pub path_label: Option<String>,
    pub messages: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileBackupSchedule {
    pub cadence: BackupCadence,
    pub hour: Option<u8>,
    pub minute: Option<u8>,
    pub weekdays: Vec<u8>,
}

impl ProfileBackupSchedule {
    pub fn manual() -> Self {
        Self {
            cadence: BackupCadence::Manual,
            hour: None,
            minute: None,
            weekdays: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileBackupRetention {
    pub max_count: u32,
    pub max_age_days: Option<u32>,
    pub max_total_bytes: Option<u64>,
}

impl Default for ProfileBackupRetention {
    fn default() -> Self {
        Self {
            max_count: 20,
            max_age_days: Some(30),
            max_total_bytes: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SteamAccountDisplaySummary {
    pub account_name: Option<String>,
    pub avatar_url: Option<String>,
    pub account_label: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileSaveSettings {
    pub profile_id: String,
    pub save_directory: ProfileDirectorySelection,
    pub backup_directory: ProfileDirectorySelection,
    pub schedule: ProfileBackupSchedule,
    pub retention: ProfileBackupRetention,
    pub steam_account: Option<SteamAccountDisplaySummary>,
    pub pre_restore_backup_enabled: bool,
    pub updated_at: u128,
}
