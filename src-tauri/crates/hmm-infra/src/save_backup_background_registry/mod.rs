#[cfg_attr(not(test), allow(dead_code))]
mod task_spec;

#[cfg(windows)]
#[cfg_attr(not(test), allow(dead_code))]
mod powershell;

#[cfg(test)]
mod tests;

use hmm_core::SaveBackupBackgroundRegistrationStatus;
use hmm_ports::{SaveBackupBackgroundRegistry, SaveBackupBackgroundRegistryResult};

use task_spec::{ScheduledTaskReadback, ScheduledTaskSpec};

#[cfg_attr(not(test), allow(dead_code))]
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

#[cfg_attr(not(test), allow(dead_code))]
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

#[cfg_attr(not(test), allow(dead_code))]
trait ScheduledTaskCommandRunner: Send + Sync {
    fn run(
        &self,
        command: ScheduledTaskCommand,
    ) -> SaveBackupBackgroundRegistryResult<ScheduledTaskCommandOutcome>;
}

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
