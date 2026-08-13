use super::{
    task_spec::{task_name_for_sid, ScheduledTaskSpec, ScheduledTaskSpecMatch, TASK_OWNER_MARKER},
    InstallerCleanupOutcome, ScheduledTaskCommand, ScheduledTaskCommandOutcome,
    ScheduledTaskCommandRunner,
};
use hmm_core::SaveBackupBackgroundRegistrationStatus;
use hmm_ports::{
    SaveBackupBackgroundRegistry, SaveBackupBackgroundRegistryError,
    SaveBackupBackgroundRegistryResult,
};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

struct RegistryInspection {
    status: SaveBackupBackgroundRegistrationStatus,
}

pub(super) struct ScheduledTaskRegistry<R> {
    runner: R,
    worker_path: Option<PathBuf>,
    current_user_sid: Mutex<Option<String>>,
}

impl<R: ScheduledTaskCommandRunner> ScheduledTaskRegistry<R> {
    #[cfg(test)]
    pub(super) fn new(runner: R, worker_path: PathBuf) -> Self {
        Self::with_worker_path(runner, Some(worker_path))
    }

    pub(super) fn with_worker_path(runner: R, worker_path: Option<PathBuf>) -> Self {
        Self {
            runner,
            worker_path,
            current_user_sid: Mutex::new(None),
        }
    }

    fn current_user_sid(&self) -> SaveBackupBackgroundRegistryResult<String> {
        let mut cached = self
            .current_user_sid
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(sid) = cached.as_ref() {
            return Ok(sid.clone());
        }

        match self.runner.run(ScheduledTaskCommand::Identity)? {
            ScheduledTaskCommandOutcome::Identity(sid) if !sid.is_empty() => {
                *cached = Some(sid.clone());
                Ok(sid)
            }
            _ => Err(SaveBackupBackgroundRegistryError::CommandInvalidOutput),
        }
    }

    fn inspect_internal(&self) -> SaveBackupBackgroundRegistryResult<RegistryInspection> {
        let worker_path = self
            .worker_path
            .as_deref()
            .ok_or(SaveBackupBackgroundRegistryError::WorkerBinaryUnavailable)?;
        let worker_path = canonical_worker_path(worker_path)?;
        let user_sid = self.current_user_sid()?;
        let spec = ScheduledTaskSpec::new(&user_sid, worker_path)
            .map_err(|_| SaveBackupBackgroundRegistryError::CommandInvalidOutput)?;
        let status = self.inspect_expected(&spec)?;
        Ok(RegistryInspection { status })
    }

    fn inspect_expected(
        &self,
        spec: &ScheduledTaskSpec,
    ) -> SaveBackupBackgroundRegistryResult<SaveBackupBackgroundRegistrationStatus> {
        let outcome = self.runner.run(ScheduledTaskCommand::Inspect {
            task_name: spec.task_name.clone(),
            owner_marker: spec.owner_marker.clone(),
        })?;
        match outcome {
            ScheduledTaskCommandOutcome::Missing => {
                Ok(SaveBackupBackgroundRegistrationStatus::NotRegistered)
            }
            ScheduledTaskCommandOutcome::PermissionRequired => {
                Ok(SaveBackupBackgroundRegistrationStatus::PermissionRequired)
            }
            ScheduledTaskCommandOutcome::ModuleUnavailable => {
                Ok(SaveBackupBackgroundRegistrationStatus::UnsupportedPlatform)
            }
            ScheduledTaskCommandOutcome::OwnershipConflict => {
                Err(SaveBackupBackgroundRegistryError::TaskOwnershipConflict)
            }
            ScheduledTaskCommandOutcome::Found(actual) => match spec.compare(&actual) {
                ScheduledTaskSpecMatch::Exact if readback_worker_is_exact(spec, &actual) => {
                    Ok(SaveBackupBackgroundRegistrationStatus::Registered)
                }
                ScheduledTaskSpecMatch::Exact | ScheduledTaskSpecMatch::OwnedDrift => {
                    Ok(SaveBackupBackgroundRegistrationStatus::ConfigurationDrift)
                }
                ScheduledTaskSpecMatch::OwnershipConflict => {
                    Err(SaveBackupBackgroundRegistryError::TaskOwnershipConflict)
                }
            },
            ScheduledTaskCommandOutcome::Identity(_)
            | ScheduledTaskCommandOutcome::Completed
            | ScheduledTaskCommandOutcome::PostDeleteOwned
            | ScheduledTaskCommandOutcome::PostDeleteForeign => {
                Err(SaveBackupBackgroundRegistryError::CommandInvalidOutput)
            }
            ScheduledTaskCommandOutcome::TaskBusy
            | ScheduledTaskCommandOutcome::StateUnverified => {
                Err(SaveBackupBackgroundRegistryError::CommandInvalidOutput)
            }
        }
    }

