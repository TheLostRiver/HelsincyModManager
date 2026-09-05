use super::path::{classify_armor_asset, is_valid_armor_slot, ArmorAsset};
use super::{ArmorPathError, ArmorResourcePath, MhwArmorCatalog};
use crate::is_rejected_executable_file_name;
use hmm_core::{
    GameId, InstallTargetPath, PackageFileId, ReplacementAdapterFacts, ReplacementAnalysis,
    ReplacementSource, ReplacementSourceId, ReplacementTarget, ReplacementTargetKind,
    ReplacementWarning, RetargetAction, RetargetPlan, REPLACEMENT_ADAPTER_FACTS_SCHEMA_VERSION,
};
use hmm_ports::{
    ReplacementAdapter, ReplacementAdapterError, ReplacementAdapterResult,
    ReplacementAnalysisRequest, ReplacementAsset, ReplacementCatalogError,
    ReplacementCatalogProvider, RetargetPlanRequest,
};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

const ARMOR_ADAPTER_ID: &str = "mhw.armor";
const ARMOR_STRATEGY_ID: &str = "slot-token-rename";
/// v1（#342）：部位段与目录深度不再参与判定，随行文件进入计划，编号段改名。
///
/// 防具侧此前从不产出 adapter facts（没有二进制改写、没有 transformer），所以这是
/// 第一版。facts 只在**确实排除了文件**时才产出——常态包一条 facts 都不多写，
/// 既有防具安装的 manifest 形状因此完全不变。
const ARMOR_STRATEGY_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, Default)]
pub struct MhwArmorReplacementAdapter;

#[derive(Debug, Clone)]
struct ParsedReplacementAsset {
    package_file_id: PackageFileId,
    path: ArmorResourcePath,
}

impl ReplacementAdapter for MhwArmorReplacementAdapter {
    fn game_id(&self) -> GameId {
        GameId::mhw()
    }

    fn analyze_replacement_assets(
        &self,
        request: ReplacementAnalysisRequest,
    ) -> ReplacementAdapterResult<ReplacementAnalysis> {
        self.ensure_game(&request.game_id)?;
        analyze_assets(&request.assets).map(|(analysis, _)| analysis)
    }

