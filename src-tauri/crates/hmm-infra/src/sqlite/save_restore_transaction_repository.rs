use anyhow::{Context, Result};
use hmm_core::{GameId, ProfileId, SaveRestoreTransaction, SaveRestoreTransactionStatus};
use hmm_ports::SaveRestoreTransactionRepository;
use rusqlite::Connection;
use std::sync::{Arc, Mutex};

pub struct SqliteSaveRestoreTransactionRepository {
    db: Arc<Mutex<Connection>>,
}

impl SqliteSaveRestoreTransactionRepository {
    pub fn new(db: Arc<Mutex<Connection>>) -> Self {
        Self { db }
    }

    fn lock_db(&self) -> Result<std::sync::MutexGuard<'_, Connection>> {
        self.db
            .lock()
            .map_err(|_| anyhow::anyhow!("database lock poisoned"))
    }

    fn row_to_transaction(row: &rusqlite::Row<'_>) -> rusqlite::Result<SaveRestoreTransaction> {
        let game_id: String = row.get(1)?;
        let status: String = row.get(5)?;
        let created_at = timestamp_from_sql(row.get(7)?, 7)?;
        let updated_at = timestamp_from_sql(row.get(8)?, 8)?;
        Ok(SaveRestoreTransaction {
            transaction_id: row.get(0)?,
            game_id: GameId::parse(game_id).map_err(|error| {
                rusqlite::Error::FromSqlConversionFailure(
                    1,
                    rusqlite::types::Type::Text,
                    Box::new(error),
                )
            })?,
            profile_id: ProfileId::new(row.get::<_, String>(2)?),
            backup_id: row.get(3)?,
            pre_restore_backup_id: row.get(4)?,
            status: parse_status(&status).map_err(|error| {
                rusqlite::Error::FromSqlConversionFailure(
                    5,
                    rusqlite::types::Type::Text,
                    Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, error)),
                )
            })?,
            error_code: row.get(6)?,
            created_at,
            updated_at,
        })
    }
}

impl SaveRestoreTransactionRepository for SqliteSaveRestoreTransactionRepository {
    fn save_transaction(&self, transaction: &SaveRestoreTransaction) -> Result<()> {
        let created_at = i64::try_from(transaction.created_at)
            .context("restore transaction created_at is outside SQLite range")?;
        let updated_at = i64::try_from(transaction.updated_at)
            .context("restore transaction updated_at is outside SQLite range")?;
        let conn = self.lock_db()?;
        conn.execute(
            "INSERT INTO save_restore_transactions
                (transaction_id, game_id, profile_id, backup_id, pre_restore_backup_id,
                 status, error_code, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
             ON CONFLICT(transaction_id) DO UPDATE SET
                game_id = excluded.game_id,
                profile_id = excluded.profile_id,
                backup_id = excluded.backup_id,
                pre_restore_backup_id = excluded.pre_restore_backup_id,
                status = excluded.status,
                error_code = excluded.error_code,
                created_at = excluded.created_at,
                updated_at = excluded.updated_at",
            rusqlite::params![
                transaction.transaction_id,
                transaction.game_id.as_str(),
                transaction.profile_id.as_str(),
                transaction.backup_id,
                transaction.pre_restore_backup_id,
                transaction.status.as_str(),
                transaction.error_code,
                created_at,
                updated_at,
            ],
        )
        .context("failed to save restore transaction")?;
        Ok(())
    }

    fn get_transaction(&self, transaction_id: &str) -> Result<Option<SaveRestoreTransaction>> {
        let conn = self.lock_db()?;
        let result = conn.query_row(
            "SELECT transaction_id, game_id, profile_id, backup_id, pre_restore_backup_id,
                    status, error_code, created_at, updated_at
             FROM save_restore_transactions WHERE transaction_id = ?1",
            rusqlite::params![transaction_id],
            Self::row_to_transaction,
        );
        match result {
            Ok(transaction) => Ok(Some(transaction)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(error) => Err(error).context("failed to read restore transaction"),
        }
    }

    fn has_incomplete_transaction_excluding(
        &self,
        game_id: &GameId,
        profile_id: &ProfileId,
        excluded_transaction_id: Option<&str>,
    ) -> Result<bool> {
        let conn = self.lock_db()?;
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM save_restore_transactions
             WHERE game_id = ?1 AND profile_id = ?2
               AND status NOT IN ('completed', 'rolled_back', 'failed')
               AND (?3 IS NULL OR transaction_id <> ?3)",
            rusqlite::params![
                game_id.as_str(),
                profile_id.as_str(),
                excluded_transaction_id
            ],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }
}

fn timestamp_from_sql(value: i64, column: usize) -> rusqlite::Result<u128> {
    u128::try_from(value).map_err(|_| {
        rusqlite::Error::FromSqlConversionFailure(
            column,
            rusqlite::types::Type::Integer,
            Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "restore transaction timestamp is negative",
            )),
        )
    })
}

