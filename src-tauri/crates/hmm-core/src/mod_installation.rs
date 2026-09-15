use crate::{GameId, ProfileId};
use serde::{Deserialize, Serialize};

/// A game's Mod installation state, independent of save-backup account profiles.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModInstallationContext {
    pub game_id: GameId,
    /// Opaque identity of the game and its normalized installation directory.
    pub installation_id: String,
    /// Existing install formats call this field `profile_id`. It is a storage namespace,
    /// never a reference to the save-backup Profile repository.
    pub scope_id: ProfileId,
}
