use super::{
    task_spec::ScheduledTaskReadback, ScheduledTaskCommand, ScheduledTaskCommandOutcome,
    ScheduledTaskCommandRunner,
};
use hmm_ports::{
    SaveBackupBackgroundRegistryError, SaveBackupBackgroundRegistryResult,
    SAVE_BACKUP_BACKGROUND_REGISTRY_SCHEMA_VERSION,
};
use serde::Deserialize;
use std::ffi::OsString;
use std::io::Read;
use std::os::windows::ffi::OsStringExt;
use std::os::windows::process::CommandExt;
use std::path::PathBuf;
use std::process::{Child, ChildStdout, Command, ExitStatus, Stdio};
use std::time::Duration;
use wait_timeout::ChildExt;
use windows_sys::Win32::System::SystemInformation::GetSystemDirectoryW;

pub(super) const COMMAND_TIMEOUT: Duration = Duration::from_secs(15);
pub(super) const MAX_OUTPUT_BYTES: usize = 64 * 1024;
pub(super) const SCRIPT: &str = include_str!("scheduled_task.ps1");

const CREATE_NO_WINDOW: u32 = 0x0800_0000;
const INTERNAL_ENV_KEYS: [&str; 6] = [
    "HMM_OPERATION",
    "HMM_SCHEDULED_TASKS_MODULE",
    "HMM_TASK_NAME",
    "HMM_OWNER_MARKER",
    "HMM_WORKER_PATH",
    "HMM_USER_SID",
];

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ScriptEnvelope {
    schema_version: u32,
    status: String,
    current_user_sid: Option<String>,
    task: Option<ScheduledTaskReadback>,
}

#[derive(Debug)]
pub(super) struct SystemPowerShellRuntime {
    pub(super) executable: PathBuf,
    pub(super) scheduled_tasks_module: PathBuf,
}

pub(super) fn system_powershell_runtime(
) -> SaveBackupBackgroundRegistryResult<SystemPowerShellRuntime> {
    let mut buffer = vec![0_u16; 32_768];
    let length = unsafe { GetSystemDirectoryW(buffer.as_mut_ptr(), buffer.len() as u32) };
    if length == 0 || length as usize >= buffer.len() {
        return Err(SaveBackupBackgroundRegistryError::OperationFailed);
    }

    buffer.truncate(length as usize);
    let powershell_root = PathBuf::from(OsString::from_wide(&buffer))
        .join("WindowsPowerShell")
        .join("v1.0");
    let executable = powershell_root.join("powershell.exe");
    let scheduled_tasks_module = powershell_root
        .join("Modules")
        .join("ScheduledTasks")
        .join("ScheduledTasks.psd1");
    if !executable.is_absolute() || !executable.is_file() || !scheduled_tasks_module.is_absolute() {
        return Err(SaveBackupBackgroundRegistryError::OperationFailed);
    }

    Ok(SystemPowerShellRuntime {
        executable,
        scheduled_tasks_module,
    })
}

pub(super) fn module_preflight_outcome(
    request: &ScheduledTaskCommand,
    runtime: &SystemPowerShellRuntime,
) -> Option<ScheduledTaskCommandOutcome> {
    if !matches!(request, ScheduledTaskCommand::Identity)
        && !runtime.scheduled_tasks_module.is_file()
    {
        Some(ScheduledTaskCommandOutcome::ModuleUnavailable)
    } else {
        None
    }
}

fn build_command_with_runtime(
    request: &ScheduledTaskCommand,
    runtime: &SystemPowerShellRuntime,
) -> Command {
    let mut command = Command::new(&runtime.executable);
    command.args([
        "-NoLogo",
        "-NoProfile",
        "-NonInteractive",
        "-Command",
        SCRIPT,
    ]);
    for key in INTERNAL_ENV_KEYS {
        command.env_remove(key);
    }

    match request {
        ScheduledTaskCommand::Identity => {
            command.env("HMM_OPERATION", "identity");
        }
        ScheduledTaskCommand::Inspect {
            task_name,
            owner_marker,
        } => {
            command
                .env("HMM_OPERATION", "inspect")
                .env(
                    "HMM_SCHEDULED_TASKS_MODULE",
                    &runtime.scheduled_tasks_module,
                )
                .env("HMM_TASK_NAME", task_name)
                .env("HMM_OWNER_MARKER", owner_marker);
        }
        ScheduledTaskCommand::Register(spec) => {
            command
                .env("HMM_OPERATION", "register")
                .env(
                    "HMM_SCHEDULED_TASKS_MODULE",
                    &runtime.scheduled_tasks_module,
                )
                .env("HMM_TASK_NAME", &spec.task_name)
                .env("HMM_OWNER_MARKER", &spec.owner_marker)
                .env("HMM_WORKER_PATH", &spec.worker_path)
                .env("HMM_USER_SID", &spec.user_sid);
        }
        ScheduledTaskCommand::Start(spec) => {
            command
                .env("HMM_OPERATION", "start")
                .env(
                    "HMM_SCHEDULED_TASKS_MODULE",
                    &runtime.scheduled_tasks_module,
                )
                .env("HMM_TASK_NAME", &spec.task_name)
                .env("HMM_OWNER_MARKER", &spec.owner_marker)
                .env("HMM_WORKER_PATH", &spec.worker_path)
                .env("HMM_USER_SID", &spec.user_sid);
        }
        ScheduledTaskCommand::Unregister {
            task_name,
            owner_marker,
        } => {
            command
                .env("HMM_OPERATION", "unregister")
                .env(
                    "HMM_SCHEDULED_TASKS_MODULE",
                    &runtime.scheduled_tasks_module,
                )
                .env("HMM_TASK_NAME", task_name)
                .env("HMM_OWNER_MARKER", owner_marker);
        }
        ScheduledTaskCommand::InstallerCleanup {
            task_name,
            owner_marker,
        } => {
            command
                .env("HMM_OPERATION", "installer_cleanup")
                .env(
                    "HMM_SCHEDULED_TASKS_MODULE",
                    &runtime.scheduled_tasks_module,
                )
                .env("HMM_TASK_NAME", task_name)
                .env("HMM_OWNER_MARKER", owner_marker);
        }
    }

    command.stdout(Stdio::piped()).stderr(Stdio::null());
    command.creation_flags(CREATE_NO_WINDOW);
    command
}

