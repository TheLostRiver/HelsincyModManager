use super::{
    task_spec::{task_name_for_sid, ScheduledTaskReadback, ScheduledTaskState},
    ScheduledTaskCommandOutcome,
};
use crate::windows_identity::{current_process_user_sid, sid_to_string};
use hmm_ports::{SaveBackupBackgroundRegistryError, SaveBackupBackgroundRegistryResult};
use std::path::PathBuf;
use windows::core::{Interface, BSTR, HRESULT, PCWSTR, PWSTR};
use windows::Win32::Foundation::{
    ERROR_INSUFFICIENT_BUFFER, E_ACCESSDENIED, E_INVALIDARG, RPC_E_CHANGED_MODE, VARIANT_BOOL,
};
use windows::Win32::Security::{LookupAccountNameW, PSID, SID_NAME_USE};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED,
};
use windows::Win32::System::TaskScheduler::{
    IAction, IExecAction, ILogonTrigger, IRegisteredTask, ITaskDefinition, ITaskService, ITrigger,
    TaskScheduler, TASK_ACTION_EXEC, TASK_INSTANCES_IGNORE_NEW, TASK_INSTANCES_PARALLEL,
    TASK_INSTANCES_QUEUE, TASK_INSTANCES_STOP_EXISTING, TASK_LOGON_GROUP,
    TASK_LOGON_INTERACTIVE_TOKEN, TASK_LOGON_INTERACTIVE_TOKEN_OR_PASSWORD, TASK_LOGON_NONE,
    TASK_LOGON_PASSWORD, TASK_LOGON_S4U, TASK_LOGON_SERVICE_ACCOUNT, TASK_RUNLEVEL_HIGHEST,
    TASK_RUNLEVEL_LUA, TASK_STATE_DISABLED, TASK_STATE_QUEUED, TASK_STATE_READY,
    TASK_STATE_RUNNING, TASK_TRIGGER_LOGON, TASK_TRIGGER_TIME,
};
use windows::Win32::System::Variant::VARIANT;

const TASK_NOT_FOUND: HRESULT = HRESULT::from_win32(2);

pub(super) fn current_user_identity(
) -> SaveBackupBackgroundRegistryResult<ScheduledTaskCommandOutcome> {
    current_process_user_sid()
        .map(ScheduledTaskCommandOutcome::Identity)
        .map_err(|_| SaveBackupBackgroundRegistryError::OperationFailed)
}

pub(super) fn inspect_scheduled_task(
    task_name: &str,
    owner_marker: &str,
) -> SaveBackupBackgroundRegistryResult<ScheduledTaskCommandOutcome> {
    if task_name.trim().is_empty() || owner_marker.trim().is_empty() {
        return Err(SaveBackupBackgroundRegistryError::OperationFailed);
    }

    let _apartment = ComApartment::initialize()?;
    let task = match open_task(task_name) {
        Ok(task) => task,
        Err(error) if error.code() == TASK_NOT_FOUND => {
            return Ok(ScheduledTaskCommandOutcome::Missing);
        }
        Err(error) if error.code() == E_ACCESSDENIED => {
            return Ok(ScheduledTaskCommandOutcome::PermissionRequired);
        }
        Err(_) => return Err(SaveBackupBackgroundRegistryError::OperationFailed),
    };
    let readback = match read_task(&task) {
        Ok(readback) => readback,
        Err(error) => return map_readback_error(error.code()),
    };
    Ok(ScheduledTaskCommandOutcome::Found(Box::new(readback)))
}

fn map_readback_error(
    code: HRESULT,
) -> SaveBackupBackgroundRegistryResult<ScheduledTaskCommandOutcome> {
    if code == E_ACCESSDENIED {
        Ok(ScheduledTaskCommandOutcome::PermissionRequired)
    } else {
        Err(SaveBackupBackgroundRegistryError::CommandInvalidOutput)
    }
}

