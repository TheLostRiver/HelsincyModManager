use anyhow::{anyhow, Context, Result};
use hmm_core::{
    BackupCadence, Profile, ProfileBackupRetention, ProfileBackupSchedule, ProfileDirectoryMode,
    ProfileDirectorySelection, ProfileDirectoryStatus, ProfileSaveSettings,
    SteamAccountDisplaySummary,
};
use hmm_ports::{ProfileRepository, ProfileSaveDirectoryValidator, ProfileSaveSettingsRepository};
use rusqlite::Connection;
use std::path::Path;
use std::sync::{Arc, Mutex};

pub struct SqliteProfileRepository {
    db: Arc<Mutex<Connection>>,
}

impl SqliteProfileRepository {
    pub fn new(db: Arc<Mutex<Connection>>) -> Self {
        Self { db }
    }

    fn lock_db(&self) -> Result<std::sync::MutexGuard<'_, Connection>> {
        self.db
            .lock()
            .map_err(|_| anyhow!("database lock poisoned"))
    }

    fn row_to_profile(row: &rusqlite::Row<'_>) -> rusqlite::Result<Profile> {
        Ok(Profile {
            id: row.get(0)?,
            name: row.get(1)?,
            description: row.get(2)?,
            is_active: row.get::<_, i64>(3)? != 0,
            created_at: row.get::<_, i64>(4)? as u128,
            updated_at: row.get::<_, i64>(5)? as u128,
        })
    }

    fn row_to_profile_save_settings(
        row: &rusqlite::Row<'_>,
    ) -> rusqlite::Result<ProfileSaveSettings> {
        let profile_id: String = row.get(0)?;
        let save_directory: Option<String> = row.get(1)?;
        let backup_directory: Option<String> = row.get(2)?;
        let cadence: String = row.get(3)?;
        let backup_hour = optional_non_negative_u8(row, 4)?;
        let backup_minute = optional_non_negative_u8(row, 5)?;
        let backup_weekdays_json: String = row.get(6)?;
        let retention_max_count = non_negative_u32(row, 7)?;
        let retention_max_age_days = optional_non_negative_u32(row, 8)?;
        let retention_max_total_bytes = optional_non_negative_u64(row, 9)?;
        let pre_restore_backup_enabled: i64 = row.get(10)?;
        let steam_account_name: Option<String> = row.get(11)?;
        let steam_avatar_url: Option<String> = row.get(12)?;
        let steam_account_label: Option<String> = row.get(13)?;
        let updated_at = non_negative_u128(row, 14)?;

        let weekdays = serde_json::from_str::<Vec<u8>>(&backup_weekdays_json).map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                6,
                rusqlite::types::Type::Text,
                Box::new(error),
            )
        })?;

        Ok(ProfileSaveSettings {
            profile_id,
            save_directory: match save_directory {
                Some(directory) => custom_directory_selection(&directory),
                None => unset_save_directory(),
            },
            backup_directory: match backup_directory {
                Some(directory) => custom_directory_selection(&directory),
                None => default_backup_directory_selection(),
            },
            schedule: ProfileBackupSchedule {
                cadence: parse_backup_cadence(&cadence),
                hour: backup_hour,
                minute: backup_minute,
                weekdays,
            },
            retention: ProfileBackupRetention {
                max_count: retention_max_count,
                max_age_days: retention_max_age_days,
                max_total_bytes: retention_max_total_bytes,
            },
            steam_account: steam_account_label.map(|account_label| SteamAccountDisplaySummary {
                account_name: steam_account_name,
                avatar_url: steam_avatar_url,
                account_label,
            }),
            pre_restore_backup_enabled: pre_restore_backup_enabled != 0,
            updated_at,
        })
    }
}

