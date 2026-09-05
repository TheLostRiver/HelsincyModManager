use super::{
    analyze_mhw_weapon_assets, build_mhw_weapon_mrl3_transform_invocation, MhwWeaponCatalogSource,
    WeaponAnalysisError, WeaponBinaryError, WeaponCompanionPlacement, WeaponMainId,
    WeaponModelPair, WeaponPackageAnalysis, WeaponPathError, WeaponSourceClosure,
    WeaponTargetMetadata, WeaponTargetStatus, MHW_WEAPON_BINARY_MAX_BYTES,
    MHW_WEAPON_MRL3_TEXTURE_PATH_TRANSFORMER_ID, MHW_WEAPON_MRL3_TEXTURE_PATH_TRANSFORMER_VERSION,
};
use crate::armor_retarget::{
    resolve_target_allowing_legacy_ids, MhwArmorCatalog, MhwArmorReplacementAdapter,
};
use crate::package_path::segment_after_native_pc_root;
use hmm_core::{
    ContentTransformerIdentity, GameId, LocalizedText, ReplacementAdapterFacts,
    ReplacementAnalysis, ReplacementCatalog, ReplacementCatalogVersion, ReplacementSource,
    ReplacementTarget, ReplacementTargetKind, ReplacementWarning, RetargetAction, RetargetPlan,
    REPLACEMENT_ADAPTER_FACTS_SCHEMA_VERSION,
};
use hmm_ports::{
    ReplacementAdapter, ReplacementAdapterError, ReplacementAdapterResult,
    ReplacementAnalysisRequest, ReplacementAssetContentReader, ReplacementCatalogError,
    ReplacementCatalogProvider, ReplacementCatalogResult, RetargetPlanRequest,
};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

/// WR-02B 全量武器 catalog 按 family 拆成 14 份分片（合并校验的约束见
/// `MhwWeaponCatalogSource::parse_sharded`）。这里必须与 artifact 测试使用同一份清单：
/// 少一份分片等于那一类武器的重定向目标整体消失。
const WEAPON_CATALOG_SHARDS: [&str; 14] = [
    include_str!("../../data/weapons/mhw-weapon-targets.bow.v1.json"),
    include_str!("../../data/weapons/mhw-weapon-targets.caxe.v1.json"),
    include_str!("../../data/weapons/mhw-weapon-targets.gun.v1.json"),
    include_str!("../../data/weapons/mhw-weapon-targets.ham.v1.json"),
    include_str!("../../data/weapons/mhw-weapon-targets.hbg.v1.json"),
    include_str!("../../data/weapons/mhw-weapon-targets.hue.v1.json"),
    include_str!("../../data/weapons/mhw-weapon-targets.lan.v1.json"),
    include_str!("../../data/weapons/mhw-weapon-targets.lbg.v1.json"),
    include_str!("../../data/weapons/mhw-weapon-targets.one.v1.json"),
    include_str!("../../data/weapons/mhw-weapon-targets.rod.v1.json"),
    include_str!("../../data/weapons/mhw-weapon-targets.saxe.v1.json"),
    include_str!("../../data/weapons/mhw-weapon-targets.sou.v1.json"),
    include_str!("../../data/weapons/mhw-weapon-targets.swo.v1.json"),
    include_str!("../../data/weapons/mhw-weapon-targets.two.v1.json"),
];
/// 聚合 catalog（armor v3 + weapon v1）的版本号，Production 与 Sandbox 共用，
/// 与 armor、weapon 各自的 catalog_version 相互独立（治理契约见
/// EQUIPMENT_CATALOG_GOVERNANCE.md）。
const REPLACEMENT_CATALOG_VERSION: &str = "mhw-replacement-v1";
const WEAPON_ADAPTER_ID: &str = "mhw.weapon";
const WEAPON_STRATEGY_ID: &str = "mrl3-texture-path";
/// v2（#336 切片②）：随行文件进入重定向计划。
/// v3（#336 切片③）：命中可执行 / 脚本拒绝清单的文件不再产出动作。
/// v4（#343）：改名改成结构规则，未登记前缀的副件模型不再否决整包。
///
/// 每次 bump 的理由相同：同一个包在相邻两版下产出的 action 集合不同（v2 多出伴生文件，
/// v3 少掉被拒绝的文件），`file_count` 与三个闭包哈希随之变化——bump 就是为了让这个差异
/// 在 facts 里可见。带 `.exe` 的包按 v2 装过之后，v3 重装会少写那一个文件，这正是需要
/// 由版本号标记的行为变化。
///
/// bump 安全性已查证（切片② 结论，切片③ 不改变前提）：facts 随 manifest 落盘且有 `Wire`
/// 兼容类型，存量安装读回自己当时的值；`plan_hash` 的两处比对（`install_recovery.rs`、
/// `reinstall_commit.rs`）都在单次操作生命周期内，不存在跨版本比较。
const WEAPON_STRATEGY_VERSION: u32 = 4;

