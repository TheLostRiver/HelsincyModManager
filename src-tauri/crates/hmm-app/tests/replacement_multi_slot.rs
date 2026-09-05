//! `#349` 切片③b-3：一次提交 N 个绑定 + M 个「保持原位」槽位（D2 三态）。
//!
//! 三态在这里是「换到 X / 保持原位 / 不在选择里」：
//!
//! | 意图 | 绑定 | 该槽位的文件 |
//! | --- | --- | --- |
//! | `Retarget` | 重定向绑定 | 重定向后的路径，走各自的 staging 根 |
//! | `KeepInPlace` | identity 绑定 | **原路径**，不经 staging（提交时直接读沙箱原包） |
//! | 不在 `slots` 里 | 无 | 不进计划 |
//!
//! 断言钉在**计划产出**上（目标路径、绑定集合、源路由、staging 批次），不是「没报错」——
//! 「装上去了但内容/位置错」正是这一串 issue 要防的失败模式。

use hmm_app::{
    InitialRetargetInstallStatusError, InitialRetargetInstallStatusReader,
    InitialRetargetSelection, InitialRetargetSlotIntent, PreviewInitialRetargetInstallRequest,
    ReplacementWorkflowError, ReplacementWorkflowService, RetargetStagingMaterializerFactory,
};
use hmm_core::{
    FileLayer, GameId, InstallTargetPath, LocalizedText, ModId, ProfileId, ReplacementAnalysis,
    ReplacementBindingId, ReplacementCatalog, ReplacementCatalogVersion, ReplacementSource,
    ReplacementSourceId, ReplacementTarget, ReplacementTargetId, ReplacementTargetKind,
    RetargetAction, RetargetPlan,
};
use hmm_ports::{
    AppClock, InstallManifestRepository, ModImportResultRepository, ModImportSandboxLocator,
    ModPackageInstallFile, ModPackageInstallFileReadRequest, ModPackageInstallFileReader,
    ModPackageInstallFileScanError, ModPackageInstallFileScanRequest, ModPackageInstallFileScanner,
    ReplacementAdapter, ReplacementAdapterError, ReplacementAnalysisRequest,
    ReplacementCatalogProvider, ReplacementCatalogResult, RetargetPlanRequest,
    RetargetStagingError, RetargetStagingFile, RetargetStagingMaterializer,
    StoredModImportAnalysis, StoredModPackageMetadata,
};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

/// 一个包里的两把「武器」：两个源槽位，各一个文件。形态复刻真实多槽位包
/// （同族、同 path_family、各自一个模型文件）。
const SLOT_ONE: &str = "one001";
const SLOT_TWO: &str = "one005";
/// 两个槽位各自的重定向目标。
const TARGET_ONE: &str = "one002";
const TARGET_TWO: &str = "one003";

fn source_id(main_id: &str) -> ReplacementSourceId {
    ReplacementSourceId::parse(format!("mhw:weapon:wp/one:{main_id}")).expect("source id")
}

fn target_id(main_id: &str) -> ReplacementTargetId {
    ReplacementTargetId::parse(format!("mhw:weapon:{main_id}")).expect("target id")
}

fn source_path(main_id: &str) -> String {
    format!("nativePC/wp/one/{main_id}/mod/{main_id}.mod3")
}

fn source(main_id: &str) -> ReplacementSource {
    ReplacementSource::new(
        source_id(main_id),
        GameId::mhw(),
        ReplacementTargetKind::parse("weapon").expect("kind"),
        main_id,
        "wp/one",
        true,
    )
    .expect("source")
}

fn catalog_target(main_id: &str) -> ReplacementTarget {
    ReplacementTarget::new(
        target_id(main_id),
        GameId::mhw(),
        ReplacementTargetKind::parse("weapon").expect("kind"),
        LocalizedText::new(BTreeMap::from([("en".to_owned(), main_id.to_owned())]))
            .expect("localized name"),
        Vec::new(),
        main_id,
        BTreeMap::from([("path_family".to_owned(), serde_json::json!("wp/one"))]),
    )
    .expect("target")
}

/// 按 `binding.source_id()` 挑本槽位的那个文件，产出一条改写到目标槽位的动作。
struct MultiSlotAdapter;

impl ReplacementAdapter for MultiSlotAdapter {
    fn game_id(&self) -> GameId {
        GameId::mhw()
    }