impl ProfileRepository for SqliteProfileRepository {
    fn get(&self, profile_id: &str) -> Result<Option<Profile>> {
        let conn = self.lock_db()?;
        let mut stmt = conn
            .prepare(
                "SELECT profile_id, name, description, is_active, created_at, updated_at
                 FROM profiles WHERE profile_id = ?1",
            )
            .context("failed to prepare get profile query")?;

        let result = stmt.query_row(rusqlite::params![profile_id], Self::row_to_profile);

        match result {
            Ok(profile) => Ok(Some(profile)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(error) => Err(error).context("failed to get profile"),
        }
    }

    fn save(&self, profile: &Profile) -> Result<()> {
        let conn = self.lock_db()?;
        conn.execute(
            "INSERT INTO profiles
                (profile_id, name, description, is_active, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(profile_id) DO UPDATE SET
                name = excluded.name,
                description = excluded.description,
                is_active = excluded.is_active,
                created_at = excluded.created_at,
                updated_at = excluded.updated_at",
            rusqlite::params![
                profile.id,
                profile.name,
                profile.description,
                if profile.is_active { 1 } else { 0 },
                profile.created_at as i64,
                profile.updated_at as i64,
            ],
        )
        .context("failed to save profile")?;
        Ok(())
    }

    fn delete(&self, profile_id: &str) -> Result<()> {
        let conn = self.lock_db()?;
        conn.execute(
            "DELETE FROM profiles WHERE profile_id = ?1",
            rusqlite::params![profile_id],
        )
        .context("failed to delete profile")?;
        Ok(())
    }

    fn list_all(&self) -> Result<Vec<Profile>> {
        let conn = self.lock_db()?;
        let mut stmt = conn
            .prepare(
                "SELECT profile_id, name, description, is_active, created_at, updated_at
                 FROM profiles ORDER BY is_active DESC, created_at ASC, name ASC",
            )
            .context("failed to prepare list profiles query")?;

        let rows = stmt
            .query_map([], Self::row_to_profile)
            .context("failed to list profiles")?;

        let mut profiles = Vec::new();
        for row in rows {
            profiles.push(row.context("failed to read profile row")?);
        }
        Ok(profiles)
    }

    fn get_active(&self) -> Result<Option<Profile>> {
        let conn = self.lock_db()?;
        let mut stmt = conn
            .prepare(
                "SELECT profile_id, name, description, is_active, created_at, updated_at
                 FROM profiles WHERE is_active = 1 LIMIT 1",
            )
            .context("failed to prepare get active profile query")?;

        let result = stmt.query_row([], Self::row_to_profile);

        match result {
            Ok(profile) => Ok(Some(profile)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(error) => Err(error).context("failed to get active profile"),
        }
    }

    fn set_active(&self, profile_id: &str, updated_at: u128) -> Result<()> {
        let conn = self.lock_db()?;
        let tx = conn
            .unchecked_transaction()
            .context("failed to begin profile activation transaction")?;

        let exists = tx
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM profiles WHERE profile_id = ?1)",
                rusqlite::params![profile_id],
                |row| row.get::<_, i64>(0),
            )
            .context("failed to check profile existence")?
            != 0;
        if !exists {
            return Err(anyhow!("profile not found: {profile_id}"));
        }

        tx.execute("UPDATE profiles SET is_active = 0 WHERE is_active = 1", [])
            .context("failed to clear active profile")?;
        let affected = tx
            .execute(
                "UPDATE profiles SET is_active = 1, updated_at = ?2 WHERE profile_id = ?1",
                rusqlite::params![profile_id, updated_at as i64],
            )
            .context("failed to set active profile")?;
        if affected != 1 {
            return Err(anyhow!("profile not found: {profile_id}"));
        }

        tx.commit().context("failed to commit profile activation")?;
        Ok(())
    }
}

impl ProfileSaveSettingsRepository for SqliteProfileRepository {
    fn get_settings(&self, profile_id: &str) -> Result<Option<ProfileSaveSettings>> {
        let conn = self.lock_db()?;
        let mut stmt = conn
            .prepare(
                "SELECT profile_id, save_directory, backup_directory, backup_cadence,
                        backup_hour, backup_minute, backup_weekdays,
                        retention_max_count, retention_max_age_days,
                        retention_max_total_bytes, pre_restore_backup_enabled,
                        steam_account_name, steam_avatar_url, steam_account_label,
                        updated_at
                 FROM profile_save_settings WHERE profile_id = ?1",
            )
            .context("failed to prepare get profile save settings query")?;

        let result = stmt.query_row(
            rusqlite::params![profile_id],
            Self::row_to_profile_save_settings,
        );

        match result {
            Ok(settings) => Ok(Some(settings)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(error) => Err(error).context("failed to get profile save settings"),
        }
    }

    fn save_settings(&self, settings: &ProfileSaveSettings) -> Result<()> {
        let weekdays_json = serde_json::to_string(&settings.schedule.weekdays)
            .context("failed to serialize backup weekdays")?;
        let retention_max_total_bytes = settings
            .retention
            .max_total_bytes
            .map(i64::try_from)
            .transpose()
            .context("backup retention space budget exceeds SQLite integer range")?;
        let updated_at = i64::try_from(settings.updated_at)
            .context("profile save settings timestamp exceeds SQLite integer range")?;
        let conn = self.lock_db()?;
        conn.execute(
            "INSERT INTO profile_save_settings
                (profile_id, save_directory, backup_directory, backup_cadence,
                 backup_hour, backup_minute, backup_weekdays,
                 retention_max_count, retention_max_age_days,
                 retention_max_total_bytes, pre_restore_backup_enabled,
                 steam_account_name, steam_avatar_url, steam_account_label, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)
             ON CONFLICT(profile_id) DO UPDATE SET
                save_directory = excluded.save_directory,
                backup_directory = excluded.backup_directory,
                backup_cadence = excluded.backup_cadence,
                backup_hour = excluded.backup_hour,
                backup_minute = excluded.backup_minute,
                backup_weekdays = excluded.backup_weekdays,
                retention_max_count = excluded.retention_max_count,
                retention_max_age_days = excluded.retention_max_age_days,
                retention_max_total_bytes = excluded.retention_max_total_bytes,
                pre_restore_backup_enabled = excluded.pre_restore_backup_enabled,
                steam_account_name = excluded.steam_account_name,
                steam_avatar_url = excluded.steam_avatar_url,
                steam_account_label = excluded.steam_account_label,
                updated_at = excluded.updated_at",
            rusqlite::params![
                settings.profile_id,
                settings.save_directory.directory.as_deref(),
                settings.backup_directory.directory.as_deref(),
                format_backup_cadence(settings.schedule.cadence),
                settings.schedule.hour.map(i64::from),
                settings.schedule.minute.map(i64::from),
                weekdays_json,
                i64::from(settings.retention.max_count),
                settings.retention.max_age_days.map(i64::from),
                retention_max_total_bytes,
                if settings.pre_restore_backup_enabled {
                    1
                } else {
                    0
                },
                settings
                    .steam_account
                    .as_ref()
                    .and_then(|summary| summary.account_name.as_deref()),
                settings
                    .steam_account
                    .as_ref()
                    .and_then(|summary| summary.avatar_url.as_deref()),
                settings
                    .steam_account
                    .as_ref()
                    .map(|summary| summary.account_label.as_str()),
                updated_at,
            ],
        )
        .context("failed to save profile save settings")?;
        Ok(())
    }
}

fn optional_non_negative_u8(row: &rusqlite::Row<'_>, index: usize) -> rusqlite::Result<Option<u8>> {
    row.get::<_, Option<i64>>(index)?
        .map(|value| u8::try_from(value).map_err(|error| integer_conversion_error(index, error)))
        .transpose()
}

fn non_negative_u32(row: &rusqlite::Row<'_>, index: usize) -> rusqlite::Result<u32> {
    let value = row.get::<_, i64>(index)?;
    u32::try_from(value).map_err(|error| integer_conversion_error(index, error))
}

fn optional_non_negative_u32(
    row: &rusqlite::Row<'_>,
    index: usize,
) -> rusqlite::Result<Option<u32>> {
    row.get::<_, Option<i64>>(index)?
        .map(|value| u32::try_from(value).map_err(|error| integer_conversion_error(index, error)))
        .transpose()
}

fn optional_non_negative_u64(
    row: &rusqlite::Row<'_>,
    index: usize,
) -> rusqlite::Result<Option<u64>> {
    row.get::<_, Option<i64>>(index)?
        .map(|value| u64::try_from(value).map_err(|error| integer_conversion_error(index, error)))
        .transpose()
}

fn non_negative_u128(row: &rusqlite::Row<'_>, index: usize) -> rusqlite::Result<u128> {
    let value = row.get::<_, i64>(index)?;
    u128::try_from(value).map_err(|error| integer_conversion_error(index, error))
}

fn integer_conversion_error(index: usize, error: std::num::TryFromIntError) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(
        index,
        rusqlite::types::Type::Integer,
        Box::new(error),
    )
}

impl ProfileSaveDirectoryValidator for SqliteProfileRepository {
    fn validate_save_directory(
        &self,
        _game_id: &str,
        directory: &str,
    ) -> Result<ProfileDirectorySelection> {
        validate_custom_directory(directory)
    }