fn parse_status(value: &str) -> std::result::Result<SaveRestoreTransactionStatus, String> {
    match value {
        "planned" => Ok(SaveRestoreTransactionStatus::Planned),
        "prepared" => Ok(SaveRestoreTransactionStatus::Prepared),
        "pre_restore_completed" => Ok(SaveRestoreTransactionStatus::PreRestoreCompleted),
        "committing" => Ok(SaveRestoreTransactionStatus::Committing),
        "committed" => Ok(SaveRestoreTransactionStatus::Committed),
        "completed" => Ok(SaveRestoreTransactionStatus::Completed),
        "rolled_back" => Ok(SaveRestoreTransactionStatus::RolledBack),
        "recovery_required" => Ok(SaveRestoreTransactionStatus::RecoveryRequired),
        "failed" => Ok(SaveRestoreTransactionStatus::Failed),
        other => Err(format!("unknown restore transaction status: {other}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sqlite::open_database;

    #[test]
    fn transaction_repository_round_trips_and_detects_incomplete_state() {
        let temp = tempfile::tempdir().expect("temp dir");
        let conn = open_database(&temp.path().join("test.db")).expect("open db");
        let repo = SqliteSaveRestoreTransactionRepository::new(Arc::new(Mutex::new(conn)));
        let transaction = SaveRestoreTransaction {
            transaction_id: "restore-1".to_owned(),
            game_id: GameId::mhw(),
            profile_id: ProfileId::new("default"),
            backup_id: "backup-1".to_owned(),
            pre_restore_backup_id: None,
            status: SaveRestoreTransactionStatus::Committing,
            error_code: None,
            created_at: 1,
            updated_at: 2,
        };
        repo.save_transaction(&transaction)
            .expect("save transaction");
        assert!(repo
            .has_incomplete_transaction(&GameId::mhw(), &ProfileId::new("default"))
            .expect("check pending"));
        assert!(!repo
            .has_incomplete_transaction_excluding(
                &GameId::mhw(),
                &ProfileId::new("default"),
                Some("restore-1"),
            )
            .expect("exclude current transaction"));
        assert_eq!(
            repo.get_transaction("restore-1")
                .expect("read transaction")
                .expect("transaction exists"),
            transaction
        );

        let mut committed = transaction.clone();
        committed.status = SaveRestoreTransactionStatus::Committed;
        committed.updated_at = 3;
        repo.save_transaction(&committed)
            .expect("save committed transaction");
        assert!(repo
            .has_incomplete_transaction(&GameId::mhw(), &ProfileId::new("default"))
            .expect("committed transaction still blocks restore"));
        assert_eq!(
            repo.get_transaction("restore-1")
                .expect("read committed transaction")
                .expect("committed transaction exists"),
            committed
        );
    }

    #[test]
    fn transaction_repository_rejects_timestamps_outside_the_domain_range() {
        let temp = tempfile::tempdir().expect("temp dir");
        let conn = open_database(&temp.path().join("test.db")).expect("open db");
        let db = Arc::new(Mutex::new(conn));
        let repo = SqliteSaveRestoreTransactionRepository::new(Arc::clone(&db));
        let mut transaction = SaveRestoreTransaction {
            transaction_id: "restore-overflow".to_owned(),
            game_id: GameId::mhw(),
            profile_id: ProfileId::new("default"),
            backup_id: "backup-1".to_owned(),
            pre_restore_backup_id: None,
            status: SaveRestoreTransactionStatus::Planned,
            error_code: None,
            created_at: i64::MAX as u128 + 1,
            updated_at: 2,
        };
        assert!(repo.save_transaction(&transaction).is_err());

        transaction.transaction_id = "restore-negative".to_owned();
        db.lock()
            .expect("database")
            .execute(
                "INSERT INTO save_restore_transactions
                    (transaction_id, game_id, profile_id, backup_id, pre_restore_backup_id,
                     status, error_code, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, NULL, ?5, NULL, -1, 2)",
                rusqlite::params![
                    transaction.transaction_id,
                    transaction.game_id.as_str(),
                    transaction.profile_id.as_str(),
                    transaction.backup_id,
                    transaction.status.as_str(),
                ],
            )
            .expect("insert corrupted row");
        assert!(repo.get_transaction("restore-negative").is_err());
    }
}