/// WR-05 起 Production 与 Sandbox 共用同一份聚合 catalog。
///
/// 原来的 developer seed（WR-04 人工 one001/one002）已退役：这两个资源路径
/// 与全量 catalog 的真实条目完全重合（stable_id 相同），人工目标与真数据
/// 无法共存于一个 catalog——真数据入册后 seed 没有存在意义。
#[derive(Debug, Clone, Copy, Default)]
pub struct MhwReplacementCatalog;

impl ReplacementCatalogProvider for MhwReplacementCatalog {
    fn game_id(&self) -> GameId {
        GameId::mhw()
    }

    fn replacement_catalog(&self) -> ReplacementCatalogResult<ReplacementCatalog> {
        let mut targets = MhwArmorCatalog.replacement_catalog()?.targets().to_vec();
        targets.extend(weapon_targets()?);
        targets.sort_by(|left, right| left.id().as_str().cmp(right.id().as_str()));
        ReplacementCatalog::new(
            ReplacementCatalogVersion::parse(REPLACEMENT_CATALOG_VERSION)
                .map_err(|_| ReplacementCatalogError::CatalogInvalid)?,
            GameId::mhw(),
            targets,
        )
        .map_err(|_| ReplacementCatalogError::CatalogInvalid)
    }

    fn find_replacement_target(
        &self,
        target_id: &hmm_core::ReplacementTargetId,
    ) -> ReplacementCatalogResult<ReplacementTarget> {
        // 与 MhwArmorCatalog 同一套回落：玩家已安装 manifest 里存的可能是
        // AR6 扩容前的旧 slug ID，不解析会碰坏他们已有的绑定。
        resolve_target_allowing_legacy_ids(&self.replacement_catalog()?, target_id)
    }

    fn search_replacement_targets(
        &self,
        query: &str,
    ) -> ReplacementCatalogResult<Vec<ReplacementTarget>> {
        let normalized = crate::normalize_armor_search_text(query);
        if normalized.is_empty() {
            return Ok(Vec::new());
        }
        Ok(self
            .replacement_catalog()?
            .targets()
            .iter()
            .filter(|target| replacement_target_matches(target, &normalized))
            .cloned()
            .collect())
    }
}

