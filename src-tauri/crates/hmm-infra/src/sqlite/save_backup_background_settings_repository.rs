use anyhow::{anyhow, Context, Result};
use hmm_core::SaveBackupBackgroundSettings;
use hmm_ports::SaveBackupBackgroundSettingsRepository;
use rusqlite::{params, Connection, OptionalExtension};
use std::sync::{Arc, Mutex};

pub struct SqliteSaveBackupBackgroundSettingsRepository {
    db: Arc<Mutex<Connection>>,
}

impl SqliteSaveBackupBackgroundSettingsRepository {
    pub fn new(db: Arc<Mutex<Connection>>) -> Self {
        Self { db }
    }

    fn lock_db(&self) -> Result<std::sync::MutexGuard<'_, Connection>> {
        self.db
            .lock()
            .map_err(|_| anyhow!("database lock poisoned"))
    }
}

impl SaveBackupBackgroundSettingsRepository for SqliteSaveBackupBackgroundSettingsRepository {
    fn load(&self) -> Result<SaveBackupBackgroundSettings> {
        let conn = self.lock_db()?;
        conn.query_row(
            "SELECT desired_enabled, enabled_at, last_worker_heartbeat_at, updated_at
             FROM save_backup_background_settings
             WHERE singleton_id = 1",
            [],
            |row| {
                Ok(SaveBackupBackgroundSettings {
                    desired_enabled: row.get::<_, i64>(0)? != 0,
                    enabled_at: optional_i64_to_u128(row.get(1)?),
                    last_worker_heartbeat_at: optional_i64_to_u128(row.get(2)?),
                    updated_at: i64_to_u128(row.get(3)?),
                })
            },
        )
        .optional()
        .context("failed to load save backup background settings")
        .map(|settings| settings.unwrap_or_else(SaveBackupBackgroundSettings::disabled))
    }

    fn begin_enable(&self, enabled_at: u128) -> Result<()> {
        let conn = self.lock_db()?;
        conn.execute(
            "INSERT INTO save_backup_background_settings (
                singleton_id, desired_enabled, enabled_at,
                last_worker_heartbeat_at, updated_at
             ) VALUES (1, 1, ?1, NULL, ?1)
             ON CONFLICT(singleton_id) DO UPDATE SET
                desired_enabled = 1,
                enabled_at = excluded.enabled_at,
                last_worker_heartbeat_at = NULL,
                updated_at = excluded.updated_at",
            params![to_i64(enabled_at)],
        )
        .context("failed to begin enabling save backup background protection")?;
        Ok(())
    }

    fn finish_disable(&self, updated_at: u128) -> Result<()> {
        let conn = self.lock_db()?;
        conn.execute(
            "INSERT INTO save_backup_background_settings (
                singleton_id, desired_enabled, enabled_at,
                last_worker_heartbeat_at, updated_at
             ) VALUES (1, 0, NULL, NULL, ?1)
             ON CONFLICT(singleton_id) DO UPDATE SET
                desired_enabled = 0,
                enabled_at = NULL,
                last_worker_heartbeat_at = NULL,
                updated_at = excluded.updated_at",
            params![to_i64(updated_at)],
        )
        .context("failed to finish disabling save backup background protection")?;
        Ok(())
    }

    fn record_worker_heartbeat(&self, heartbeat_at: u128) -> Result<()> {
        let conn = self.lock_db()?;
        let updated = conn
            .execute(
                "UPDATE save_backup_background_settings
                 SET last_worker_heartbeat_at = ?1, updated_at = ?1
                 WHERE singleton_id = 1 AND desired_enabled = 1",
                params![to_i64(heartbeat_at)],
            )
            .context("failed to record save backup background worker heartbeat")?;
        if updated != 1 {
            return Err(anyhow!(
                "background protection must be enabled before recording worker heartbeat"
            ));
        }
        Ok(())
    }
}

fn optional_i64_to_u128(value: Option<i64>) -> Option<u128> {
    value.map(i64_to_u128)
}

fn i64_to_u128(value: i64) -> u128 {
    value.max(0) as u128
}

fn to_i64(value: u128) -> i64 {
    value.min(i64::MAX as u128) as i64
}
