#[cfg(any(windows, test))]
mod task_spec;

#[cfg(windows)]
mod powershell;

#[cfg(any(windows, test))]
mod registry;

#[cfg(test)]
mod tests;

#[cfg(windows)]
mod windows;

use hmm_core::SaveBackupBackgroundRegistrationStatus;
use hmm_ports::{SaveBackupBackgroundRegistry, SaveBackupBackgroundRegistryResult};

#[cfg(any(windows, test))]
use task_spec::{ScheduledTaskReadback, ScheduledTaskSpec};

#[cfg(any(windows, test))]
#[derive(Debug, Clone, PartialEq, Eq)]
enum ScheduledTaskCommand {
    Identity,
    Inspect {
        task_name: String,
        owner_marker: String,
    },
    Register(ScheduledTaskSpec),
    Unregister {
        task_name: String,
        owner_marker: String,
    },
}

#[cfg(any(windows, test))]
#[derive(Debug, Clone, PartialEq, Eq)]
enum ScheduledTaskCommandOutcome {
    Identity(String),
    Missing,
    Found(Box<ScheduledTaskReadback>),
    Completed,
    PermissionRequired,
    ModuleUnavailable,
    OwnershipConflict,
}

#[cfg(any(windows, test))]
trait ScheduledTaskCommandRunner: Send + Sync {
    fn run(
        &self,
        command: ScheduledTaskCommand,
    ) -> SaveBackupBackgroundRegistryResult<ScheduledTaskCommandOutcome>;
}

#[cfg(windows)]
pub use windows::WindowsScheduledTaskRegistry;

#[derive(Debug, Default)]
pub struct UnsupportedSaveBackupBackgroundRegistry;

impl SaveBackupBackgroundRegistry for UnsupportedSaveBackupBackgroundRegistry {
    fn inspect(
        &self,
    ) -> SaveBackupBackgroundRegistryResult<SaveBackupBackgroundRegistrationStatus> {
        Ok(SaveBackupBackgroundRegistrationStatus::UnsupportedPlatform)
    }

    fn register(
        &self,
    ) -> SaveBackupBackgroundRegistryResult<SaveBackupBackgroundRegistrationStatus> {
        Ok(SaveBackupBackgroundRegistrationStatus::UnsupportedPlatform)
    }

    fn unregister(
        &self,
    ) -> SaveBackupBackgroundRegistryResult<SaveBackupBackgroundRegistrationStatus> {
        Ok(SaveBackupBackgroundRegistrationStatus::UnsupportedPlatform)
    }
}