    fn analyze_replacement_assets(
        &self,
        request: ReplacementAnalysisRequest,
    ) -> Result<ReplacementAnalysis, ReplacementAdapterError> {
        ReplacementAnalysis::new(
            request.game_id,
            vec![source(SLOT_ONE), source(SLOT_TWO)],
            request.assets.len(),
            Vec::new(),
        )
        .map_err(|_| ReplacementAdapterError::InvalidRetargetPlan)
    }

    fn build_retarget_plan(
        &self,
        request: RetargetPlanRequest,
    ) -> Result<RetargetPlan, ReplacementAdapterError> {
        let source = [SLOT_ONE, SLOT_TWO]
            .into_iter()
            .map(source)
            .find(|source| source.id() == request.binding.source_id())
            .ok_or(ReplacementAdapterError::SourceBindingMismatch)?;
        let target_main = target_main_id(request.binding.target_id());
        let asset = request
            .assets
            .iter()
            .find(|asset| asset.relative_path() == source_path(source.internal_id()))
            .ok_or(ReplacementAdapterError::InvalidRetargetPlan)?;
        let action = RetargetAction::new(
            asset.package_file_id().clone(),
            InstallTargetPath::parse(asset.relative_path(), ["nativePC"]).expect("source path"),
            InstallTargetPath::parse(source_path(&target_main), ["nativePC"]).expect("target path"),
            source.id().clone(),
            source.internal_id(),
            &target_main,
            source.path_family(),
            "wp/one",
        )
        .map_err(|_| ReplacementAdapterError::InvalidRetargetPlan)?;
        RetargetPlan::new(request.binding, source, vec![action], Vec::new())
            .map_err(|_| ReplacementAdapterError::InvalidRetargetPlan)
    }
}

fn target_main_id(target_id: &ReplacementTargetId) -> String {
    target_id
        .as_str()
        .rsplit(':')
        .next()
        .expect("target id has a trailing segment")
        .to_owned()
}

struct MultiSlotCatalog;

impl ReplacementCatalogProvider for MultiSlotCatalog {
    fn game_id(&self) -> GameId {
        GameId::mhw()
    }

    fn replacement_catalog(&self) -> ReplacementCatalogResult<ReplacementCatalog> {
        // `SLOT_ONE` 自己也在 catalog 里（「保持原位」要靠它解析出自身目标）；
        // `SLOT_TWO` **故意不在**，用来钉住 `KeepInPlaceUnavailable`。
        ReplacementCatalog::new(
            ReplacementCatalogVersion::parse("test-v1").expect("catalog version"),
            GameId::mhw(),
            vec![
                catalog_target(SLOT_ONE),
                catalog_target(TARGET_ONE),
                catalog_target(TARGET_TWO),
            ],
        )
        .map_err(|_| hmm_ports::ReplacementCatalogError::CatalogInvalid)
    }