/// WR-05 门禁翻转后，武器 analysis/plan 在 Production 与 Sandbox 一视同仁；
/// 原 `weapon_developer_seed_unavailable` 拒绝路径随 developer seed 一并退役。
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
        if contains_weapon_candidate(&request) {
            MhwWeaponReplacementAdapter.analyze_replacement_assets(request)
        } else {
            MhwArmorReplacementAdapter.analyze_replacement_assets(request)
        }
    }

    fn build_retarget_plan(
        &self,
        request: RetargetPlanRequest,
    ) -> ReplacementAdapterResult<RetargetPlan> {
        if contains_weapon_plan_candidate(&request) {
            MhwWeaponReplacementAdapter.build_retarget_plan(request)
        } else {
            MhwArmorReplacementAdapter.build_retarget_plan(request)
        }
    }

    fn build_retarget_plan_with_content(
        &self,
        request: RetargetPlanRequest,
        content_reader: &dyn ReplacementAssetContentReader,
    ) -> ReplacementAdapterResult<RetargetPlan> {
        if contains_weapon_plan_candidate(&request) {
            MhwWeaponReplacementAdapter.build_retarget_plan_with_content(request, content_reader)
        } else {
            MhwArmorReplacementAdapter.build_retarget_plan(request)
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct MhwWeaponReplacementAdapter;

impl ReplacementAdapter for MhwWeaponReplacementAdapter {
    fn game_id(&self) -> GameId {
        GameId::mhw()
    }

    fn analyze_replacement_assets(
        &self,
        request: ReplacementAnalysisRequest,
    ) -> ReplacementAdapterResult<ReplacementAnalysis> {
        ensure_mhw(&request.game_id)?;
        let analysis = analyze_mhw_weapon_assets(&request.assets).map_err(map_analysis_error)?;
        analysis_from_units(&analysis)
    }

    fn build_retarget_plan(
        &self,
        _request: RetargetPlanRequest,
    ) -> ReplacementAdapterResult<RetargetPlan> {
        Err(ReplacementAdapterError::SourceContentUnavailable)
    }

    fn build_retarget_plan_with_content(
        &self,
        request: RetargetPlanRequest,
        content_reader: &dyn ReplacementAssetContentReader,
    ) -> ReplacementAdapterResult<RetargetPlan> {
        ensure_mhw(&request.game_id)?;
        let analysis = analyze_mhw_weapon_assets(&request.assets).map_err(map_analysis_error)?;
        /*
         * `#349`：按 binding 指定的源**在单元里挑**，而不是要求整包恰好一个槽位。
         *
         * binding 本来就带 `source_id`，所以「包里有几个槽位」与「这次要装哪个」是两件事。
         * 此前分类器在多槽位时否决整包，这里也就只可能拿到唯一的那个；现在多槽位包能正常
         * 分析出来，建计划就按 binding 选中的那个单元来。挑不到才是真正的绑定不匹配。
         */
        let closure = analysis
            .units()
            .iter()
            .find(|unit| unit.source_id() == request.binding.source_id())
            .ok_or(ReplacementAdapterError::SourceBindingMismatch)?;

        let target = weapon_target(request.binding.target_id())?;
        if target.target_type().as_str() != "weapon" {
            return Err(ReplacementAdapterError::UnsupportedReplacementTarget);
        }
        let target_main = WeaponMainId::parse(target.internal_id())
            .map_err(|_| ReplacementAdapterError::UnsupportedReplacementTarget)?;
        if target_main.family() != closure.family()
            || target.metadata().get("path_family").and_then(Value::as_str)
                != Some(closure.root().path_family())
        {
            return Err(ReplacementAdapterError::AnalysisRejected {
                code: "weapon_cross_family_target",
            });
        }

        let loaded_pairs = load_pair_contents(closure, content_reader)?;
        let source = source_from_closure(closure)?;
        let actions = build_weapon_actions(closure, &target_main, &loaded_pairs)?;
        let warnings = (source.internal_id() == target_main.as_str())
            .then_some(ReplacementWarning::SourceMatchesTarget)
            .into_iter()
            .collect();
        let plan = RetargetPlan::new(request.binding, source, actions, warnings)
            .map_err(|_| ReplacementAdapterError::InvalidRetargetPlan)?;
        let facts = ReplacementAdapterFacts::new(
            REPLACEMENT_ADAPTER_FACTS_SCHEMA_VERSION,
            WEAPON_ADAPTER_ID,
            WEAPON_STRATEGY_ID,
            WEAPON_STRATEGY_VERSION,
            source_closure_digest(closure, &loaded_pairs),
            part_set_digest(closure),
            plan.content_transform_set_sha256(),
        )
        .and_then(|facts| {
            facts.with_transformers(
                vec![ContentTransformerIdentity::new(
                    MHW_WEAPON_MRL3_TEXTURE_PATH_TRANSFORMER_ID,
                    MHW_WEAPON_MRL3_TEXTURE_PATH_TRANSFORMER_VERSION,
                )?],
                closure.pairs().len() as u32,
                plan.actions().len() as u32,
            )
        })
        // 审计留痕（#336 切片③）：被拒绝清单丢弃的文件数随 manifest 落盘，
        // 并由 `hmm-app` 的 replacement audit 写进 Audit Log。只出计数不出路径——
        // 路径属于第三方 Mod 内容，`SECURITY.md` 禁止进日志。
        .map(|facts| facts.with_excluded_file_count(closure.excluded().len() as u32))
        .map_err(|_| ReplacementAdapterError::InvalidRetargetPlan)?;
        plan.with_adapter_facts(facts)
            .map_err(|_| ReplacementAdapterError::InvalidRetargetPlan)
    }
}

fn contains_weapon_candidate(request: &ReplacementAnalysisRequest) -> bool {
    request
        .assets
        .iter()
        .any(|asset| is_weapon_path(asset.relative_path()))
}

fn contains_weapon_plan_candidate(request: &RetargetPlanRequest) -> bool {
    request
        .assets
        .iter()
        .any(|asset| is_weapon_path(asset.relative_path()))
}

/// 判定是否含武器资源候选。
///
/// 这里只做粗筛，真正的语法校验在 `analyze_mhw_weapon_assets`。
/// 不能要求 `nativePC` 是首段：绝大多数真实 Mod 压缩包在 `nativePC` 之外
/// 还包了一层作者自建目录（`MyWeaponMod/nativePC/wp/...`），而上游解压与
/// 扫描链路没有剥离公共根目录。过去用前缀匹配导致这类包整包被送去防具
/// 适配器，最后报一个与武器无关的错误。
///
/// 注意这里**不会**把防具包误判成武器：防具路径是 `nativePC/pl/...`、
/// `nativePC/ch/...` 等，`nativePC` 之后紧跟的分量不是 `wp`。
fn is_weapon_path(path: &str) -> bool {
    // 游戏根这一段按大小写不敏感定位（#345）：真实包里 `nativepc` / `NativePC` 很常见，
    // 而它们在 Windows 上与 `nativePC` 是同一个目录。归一化在 `package_path`，见那里的说明。
    segment_after_native_pc_root(path).as_deref() == Some("wp")
}

fn ensure_mhw(game_id: &GameId) -> ReplacementAdapterResult<()> {
    if game_id == &GameId::mhw() {
        Ok(())
    } else {
        Err(ReplacementAdapterError::UnsupportedGame)
    }
}

/// `#349`：包里的**每个**槽位都作为一个源呈现出去。
///
/// `ReplacementAnalysis` 的 `sources` 一直是列表（还自带 id 去重与 game 一致性校验），
/// 是这里此前把它压成 `vec![单个]`、并让分类器在多槽位时否决整包。现在如实报出全部。
fn analysis_from_units(
    analysis: &WeaponPackageAnalysis,
) -> ReplacementAdapterResult<ReplacementAnalysis> {
    let mut sources = Vec::with_capacity(analysis.units().len());
    let mut partial_part_set = false;
    for closure in analysis.units() {
        sources.push(source_from_closure(closure)?);
        partial_part_set |= !closure.warnings().is_empty();
    }
    // 警告是包级的一句提示，多个单元都缺件也只说一次。
    let warnings = partial_part_set
        .then_some(ReplacementWarning::WeaponPartialPartSet)
        .into_iter()
        .collect();
    ReplacementAnalysis::new(GameId::mhw(), sources, analysis.asset_count(), warnings)
        .map_err(|_| ReplacementAdapterError::InvalidRetargetPlan)
}

fn source_from_closure(
    closure: &WeaponSourceClosure,
) -> ReplacementAdapterResult<ReplacementSource> {
    ReplacementSource::new(
        closure.source_id().clone(),
        GameId::mhw(),
        ReplacementTargetKind::parse("weapon")
            .map_err(|_| ReplacementAdapterError::InvalidRetargetPlan)?,
        closure.root().main_id().as_str(),
        closure.root().path_family(),
        true,
    )
    .map_err(|_| ReplacementAdapterError::InvalidRetargetPlan)
}

struct LoadedPair<'a> {
    pair: &'a WeaponModelPair,
    mod3: Vec<u8>,
    mrl3: Vec<u8>,
}

fn load_pair_contents<'a>(
    closure: &'a WeaponSourceClosure,
    content_reader: &dyn ReplacementAssetContentReader,
) -> ReplacementAdapterResult<Vec<LoadedPair<'a>>> {
    closure
        .pairs()
        .iter()
        .map(|pair| {
            Ok(LoadedPair {
                pair,
                mod3: content_reader.read_asset_content(
                    pair.mod3().package_file_id(),
                    MHW_WEAPON_BINARY_MAX_BYTES as u64,
                )?,
                mrl3: content_reader.read_asset_content(
                    pair.mrl3().package_file_id(),
                    MHW_WEAPON_BINARY_MAX_BYTES as u64,
                )?,
            })
        })
        .collect()
}

