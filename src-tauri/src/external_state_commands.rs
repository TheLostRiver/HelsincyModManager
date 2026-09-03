//! #286 外部 MOD 状态扫描的 command 边界。
//!
//! 命令只做三件事：校验 opaque ID、把活交给 runtime 服务、映射错误。
//! 扫描结果**不进进度事件**（契约禁止 payload 携带 target_path）——
//! 事件只报任务身份与阶段，结果走 `get_external_mod_state` 查询。

use crate::dto::CommandErrorDto;
use crate::external_state_dto::{ExternalModStateDto, ExternalStateScanStartedDto};
use crate::state::AppState;
use crate::task_events::emit_task_progress;
use hmm_app::{TaskKind, TaskManager, TaskProgressEvent, TaskStatus};
use hmm_core::{GameId, ModId, ProfileId};
use hmm_runtime::{
    queued_scan_event, ExternalStateScanTaskError, ExternalStateScanTaskLaunch,
    ExternalStateScanTaskService, EXTERNAL_STATE_SCAN_CANCELLED_PHASE,
    EXTERNAL_STATE_SCAN_FAILED_PHASE,
};
use std::sync::Arc;
use tauri::{AppHandle, State};

/// 与 `external_import_commands` 同一上界：这些 ID 全部由后端生成，160 足够。
const EXTERNAL_STATE_ID_MAX_LENGTH: usize = 160;

#[tauri::command]
pub fn start_external_mod_state_scan(
    game_id: String,
    profile_id: String,
    mod_id: String,
    state: State<'_, AppState>,
    app_handle: AppHandle,
) -> Result<ExternalStateScanStartedDto, CommandErrorDto> {
    let (game_id, profile_id, mod_id) = parse_external_state_ids(game_id, profile_id, mod_id)?;

    let launch = state
        .external_state_scan_tasks
        .start_scan(game_id, profile_id, mod_id)
        .map_err(external_state_task_error)?;
    let response = ExternalStateScanStartedDto::from(&launch);

    if let Err(error) = emit_task_progress(&app_handle, queued_scan_event(&launch)) {
        // queued 事件发不出去 = 前端拿不到任务身份，留着任务就是泄漏。
        let _ = state.external_state_scan_tasks.abort_queued_scan(&launch);
        return Err(error);
    }
    spawn_external_state_scan_runner(
        Arc::clone(&state.external_state_scan_tasks),
        Arc::clone(&state.task_manager),
        app_handle,
        launch,
    );

    Ok(response)
}

#[tauri::command]
pub fn get_external_mod_state(
    game_id: String,
    profile_id: String,
    mod_id: String,
    state: State<'_, AppState>,
) -> Result<ExternalModStateDto, CommandErrorDto> {
    let (game_id, profile_id, mod_id) = parse_external_state_ids(game_id, profile_id, mod_id)?;

    let query = state
        .external_state_scanner
        .query(&game_id, &profile_id, &mod_id);
    // 占用者显示名随查随取（get_mod_detail 取名链：analysis 取名 + 用户改名覆盖）；
    // 解析失败按「没有名字」处理——归因事实（id）仍然如实返回，前端回退显示 id。
    Ok(ExternalModStateDto::from_query(query, |claimant_id| {
        state
            .mod_library
            .get_mod_detail(claimant_id)
            .ok()
            .flatten()
            .map(|detail| detail.name)
    }))
}

/// 接管命令（#286 adopt，写清单）与扫描共用同一套 ID 校验——两者是同一族命令。
pub(crate) fn parse_external_state_ids(
    game_id: String,
    profile_id: String,
    mod_id: String,
) -> Result<(GameId, ProfileId, ModId), CommandErrorDto> {
    // `GameId::parse` 是它唯一的公开构造，自带格式校验（与 install_commands 同口径）。
    let game_id = GameId::parse(game_id).map_err(|_| CommandErrorDto {
        code: "external_state_game_id_invalid".to_owned(),
        message: "external state game id is invalid".to_owned(),
    })?;
    let profile_id = ProfileId::new(parse_external_state_id(
        profile_id,
        "external_state_profile_id_invalid",
        "external state profile id is invalid",
    )?);
    let mod_id = ModId::new(parse_external_state_id(
        mod_id,
        "external_state_mod_id_invalid",
        "external state mod id is invalid",
    )?);
    Ok((game_id, profile_id, mod_id))
}

