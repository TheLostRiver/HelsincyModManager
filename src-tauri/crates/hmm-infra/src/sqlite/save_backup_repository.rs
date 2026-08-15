use anyhow::{anyhow, Context, Result};
use hmm_core::{
    GameId, ProfileDirectoryMode, ProfileDirectorySelection, ProfileDirectoryStatus, ProfileId,
    SaveBackupRetentionReason, SaveBackupStatus, SaveBackupSummary, SaveBackupTrigger,
};
use hmm_ports::{
    SaveBackupCenterRepositoryFacts, SaveBackupCenterRepositoryItem,
    SaveBackupCenterRepositoryPage, SaveBackupCenterRepositoryProfileFacts,
    SaveBackupCenterRepositoryQuery, SaveBackupRepository,
};
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
            archive_size_bytes: non_negative_u64(row, 7)?,
            retention_released_bytes: non_negative_u64(row, 16)?,
            archive_sha256: row.get(8)?,
            file_count: non_negative_u32(row, 9)?,
            created_at: non_negative_u128(row, 10)?,
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
        let archive_size_bytes = i64::try_from(summary.archive_size_bytes)
            .context("save backup archive size exceeds SQLite integer range")?;
        let created_at = i64::try_from(summary.created_at)
            .context("save backup timestamp exceeds SQLite integer range")?;
        let retention_released_bytes = i64::try_from(summary.retention_released_bytes)
            .context("save backup released bytes exceed SQLite integer range")?;
        let conn = self.lock_db()?;
        conn.execute(
            "INSERT INTO save_backups
                (backup_id, game_id, profile_id, trigger, status, archive_file_name,
                 manifest_file_name, archive_size_bytes, archive_sha256, file_count,
                 created_at, source_path_label, source_path_hash, notes,
                 backup_directory_mode, backup_directory, retention_released_bytes)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)
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
                backup_directory = excluded.backup_directory,
                retention_released_bytes = excluded.retention_released_bytes",
            rusqlite::params![
                summary.backup_id,
                summary.game_id.as_str(),
                summary.profile_id.as_str(),
                summary.trigger.as_str(),
                summary.status.as_str(),
                summary.archive_file_name,
                summary.manifest_file_name,
                archive_size_bytes,
                summary.archive_sha256,
                i64::from(summary.file_count),
                created_at,
                summary.source_path_label,
                summary.source_path_hash,
                summary.notes,
                format_directory_mode(summary.backup_directory.mode),
                summary.backup_directory.directory.as_deref(),
                retention_released_bytes,
            ],
        )
        .context("failed to save save backup summary")?;
        Ok(())
    }

    fn get_for_restore(
        &self,
        game_id: &GameId,
        profile_id: &ProfileId,
        backup_id: &str,
    ) -> Result<Option<SaveBackupSummary>> {
        let conn = self.lock_db()?;
        let result = conn.query_row(
            "SELECT backup_id, game_id, profile_id, trigger, status, archive_file_name,
                    manifest_file_name, archive_size_bytes, archive_sha256, file_count,
                    created_at, source_path_label, source_path_hash, notes,
                    backup_directory_mode, backup_directory, retention_released_bytes
             FROM save_backups
             WHERE game_id = ?1 AND profile_id = ?2 AND backup_id = ?3",
            rusqlite::params![game_id.as_str(), profile_id.as_str(), backup_id],
            Self::row_to_summary,
        );

        match result {
            Ok(summary) => Ok(Some(summary)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(error) => Err(error).context("failed to get save backup for restore"),
        }
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
                        backup_directory_mode, backup_directory, retention_released_bytes
                 FROM save_backups
                 WHERE game_id = ?1 AND profile_id = ?2
                 ORDER BY created_at DESC, backup_id DESC
                 LIMIT ?3"
            }
            None => {
                "SELECT backup_id, game_id, profile_id, trigger, status, archive_file_name,
                        manifest_file_name, archive_size_bytes, archive_sha256, file_count,
                        created_at, source_path_label, source_path_hash, notes,
                        backup_directory_mode, backup_directory, retention_released_bytes
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

    fn list_for_game(&self, game_id: &GameId) -> Result<Vec<SaveBackupSummary>> {
        let conn = self.lock_db()?;
        let mut stmt = conn
            .prepare(
                "SELECT backup_id, game_id, profile_id, trigger, status, archive_file_name,
                        manifest_file_name, archive_size_bytes, archive_sha256, file_count,
                        created_at, source_path_label, source_path_hash, notes,
                        backup_directory_mode, backup_directory, retention_released_bytes
                 FROM save_backups
                 WHERE game_id = ?1
                 ORDER BY created_at DESC, backup_id DESC",
            )
            .context("failed to prepare list save backups for game query")?;
        let rows = stmt
            .query_map(rusqlite::params![game_id.as_str()], Self::row_to_summary)
            .context("failed to list save backups for game")?;
        let mut backups = Vec::new();
        for row in rows {
            backups.push(row.context("failed to read save backup center row")?);
        }
        Ok(backups)
    }

    fn query_for_center(
        &self,
        request: &SaveBackupCenterRepositoryQuery,
    ) -> Result<Option<SaveBackupCenterRepositoryPage>> {
        let limit = i64::try_from(request.limit)
            .context("save backup center limit is outside the supported range")?;
        let offset = i64::try_from(request.offset)
            .context("save backup center offset is outside the supported range")?;
        let conn = self.lock_db()?;
        let profile_id = request.profile_id.as_ref().map(ProfileId::as_str);
        let trigger = request.trigger.map(SaveBackupTrigger::as_str);
        let status = request.status.map(SaveBackupStatus::as_str);
        let search = request.search.as_ref().map(|value| {
            format!(
                "%{}%",
                value
                    .replace('\\', "\\\\")
                    .replace('%', "\\%")
                    .replace('_', "\\_")
            )
        });
        let params = rusqlite::params![
            request.game_id.as_str(),
            profile_id,
            trigger,
            status,
            search.as_deref(),
        ];
        let filter = "b.game_id = ?1
            AND (?2 IS NULL OR b.profile_id = ?2)
            AND (?3 IS NULL OR b.trigger = ?3)
            AND (?4 IS NULL OR b.status = ?4)
            AND (?5 IS NULL OR lower(COALESCE(b.notes, '')) LIKE lower(?5) ESCAPE '\\'
                 OR lower(COALESCE(p.name, '')) LIKE lower(?5) ESCAPE '\\')";

        let total_count: i64 = conn
            .query_row(
                &format!("SELECT COUNT(*) FROM save_backups b LEFT JOIN profiles p ON p.profile_id = b.profile_id WHERE {filter}"),
                params,
                |row| row.get(0),
            )
            .context("failed to count save backup center rows")?;
        let summary = conn
            .query_row(
                &format!(
                    "SELECT COUNT(*),
                            COALESCE(SUM(CASE WHEN b.status <> 'deleted_by_retention'
                                THEN CASE WHEN b.archive_size_bytes > b.retention_released_bytes
                                    THEN b.archive_size_bytes - b.retention_released_bytes ELSE 0 END
                                ELSE 0 END), 0),
                            COALESCE(SUM(CASE WHEN b.trigger = 'pre_restore'
                                AND b.status <> 'deleted_by_retention' THEN 1 ELSE 0 END), 0),
                            COALESCE(SUM(CASE WHEN b.status IN
                                ('retention_pending', 'retention_partial', 'missing', 'invalid')
                                THEN 1 ELSE 0 END), 0)
                     FROM save_backups b LEFT JOIN profiles p ON p.profile_id = b.profile_id
                     WHERE {filter}"
                ),
                params,
                |row| {
                    Ok(SaveBackupCenterRepositoryFacts {
                        backup_count: non_negative_u32(row, 0)?,
                        archive_bytes: non_negative_u64(row, 1)?,
                        protected_count: non_negative_u32(row, 2)?,
                        attention_count: non_negative_u32(row, 3)?,
                    })
                },
            )
            .context("failed to summarize save backup center rows")?;
        let mut item_statement = conn
            .prepare(&format!(
                "SELECT b.backup_id, b.game_id, b.profile_id, b.trigger, b.status,
                        b.archive_file_name, b.manifest_file_name, b.archive_size_bytes,
                        b.archive_sha256, b.file_count, b.created_at, b.source_path_label,
                        b.source_path_hash, b.notes, b.backup_directory_mode, b.backup_directory,
                        b.retention_released_bytes, p.name
                 FROM save_backups b LEFT JOIN profiles p ON p.profile_id = b.profile_id
                 WHERE {filter}
                 ORDER BY b.created_at DESC, b.backup_id DESC
                 LIMIT ?6 OFFSET ?7"
            ))
            .context("failed to prepare save backup center page")?;
        let page_params = rusqlite::params![
            request.game_id.as_str(),
            profile_id,
            trigger,
            status,
            search.as_deref(),
            limit,
            offset,
        ];
        let rows = item_statement
            .query_map(page_params, |row| {
                let backup = Self::row_to_summary(row)?;
                let profile_name = row
                    .get::<_, Option<String>>(17)?
                    .unwrap_or_else(|| backup.profile_id.as_str().to_owned());
                Ok(SaveBackupCenterRepositoryItem {
                    profile_name,
                    backup,
                })
            })
            .context("failed to query save backup center page")?;
        let mut items = Vec::new();
        for row in rows {
            items.push(row.context("failed to read save backup center page row")?);
        }

        let mut profile_statement = conn
            .prepare(
                "SELECT profile_id, COUNT(*),
                        COALESCE(SUM(CASE WHEN status <> 'deleted_by_retention'
                            THEN CASE WHEN archive_size_bytes > retention_released_bytes
                                THEN archive_size_bytes - retention_released_bytes ELSE 0 END
                            ELSE 0 END), 0),
                        COALESCE(SUM(CASE WHEN trigger = 'pre_restore'
                            AND status <> 'deleted_by_retention' THEN 1 ELSE 0 END), 0),
                        COALESCE(SUM(CASE WHEN status IN
                            ('retention_pending', 'retention_partial', 'missing', 'invalid')
                            THEN 1 ELSE 0 END), 0)
                 FROM save_backups
                 WHERE game_id = ?1
                 GROUP BY profile_id",
            )
            .context("failed to prepare save backup center profile facts")?;
        let rows = profile_statement
            .query_map(rusqlite::params![request.game_id.as_str()], |row| {
                Ok(SaveBackupCenterRepositoryProfileFacts {
                    profile_id: ProfileId::new(row.get::<_, String>(0)?),
                    facts: SaveBackupCenterRepositoryFacts {
                        backup_count: non_negative_u32(row, 1)?,
                        archive_bytes: non_negative_u64(row, 2)?,
                        protected_count: non_negative_u32(row, 3)?,
                        attention_count: non_negative_u32(row, 4)?,
                    },
                })
            })
            .context("failed to query save backup center profile facts")?;
        let mut profiles = Vec::new();
        for row in rows {
            profiles.push(row.context("failed to read save backup center profile facts")?);
        }

        Ok(Some(SaveBackupCenterRepositoryPage {
            total_count: usize::try_from(total_count)
                .context("save backup center count is outside the supported range")?,
            summary,
            profiles,
            items,
        }))
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

    fn begin_retention(
        &self,
        game_id: &GameId,
        profile_id: &ProfileId,
        backup_id: &str,
        reasons: &[SaveBackupRetentionReason],
        attempted_at: u128,
    ) -> Result<bool> {
        let attempted_at = i64::try_from(attempted_at)
            .context("save backup retention timestamp exceeds SQLite integer range")?;
        let conn = self.lock_db()?;
        let reasons = serde_json::to_string(
            &reasons
                .iter()
                .map(|reason| reason.as_str())
                .collect::<Vec<_>>(),
        )
        .context("failed to serialize save backup retention reasons")?;
        let affected = conn
            .execute(
                "UPDATE save_backups
                 SET status = 'retention_pending', retention_reasons = ?4,
                     retention_attempted_at = ?5, retention_error_code = NULL
                 WHERE game_id = ?1 AND profile_id = ?2 AND backup_id = ?3
                   AND status IN ('completed', 'retention_pending', 'retention_partial')",
                rusqlite::params![
                    game_id.as_str(),
                    profile_id.as_str(),
                    backup_id,
                    reasons,
                    attempted_at,
                ],
            )
            .context("failed to begin save backup retention")?;
        Ok(affected == 1)
    }

    fn finish_retention(
        &self,
        game_id: &GameId,
        profile_id: &ProfileId,
        backup_id: &str,
        status: SaveBackupStatus,
        error_code: Option<&str>,
        released_bytes: u64,
    ) -> Result<()> {
        if !matches!(
            status,
            SaveBackupStatus::DeletedByRetention | SaveBackupStatus::RetentionPartial
        ) {
            return Err(anyhow!("invalid save backup retention final status"));
        }
        let released_bytes = i64::try_from(released_bytes)
            .context("save backup retention released bytes exceed SQLite integer range")?;
        let conn = self.lock_db()?;
        let affected = conn
            .execute(
                "UPDATE save_backups
                 SET status = ?4, retention_error_code = ?5,
                     retention_released_bytes = retention_released_bytes + ?6
                 WHERE game_id = ?1 AND profile_id = ?2 AND backup_id = ?3
                   AND status = 'retention_pending'",
                rusqlite::params![
                    game_id.as_str(),
                    profile_id.as_str(),
                    backup_id,
                    status.as_str(),
                    error_code,
                    released_bytes,
                ],
            )
            .context("failed to finish save backup retention")?;
        if affected != 1 {
            return Err(anyhow!("save backup retention intent is stale"));
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
        let conn = self.lock_db()?;
        let affected = conn
            .execute(
                "UPDATE save_backups SET notes = ?4
                 WHERE game_id = ?1 AND profile_id = ?2 AND backup_id = ?3",
                rusqlite::params![game_id.as_str(), profile_id.as_str(), backup_id, note],
            )
            .context("failed to update save backup note")?;
        Ok(affected == 1)
    }
}

fn non_negative_u32(row: &rusqlite::Row<'_>, index: usize) -> rusqlite::Result<u32> {
    let value = row.get::<_, i64>(index)?;
    u32::try_from(value).map_err(|error| integer_conversion_error(index, error))
}

fn non_negative_u64(row: &rusqlite::Row<'_>, index: usize) -> rusqlite::Result<u64> {
    let value = row.get::<_, i64>(index)?;
    u64::try_from(value).map_err(|error| integer_conversion_error(index, error))
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

fn parse_trigger(value: &str) -> std::result::Result<SaveBackupTrigger, String> {
    match value {
        "manual" => Ok(SaveBackupTrigger::Manual),
        "auto" => Ok(SaveBackupTrigger::Auto),
        "pre_install" => Ok(SaveBackupTrigger::PreInstall),
        "pre_restore" => Ok(SaveBackupTrigger::PreRestore),
        other => Err(format!("unknown save backup trigger: {other}")),
    }
}

fn parse_status(value: &str) -> std::result::Result<SaveBackupStatus, String> {
    match value {
        "completed" => Ok(SaveBackupStatus::Completed),
        "retention_pending" => Ok(SaveBackupStatus::RetentionPending),
        "retention_partial" => Ok(SaveBackupStatus::RetentionPartial),
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