struct ComApartment {
    uninitialize: bool,
}

impl ComApartment {
    fn initialize() -> SaveBackupBackgroundRegistryResult<Self> {
        let result = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
        if result.is_ok() {
            return Ok(Self { uninitialize: true });
        }
        if result == RPC_E_CHANGED_MODE {
            return Ok(Self {
                uninitialize: false,
            });
        }
        Err(SaveBackupBackgroundRegistryError::OperationFailed)
    }
}

impl Drop for ComApartment {
    fn drop(&mut self) {
        if self.uninitialize {
            unsafe { CoUninitialize() };
        }
    }
}

fn open_task(task_name: &str) -> windows::core::Result<IRegisteredTask> {
    unsafe {
        let service: ITaskService = CoCreateInstance(&TaskScheduler, None, CLSCTX_INPROC_SERVER)?;
        let empty = VARIANT::default();
        service.Connect(&empty, &empty, &empty, &empty)?;
        let root = service.GetFolder(&BSTR::from("\\"))?;
        root.GetTask(&BSTR::from(task_name))
    }
}

fn read_task(task: &IRegisteredTask) -> windows::core::Result<ScheduledTaskReadback> {
    unsafe {
        let definition = task.Definition()?;
        let state = scheduled_task_state(task.State()?);
        let owner_marker = read_registration_description(&definition)?;
        let principal = definition.Principal()?;
        let user_sid = normalize_account_sid(read_bstr(|value| principal.UserId(value))?)?;
        let (logon_type, run_level) = read_principal_policy(&principal)?;
        let (action_count, action_execute, action_arguments, action_working_directory) =
            read_action(&definition)?;
        let triggers = read_triggers(&definition)?;
        let settings = read_settings(&definition)?;

        Ok(ScheduledTaskReadback {
            task_path: "\\".to_owned(),
            owner_marker,
            user_sid,
            action_count,
            action_execute,
            action_arguments,
            action_working_directory,
            logon_trigger_count: triggers.logon_count,
            time_trigger_count: triggers.time_count,
            logon_trigger_user_sid: triggers.logon_user_sid,
            logon_trigger_enabled: triggers.logon_enabled,
            time_trigger_enabled: triggers.time_enabled,
            logon_delay: triggers.logon_delay,
            periodic_interval: triggers.periodic_interval,
            periodic_duration: triggers.periodic_duration,
            logon_type,
            run_level,
            multiple_instances: settings.multiple_instances,
            start_when_available: settings.start_when_available,
            allow_start_on_batteries: !settings.disallow_start_on_batteries,
            dont_stop_on_batteries: !settings.stop_on_batteries,
            wake_to_run: settings.wake_to_run,
            run_only_if_network_available: settings.run_only_if_network_available,
            execution_time_limit: settings.execution_time_limit,
            enabled: state != ScheduledTaskState::Disabled,
            state,
        })
    }
}

unsafe fn read_registration_description(
    definition: &ITaskDefinition,
) -> windows::core::Result<String> {
    let registration = unsafe { definition.RegistrationInfo()? };
    read_bstr(|value| unsafe { registration.Description(value) })
}

unsafe fn read_principal_policy(
    principal: &windows::Win32::System::TaskScheduler::IPrincipal,
) -> windows::core::Result<(String, String)> {
    let mut logon_type = Default::default();
    unsafe { principal.LogonType(&mut logon_type)? };
    let logon_type = if logon_type == TASK_LOGON_INTERACTIVE_TOKEN {
        "Interactive"
    } else if logon_type == TASK_LOGON_PASSWORD {
        "Password"
    } else if logon_type == TASK_LOGON_S4U {
        "S4U"
    } else if logon_type == TASK_LOGON_GROUP {
        "Group"
    } else if logon_type == TASK_LOGON_SERVICE_ACCOUNT {
        "ServiceAccount"
    } else if logon_type == TASK_LOGON_INTERACTIVE_TOKEN_OR_PASSWORD {
        "InteractiveOrPassword"
    } else if logon_type == TASK_LOGON_NONE {
        "None"
    } else {
        "Unknown"
    };

    let mut run_level = Default::default();
    unsafe { principal.RunLevel(&mut run_level)? };
    let run_level = if run_level == TASK_RUNLEVEL_LUA {
        "Limited"
    } else if run_level == TASK_RUNLEVEL_HIGHEST {
        "Highest"
    } else {
        "Unknown"
    };
    Ok((logon_type.to_owned(), run_level.to_owned()))
}

