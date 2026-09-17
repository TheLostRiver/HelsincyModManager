use crate::dto::{CommandErrorDto, TaskKindDto, TaskProgressEventDto, TaskStatusDto};
use crate::state::AppState;
use hmm_app::TaskProgressEvent;
use hmm_infra::{emit_safe_app_log, AppLogEvent};
use hmm_ports::TaskLogRecord;
use hmm_runtime::TaskProgressObserver;
use std::time::{SystemTime, UNIX_EPOCH};
use std::{collections::BTreeMap, sync::Mutex};
use tauri::{AppHandle, Emitter, Manager};

pub const TASK_PROGRESS_EVENT_NAME: &str = "hmm://task-progress";
pub const INSTALL_REINSTALL_QUEUED_PHASE: &str = "install.reinstall.queued";

/// Last published progress, also retained when the WebView event transport fails. Reading
/// this is observational only; a missing event is never interpreted as task completion.
#[derive(Default)]
pub(crate) struct TaskProgressSnapshots(Mutex<BTreeMap<String, TaskProgressEventDto>>);

impl TaskProgressSnapshots {
    pub(crate) fn record(&self, event: &TaskProgressEventDto) {
        if let Ok(mut snapshots) = self.0.lock() {
            if snapshots
                .get(&event.task_id)
                .is_some_and(|previous| is_terminal(previous.status))
                && !is_terminal(event.status)
            {
                return;
            }
            snapshots.insert(event.task_id.clone(), event.clone());
            if snapshots.len() > 512 {
                let removable: Vec<_> = snapshots
                    .iter()
                    .filter(|(id, value)| **id != event.task_id && is_terminal(value.status))
                    .take(snapshots.len() - 512)
                    .map(|(id, _)| id.clone())
                    .collect();
                for id in removable {
                    snapshots.remove(&id);
                }
            }
        }
    }

    pub(crate) fn get(
        &self,
        task_id: &str,
    ) -> Result<Option<TaskProgressEventDto>, CommandErrorDto> {
        self.0
            .lock()
            .map(|snapshots| snapshots.get(task_id).cloned())
            .map_err(|_| CommandErrorDto {
                code: "task_store_unavailable".to_owned(),
                message: "task progress is unavailable".to_owned(),
            })
    }
}

fn is_terminal(status: TaskStatusDto) -> bool {
    matches!(
        status,
        TaskStatusDto::Completed | TaskStatusDto::Failed | TaskStatusDto::Cancelled
    )
}

pub fn emit_task_progress(
    app_handle: &AppHandle,
    event: TaskProgressEvent,
) -> Result<(), CommandErrorDto> {
    TauriTaskProgressObserver::new(app_handle).observe(&event)
}

pub(crate) struct TauriTaskProgressObserver<'a> {
    app_handle: &'a AppHandle,
    installation: Option<(hmm_core::GameId, hmm_core::ProfileId, hmm_core::ModId)>,
}

impl<'a> TauriTaskProgressObserver<'a> {
    pub(crate) fn new(app_handle: &'a AppHandle) -> Self {
        Self {
            app_handle,
            installation: None,
        }
    }

    pub(crate) fn for_mod(
        app_handle: &'a AppHandle,
        game_id: &hmm_core::GameId,
        profile_id: &hmm_core::ProfileId,
        mod_id: &hmm_core::ModId,
    ) -> Self {
        Self {
            app_handle,
            installation: Some((game_id.clone(), profile_id.clone(), mod_id.clone())),
        }
    }
}

impl TaskProgressObserver for TauriTaskProgressObserver<'_> {
    type Error = CommandErrorDto;

    fn observe(&self, event: &TaskProgressEvent) -> Result<(), Self::Error> {
        if matches!(
            event.status,
            hmm_app::TaskStatus::Completed
                | hmm_app::TaskStatus::Failed
                | hmm_app::TaskStatus::Cancelled
        ) {
            if let Some((game_id, profile_id, mod_id)) = &self.installation {
                use hmm_app::ModInstallationStateObserver;
                crate::mod_installation_state::TauriModInstallationStateObserver::new(
                    self.app_handle,
                )
                .state_changed(&event.task_id, game_id, profile_id, mod_id);
            }
        }
        emit_task_progress_dto(self.app_handle, event.clone().into())
    }
}

