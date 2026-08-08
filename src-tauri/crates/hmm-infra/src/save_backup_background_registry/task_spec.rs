use sha2::{Digest, Sha256};
use std::path::PathBuf;

pub(super) const TASK_OWNER_MARKER: &str = "dev.helsincy.modmanager/save-backup";
pub(super) const TASK_PATH: &str = "\\";
pub(super) const TASK_ARGUMENTS: &str = "--once";
pub(super) const LOGON_DELAY: &str = "PT1M";
pub(super) const PERIODIC_INTERVAL: &str = "PT15M";
pub(super) const EXECUTION_TIME_LIMIT: &str = "PT1H";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ScheduledTaskSpec {
    pub task_name: String,
    pub task_path: String,
    pub owner_marker: String,
    pub user_sid: String,
    pub worker_path: PathBuf,
    pub action_arguments: String,
    pub logon_delay: String,
    pub periodic_interval: String,
    pub execution_time_limit: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct ScheduledTaskReadback {
    pub task_path: String,
    pub owner_marker: String,
    pub user_sid: String,
    pub action_count: u32,
    pub action_execute: PathBuf,
    pub action_arguments: String,
    pub action_working_directory: String,
    pub logon_trigger_count: u32,
    pub time_trigger_count: u32,
    pub logon_trigger_user_sid: String,
    pub logon_trigger_enabled: bool,
    pub time_trigger_enabled: bool,
    pub logon_delay: String,
    pub periodic_interval: String,
    pub periodic_duration: String,
    pub logon_type: String,
    pub run_level: String,
    pub multiple_instances: String,
    pub start_when_available: bool,
    pub allow_start_on_batteries: bool,
    pub dont_stop_on_batteries: bool,
    pub wake_to_run: bool,
    pub run_only_if_network_available: bool,
    pub execution_time_limit: String,
    pub enabled: bool,
    pub state: ScheduledTaskState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "PascalCase")]
pub(super) enum ScheduledTaskState {
    Unknown,
    Disabled,
    Queued,
    Ready,
    Running,
}

impl ScheduledTaskState {
    pub(super) fn is_busy(self) -> bool {
        matches!(self, Self::Running | Self::Queued)
    }

    pub(super) fn is_quiescent(self) -> bool {
        matches!(self, Self::Ready | Self::Disabled)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ScheduledTaskSpecMatch {
    Exact,
    OwnedDrift,
    OwnershipConflict,
}

impl ScheduledTaskSpec {
    pub fn new(user_sid: &str, worker_path: PathBuf) -> Result<Self, &'static str> {
        let task_name = task_name_for_sid(user_sid)?;
        if !worker_path.is_absolute() {
            return Err("invalid scheduled task identity");
        }

        Ok(Self {
            task_name,
            task_path: TASK_PATH.to_owned(),
            owner_marker: TASK_OWNER_MARKER.to_owned(),
            user_sid: user_sid.to_owned(),
            worker_path,
            action_arguments: TASK_ARGUMENTS.to_owned(),
            logon_delay: LOGON_DELAY.to_owned(),
            periodic_interval: PERIODIC_INTERVAL.to_owned(),
            execution_time_limit: EXECUTION_TIME_LIMIT.to_owned(),
        })
    }

    pub fn compare(&self, actual: &ScheduledTaskReadback) -> ScheduledTaskSpecMatch {
        if actual.owner_marker != self.owner_marker {
            return ScheduledTaskSpecMatch::OwnershipConflict;
        }

        let exact = actual.task_path == self.task_path
            && actual.user_sid == self.user_sid
            && actual.action_count == 1
            && actual.action_execute == self.worker_path
            && actual.action_arguments == self.action_arguments
            && actual.action_working_directory.is_empty()
            && actual.logon_trigger_count == 1
            && actual.time_trigger_count == 1
            && actual.logon_trigger_user_sid == self.user_sid
            && actual.logon_trigger_enabled
            && actual.time_trigger_enabled
            && actual.logon_delay == self.logon_delay
            && actual.periodic_interval == self.periodic_interval
            && actual.periodic_duration.is_empty()
            && actual.logon_type.eq_ignore_ascii_case("Interactive")
            && actual.run_level.eq_ignore_ascii_case("Limited")
            && actual.multiple_instances.eq_ignore_ascii_case("IgnoreNew")
            && actual.start_when_available
            && actual.allow_start_on_batteries
            && actual.dont_stop_on_batteries
            && !actual.wake_to_run
            && !actual.run_only_if_network_available
            && actual.execution_time_limit == self.execution_time_limit
            && actual.enabled;

        if exact {
            ScheduledTaskSpecMatch::Exact
        } else {
            ScheduledTaskSpecMatch::OwnedDrift
        }
    }
}

pub(super) fn task_name_for_sid(user_sid: &str) -> Result<String, &'static str> {
    let sid_segments = user_sid.split('-').collect::<Vec<_>>();
    let valid_sid = user_sid.len() <= 184
        && sid_segments.len() >= 3
        && sid_segments[0] == "S"
        && sid_segments[1..].iter().all(|segment| {
            !segment.is_empty() && segment.bytes().all(|value| value.is_ascii_digit())
        });
    if !valid_sid {
        return Err("invalid scheduled task identity");
    }

    let digest = Sha256::digest(user_sid.as_bytes());
    let suffix = digest[..8]
        .iter()
        .map(|value| format!("{value:02x}"))
        .collect::<String>();
    Ok(format!("HelsincyModManager.SaveBackup.{suffix}"))
}
