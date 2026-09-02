//! #286 切片 2b 第 3 步：把外部 MOD 状态扫描接进任务框架。
//!
//! 与 `hmm-app` 的 `ExternalImportScanService` 同构（queued → runner 跑活 → 事件序列），
//! 但**没有持久化 batch 行**：扫描结果在 [`ConfiguredExternalStateScanner`] 的进程内
//! 缓存里。事件只承载任务身份、phase 与稳定错误码——契约禁止进度事件携带
//! `target_path`，而扫描结果正是每文件路径，所以结果一律走 `get_external_mod_state`
//! 查询，绝不进事件 payload。
//!
//! 服务放在 `hmm-runtime` 而不是 `hmm-app`：它编排的是 game-instance 作用域的
//! `Configured*` 扫描器（构造依赖 infra 装配），`hmm-app` 拿不到它。

use std::sync::Arc;

use crate::external_state_scan::{
    ConfiguredExternalStateScanError, ConfiguredExternalStateScanRequest,
    ConfiguredExternalStateScanner,
};
use hmm_app::{
    TaskKind, TaskManager, TaskManagerError, TaskProgressEvent, TaskStarted, TaskStatus,
};
use hmm_core::{GameId, ModId, ProfileId};
use hmm_ports::CancellationToken;

pub const EXTERNAL_STATE_SCAN_QUEUED_PHASE: &str = "external_state.scan.queued";
pub const EXTERNAL_STATE_SCAN_PROCESSING_PHASE: &str = "external_state.scan.processing";
pub const EXTERNAL_STATE_SCAN_COMPLETED_PHASE: &str = "external_state.scan.completed";
pub const EXTERNAL_STATE_SCAN_FAILED_PHASE: &str = "external_state.scan.failed";
pub const EXTERNAL_STATE_SCAN_CANCELLED_PHASE: &str = "external_state.scan.cancelled";

/// 一次已登记、待运行的扫描。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalStateScanTaskLaunch {
    pub task: TaskStarted,
    pub game_id: GameId,
    pub profile_id: ProfileId,
    pub mod_id: ModId,
}

/// 任务编排自身的失败。
///
/// 扫描本身的失败不在这里——它表现为 failed 事件里的稳定错误码
/// （`ConfiguredExternalStateScanError::code()`），且缓存里同时记着原因，
/// getter 能看到。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExternalStateScanTaskError {
    /// TaskManager 不可用或任务状态机拒绝了转移。
    TaskUnavailable,
}

impl ExternalStateScanTaskError {
    pub const fn code(self) -> &'static str {
        match self {
            Self::TaskUnavailable => "external_state_scan_task_unavailable",
        }
    }
}

pub struct ExternalStateScanTaskService {
    task_manager: Arc<TaskManager>,
    scanner: Arc<ConfiguredExternalStateScanner>,
}

impl ExternalStateScanTaskService {
    pub fn new(
        task_manager: Arc<TaskManager>,
        scanner: Arc<ConfiguredExternalStateScanner>,
    ) -> Self {
        Self {
            task_manager,
            scanner,
        }
    }

    /// 登记一个 queued 任务。**不做任何 IO**——真正的活在 [`Self::run_scan`]。
    pub fn start_scan(
        &self,
        game_id: GameId,
        profile_id: ProfileId,
        mod_id: ModId,
    ) -> Result<ExternalStateScanTaskLaunch, ExternalStateScanTaskError> {
        let task = self
            .task_manager
            .create_task(TaskKind::ExternalStateScan)
            .map_err(map_task_manager_error)?;
        Ok(ExternalStateScanTaskLaunch {
            task: TaskStarted {
                task_id: task.task_id,
                kind: task.kind,
                status: task.status,
            },
            game_id,
            profile_id,
            mod_id,
        })
    }

