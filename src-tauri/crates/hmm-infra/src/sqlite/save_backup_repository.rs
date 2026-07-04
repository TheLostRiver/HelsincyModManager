use anyhow::{anyhow, Context, Result};
use hmm_core::{
    GameId, ProfileDirectoryMode, ProfileDirectorySelection, ProfileDirectoryStatus, ProfileId,
    SaveBackupStatus, SaveBackupSummary, SaveBackupTrigger,
};
use hmm_ports::SaveBackupRepository;
use rusqlite::Connection;
use std::sync::{Arc, Mutex};

pub struct SqliteSaveBackupRepository {
    db: Arc<Mutex<Connection>>,
}

impl SqliteSaveBackupRepository {
    pub fn new(db: Arc<Mutex<Connection>>) -> Self {
        Self { db }
    }

    fn lock_db(&self) -> Result<std::sync::MutexGuard<'_, Connection>> {
        self.db
            .lock()
            .map_err(|_| anyhow!("database lock poisoned"))
    }

    fn row_to_summary(row: &rusqlite::Row<'_>) -> rusqlite::Result<SaveBackupSummary> {
        let game_id: String = row.get(1)?;
        let trigger: String = row.get(3)?;
        let status: String = row.get(4)?;
        let backup_directory_mode: String = row.get(14)?;
        let backup_directory: Option<String> = row.get(15)?;

        Ok(SaveBackupSummary {
            backup_id: row.get(0)?,
            game_id: GameId::parse(game_id).map_err(|error| {
                rusqlite::Error::FromSqlConversionFailure(
                    1,
                    rusqlite::types::Type::Text,
                    Box::new(error),
                )
            })?,
            profile_id: ProfileId::new(row.get::<_, String>(2)?),
            trigger: parse_trigger(&trigger).map_err(|error| {
                rusqlite::Error::FromSqlConversionFailure(
                    3,
                    rusqlite::types::Type::Text,
                    Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, error)),
                )
            })?,
            status: parse_status(&status).map_err(|error| {
                rusqlite::Error::FromSqlConversionFailure(
                    4,
                    rusqlite::types::Type::Text,
                    Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, error)),
                )
            })?,
            archive_file_name: row.get(5)?,
            manifest_file_name: row.get(6)?,
            archive_size_bytes: row.get::<_, i64>(7)? as u64,
            archive_sha256: row.get(8)?,
            file_count: row.get::<_, i64>(9)? as u32,
            created_at: row.get::<_, i64>(10)? as u128,
            source_path_label: row.get(11)?,
            source_path_hash: row.get(12)?,
            notes: row.get(13)?,
            backup_directory: backup_directory_selection_from_row(
                &backup_directory_mode,
                backup_directory,
            )
            .map_err(|error| {
                rusqlite::Error::FromSqlConversionFailure(
                    14,
                    rusqlite::types::Type::Text,
                    Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, error)),
                )
            })?,
        })
    }
}

