use anyhow::Result;
use hmm_core::SaveBackupBackgroundRegistrationStatus;
use hmm_ports::SaveBackupBackgroundRegistry;

#[derive(Debug, Default)]
pub struct UnsupportedSaveBackupBackgroundRegistry;

impl SaveBackupBackgroundRegistry for UnsupportedSaveBackupBackgroundRegistry {
    fn inspect(&self) -> Result<SaveBackupBackgroundRegistrationStatus> {
        Ok(SaveBackupBackgroundRegistrationStatus::UnsupportedPlatform)
    }

    fn register(&self) -> Result<SaveBackupBackgroundRegistrationStatus> {
        Ok(SaveBackupBackgroundRegistrationStatus::UnsupportedPlatform)
    }

    fn unregister(&self) -> Result<SaveBackupBackgroundRegistrationStatus> {
        Ok(SaveBackupBackgroundRegistrationStatus::UnsupportedPlatform)
    }
}