    /// 关闭一个没能交给 runner 的 launch（queued 事件发不出去时）。
    ///
    /// 命令此时还没把任务身份返回给前端，留下一个永远 queued 的任务等于泄漏。
    pub fn abort_queued_scan(
        &self,
        launch: &ExternalStateScanTaskLaunch,
    ) -> Result<(), ExternalStateScanTaskError> {
        match self.task_manager.task_status(&launch.task.task_id) {
            Some(TaskStatus::Queued | TaskStatus::Running) => self
                .task_manager
                .fail_task(&launch.task.task_id)
                .map(|_| ())
                .map_err(map_task_manager_error),
            Some(TaskStatus::Failed | TaskStatus::Cancelled | TaskStatus::Completed) => Ok(()),
            None => Err(ExternalStateScanTaskError::TaskUnavailable),
        }
    }

    /// 跑扫描并产出事件序列。调用方负责线程调度与逐条 emit。
    ///
    /// 只有任务编排本身坏掉（TaskManager 拒绝）才返回 `Err`；扫描失败、取消
    /// 都是**正常产出**——表现为终态事件，调用方无需特殊处理。
    pub fn run_scan(
        &self,
        launch: ExternalStateScanTaskLaunch,
    ) -> Result<Vec<TaskProgressEvent>, ExternalStateScanTaskError> {
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

        let mut events = vec![scan_event(
            &launch,
            TaskStatus::Running,
            EXTERNAL_STATE_SCAN_PROCESSING_PHASE,
        )];

        let cancellation_token = TaskManagerCancellationToken {
            task_manager: Arc::clone(&self.task_manager),
            task_id: launch.task.task_id.clone(),
        };
        let scan_result = self.scanner.scan(ConfiguredExternalStateScanRequest {
            game_id: &launch.game_id,
            profile_id: &launch.profile_id,
            mod_id: &launch.mod_id,
            cancellation_token: &cancellation_token,
        });

        match scan_result {
            Ok(_summary) => {
                // 结果已在扫描器的缓存里；事件不携带它（契约禁 target_path）。
                if cancellation_token.is_cancelled() {
                    events.push(cancelled_event(&launch));
                    return Ok(events);
                }
                match self.task_manager.complete_task(&launch.task.task_id) {
                    Ok(_) => {}
                    // 成功与取消赛跑：结果保留（缓存已写入），任务终态如实为 cancelled。
                    Err(_) if self.is_cancelled(&launch.task.task_id) => {
                        events.push(cancelled_event(&launch));
                        return Ok(events);
                    }
                    Err(error) => return Err(map_task_manager_error(error)),
                }
                events.push(scan_event(
                    &launch,
                    TaskStatus::Completed,
                    EXTERNAL_STATE_SCAN_COMPLETED_PHASE,
                ));
                Ok(events)
            }
            Err(error) => {
                // 取消不是失败：`Cancelled` 只在令牌命中时产生，二者语义一致，
                // 但都查一遍——令牌可能在 scan 返回之后才翻转。
                if matches!(error, ConfiguredExternalStateScanError::Cancelled)
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
                let mut event = scan_event(
                    &launch,
                    TaskStatus::Failed,
                    EXTERNAL_STATE_SCAN_FAILED_PHASE,
                );
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
pub fn queued_scan_event(launch: &ExternalStateScanTaskLaunch) -> TaskProgressEvent {
    scan_event(launch, launch.task.status, EXTERNAL_STATE_SCAN_QUEUED_PHASE)
}

fn cancelled_event(launch: &ExternalStateScanTaskLaunch) -> TaskProgressEvent {
    // 走到这里时任务已被 `cancel_task` 置为 Cancelled（令牌读的就是那个状态），
    // 所以只构造事件，不再做状态转移。
    scan_event(
        launch,
        TaskStatus::Cancelled,
        EXTERNAL_STATE_SCAN_CANCELLED_PHASE,
    )
}

fn scan_event(
    launch: &ExternalStateScanTaskLaunch,
    status: TaskStatus,
    phase: &'static str,
) -> TaskProgressEvent {
    let mut event =
        TaskProgressEvent::new(launch.task.task_id.clone(), launch.task.kind, status, phase);
    // resultRef 是 opaque mod ID：getter 按 (game, profile, mod) 取结果，
    // 事件只需要让前端知道「哪个 MOD 的扫描」。
    event.result_ref = Some(launch.mod_id.as_str().to_owned());
    event
}

fn map_task_manager_error(_error: TaskManagerError) -> ExternalStateScanTaskError {
    ExternalStateScanTaskError::TaskUnavailable
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