fn build_weapon_actions(
    closure: &WeaponSourceClosure,
    target_main: &WeaponMainId,
    loaded_pairs: &[LoadedPair<'_>],
) -> ReplacementAdapterResult<Vec<RetargetAction>> {
    let mut actions = Vec::with_capacity(loaded_pairs.len() * 2 + closure.companions().len());
    for loaded in loaded_pairs {
        let invocation = build_mhw_weapon_mrl3_transform_invocation(
            loaded.pair,
            target_main,
            &loaded.mod3,
            &loaded.mrl3,
        )
        .map_err(map_binary_error)?;
        for (asset, transform) in [
            (loaded.pair.mod3(), None),
            (loaded.pair.mrl3(), Some(invocation.clone())),
        ] {
            let action = RetargetAction::new(
                asset.package_file_id().clone(),
                asset.relative_path().clone(),
                asset.model_path().retarget(target_main).map_err(|_| {
                    ReplacementAdapterError::AnalysisRejected {
                        code: "weapon_cross_family_target",
                    }
                })?,
                closure.source_id().clone(),
                closure.root().main_id().as_str(),
                target_main.as_str(),
                closure.root().path_family(),
                target_main.family().path_family(),
            )
            .map_err(|_| ReplacementAdapterError::InvalidRetargetPlan)?;
            actions.push(match transform {
                Some(transform) => action.with_content_transform(transform),
                None => action,
            });
        }
    }

    /*
     * 随行文件（#336 切片②）。
     *
     * 只有模型对需要二进制改写，随行文件一律 `content_transform: None`——它们是贴图、
     * 特效、附件，字节与槽位无关，搬走即可。`RetargetAction` 因此无需改 schema。
     *
     * 分类已在 analysis 阶段完成，这里只把两档落位翻译成目标路径：
     * - `Relocated`（源槽位目录内）→ `relocate_within`：换槽位段 + 按部件 ID 前缀改名，
     *   与 MRL3 引用改写共用 `part_rename` 的同一张对照表，两处结果必然一致。
     * - `Verbatim`（`nativePC/wp/` 下但与槽位无关）→ 目标路径 = 原路径。这是参照实现
     *   在真机实验里的实测行为：作者自建贴图目录换任何目标槽位都仍被引用命中，搬了反而断链。
     */
    for companion in closure.companions() {
        let target_relative_path = match companion.placement() {
            WeaponCompanionPlacement::Relocated => closure
                .root()
                .relocate_within(companion.relative_path(), target_main)
                .map_err(map_companion_relocation_error)?,
            WeaponCompanionPlacement::Verbatim => companion.relative_path().clone(),
        };
        actions.push(
            RetargetAction::new(
                companion.package_file_id().clone(),
                companion.relative_path().clone(),
                target_relative_path,
                closure.source_id().clone(),
                closure.root().main_id().as_str(),
                target_main.as_str(),
                closure.root().path_family(),
                target_main.family().path_family(),
            )
            .map_err(|_| ReplacementAdapterError::InvalidRetargetPlan)?,
        );
    }
    Ok(actions)
}

/// 随行文件重定位失败的错误码映射。
///
/// `relocate_within` 用 `UnsupportedResource` 表示「文件名撞守卫②」——部件 ID 在文件名里
/// 出现多次，无法判断作者意图。这里**不**沿用它的码：`weapon_unsupported_resource` 的前端
/// 文案是「只支持 .mod3 与 .mrl3」，对改名失败是误导，正是 #336 抱怨的那种错误引导。
///
/// 改用 `weapon_binary_reference_ambiguous`：磁盘改名与 MRL3 引用改写共用 `part_rename` 的
/// 同一张对照表和同两条守卫，同一个文件名在两处必然得到同一个结论，本就是同一个根因的两个
/// 出口。复用既有码也让切片② 保持「不碰 public 错误码契约」。
///
/// 这里选择失败关闭而非降级（留源路径）：降级的完整语义要求引用侧同步不改写并计入告警，
/// 而告警变体在切片⑤。只做磁盘侧降级会静默产出引用与文件位置不一致的安装——**静默产出
/// 坏结果比失败关闭更糟**。
fn map_companion_relocation_error(error: WeaponPathError) -> ReplacementAdapterError {
    match error {
        WeaponPathError::UnsupportedResource => {
            map_binary_error(WeaponBinaryError::ReferenceAmbiguous)
        }
        other => ReplacementAdapterError::AnalysisRejected { code: other.code() },
    }
}

fn source_closure_digest(closure: &WeaponSourceClosure, loaded_pairs: &[LoadedPair<'_>]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"hmm-mhw-weapon-source-closure-v1\0");
    hasher.update(closure.source_id().as_str().as_bytes());
    hasher.update([0]);
    for loaded in loaded_pairs {
        hasher.update(loaded.pair.part_id().as_str().as_bytes());
        hasher.update([0]);
        hasher.update(Sha256::digest(&loaded.mod3));
        hasher.update(Sha256::digest(&loaded.mrl3));
    }
    format!("{:x}", hasher.finalize())
}

fn part_set_digest(closure: &WeaponSourceClosure) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"hmm-mhw-weapon-part-set-v1\0");
    for pair in closure.pairs() {
        hasher.update(pair.part_id().as_str().as_bytes());
        hasher.update([0]);
    }
    format!("{:x}", hasher.finalize())
}

