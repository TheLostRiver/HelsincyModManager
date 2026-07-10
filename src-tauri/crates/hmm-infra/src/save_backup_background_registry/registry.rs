use super::{
    task_spec::{task_name_for_sid, ScheduledTaskSpec, ScheduledTaskSpecMatch, TASK_OWNER_MARKER},
    ScheduledTaskCommand, ScheduledTaskCommandOutcome, ScheduledTaskCommandRunner,
};
use hmm_core::SaveBackupBackgroundRegistrationStatus;
use hmm_ports::{
    SaveBackupBackgroundRegistry, SaveBackupBackgroundRegistryError,
    SaveBackupBackgroundRegistryResult,
};
use std::path::{Path, PathBuf};

struct RegistryInspection {
    status: SaveBackupBackgroundRegistrationStatus,
    spec: ScheduledTaskSpec,
}

pub(super) struct ScheduledTaskRegistry<R> {
    runner: R,
    worker_path: Option<PathBuf>,
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
        }
    }

    fn current_user_sid(&self) -> SaveBackupBackgroundRegistryResult<String> {
        match self.runner.run(ScheduledTaskCommand::Identity)? {
            ScheduledTaskCommandOutcome::Identity(sid) if !sid.is_empty() => Ok(sid),
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
        Ok(RegistryInspection { status, spec })
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
            ScheduledTaskCommandOutcome::Identity(_) | ScheduledTaskCommandOutcome::Completed => {
                Err(SaveBackupBackgroundRegistryError::CommandInvalidOutput)
            }
        }
    }

    fn inspect_owned_raw(
        &self,
    ) -> SaveBackupBackgroundRegistryResult<(String, ScheduledTaskCommandOutcome)> {
        let user_sid = self.current_user_sid()?;
        let task_name = task_name_for_sid(&user_sid)
            .map_err(|_| SaveBackupBackgroundRegistryError::CommandInvalidOutput)?;
        let outcome = self.runner.run(ScheduledTaskCommand::Inspect {
            task_name: task_name.clone(),
            owner_marker: TASK_OWNER_MARKER.to_owned(),
        })?;
        Ok((task_name, outcome))
    }

    fn post_delete_status(
        &self,
        task_name: String,
    ) -> SaveBackupBackgroundRegistryResult<SaveBackupBackgroundRegistrationStatus> {
        match self.runner.run(ScheduledTaskCommand::Inspect {
            task_name,
            owner_marker: TASK_OWNER_MARKER.to_owned(),
        })? {
            ScheduledTaskCommandOutcome::Missing => {
                Ok(SaveBackupBackgroundRegistrationStatus::NotRegistered)
            }
            ScheduledTaskCommandOutcome::PermissionRequired => {
                Ok(SaveBackupBackgroundRegistrationStatus::PermissionRequired)
            }
            ScheduledTaskCommandOutcome::ModuleUnavailable => {
                Ok(SaveBackupBackgroundRegistrationStatus::UnsupportedPlatform)
            }
            ScheduledTaskCommandOutcome::Found(actual) => {
                if actual.owner_marker == TASK_OWNER_MARKER {
                    Ok(SaveBackupBackgroundRegistrationStatus::RegistrationFailed)
                } else {
                    Err(SaveBackupBackgroundRegistryError::TaskOwnershipConflict)
                }
            }
            ScheduledTaskCommandOutcome::Identity(_)
            | ScheduledTaskCommandOutcome::Completed
            | ScheduledTaskCommandOutcome::OwnershipConflict => {
                Err(SaveBackupBackgroundRegistryError::CommandInvalidOutput)
            }
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
        let before = self.inspect_internal()?;
        match before.status {
            SaveBackupBackgroundRegistrationStatus::Registered => {
                return Ok(SaveBackupBackgroundRegistrationStatus::Registered);
            }
            SaveBackupBackgroundRegistrationStatus::NotRegistered
            | SaveBackupBackgroundRegistrationStatus::ConfigurationDrift => {}
            SaveBackupBackgroundRegistrationStatus::PermissionRequired
            | SaveBackupBackgroundRegistrationStatus::UnsupportedPlatform
            | SaveBackupBackgroundRegistrationStatus::RegistrationFailed => {
                return Ok(before.status);
            }
        }

        match self
            .runner
            .run(ScheduledTaskCommand::Register(before.spec.clone()))?
        {
            ScheduledTaskCommandOutcome::Completed => {}
            ScheduledTaskCommandOutcome::PermissionRequired => {
                return Ok(SaveBackupBackgroundRegistrationStatus::PermissionRequired);
            }
            ScheduledTaskCommandOutcome::ModuleUnavailable => {
                return Ok(SaveBackupBackgroundRegistrationStatus::UnsupportedPlatform);
            }
            ScheduledTaskCommandOutcome::OwnershipConflict => {
                return Err(SaveBackupBackgroundRegistryError::TaskOwnershipConflict);
            }
            _ => return Err(SaveBackupBackgroundRegistryError::CommandInvalidOutput),
        }

        self.inspect_expected(&before.spec)
    }

    fn unregister(
        &self,
    ) -> SaveBackupBackgroundRegistryResult<SaveBackupBackgroundRegistrationStatus> {
        let (task_name, before) = self.inspect_owned_raw()?;
        match before {
            ScheduledTaskCommandOutcome::Missing => {
                return Ok(SaveBackupBackgroundRegistrationStatus::NotRegistered);
            }
            ScheduledTaskCommandOutcome::PermissionRequired => {
                return Ok(SaveBackupBackgroundRegistrationStatus::PermissionRequired);
            }
            ScheduledTaskCommandOutcome::ModuleUnavailable => {
                return Ok(SaveBackupBackgroundRegistrationStatus::UnsupportedPlatform);
            }
            ScheduledTaskCommandOutcome::OwnershipConflict => {
                return Err(SaveBackupBackgroundRegistryError::TaskOwnershipConflict);
            }
            ScheduledTaskCommandOutcome::Found(actual) => {
                if actual.owner_marker != TASK_OWNER_MARKER {
                    return Err(SaveBackupBackgroundRegistryError::TaskOwnershipConflict);
                }
            }
            ScheduledTaskCommandOutcome::Identity(_) | ScheduledTaskCommandOutcome::Completed => {
                return Err(SaveBackupBackgroundRegistryError::CommandInvalidOutput);
            }
        }

        match self.runner.run(ScheduledTaskCommand::Unregister {
            task_name: task_name.clone(),
            owner_marker: TASK_OWNER_MARKER.to_owned(),
        })? {
            ScheduledTaskCommandOutcome::Completed => {}
            ScheduledTaskCommandOutcome::PermissionRequired => {
                return Ok(SaveBackupBackgroundRegistrationStatus::PermissionRequired);
            }
            ScheduledTaskCommandOutcome::ModuleUnavailable => {
                return Ok(SaveBackupBackgroundRegistrationStatus::UnsupportedPlatform);
            }
            ScheduledTaskCommandOutcome::OwnershipConflict => {
                return Err(SaveBackupBackgroundRegistryError::TaskOwnershipConflict);
            }
            _ => return Err(SaveBackupBackgroundRegistryError::CommandInvalidOutput),
        }

        self.post_delete_status(task_name)
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
