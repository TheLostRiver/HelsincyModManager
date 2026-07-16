use hmm_app::{
    InitialRetargetInstallStatusError, InitialRetargetInstallStatusReader,
    MaterializeRetargetRequest, PreviewInitialRetargetInstallRequest, ReplacementService,
    ReplacementServiceError, ReplacementWorkflowError, ReplacementWorkflowService,
};
use hmm_core::{
    FileLayer, GameId, InstallPlan, InstallTargetPath, LocalizedText, ModId, PackageFileId,
    ProfileId, ReplacementAnalysis, ReplacementBinding, ReplacementBindingId, ReplacementCatalog,
    ReplacementCatalogVersion, ReplacementSource, ReplacementSourceId, ReplacementTarget,
    ReplacementTargetId, ReplacementTargetKind, RetargetAction, RetargetPlan,
};
use hmm_ports::{
    AppClock, ModImportResultRepository, ModImportSandboxLocator, ModPackageInstallFile,
    ModPackageInstallFileScanRequest, ModPackageInstallFileScanner, ReplacementAdapter,
    ReplacementAdapterError, ReplacementAnalysisRequest, ReplacementAsset,
    ReplacementCatalogProvider, ReplacementCatalogResult, RetargetPlanRequest,
    RetargetStagingError, RetargetStagingFile, RetargetStagingMaterializer,
    StoredModImportAnalysis, StoredModPackageMetadata,
};
use std::collections::BTreeMap;
use std::path::PathBuf;
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
        build_plan_for_binding(request.binding)
            .map_err(|_| ReplacementAdapterError::InvalidRetargetPlan)
    }
}

struct FakeCatalog;

impl ReplacementCatalogProvider for FakeCatalog {
    fn game_id(&self) -> GameId {
        GameId::mhw()
    }

    fn replacement_catalog(&self) -> ReplacementCatalogResult<ReplacementCatalog> {
        ReplacementCatalog::new(
            ReplacementCatalogVersion::parse("test-v1").expect("catalog version"),
            GameId::mhw(),
            vec![target()],
        )
        .map_err(|_| hmm_ports::ReplacementCatalogError::CatalogInvalid)
    }

    fn search_replacement_targets(
        &self,
        query: &str,
    ) -> ReplacementCatalogResult<Vec<ReplacementTarget>> {
        Ok(
            (query.contains("黑龙") || query.eq_ignore_ascii_case("fatalis"))
                .then(target)
                .into_iter()
                .collect(),
        )
    }
}

struct FakeRepository;

impl ModImportResultRepository for FakeRepository {
    fn save_analysis(&self, _analysis: &StoredModImportAnalysis) -> anyhow::Result<()> {
        Ok(())
    }

    fn list_analysis(&self) -> anyhow::Result<Vec<StoredModImportAnalysis>> {
        Ok(vec![stored_analysis()])
    }

    fn get_analysis(&self, mod_id: &str) -> anyhow::Result<Option<StoredModImportAnalysis>> {
        Ok((mod_id == "mod-a").then(stored_analysis))
    }
}

struct FakeSandboxLocator;

impl ModImportSandboxLocator for FakeSandboxLocator {
    fn sandbox_root_for_package(&self, package_id: &str) -> anyhow::Result<PathBuf> {
        anyhow::ensure!(package_id == "revision-v1", "unexpected package");
        Ok(PathBuf::from("controlled-test-sandbox"))
    }
}

struct FakeFileScanner;

impl ModPackageInstallFileScanner for FakeFileScanner {
    fn scan_install_files(
        &self,
        request: ModPackageInstallFileScanRequest<'_>,
    ) -> anyhow::Result<Vec<ModPackageInstallFile>> {
        anyhow::ensure!(request.package_id == "revision-v1", "unexpected package");
        Ok(vec![ModPackageInstallFile {
            package_file_id: "nativePC/pl/f_equip/pl121_0000/arm/mod/f_body.mod3".to_owned(),
            target_path: "nativePC/pl/f_equip/pl121_0000/arm/mod/f_body.mod3".to_owned(),
        }])
    }
}

struct FixedStatus(hmm_app::InstallRecoveryStatus);

impl InitialRetargetInstallStatusReader for FixedStatus {
    fn recovery_status(
        &self,
        _game_id: &GameId,
        _profile_id: &ProfileId,
        _mod_id: &ModId,
    ) -> Result<hmm_app::InstallRecoveryStatus, InitialRetargetInstallStatusError> {
        Ok(self.0)
    }
}

struct FixedClock;