    fn build_retarget_plan(
        &self,
        request: RetargetPlanRequest,
    ) -> ReplacementAdapterResult<RetargetPlan> {
        self.ensure_game(&request.game_id)?;
        let (analysis, classified) = analyze_assets(&request.assets)?;
        let source = match analysis.sources() {
            [] => return Err(ReplacementAdapterError::UnrecognizedSourceSlot),
            [source] if source.is_supported() => source.clone(),
            _ => return Err(ReplacementAdapterError::AmbiguousSourceSlot),
        };

        if request.binding.source_id() != source.id() {
            return Err(ReplacementAdapterError::SourceBindingMismatch);
        }

        let target = find_target(request.binding.target_id())?;
        let target_path_family = target_path_family(&target)?;
        if target.target_type().as_str() != "armor"
            || !is_valid_armor_slot(target.internal_id())
            || source.path_family() != target_path_family
        {
            return Err(ReplacementAdapterError::UnsupportedReplacementTarget);
        }

        let mut actions = classified
            .in_slot
            .into_iter()
            .filter(|asset| {
                asset.path.path_family() == source.path_family()
                    && asset.path.slot() == source.internal_id()
            })
            .map(|asset| {
                let target_relative_path = asset.path.retarget(target.internal_id())?;
                Ok((
                    asset.package_file_id,
                    asset.path.normalized_path().clone(),
                    target_relative_path,
                ))
            })
            .collect::<Result<Vec<_>, ArmorPathError>>()
            .map_err(map_path_error)?;

        /*
         * 原路径保留（#342）。两类：作者自建目录（`mod_pl_rosedress/`）和槽位内的 `.tex`。
         * 共同理由是它们被 MRL3 按**路径**引用，而防具侧零二进制改写——搬走就是静默断链。
         * 目标路径 = 原路径，`content_transform` 同样是 `None`。
         */
        actions.extend(classified.kept_in_place.into_iter().map(|asset| {
            (
                asset.package_file_id,
                asset.relative_path.clone(),
                asset.relative_path,
            )
        }));

        let actions = actions
            .into_iter()
            .map(|(package_file_id, source_path, target_path)| {
                RetargetAction::new(
                    package_file_id,
                    source_path,
                    target_path,
                    source.id().clone(),
                    source.internal_id(),
                    target.internal_id(),
                    source.path_family(),
                    target_path_family,
                )
                .map_err(|_| ReplacementAdapterError::InvalidRetargetPlan)
            })
            .collect::<ReplacementAdapterResult<Vec<_>>>()?;

        let warnings = (source.internal_id() == target.internal_id())
            .then_some(ReplacementWarning::SourceMatchesTarget)
            .into_iter()
            .collect();

        let plan = RetargetPlan::new(request.binding, source, actions, warnings)
            .map_err(|_| ReplacementAdapterError::InvalidRetargetPlan)?;

        /*
         * 审计留痕（#336 切片③）。防具侧不做二进制改写，所以 facts 里没有 transformer
         * 三元组——只带拒绝清单的计数。没有它，「装完之后包里少了一个文件」事后无从追溯。
         */
        if classified.excluded_count == 0 {
            return Ok(plan);
        }
        let facts = ReplacementAdapterFacts::new(
            REPLACEMENT_ADAPTER_FACTS_SCHEMA_VERSION,
            ARMOR_ADAPTER_ID,
            ARMOR_STRATEGY_ID,
            ARMOR_STRATEGY_VERSION,
            armor_source_closure_digest(&plan),
            armor_slot_digest(plan.source().internal_id()),
            plan.content_transform_set_sha256(),
        )
        .map(|facts| facts.with_excluded_file_count(classified.excluded_count))
        .map_err(|_| ReplacementAdapterError::InvalidRetargetPlan)?;
        plan.with_adapter_facts(facts)
            .map_err(|_| ReplacementAdapterError::InvalidRetargetPlan)
    }
}

impl MhwArmorReplacementAdapter {
    fn ensure_game(&self, game_id: &GameId) -> ReplacementAdapterResult<()> {
        if game_id == &self.game_id() {
            Ok(())
        } else {
            Err(ReplacementAdapterError::UnsupportedGame)
        }
    }
}

fn analyze_assets(
    assets: &[ReplacementAsset],
) -> ReplacementAdapterResult<(ReplacementAnalysis, ClassifiedAssets)> {
    let classified = classify_assets(assets)?;
    let mut grouped_sources = BTreeMap::<(String, String), bool>::new();
    for asset in &classified.in_slot {
        grouped_sources.insert(
            (
                asset.path.path_family().to_owned(),
                asset.path.slot().to_owned(),
            ),
            asset.path.is_supported(),
        );
    }

    let sources = grouped_sources
        .into_iter()
        .map(|((path_family, slot), supported)| {
            let equip_family = path_family
                .rsplit('/')
                .next()
                .expect("MHW armor path families always contain an equip family");
            ReplacementSource::new(
                ReplacementSourceId::parse(format!("mhw:armor:{equip_family}:{slot}"))
                    .map_err(|_| ReplacementAdapterError::InvalidRetargetPlan)?,
                GameId::mhw(),
                ReplacementTargetKind::parse("armor")
                    .map_err(|_| ReplacementAdapterError::InvalidRetargetPlan)?,
                slot,
                path_family,
                supported,
            )
            .map_err(|_| ReplacementAdapterError::InvalidRetargetPlan)
        })
        .collect::<ReplacementAdapterResult<Vec<_>>>()?;

    let mut warnings = Vec::new();
    if sources.is_empty() {
        warnings.push(ReplacementWarning::NoSupportedAssets);
    }
    if sources.len() > 1 {
        warnings.push(ReplacementWarning::MultipleSources);
    }
    if sources.iter().any(|source| !source.is_supported()) {
        warnings.push(ReplacementWarning::UnsupportedSource);
    }

    // 随行·原样的文件同样会被安装，计入「本次影响的文件数」；被拒绝的不计——它们不落盘。
    let matched = classified.in_slot.len() + classified.kept_in_place.len();
    let analysis = ReplacementAnalysis::new(GameId::mhw(), sources, matched, warnings)
        .map_err(|_| ReplacementAdapterError::InvalidRetargetPlan)?;
    Ok((analysis, classified))
}

