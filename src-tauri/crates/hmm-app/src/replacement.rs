use hmm_core::{
    FileLayer, InstallFileProvider, InstallPlan, InstallPlanValidationError, ModRevisionId,
    ReplacementAnalysis, ReplacementBindingSnapshot, RetargetPlan,
};
use hmm_ports::{
    ReplacementAdapter, ReplacementAdapterError, ReplacementAnalysisRequest, RetargetPlanRequest,
    RetargetStagingError, RetargetStagingFile, RetargetStagingMaterializer,
};
use std::sync::Arc;
use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ReplacementServiceError {
    #[error("replacement is unsupported for the requested game")]
    UnsupportedGame,
    #[error("replacement adapter failed")]
    Adapter(#[from] ReplacementAdapterError),
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum RetargetMaterializeError {
    #[error("retarget install plan is invalid")]
    InvalidInstallPlan(#[from] InstallPlanValidationError),
    #[error("retarget staging failed")]
    Staging(#[from] RetargetStagingError),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaterializeRetargetRequest {
    pub plan: RetargetPlan,
    pub layer: FileLayer,
    pub revision_id: Option<ModRevisionId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaterializedRetarget {
    retarget_plan: RetargetPlan,
    install_plan: InstallPlan,
}

impl MaterializedRetarget {
    pub fn retarget_plan(&self) -> &RetargetPlan {
        &self.retarget_plan
    }

    pub fn install_plan(&self) -> &InstallPlan {
        &self.install_plan
    }

    pub fn into_parts(self) -> (RetargetPlan, InstallPlan) {
        (self.retarget_plan, self.install_plan)
    }
}

pub struct ReplacementService {
    adapters: Vec<Arc<dyn ReplacementAdapter>>,
}

impl ReplacementService {
    pub fn new(adapters: Vec<Arc<dyn ReplacementAdapter>>) -> Self {
        Self { adapters }
    }

    pub fn analyze(
        &self,
        request: ReplacementAnalysisRequest,
    ) -> Result<ReplacementAnalysis, ReplacementServiceError> {
        let adapter = self.adapter_for(&request.game_id)?;
        adapter
            .analyze_replacement_assets(request)
            .map_err(Into::into)
    }

    pub fn build_retarget_plan(
        &self,
        request: RetargetPlanRequest,
    ) -> Result<RetargetPlan, ReplacementServiceError> {
        let adapter = self.adapter_for(&request.game_id)?;
        adapter.build_retarget_plan(request).map_err(Into::into)
    }

    pub fn materialize_retarget(
        &self,
        staging: &dyn RetargetStagingMaterializer,
        request: MaterializeRetargetRequest,
    ) -> Result<MaterializedRetarget, RetargetMaterializeError> {
        let snapshot =
            ReplacementBindingSnapshot::from_retarget_plan(&request.plan, request.revision_id);
        let staging_files = request
            .plan
            .actions()
            .iter()
            .map(|action| {
                RetargetStagingFile::new(
                    action.package_file_id().clone(),
                    action.target_relative_path().clone(),
                )
            })
            .collect::<Vec<_>>();
        let providers = request.plan.actions().iter().map(|action| {
            InstallFileProvider::new(
                request.plan.binding().mod_id().clone(),
                action.package_file_id().clone(),
                action.target_relative_path().clone(),
                request.layer.clone(),
            )
        });
        let install_plan =
            InstallPlan::from_providers(providers).with_replacement_bindings(vec![snapshot])?;

        staging.materialize(&staging_files)?;

        Ok(MaterializedRetarget {
            retarget_plan: request.plan,
            install_plan,
        })
    }

    fn adapter_for(
        &self,
        game_id: &hmm_core::GameId,
    ) -> Result<Arc<dyn ReplacementAdapter>, ReplacementServiceError> {
        self.adapters
            .iter()
            .find(|adapter| adapter.game_id() == *game_id)
            .cloned()
            .ok_or(ReplacementServiceError::UnsupportedGame)
    }
}