impl AppClock for FixedClock {
    fn now_unix_millis(&self) -> anyhow::Result<u128> {
        Ok(42)
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
    build_plan_for_binding(binding)
}

fn build_plan_for_binding(
    binding: ReplacementBinding,
) -> Result<RetargetPlan, hmm_core::RetargetError> {
    let source = source();
    let action = RetargetAction::new(
        PackageFileId::new(format!("package-{}-body", binding.mod_id().as_str())),
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

fn target() -> ReplacementTarget {
    ReplacementTarget::new(
        ReplacementTargetId::parse("mhw:armor:fatalis-alpha").expect("target id"),
        GameId::mhw(),
        ReplacementTargetKind::parse("armor").expect("target kind"),
        LocalizedText::new(BTreeMap::from([
            ("zh_cn".to_owned(), "【精英‧龙α】服装".to_owned()),
            ("en".to_owned(), "Fatalis Alpha +".to_owned()),
        ]))
        .expect("localized name"),
        vec!["黑龙".to_owned(), "Fatalis".to_owned()],
        "pl129_0000",
        BTreeMap::from([("path_family".to_owned(), serde_json::json!("pl/f_equip"))]),
    )
    .expect("target")
}

fn stored_analysis() -> StoredModImportAnalysis {
    StoredModImportAnalysis {
        mod_id: "mod-a".to_owned(),
        task_id: "task-v1".to_owned(),
        package_id: "revision-v1".to_owned(),
        display_name: "Armor Mod".to_owned(),
        metadata: StoredModPackageMetadata::default(),
        preview_image: hmm_ports::StoredImportPreviewImage::Fallback {
            reason: hmm_core::PreviewImageRejectionReason::Missing,
        },
    }
}

fn workflow(status: hmm_app::InstallRecoveryStatus) -> ReplacementWorkflowService {
    ReplacementWorkflowService::new(
        vec![Arc::new(FakeAdapter)],
        vec![Arc::new(FakeCatalog)],
        Arc::new(FakeRepository),
        Arc::new(FakeSandboxLocator),
        Arc::new(FakeFileScanner),
        Arc::new(FixedStatus(status)),
        Arc::new(FixedClock),
    )
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

#[test]
fn workflow_resolves_display_revision_and_previews_revision_owned_retarget_plan() {
    let preview = workflow(hmm_app::InstallRecoveryStatus::NotInstalled)
        .preview_initial_install(PreviewInitialRetargetInstallRequest {
            game_id: GameId::mhw(),
            profile_id: ProfileId::new("profile-a"),
            mod_id: ModId::new("mod-a"),
            target_id: ReplacementTargetId::parse("mhw:armor:fatalis-alpha").expect("target id"),
            layer: FileLayer::new("base", 0),
        })
        .expect("preview initial retarget install");

    assert_eq!(preview.package_id(), "revision-v1");
    assert_eq!(preview.revision_id().as_str(), "revision-v1");
    assert!(preview.analysis().is_retargetable());
    assert_eq!(preview.target().internal_id(), "pl129_0000");
    assert_eq!(
        preview.install_plan().actions[0].target_path.as_str(),
        "nativePC/pl/f_equip/pl129_0000/arm/mod/f_body.mod3"
    );
    assert_eq!(
        preview.install_plan().replacement_bindings[0].revision_id(),
        None,
        "initial install entries are not revision-owned; AR5 true reinstall adds revision facts"
    );
}

#[test]
fn workflow_uses_catalog_search_without_exposing_package_facts() {
    let targets = workflow(hmm_app::InstallRecoveryStatus::NotInstalled)
        .list_targets(&GameId::mhw(), Some("黑龙"))
        .expect("catalog search");

    assert_eq!(targets.len(), 1);
    assert_eq!(targets[0].id().as_str(), "mhw:armor:fatalis-alpha");
}

#[test]
fn workflow_fails_closed_for_every_non_not_installed_status() {
    for status in [
        hmm_app::InstallRecoveryStatus::Completed,
        hmm_app::InstallRecoveryStatus::CommittedCleanupPending,
        hmm_app::InstallRecoveryStatus::CleanupPending,
        hmm_app::InstallRecoveryStatus::RollbackRequired,
        hmm_app::InstallRecoveryStatus::RepairRequired,
        hmm_app::InstallRecoveryStatus::Unknown,
    ] {
        let error = workflow(status)
            .preview_initial_install(PreviewInitialRetargetInstallRequest {
                game_id: GameId::mhw(),
                profile_id: ProfileId::new("profile-a"),
                mod_id: ModId::new("mod-a"),
                target_id: ReplacementTargetId::parse("mhw:armor:fatalis-alpha")
                    .expect("target id"),
                layer: FileLayer::new("base", 0),
            })
            .expect_err("unsafe state must block initial install");

        assert_eq!(
            error,
            ReplacementWorkflowError::InitialInstallBlocked { status }
        );
    }
}