/// 把包内文件分档（#342）。
///
/// 旧实现是一遍二分：`ArmorResourcePath::parse` 成功就收下，失败（除 `NotArmorPath`）
/// 就**否决整包**。而 `parse` 当时要求槽位之后必须是 `arm/mod` 且恰好 7 段，于是真实
/// 套装的 `body` `helm` `leg` `wst` 四个部位、以及一切作者自建子目录全部撞上——库里的
/// 防具包 100% 不可重定向。
///
/// 现在只有一档失败：`UnsafePath`（路径穿越等真实安全信号）。其余分四档：
///
/// - 槽位目录内、且不是 `.tex` → 随行·需重定位，按编号段改名
/// - 槽位目录内的 `.tex` → **原路径保留**，理由见 [`is_path_referenced_texture`]
/// - `pl/<equip>/` 下但与槽位无关（作者自建目录）→ 原路径保留（实验 A 实证）
/// - 命中可执行 / 脚本拒绝清单 → 排除并计数（#336 切片③）
/// - 其余 → 忽略
fn classify_assets(assets: &[ReplacementAsset]) -> ReplacementAdapterResult<ClassifiedAssets> {
    let mut classified = ClassifiedAssets::default();
    for asset in assets {
        let kind = classify_armor_asset(asset.relative_path())
            .map_err(|_| ReplacementAdapterError::UnsafeRetargetPath)?;
        if matches!(kind, ArmorAsset::Unrelated) {
            continue;
        }
        let file_name = asset
            .relative_path()
            .replace('\\', "/")
            .rsplit('/')
            .next()
            .unwrap_or_default()
            .to_owned();

        /*
         * 拒绝清单排在归档之前，与武器侧同一顺序：命中的文件不该有机会落进任何一档
         * 随行处置，也不该因为恰好长得像别的东西而改变结论。
         */
        if is_rejected_executable_file_name(&file_name) {
            classified.excluded_count += 1;
            continue;
        }

        // 一律用分类器归一化后的路径，不要拿原始字符串重新解析——小写根与外层目录
        // 会在第二次解析时被打回原形（#345）。
        let (keep_in_place, normalized_path) = match &kind {
            ArmorAsset::SlotIndependent {
                normalized_path, ..
            } => (true, normalized_path.clone()),
            ArmorAsset::InSlot(path) => (
                is_path_referenced_texture(&file_name),
                path.normalized_path().clone(),
            ),
            ArmorAsset::Unrelated => unreachable!("filtered above"),
        };
        if !keep_in_place {
            let ArmorAsset::InSlot(path) = kind else {
                unreachable!("only in-slot assets relocate");
            };
            classified.in_slot.push(ParsedReplacementAsset {
                package_file_id: asset.package_file_id().clone(),
                path,
            });
            continue;
        }
        classified.kept_in_place.push(KeptInPlaceAsset {
            package_file_id: asset.package_file_id().clone(),
            relative_path: normalized_path,
        });
    }
    Ok(classified)
}

