//! `#292`：包内游戏根是大小写变体时，**普通安装**（不重定向）也必须能装上。
//!
//! 这里刻意跑**真实**组件而不是替身：真实的沙箱扫描器（它决定 `target_path` 的写法）、
//! 真实的 MHW 适配器（它声明 `allowed_install_roots`）、真实的 `InstallPlanningService`
//! （它负责过滤与建计划）。只有仓库与沙箱定位这两个端口用了最小替身——它们只是把
//! `mod_id` 映射到磁盘上的夹具目录，不参与任何路径判定。
//!
//! 判据是**等价性**：四种写法必须产出逐字相同的计划。只断言「小写也能装」会漏掉
//! 「认了但把非规范大小写原样带进清单」这类更隐蔽的失败——那正是 `#292` 方案 A 的缺陷，
//! NTFS 上 `nativePC/x` 与 `nativepc/x` 是同一个文件，冲突检测会失效。

use hmm_app::{BuildImportedModInstallPlanRequest, InstallPlanningService};
use hmm_core::{FileLayer, GameId, ModId};
use hmm_games_mhw::MonsterHunterWorldAdapter;
use hmm_infra::{ReadOnlyJsonGamePrerequisiteRuleRepository, SandboxModPackageInstallFileScanner};
use hmm_ports::{
    GameAdapter, ModImportResultRepository, ModImportSandboxLocator, StoredImportPreviewImage,
    StoredModImportAnalysis, StoredModPackageMetadata,
};
use std::path::{Path, PathBuf};
use std::sync::Arc;

const PACKAGE_ID: &str = "package-a";
const MOD_ID: &str = "mod-a";

struct FixtureAnalysisRepository;

impl ModImportResultRepository for FixtureAnalysisRepository {
    fn save_analysis(&self, _analysis: &StoredModImportAnalysis) -> anyhow::Result<()> {
        unreachable!("建计划不写导入结果")
    }

    fn list_analysis(&self) -> anyhow::Result<Vec<StoredModImportAnalysis>> {
        unreachable!("建计划不列举导入结果")
    }

    fn get_analysis(&self, mod_id: &str) -> anyhow::Result<Option<StoredModImportAnalysis>> {
        if mod_id != MOD_ID {
            return Ok(None);
        }
        Ok(Some(StoredModImportAnalysis {
            mod_id: MOD_ID.to_owned(),
            task_id: "task-a".to_owned(),
            package_id: PACKAGE_ID.to_owned(),
            display_name: "弓形态夹具".to_owned(),
            metadata: StoredModPackageMetadata::default(),
            preview_image: StoredImportPreviewImage::Fallback {
                reason: hmm_core::PreviewImageRejectionReason::Missing,
            },
        }))
    }
}

struct FixtureSandboxLocator {
    root: PathBuf,
}

impl ModImportSandboxLocator for FixtureSandboxLocator {
    fn sandbox_root_for_package(&self, package_id: &str) -> anyhow::Result<PathBuf> {
        anyhow::ensure!(package_id == PACKAGE_ID, "夹具只认识一个包");
        Ok(self.root.clone())
    }
}

fn mhw_adapter(rules_dir: &Path) -> Arc<dyn GameAdapter> {
    // 规则文件不存在也无妨：建计划只问 `allowed_install_roots`，不读前置规则。
    Arc::new(MonsterHunterWorldAdapter::new(Arc::new(
        ReadOnlyJsonGamePrerequisiteRuleRepository::new(rules_dir.join("missing-rules.json")),
    )))
}

/// 按给定的根段写一份弓形态夹具，走**完整**建计划链路，返回 `(目标路径, 包内文件 id)` 列表。
fn plan_targets(root_segment: &str) -> Vec<(String, String)> {
    let temp = tempfile::tempdir().expect("temp dir");
    let sandbox_root = temp.path().join(PACKAGE_ID);
    let model_dir = sandbox_root
        .join(root_segment)
        .join("wp")
        .join("bow")
        .join("bow017")
        .join("mod");
    // `PGR` 是族级作者目录，真实弓包里就是全大写——它验证「只归一化根段」。
    let texture_dir = sandbox_root
        .join(root_segment)
        .join("wp")
        .join("bow")
        .join("PGR");

    std::fs::create_dir_all(&model_dir).expect("create model fixture dirs");
    std::fs::create_dir_all(&texture_dir).expect("create texture fixture dirs");
    std::fs::write(model_dir.join("bow017.mod3"), b"model").expect("write mod3");
    std::fs::write(model_dir.join("bow017.mrl3"), b"material").expect("write mrl3");
    std::fs::write(texture_dir.join("BML.tex"), b"texture").expect("write tex");

    let planning = InstallPlanningService::with_imported_mod_sources(
        Arc::new(FixtureAnalysisRepository),
        Arc::new(FixtureSandboxLocator {
            root: sandbox_root.clone(),
        }),
        Arc::new(SandboxModPackageInstallFileScanner),
        vec![mhw_adapter(temp.path())],
    );

    let plan = planning
        .build_plan_from_imported_mod(BuildImportedModInstallPlanRequest {
            game_id: GameId::mhw(),
            mod_id: ModId::new(MOD_ID),
            layer: FileLayer::new("base", 0),
        })
        .unwrap_or_else(|error| panic!("根段 {root_segment} 的建计划失败：{error:?}"));

    assert!(
        plan.conflicts.is_empty(),
        "单包夹具不该有冲突：{:?}",
        plan.conflicts
    );

    plan.actions
        .iter()
        .map(|action| {
            (
                action.target_path.as_str().to_owned(),
                action.provider.package_file_id.as_str().to_owned(),
            )
        })
        .collect()
}