    fn exact_registration_status(
        &self,
        spec: &ScheduledTaskSpec,
        outcome: ScheduledTaskCommandOutcome,
    ) -> SaveBackupBackgroundRegistryResult<SaveBackupBackgroundRegistrationStatus> {
        match outcome {
            ScheduledTaskCommandOutcome::Found(actual) => match spec.compare(&actual) {
                ScheduledTaskSpecMatch::Exact if readback_worker_is_exact(spec, &actual) => {
                    Ok(SaveBackupBackgroundRegistrationStatus::Registered)
                }
                ScheduledTaskSpecMatch::Exact | ScheduledTaskSpecMatch::OwnedDrift => {
                    Ok(SaveBackupBackgroundRegistrationStatus::ConfigurationDrift)
                }
                ScheduledTaskSpecMatch::OwnershipConflict => {
                    Err(SaveBackupBackgroundRegistryError::TaskOwnershipConflict)
                }
            },
            ScheduledTaskCommandOutcome::PermissionRequired => {
                Ok(SaveBackupBackgroundRegistrationStatus::PermissionRequired)
            }
            ScheduledTaskCommandOutcome::ModuleUnavailable => {
                Ok(SaveBackupBackgroundRegistrationStatus::UnsupportedPlatform)
            }
            ScheduledTaskCommandOutcome::OwnershipConflict => {
                Err(SaveBackupBackgroundRegistryError::TaskOwnershipConflict)
            }
            ScheduledTaskCommandOutcome::Identity(_)
            | ScheduledTaskCommandOutcome::Missing
            | ScheduledTaskCommandOutcome::Completed
            | ScheduledTaskCommandOutcome::PostDeleteOwned
            | ScheduledTaskCommandOutcome::PostDeleteForeign
            | ScheduledTaskCommandOutcome::TaskBusy
            | ScheduledTaskCommandOutcome::StateUnverified => {
                Err(SaveBackupBackgroundRegistryError::CommandInvalidOutput)
            }
        }
    }

    pub(super) fn cleanup_for_installer(&self) -> InstallerCleanupOutcome {
        let user_sid = match self.runner.run(ScheduledTaskCommand::Identity) {
            Ok(ScheduledTaskCommandOutcome::Identity(sid)) if !sid.is_empty() => sid,
            Ok(ScheduledTaskCommandOutcome::ModuleUnavailable) => {
                return InstallerCleanupOutcome::PlatformUnavailable;
            }
            Ok(_) | Err(_) => return InstallerCleanupOutcome::OwnershipUnverified,
        };
        let task_name = match task_name_for_sid(&user_sid) {
            Ok(task_name) => task_name,
            Err(_) => return InstallerCleanupOutcome::OwnershipUnverified,
        };
        let owner_marker = TASK_OWNER_MARKER.to_owned();

        match self.runner.run(ScheduledTaskCommand::InstallerCleanup {
            task_name,
            owner_marker,
        }) {
            Ok(ScheduledTaskCommandOutcome::Completed) => InstallerCleanupOutcome::Removed,
            Ok(ScheduledTaskCommandOutcome::Missing) => InstallerCleanupOutcome::AlreadyAbsent,
            Ok(ScheduledTaskCommandOutcome::OwnershipConflict) => {
                InstallerCleanupOutcome::ForeignPreserved
            }
            Ok(ScheduledTaskCommandOutcome::TaskBusy) => InstallerCleanupOutcome::OwnedTaskRunning,
            Ok(ScheduledTaskCommandOutcome::ModuleUnavailable) => {
                InstallerCleanupOutcome::PlatformUnavailable
            }
            Ok(ScheduledTaskCommandOutcome::PermissionRequired)
            | Ok(ScheduledTaskCommandOutcome::StateUnverified)
            | Ok(ScheduledTaskCommandOutcome::PostDeleteForeign) => {
                InstallerCleanupOutcome::OwnershipUnverified
            }
            Ok(ScheduledTaskCommandOutcome::PostDeleteOwned) => {
                InstallerCleanupOutcome::RemovalUnverified
            }
            Ok(ScheduledTaskCommandOutcome::Identity(_))
            | Ok(ScheduledTaskCommandOutcome::Found(_))
            | Err(_) => InstallerCleanupOutcome::RemovalUnverified,
        }
    }
}

impl<R: ScheduledTaskCommandRunner> SaveBackupBackgroundRegistry for ScheduledTaskRegistry<R> {
    fn inspect(
        &self,
    ) -> SaveBackupBackgroundRegistryResult<SaveBackupBackgroundRegistrationStatus> {
        Ok(self.inspect_internal()?.status)
    }