fn map_analysis_error(error: WeaponAnalysisError) -> ReplacementAdapterError {
    ReplacementAdapterError::AnalysisRejected { code: error.code() }
}

fn map_binary_error(error: WeaponBinaryError) -> ReplacementAdapterError {
    ReplacementAdapterError::AnalysisRejected { code: error.code() }
}

fn weapon_catalog() -> ReplacementCatalogResult<MhwWeaponCatalogSource> {
    MhwWeaponCatalogSource::parse_sharded(&WEAPON_CATALOG_SHARDS)
        .map_err(|_| ReplacementCatalogError::CatalogInvalid)
}

fn weapon_targets() -> ReplacementCatalogResult<Vec<ReplacementTarget>> {
    weapon_catalog()?
        .targets()
        .iter()
        .filter(|target| target.status() == WeaponTargetStatus::Active)
        .map(weapon_target_from_metadata)
        .collect()
}

/// plan 阶段按 ID 解析武器目标。`MhwWeaponCatalogSource` 的 resolver 同时登记
/// stable_id 与 legacy_id，legacy 回落内建于 source 层。
fn weapon_target(
    target_id: &hmm_core::ReplacementTargetId,
) -> ReplacementAdapterResult<ReplacementTarget> {
    let source = weapon_catalog().map_err(|_| ReplacementAdapterError::TargetCatalogUnavailable)?;
    let target = source.resolve(target_id.as_str()).ok_or_else(|| {
        ReplacementAdapterError::TargetCatalogMissing {
            target_id: target_id.clone(),
        }
    })?;
    weapon_target_from_metadata(target)
        .map_err(|_| ReplacementAdapterError::TargetCatalogUnavailable)
}