unsafe fn read_action(
    definition: &ITaskDefinition,
) -> windows::core::Result<(u32, PathBuf, String, String)> {
    let actions = unsafe { definition.Actions()? };
    let mut count = 0;
    unsafe { actions.Count(&mut count)? };
    let count = u32::try_from(count).unwrap_or(u32::MAX);
    if count != 1 {
        return Ok((count, PathBuf::new(), String::new(), String::new()));
    }

    let action: IAction = unsafe { actions.get_Item(1)? };
    let mut action_type = Default::default();
    unsafe { action.Type(&mut action_type)? };
    if action_type != TASK_ACTION_EXEC {
        return Ok((count, PathBuf::new(), String::new(), String::new()));
    }
    let action: IExecAction = action.cast()?;
    Ok((
        count,
        PathBuf::from(read_bstr(|value| unsafe { action.Path(value) })?),
        read_bstr(|value| unsafe { action.Arguments(value) })?,
        read_bstr(|value| unsafe { action.WorkingDirectory(value) })?,
    ))
}

#[derive(Default)]
struct TriggerReadback {
    logon_count: u32,
    time_count: u32,
    logon_user_sid: String,
    logon_enabled: bool,
    time_enabled: bool,
    logon_delay: String,
    periodic_interval: String,
    periodic_duration: String,
}

unsafe fn read_triggers(definition: &ITaskDefinition) -> windows::core::Result<TriggerReadback> {
    let collection = unsafe { definition.Triggers()? };
    let mut count = 0;
    unsafe { collection.Count(&mut count)? };
    let mut readback = TriggerReadback::default();
    for index in 1..=count {
        let trigger: ITrigger = unsafe { collection.get_Item(index)? };
        let mut trigger_type = Default::default();
        unsafe { trigger.Type(&mut trigger_type)? };
        if trigger_type == TASK_TRIGGER_LOGON {
            readback.logon_count = readback.logon_count.saturating_add(1);
            if readback.logon_count == 1 {
                let logon: ILogonTrigger = trigger.cast()?;
                readback.logon_user_sid =
                    normalize_account_sid(read_bstr(|value| unsafe { logon.UserId(value) })?)?;
                readback.logon_delay = read_bstr(|value| unsafe { logon.Delay(value) })?;
                readback.logon_enabled = read_trigger_enabled(&trigger)?;
            }
        } else if trigger_type == TASK_TRIGGER_TIME {
            readback.time_count = readback.time_count.saturating_add(1);
            if readback.time_count == 1 {
                let repetition = unsafe { trigger.Repetition()? };
                readback.periodic_interval =
                    read_bstr(|value| unsafe { repetition.Interval(value) })?;
                readback.periodic_duration =
                    read_bstr(|value| unsafe { repetition.Duration(value) })?;
                readback.time_enabled = read_trigger_enabled(&trigger)?;
            }
        }
    }
    if readback.logon_count != 1 {
        readback.logon_user_sid.clear();
        readback.logon_delay.clear();
        readback.logon_enabled = false;
    }
    if readback.time_count != 1 {
        readback.periodic_interval.clear();
        readback.periodic_duration.clear();
        readback.time_enabled = false;
    }
    Ok(readback)
}

unsafe fn read_trigger_enabled(trigger: &ITrigger) -> windows::core::Result<bool> {
    let mut enabled = VARIANT_BOOL::default();
    unsafe { trigger.Enabled(&mut enabled)? };
    Ok(variant_bool(enabled))
}

