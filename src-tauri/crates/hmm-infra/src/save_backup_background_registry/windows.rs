use super::{powershell::PowerShellScheduledTaskCommandRunner, registry::ScheduledTaskRegistry};
use hmm_core::SaveBackupBackgroundRegistrationStatus;
use hmm_ports::{SaveBackupBackgroundRegistry, SaveBackupBackgroundRegistryResult};

pub struct WindowsScheduledTaskRegistry {
    inner: ScheduledTaskRegistry<PowerShellScheduledTaskCommandRunner>,
}

impl WindowsScheduledTaskRegistry {
    pub fn from_current_exe() -> Self {
        let worker_path = std::env::current_exe()
            .ok()
            .and_then(|current_exe| current_exe.parent().map(|parent| parent.to_path_buf()))
            .map(|parent| parent.join("hmm-save-backup-worker.exe"));
        Self {
            inner: ScheduledTaskRegistry::with_worker_path(
                PowerShellScheduledTaskCommandRunner,
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