    fn register(
        &self,
    ) -> SaveBackupBackgroundRegistryResult<SaveBackupBackgroundRegistrationStatus> {
        let worker_path = self
            .worker_path
            .as_deref()
            .ok_or(SaveBackupBackgroundRegistryError::WorkerBinaryUnavailable)?;
        let worker_path = canonical_worker_path(worker_path)?;
        let user_sid = self.current_user_sid()?;
        let spec = ScheduledTaskSpec::new(&user_sid, worker_path)
            .map_err(|_| SaveBackupBackgroundRegistryError::CommandInvalidOutput)?;

        let registration_status = self.exact_registration_status(
            &spec,
            self.runner
                .run(ScheduledTaskCommand::Register(spec.clone()))?,
        )?;
        if registration_status != SaveBackupBackgroundRegistrationStatus::Registered {
            return Ok(registration_status);
        }

        self.exact_registration_status(
            &spec,
            self.runner.run(ScheduledTaskCommand::Start(spec.clone()))?,
        )
    }

    fn unregister(
        &self,
    ) -> SaveBackupBackgroundRegistryResult<SaveBackupBackgroundRegistrationStatus> {
        let user_sid = self.current_user_sid()?;
        let task_name = task_name_for_sid(&user_sid)
            .map_err(|_| SaveBackupBackgroundRegistryError::CommandInvalidOutput)?;
        match self.runner.run(ScheduledTaskCommand::Unregister {
            task_name,
            owner_marker: TASK_OWNER_MARKER.to_owned(),
        })? {
            ScheduledTaskCommandOutcome::Missing | ScheduledTaskCommandOutcome::Completed => {
                Ok(SaveBackupBackgroundRegistrationStatus::NotRegistered)
            }
            ScheduledTaskCommandOutcome::PermissionRequired => {
                Ok(SaveBackupBackgroundRegistrationStatus::PermissionRequired)
            }
            ScheduledTaskCommandOutcome::ModuleUnavailable => {
                Ok(SaveBackupBackgroundRegistrationStatus::UnsupportedPlatform)
            }
            ScheduledTaskCommandOutcome::OwnershipConflict => {
                Err(SaveBackupBackgroundRegistryError::TaskOwnershipConflict)
            }
            ScheduledTaskCommandOutcome::PostDeleteOwned => {
                Ok(SaveBackupBackgroundRegistrationStatus::RegistrationFailed)
            }
            ScheduledTaskCommandOutcome::PostDeleteForeign => {
                Err(SaveBackupBackgroundRegistryError::TaskOwnershipConflict)
            }
            ScheduledTaskCommandOutcome::Identity(_)
            | ScheduledTaskCommandOutcome::Found(_)
            | ScheduledTaskCommandOutcome::TaskBusy
            | ScheduledTaskCommandOutcome::StateUnverified => {
                Err(SaveBackupBackgroundRegistryError::CommandInvalidOutput)
            }
        }
    }
}

fn canonical_worker_path(worker_path: &Path) -> SaveBackupBackgroundRegistryResult<PathBuf> {
    if !worker_path.is_absolute() {
        return Err(SaveBackupBackgroundRegistryError::WorkerBinaryUnavailable);
    }
    let parent = worker_path
        .parent()
        .ok_or(SaveBackupBackgroundRegistryError::WorkerBinaryUnavailable)?;
    let canonical_parent = std::fs::canonicalize(parent)
        .map_err(|_| SaveBackupBackgroundRegistryError::WorkerBinaryUnavailable)?;
    let metadata = std::fs::symlink_metadata(worker_path)
        .map_err(|_| SaveBackupBackgroundRegistryError::WorkerBinaryUnavailable)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(SaveBackupBackgroundRegistryError::WorkerBinaryUnavailable);
    }
    let canonical_worker = std::fs::canonicalize(worker_path)
        .map_err(|_| SaveBackupBackgroundRegistryError::WorkerBinaryUnavailable)?;
    if canonical_worker.parent() != Some(canonical_parent.as_path()) {
        return Err(SaveBackupBackgroundRegistryError::WorkerBinaryUnavailable);
    }
    Ok(canonical_worker)
}

fn readback_worker_is_exact(
    spec: &ScheduledTaskSpec,
    actual: &super::task_spec::ScheduledTaskReadback,
) -> bool {
    if actual.action_execute != spec.worker_path {
        return false;
    }
    let Ok(metadata) = std::fs::symlink_metadata(&actual.action_execute) else {
        return false;
    };
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return false;
    }
    std::fs::canonicalize(&actual.action_execute)
        .map(|path| path == spec.worker_path)
        .unwrap_or(false)
}
