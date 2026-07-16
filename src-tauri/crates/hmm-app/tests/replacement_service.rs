use hmm_app::{MaterializeRetargetRequest, ReplacementService, ReplacementServiceError};
use hmm_core::{
    FileLayer, GameId, InstallPlan, InstallTargetPath, ModId, PackageFileId, ProfileId,
    ReplacementAnalysis, ReplacementBinding, ReplacementBindingId, ReplacementSource,
    ReplacementSourceId, ReplacementTargetId, ReplacementTargetKind, RetargetAction, RetargetPlan,
};
use hmm_ports::{
    ReplacementAdapter, ReplacementAdapterError, ReplacementAnalysisRequest, ReplacementAsset,
    RetargetPlanRequest, RetargetStagingError, RetargetStagingFile, RetargetStagingMaterializer,
};
use std::sync::{Arc, Mutex};

struct FakeAdapter;

impl ReplacementAdapter for FakeAdapter {
    fn game_id(&self) -> GameId {
        GameId::mhw()
    }

    fn analyze_replacement_assets(
        &self,
        request: ReplacementAnalysisRequest,
    ) -> Result<ReplacementAnalysis, ReplacementAdapterError> {
        let source = source();
        ReplacementAnalysis::new(
            request.game_id,
            vec![source],
            request.assets.len(),
            Vec::new(),
        )
        .map_err(|_| ReplacementAdapterError::InvalidRetargetPlan)
    }

    fn build_retarget_plan(
        &self,
        request: RetargetPlanRequest,
    ) -> Result<RetargetPlan, ReplacementAdapterError> {
        build_plan(
            request.binding.mod_id().as_str(),
            request.binding.profile_id().as_str(),
        )
        .map_err(|_| ReplacementAdapterError::InvalidRetargetPlan)
    }
}

#[derive(Default)]
struct RecordingStaging {
    batches: Mutex<Vec<Vec<RetargetStagingFile>>>,
}

impl RetargetStagingMaterializer for RecordingStaging {
    fn materialize(&self, files: &[RetargetStagingFile]) -> Result<(), RetargetStagingError> {
        self.batches.lock().expect("batches").push(files.to_vec());
        Ok(())
    }
}

fn source() -> ReplacementSource {
    ReplacementSource::new(
        ReplacementSourceId::parse("mhw:armor:f_equip:pl121_0000").expect("source id"),
        GameId::mhw(),
        ReplacementTargetKind::parse("armor").expect("kind"),
        "pl121_0000",
        "pl/f_equip",
        true,
    )
    .expect("source")
}

fn build_plan(mod_id: &str, profile_id: &str) -> Result<RetargetPlan, hmm_core::RetargetError> {
    let source = source();
    let binding = ReplacementBinding::new(
        ReplacementBindingId::parse(format!("binding-{mod_id}")).expect("binding id"),
        ModId::new(mod_id),
        ProfileId::new(profile_id),
        source.id().clone(),
        ReplacementTargetId::parse("mhw:armor:fatalis-alpha").expect("target id"),
        42,
    )
    .expect("binding");
    let action = RetargetAction::new(
        PackageFileId::new(format!("package-{mod_id}-body")),
        InstallTargetPath::parse(
            "nativePC/pl/f_equip/pl121_0000/arm/mod/f_body.mod3",
            ["nativePC"],
        )
        .expect("source path"),
        InstallTargetPath::parse(
            "nativePC/pl/f_equip/pl129_0000/arm/mod/f_body.mod3",
            ["nativePC"],
        )
        .expect("target path"),
        source.id().clone(),
        source.internal_id(),
        "pl129_0000",
        source.path_family(),
        "pl/f_equip",
    )?;
    RetargetPlan::new(binding, source, vec![action], Vec::new())
}

#[test]
fn replacement_service_routes_analysis_by_game() {
    let service = ReplacementService::new(vec![Arc::new(FakeAdapter)]);
    let analysis = service
        .analyze(ReplacementAnalysisRequest {
            game_id: GameId::mhw(),
            assets: vec![ReplacementAsset::new(
                PackageFileId::new("body"),
                "nativePC/pl/f_equip/pl121_0000/arm/mod/f_body.mod3",
            )],
        })
        .expect("analysis");

    assert_eq!(analysis.matched_asset_count(), 1);

    let unsupported = ReplacementService::new(Vec::new())
        .analyze(ReplacementAnalysisRequest {
            game_id: GameId::mhw(),
            assets: Vec::new(),
        })
        .expect_err("unsupported game");
    assert_eq!(unsupported, ReplacementServiceError::UnsupportedGame);
}

#[test]
fn materialize_preserves_package_identity_and_uses_final_target_for_install_plan() {
    let staging = RecordingStaging::default();
    let materialized = ReplacementService::new(Vec::new())
        .materialize_retarget(
            &staging,
            MaterializeRetargetRequest {
                plan: build_plan("mod-a", "profile-a").expect("plan"),
                layer: FileLayer::new("base", 0),
                revision_id: None,
            },
        )
        .expect("materialize");

    let batches = staging.batches.lock().expect("batches");
    assert_eq!(batches.len(), 1);
    assert_eq!(
        batches[0][0].package_file_id().as_str(),
        "package-mod-a-body"
    );
    assert_eq!(
        batches[0][0].target_path().as_str(),
        "nativePC/pl/f_equip/pl129_0000/arm/mod/f_body.mod3"
    );
    assert_eq!(
        materialized.install_plan().actions[0]
            .provider
            .package_file_id
            .as_str(),
        "package-mod-a-body"
    );
    assert_eq!(
        materialized.install_plan().actions[0].target_path.as_str(),
        "nativePC/pl/f_equip/pl129_0000/arm/mod/f_body.mod3"
    );
    assert_eq!(materialized.install_plan().replacement_bindings.len(), 1);
}

#[test]
fn final_retarget_path_remains_the_install_conflict_key() {
    let service = ReplacementService::new(Vec::new());
    let staging = RecordingStaging::default();
    let first = service
        .materialize_retarget(
            &staging,
            MaterializeRetargetRequest {
                plan: build_plan("mod-a", "profile-a").expect("first plan"),
                layer: FileLayer::new("base", 0),
                revision_id: None,
            },
        )
        .expect("first materialization");
    let second = service
        .materialize_retarget(
            &staging,
            MaterializeRetargetRequest {
                plan: build_plan("mod-b", "profile-a").expect("second plan"),
                layer: FileLayer::new("base", 0),
                revision_id: None,
            },
        )
        .expect("second materialization");
    let providers = first
        .install_plan()
        .actions
        .iter()
        .chain(second.install_plan().actions.iter())
        .map(|action| action.provider.clone());
    let combined = InstallPlan::from_providers(providers);

    assert!(combined.has_blocking_conflicts());
    assert_eq!(
        combined.conflicts[0].target_path.as_str(),
        "nativePC/pl/f_equip/pl129_0000/arm/mod/f_body.mod3"
    );
    assert!(!combined.conflicts[0]
        .target_path
        .as_str()
        .contains("pl121_0000"));
}