fn emit_task_progress_dto(
    app_handle: &AppHandle,
    event: TaskProgressEventDto,
) -> Result<(), CommandErrorDto> {
    app_handle
        .state::<AppState>()
        .task_progress_snapshots
        .record(&event);
    record_task_log(app_handle, &event);
    if let Some(registration) = queued_task_registration_event(&event) {
        emit_safe_app_log(registration);
    }
    let failure_event = AppLogEvent::warning("task.progress_emit_failed")
        .with_task_id(event.task_id.clone())
        .with_task_kind(task_kind_code(event.kind))
        .with_task_status(task_status_code(event.status))
        .with_phase(event.phase.clone())
        .with_error_code("task_progress_emit_failed");
    app_handle
        .emit(TASK_PROGRESS_EVENT_NAME, event)
        .map_err(|_| {
            emit_safe_app_log(failure_event);
            CommandErrorDto {
                code: "task_progress_emit_failed".to_owned(),
                message: "failed to emit task progress event".to_owned(),
            }
        })
}

fn record_task_log(app_handle: &AppHandle, event: &TaskProgressEventDto) {
    let Ok(timestamp_unix_millis) = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_millis())
    else {
        return;
    };
    let error_code = event
        .error
        .as_deref()
        .filter(|value| is_stable_code(value))
        .map(str::to_owned);
    let record = TaskLogRecord {
        timestamp_unix_millis,
        task_id: event.task_id.clone(),
        kind: task_kind_code(event.kind).to_owned(),
        status: task_status_code(event.status).to_owned(),
        phase: event.phase.clone(),
        current: event.current,
        total: event.total,
        duration_ms: None,
        error_code,
    };
    let _ = app_handle
        .state::<AppState>()
        .task_log_writer
        .record(record);
}

fn is_stable_code(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 96
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.' | b':' | b'-'))
}

fn queued_task_registration_event(event: &TaskProgressEventDto) -> Option<AppLogEvent> {
    (event.status == TaskStatusDto::Queued).then(|| {
        AppLogEvent::info("task.registered")
            .with_task_id(event.task_id.clone())
            .with_task_kind(task_kind_code(event.kind))
            .with_task_status(task_status_code(event.status))
            .with_phase(event.phase.clone())
    })
}

fn task_kind_code(kind: TaskKindDto) -> &'static str {
    match kind {
        TaskKindDto::ModImport => "mod_import",
        TaskKindDto::Install => "install",
        TaskKindDto::SaveBackup => "save_backup",
        TaskKindDto::SaveRestore => "save_restore",
        TaskKindDto::ExternalStateScan => "external_state_scan",
        TaskKindDto::ExternalModAdopt => "external_mod_adopt",
        TaskKindDto::ModStorageMigration => "mod_storage_migration",
    }
}

fn task_status_code(status: TaskStatusDto) -> &'static str {
    match status {
        TaskStatusDto::Queued => "queued",
        TaskStatusDto::Running => "running",
        TaskStatusDto::Completed => "completed",
        TaskStatusDto::Failed => "failed",
        TaskStatusDto::Cancelled => "cancelled",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn progress_event(status: TaskStatusDto) -> TaskProgressEventDto {
        TaskProgressEventDto {
            task_id: "install-123".to_owned(),
            kind: TaskKindDto::Install,
            status,
            phase: "install.queued".to_owned(),
            current: None,
            total: None,
            message: Some("not logged".to_owned()),
            error: Some("C:/Users/Alice must not be logged".to_owned()),
            result_ref: Some("not-logged".to_owned()),
        }
    }

    #[test]
    fn progress_snapshots_preserve_terminal_events_and_unknown_is_not_completion() {
        let snapshots = TaskProgressSnapshots::default();
        assert_eq!(snapshots.get("missing").unwrap(), None);
        let terminal = progress_event(TaskStatusDto::Completed);
        snapshots.record(&terminal);
        snapshots.record(&progress_event(TaskStatusDto::Running));
        assert_eq!(snapshots.get(&terminal.task_id).unwrap(), Some(terminal));
    }

    #[test]
    fn bounded_progress_history_retains_active_tasks() {
        let snapshots = TaskProgressSnapshots::default();
        let active = progress_event(TaskStatusDto::Running);
        snapshots.record(&active);
        for index in 0..600 {
            let mut terminal = progress_event(TaskStatusDto::Completed);
            terminal.task_id = format!("finished-{index:04}");
            snapshots.record(&terminal);
        }
        assert_eq!(snapshots.get(&active.task_id).unwrap(), Some(active));
        assert!(snapshots.0.lock().unwrap().len() <= 512);
    }

    #[test]
    fn queued_event_builds_task_registration_from_allowlisted_fields_only() {
        assert_eq!(
            queued_task_registration_event(&progress_event(TaskStatusDto::Queued)),
            Some(
                AppLogEvent::info("task.registered")
                    .with_task_id("install-123")
                    .with_task_kind("install")
                    .with_task_status("queued")
                    .with_phase("install.queued")
            )
        );
    }

    #[test]
    fn non_queued_event_does_not_create_a_registration_log() {
        assert!(queued_task_registration_event(&progress_event(TaskStatusDto::Running)).is_none());
    }
}
