#[cfg_attr(not(test), allow(dead_code))]
mod task_spec;

#[cfg(test)]
mod tests;

use hmm_core::SaveBackupBackgroundRegistrationStatus;
use hmm_ports::{SaveBackupBackgroundRegistry, SaveBackupBackgroundRegistryResult};

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