fn weapon_target_from_metadata(
    target: &WeaponTargetMetadata,
) -> ReplacementCatalogResult<ReplacementTarget> {
    let aliases = target
        .aliases()
        .values()
        .flatten()
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let metadata = BTreeMap::from([
        (
            "family".to_owned(),
            Value::String(target.family().as_str().to_owned()),
        ),
        (
            "path_family".to_owned(),
            Value::String(target.root().path_family().to_owned()),
        ),
    ]);
    ReplacementTarget::new(
        target.id().clone(),
        GameId::mhw(),
        ReplacementTargetKind::parse("weapon")
            .map_err(|_| ReplacementCatalogError::CatalogInvalid)?,
        LocalizedText::new(target.display_names().clone())
            .map_err(|_| ReplacementCatalogError::CatalogInvalid)?,
        aliases,
        target.root().main_id().as_str(),
        metadata,
    )
    // 上面的平表是三语别名压成的 BTreeSet，丢了 locale；artifact 本来就按语言分（#274），
    // 原样带过去供前端按界面语言展示，平表继续只做检索。
    .and_then(|target_without_locales| {
        target_without_locales.with_localized_aliases(target.aliases().clone())
    })
    .map_err(|_| ReplacementCatalogError::CatalogInvalid)
}

fn replacement_target_matches(target: &ReplacementTarget, query: &str) -> bool {
    let mut terms = vec![target.id().as_str(), target.internal_id()];
    terms.extend(target.display_name().values());
    terms.extend(target.aliases().iter().map(String::as_str));
    terms
        .into_iter()
        .any(|term| crate::normalize_armor_search_text(term).contains(query))
}

#[cfg(test)]
mod tests {
    use super::*;
    use hmm_core::{ModId, ProfileId, ReplacementBinding, ReplacementBindingId};
    use hmm_ports::{ReplacementAsset, RetargetPlanRequest};

    #[test]
    fn bundled_weapon_catalog_names_cover_all_mhw_locales() {
        // 与 armor catalog 同一约束：names 键集即 per-game locale 能力声明（I18N-08），
        // 分片扩容时漏语言会导致对应界面 fallback 与检索缺口。
        for shard in WEAPON_CATALOG_SHARDS {
            let raw: Value = serde_json::from_str(shard).expect("weapon shard json");
            for target in raw["targets"].as_array().expect("targets array") {
                let names = target["names"].as_object().expect("names object");
                let mut keys: Vec<_> = names.keys().map(String::as_str).collect();
                keys.sort_unstable();
                assert_eq!(
                    keys,
                    ["en", "ja", "zh_cn"],
                    "weapon target {} must carry the full locale set",
                    target["internal_id"]
                );
            }
        }
    }

    #[test]
    fn bundled_weapon_targets_carry_aliases_per_locale_alongside_the_flat_search_list() {
        // #274：平表是三语压平的 BTreeSet（ASCII 排前），界面按语言展示别名必须走 localized_aliases。
        // 键集与展示名键集一致（三语齐全），每个按语言别名都在平表里（core 构造时已校验，这里
        // 用真实分片再确认一次），且平表 = 三语别名的并集，不多不少。
        let targets = weapon_targets().expect("bundled weapon targets");
        assert_eq!(targets.len(), 601);
        for target in &targets {
            let localized = target
                .localized_aliases()
                .unwrap_or_else(|| panic!("{} must carry localized aliases", target.internal_id()));
            let display_names: BTreeMap<String, String> = target.display_name().clone().into();
            let display_locales: BTreeSet<&str> =
                display_names.keys().map(String::as_str).collect();
            let alias_locales: BTreeSet<&str> = localized.keys().map(String::as_str).collect();
            assert_eq!(alias_locales, display_locales, "{}", target.internal_id());
            assert_eq!(
                alias_locales,
                ["en", "ja", "zh_cn"].into_iter().collect::<BTreeSet<_>>(),
                "{}",
                target.internal_id()
            );
            let union: BTreeSet<&str> = localized.values().flatten().map(String::as_str).collect();
            let flat: BTreeSet<&str> = target.aliases().iter().map(String::as_str).collect();
            assert_eq!(union, flat, "{}", target.internal_id());
        }

        let fatalis_blade = targets
            .iter()
            .find(|target| target.internal_id() == "two029")
            .expect("two029");
        let localized = fatalis_blade
            .localized_aliases()
            .expect("two029 localized aliases");
        assert_eq!(localized["zh_cn"], ["黑龙玄刃"]);
        assert_eq!(localized["en"], ["Black Fatalis Blade"]);
        assert_eq!(localized["ja"], ["ブラックミラブレイド"]);
        // 平表把三语压在一起且按字节序排列——正是前端不能直接「取前两个」的原因。
        assert_eq!(
            fatalis_blade.aliases(),
            ["Black Fatalis Blade", "ブラックミラブレイド", "黑龙玄刃"]
        );
    }