struct SettingsReadback {
    multiple_instances: String,
    start_when_available: bool,
    disallow_start_on_batteries: bool,
    stop_on_batteries: bool,
    wake_to_run: bool,
    run_only_if_network_available: bool,
    execution_time_limit: String,
}

unsafe fn read_settings(definition: &ITaskDefinition) -> windows::core::Result<SettingsReadback> {
    let settings = unsafe { definition.Settings()? };
    let mut multiple_instances = Default::default();
    unsafe { settings.MultipleInstances(&mut multiple_instances)? };
    let multiple_instances = if multiple_instances == TASK_INSTANCES_IGNORE_NEW {
        "IgnoreNew"
    } else if multiple_instances == TASK_INSTANCES_PARALLEL {
        "Parallel"
    } else if multiple_instances == TASK_INSTANCES_QUEUE {
        "Queue"
    } else if multiple_instances == TASK_INSTANCES_STOP_EXISTING {
        "StopExisting"
    } else {
        "Unknown"
    };

    Ok(SettingsReadback {
        multiple_instances: multiple_instances.to_owned(),
        start_when_available: read_bool(|value| unsafe { settings.StartWhenAvailable(value) })?,
        disallow_start_on_batteries: read_bool(|value| unsafe {
            settings.DisallowStartIfOnBatteries(value)
        })?,
        stop_on_batteries: read_bool(|value| unsafe { settings.StopIfGoingOnBatteries(value) })?,
        wake_to_run: read_bool(|value| unsafe { settings.WakeToRun(value) })?,
        run_only_if_network_available: read_bool(|value| unsafe {
            settings.RunOnlyIfNetworkAvailable(value)
        })?,
        execution_time_limit: read_bstr(|value| unsafe { settings.ExecutionTimeLimit(value) })?,
    })
}

fn read_bstr(
    read: impl FnOnce(*mut BSTR) -> windows::core::Result<()>,
) -> windows::core::Result<String> {
    let mut value = BSTR::new();
    read(&mut value)?;
    String::try_from(value).map_err(|_| {
        windows::core::Error::new(
            HRESULT(0x8007_0057_u32 as i32),
            "scheduled task string is invalid",
        )
    })
}

fn read_bool(
    read: impl FnOnce(*mut VARIANT_BOOL) -> windows::core::Result<()>,
) -> windows::core::Result<bool> {
    let mut value = VARIANT_BOOL::default();
    read(&mut value)?;
    Ok(variant_bool(value))
}

fn variant_bool(value: VARIANT_BOOL) -> bool {
    value.0 != 0
}

fn normalize_account_sid(value: String) -> windows::core::Result<String> {
    normalize_account_sid_with(value, lookup_account_sid)
}

fn normalize_account_sid_with(
    value: String,
    lookup: impl FnOnce(&str) -> windows::core::Result<String>,
) -> windows::core::Result<String> {
    if task_name_for_sid(&value).is_ok() {
        return Ok(value);
    }
    if value.trim().is_empty() {
        return Err(native_readback_error());
    }
    lookup(&value)
}

