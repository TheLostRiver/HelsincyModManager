//! Write gate over `<storage root>/sandboxes` (#275, slice 2).
//!
//! A storage-root migration copies every package out of the effective root and then switches
//! the persisted setting; the running process keeps reading the old root until restart. Any
//! sandbox write that lands in between would either be missed by the copy or end up in a root
//! nobody reads after restart. The gate therefore refuses new sandbox writers from the moment a
//! migration is admitted until the process restarts. Reads (install, thumbnails, external state
//! scan, adopt) are never gated.

use std::sync::{Mutex, PoisonError};
use thiserror::Error;

/// Why sandbox writes are currently refused. Mirrors the `writesFrozen` DTO field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ModStorageWriteFreeze {
    #[default]
    None,
    /// A migration task is copying packages; the setting still names the current root.
    Migration,
    /// The setting now names another root (migration switched, or empty-library `set`);
    /// writes would land in the old root, so they wait for the restart.
    RestartRequired,
}

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum ModStorageWriteGateError {
    #[error("a mod storage migration is in progress")]
    MigrationInProgress,
    #[error("the mod storage directory changed; restart before writing to the library")]
    RestartRequired,
}

impl ModStorageWriteGateError {
    pub const fn code(self) -> &'static str {
        match self {
            Self::MigrationInProgress => "mod_storage_migration_in_progress",
            Self::RestartRequired => "mod_storage_restart_required",
        }
    }
}

#[derive(Debug, Default)]
pub struct ModStorageWriteGate {
    state: Mutex<ModStorageWriteFreeze>,
}

impl ModStorageWriteGate {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn freeze(&self) -> ModStorageWriteFreeze {
        *self.lock()
    }

    /// Fails while writes are frozen. Use [`Self::admit`] when the caller registers a task, so
    /// registration and the freeze check happen under one lock.
    pub fn ensure_open(&self) -> Result<(), ModStorageWriteGateError> {
        freeze_error(*self.lock())
    }

    /// Runs `register` while holding the gate, so a migration admitted right afterwards sees the
    /// task this writer registered and refuses to start (see `begin_migration`).
    pub fn admit<T>(&self, register: impl FnOnce() -> T) -> Result<T, ModStorageWriteGateError> {
        let state = self.lock();
        freeze_error(*state)?;
        Ok(register())
    }

    /// Freezes writes for a migration once `ready` (evaluated under the same lock as `admit`)
    /// agrees; a frozen gate fails before `ready` runs. Errors from `ready` propagate unchanged.
    pub fn begin_migration<E: From<ModStorageWriteGateError>>(
        &self,
        ready: impl FnOnce() -> Result<(), E>,
    ) -> Result<(), E> {
        let mut state = self.lock();
        freeze_error(*state)?;
        ready()?;
        *state = ModStorageWriteFreeze::Migration;
        Ok(())
    }

    /// Ends the migration freeze: a rolled-back migration reopens the gate, a switched one keeps
    /// writes frozen until restart. A gate that is not in the migration state is left alone.
    pub fn end_migration(&self, switched: bool) {
        let mut state = self.lock();
        if *state == ModStorageWriteFreeze::Migration {
            *state = if switched {
                ModStorageWriteFreeze::RestartRequired
            } else {
                ModStorageWriteFreeze::None
            };
        }
    }