pub(crate) fn parse_external_state_id(
    value: String,
    code: &'static str,
    message: &'static str,
) -> Result<String, CommandErrorDto> {
    let trimmed = value.trim();
    if trimmed.is_empty()
        || trimmed.len() > EXTERNAL_STATE_ID_MAX_LENGTH
        || !trimmed
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err(CommandErrorDto {
            code: code.to_owned(),
            message: message.to_owned(),
        });
    }
    Ok(trimmed.to_owned())
}

fn spawn_external_state_scan_runner(
    service: Arc<ExternalStateScanTaskService>,
    task_manager: Arc<TaskManager>,
    app_handle: AppHandle,
    launch: ExternalStateScanTaskLaunch,
) {
    std::thread::spawn(move || {
        let task_id = launch.task.task_id.clone();
        let task_kind = launch.task.kind;
        let mod_id = launch.mod_id.as_str().to_owned();
        let events = match service.run_scan(launch) {
            Ok(events) => events,
            Err(error) => vec![fallback_scan_terminal_event(
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
fn fallback_scan_terminal_event(
    task_manager: &TaskManager,
    task_id: String,
    task_kind: TaskKind,
    mod_id: String,
    error: ExternalStateScanTaskError,
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
            EXTERNAL_STATE_SCAN_CANCELLED_PHASE
        } else {
            EXTERNAL_STATE_SCAN_FAILED_PHASE
        },
    );
    event.result_ref = Some(mod_id);
    if status != TaskStatus::Cancelled {
        event.error = Some(error.code().to_owned());
    }
    event
}

fn external_state_task_error(error: ExternalStateScanTaskError) -> CommandErrorDto {
    CommandErrorDto {
        code: error.code().to_owned(),
        message: "external mod state scan task is unavailable".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn id_parser_rejects_paths_and_trims_valid_ids() {
        assert!(parse_external_state_id(String::new(), "code", "message").is_err());
        assert!(parse_external_state_id("  ".to_owned(), "code", "message").is_err());
        assert!(parse_external_state_id("a/b".to_owned(), "code", "message").is_err());
        assert!(parse_external_state_id("a\\b".to_owned(), "code", "message").is_err());
        assert!(parse_external_state_id("..".to_owned(), "code", "message").is_err());
        assert!(parse_external_state_id("函数".to_owned(), "code", "message").is_err());
        assert!(
            parse_external_state_id("a".repeat(EXTERNAL_STATE_ID_MAX_LENGTH + 1), "code", "m")
                .is_err()
        );

        let parsed = parse_external_state_id("  mod-import-42_a  ".to_owned(), "code", "message")
            .expect("valid id");
        assert_eq!(parsed, "mod-import-42_a");
    }

    #[test]
    fn fallback_event_marks_a_stuck_task_failed_with_the_stable_code() {
        let task_manager = TaskManager::new();
        let task = task_manager
            .create_task(TaskKind::ExternalStateScan)
            .expect("create task");

        let event = fallback_scan_terminal_event(
            &task_manager,
            task.task_id.clone(),
            task.kind,
            "mod-a".to_owned(),
            ExternalStateScanTaskError::TaskUnavailable,
        );

        assert_eq!(
            task_manager.task_status(&task.task_id),
            Some(TaskStatus::Failed)
        );
        assert_eq!(event.status, TaskStatus::Failed);
        assert_eq!(event.phase, EXTERNAL_STATE_SCAN_FAILED_PHASE);
        assert_eq!(event.result_ref.as_deref(), Some("mod-a"));
        assert_eq!(
            event.error.as_deref(),
            Some("external_state_scan_task_unavailable")
        );
    }

    #[test]
    fn fallback_event_respects_a_cancelled_task_instead_of_forcing_failed() {
        let task_manager = TaskManager::new();
        let task = task_manager
            .create_task(TaskKind::ExternalStateScan)
            .expect("create task");
        task_manager
            .cancel_task(&task.task_id)
            .expect("cancel task");

        let event = fallback_scan_terminal_event(
            &task_manager,
            task.task_id.clone(),
            task.kind,
            "mod-a".to_owned(),
            ExternalStateScanTaskError::TaskUnavailable,
        );

        assert_eq!(event.status, TaskStatus::Cancelled);
        assert_eq!(event.phase, EXTERNAL_STATE_SCAN_CANCELLED_PHASE);
        // 取消不是失败：不得给取消终态贴错误码。
        assert_eq!(event.error, None);
    }
}
