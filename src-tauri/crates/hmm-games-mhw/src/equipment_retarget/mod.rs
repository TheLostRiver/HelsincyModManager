//! MHW 默认重定向：只改资源路径，保留材质字节、贴图位置和未映射的资源。
mod identity;
mod inventory;
mod path_strategy;
pub(crate) use identity::original_target_identity;

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
        path_strategy::build_plan(request)
    }

    fn build_retarget_plan_with_content(
        &self,
        request: RetargetPlanRequest,
        _: &dyn ReplacementAssetContentReader,
    ) -> ReplacementAdapterResult<RetargetPlan> {
        self.build_retarget_plan(request)
    }
}

fn ensure_game(game: &GameId) -> ReplacementAdapterResult<()> {
    if *game == GameId::mhw() {
        Ok(())
    } else {
        Err(ReplacementAdapterError::UnsupportedGame)
    }
}
