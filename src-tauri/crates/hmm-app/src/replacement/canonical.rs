use super::*;
use crate::{
    InstallPlanningService, ReinstallCandidatePlanError, ReinstallCandidatePlanRequest,
    ReinstallCandidatePlanner,
};

impl ReplacementWorkflowService {
    /// 普通安装先确定不可变 revision，随后计划、来源记录和提交共用该版本。
    pub fn current_install_revision(
        &self,
        mod_id: &ModId,
    ) -> Result<ModRevisionId, ReplacementWorkflowError> {
        self.result_repository
            .get_mod(mod_id)
            .map_err(|_| ReplacementWorkflowError::ModRepositoryUnavailable)?
            .map(|item| item.display_revision_id)
            .ok_or(ReplacementWorkflowError::ModNotFound)
    }

    /// 仅供普通安装计划使用：按最终实际 provider 建立原位事实，不改变文件集合或内容策略。
    pub fn bind_canonical_install_sources(
        &self,
        game_id: &GameId,
        profile_id: &ProfileId,
        mod_id: &ModId,
        revision_id: &ModRevisionId,
        plan: InstallPlan,
    ) -> Result<InstallPlan, ReplacementWorkflowError> {
        if !plan.replacement_bindings.is_empty() {
            return Err(ReplacementWorkflowError::BindingUnavailable);
        }
        self.result_repository
            .get_revision(revision_id)
            .map_err(|_| ReplacementWorkflowError::ModRepositoryUnavailable)?
            .filter(|revision| revision.mod_id == *mod_id)
            .ok_or(ReplacementWorkflowError::RevisionNotFound)?;
        if plan.actions.is_empty() || plan.has_blocking_conflicts() {
            return Ok(plan);
        }
        let mut file_ids = BTreeSet::new();
        let mut assets = Vec::with_capacity(plan.actions.len());
        for action in &plan.actions {
            if action.provider.mod_id != *mod_id
                || action.target_path != action.provider.target_path
                || !file_ids.insert(action.provider.package_file_id.clone())
            {
                return Err(ReplacementWorkflowError::PlanUnavailable);
            }
            assets.push(ReplacementAsset::new(
                action.provider.package_file_id.clone(),
                action.target_path.as_str(),
            ));
        }
        let Ok(analysis) = self.replacement.analyze(ReplacementAnalysisRequest {
            game_id: game_id.clone(),
            assets,
        }) else {
            // 来源描述不可用不改变普通安装能力；后续重定向仍要求可信来源事实。
            return Ok(plan);
        };
        let mut snapshots = Vec::with_capacity(analysis.sources().len());
        for source in analysis.sources() {
            if !source.is_supported() || source.game_id() != game_id {
                continue;
            }
            let Ok(target) = self.self_target_for(game_id, source) else {
                continue;
            };
            if target.game_id() != game_id
                || target.target_type() != source.source_type()
                || target.internal_id() != source.internal_id()
                || target
                    .metadata()
                    .get("path_family")
                    .and_then(serde_json::Value::as_str)
                    != Some(source.path_family())
            {
                continue;
            }
            let binding = ReplacementBinding::new(
                canonical_source_binding_id(game_id, profile_id, mod_id, source.id(), target.id())?,
                mod_id.clone(),
                profile_id.clone(),
                source.id().clone(),
                target.id().clone(),
                0,
            )
            .map_err(|_| ReplacementWorkflowError::BindingUnavailable)?;
            // 普通安装未执行 transformer 或重定向排除政策，不填造这些策略的事实和计数。
            snapshots.push(
                ReplacementBindingSnapshot::new(
                    binding,
                    Some(revision_id.clone()),
                    source.internal_id(),
                    source.internal_id(),
                    source.path_family(),
                    source.path_family(),
                    source.source_type().clone(),
                )
                .map_err(|_| ReplacementWorkflowError::BindingUnavailable)?,
            );
        }
        plan.with_replacement_bindings(snapshots)
            .map_err(|_| ReplacementWorkflowError::PlanUnavailable)
    }
}

/// 普通重装与普通首次安装共用原位事实生成；目标切换仍使用自己的完整候选计划。
pub struct CanonicalReinstallPlanner {
    planning: Arc<InstallPlanningService>,
    replacement: Arc<ReplacementWorkflowService>,
}

impl CanonicalReinstallPlanner {
    pub fn new(
        planning: Arc<InstallPlanningService>,
        replacement: Arc<ReplacementWorkflowService>,
    ) -> Self {
        Self {
            planning,
            replacement,
        }
    }
}

impl ReinstallCandidatePlanner for CanonicalReinstallPlanner {
    fn build_candidate_plan(
        &self,
        request: ReinstallCandidatePlanRequest<'_>,
    ) -> Result<InstallPlan, ReinstallCandidatePlanError> {
        let (game_id, profile_id, mod_id, revision_id) = (
            request.game_id,
            request.profile_id,
            request.mod_id,
            &request.candidate.revision_id,
        );
        let plan = self.planning.build_candidate_plan(request)?;
        self.replacement
            .bind_canonical_install_sources(game_id, profile_id, mod_id, revision_id, plan)
            .map_err(|_| ReinstallCandidatePlanError::NotReady)
    }
}
