use anyhow::{anyhow, Context, Result};
use hmm_core::{
    GameId, ProfileId, SaveBackupBackgroundProtectionStatus,
    SaveBackupSchedulerLeaseRenewalRequest, SaveBackupSchedulerLeaseRequest,
    SaveBackupSchedulerPendingReason, SaveBackupSchedulerState, SaveBackupWorkerHeartbeat,
};
use hmm_ports::SaveBackupSchedulerStateRepository;
use rusqlite::{params, Connection, OptionalExtension};
use std::sync::{Arc, Mutex};

pub struct SqliteSaveBackupSchedulerStateRepository {
    db: Arc<Mutex<Connection>>,
}

impl SqliteSaveBackupSchedulerStateRepository {
    pub fn new(db: Arc<Mutex<Connection>>) -> Self {
        Self { db }
    }

    fn lock_db(&self) -> Result<std::sync::MutexGuard<'_, Connection>> {
        self.db
            .lock()
            .map_err(|_| anyhow!("database lock poisoned"))
    }
}

impl SaveBackupSchedulerStateRepository for SqliteSaveBackupSchedulerStateRepository {
    fn get_state(
        &self,
        game_id: &GameId,
        profile_id: &ProfileId,
    ) -> Result<Option<SaveBackupSchedulerState>> {
        let conn = self.lock_db()?;
        select_state(&conn, game_id, profile_id)
    }

    fn upsert_state(&self, state: &SaveBackupSchedulerState) -> Result<()> {
        let conn = self.lock_db()?;
        conn.execute(
            "INSERT INTO save_backup_scheduler_state
                (game_id, profile_id, enabled, background_protection_enabled, background_status,
                 last_checked_at, last_attempt_at, last_success_at, next_due_at,
                 pending_reason, last_error_code, worker_instance_id, worker_heartbeat_at,
                 lease_owner, lease_expires_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)
             ON CONFLICT(game_id, profile_id) DO UPDATE SET
                -- Active lease fields are intentionally omitted; only lease lifecycle methods mutate them.
                enabled = excluded.enabled,
                background_protection_enabled = excluded.background_protection_enabled,
                background_status = excluded.background_status,
                last_checked_at = excluded.last_checked_at,
                last_attempt_at = excluded.last_attempt_at,
                last_success_at = excluded.last_success_at,
                next_due_at = excluded.next_due_at,
                pending_reason = excluded.pending_reason,
                last_error_code = excluded.last_error_code,
                worker_instance_id = excluded.worker_instance_id,
                worker_heartbeat_at = excluded.worker_heartbeat_at,
                updated_at = excluded.updated_at",
            params![
                state.game_id.as_str(),
                state.profile_id.as_str(),
                bool_to_i64(state.enabled),
                bool_to_i64(state.background_protection_enabled),
                state.background_status.as_str(),
                state.last_checked_at.map(to_i64),
                state.last_attempt_at.map(to_i64),
                state.last_success_at.map(to_i64),
                state.next_due_at.map(to_i64),
                state.pending_reason.map(|reason| reason.as_str()),
                state.last_error_code.as_deref(),
                state.worker_instance_id.as_deref(),
                state.worker_heartbeat_at.map(to_i64),
                state.lease_owner.as_deref(),
                state.lease_expires_at.map(to_i64),
                to_i64(state.updated_at),
            ],
        )
        .context("failed to upsert save backup scheduler state")?;
        Ok(())
    }

    fn acquire_due_lease(
        &self,
        request: SaveBackupSchedulerLeaseRequest,
    ) -> Result<Option<SaveBackupSchedulerState>> {
        let mut conn = self.lock_db()?;
        let tx = conn
            .transaction()
            .context("failed to start scheduler lease transaction")?;

        let Some(mut state) = select_state(&tx, &request.game_id, &request.profile_id)? else {
            tx.commit()
                .context("failed to finish missing scheduler lease transaction")?;
            return Ok(None);
        };

        let lease_is_busy = state.lease_owner.is_some()
            && state
                .lease_expires_at
                .is_some_and(|expires_at| expires_at > request.now_unix_millis);
        if lease_is_busy {
            tx.commit()
                .context("failed to finish busy scheduler lease transaction")?;
            return Ok(None);
        }

        state.lease_owner = Some(request.lease_owner);
        state.lease_expires_at = Some(request.lease_expires_at);
        state.last_checked_at = request.last_checked_at;
        state.next_due_at = request.next_due_at;
        state.updated_at = request.now_unix_millis;

        update_state_in_transaction(&tx, &state)?;
        tx.commit()
            .context("failed to commit scheduler lease transaction")?;
        Ok(Some(state))
    }

