//! MHW 装备资源与包内材质引用同步迁移，旧落点由安装事务按清单处理。
mod file_effects;
mod identity;
mod inventory;
mod material_plan;
mod material_table;
mod material_transform;
mod numbered_identity;
mod path_strategy;
mod resource_path;
pub(crate) use identity::original_target_identity;
pub use material_transform::MhwEquipmentMrl3TexturePathTransformer;

use hmm_core::{GameId, ReplacementAnalysis, RetargetPlan};
use hmm_ports::{
    ReplacementAdapter, ReplacementAdapterError, ReplacementAdapterResult,
    ReplacementAnalysisRequest, ReplacementAssetContentReader, RetargetPlanRequest,
};

#[derive(Debug, Clone, Copy, Default)]
pub struct MhwReplacementAdapter;

impl ReplacementAdapter for MhwReplacementAdapter {
    fn game_id(&self) -> GameId {
        GameId::mhw()
    }

    fn analyze_replacement_assets(
        &self,
        request: ReplacementAnalysisRequest,
    ) -> ReplacementAdapterResult<ReplacementAnalysis> {
        ensure_game(&request.game_id)?;
        inventory::PackageResources::classify(&request.assets)?.analysis()
    }

    fn build_retarget_plan(
        &self,
        request: RetargetPlanRequest,
    ) -> ReplacementAdapterResult<RetargetPlan> {
        ensure_game(&request.game_id)?;
        let plan = path_strategy::build_plan(request)?;
        if material_plan::requires_content(std::slice::from_ref(&plan)) {
            return Err(ReplacementAdapterError::SourceContentUnavailable);
        }
        Ok(plan)
    }

    fn build_retarget_plan_with_content(
        &self,
        request: RetargetPlanRequest,
        content_reader: &dyn ReplacementAssetContentReader,
    ) -> ReplacementAdapterResult<RetargetPlan> {
        self.build_retarget_plans_with_content(vec![request], content_reader)?
            .pop()
            .ok_or(ReplacementAdapterError::InvalidRetargetPlan)
    }

    fn build_retarget_plans_with_content(
        &self,
        requests: Vec<RetargetPlanRequest>,
        content_reader: &dyn ReplacementAssetContentReader,
    ) -> ReplacementAdapterResult<Vec<RetargetPlan>> {
        let plans = requests
            .into_iter()
            .map(|request| {
                ensure_game(&request.game_id)?;
                path_strategy::build_plan(request)
            })
            .collect::<ReplacementAdapterResult<Vec<_>>>()?;
        material_plan::complete(plans, content_reader)
    }
}

fn ensure_game(game: &GameId) -> ReplacementAdapterResult<()> {
    if *game == GameId::mhw() {
        Ok(())
    } else {
        Err(ReplacementAdapterError::UnsupportedGame)
    }
}