    /// Empty-library root switch: runs the settings write under the gate and, when it succeeds,
    /// freezes writes until restart in the same critical section — a writer admitted in between
    /// would land in the old root that nobody reads after the restart.
    pub fn admit_root_switch<T, E>(
        &self,
        write: impl FnOnce() -> Result<T, E>,
    ) -> Result<Result<T, E>, ModStorageWriteGateError> {
        let mut state = self.lock();
        freeze_error(*state)?;
        let outcome = write();
        if outcome.is_ok() {
            *state = ModStorageWriteFreeze::RestartRequired;
        }
        Ok(outcome)
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, ModStorageWriteFreeze> {
        // The state is a plain enum, so a panic while holding the lock cannot leave it torn.
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

fn freeze_error(freeze: ModStorageWriteFreeze) -> Result<(), ModStorageWriteGateError> {
    match freeze {
        ModStorageWriteFreeze::None => Ok(()),
        ModStorageWriteFreeze::Migration => Err(ModStorageWriteGateError::MigrationInProgress),
        ModStorageWriteFreeze::RestartRequired => Err(ModStorageWriteGateError::RestartRequired),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fresh_gate_admits_writers_and_reports_no_freeze() {
        let gate = ModStorageWriteGate::new();

        assert_eq!(gate.freeze(), ModStorageWriteFreeze::None);
        assert_eq!(gate.ensure_open(), Ok(()));
        assert_eq!(gate.admit(|| 7), Ok(7));
    }

    #[test]
    fn begin_migration_freezes_writers_until_the_outcome_is_known() {
        let gate = ModStorageWriteGate::new();

        gate.begin_migration(|| Ok::<(), ModStorageWriteGateError>(()))
            .expect("open gate admits a migration");

        assert_eq!(gate.freeze(), ModStorageWriteFreeze::Migration);
        assert_eq!(
            gate.ensure_open(),
            Err(ModStorageWriteGateError::MigrationInProgress)
        );
        let mut registered = false;
        assert_eq!(
            gate.admit(|| registered = true),
            Err(ModStorageWriteGateError::MigrationInProgress)
        );
        assert!(!registered, "a refused writer must not register anything");
        assert_eq!(
            gate.begin_migration(|| Ok::<(), ModStorageWriteGateError>(())),
            Err(ModStorageWriteGateError::MigrationInProgress)
        );

        gate.end_migration(false);
        assert_eq!(gate.freeze(), ModStorageWriteFreeze::None);
        assert_eq!(gate.ensure_open(), Ok(()));
    }

    #[test]
    fn a_switched_migration_keeps_writes_frozen_until_restart() {
        let gate = ModStorageWriteGate::new();
        gate.begin_migration(|| Ok::<(), ModStorageWriteGateError>(()))
            .expect("migration admitted");

        gate.end_migration(true);

        assert_eq!(gate.freeze(), ModStorageWriteFreeze::RestartRequired);
        assert_eq!(
            gate.ensure_open(),
            Err(ModStorageWriteGateError::RestartRequired)
        );
        assert_eq!(
            gate.begin_migration(|| Ok::<(), ModStorageWriteGateError>(())),
            Err(ModStorageWriteGateError::RestartRequired)
        );
        gate.end_migration(false);
        assert_eq!(
            gate.freeze(),
            ModStorageWriteFreeze::RestartRequired,
            "ending a migration never reopens a gate that is not in the migration state"
        );
    }

    #[derive(Debug, PartialEq, Eq)]
    enum ReadinessError {
        Gate(ModStorageWriteGateError),
        ImportsRunning,
    }

    impl From<ModStorageWriteGateError> for ReadinessError {
        fn from(error: ModStorageWriteGateError) -> Self {
            Self::Gate(error)
        }
    }

    #[test]
    fn a_failing_readiness_check_leaves_the_gate_open() {
        let gate = ModStorageWriteGate::new();

        let error = gate
            .begin_migration(|| Err::<(), ReadinessError>(ReadinessError::ImportsRunning))
            .expect_err("readiness failure propagates");

        assert_eq!(error, ReadinessError::ImportsRunning);
        assert_eq!(gate.freeze(), ModStorageWriteFreeze::None);
        assert_eq!(gate.ensure_open(), Ok(()));

        gate.begin_migration(|| Ok::<(), ReadinessError>(()))
            .expect("migration admitted");
        let mut readiness_ran = false;
        let error = gate
            .begin_migration(|| {
                readiness_ran = true;
                Ok::<(), ReadinessError>(())
            })
            .expect_err("frozen gate refuses before readiness runs");
        assert_eq!(
            error,
            ReadinessError::Gate(ModStorageWriteGateError::MigrationInProgress)
        );
        assert!(!readiness_ran);
    }

    #[test]
    fn a_successful_root_switch_freezes_writes_until_restart() {
        let gate = ModStorageWriteGate::new();

        let failed = gate
            .admit_root_switch(|| Err::<(), &str>("settings write failed"))
            .expect("open gate admits the switch");
        assert_eq!(failed, Err("settings write failed"));
        assert_eq!(
            gate.freeze(),
            ModStorageWriteFreeze::None,
            "a failed write leaves the gate open"
        );

        let switched = gate
            .admit_root_switch(|| Ok::<u8, &str>(1))
            .expect("open gate admits the switch");
        assert_eq!(switched, Ok(1));
        assert_eq!(gate.freeze(), ModStorageWriteFreeze::RestartRequired);
        assert_eq!(
            gate.ensure_open().map_err(|error| error.code()),
            Err("mod_storage_restart_required")
        );
        assert_eq!(
            gate.admit_root_switch(|| Ok::<u8, &str>(2)),
            Err(ModStorageWriteGateError::RestartRequired)
        );
        assert_eq!(
            ModStorageWriteGateError::MigrationInProgress.code(),
            "mod_storage_migration_in_progress"
        );
    }
}
