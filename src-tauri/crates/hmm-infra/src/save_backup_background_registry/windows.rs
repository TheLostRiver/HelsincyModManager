use super::{
    native_inspect::{current_user_identity, inspect_scheduled_task},
    powershell::PowerShellScheduledTaskCommandRunner,
    registry::ScheduledTaskRegistry,
    ScheduledTaskCommand, ScheduledTaskCommandOutcome, ScheduledTaskCommandRunner,
};
use hmm_core::SaveBackupBackgroundRegistrationStatus;
use hmm_ports::{SaveBackupBackgroundRegistry, SaveBackupBackgroundRegistryResult};

#[derive(Debug, Clone, Copy, Default)]
struct WindowsScheduledTaskCommandRunner;

impl ScheduledTaskCommandRunner for WindowsScheduledTaskCommandRunner {
    fn run(
        &self,
        command: ScheduledTaskCommand,
    ) -> SaveBackupBackgroundRegistryResult<ScheduledTaskCommandOutcome> {
        run_windows_command(
            command,
            current_user_identity,
            inspect_scheduled_task,
            |command| PowerShellScheduledTaskCommandRunner.run(command),
        )
    }
}

fn run_windows_command(
    command: ScheduledTaskCommand,
    identity: impl FnOnce() -> SaveBackupBackgroundRegistryResult<ScheduledTaskCommandOutcome>,
    inspect: impl FnOnce(&str, &str) -> SaveBackupBackgroundRegistryResult<ScheduledTaskCommandOutcome>,
    powershell: impl FnOnce(
        ScheduledTaskCommand,
    ) -> SaveBackupBackgroundRegistryResult<ScheduledTaskCommandOutcome>,
) -> SaveBackupBackgroundRegistryResult<ScheduledTaskCommandOutcome> {
    match command {
        ScheduledTaskCommand::Identity => identity(),
        ScheduledTaskCommand::Inspect {
            task_name,
            owner_marker,
        } => inspect(&task_name, &owner_marker),
        command => powershell(command),
    }
}

pub struct WindowsScheduledTaskRegistry {
    inner: ScheduledTaskRegistry<WindowsScheduledTaskCommandRunner>,
}

impl WindowsScheduledTaskRegistry {
    pub fn from_current_exe() -> Self {
        let worker_path = std::env::current_exe()
            .ok()
            .and_then(|current_exe| current_exe.parent().map(|parent| parent.to_path_buf()))
            .map(|parent| parent.join("hmm-save-backup-worker.exe"));
        Self {
            inner: ScheduledTaskRegistry::with_worker_path(
                WindowsScheduledTaskCommandRunner,
                worker_path,
            ),
        }
    }
}

impl SaveBackupBackgroundRegistry for WindowsScheduledTaskRegistry {
    fn inspect(
        &self,
    ) -> SaveBackupBackgroundRegistryResult<SaveBackupBackgroundRegistrationStatus> {
        self.inner.inspect()
    }

    fn register(
        &self,
    ) -> SaveBackupBackgroundRegistryResult<SaveBackupBackgroundRegistrationStatus> {
        self.inner.register()
    }

    fn unregister(
        &self,
    ) -> SaveBackupBackgroundRegistryResult<SaveBackupBackgroundRegistrationStatus> {
        self.inner.unregister()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::save_backup_background_registry::task_spec::TASK_OWNER_MARKER;
    use hmm_ports::SaveBackupBackgroundRegistryError;
    use std::cell::Cell;

    #[test]
    fn inspect_uses_native_reader_without_starting_powershell() {
        let identity_called = Cell::new(false);
        let native_called = Cell::new(false);
        let powershell_called = Cell::new(false);
        let outcome = run_windows_command(
            ScheduledTaskCommand::Inspect {
                task_name: "HelsincyModManager.SaveBackup.test".to_owned(),
                owner_marker: TASK_OWNER_MARKER.to_owned(),
            },
            || {
                identity_called.set(true);
                Err(SaveBackupBackgroundRegistryError::OperationFailed)
            },
            |task_name, owner_marker| {
                native_called.set(true);
                assert_eq!(task_name, "HelsincyModManager.SaveBackup.test");
                assert_eq!(owner_marker, TASK_OWNER_MARKER);
                Ok(ScheduledTaskCommandOutcome::Missing)
            },
            |_| {
                powershell_called.set(true);
                Err(SaveBackupBackgroundRegistryError::OperationFailed)
            },
        )
        .expect("native inspect outcome");

        assert_eq!(outcome, ScheduledTaskCommandOutcome::Missing);
        assert!(!identity_called.get());
        assert!(native_called.get());
        assert!(!powershell_called.get());
    }

    #[test]
    fn identity_uses_native_reader_without_starting_powershell() {
        let inspect_called = Cell::new(false);
        let powershell_called = Cell::new(false);
        let outcome = run_windows_command(
            ScheduledTaskCommand::Identity,
            || {
                Ok(ScheduledTaskCommandOutcome::Identity(
                    "S-1-5-21-9".to_owned(),
                ))
            },
            |_, _| {
                inspect_called.set(true);
                Err(SaveBackupBackgroundRegistryError::OperationFailed)
            },
            |_| {
                powershell_called.set(true);
                Err(SaveBackupBackgroundRegistryError::OperationFailed)
            },
        )
        .expect("native identity outcome");

        assert_eq!(
            outcome,
            ScheduledTaskCommandOutcome::Identity("S-1-5-21-9".to_owned())
        );
        assert!(!inspect_called.get());
        assert!(!powershell_called.get());
    }

    #[test]
    fn mutations_still_use_existing_powershell_runner() {
        let identity_called = Cell::new(false);
        let native_called = Cell::new(false);
        let powershell_called = Cell::new(false);
        let outcome = run_windows_command(
            ScheduledTaskCommand::Unregister {
                task_name: "HelsincyModManager.SaveBackup.test".to_owned(),
                owner_marker: TASK_OWNER_MARKER.to_owned(),
            },
            || {
                identity_called.set(true);
                Err(SaveBackupBackgroundRegistryError::OperationFailed)
            },
            |_, _| {
                native_called.set(true);
                Err(SaveBackupBackgroundRegistryError::OperationFailed)
            },
            |command| {
                powershell_called.set(true);
                assert!(matches!(command, ScheduledTaskCommand::Unregister { .. }));
                Ok(ScheduledTaskCommandOutcome::Completed)
            },
        )
        .expect("powershell mutation outcome");

        assert_eq!(outcome, ScheduledTaskCommandOutcome::Completed);
        assert!(!identity_called.get());
        assert!(!native_called.get());
        assert!(powershell_called.get());
    }
}
