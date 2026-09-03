//! 外部 MOD 接管（#286 adopt）的任务编排：queued → runner 跑活 → 事件序列。
//!
//! 与 `external_state_scan_tasks` 同构。事件只承载任务身份、phase 与稳定错误码——
//! 接管的结果只有几个计数，且成功即等于用户确认的预览（否则 stale 拒绝），
//! 所以不需要单独的 getter；`resultRef` 只带 opaque `modId`。

use std::sync::Arc;

use crate::external_mod_adopt::{
    ConfiguredExternalModAdoptError, ConfiguredExternalModAdoptRequest,
    ConfiguredExternalModAdopter,
};
use hmm_app::{
    TaskKind, TaskManager, TaskManagerError, TaskProgressEvent, TaskStarted, TaskStatus,
};
use hmm_core::{FileLayer, GameId, ModId, ProfileId};
use hmm_ports::CancellationToken;

pub const EXTERNAL_MOD_ADOPT_QUEUED_PHASE: &str = "external_mod.adopt.queued";
pub const EXTERNAL_MOD_ADOPT_PROCESSING_PHASE: &str = "external_mod.adopt.processing";
pub const EXTERNAL_MOD_ADOPT_COMPLETED_PHASE: &str = "external_mod.adopt.completed";
pub const EXTERNAL_MOD_ADOPT_FAILED_PHASE: &str = "external_mod.adopt.failed";
pub const EXTERNAL_MOD_ADOPT_CANCELLED_PHASE: &str = "external_mod.adopt.cancelled";

/// 清单已写成、审计却没写进去时挂在 completed 事件上的显式降级码（与 install 同口径）。
pub const EXTERNAL_MOD_ADOPT_AUDIT_UNAVAILABLE_CODE: &str = "external_mod_adopt_audit_unavailable";

/// 一次已登记、待运行的接管。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalModAdoptTaskLaunch {
    pub task: TaskStarted,
    pub game_id: GameId,
    pub profile_id: ProfileId,
    pub mod_id: ModId,
    pub layer: FileLayer,
}

/// 任务编排自身的失败。接管本身的失败表现为 failed 事件里的稳定错误码。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExternalModAdoptTaskError {
    TaskUnavailable,
}

impl ExternalModAdoptTaskError {
    pub const fn code(self) -> &'static str {
        match self {
            Self::TaskUnavailable => "external_mod_adopt_task_unavailable",
        }
    }
}

pub struct ExternalModAdoptTaskService {
    task_manager: Arc<TaskManager>,
    adopter: Arc<ConfiguredExternalModAdopter>,
}

impl ExternalModAdoptTaskService {
    pub fn new(task_manager: Arc<TaskManager>, adopter: Arc<ConfiguredExternalModAdopter>) -> Self {
        Self {
            task_manager,
            adopter,
        }
    }

    /// 登记一个 queued 任务。**不做任何 IO**——真正的活在 [`Self::run_adopt`]。
    pub fn start_adopt(
        &self,
        game_id: GameId,
        profile_id: ProfileId,
        mod_id: ModId,
        layer: FileLayer,
    ) -> Result<ExternalModAdoptTaskLaunch, ExternalModAdoptTaskError> {
        let task = self
            .task_manager
            .create_task(TaskKind::ExternalModAdopt)
            .map_err(map_task_manager_error)?;
        Ok(ExternalModAdoptTaskLaunch {
            task: TaskStarted {
                task_id: task.task_id,
                kind: task.kind,
                status: task.status,
            },
            game_id,
            profile_id,
            mod_id,
            layer,
        })
    }

    /// 关闭一个没能交给 runner 的 launch（queued 事件发不出去时），避免泄漏永远 queued 的任务。
    pub fn abort_queued_adopt(
        &self,
        launch: &ExternalModAdoptTaskLaunch,
    ) -> Result<(), ExternalModAdoptTaskError> {
        match self.task_manager.task_status(&launch.task.task_id) {
            Some(TaskStatus::Queued | TaskStatus::Running) => self
                .task_manager
                .fail_task(&launch.task.task_id)
                .map(|_| ())
                .map_err(map_task_manager_error),
            Some(TaskStatus::Failed | TaskStatus::Cancelled | TaskStatus::Completed) => Ok(()),
            None => Err(ExternalModAdoptTaskError::TaskUnavailable),
        }
    }