    const MOD3_HEADER_SIZE: usize = 320;
    const MOD3_MATERIAL_ENTRY_SIZE: usize = 128;
    const MOD3_MESH_ENTRY_SIZE: usize = 80;
    const MRL3_HEADER_SIZE: usize = 40;
    const MRL3_TEXTURE_ENTRY_SIZE: usize = 272;
    const MRL3_MATERIAL_ENTRY_SIZE: usize = 56;
    const MRL3_TEXTURE_PATH_OFFSET: usize = 16;
    const ARTIFICIAL_MATERIAL_HASH: u32 = 0xa7f6_8bf8;

    struct ArtificialContentReader {
        mod3: Vec<u8>,
        mrl3: Vec<u8>,
    }

    impl ReplacementAssetContentReader for ArtificialContentReader {
        fn read_asset_content(
            &self,
            package_file_id: &hmm_core::PackageFileId,
            max_bytes: u64,
        ) -> ReplacementAdapterResult<Vec<u8>> {
            let bytes = match package_file_id.as_str() {
                "weapon.mod3" => &self.mod3,
                "weapon.mrl3" => &self.mrl3,
                _ => return Err(ReplacementAdapterError::SourceContentUnavailable),
            };
            if bytes.len() as u64 > max_bytes {
                return Err(ReplacementAdapterError::SourceContentUnavailable);
            }
            Ok(bytes.clone())
        }
    }

    fn write_u16(bytes: &mut [u8], offset: usize, value: u16) {
        bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
    }

    fn write_u32(bytes: &mut [u8], offset: usize, value: u32) {
        bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }

    fn write_u64(bytes: &mut [u8], offset: usize, value: u64) {
        bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
    }

    fn artificial_mod3() -> Vec<u8> {
        let material_offset = MOD3_HEADER_SIZE;
        let mesh_offset = material_offset + MOD3_MATERIAL_ENTRY_SIZE;
        let vertex_offset = mesh_offset + MOD3_MESH_ENTRY_SIZE + 4;
        let vertex_buffer_size = 36usize;
        let face_offset = vertex_offset + vertex_buffer_size;
        let face_buffer_size = 8usize;
        let vertex_remap_offset = face_offset + face_buffer_size;
        let mut bytes = vec![0u8; vertex_remap_offset + 24];
        write_u32(&mut bytes, 0, 0x0044_4f4d);
        write_u16(&mut bytes, 4, 237);
        write_u16(&mut bytes, 8, 1);
        write_u16(&mut bytes, 10, 1);
        write_u32(&mut bytes, 12, 3);
        write_u32(&mut bytes, 16, 3);
        write_u64(&mut bytes, 24, vertex_buffer_size as u64);
        write_u64(&mut bytes, 64, material_offset as u64);
        write_u64(&mut bytes, 72, mesh_offset as u64);
        write_u64(&mut bytes, 80, vertex_offset as u64);
        write_u64(&mut bytes, 88, face_offset as u64);
        write_u64(&mut bytes, 96, vertex_remap_offset as u64);
        let material = b"ArtificialWeaponMaterial";
        bytes[material_offset..material_offset + material.len()].copy_from_slice(material);
        write_u16(&mut bytes, mesh_offset + 2, 3);
        write_u16(&mut bytes, mesh_offset + 6, 0);
        write_u16(&mut bytes, mesh_offset + 8, 1);
        bytes[mesh_offset + 14] = 12;
        write_u32(&mut bytes, mesh_offset + 32, 3);
        write_u32(&mut bytes, vertex_remap_offset, 4);
        bytes
    }

    fn artificial_mrl3() -> Vec<u8> {
        let texture_offset = MRL3_HEADER_SIZE;
        let material_offset = texture_offset + MRL3_TEXTURE_ENTRY_SIZE;
        let material_end = material_offset + MRL3_MATERIAL_ENTRY_SIZE;
        let resource_offset = (material_end + 15) & !15;
        let mut bytes = vec![0u8; resource_offset + 16];
        write_u32(&mut bytes, 0, 0x004c_524d);
        write_u32(&mut bytes, 4, 12);
        write_u32(&mut bytes, 16, 1);
        write_u32(&mut bytes, 20, 1);
        write_u64(&mut bytes, 24, texture_offset as u64);
        write_u64(&mut bytes, 32, material_offset as u64);
        write_u32(&mut bytes, texture_offset, 0x241f_5deb);
        let path = br"wp\one\one001\tex\weapon_BM";
        let path_offset = texture_offset + MRL3_TEXTURE_PATH_OFFSET;
        bytes[path_offset..path_offset + path.len()].copy_from_slice(path);
        write_u32(&mut bytes, material_offset, 0x4516_e7ab);
        write_u32(&mut bytes, material_offset + 4, ARTIFICIAL_MATERIAL_HASH);
        write_u32(&mut bytes, material_offset + 16, 16);
        write_u16(&mut bytes, material_offset + 22, 2);
        write_u64(&mut bytes, material_offset + 48, resource_offset as u64);
        bytes
    }

