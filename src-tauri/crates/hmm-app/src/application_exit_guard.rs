use crate::{
    SaveBackupExitDecision, SaveBackupExitGuard, SaveBackupExitGuardError, SaveBackupExitReason,
    SaveRestoreTaskScopeRegistry,
};
use std::sync::Arc;
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApplicationExitBlockReason {
    SaveRestoreInProgress,
    SaveRestoreStatusUnavailable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApplicationExitDecision {
    Safe,
    ConfirmationRequired { reason: SaveBackupExitReason },
    Blocked { reason: ApplicationExitBlockReason },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApplicationExitBeginDecision {
    Proceed,
    Blocked { reason: ApplicationExitBlockReason },
}

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum ApplicationExitGuardError {
    #[error("save backup exit guard is unavailable")]
    SaveBackup(#[from] SaveBackupExitGuardError),
}

pub struct ApplicationExitGuard {
    save_backup_guard: Arc<SaveBackupExitGuard>,
    save_restore_scopes: Arc<SaveRestoreTaskScopeRegistry>,
}

impl ApplicationExitGuard {
    pub fn new(
        save_backup_guard: Arc<SaveBackupExitGuard>,
        save_restore_scopes: Arc<SaveRestoreTaskScopeRegistry>,
    ) -> Self {
        Self {
            save_backup_guard,
            save_restore_scopes,
        }
    }

    pub fn evaluate(&self) -> Result<ApplicationExitDecision, ApplicationExitGuardError> {
        match self.save_restore_scopes.has_active_task() {
            Ok(true) => Ok(ApplicationExitDecision::Blocked {
                reason: ApplicationExitBlockReason::SaveRestoreInProgress,
            }),
            Ok(false) => Ok(map_save_backup_decision(self.save_backup_guard.evaluate()?)),
            Err(_) => Ok(ApplicationExitDecision::Blocked {
                reason: ApplicationExitBlockReason::SaveRestoreStatusUnavailable,
            }),
        }
    }

    pub fn begin_exit(&self) -> ApplicationExitBeginDecision {
        match self.save_restore_scopes.begin_exit_if_idle() {
            Ok(true) => ApplicationExitBeginDecision::Proceed,
            Ok(false) => ApplicationExitBeginDecision::Blocked {
                reason: ApplicationExitBlockReason::SaveRestoreInProgress,
            },
            Err(_) => ApplicationExitBeginDecision::Blocked {
                reason: ApplicationExitBlockReason::SaveRestoreStatusUnavailable,
            },
        }
    }

    pub fn record_override(
        &self,
        reason: SaveBackupExitReason,
    ) -> Result<(), SaveBackupExitGuardError> {
        self.save_backup_guard.record_override(reason)
    }
}

fn map_save_backup_decision(decision: SaveBackupExitDecision) -> ApplicationExitDecision {
    match decision {
        SaveBackupExitDecision::Safe => ApplicationExitDecision::Safe,
        SaveBackupExitDecision::ConfirmationRequired { reason } => {
            ApplicationExitDecision::ConfirmationRequired { reason }
        }
    }
}
