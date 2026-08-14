use crate::{GameId, ProfileId};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaveRestoreTransactionStatus {
    Planned,
    Prepared,
    PreRestoreCompleted,
    Committing,
    Committed,
    Completed,
    RolledBack,
    RecoveryRequired,
    Failed,
}

impl SaveRestoreTransactionStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Planned => "planned",
            Self::Prepared => "prepared",
            Self::PreRestoreCompleted => "pre_restore_completed",
            Self::Committing => "committing",
            Self::Committed => "committed",
            Self::Completed => "completed",
            Self::RolledBack => "rolled_back",
            Self::RecoveryRequired => "recovery_required",
            Self::Failed => "failed",
        }
    }

    pub const fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Completed | Self::RolledBack | Self::RecoveryRequired | Self::Failed
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaveRestoreTransaction {
    pub transaction_id: String,
    pub game_id: GameId,
    pub profile_id: ProfileId,
    pub backup_id: String,
    pub pre_restore_backup_id: Option<String>,
    pub status: SaveRestoreTransactionStatus,
    pub error_code: Option<String>,
    pub created_at: u128,
    pub updated_at: u128,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transaction_statuses_have_stable_codes_and_terminal_semantics() {
        assert_eq!(SaveRestoreTransactionStatus::Planned.as_str(), "planned");
        assert_eq!(
            SaveRestoreTransactionStatus::PreRestoreCompleted.as_str(),
            "pre_restore_completed"
        );
        assert!(!SaveRestoreTransactionStatus::Committing.is_terminal());
        assert_eq!(
            SaveRestoreTransactionStatus::Committed.as_str(),
            "committed"
        );
        assert!(!SaveRestoreTransactionStatus::Committed.is_terminal());
        assert!(SaveRestoreTransactionStatus::RecoveryRequired.is_terminal());
    }
}