    fn renew_lease(&self, request: SaveBackupSchedulerLeaseRenewalRequest) -> Result<bool> {
        if request.lease_expires_at <= request.now_unix_millis {
            return Ok(false);
        }

        let conn = self.lock_db()?;
        let updated = conn
            .execute(
                "UPDATE save_backup_scheduler_state
                 SET lease_expires_at = ?1, updated_at = ?2
                 WHERE game_id = ?3 AND profile_id = ?4 AND lease_owner = ?5
                   AND lease_expires_at > ?2 AND lease_expires_at <= ?1",
                params![
                    to_i64(request.lease_expires_at),
                    to_i64(request.now_unix_millis),
                    request.game_id.as_str(),
                    request.profile_id.as_str(),
                    request.lease_owner,
                ],
            )
            .context("failed to renew save backup scheduler lease")?;
        Ok(updated == 1)
    }

    fn release_lease(
        &self,
        game_id: &GameId,
        profile_id: &ProfileId,
        lease_owner: &str,
    ) -> Result<()> {
        let conn = self.lock_db()?;
        conn.execute(
            "UPDATE save_backup_scheduler_state
             SET lease_owner = NULL, lease_expires_at = NULL
             WHERE game_id = ?1 AND profile_id = ?2 AND lease_owner = ?3",
            params![game_id.as_str(), profile_id.as_str(), lease_owner],
        )
        .context("failed to release save backup scheduler lease")?;
        Ok(())
    }

    fn record_worker_heartbeat(&self, heartbeat: SaveBackupWorkerHeartbeat) -> Result<()> {
        let conn = self.lock_db()?;
        let updated = conn
            .execute(
                "UPDATE save_backup_scheduler_state
             SET worker_instance_id = ?3,
                 worker_heartbeat_at = ?4,
                 updated_at = ?4
             WHERE game_id = ?1 AND profile_id = ?2",
                params![
                    heartbeat.game_id.as_str(),
                    heartbeat.profile_id.as_str(),
                    heartbeat.worker_instance_id,
                    to_i64(heartbeat.heartbeat_at),
                ],
            )
            .context("failed to record save backup worker heartbeat")?;
        if updated == 0 {
            return Err(anyhow!(
                "no scheduler state row exists for worker heartbeat"
            ));
        }
        Ok(())
    }
}

fn select_state(
    conn: &Connection,
    game_id: &GameId,
    profile_id: &ProfileId,
) -> Result<Option<SaveBackupSchedulerState>> {
    conn.query_row(
        "SELECT game_id, profile_id, enabled, background_protection_enabled, background_status,
                last_checked_at, last_attempt_at, last_success_at, next_due_at,
                pending_reason, last_error_code, worker_instance_id, worker_heartbeat_at,
                lease_owner, lease_expires_at, updated_at
         FROM save_backup_scheduler_state
         WHERE game_id = ?1 AND profile_id = ?2",
        params![game_id.as_str(), profile_id.as_str()],
        row_to_state,
    )
    .optional()
    .context("failed to load save backup scheduler state")
}

fn row_to_state(row: &rusqlite::Row<'_>) -> rusqlite::Result<SaveBackupSchedulerState> {
    let game_id: String = row.get(0)?;
    let profile_id: String = row.get(1)?;
    let background_status: String = row.get(4)?;
    let pending_reason: Option<String> = row.get(9)?;

    Ok(SaveBackupSchedulerState {
        game_id: GameId::parse(game_id).map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                0,
                rusqlite::types::Type::Text,
                Box::new(error),
            )
        })?,
        profile_id: ProfileId::new(profile_id),
        enabled: row.get::<_, i64>(2)? != 0,
        background_protection_enabled: row.get::<_, i64>(3)? != 0,
        background_status: parse_background_status(&background_status).map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                4,
                rusqlite::types::Type::Text,
                Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, error)),
            )
        })?,
        last_checked_at: optional_i64_to_u128(row.get(5)?),
        last_attempt_at: optional_i64_to_u128(row.get(6)?),
        last_success_at: optional_i64_to_u128(row.get(7)?),
        next_due_at: optional_i64_to_u128(row.get(8)?),
        pending_reason: pending_reason
            .as_deref()
            .map(parse_pending_reason)
            .transpose()
            .map_err(|error| {
                rusqlite::Error::FromSqlConversionFailure(
                    9,
                    rusqlite::types::Type::Text,
                    Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, error)),
                )
            })?,
        last_error_code: row.get(10)?,
        worker_instance_id: row.get(11)?,
        worker_heartbeat_at: optional_i64_to_u128(row.get(12)?),
        lease_owner: row.get(13)?,
        lease_expires_at: optional_i64_to_u128(row.get(14)?),
        updated_at: i64_to_u128(row.get(15)?),
    })
}