    fn validate_backup_directory(
        &self,
        _game_id: &str,
        directory: &str,
    ) -> Result<ProfileDirectorySelection> {
        validate_custom_directory(directory)
    }

    fn default_backup_directory(&self, _game_id: &str) -> Result<ProfileDirectorySelection> {
        Ok(default_backup_directory_selection())
    }
}

fn validate_custom_directory(directory: &str) -> Result<ProfileDirectorySelection> {
    let directory = directory.trim();
    if directory.is_empty() {
        return Err(anyhow!("directory must not be empty"));
    }
    if !Path::new(directory).is_absolute() {
        return Err(anyhow!("directory must be absolute"));
    }

    Ok(custom_directory_selection(directory))
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

fn unset_save_directory() -> ProfileDirectorySelection {
    ProfileDirectorySelection {
        mode: ProfileDirectoryMode::Unset,
        status: ProfileDirectoryStatus::Unset,
        directory: None,
        path_label: None,
        messages: vec!["尚未选择游戏存档目录".to_owned()],
    }
}

fn default_backup_directory_selection() -> ProfileDirectorySelection {
    ProfileDirectorySelection {
        mode: ProfileDirectoryMode::Default,
        status: ProfileDirectoryStatus::Defaulted,
        directory: None,
        path_label: Some("HelsincyModManager/Backups".to_owned()),
        messages: vec!["使用默认备份目录".to_owned()],
    }
}

fn path_label(path: &str) -> String {
    path.replace('\\', "/")
        .rsplit('/')
        .next()
        .unwrap_or(path)
        .to_owned()
}

fn parse_backup_cadence(value: &str) -> BackupCadence {
    match value {
        "daily" => BackupCadence::Daily,
        "weekly" => BackupCadence::Weekly,
        _ => BackupCadence::Manual,
    }
}

fn format_backup_cadence(value: BackupCadence) -> &'static str {
    match value {
        BackupCadence::Manual => "manual",
        BackupCadence::Daily => "daily",
        BackupCadence::Weekly => "weekly",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sqlite::open_database;

    fn test_repo() -> SqliteProfileRepository {
        let temp = tempfile::tempdir().expect("temp dir");
        let db_path = temp.path().join("test.db");
        let conn = open_database(&db_path).unwrap();
        std::mem::forget(temp);
        SqliteProfileRepository::new(Arc::new(Mutex::new(conn)))
    }

    fn sample_profile(id: &str, name: &str, is_active: bool) -> Profile {
        Profile {
            id: id.to_owned(),
            name: name.to_owned(),
            description: None,
            is_active,
            created_at: 1000,
            updated_at: 1000,
        }
    }

    #[test]
    fn default_profile_exists() {
        let repo = test_repo();
        let active = repo.get_active().unwrap().expect("active profile");

        assert_eq!(active.id, "default");
        assert_eq!(active.name, "Default");
        assert!(active.is_active);
    }

    #[test]
    fn save_and_get_round_trips() {
        let repo = test_repo();
        let mut profile = sample_profile("profile-1", "Hunt", false);
        profile.description = Some("Testing".to_owned());

        repo.save(&profile).unwrap();
        let loaded = repo.get("profile-1").unwrap().expect("profile exists");

        assert_eq!(loaded.name, "Hunt");
        assert_eq!(loaded.description.as_deref(), Some("Testing"));
        assert!(!loaded.is_active);
    }

    #[test]
    fn set_active_deactivates_existing_active_profile() {
        let repo = test_repo();
        repo.save(&sample_profile("profile-2", "Second", false))
            .unwrap();

        repo.set_active("profile-2", 5000).unwrap();

        assert!(!repo.get("default").unwrap().unwrap().is_active);
        let active = repo.get_active().unwrap().expect("active profile");
        assert_eq!(active.id, "profile-2");
        assert_eq!(active.updated_at, 5000);
    }

    #[test]
    fn set_active_unknown_profile_keeps_existing_active_profile() {
        let repo = test_repo();

        let result = repo.set_active("missing", 5000);

        assert!(result.is_err());
        let active = repo.get_active().unwrap().expect("active profile");
        assert_eq!(active.id, "default");
        assert!(active.is_active);
    }

    #[test]
    fn list_all_orders_active_profile_first() {
        let repo = test_repo();
        repo.save(&sample_profile("profile-2", "Second", false))
            .unwrap();
        repo.set_active("profile-2", 5000).unwrap();

        let profiles = repo.list_all().unwrap();

        assert_eq!(profiles[0].id, "profile-2");
    }

    #[test]
    fn save_settings_round_trips_retention_account_and_pre_restore_preference() {
        let repo = test_repo();
        let settings = ProfileSaveSettings {
            profile_id: "default".to_owned(),
            save_directory: custom_directory_selection("C:/Fixture/Saves"),
            backup_directory: custom_directory_selection("D:/Fixture/Backups"),
            schedule: ProfileBackupSchedule::manual(),
            retention: ProfileBackupRetention {
                max_count: 12,
                max_age_days: Some(45),
                max_total_bytes: Some(64 * 1024 * 1024),
            },
            steam_account: Some(SteamAccountDisplaySummary {
                account_name: Some("Synthetic Hunter".to_owned()),
                avatar_url: Some("https://avatars.steamstatic.com/fixture.jpg".to_owned()),
                account_label: "Steam 12****34".to_owned(),
            }),
            pre_restore_backup_enabled: false,
            updated_at: 42,
        };

        repo.save_settings(&settings).expect("save settings");
        let loaded = repo
            .get_settings("default")
            .expect("read settings")
            .expect("settings exist");

        assert_eq!(loaded, settings);
    }

    #[test]
    fn save_settings_rejects_negative_retention_numbers_in_corrupt_storage() {
        let repo = test_repo();
        let settings = ProfileSaveSettings {
            profile_id: "default".to_owned(),
            save_directory: custom_directory_selection("C:/Fixture/Saves"),
            backup_directory: custom_directory_selection("D:/Fixture/Backups"),
            schedule: ProfileBackupSchedule::manual(),
            retention: ProfileBackupRetention::default(),
            steam_account: None,
            pre_restore_backup_enabled: true,
            updated_at: 42,
        };
        repo.save_settings(&settings).expect("save settings");

        repo.db
            .lock()
            .expect("db lock")
            .execute_batch(
                "PRAGMA ignore_check_constraints = ON;
                 UPDATE profile_save_settings
                 SET retention_max_total_bytes = -1
                 WHERE profile_id = 'default';
                 PRAGMA ignore_check_constraints = OFF;",
            )
            .expect("seed corrupt retention budget");

        assert!(repo.get_settings("default").is_err());
    }
}