#[cfg(test)]
pub(super) fn build_command(
    request: &ScheduledTaskCommand,
) -> SaveBackupBackgroundRegistryResult<Command> {
    let runtime = system_powershell_runtime()?;
    Ok(build_command_with_runtime(request, &runtime))
}

pub(super) fn parse_script_output(
    output: &[u8],
) -> SaveBackupBackgroundRegistryResult<ScheduledTaskCommandOutcome> {
    if output.len() > MAX_OUTPUT_BYTES {
        return Err(SaveBackupBackgroundRegistryError::CommandInvalidOutput);
    }

    let envelope: ScriptEnvelope = serde_json::from_slice(output)
        .map_err(|_| SaveBackupBackgroundRegistryError::CommandInvalidOutput)?;
    if envelope.schema_version != SAVE_BACKUP_BACKGROUND_REGISTRY_SCHEMA_VERSION {
        return Err(SaveBackupBackgroundRegistryError::CommandInvalidOutput);
    }

    match (
        envelope.status.as_str(),
        envelope.current_user_sid,
        envelope.task,
    ) {
        ("identity", Some(sid), None) if !sid.is_empty() => {
            Ok(ScheduledTaskCommandOutcome::Identity(sid))
        }
        ("not_found", None, None) => Ok(ScheduledTaskCommandOutcome::Missing),
        ("found", None, Some(task)) => Ok(ScheduledTaskCommandOutcome::Found(Box::new(task))),
        ("completed", None, None) => Ok(ScheduledTaskCommandOutcome::Completed),
        ("post_delete_owned", None, None) => Ok(ScheduledTaskCommandOutcome::PostDeleteOwned),
        ("post_delete_foreign", None, None) => Ok(ScheduledTaskCommandOutcome::PostDeleteForeign),
        ("permission_required", None, None) => Ok(ScheduledTaskCommandOutcome::PermissionRequired),
        ("module_unavailable", None, None) => Ok(ScheduledTaskCommandOutcome::ModuleUnavailable),
        ("ownership_conflict", None, None) => Ok(ScheduledTaskCommandOutcome::OwnershipConflict),
        ("task_busy", None, None) => Ok(ScheduledTaskCommandOutcome::TaskBusy),
        ("state_unverified", None, None) => Ok(ScheduledTaskCommandOutcome::StateUnverified),
        ("operation_failed", None, None) => Err(SaveBackupBackgroundRegistryError::OperationFailed),
        _ => Err(SaveBackupBackgroundRegistryError::CommandInvalidOutput),
    }
}

fn drain_stdout(mut stdout: ChildStdout) -> std::io::Result<Vec<u8>> {
    let mut retained = Vec::with_capacity(MAX_OUTPUT_BYTES + 1);
    let mut buffer = [0_u8; 8 * 1024];
    loop {
        let count = stdout.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        let remaining = (MAX_OUTPUT_BYTES + 1).saturating_sub(retained.len());
        retained.extend_from_slice(&buffer[..count.min(remaining)]);
    }
    Ok(retained)
}

fn stop_child(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

fn wait_for_child(child: &mut Child) -> SaveBackupBackgroundRegistryResult<ExitStatus> {
    match child.wait_timeout(COMMAND_TIMEOUT) {
        Ok(Some(status)) => Ok(status),
        Ok(None) => {
            stop_child(child);
            Err(SaveBackupBackgroundRegistryError::CommandTimeout)
        }
        Err(_) => {
            stop_child(child);
            Err(SaveBackupBackgroundRegistryError::OperationFailed)
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub(super) struct PowerShellScheduledTaskCommandRunner;

impl ScheduledTaskCommandRunner for PowerShellScheduledTaskCommandRunner {
    fn run(
        &self,
        request: ScheduledTaskCommand,
    ) -> SaveBackupBackgroundRegistryResult<ScheduledTaskCommandOutcome> {
        let runtime = system_powershell_runtime()?;
        if let Some(outcome) = module_preflight_outcome(&request, &runtime) {
            return Ok(outcome);
        }
        let mut command = build_command_with_runtime(&request, &runtime);
        let mut child = command
            .spawn()
            .map_err(|_| SaveBackupBackgroundRegistryError::OperationFailed)?;
        let stdout = match child.stdout.take() {
            Some(stdout) => stdout,
            None => {
                stop_child(&mut child);
                return Err(SaveBackupBackgroundRegistryError::CommandInvalidOutput);
            }
        };
        let reader = std::thread::spawn(move || drain_stdout(stdout));

        let status = match wait_for_child(&mut child) {
            Ok(status) => status,
            Err(error) => {
                let _ = reader.join();
                return Err(error);
            }
        };
        let output = reader
            .join()
            .map_err(|_| SaveBackupBackgroundRegistryError::CommandInvalidOutput)?
            .map_err(|_| SaveBackupBackgroundRegistryError::CommandInvalidOutput)?;
        if !status.success() {
            return Err(SaveBackupBackgroundRegistryError::OperationFailed);
        }

        parse_script_output(&output)
    }
}