fn update_state_in_transaction(conn: &Connection, state: &SaveBackupSchedulerState) -> Result<()> {
    conn.execute(
        "UPDATE save_backup_scheduler_state
         SET enabled = ?3,
             background_protection_enabled = ?4,
             background_status = ?5,
             last_checked_at = ?6,
             last_attempt_at = ?7,
             last_success_at = ?8,
             next_due_at = ?9,
             pending_reason = ?10,
             last_error_code = ?11,
             worker_instance_id = ?12,
             worker_heartbeat_at = ?13,
             lease_owner = ?14,
             lease_expires_at = ?15,
             updated_at = ?16
         WHERE game_id = ?1 AND profile_id = ?2",
        params![
            state.game_id.as_str(),
            state.profile_id.as_str(),
            bool_to_i64(state.enabled),
            bool_to_i64(state.background_protection_enabled),
            state.background_status.as_str(),
            state.last_checked_at.map(to_i64),
            state.last_attempt_at.map(to_i64),
            state.last_success_at.map(to_i64),
            state.next_due_at.map(to_i64),
            state.pending_reason.map(|reason| reason.as_str()),
            state.last_error_code.as_deref(),
            state.worker_instance_id.as_deref(),
            state.worker_heartbeat_at.map(to_i64),
            state.lease_owner.as_deref(),
            state.lease_expires_at.map(to_i64),
            to_i64(state.updated_at),
        ],
    )
    .context("failed to update save backup scheduler state")?;
    Ok(())
}

fn parse_background_status(
    value: &str,
) -> std::result::Result<SaveBackupBackgroundProtectionStatus, String> {
    match value {
        "protected" => Ok(SaveBackupBackgroundProtectionStatus::Protected),
        "tray_only" => Ok(SaveBackupBackgroundProtectionStatus::TrayOnly),
        "not_enabled" => Ok(SaveBackupBackgroundProtectionStatus::NotEnabled),
        "starting" => Ok(SaveBackupBackgroundProtectionStatus::Starting),
        "registration_failed" => Ok(SaveBackupBackgroundProtectionStatus::RegistrationFailed),
        "worker_unhealthy" => Ok(SaveBackupBackgroundProtectionStatus::WorkerUnhealthy),
        "permission_required" => Ok(SaveBackupBackgroundProtectionStatus::PermissionRequired),
        "unsupported_platform" => Ok(SaveBackupBackgroundProtectionStatus::UnsupportedPlatform),
        other => Err(format!("unknown save backup background status: {other}")),
    }
}

fn parse_pending_reason(
    value: &str,
) -> std::result::Result<SaveBackupSchedulerPendingReason, String> {
    match value {
        "game_running" => Ok(SaveBackupSchedulerPendingReason::GameRunning),
        "game_running_unknown" => Ok(SaveBackupSchedulerPendingReason::GameRunningUnknown),
        "source_invalid" => Ok(SaveBackupSchedulerPendingReason::SourceInvalid),
        "destination_unavailable" => Ok(SaveBackupSchedulerPendingReason::DestinationUnavailable),
        "task_conflict" => Ok(SaveBackupSchedulerPendingReason::TaskConflict),
        other => Err(format!("unknown save backup pending reason: {other}")),
    }
}

fn bool_to_i64(value: bool) -> i64 {
    if value {
        1
    } else {
        0
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn persisted_starting_background_status_is_supported() {
        assert_eq!(
            parse_background_status("starting"),
            Ok(SaveBackupBackgroundProtectionStatus::Starting)
        );
    }
}
