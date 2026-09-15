use crate::{InstallWriteAdmission, InstallWriteAdmissionError};
use hmm_core::{GameId, GameInstance, ModInstallationContext, ProfileId};
use hmm_ports::{GameConfigRepository, ModInstallationScopeError, ModInstallationScopeRepository};
use std::sync::Arc;

pub struct ModInstallationScopeService {
    games: Arc<dyn GameConfigRepository>,
    scopes: Arc<dyn ModInstallationScopeRepository>,
}

impl ModInstallationScopeService {
    pub fn new(
        games: Arc<dyn GameConfigRepository>,
        scopes: Arc<dyn ModInstallationScopeRepository>,
    ) -> Self {
        Self { games, scopes }
    }

    pub fn context(
        &self,
        game_id: &GameId,
    ) -> Result<ModInstallationContext, ModInstallationScopeError> {
        self.scopes.resolve_scope(&self.game(game_id)?)
    }

    fn game(&self, game_id: &GameId) -> Result<GameInstance, ModInstallationScopeError> {
        let game = self
            .games
            .load_game_instance(game_id)
            .map_err(|_| ModInstallationScopeError::Unavailable)?
            .ok_or(ModInstallationScopeError::GameUnavailable)?;
        if &game.game_id != game_id {
            return Err(ModInstallationScopeError::GameUnavailable);
        }
        Ok(game)
    }

    pub fn require_current(
        &self,
        game_id: &GameId,
        scope_id: &ProfileId,
    ) -> Result<ModInstallationContext, ModInstallationScopeError> {
        let context = self.scopes.inspect_scope(&self.game(game_id)?)?;
        if &context.scope_id != scope_id {
            return Err(ModInstallationScopeError::Mismatch);
        }
        Ok(context)
    }
}

impl InstallWriteAdmission for ModInstallationScopeService {
    fn ensure_write_allowed(
        &self,
        game_id: &GameId,
        scope_id: &ProfileId,
    ) -> Result<(), InstallWriteAdmissionError> {
        let context = self
            .context(game_id)
            .map_err(|_| InstallWriteAdmissionError::SafetyRejected)?;
        if &context.scope_id != scope_id {
            return Err(InstallWriteAdmissionError::SafetyRejected);
        }
        Ok(())
    }
}
