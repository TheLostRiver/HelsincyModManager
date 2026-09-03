//! #286 外部 MOD 接管（adopt）的 command 边界——本族命令里唯一有写入的一条。
//!
//! 命令只做三件事：校验 opaque ID 与 layer 摘要、把活交给 runtime 任务服务、映射错误。
//! 接管只写安装清单、不碰任何文件；它写出的永远等于用户刚在弹窗里确认的那份扫描结果，
//! 否则以 `external_mod_adopt_stale` 失败（见 `hmm_runtime::external_mod_adopt`）。
//! 事件只报任务身份、阶段与稳定错误码；成功后前端重查安装状态即可，没有独立 getter。

use crate::dto::CommandErrorDto;
use crate::external_state_commands::{parse_external_state_id, parse_external_state_ids};
use crate::external_state_dto::ExternalModAdoptStartedDto;
use crate::state::AppState;
use crate::task_events::emit_task_progress;
use hmm_app::{TaskKind, TaskManager, TaskProgressEvent, TaskStatus};
use hmm_core::FileLayer;
use hmm_runtime::{
    queued_adopt_event, ExternalModAdoptTaskError, ExternalModAdoptTaskLaunch,
    ExternalModAdoptTaskService, EXTERNAL_MOD_ADOPT_CANCELLED_PHASE,
    EXTERNAL_MOD_ADOPT_FAILED_PHASE,
};
use std::sync::Arc;
use tauri::{AppHandle, State};

#[tauri::command]
pub fn start_external_mod_adopt(
    game_id: String,
    profile_id: String,
    mod_id: String,
    layer_name: String,
    layer_priority: i32,
    state: State<'_, AppState>,
    app_handle: AppHandle,
) -> Result<ExternalModAdoptStartedDto, CommandErrorDto> {
    let (game_id, profile_id, mod_id) = parse_external_state_ids(game_id, profile_id, mod_id)?;
    let layer = parse_layer(layer_name, layer_priority)?;

    let launch = state
        .external_mod_adopt_tasks
        .start_adopt(game_id, profile_id, mod_id, layer)
        .map_err(external_mod_adopt_task_error)?;
    let response = ExternalModAdoptStartedDto::from(&launch);

    if let Err(error) = emit_task_progress(&app_handle, queued_adopt_event(&launch)) {
        // queued 事件发不出去 = 前端拿不到任务身份，留着任务就是泄漏。
        let _ = state.external_mod_adopt_tasks.abort_queued_adopt(&launch);
        return Err(error);
    }
    spawn_external_mod_adopt_runner(
        Arc::clone(&state.external_mod_adopt_tasks),
        Arc::clone(&state.task_manager),
        app_handle,
        launch,
    );

    Ok(response)
}

/// layer 与 `start_install_task` 同形（前端与安装一样传 `base` / `0`）。名字按本族 opaque ID
/// 的受限字符集校验：它会原样落进清单条目，不接受路径样式或非 ASCII。
fn parse_layer(layer_name: String, layer_priority: i32) -> Result<FileLayer, CommandErrorDto> {
    let layer_name = parse_external_state_id(
        layer_name,
        "external_mod_adopt_layer_name_invalid",
        "external mod adopt layer name is invalid",
    )?;
    Ok(FileLayer::new(layer_name, layer_priority))
}

fn spawn_external_mod_adopt_runner(
    service: Arc<ExternalModAdoptTaskService>,
    task_manager: Arc<TaskManager>,
    app_handle: AppHandle,
    launch: ExternalModAdoptTaskLaunch,
) {
    std::thread::spawn(move || {
        let task_id = launch.task.task_id.clone();
        let task_kind = launch.task.kind;
        let mod_id = launch.mod_id.as_str().to_owned();
        let events = match service.run_adopt(launch) {
            Ok(events) => events,
            Err(error) => vec![fallback_adopt_terminal_event(
                &task_manager,
                task_id,
                task_kind,
                mod_id,
                error,
            )],
        };

        for event in events {
            let _ = emit_task_progress(&app_handle, event);
        }
    });
}