/// 这个文件会被 MRL3 按**路径**引用吗？是则不能搬。
///
/// 防具侧**不做二进制改写**（#336 实验 A：`内容变化 0`，MRL3 里零条引用指向源槽位），
/// 所以搬走一个被按路径引用的文件 = 引用断链，而且是**静默**断链：计划成功、装完游戏里
/// 贴图直接没了。
///
/// 判据是扩展名 `.tex`，这不是「记录某个 Mod 的文件类型」那种词表，而是格式事实：MRL3 是
/// 材质/贴图表，它按路径引用的就只有 `.tex`。
///
/// > #336 正文由真机实验 B 推断出的「被 MRL3 引用的文件留在原路径」不是可靠口径：那次观测
/// > 里「留下的文件」与「被引用的文件」恰好重合，但两者没有因果关系。按扩展名判定才是稳的
/// > ——一个没被任何 MRL3 引用的 `.tex` 同样不该搬，因为下一版包可能就引用它了。
///
/// **武器侧不适用**：武器走策略 B，`.tex` 照搬且同步改写 MRL3 引用，闭环由
/// `rewritten_references_land_on_files_the_plan_actually_produces` 钉住，真机也验过贴图正常。
fn is_path_referenced_texture(file_name: &str) -> bool {
    file_name
        .rsplit_once('.')
        .is_some_and(|(_, extension)| extension.eq_ignore_ascii_case("tex"))
}

/// 原路径安装的随行文件：换任何目标槽位都仍被引用命中，搬走反而断链。
#[derive(Debug, Clone)]
struct KeptInPlaceAsset {
    package_file_id: PackageFileId,
    relative_path: InstallTargetPath,
}

#[derive(Debug, Default)]
struct ClassifiedAssets {
    in_slot: Vec<ParsedReplacementAsset>,
    kept_in_place: Vec<KeptInPlaceAsset>,
    excluded_count: u32,
}

fn find_target(
    target_id: &hmm_core::ReplacementTargetId,
) -> ReplacementAdapterResult<ReplacementTarget> {
    MhwArmorCatalog
        .find_replacement_target(target_id)
        .map_err(|error| match error {
            ReplacementCatalogError::TargetNotFound { target_id } => {
                ReplacementAdapterError::TargetCatalogMissing { target_id }
            }
            ReplacementCatalogError::CatalogUnavailable
            | ReplacementCatalogError::CatalogInvalid
            | ReplacementCatalogError::UnsupportedSchemaVersion { .. } => {
                ReplacementAdapterError::TargetCatalogUnavailable
            }
        })
}

fn target_path_family(target: &ReplacementTarget) -> ReplacementAdapterResult<&str> {
    target
        .metadata()
        .get("path_family")
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or(ReplacementAdapterError::UnsupportedReplacementTarget)
}

/// 源闭包摘要：本次计划覆盖了包里的哪些文件。
///
/// 防具侧不做二进制改写（实验 A 实证 `内容变化 0`），所以摘要取的是**路径集合**而非
/// 字节内容——武器侧那份哈希 MOD3/MRL3 字节的做法在这里没有对应物。
fn armor_source_closure_digest(plan: &RetargetPlan) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"hmm-mhw-armor-source-closure-v1\0");
    for action in plan.actions() {
        hasher.update(action.package_file_id().as_str().as_bytes());
        hasher.update([0]);
        hasher.update(action.source_relative_path().as_str().as_bytes());
        hasher.update([0]);
    }
    format!("{:x}", hasher.finalize())
}

/// 防具侧没有「部件集合」的概念（部位不再参与判定），槽位就是身份。
fn armor_slot_digest(slot: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"hmm-mhw-armor-slot-v1\0");
    hasher.update(slot.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn map_path_error(error: ArmorPathError) -> ReplacementAdapterError {
    match error {
        ArmorPathError::UnsafePath => ReplacementAdapterError::UnsafeRetargetPath,
        ArmorPathError::NotArmorPath
        | ArmorPathError::MalformedArmorPath
        | ArmorPathError::InvalidSlot => ReplacementAdapterError::UnrecognizedSourceSlot,
    }
}