fn lookup_account_sid(account_name: &str) -> windows::core::Result<String> {
    let account_name_utf16 = account_name
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    // Keep the UTF-16 storage alive across both account lookup calls.
    let account_name = PCWSTR::from_raw(account_name_utf16.as_ptr());
    let mut sid_bytes = 0_u32;
    let mut domain_chars = 0_u32;
    let mut sid_name_use = SID_NAME_USE::default();
    let first = unsafe {
        LookupAccountNameW(
            PCWSTR::null(),
            account_name,
            None,
            &mut sid_bytes,
            None,
            &mut domain_chars,
            &mut sid_name_use,
        )
    };
    let first_error = match first {
        Err(error) => error,
        Ok(()) => return Err(native_readback_error()),
    };
    if first_error.code() != HRESULT::from_win32(ERROR_INSUFFICIENT_BUFFER.0) || sid_bytes == 0 {
        return Err(first_error);
    }

    let mut sid = vec![0_u8; sid_bytes as usize];
    let mut domain = vec![0_u16; domain_chars as usize];
    let domain_buffer = (!domain.is_empty()).then(|| PWSTR::from_raw(domain.as_mut_ptr()));
    unsafe {
        LookupAccountNameW(
            PCWSTR::null(),
            account_name,
            Some(PSID(sid.as_mut_ptr().cast())),
            &mut sid_bytes,
            domain_buffer,
            &mut domain_chars,
            &mut sid_name_use,
        )?;
    }

    sid_to_string(PSID(sid.as_mut_ptr().cast()))
}

fn native_readback_error() -> windows::core::Error {
    windows::core::Error::new(E_INVALIDARG, "scheduled task readback is invalid")
}

fn scheduled_task_state(
    state: windows::Win32::System::TaskScheduler::TASK_STATE,
) -> ScheduledTaskState {
    if state == TASK_STATE_DISABLED {
        ScheduledTaskState::Disabled
    } else if state == TASK_STATE_QUEUED {
        ScheduledTaskState::Queued
    } else if state == TASK_STATE_READY {
        ScheduledTaskState::Ready
    } else if state == TASK_STATE_RUNNING {
        ScheduledTaskState::Running
    } else {
        ScheduledTaskState::Unknown
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_enum_mappings_preserve_existing_readback_strings() {
        assert_eq!(
            scheduled_task_state(TASK_STATE_DISABLED),
            ScheduledTaskState::Disabled
        );
        assert_eq!(
            scheduled_task_state(TASK_STATE_READY),
            ScheduledTaskState::Ready
        );
        assert_eq!(
            scheduled_task_state(TASK_STATE_RUNNING),
            ScheduledTaskState::Running
        );
        assert!(!variant_bool(VARIANT_BOOL(0)));
        assert!(variant_bool(VARIANT_BOOL(-1)));
    }

    #[test]
    fn native_readback_preserves_permission_required_and_fails_closed_other_errors() {
        assert_eq!(
            map_readback_error(E_ACCESSDENIED).expect("permission outcome"),
            ScheduledTaskCommandOutcome::PermissionRequired
        );
        assert_eq!(
            map_readback_error(E_INVALIDARG).expect_err("invalid readback must fail closed"),
            SaveBackupBackgroundRegistryError::CommandInvalidOutput
        );
    }

    #[test]
    fn sid_normalization_skips_lookup_for_sid_and_requires_resolution_for_account_names() {
        let sid = "S-1-5-21-100-200-300-400".to_owned();
        let normalized = normalize_account_sid_with(sid.clone(), |_| {
            panic!("valid SID must not call account lookup")
        })
        .expect("valid SID");
        assert_eq!(normalized, sid);

        let normalized = normalize_account_sid_with("DOMAIN\\Player".to_owned(), |account| {
            assert_eq!(account, "DOMAIN\\Player");
            Ok("S-1-5-21-9".to_owned())
        })
        .expect("resolved account SID");
        assert_eq!(normalized, "S-1-5-21-9");

        assert!(
            normalize_account_sid_with(" ".to_owned(), |_| { Ok("S-1-5-21-9".to_owned()) })
                .is_err()
        );

        assert!(
            normalize_account_sid_with("DOMAIN\\Missing".to_owned(), |_| {
                Err(native_readback_error())
            })
            .is_err()
        );
    }

    #[test]
    fn current_process_identity_is_a_valid_task_sid() {
        let ScheduledTaskCommandOutcome::Identity(sid) =
            current_user_identity().expect("current process identity")
        else {
            panic!("current process identity must return a SID");
        };
        task_name_for_sid(&sid).expect("current process SID must be valid");
    }
}