/// runner 自身坏掉（TaskManager 拒绝）时的兜底终态：任务不能停在 queued/running。
fn fallback_adopt_terminal_event(
    task_manager: &TaskManager,
    task_id: String,
    task_kind: TaskKind,
    mod_id: String,
    error: ExternalModAdoptTaskError,
) -> TaskProgressEvent {
    if matches!(
        task_manager.task_status(&task_id),
        Some(TaskStatus::Queued | TaskStatus::Running)
    ) {
        let _ = task_manager.fail_task(&task_id);
    }
    let status = task_manager
        .task_status(&task_id)
        .unwrap_or(TaskStatus::Failed);
    let mut event = TaskProgressEvent::new(
        task_id,
        task_kind,
        status,
        if status == TaskStatus::Cancelled {
            EXTERNAL_MOD_ADOPT_CANCELLED_PHASE
        } else {
            EXTERNAL_MOD_ADOPT_FAILED_PHASE
        },
    );
    event.result_ref = Some(mod_id);
    if status != TaskStatus::Cancelled {
        event.error = Some(error.code().to_owned());
    }
    event
}

fn external_mod_adopt_task_error(error: ExternalModAdoptTaskError) -> CommandErrorDto {
    CommandErrorDto {
        code: error.code().to_owned(),
        message: "external mod adopt task is unavailable".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layer_parser_accepts_the_install_shape_and_rejects_path_like_names() {
        let layer = parse_layer(" base ".to_owned(), 0).expect("valid layer");
        assert_eq!(layer, FileLayer::new("base", 0));

        for invalid in ["", "  ", "a/b", "a\\b", "..", "层"] {
            let error = parse_layer(invalid.to_owned(), 0).expect_err("invalid layer name");
            assert_eq!(error.code, "external_mod_adopt_layer_name_invalid");
        }
    }

    #[test]
    fn fallback_event_marks_a_stuck_task_failed_with_the_stable_code() {
        let task_manager = TaskManager::new();
        let task = task_manager
            .create_task(TaskKind::ExternalModAdopt)
            .expect("create task");

        let event = fallback_adopt_terminal_event(
            &task_manager,
            task.task_id.clone(),
            task.kind,
            "mod-a".to_owned(),
            ExternalModAdoptTaskError::TaskUnavailable,
        );

        assert_eq!(
            task_manager.task_status(&task.task_id),
            Some(TaskStatus::Failed)
        );
        assert_eq!(event.kind, TaskKind::ExternalModAdopt);
        assert_eq!(event.status, TaskStatus::Failed);
        assert_eq!(event.phase, EXTERNAL_MOD_ADOPT_FAILED_PHASE);
        assert_eq!(event.result_ref.as_deref(), Some("mod-a"));
        assert_eq!(
            event.error.as_deref(),
            Some("external_mod_adopt_task_unavailable")
        );
    }

    #[test]
    fn fallback_event_respects_a_cancelled_task_instead_of_forcing_failed() {
        let task_manager = TaskManager::new();
        let task = task_manager
            .create_task(TaskKind::ExternalModAdopt)
            .expect("create task");
        task_manager
            .cancel_task(&task.task_id)
            .expect("cancel task");

        let event = fallback_adopt_terminal_event(
            &task_manager,
            task.task_id.clone(),
            task.kind,
            "mod-a".to_owned(),
            ExternalModAdoptTaskError::TaskUnavailable,
        );

        assert_eq!(event.status, TaskStatus::Cancelled);
        assert_eq!(event.phase, EXTERNAL_MOD_ADOPT_CANCELLED_PHASE);
        // 取消不是失败：不得给取消终态贴错误码。
        assert_eq!(event.error, None);
    }
}
