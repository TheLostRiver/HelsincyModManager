use hmm_core::{GameInstance, ModInstallationContext, ProfileId};
use thiserror::Error;

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum ModInstallationScopeError {
    #[error("game installation is unavailable")]
    GameUnavailable,
    #[error("multiple legacy Mod installation namespaces require reconciliation")]
    LegacyAmbiguous,
    #[error("Mod installation scope is unavailable")]
    Unavailable,
    #[error("Mod installation scope no longer matches the configured game directory")]
    Mismatch,
}

impl ModInstallationScopeError {
    pub const fn code(self) -> &'static str {
        match self {
            Self::GameUnavailable => "mod_installation_game_unavailable",
            Self::LegacyAmbiguous => "mod_installation_legacy_ambiguous",
            Self::Unavailable => "mod_installation_scope_unavailable",
            Self::Mismatch => "mod_installation_scope_mismatch",
        }
    }
}

/// Enumerates authoritative install namespaces, including legacy/orphaned records.
/// Save-backup profiles must not be used as an index of installed Mods.
pub trait ModInstallationScopeIndex: Send + Sync {
    fn list_scope_ids(&self) -> anyhow::Result<Vec<ProfileId>>;
}

pub trait ModInstallationScopeRepository: ModInstallationScopeIndex {
    /// Inspect the binding without creating directories, lock files, or metadata.
    fn inspect_scope(
        &self,
        game: &GameInstance,
    ) -> Result<ModInstallationContext, ModInstallationScopeError>;

    /// Persist a directory-to-namespace binding without changing installed files or evidence.
    /// A unique legacy namespace may be retained; ambiguous legacy state must fail closed.
    fn resolve_scope(
        &self,
        game: &GameInstance,
    ) -> Result<ModInstallationContext, ModInstallationScopeError>;
}
