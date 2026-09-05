use hmm_core::{
    GameId, ModId, ProfileId, ReplacementAnalysis, ReplacementBinding, ReplacementBindingId,
    ReplacementSource, ReplacementSourceId, ReplacementTargetId, ReplacementTargetKind,
    RetargetPlan,
};
use hmm_ports::{
    ReplacementAdapter, ReplacementAdapterError, ReplacementAdapterResult,
    ReplacementAnalysisRequest, RetargetPlanRequest,
};

struct FakeReplacementAdapter;

impl ReplacementAdapter for FakeReplacementAdapter {
    fn game_id(&self) -> GameId {
        GameId::mhw()
    }

    fn analyze_replacement_assets(
        &self,
        _request: ReplacementAnalysisRequest,
    ) -> ReplacementAdapterResult<ReplacementAnalysis> {
        ReplacementAnalysis::new(GameId::mhw(), Vec::new(), 0, Vec::new())
            .map_err(|_| ReplacementAdapterError::InvalidRetargetPlan)
    }

    fn build_retarget_plan(
        &self,
        request: RetargetPlanRequest,
    ) -> ReplacementAdapterResult<RetargetPlan> {
        let source = ReplacementSource::new(
            request.binding.source_id().clone(),
            GameId::mhw(),
            ReplacementTargetKind::parse("armor").expect("kind"),
            "pl121_0000",
            "pl/f_equip",
            true,
        )
        .expect("source");

        RetargetPlan::new(request.binding, source, Vec::new(), Vec::new())
            .map_err(|_| ReplacementAdapterError::InvalidRetargetPlan)
    }
}

#[test]
fn replacement_adapter_contract_is_separate_from_catalog_and_filesystem_ports() {
    let adapter = FakeReplacementAdapter;
    let analysis = adapter
        .analyze_replacement_assets(ReplacementAnalysisRequest {
            game_id: GameId::mhw(),
            assets: Vec::new(),
        })
        .expect("analysis");

    assert_eq!(adapter.game_id(), GameId::mhw());
    assert!(analysis.sources().is_empty());
}

#[test]
fn replacement_adapter_errors_are_stable_and_do_not_include_paths() {
    let target_id = ReplacementTargetId::parse("mhw:armor:missing").expect("target id");
    let error = ReplacementAdapterError::TargetCatalogMissing {
        target_id: target_id.clone(),
    };

    assert_eq!(
        error.to_string(),
        "replacement target is missing from the catalog: mhw:armor:missing"
    );
    assert_eq!(
        error,
        ReplacementAdapterError::TargetCatalogMissing { target_id }
    );
}

#[test]
fn retarget_plan_request_owns_only_domain_values_and_relative_assets() {
    let binding = ReplacementBinding::new(
        ReplacementBindingId::parse("binding-1").expect("binding id"),
        ModId::new("mod-1"),
        ProfileId::new("profile-1"),
        ReplacementSourceId::parse("mhw:armor:f_equip:pl121_0000").expect("source id"),
        ReplacementTargetId::parse("mhw:armor:fatalis-alpha").expect("target id"),
        42,
    )
    .expect("binding");
    let request = RetargetPlanRequest {
        game_id: GameId::mhw(),
        binding,
        assets: Vec::new(),
        carries_package_companions: true,
    };

    let error = FakeReplacementAdapter
        .build_retarget_plan(request)
        .expect_err("empty actions are not a valid plan");
    assert_eq!(error, ReplacementAdapterError::InvalidRetargetPlan);
}