impl SaveBackupRepository for SqliteSaveBackupRepository {
    fn save(&self, summary: &SaveBackupSummary) -> Result<()> {
        let conn = self.lock_db()?;
        conn.execute(
            "INSERT INTO save_backups
                (backup_id, game_id, profile_id, trigger, status, archive_file_name,
                 manifest_file_name, archive_size_bytes, archive_sha256, file_count,
                 created_at, source_path_label, source_path_hash, notes,
                 backup_directory_mode, backup_directory)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)
             ON CONFLICT(backup_id) DO UPDATE SET
                game_id = excluded.game_id,
                profile_id = excluded.profile_id,
                trigger = excluded.trigger,
                status = excluded.status,
                archive_file_name = excluded.archive_file_name,
                manifest_file_name = excluded.manifest_file_name,
                archive_size_bytes = excluded.archive_size_bytes,
                archive_sha256 = excluded.archive_sha256,
                file_count = excluded.file_count,
                created_at = excluded.created_at,
                source_path_label = excluded.source_path_label,
                source_path_hash = excluded.source_path_hash,
                notes = excluded.notes,
                backup_directory_mode = excluded.backup_directory_mode,
                backup_directory = excluded.backup_directory",
            rusqlite::params![
                summary.backup_id,
                summary.game_id.as_str(),
                summary.profile_id.as_str(),
                summary.trigger.as_str(),
                summary.status.as_str(),
                summary.archive_file_name,
                summary.manifest_file_name,
                summary.archive_size_bytes as i64,
                summary.archive_sha256,
                i64::from(summary.file_count),
                summary.created_at as i64,
                summary.source_path_label,
                summary.source_path_hash,
                summary.notes,
                format_directory_mode(summary.backup_directory.mode),
                summary.backup_directory.directory.as_deref(),
            ],
        )
        .context("failed to save save backup summary")?;
        Ok(())
    }

    fn list_for_profile(
        &self,
        game_id: &GameId,
        profile_id: &ProfileId,
        limit: Option<usize>,
    ) -> Result<Vec<SaveBackupSummary>> {
        let conn = self.lock_db()?;
        let sql = match limit {
            Some(_) => {
                "SELECT backup_id, game_id, profile_id, trigger, status, archive_file_name,
                        manifest_file_name, archive_size_bytes, archive_sha256, file_count,
                        created_at, source_path_label, source_path_hash, notes,
                        backup_directory_mode, backup_directory
                 FROM save_backups
                 WHERE game_id = ?1 AND profile_id = ?2
                 ORDER BY created_at DESC, backup_id DESC
                 LIMIT ?3"
            }
            None => {
                "SELECT backup_id, game_id, profile_id, trigger, status, archive_file_name,
                        manifest_file_name, archive_size_bytes, archive_sha256, file_count,
                        created_at, source_path_label, source_path_hash, notes,
                        backup_directory_mode, backup_directory
                 FROM save_backups
                 WHERE game_id = ?1 AND profile_id = ?2
                 ORDER BY created_at DESC, backup_id DESC"
            }
        };

        let mut stmt = conn
            .prepare(sql)
            .context("failed to prepare list save backups query")?;
        let mut backups = Vec::new();

        match limit {
            Some(limit) => {
                let rows = stmt
                    .query_map(
                        rusqlite::params![game_id.as_str(), profile_id.as_str(), limit as i64],
                        Self::row_to_summary,
                    )
                    .context("failed to list save backups")?;
                for row in rows {
                    backups.push(row.context("failed to read save backup row")?);
                }
            }
            None => {
                let rows = stmt
                    .query_map(
                        rusqlite::params![game_id.as_str(), profile_id.as_str()],
                        Self::row_to_summary,
                    )
                    .context("failed to list save backups")?;
                for row in rows {
                    backups.push(row.context("failed to read save backup row")?);
                }
            }
        }

        Ok(backups)
    }

    fn mark_status(&self, backup_id: &str, status: SaveBackupStatus) -> Result<()> {
        let conn = self.lock_db()?;
        conn.execute(
            "UPDATE save_backups SET status = ?2 WHERE backup_id = ?1",
            rusqlite::params![backup_id, status.as_str()],
        )
        .context("failed to update save backup status")?;
        Ok(())
    }
}

fn parse_trigger(value: &str) -> std::result::Result<SaveBackupTrigger, String> {
    match value {
        "manual" => Ok(SaveBackupTrigger::Manual),
        "auto" => Ok(SaveBackupTrigger::Auto),
        "pre_install" => Ok(SaveBackupTrigger::PreInstall),
        other => Err(format!("unknown save backup trigger: {other}")),
    }
}

fn parse_status(value: &str) -> std::result::Result<SaveBackupStatus, String> {
    match value {
        "completed" => Ok(SaveBackupStatus::Completed),
        "deleted_by_retention" => Ok(SaveBackupStatus::DeletedByRetention),
        "missing" => Ok(SaveBackupStatus::Missing),
        "invalid" => Ok(SaveBackupStatus::Invalid),
        other => Err(format!("unknown save backup status: {other}")),
    }
}

fn format_directory_mode(value: ProfileDirectoryMode) -> &'static str {
    match value {
        ProfileDirectoryMode::Unset => "unset",
        ProfileDirectoryMode::Custom => "custom",
        ProfileDirectoryMode::Default => "default",
    }
}

fn parse_directory_mode(value: &str) -> std::result::Result<ProfileDirectoryMode, String> {
    match value {
        "unset" => Ok(ProfileDirectoryMode::Unset),
        "custom" => Ok(ProfileDirectoryMode::Custom),
        "default" => Ok(ProfileDirectoryMode::Default),
        other => Err(format!("unknown backup directory mode: {other}")),
    }
}

fn backup_directory_selection_from_row(
    mode: &str,
    directory: Option<String>,
) -> std::result::Result<ProfileDirectorySelection, String> {
    let mode = parse_directory_mode(mode)?;
    let status = match mode {
        ProfileDirectoryMode::Unset => ProfileDirectoryStatus::Unset,
        ProfileDirectoryMode::Default => ProfileDirectoryStatus::Defaulted,
        ProfileDirectoryMode::Custom if directory.is_some() => ProfileDirectoryStatus::Valid,
        ProfileDirectoryMode::Custom => ProfileDirectoryStatus::Invalid,
    };
    let path_label = directory.as_deref().map(path_label);

    Ok(ProfileDirectorySelection {
        mode,
        status,
        directory,
        path_label,
        messages: Vec::new(),
    })
}

fn path_label(path: &str) -> String {
    path.replace('\\', "/")
        .rsplit('/')
        .next()
        .unwrap_or(path)
        .to_owned()
}
