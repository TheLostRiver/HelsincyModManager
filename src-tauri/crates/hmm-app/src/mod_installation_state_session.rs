use crate::{
    InstallManifestQueryError, InstallManifestStatusSummary, ModInstallationStateQueryService,
    ModInstallationStateSnapshot,
};
use hmm_core::{GameId, ModId, ProfileId};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};

pub const MAX_MOD_INSTALLATION_STATE_IDS: usize = 2048;
const MAX_SCOPES: usize = 16;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModInstallationStateUpdate {
    pub game_id: GameId,
    pub profile_id: ProfileId,
    pub epoch: String,
    pub revision: u32,
    pub reset: bool,
    pub available: bool,
    pub mod_ids: Vec<ModId>,
    pub summaries: Vec<InstallManifestStatusSummary>,
}

struct ObservedScope {
    epoch: String,
    revision: u32,
    snapshot: Option<ModInstallationStateSnapshot>,
}

/// Orders observations, including failed reads. No snapshot substitutes for a fresh
/// repository read; retained metadata is only used to identify affected Mod ids.
pub struct ModInstallationStateSession {
    query: Arc<ModInstallationStateQueryService>,
    scopes: Mutex<BTreeMap<(String, ProfileId), ObservedScope>>,
}

impl ModInstallationStateSession {
    pub fn new(query: Arc<ModInstallationStateQueryService>) -> Self {
        Self {
            query,
            scopes: Mutex::new(BTreeMap::new()),
        }
    }

    pub fn read(
        &self,
        game_id: &GameId,
        profile_id: &ProfileId,
        mod_ids: &[ModId],
    ) -> Result<ModInstallationStateUpdate, InstallManifestQueryError> {
        if mod_ids.len() > MAX_MOD_INSTALLATION_STATE_IDS {
            return Err(InstallManifestQueryError::ManifestUnavailable);
        }
        let mut scopes = self
            .scopes
            .lock()
            .map_err(|_| InstallManifestQueryError::ManifestUnavailable)?;
        let key = (game_id.as_str().to_owned(), profile_id.clone());
        if !scopes.contains_key(&key) && scopes.len() >= MAX_SCOPES {
            scopes.pop_first();
        }
        let scope = scopes.entry(key).or_insert_with(|| ObservedScope {
            epoch: uuid::Uuid::new_v4().to_string(),
            revision: 0,
            snapshot: None,
        });
        scope.revision = scope
            .revision
            .checked_add(1)
            .ok_or(InstallManifestQueryError::ManifestUnavailable)?;
        let fresh = self.query.inspect(profile_id);
        let mut ids = mod_ids.iter().cloned().collect::<BTreeSet<_>>();
        let mut reset = scope.snapshot.is_none();
        if let (Some(previous), Ok(current)) = (&scope.snapshot, &fresh) {
            reset |= previous.default_status != current.default_status;
            let changed = current.changed_mod_ids(previous);
            if ids.union(&changed).count() > MAX_MOD_INSTALLATION_STATE_IDS {
                reset = true;
            } else {
                ids.extend(changed);
            }
        }
        let summaries = fresh
            .as_ref()
            .map(|snapshot| ids.iter().map(|id| snapshot.summary(id)).collect())
            .unwrap_or_default();
        let available = fresh.is_ok();
        reset |= !available;
        scope.snapshot = fresh.ok();
        Ok(ModInstallationStateUpdate {
            game_id: game_id.clone(),
            profile_id: profile_id.clone(),
            epoch: scope.epoch.clone(),
            revision: scope.revision,
            reset,
            available,
            mod_ids: ids.into_iter().collect(),
            summaries,
        })
    }
}
