use super::*;
use crate::EquipmentRetargetReinstallRequest;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartEquipmentRetargetReinstallTaskRequest {
    pub selection: EquipmentRetargetReinstallRequest,
    pub plan_token: String,
}

pub trait EquipmentRetargetReinstallTaskExecutor: ReinstallTaskExecutor {
    fn prepare_equipment_retarget_reinstall(
        &self,
        request: EquipmentRetargetReinstallRequest,
    ) -> Result<Self::Prepared, ReinstallTaskPrepareError>;
}

impl ReinstallTaskRequestContext for StartEquipmentRetargetReinstallTaskRequest {
    fn game_id(&self) -> &GameId {
        &self.selection.game_id
    }
    fn profile_id(&self) -> &ProfileId {
        &self.selection.profile_id
    }
    fn mod_id(&self) -> &ModId {
        &self.selection.mod_id
    }
    // 组合没有单个 target_id；完整绑定与目标保存在候选计划、事务和 manifest 中。
    fn target_id(&self) -> Option<&ReplacementTargetId> {
        None
    }
}

impl ReinstallTaskService {
    pub fn start_equipment_retarget_reinstall_task(
        &self,
        _request: StartEquipmentRetargetReinstallTaskRequest,
    ) -> Result<TaskStarted, TaskManagerError> {
        let task = self.task_manager.create_task(TaskKind::Install)?;
        Ok(TaskStarted {
            task_id: task.task_id,
            kind: task.kind,
            status: task.status,
        })
    }
}

impl<E: EquipmentRetargetReinstallTaskExecutor> ReinstallTaskRunner<E> {
    pub(crate) fn verify_equipment_reapply_noop(
        &self,
        request: EquipmentRetargetReinstallRequest,
        expected_digest: &str,
    ) -> bool {
        if request.intent != hmm_core::ReinstallIntent::ReapplyEquipmentTargets {
            return false;
        }
        let Ok(prepared) = self.executor.prepare_equipment_retarget_reinstall(request) else {
            return false;
        };
        let counts = prepared.audit_context().counts;
        prepared.batch_plan_digest() == expected_digest
            && counts.added == 0
            && counts.replaced == 0
            && counts.stale == 0
    }

    pub(crate) fn run_equipment_retarget_reinstall_task_for_orchestration_with_observer<
        O: TaskProgressObserver + ?Sized,
    >(
        &self,
        task_id: &str,
        request: StartEquipmentRetargetReinstallTaskRequest,
        expected_batch_plan_digest: &str,
        observer: &O,
    ) -> Result<Vec<TaskProgressEvent>, ReinstallTaskOrchestrationError> {
        let selection = request.selection.clone();
        self.run_task(
            task_id,
            &request,
            observer,
            || {
                self.executor
                    .prepare_equipment_retarget_reinstall(selection)
            },
            |prepared| orchestration_plan_token(prepared, expected_batch_plan_digest),
        )
    }

    pub fn run_equipment_retarget_reinstall_task(
        &self,
        task_id: &str,
        request: StartEquipmentRetargetReinstallTaskRequest,
    ) -> Result<Vec<TaskProgressEvent>, ReinstallTaskRunError> {
        self.run_equipment_retarget_reinstall_task_with_observer(
            task_id,
            request,
            &noop_task_progress_observer(),
        )
    }

    pub fn run_equipment_retarget_reinstall_task_with_observer<O: TaskProgressObserver + ?Sized>(
        &self,
        task_id: &str,
        request: StartEquipmentRetargetReinstallTaskRequest,
        observer: &O,
    ) -> Result<Vec<TaskProgressEvent>, ReinstallTaskRunError> {
        let selection = request.selection.clone();
        let token = request.plan_token.clone();
        self.run_task(
            task_id,
            &request,
            observer,
            || {
                self.executor
                    .prepare_equipment_retarget_reinstall(selection)
            },
            |_| Ok(token),
        )
        .map_err(public_run_error)
    }
}
