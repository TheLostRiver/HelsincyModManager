use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

use hmm_core::{GameId, ProfileId};

use crate::{TaskKind, TaskManagerError, TaskSnapshot};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct SaveProfileMaintenanceScope {
    game_id: String,
    profile_id: String,
}

impl SaveProfileMaintenanceScope {
    fn new(game_id: &GameId, profile_id: &ProfileId) -> Self {
        Self {
            game_id: game_id.as_str().to_owned(),
            profile_id: profile_id.as_str().to_owned(),
        }
    }
}

#[derive(Debug, Default)]
pub struct SaveProfileMaintenanceScopeRegistry {
    active_reservations: Mutex<BTreeMap<SaveProfileMaintenanceScope, String>>,
    pending_sequence: AtomicU64,
}

impl SaveProfileMaintenanceScopeRegistry {
    pub(crate) fn reserve_task(
        &self,
        game_id: &GameId,
        profile_id: &ProfileId,
        kind: TaskKind,
        create_task: impl FnOnce() -> Result<TaskSnapshot, TaskManagerError>,
    ) -> Result<TaskSnapshot, TaskManagerError> {
        let pending_id = format!(
            "save-profile-maintenance-pending-{}",
            self.pending_sequence.fetch_add(1, Ordering::Relaxed)
        );
        let mut pending = self.reserve_pending(game_id, profile_id, kind, pending_id)?;
        let task = create_task()?;
        pending.commit(&task.task_id)?;
        Ok(task)
    }

    pub(crate) fn reserve_maintenance(
        &self,
        game_id: &GameId,
        profile_id: &ProfileId,
        kind: TaskKind,
        reservation_id: impl Into<String>,
    ) -> Result<SaveProfileMaintenanceScopeGuard<'_>, TaskManagerError> {
        let scope = SaveProfileMaintenanceScope::new(game_id, profile_id);
        let reservation_id = reservation_id.into();
        let mut active_reservations = self
            .active_reservations
            .lock()
            .map_err(|_| TaskManagerError::TaskStoreUnavailable)?;
        if let Some(active_id) = active_reservations.get(&scope) {
            return Err(TaskManagerError::TaskScopeBusy {
                kind,
                task_id: active_id.clone(),
            });
        }
        active_reservations.insert(scope.clone(), reservation_id.clone());
        drop(active_reservations);

        Ok(SaveProfileMaintenanceScopeGuard {
            registry: self,
            scope,
            reservation_id,
        })
    }

    pub(crate) fn release_task(&self, game_id: &GameId, profile_id: &ProfileId, task_id: &str) {
        let scope = SaveProfileMaintenanceScope::new(game_id, profile_id);
        self.release_scope(&scope, task_id);
    }

    fn reserve_pending(
        &self,
        game_id: &GameId,
        profile_id: &ProfileId,
        kind: TaskKind,
        reservation_id: String,
    ) -> Result<PendingSaveProfileMaintenanceScopeGuard<'_>, TaskManagerError> {
        let scope = SaveProfileMaintenanceScope::new(game_id, profile_id);
        let mut active_reservations = self
            .active_reservations
            .lock()
            .map_err(|_| TaskManagerError::TaskStoreUnavailable)?;
        if let Some(active_id) = active_reservations.get(&scope) {
            return Err(TaskManagerError::TaskScopeBusy {
                kind,
                task_id: active_id.clone(),
            });
        }
        active_reservations.insert(scope.clone(), reservation_id.clone());
        drop(active_reservations);

        Ok(PendingSaveProfileMaintenanceScopeGuard {
            registry: self,
            scope,
            reservation_id,
            committed: false,
        })
    }

    fn release_scope(&self, scope: &SaveProfileMaintenanceScope, reservation_id: &str) {
        let Ok(mut active_reservations) = self.active_reservations.lock() else {
            return;
        };
        if active_reservations
            .get(scope)
            .is_some_and(|active_id| active_id == reservation_id)
        {
            active_reservations.remove(scope);
        }
    }
}

struct PendingSaveProfileMaintenanceScopeGuard<'a> {
    registry: &'a SaveProfileMaintenanceScopeRegistry,
    scope: SaveProfileMaintenanceScope,
    reservation_id: String,
    committed: bool,
}

impl PendingSaveProfileMaintenanceScopeGuard<'_> {
    fn commit(&mut self, task_id: &str) -> Result<(), TaskManagerError> {
        let mut active_reservations = self
            .registry
            .active_reservations
            .lock()
            .map_err(|_| TaskManagerError::TaskStoreUnavailable)?;
        if active_reservations
            .get(&self.scope)
            .is_none_or(|active_id| active_id != &self.reservation_id)
        {
            return Err(TaskManagerError::TaskStoreUnavailable);
        }
        active_reservations.insert(self.scope.clone(), task_id.to_owned());
        self.committed = true;
        Ok(())
    }
}

impl Drop for PendingSaveProfileMaintenanceScopeGuard<'_> {
    fn drop(&mut self) {
        if !self.committed {
            self.registry
                .release_scope(&self.scope, &self.reservation_id);
        }
    }
}

pub(crate) struct SaveProfileMaintenanceScopeGuard<'a> {
    registry: &'a SaveProfileMaintenanceScopeRegistry,
    scope: SaveProfileMaintenanceScope,
    reservation_id: String,
}

impl Drop for SaveProfileMaintenanceScopeGuard<'_> {
    fn drop(&mut self) {
        self.registry
            .release_scope(&self.scope, &self.reservation_id);
    }
}