#[test]
fn install_plan_is_identical_across_native_pc_root_casings() {
    let canonical = plan_targets("nativePC");

    assert_eq!(
        canonical
            .iter()
            .map(|(target, _)| target.as_str())
            .collect::<Vec<_>>(),
        vec![
            "nativePC/wp/bow/PGR/BML.tex",
            "nativePC/wp/bow/bow017/mod/bow017.mod3",
            "nativePC/wp/bow/bow017/mod/bow017.mrl3",
        ],
        "规范写法的基线本身要对，否则等价性断言没有意义"
    );

    // 触发本 issue 的真实弓包用的就是小写 `nativepc/`；另两种是同族变体。
    for variant in ["nativepc", "NativePC", "NATIVEpc"] {
        let targets = plan_targets(variant);

        assert_eq!(
            targets.iter().map(|(target, _)| target).collect::<Vec<_>>(),
            canonical
                .iter()
                .map(|(target, _)| target)
                .collect::<Vec<_>>(),
            "根段 {variant} 的目标路径必须与规范写法逐字相同"
        );
    }
}

/// `package_file_id` 相对**沙箱根**，读取链路靠它定位文件——它必须保留包内的原始
/// 大小写，否则在大小写敏感的文件系统上读不到文件。归一化只发生在 `target_path`。
#[test]
fn package_file_ids_keep_the_on_disk_casing() {
    for variant in ["nativePC", "nativepc", "NativePC", "NATIVEpc"] {
        let targets = plan_targets(variant);

        assert_eq!(
            targets
                .iter()
                .map(|(_, package_file_id)| package_file_id.as_str())
                .collect::<Vec<_>>(),
            vec![
                format!("{variant}/wp/bow/PGR/BML.tex"),
                format!("{variant}/wp/bow/bow017/mod/bow017.mod3"),
                format!("{variant}/wp/bow/bow017/mod/bow017.mrl3"),
            ],
            "根段 {variant} 的 package_file_id 必须逐字保留磁盘上的写法"
        );
    }
}

/// 反面：像 `nativePC` 但不是它的根段仍然整包落空——归一化不能把段边界让出去。
#[test]
fn a_root_that_merely_shares_a_prefix_still_yields_an_empty_plan() {
    let targets = plan_targets_allowing_empty("nativePCX");

    assert!(
        targets.is_empty(),
        "nativePCX 不是 nativePC，必须仍然产出空计划：{targets:?}"
    );
}

fn plan_targets_allowing_empty(root_segment: &str) -> Vec<String> {
    let temp = tempfile::tempdir().expect("temp dir");
    let sandbox_root = temp.path().join(PACKAGE_ID);
    let model_dir = sandbox_root.join(root_segment).join("wp").join("bow");
    std::fs::create_dir_all(&model_dir).expect("create fixture dirs");
    std::fs::write(model_dir.join("bow017.mod3"), b"model").expect("write mod3");

    let planning = InstallPlanningService::with_imported_mod_sources(
        Arc::new(FixtureAnalysisRepository),
        Arc::new(FixtureSandboxLocator {
            root: sandbox_root.clone(),
        }),
        Arc::new(SandboxModPackageInstallFileScanner),
        vec![mhw_adapter(temp.path())],
    );

    planning
        .build_plan_from_imported_mod(BuildImportedModInstallPlanRequest {
            game_id: GameId::mhw(),
            mod_id: ModId::new(MOD_ID),
            layer: FileLayer::new("base", 0),
        })
        .expect("空计划在这一层不是错误，拦截发生在更上层")
        .actions
        .iter()
        .map(|action| action.target_path.as_str().to_owned())
        .collect()
}