    fn search_replacement_targets(
        &self,
        _query: &str,
    ) -> ReplacementCatalogResult<Vec<ReplacementTarget>> {
        Ok(Vec::new())
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

fn stored_analysis() -> StoredModImportAnalysis {
    StoredModImportAnalysis {
        mod_id: "mod-a".to_owned(),
        task_id: "task-v1".to_owned(),
        package_id: "revision-v1".to_owned(),
        display_name: "Two weapons".to_owned(),
        metadata: StoredModPackageMetadata::default(),
        preview_image: hmm_ports::StoredImportPreviewImage::Fallback {
            reason: hmm_core::PreviewImageRejectionReason::Missing,
        },
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
    ) -> Result<Vec<ModPackageInstallFile>, ModPackageInstallFileScanError> {
        if request.package_id != "revision-v1" {
            return Err(ModPackageInstallFileScanError::Unavailable);
        }
        Ok([SLOT_ONE, SLOT_TWO]
            .into_iter()
            .map(|main_id| ModPackageInstallFile {
                package_file_id: source_path(main_id),
                target_path: source_path(main_id),
            })
            .collect())
    }
}

impl ModPackageInstallFileReader for FakeFileScanner {
    fn read_install_file(
        &self,
        _request: ModPackageInstallFileReadRequest<'_>,
    ) -> anyhow::Result<Vec<u8>> {
        anyhow::bail!("multi-slot planning must not read source content")
    }
}

struct NotInstalled;

impl InitialRetargetInstallStatusReader for NotInstalled {
    fn recovery_status(
        &self,
        _game_id: &GameId,
        _profile_id: &ProfileId,
        _mod_id: &ModId,
    ) -> Result<hmm_app::InstallRecoveryStatus, InitialRetargetInstallStatusError> {
        Ok(hmm_app::InstallRecoveryStatus::NotInstalled)
    }
}

struct FixedClock;

impl AppClock for FixedClock {
    fn now_unix_millis(&self) -> anyhow::Result<u128> {
        Ok(42)
    }
}

struct NoManifest;

impl InstallManifestRepository for NoManifest {
    fn load_manifest(
        &self,
        _profile_id: &ProfileId,
    ) -> anyhow::Result<Option<hmm_core::InstallManifest>> {
        Ok(None)
    }

    fn save_manifest(&self, _manifest: &hmm_core::InstallManifest) -> anyhow::Result<()> {
        anyhow::bail!("multi-slot planning must not write a manifest")
    }
}

type StagingBatches = Vec<(ReplacementBindingId, Vec<RetargetStagingFile>)>;

/// 记录「哪个绑定 materialize 了哪些文件」——多绑定的核心判据就是这份归属。
#[derive(Default)]
struct RecordingStagingFactory {
    batches: Arc<Mutex<StagingBatches>>,
}

impl RetargetStagingMaterializerFactory for RecordingStagingFactory {
    fn materializer_for(
        &self,
        binding_id: &ReplacementBindingId,
    ) -> Result<Box<dyn RetargetStagingMaterializer>, ReplacementWorkflowError> {
        Ok(Box::new(RecordingMaterializer {
            binding_id: binding_id.clone(),
            batches: Arc::clone(&self.batches),
        }))
    }
}

impl RecordingStagingFactory {
    fn recorded(&self) -> StagingBatches {
        self.batches.lock().expect("batches").clone()
    }
}

struct RecordingMaterializer {
    binding_id: ReplacementBindingId,
    batches: Arc<Mutex<StagingBatches>>,
}

impl RetargetStagingMaterializer for RecordingMaterializer {
    fn materialize(&self, files: &[RetargetStagingFile]) -> Result<(), RetargetStagingError> {
        self.batches
            .lock()
            .expect("batches")
            .push((self.binding_id.clone(), files.to_vec()));
        Ok(())
    }
}

fn workflow() -> ReplacementWorkflowService {
    ReplacementWorkflowService::new(
        vec![Arc::new(MultiSlotAdapter)],
        vec![Arc::new(MultiSlotCatalog)],
        Arc::new(FakeRepository),
        Arc::new(FakeSandboxLocator),
        Arc::new(FakeFileScanner),
        Arc::new(FakeFileScanner),
        Arc::new(NotInstalled),
        Arc::new(NoManifest),
        Arc::new(FixedClock),
    )
}

fn preview(
    slots: Vec<InitialRetargetSlotIntent>,
) -> Result<hmm_app::PlannedInitialRetargetInstall, ReplacementWorkflowError> {
    workflow().preview_initial_install(PreviewInitialRetargetInstallRequest {
        game_id: GameId::mhw(),
        profile_id: ProfileId::new("default"),
        mod_id: ModId::new("mod-a"),
        selection: InitialRetargetSelection::PerSlot(slots),
        layer: FileLayer::new("base", 0),
    })
}

fn retarget(main_id: &str, target_main: &str) -> InitialRetargetSlotIntent {
    InitialRetargetSlotIntent::Retarget {
        source_id: source_id(main_id),
        target_id: target_id(target_main),
    }
}

fn keep_in_place(main_id: &str) -> InitialRetargetSlotIntent {
    InitialRetargetSlotIntent::KeepInPlace {
        source_id: source_id(main_id),
    }
}

fn target_paths(plan: &hmm_core::InstallPlan) -> Vec<&str> {
    let mut paths = plan
        .actions
        .iter()
        .map(|action| action.target_path.as_str())
        .collect::<Vec<_>>();
    paths.sort_unstable();
    paths
}

/// 两个槽位在**一次**提交里各自重定向：两个绑定、两条动作、各自落到各自的目标。
///
/// `#349` 之前这个包连分析都过不去（`MultipleSourceRoots` 拒整包）；切片①/③a 之后能分析、
/// 能建单个绑定，但运行时提交时失败关闭。
#[test]
fn two_slots_retarget_to_two_targets_in_one_submission() {
    let planned = preview(vec![
        retarget(SLOT_ONE, TARGET_ONE),
        retarget(SLOT_TWO, TARGET_TWO),
    ])
    .expect("两个槽位必须能一起规划");

    assert_eq!(planned.retarget_plans().len(), 2);
    assert_eq!(planned.install_plan().replacement_bindings.len(), 2);
    assert_eq!(
        target_paths(planned.install_plan()),
        vec![source_path(TARGET_ONE), source_path(TARGET_TWO)],
        "两个槽位各自落到各自的目标"
    );
    assert!(
        !planned.install_plan().has_blocking_conflicts(),
        "两个目标互不相同，不该有冲突"
    );

    // 源路由必须覆盖两个绑定——漏掉任何一个，提交时那些文件会退回读沙箱原包的
    // 未重定向字节，写进重定向后的路径。
    let routing = planned.source_routing().expect("source routing");
    assert_eq!(routing.len(), 2);
    assert_eq!(planned.staged_binding_ids().len(), 2);
    for plan in planned.retarget_plans() {
        let package_file_id = plan.actions()[0].package_file_id();
        assert_eq!(
            routing.staged_binding_for(package_file_id),
            Some(plan.binding().id()),
            "每个文件必须归属自己的绑定"
        );
    }
}

/// 每个绑定 materialize 到**自己**的 staging 根，且只放自己的文件。
#[test]
fn each_binding_materializes_only_its_own_files() {
    let planned = preview(vec![
        retarget(SLOT_ONE, TARGET_ONE),
        retarget(SLOT_TWO, TARGET_TWO),
    ])
    .expect("两个槽位必须能一起规划");
    let expected = planned
        .retarget_plans()
        .iter()
        .map(|plan| {
            (
                plan.binding().id().clone(),
                plan.actions()[0].target_relative_path().as_str().to_owned(),
            )
        })
        .collect::<Vec<_>>();

    let factory = RecordingStagingFactory::default();
    workflow()
        .materialize_initial_install(&factory, planned)
        .expect("materialize 两个绑定");

    let recorded = factory.recorded();
    assert_eq!(recorded.len(), 2, "两个绑定各 materialize 一次");
    for (binding_id, files) in &recorded {
        assert_eq!(files.len(), 1, "每个绑定只放自己的那个文件");
        let expected_target = expected
            .iter()
            .find(|(id, _)| id == binding_id)
            .map(|(_, target)| target.as_str())
            .expect("materialize 的绑定必须来自计划");
        assert_eq!(
            files[0].target_path().as_str(),
            expected_target,
            "文件必须落进它自己那个绑定的 staging 根"
        );
    }
}

/// 「保持原位」：文件按**原路径**进计划，且**不经 staging**。
///
/// 少了这一档，多槽位包就只能整包重定向或整包放弃（`#349` D2）。
#[test]
fn a_kept_in_place_slot_installs_at_its_original_path_without_staging() {
    let planned = preview(vec![
        retarget(SLOT_TWO, TARGET_TWO),
        keep_in_place(SLOT_ONE),
    ])
    .expect("一个重定向 + 一个保持原位");

    assert_eq!(
        target_paths(planned.install_plan()),
        vec![source_path(SLOT_ONE), source_path(TARGET_TWO)],
        "保持原位的槽位留在原路径，重定向的槽位换到目标"
    );

    // identity 绑定仍然进计划（它记录了「这个槽位装在自己身上」），但它的文件读原包、
    // 不建 staging。
    assert_eq!(planned.install_plan().replacement_bindings.len(), 2);
    let routing = planned.source_routing().expect("source routing");

    // 路由是**全映射**：两个动作都有显式来源。少了「保持原位」那一条，提交侧就无法区分
    // 「这个文件本来就该读原包」与「组装方漏记了一个文件」——后者会拿未重定向的原包字节
    // 写进重定向后的目标路径。
    assert_eq!(routing.len(), 2, "每个动作都必须有显式来源");
    for action in &planned.install_plan().actions {
        assert!(routing.covers(&action.provider.package_file_id));
    }
    assert_eq!(
        routing.origin_for(&hmm_core::PackageFileId::new(source_path(SLOT_ONE))),
        Some(&hmm_core::RetargetSourceOrigin::ImportedPackage),
        "保持原位的文件读沙箱原包"
    );
    assert_eq!(
        routing.staged_entries().count(),
        1,
        "只有重定向的那个槽位走 staging"
    );
    assert_eq!(planned.staged_binding_ids().len(), 1);

    let factory = RecordingStagingFactory::default();
    workflow()
        .materialize_initial_install(&factory, planned)
        .expect("materialize");

    let recorded = factory.recorded();
    assert_eq!(recorded.len(), 1, "保持原位的槽位一个 staging 目录都不该建");
    assert_eq!(
        recorded[0].1[0].target_path().as_str(),
        source_path(TARGET_TWO)
    );
}

/// 「不装」：不把槽位放进选择里，它的文件就不进计划。
#[test]
fn a_slot_left_out_of_the_selection_does_not_enter_the_plan() {
    let planned = preview(vec![retarget(SLOT_ONE, TARGET_ONE)]).expect("只装一个槽位");

    assert_eq!(planned.retarget_plans().len(), 1);
    assert_eq!(
        target_paths(planned.install_plan()),
        vec![source_path(TARGET_ONE)],
        "没被选中的槽位一个动作都不该产出"
    );
    assert!(planned.install_plan().actions.iter().all(|action| action
        .provider
        .package_file_id
        .as_str()
        != source_path(SLOT_TWO)));
}

/// 同一个源槽位给了两条意图：谁生效都可能是错的，拒绝。
#[test]
fn two_intents_for_one_source_are_rejected() {
    let error = preview(vec![
        retarget(SLOT_ONE, TARGET_ONE),
        retarget(SLOT_ONE, TARGET_TWO),
    ])
    .expect_err("同一个槽位两条意图必须被拒");

    assert_eq!(error, ReplacementWorkflowError::DuplicateSlotIntent);
}

/// 空选择不是「装个空计划」，是没得装。
#[test]
fn an_empty_selection_is_rejected() {
    let error = preview(Vec::new()).expect_err("空选择必须被拒");

    assert_eq!(error, ReplacementWorkflowError::SourceNotRetargetable);
}

/// 「保持原位」要求源槽位本身在 catalog 里能唯一解析成目标。解析不出就明确报错，不猜——
/// 猜错会把这个槽位的文件装到别的装备上。
#[test]
fn keeping_a_slot_in_place_fails_when_the_slot_is_not_a_catalog_target() {
    // `SLOT_TWO` 不在 catalog 里（见 `MultiSlotCatalog`）。
    let error = preview(vec![keep_in_place(SLOT_TWO)])
        .expect_err("解析不出自身目标时必须报 KeepInPlaceUnavailable");

    assert_eq!(error, ReplacementWorkflowError::KeepInPlaceUnavailable);
}

/// 选择里出现包里没有的源槽位：组装方与分析已不同步，拒绝。
#[test]
fn a_slot_that_the_package_does_not_contain_is_rejected() {
    let error =
        preview(vec![retarget("one999", TARGET_ONE)]).expect_err("包里没有的源槽位必须被拒");

    assert_eq!(error, ReplacementWorkflowError::SourceNotRetargetable);
}

/// 两个槽位指向**同一个**目标：具名拒绝，而不是「计划不可用」。
///
/// 不拦也装不上（两个 provider 撞同一个 `target_path`，动作全进 `conflicts`、`actions`
/// 为空，绑定校验随后报 `ReplacementBindingOwnerMissing`），但报出来的是通用的
/// `PlanUnavailable`——玩家看不出是自己把两件装备指到了一处。
#[test]
fn two_slots_aimed_at_one_target_are_rejected_by_name() {
    let error = preview(vec![
        retarget(SLOT_ONE, TARGET_ONE),
        retarget(SLOT_TWO, TARGET_ONE),
    ])
    .expect_err("两个槽位指向同一个目标必须被拒");

    assert_eq!(error, ReplacementWorkflowError::DuplicateSlotTarget);
}

/// 「把 A 换到 B 的位置」同时「让 B 保持原位」同样是撞车——检查不能只看 Retarget 意图。
#[test]
fn a_retarget_onto_a_slot_that_stays_in_place_is_rejected_by_name() {
    let error = preview(vec![retarget(SLOT_TWO, SLOT_ONE), keep_in_place(SLOT_ONE)])
        .expect_err("重定向到一个保持原位的槽位必须被拒");

    assert_eq!(error, ReplacementWorkflowError::DuplicateSlotTarget);
}
