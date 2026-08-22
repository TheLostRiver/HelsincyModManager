use super::{
    analyze_mhw_weapon_assets, build_mhw_weapon_mrl3_transform_invocation, MhwWeaponCatalogSource,
    WeaponAnalysisError, WeaponBinaryError, WeaponMainId, WeaponModelPair, WeaponSourceClosure,
    WeaponTargetMetadata, WeaponTargetStatus, MHW_WEAPON_BINARY_MAX_BYTES,
    MHW_WEAPON_MRL3_TEXTURE_PATH_TRANSFORMER_ID, MHW_WEAPON_MRL3_TEXTURE_PATH_TRANSFORMER_VERSION,
};
use crate::armor_retarget::{
    resolve_target_allowing_legacy_ids, MhwArmorCatalog, MhwArmorReplacementAdapter,
};
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
/// 聚合 catalog（armor v2 + weapon v1）的版本号，Production 与 Sandbox 共用，
/// 与 armor、weapon 各自的 catalog_version 相互独立（治理契约见
/// EQUIPMENT_CATALOG_GOVERNANCE.md）。
const REPLACEMENT_CATALOG_VERSION: &str = "mhw-replacement-v1";
const WEAPON_ADAPTER_ID: &str = "mhw.weapon";
const WEAPON_STRATEGY_ID: &str = "mrl3-texture-path";
const WEAPON_STRATEGY_VERSION: u32 = 1;

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
        let closure = analyze_mhw_weapon_assets(&request.assets).map_err(map_analysis_error)?;
        analysis_from_closure(&closure)
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
        let closure = analyze_mhw_weapon_assets(&request.assets).map_err(map_analysis_error)?;
        if request.binding.source_id() != closure.source_id() {
            return Err(ReplacementAdapterError::SourceBindingMismatch);
        }

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

        let loaded_pairs = load_pair_contents(&closure, content_reader)?;
        let source = source_from_closure(&closure)?;
        let actions = build_weapon_actions(&closure, &target_main, &loaded_pairs)?;
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
            source_closure_digest(&closure, &loaded_pairs),
            part_set_digest(&closure),
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

fn is_weapon_path(path: &str) -> bool {
    path.replace('\\', "/").starts_with("nativePC/wp/")
}

fn ensure_mhw(game_id: &GameId) -> ReplacementAdapterResult<()> {
    if game_id == &GameId::mhw() {
        Ok(())
    } else {
        Err(ReplacementAdapterError::UnsupportedGame)
    }
}

fn analysis_from_closure(
    closure: &WeaponSourceClosure,
) -> ReplacementAdapterResult<ReplacementAnalysis> {
    let source = source_from_closure(closure)?;
    let warnings = closure
        .warnings()
        .iter()
        .map(|_| ReplacementWarning::WeaponPartialPartSet)
        .collect();
    ReplacementAnalysis::new(GameId::mhw(), vec![source], closure.asset_count(), warnings)
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
    let mut actions = Vec::with_capacity(loaded_pairs.len() * 2);
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
    Ok(actions)
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