    fn artificial_weapon_assets() -> Vec<ReplacementAsset> {
        vec![
            ReplacementAsset::new(
                hmm_core::PackageFileId::new("weapon.mod3"),
                "nativePC/wp/one/one001/mod/one001.mod3",
            ),
            ReplacementAsset::new(
                hmm_core::PackageFileId::new("weapon.mrl3"),
                "nativePC/wp/one/one001/mod/one001.mrl3",
            ),
        ]
    }

    /// WR-05 门禁翻转后，武器候选在聚合 router 上直接可用——这条测试同时
    /// 钉住"拒绝路径已删除"：如果有人重建 developer 门禁，这里要显式改回来。
    #[test]
    fn router_accepts_weapon_candidate_without_developer_gate() {
        let analysis = MhwReplacementAdapter
            .analyze_replacement_assets(ReplacementAnalysisRequest {
                game_id: GameId::mhw(),
                assets: artificial_weapon_assets(),
            })
            .expect("weapon analysis must not require a developer gate");
        let source = analysis.single_source().expect("single weapon source");
        assert_eq!(source.source_type().as_str(), "weapon");
    }

    /// 真实压缩包在 `nativePC` 之外常包一层作者自建目录。过去 router 用前缀
    /// 匹配，这类包整包被送去防具适配器、报一个与武器无关的错误——这是
    /// 真机上第一个被击中的点。
    #[test]
    fn router_recognizes_weapon_candidate_under_an_author_package_root_directory() {
        let analysis = MhwReplacementAdapter
            .analyze_replacement_assets(ReplacementAnalysisRequest {
                game_id: GameId::mhw(),
                assets: vec![
                    ReplacementAsset::new(
                        hmm_core::PackageFileId::new("wrapped.mod3"),
                        "Cool Greatsword v1.2/nativePC/wp/one/one001/mod/one001.mod3",
                    ),
                    ReplacementAsset::new(
                        hmm_core::PackageFileId::new("wrapped.mrl3"),
                        "Cool Greatsword v1.2/nativePC/wp/one/one001/mod/one001.mrl3",
                    ),
                    ReplacementAsset::new(
                        hmm_core::PackageFileId::new("readme"),
                        "Cool Greatsword v1.2/readme.txt",
                    ),
                ],
            })
            .expect("wrapped weapon package must route to the weapon adapter");
        let source = analysis.single_source().expect("single weapon source");
        assert_eq!(source.source_type().as_str(), "weapon");
    }

    #[test]
    fn router_builds_content_sealed_weapon_plan_from_artificial_bytes() {
        let adapter = MhwReplacementAdapter;
        let assets = artificial_weapon_assets();
        let analysis = adapter
            .analyze_replacement_assets(ReplacementAnalysisRequest {
                game_id: GameId::mhw(),
                assets: assets.clone(),
            })
            .expect("artificial weapon analysis");
        let source = analysis.single_source().expect("single weapon source");
        // 这个 stable_id 与全量 catalog 的真实 one002 重合（WR-05 起 seed 退役，
        // 人工 fixture 的目标解析走全量 catalog）。
        let binding = ReplacementBinding::new(
            ReplacementBindingId::parse("binding-weapon").expect("binding id"),
            ModId::new("weapon-mod"),
            ProfileId::new("default"),
            source.id().clone(),
            hmm_core::ReplacementTargetId::parse(
                "mhw:weapon:0784b06e3b1e031bee9d1da31deeb995cba0d35dca4f7583f1cd8a019c5facc1",
            )
            .expect("catalog target id"),
            1,
        )
        .expect("weapon binding");

        // 解析出的必须是全量 catalog 的真实条目，不是退役 seed 的人工命名。
        let resolved = weapon_target(binding.target_id()).expect("catalog weapon target");
        assert_eq!(resolved.internal_id(), "one002");
        assert!(resolved
            .display_name()
            .values()
            .all(|name| !name.contains("WR-04")));

        let plan = adapter
            .build_retarget_plan_with_content(
                RetargetPlanRequest {
                    game_id: GameId::mhw(),
                    binding,
                    assets,
                },
                &ArtificialContentReader {
                    mod3: artificial_mod3(),
                    mrl3: artificial_mrl3(),
                },
            )
            .expect("content-aware weapon plan");

        assert_eq!(plan.actions().len(), 2);
        assert_eq!(
            plan.actions()
                .iter()
                .filter(|action| action.content_transform().is_some())
                .count(),
            1
        );
        assert!(plan
            .actions()
            .iter()
            .all(|action| action.target_internal_id() == "one002"));
        let facts = plan.adapter_facts().expect("sealed adapter facts");
        assert_eq!(facts.adapter_id(), WEAPON_ADAPTER_ID);
        assert_eq!(facts.part_count(), 1);
        assert_eq!(facts.file_count(), 2);
        plan.validate_transform_facts()
            .expect("transform facts remain internally consistent");
    }
}
