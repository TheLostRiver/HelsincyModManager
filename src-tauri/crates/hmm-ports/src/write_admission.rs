use crate::CancellationToken;
use hmm_core::{GameId, ProfileId};
use std::time::Duration;
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CrossProcessWriteScopeKind {
    BackgroundRegistrationWrite,
    SaveProfileWrite,
    GameProfileWrite,
}

impl CrossProcessWriteScopeKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::BackgroundRegistrationWrite => "background-registration-write",
            Self::SaveProfileWrite => "save-profile-write",
            Self::GameProfileWrite => "game-profile-write",
        }
    }

    pub const fn order_rank(self) -> u8 {
        match self {
            Self::BackgroundRegistrationWrite => 0,
            Self::SaveProfileWrite => 1,
            Self::GameProfileWrite => 2,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum CrossProcessWriteScope {
    BackgroundRegistration,
    SaveProfile {
        game_id: GameId,
        profile_id: ProfileId,
    },
    GameProfile {
        game_id: GameId,
        profile_id: ProfileId,
    },
}

impl CrossProcessWriteScope {
    pub fn background_registration() -> Self {
        Self::BackgroundRegistration
    }

    pub fn save_profile(game_id: &GameId, profile_id: &ProfileId) -> Self {
        Self::SaveProfile {
            game_id: game_id.clone(),
            profile_id: profile_id.clone(),
        }
    }

    pub fn game_profile(game_id: &GameId, profile_id: &ProfileId) -> Self {
        Self::GameProfile {
            game_id: game_id.clone(),
            profile_id: profile_id.clone(),
        }
    }

    pub const fn kind(&self) -> CrossProcessWriteScopeKind {
        match self {
            Self::BackgroundRegistration => CrossProcessWriteScopeKind::BackgroundRegistrationWrite,
            Self::SaveProfile { .. } => CrossProcessWriteScopeKind::SaveProfileWrite,
            Self::GameProfile { .. } => CrossProcessWriteScopeKind::GameProfileWrite,
        }
    }

    pub fn game_profile_identity(&self) -> Option<(&GameId, &ProfileId)> {
        match self {
            Self::BackgroundRegistration => None,
            Self::SaveProfile {
                game_id,
                profile_id,
            }
            | Self::GameProfile {
                game_id,
                profile_id,
            } => Some((game_id, profile_id)),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CrossProcessWriteRecovery {
    AbandonedOwner,
    StaleOwnerMetadata,
}

impl CrossProcessWriteRecovery {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::AbandonedOwner => "abandoned_owner",
            Self::StaleOwnerMetadata => "stale_owner_metadata",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CrossProcessWriteAcquisition {
    pub recovery: Option<CrossProcessWriteRecovery>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum CrossProcessWriteAdmissionError {
    #[error("cross-process write admission is busy")]
    Busy,
    #[error("cross-process write admission was cancelled")]
    Cancelled,
    #[error("cross-process write admission order was violated")]
    OrderViolation,
    #[error("cross-process write admission is unavailable")]
    Unavailable,
}

impl CrossProcessWriteAdmissionError {
    pub const fn code(self) -> &'static str {
        match self {
            Self::Busy => "write_admission_busy",
            Self::Cancelled => "write_admission_cancelled",
            Self::OrderViolation => "write_admission_order_violation",
            Self::Unavailable => "write_admission_unavailable",
        }
    }
}

pub type CrossProcessWriteAdmissionResult<T> =
    std::result::Result<T, CrossProcessWriteAdmissionError>;

pub trait CrossProcessWriteGuard {
    fn scope(&self) -> &CrossProcessWriteScope;

    fn acquisition(&self) -> CrossProcessWriteAcquisition;
}

pub trait CrossProcessWriteAdmission: Send + Sync {
    fn acquire(
        &self,
        scope: &CrossProcessWriteScope,
        timeout: Duration,
        cancellation: &dyn CancellationToken,
    ) -> CrossProcessWriteAdmissionResult<Box<dyn CrossProcessWriteGuard>>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scope_kinds_have_stable_names_and_order() {
        let background = CrossProcessWriteScope::background_registration();
        let save =
            CrossProcessWriteScope::save_profile(&GameId::mhw(), &ProfileId::new("profile-a"));
        let game =
            CrossProcessWriteScope::game_profile(&GameId::mhw(), &ProfileId::new("profile-a"));

        assert_eq!(background.kind().as_str(), "background-registration-write");
        assert_eq!(save.kind().as_str(), "save-profile-write");
        assert_eq!(game.kind().as_str(), "game-profile-write");
        assert!(background.kind().order_rank() < save.kind().order_rank());
        assert!(save.kind().order_rank() < game.kind().order_rank());
    }

    #[test]
    fn admission_errors_have_stable_codes() {
        assert_eq!(
            CrossProcessWriteAdmissionError::Busy.code(),
            "write_admission_busy"
        );
        assert_eq!(
            CrossProcessWriteAdmissionError::Cancelled.code(),
            "write_admission_cancelled"
        );
        assert_eq!(
            CrossProcessWriteAdmissionError::OrderViolation.code(),
            "write_admission_order_violation"
        );
        assert_eq!(
            CrossProcessWriteAdmissionError::Unavailable.code(),
            "write_admission_unavailable"
        );
    }
}