    /// 跑接管并产出事件序列。只有任务编排本身坏掉才返回 `Err`；接管失败、取消都是终态事件。
    pub fn run_adopt(
        &self,
        launch: ExternalModAdoptTaskLaunch,
    ) -> Result<Vec<TaskProgressEvent>, ExternalModAdoptTaskError> {
        if self.is_cancelled(&launch.task.task_id) {
            return Ok(vec![cancelled_event(&launch)]);
        }
        match self.task_manager.start_task(&launch.task.task_id) {
            Ok(_) => {}
            Err(_) if self.is_cancelled(&launch.task.task_id) => {
                return Ok(vec![cancelled_event(&launch)]);
            }
            Err(error) => return Err(map_task_manager_error(error)),
        }

        let mut events = vec![adopt_event(
            &launch,
            TaskStatus::Running,
            EXTERNAL_MOD_ADOPT_PROCESSING_PHASE,
        )];

        let cancellation_token = TaskManagerCancellationToken {
            task_manager: Arc::clone(&self.task_manager),
            task_id: launch.task.task_id.clone(),
        };
        let outcome = self.adopter.adopt(ConfiguredExternalModAdoptRequest {
            task_id: &launch.task.task_id,
            game_id: &launch.game_id,
            profile_id: &launch.profile_id,
            mod_id: &launch.mod_id,
            layer: &launch.layer,
            cancellation_token: &cancellation_token,
        });

        match outcome {
            Ok(outcome) => {
                // 清单已经写成：哪怕任务此刻被标成 cancelled，也不能把已提交的事实说成没发生。
                // 提交屏障保证 cancel 只能在 save_manifest 之后落地，complete_task 会如实报出。
                match self.task_manager.complete_task(&launch.task.task_id) {
                    Ok(_) => {}
                    Err(_) if self.is_cancelled(&launch.task.task_id) => {
                        events.push(cancelled_event(&launch));
                        return Ok(events);
                    }
                    Err(error) => return Err(map_task_manager_error(error)),
                }
                let mut event = adopt_event(
                    &launch,
                    TaskStatus::Completed,
                    EXTERNAL_MOD_ADOPT_COMPLETED_PHASE,
                );
                if outcome.audit_degraded {
                    event.error = Some(EXTERNAL_MOD_ADOPT_AUDIT_UNAVAILABLE_CODE.to_owned());
                }
                events.push(event);
                Ok(events)
            }
            Err(error) => {
                if error == ConfiguredExternalModAdoptError::Cancelled
                    || cancellation_token.is_cancelled()
                {
                    events.push(cancelled_event(&launch));
                    return Ok(events);
                }
                if matches!(
                    self.task_manager.task_status(&launch.task.task_id),
                    Some(TaskStatus::Queued | TaskStatus::Running)
                ) {
                    self.task_manager
                        .fail_task(&launch.task.task_id)
                        .map_err(map_task_manager_error)?;
                }
                let mut event =
                    adopt_event(&launch, TaskStatus::Failed, EXTERNAL_MOD_ADOPT_FAILED_PHASE);
                event.error = Some(error.code().to_owned());
                events.push(event);
                Ok(events)
            }
        }
    }

    fn is_cancelled(&self, task_id: &str) -> bool {
        self.task_manager.task_status(task_id) == Some(TaskStatus::Cancelled)
    }
}

/// 命令层在返回任务身份前发出的 queued 事件。
pub fn queued_adopt_event(launch: &ExternalModAdoptTaskLaunch) -> TaskProgressEvent {
    adopt_event(launch, launch.task.status, EXTERNAL_MOD_ADOPT_QUEUED_PHASE)
}

fn cancelled_event(launch: &ExternalModAdoptTaskLaunch) -> TaskProgressEvent {
    adopt_event(
        launch,
        TaskStatus::Cancelled,
        EXTERNAL_MOD_ADOPT_CANCELLED_PHASE,
    )
}

fn adopt_event(
    launch: &ExternalModAdoptTaskLaunch,
    status: TaskStatus,
    phase: &'static str,
) -> TaskProgressEvent {
    let mut event =
        TaskProgressEvent::new(launch.task.task_id.clone(), launch.task.kind, status, phase);
    event.result_ref = Some(launch.mod_id.as_str().to_owned());
    event
}

fn map_task_manager_error(_error: TaskManagerError) -> ExternalModAdoptTaskError {
    ExternalModAdoptTaskError::TaskUnavailable
}

struct TaskManagerCancellationToken {
    task_manager: Arc<TaskManager>,
    task_id: String,
}

impl CancellationToken for TaskManagerCancellationToken {
    fn is_cancelled(&self) -> bool {
        self.task_manager.task_status(&self.task_id) == Some(TaskStatus::Cancelled)
    }
}
