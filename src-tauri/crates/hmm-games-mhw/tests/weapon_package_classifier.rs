//! #336 切片②：武器包两遍分类器 + 随行文件进入重定向计划。
//! #336 切片③：可执行 / 脚本拒绝清单——命中的文件不产出动作、计入 facts。
//!
//! 夹具是三个真实第三方包的**完整路径清单**（纯字符串，逐条来自沙箱实测），二进制字节
//! 合成。真实包本身不入库（`自动测试不得使用第三方 Mod 包`），但路径清单必须原样保留——
//! 防具侧（#337）代码、文档、测试三方自洽地错，根源正是夹具用了 `f_121_0000_extra.mod3`
//! 这种不符合真实命名约定的合成名。
//!
//! | 夹具 | 沙箱包 | 形态出处 |
//! | --- | --- | --- |
//! | [`BLACK_KNIGHT_TWO003`] | `external-import-package-26f952af` | 贴图/`.dds` 中间产物/`.exe` 全在源槽位 `mod/` 内 |
//! | [`BLACK_KNIGHT_BS_TWO012`] | `external-import-package-bf658e3c` | 同上，但源槽位带 `bs_` 前缀而目标不带 |
//! | [`FOX_LONGSWORD_SWO035`] | `external-import-package-43a4cae5` | 作者自建目录 + 族级 `epv/` `sound/` + 未登记部件 `saya035ol` |
//!
//! MRL3 引用串同样复刻实测形态，见 `tests/weapon_reference_rewrite.rs` 的出处表。

use hmm_core::{
    GameId, ModId, PackageFileId, ProfileId, ReplacementBinding, ReplacementBindingId, RetargetPlan,
};
use hmm_games_mhw::{
    analyze_mhw_weapon_assets, is_rejected_executable_file_name,
    transform_mhw_weapon_mrl3_texture_paths, MhwReplacementAdapter, MhwReplacementCatalog,
    WeaponMainId, MHW_EXECUTABLE_REJECT_EXTENSIONS,
};
use hmm_ports::{
    ReplacementAdapter, ReplacementAdapterError, ReplacementAdapterResult,
    ReplacementAnalysisRequest, ReplacementAsset, ReplacementAssetContentReader,
    ReplacementCatalogProvider, RetargetPlanRequest,
};

/// 「黑骑士特大」`external-import-package-26f952af`，源槽位 `two003`（大剑）。
///
/// 16 个文件全部落在源槽位 `mod/` 目录内：一对模型 + `.evwp` 附件 + `.tex` 贴图 +
/// 作者的 `.dds`/`.PNG` 中间产物 + 一个**转换器可执行文件**。
const BLACK_KNIGHT_TWO003: &[&str] = &[
    "nativePC/wp/two/two003/mod/131072_2599467785140006031 BML.dds",
    "nativePC/wp/two/two003/mod/131072_2599467785140006031 BML2.dds",
    "nativePC/wp/two/two003/mod/262144_7957987807731807324 nm.dds",
    "nativePC/wp/two/two003/mod/524288_15005912812814638262 RMT.dds",
    "nativePC/wp/two/two003/mod/524288_15005912812814638262 RMT2.dds",
    "nativePC/wp/two/two003/mod/MHWTexConverter_by_Jodo.exe",
    "nativePC/wp/two/two003/mod/two003.evwp",
    "nativePC/wp/two/two003/mod/two003.mod3",
    "nativePC/wp/two/two003/mod/two003.mrl3",
    "nativePC/wp/two/two003/mod/two003_BML.PNG",
    "nativePC/wp/two/two003/mod/two003_BML.dds",
    "nativePC/wp/two/two003/mod/two003_BML.tex",
    "nativePC/wp/two/two003/mod/two003_NM.tex",
    "nativePC/wp/two/two003/mod/two003_RMT.dds",
    "nativePC/wp/two/two003/mod/two003_RMT.tex",
    "nativePC/wp/two/two003/mod/two003_XM.tex",
];

/// 另一个黑骑士版本 `external-import-package-bf658e3c`，源槽位 `bs_two012`。
///
/// 参照实现把这个包重定向到 `two020`，实测改名 `bs_two012_BML.dds` → `two020_BML.dds`
/// ——**源含 `bs_`、目标不含**，整个部件 ID 前缀被替换。
const BLACK_KNIGHT_BS_TWO012: &[&str] = &[
    "nativePC/wp/two/bs_two012/mod/1 RMT.dds",
    "nativePC/wp/two/bs_two012/mod/MHWTexConverter_by_Jodo.exe",
    "nativePC/wp/two/bs_two012/mod/bs_two012.evwp",
    "nativePC/wp/two/bs_two012/mod/bs_two012.mod3",
    "nativePC/wp/two/bs_two012/mod/bs_two012.mrl3",
    "nativePC/wp/two/bs_two012/mod/bs_two012_BML.dds",
    "nativePC/wp/two/bs_two012/mod/bs_two012_BML.tex",
    "nativePC/wp/two/bs_two012/mod/bs_two012_FM.tex",
    "nativePC/wp/two/bs_two012/mod/bs_two012_NM.dds",
    "nativePC/wp/two/bs_two012/mod/bs_two012_NM.tex",
    "nativePC/wp/two/bs_two012/mod/bs_two012_RMT.tex",
    "nativePC/wp/two/bs_two012/mod/bs_two012_XM.tex",
];

/// 「泡狐太刀」`external-import-package-43a4cae5`，源槽位 `swo035`（太刀）。
///
/// 三种「槽位之外但仍在 `nativePC/wp/` 下」的形态同时出现：作者自建贴图目录
/// `wp/swo/Tamonowo/`、族级特效 `wp/swo/epv/`、族级音效 `wp/swo/sound/`。
/// 另含未登记部件 `saya035ol`——那是本切片仍失败关闭的一档，见 [`FOX_LONGSWORD_REGISTERED`]。
const FOX_LONGSWORD_SWO035: &[&str] = &[
    "nativePC/wp/swo/Tamonowo/PetalTama_BML.tex",
    "nativePC/wp/swo/Tamonowo/TamoRing_NM.tex",
    "nativePC/wp/swo/Tamonowo/Tamonowo_BML.tex",
    "nativePC/wp/swo/Tamonowo/helmsplitter.efx",
    "nativePC/wp/swo/Tamonowo/petals.efx",
    "nativePC/wp/swo/epv/hm_wp03_82.epv3",
    "nativePC/wp/swo/sound/hm_wp03_82.epvsp",
    "nativePC/wp/swo/swo035/epv/swo035.epv3",
    "nativePC/wp/swo/swo035/mod/saya035.evwp",
    "nativePC/wp/swo/swo035/mod/saya035.mod3",
    "nativePC/wp/swo/swo035/mod/saya035.mrl3",
    "nativePC/wp/swo/swo035/mod/saya035ol.mod3",
    "nativePC/wp/swo/swo035/mod/saya035ol.mrl3",
    "nativePC/wp/swo/swo035/mod/swo035.evwp",
    "nativePC/wp/swo/swo035/mod/swo035.mod3",
    "nativePC/wp/swo/swo035/mod/swo035.mrl3",
    "nativePC/wp/swo/swo035/mod/swo035_off_deco.ctc",
    "nativePC/wp/swo/swo035/mod/swo035_on_deco.ctc",
];

/// 泡狐太刀去掉未登记部件 `saya035ol` 后的清单——用来单独验证随行文件的两档落位，
/// 不与 ②b 的失败关闭纠缠。
fn fox_longsword_registered() -> Vec<&'static str> {
    FOX_LONGSWORD_SWO035
        .iter()
        .copied()
        .filter(|path| !path.contains("saya035ol"))
        .collect()
}

// 以下常量与两个合成构造器与 tests/weapon_binary.rs、tests/weapon_reference_rewrite.rs
// 保持一致（Rust 集成测试各自独立编译，无法共享 helper）。改动其中任何一个都要同步另两处。
const MOD3_HEADER_SIZE: usize = 320;
const MOD3_MATERIAL_ENTRY_SIZE: usize = 128;
const MOD3_MESH_ENTRY_SIZE: usize = 80;
const MRL3_HEADER_SIZE: usize = 40;
const MRL3_TEXTURE_ENTRY_SIZE: usize = 272;
const MRL3_MATERIAL_ENTRY_SIZE: usize = 56;
const MRL3_TEXTURE_PATH_OFFSET: usize = 16;
const MRL3_TEXTURE_PATH_CAPACITY: usize = 256;
const ARTIFICIAL_MATERIAL: &str = "ArtificialWeaponMaterial";
const ARTIFICIAL_MATERIAL_HASH: u32 = 0xa7f6_8bf8;

fn write_u16(bytes: &mut [u8], offset: usize, value: u16) {
    bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn write_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn write_u64(bytes: &mut [u8], offset: usize, value: u64) {
    bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

fn align_16(value: usize) -> usize {
    (value + 15) & !15
}

fn artificial_mod3(material_names: &[&str]) -> Vec<u8> {
    let material_offset = MOD3_HEADER_SIZE;
    let mesh_offset = material_offset + material_names.len() * MOD3_MATERIAL_ENTRY_SIZE;
    let vertex_offset = mesh_offset + MOD3_MESH_ENTRY_SIZE + 4;
    let vertex_buffer_size = 36usize;
    let face_offset = vertex_offset + vertex_buffer_size;
    let face_buffer_size = 8usize;
    let vertex_remap_offset = face_offset + face_buffer_size;
    let mut bytes = vec![0u8; vertex_remap_offset + 24];

    write_u32(&mut bytes, 0, 0x0044_4f4d);
    write_u16(&mut bytes, 4, 237);
    write_u16(&mut bytes, 8, 1);
    write_u16(
        &mut bytes,
        10,
        u16::try_from(material_names.len()).expect("artificial material count"),
    );
    write_u32(&mut bytes, 12, 3);
    write_u32(&mut bytes, 16, 3);
    write_u64(&mut bytes, 24, vertex_buffer_size as u64);
    write_u64(&mut bytes, 64, material_offset as u64);
    write_u64(&mut bytes, 72, mesh_offset as u64);
    write_u64(&mut bytes, 80, vertex_offset as u64);
    write_u64(&mut bytes, 88, face_offset as u64);
    write_u64(&mut bytes, 96, vertex_remap_offset as u64);

    for (index, name) in material_names.iter().enumerate() {
        let start = material_offset + index * MOD3_MATERIAL_ENTRY_SIZE;
        bytes[start..start + name.len()].copy_from_slice(name.as_bytes());
    }

    write_u16(&mut bytes, mesh_offset + 2, 3);
    write_u16(&mut bytes, mesh_offset + 6, 0);
    write_u16(&mut bytes, mesh_offset + 8, 1);
    bytes[mesh_offset + 14] = 12;
    write_u32(&mut bytes, mesh_offset + 32, 3);
    write_u32(&mut bytes, vertex_remap_offset, 4);
    bytes
}

fn artificial_mrl3(paths: &[&str], material_hashes: &[u32]) -> Vec<u8> {
    let texture_offset = MRL3_HEADER_SIZE;
    let material_offset = texture_offset + paths.len() * MRL3_TEXTURE_ENTRY_SIZE;
    let material_end = material_offset + material_hashes.len() * MRL3_MATERIAL_ENTRY_SIZE;
    let resource_offset = align_16(material_end);
    let mut bytes = vec![0u8; resource_offset + material_hashes.len() * 16];

    write_u32(&mut bytes, 0, 0x004c_524d);
    write_u32(&mut bytes, 4, 12);
    write_u32(
        &mut bytes,
        16,
        u32::try_from(material_hashes.len()).expect("artificial material count"),
    );
    write_u32(
        &mut bytes,
        20,
        u32::try_from(paths.len()).expect("artificial texture count"),
    );
    write_u64(&mut bytes, 24, texture_offset as u64);
    write_u64(&mut bytes, 32, material_offset as u64);

    for (index, path) in paths.iter().enumerate() {
        assert!(path.len() < MRL3_TEXTURE_PATH_CAPACITY);
        let record = texture_offset + index * MRL3_TEXTURE_ENTRY_SIZE;
        write_u32(&mut bytes, record, 0x241f_5deb);
        let path_start = record + MRL3_TEXTURE_PATH_OFFSET;
        bytes[path_start..path_start + path.len()].copy_from_slice(path.as_bytes());
    }

    for (index, hash) in material_hashes.iter().enumerate() {
        let record = material_offset + index * MRL3_MATERIAL_ENTRY_SIZE;
        write_u32(&mut bytes, record, 0x4516_e7ab);
        write_u32(&mut bytes, record + 4, *hash);
        write_u32(&mut bytes, record + 16, 16);
        write_u16(&mut bytes, record + 22, 2);
        write_u64(
            &mut bytes,
            record + 48,
            (resource_offset + index * 16) as u64,
        );
    }
    bytes
}

fn mrl3_paths(bytes: &[u8], count: usize) -> Vec<String> {
    (0..count)
        .map(|index| {
            let start =
                MRL3_HEADER_SIZE + index * MRL3_TEXTURE_ENTRY_SIZE + MRL3_TEXTURE_PATH_OFFSET;
            let field = &bytes[start..start + MRL3_TEXTURE_PATH_CAPACITY];
            let end = field.iter().position(|byte| *byte == 0).expect("path NUL");
            String::from_utf8(field[..end].to_vec()).expect("artificial ASCII path")
        })
        .collect()
}

/// 所有 `.mod3` / `.mrl3` 共用同一份合成字节——本切片验证的是**落位**，不是二进制内容。
struct SyntheticContentReader {
    mod3: Vec<u8>,
    mrl3: Vec<u8>,
}

impl ReplacementAssetContentReader for SyntheticContentReader {
    fn read_asset_content(
        &self,
        package_file_id: &PackageFileId,
        max_bytes: u64,
    ) -> ReplacementAdapterResult<Vec<u8>> {
        let id = package_file_id.as_str();
        let bytes = if id.ends_with(".mod3") {
            &self.mod3
        } else if id.ends_with(".mrl3") {
            &self.mrl3
        } else {
            return Err(ReplacementAdapterError::SourceContentUnavailable);
        };
        if bytes.len() as u64 > max_bytes {
            return Err(ReplacementAdapterError::SourceContentUnavailable);
        }
        Ok(bytes.clone())
    }
}

/// 包内路径同时用作 `package_file_id`——真实链路里两者一一对应，这里保持同构。
fn assets(paths: &[&str]) -> Vec<ReplacementAsset> {
    paths
        .iter()
        .map(|path| ReplacementAsset::new(PackageFileId::new(*path), *path))
        .collect()
}

fn target_id(internal_id: &str) -> hmm_core::ReplacementTargetId {
    MhwReplacementCatalog
        .replacement_catalog()
        .expect("aggregate catalog")
        .targets()
        .iter()
        .find(|target| target.internal_id() == internal_id)
        .unwrap_or_else(|| panic!("catalog must carry {internal_id}"))
        .id()
        .clone()
}

/// 跑完整链路：分析 → 绑定目标 → 产出计划。
fn plan_for(
    paths: &[&str],
    target_internal_id: &str,
    mrl3_references: &[&str],
) -> Result<RetargetPlan, ReplacementAdapterError> {
    let adapter = MhwReplacementAdapter;
    let assets = assets(paths);
    let analysis = adapter.analyze_replacement_assets(ReplacementAnalysisRequest {
        game_id: GameId::mhw(),
        assets: assets.clone(),
    })?;
    let source = analysis.single_source().expect("single weapon source");
    let binding = ReplacementBinding::new(
        ReplacementBindingId::parse("binding-classifier").expect("binding id"),
        ModId::new("classifier-mod"),
        ProfileId::new("default"),
        source.id().clone(),
        target_id(target_internal_id),
        1,
    )
    .expect("binding");

    adapter.build_retarget_plan_with_content(
        RetargetPlanRequest {
            game_id: GameId::mhw(),
            binding,
            assets,
            carries_package_companions: true,
        },
        &SyntheticContentReader {
            mod3: artificial_mod3(&[ARTIFICIAL_MATERIAL]),
            mrl3: artificial_mrl3(mrl3_references, &[ARTIFICIAL_MATERIAL_HASH]),
        },
    )
}

/// 多槽位包用：按源槽位编号挑单元，并显式指定这次是否承载包级随行资源。
///
/// `plan_for` 走 `single_source()`，多槽位包在那里拿不到源——那正是 `#349` 之前
/// 「两把武器就拒整包」的形状。
fn plan_for_source(
    paths: &[&str],
    source_main_id: &str,
    target_internal_id: &str,
    mrl3_references: &[&str],
    carries_package_companions: bool,
) -> Result<RetargetPlan, ReplacementAdapterError> {
    let adapter = MhwReplacementAdapter;
    let assets = assets(paths);
    let analysis = adapter.analyze_replacement_assets(ReplacementAnalysisRequest {
        game_id: GameId::mhw(),
        assets: assets.clone(),
    })?;
    let source = analysis
        .sources()
        .iter()
        .find(|source| source.internal_id() == source_main_id)
        .unwrap_or_else(|| panic!("分析必须报出源槽位 {source_main_id}"));
    let binding = ReplacementBinding::new(
        ReplacementBindingId::parse(format!("binding-{source_main_id}")).expect("binding id"),
        ModId::new("classifier-mod"),
        ProfileId::new("default"),
        source.id().clone(),
        target_id(target_internal_id),
        1,
    )
    .expect("binding");

    adapter.build_retarget_plan_with_content(
        RetargetPlanRequest {
            game_id: GameId::mhw(),
            binding,
            assets,
            carries_package_companions,
        },
        &SyntheticContentReader {
            mod3: artificial_mod3(&[ARTIFICIAL_MATERIAL]),
            mrl3: artificial_mrl3(mrl3_references, &[ARTIFICIAL_MATERIAL_HASH]),
        },
    )
}

/// 某个包内文件在计划里的目标路径。找不到 action 直接 panic——「随行文件被丢掉」
/// 正是 #336 要防的失败模式，不能退化成 `None` 被断言悄悄放过。
fn target_of<'a>(plan: &'a RetargetPlan, source_path: &str) -> &'a str {
    plan.actions()
        .iter()
        .find(|action| action.source_relative_path().as_str() == source_path)
        .unwrap_or_else(|| panic!("计划必须为 {source_path} 产出动作"))
        .target_relative_path()
        .as_str()
}

#[test]
fn a_real_weapon_package_carries_every_companion_file_into_the_plan() {
    /*
     * #336 的核心回归：旧版一遍二分把 `nativePC/wp/` 内的一切伴生文件当成包结构错误
     * 否决整包（`weapon_unsupported_resource`），库里 4/4 真实包不可重定向。
     */
    let plan = plan_for(
        BLACK_KNIGHT_TWO003,
        "two019",
        &[r"wp\two\two003\mod\two003_BML"],
    )
    .expect("真实黑骑士包必须能产出计划");

    // 除拒绝清单命中的一个 `.exe` 外，一个包内文件都不能丢：16 个文件 = 15 条动作。
    assert_eq!(plan.actions().len(), BLACK_KNIGHT_TWO003.len() - 1);
    for path in BLACK_KNIGHT_TWO003 {
        if path.ends_with(".exe") {
            continue;
        }
        target_of(&plan, path);
    }

    // 模型对：部件 ID 改名 + 换槽位段，只有 MRL3 带 content_transform。
    assert_eq!(
        target_of(&plan, "nativePC/wp/two/two003/mod/two003.mod3"),
        "nativePC/wp/two/two019/mod/two019.mod3"
    );
    assert_eq!(
        target_of(&plan, "nativePC/wp/two/two003/mod/two003.mrl3"),
        "nativePC/wp/two/two019/mod/two019.mrl3"
    );
    assert_eq!(
        plan.actions()
            .iter()
            .filter(|action| action.content_transform().is_some())
            .count(),
        1,
        "随行文件的字节与槽位无关，只有 MRL3 需要改写"
    );

    // 随行 · 需重定位：名字带部件 ID 的按前缀改名，不带的只换槽位段。
    assert_eq!(
        target_of(&plan, "nativePC/wp/two/two003/mod/two003_BML.tex"),
        "nativePC/wp/two/two019/mod/two019_BML.tex"
    );
    assert_eq!(
        target_of(&plan, "nativePC/wp/two/two003/mod/two003_XM.tex"),
        "nativePC/wp/two/two019/mod/two019_XM.tex"
    );
    assert_eq!(
        target_of(&plan, "nativePC/wp/two/two003/mod/two003.evwp"),
        "nativePC/wp/two/two019/mod/two019.evwp",
        "附件与模型同名，必须一起改名"
    );
    assert_eq!(
        target_of(&plan, "nativePC/wp/two/two003/mod/two003_BML.PNG"),
        "nativePC/wp/two/two019/mod/two019_BML.PNG",
        "扩展名大小写不参与判定"
    );
    assert_eq!(
        target_of(
            &plan,
            "nativePC/wp/two/two003/mod/131072_2599467785140006031 BML.dds"
        ),
        "nativePC/wp/two/two019/mod/131072_2599467785140006031 BML.dds",
        "名字不含部件 ID（且含空格）的作者中间产物只换槽位段"
    );

    let facts = plan.adapter_facts().expect("sealed adapter facts");
    assert_eq!(
        facts.strategy_version(),
        4,
        "action 集合每次变化（② 加随行文件、③ 去掉被拒绝的文件、#343 改名规则）都必须由 strategy_version 标记"
    );
    assert_eq!(facts.part_count(), 1);
    assert_eq!(
        facts.file_count(),
        (BLACK_KNIGHT_TWO003.len() - 1) as u32,
        "file_count 含随行文件，但不含被拒绝的文件"
    );
    assert_eq!(
        facts.excluded_file_count(),
        1,
        "被丢弃的 .exe 必须在 facts 里留痕，否则拒绝就是一次无痕的静默丢弃"
    );
    plan.validate_transform_facts()
        .expect("transform facts remain internally consistent");
}

#[test]
fn companion_filenames_drop_the_bs_prefix_when_the_target_slot_has_none() {
    /*
     * 参照实现的真机实验 B：`bs_two012` → `two020`，实测 `bs_two012_BML.dds` 落成
     * `two020_BML.dds`。整个部件 ID 前缀被替换，`bs_` 不是独立的可保留片段。
     */
    let plan = plan_for(
        BLACK_KNIGHT_BS_TWO012,
        "two020",
        &[r"wp\two\bs_two012\mod\bs_two012_BML"],
    )
    .expect("bs_ 前缀的真实包必须能产出计划");

    assert_eq!(plan.actions().len(), BLACK_KNIGHT_BS_TWO012.len() - 1);
    assert_eq!(
        target_of(&plan, "nativePC/wp/two/bs_two012/mod/bs_two012_XM.tex"),
        "nativePC/wp/two/two020/mod/two020_XM.tex"
    );
    assert_eq!(
        target_of(&plan, "nativePC/wp/two/bs_two012/mod/bs_two012_BML.dds"),
        "nativePC/wp/two/two020/mod/two020_BML.dds"
    );
    assert_eq!(
        target_of(&plan, "nativePC/wp/two/bs_two012/mod/bs_two012.evwp"),
        "nativePC/wp/two/two020/mod/two020.evwp"
    );
    assert_eq!(
        target_of(&plan, "nativePC/wp/two/bs_two012/mod/1 RMT.dds"),
        "nativePC/wp/two/two020/mod/1 RMT.dds",
        "名字不含部件 ID 的只换槽位段"
    );
}

#[test]
fn slot_independent_files_keep_their_original_path_while_in_slot_files_relocate() {
    /*
     * 两档落位的分界（真机实验实证）：作者自建贴图目录换任何目标槽位都仍被引用命中，
     * 搬走反而断链；源槽位目录内的文件则必须跟着走，否则目标槽位缺资源。
     */
    let paths = fox_longsword_registered();
    let plan = plan_for(&paths, "swo019", &[r"wp\swo\Tamonowo\Tamonowo_BML"])
        .expect("去掉未登记部件后的泡狐包必须能产出计划");

    assert_eq!(plan.actions().len(), paths.len());

    // 随行 · 原样：`nativePC/wp/` 下但与槽位无关。
    for verbatim in [
        "nativePC/wp/swo/Tamonowo/Tamonowo_BML.tex",
        "nativePC/wp/swo/Tamonowo/petals.efx",
        "nativePC/wp/swo/epv/hm_wp03_82.epv3",
        "nativePC/wp/swo/sound/hm_wp03_82.epvsp",
    ] {
        assert_eq!(
            target_of(&plan, verbatim),
            verbatim,
            "与槽位无关的伴生文件必须留在原路径"
        );
    }

    // 随行 · 需重定位：源槽位目录内，含非 `mod/` 的子目录。
    assert_eq!(
        target_of(&plan, "nativePC/wp/swo/swo035/epv/swo035.epv3"),
        "nativePC/wp/swo/swo019/epv/swo019.epv3",
        "槽位内的特效子目录同样重定位并改名"
    );
    // #336 正文改名表的两条实测样例。
    assert_eq!(
        target_of(&plan, "nativePC/wp/swo/swo035/mod/swo035_off_deco.ctc"),
        "nativePC/wp/swo/swo019/mod/swo019_off_deco.ctc"
    );
    // 太刀的鞘是本族已登记的副件，与主件用各自的部件 ID 改名。
    assert_eq!(
        target_of(&plan, "nativePC/wp/swo/swo035/mod/saya035.mod3"),
        "nativePC/wp/swo/swo019/mod/saya019.mod3"
    );
    assert_eq!(
        target_of(&plan, "nativePC/wp/swo/swo035/mod/saya035.evwp"),
        "nativePC/wp/swo/swo019/mod/saya019.evwp",
        "副件的附件跟着副件的部件 ID 走"
    );
}

#[test]
fn rewritten_references_land_on_files_the_plan_actually_produces() {
    /*
     * 切片② 的正确性闭环，也是真机验收「重定向后贴图正常」的静态对应物：
     * MRL3 改写后指向目标槽位的每一条引用，都必须能在计划的目标路径里找到对应文件；
     * 且不得残留任何指向**源**槽位的引用。
     *
     * 两侧共用 `part_rename` 的同一张对照表，这条测试钉住的正是「两处结果一致」——
     * 一旦有人只改其中一侧，重定向会「成功」但游戏里贴图缺失。
     */
    let references = [
        r"wp\two\two003\mod\two003_BML",
        r"wp\two\two003\mod\two003_NM",
        r"wp\two\two003\mod\two003_RMT",
        r"wp\two\two003\mod\two003_XM",
        // 与槽位无关的引用：一个字节都不该被改。
        r"wp\two\DARKMOON\DARKMOON_BML",
        r"Assets\default_tex\CM\country_road_hor[1]_CM-00",
    ];
    let plan = plan_for(BLACK_KNIGHT_TWO003, "two019", &references).expect("计划");

    let pair_closure = analyze_mhw_weapon_assets(&assets(BLACK_KNIGHT_TWO003))
        .expect("closure")
        .sole_unit()
        .expect("恰好一个可重定向单元")
        .clone();
    let pair = pair_closure.pairs().first().expect("one pair");
    let output = transform_mhw_weapon_mrl3_texture_paths(
        pair,
        &WeaponMainId::parse("two019").expect("target main id"),
        &artificial_mod3(&[ARTIFICIAL_MATERIAL]),
        &artificial_mrl3(&references, &[ARTIFICIAL_MATERIAL_HASH]),
    )
    .expect("transform");

    let rewritten = mrl3_paths(output.bytes(), references.len());
    assert_eq!(output.report().rewritten_reference_count(), 4);

    let target_paths: Vec<String> = plan
        .actions()
        .iter()
        .map(|action| action.target_relative_path().as_str().replace('/', "\\"))
        .collect();
    for reference in &rewritten {
        assert!(
            !reference.starts_with(r"wp\two\two003\"),
            "改写后不得残留指向源槽位的引用：{reference}"
        );
        if !reference.starts_with(r"wp\two\two019\") {
            continue;
        }
        // 引用不带扩展名，游戏加载的是 `.tex`。
        assert!(
            target_paths
                .iter()
                .any(|path| path == &format!(r"nativePC\{reference}.tex")),
            "引用 {reference} 在计划里没有对应文件——重定向后会断链"
        );
    }

    // 与槽位无关的两条逐字保留。
    assert_eq!(rewritten[4], r"wp\two\DARKMOON\DARKMOON_BML");
    assert_eq!(
        rewritten[5],
        r"Assets\default_tex\CM\country_road_hor[1]_CM-00"
    );
}

#[test]
fn a_part_variant_suffix_travels_with_the_model_instead_of_failing_the_package() {
    /*
     * ②b。`saya035ol` 过去判 `weapon_unknown_part` 并否决整包——库里唯一一把太刀 Mod
     * （泡狐太刀）因此完全不可重定向。
     *
     * 它本质上不是「没见过的部件」：前缀就是本族已登记的鞘 `saya035`，作者只是加了个
     * 变体标记 `ol`。改名规则本来就能算对，挡住它的只有 `parse_for_main` 的全等比较。
     *
     * 关键在于后缀必须**跟着走**：`saya035ol` → `saya019ol`。若丢掉后缀，它会和同族的
     * `saya035` 双双落到 `saya019`，两个不同模型互相覆盖，而且计划看起来完全成功。
     */
    let plan = plan_for(
        FOX_LONGSWORD_SWO035,
        "swo019",
        &[r"wp\swo\Tamonowo\Tamonowo_BML"],
    )
    .expect("带变体后缀的完整泡狐包必须能产出计划");

    assert_eq!(plan.actions().len(), FOX_LONGSWORD_SWO035.len());
    assert_eq!(
        target_of(&plan, "nativePC/wp/swo/swo035/mod/saya035ol.mod3"),
        "nativePC/wp/swo/swo019/mod/saya019ol.mod3"
    );
    assert_eq!(
        target_of(&plan, "nativePC/wp/swo/swo035/mod/saya035ol.mrl3"),
        "nativePC/wp/swo/swo019/mod/saya019ol.mrl3"
    );
    // 变体与本体是两个独立模型对，各自落位、互不覆盖。
    assert_eq!(
        target_of(&plan, "nativePC/wp/swo/swo035/mod/saya035.mod3"),
        "nativePC/wp/swo/swo019/mod/saya019.mod3"
    );
    let targets = plan
        .actions()
        .iter()
        .map(|action| action.target_relative_path().as_str())
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        targets.len(),
        plan.actions().len(),
        "目标路径必须两两不同，后缀丢失会在这里撞车"
    );
    // 三个模型对（swo035 主件、saya035 鞘、saya035ol 变体）都要带上 MRL3 改写。
    assert_eq!(
        plan.actions()
            .iter()
            .filter(|action| action.content_transform().is_some())
            .count(),
        3
    );
}

#[test]
fn a_variant_parts_rewritten_references_land_on_files_the_plan_actually_produces() {
    /*
     * ②b 的闭环校验，也是真机验收「贴图正常」的静态对应物。断言「计划里有 3 个 transform」
     * 只证明改写发生过，不证明改写**对**。这里要求：`saya035ol.mrl3` 改写后指向目标槽位的
     * 每一条引用，都能在计划的目标路径里找到对应文件，且不残留指向源槽位的引用。
     *
     * 这条最容易挂的地方正是变体后缀：引用侧走 `rename_part_prefix`、磁盘侧走
     * `part_for_role` + `with_variant_suffix`，两条不同的代码路径必须给出同一个名字。
     */
    let references = [
        r"wp\swo\swo035\mod\saya035ol_BML",
        r"wp\swo\swo035\mod\saya035ol_NM",
        // 与槽位无关的引用：一个字节都不该被改。
        r"wp\swo\Tamonowo\Tamonowo_BML",
    ];
    let mut paths = FOX_LONGSWORD_SWO035.to_vec();
    paths.extend([
        "nativePC/wp/swo/swo035/mod/saya035ol_BML.tex",
        "nativePC/wp/swo/swo035/mod/saya035ol_NM.tex",
    ]);
    let plan = plan_for(&paths, "swo019", &references).expect("计划");

    let closure = analyze_mhw_weapon_assets(&assets(&paths))
        .expect("closure")
        .sole_unit()
        .expect("恰好一个可重定向单元")
        .clone();
    let variant = closure
        .pairs()
        .iter()
        .find(|pair| pair.part_id().as_str() == "saya035ol")
        .expect("变体部件必须成对进入闭包");
    let output = transform_mhw_weapon_mrl3_texture_paths(
        variant,
        &WeaponMainId::parse("swo019").expect("target main id"),
        &artificial_mod3(&[ARTIFICIAL_MATERIAL]),
        &artificial_mrl3(&references, &[ARTIFICIAL_MATERIAL_HASH]),
    )
    .expect("transform");

    let rewritten = mrl3_paths(output.bytes(), references.len());
    assert_eq!(output.report().rewritten_reference_count(), 2);

    let target_paths = plan
        .actions()
        .iter()
        .map(|action| action.target_relative_path().as_str().replace('/', "\\"))
        .collect::<Vec<_>>();
    for reference in &rewritten {
        assert!(
            !reference.starts_with(r"wp\swo\swo035\"),
            "改写后不得残留指向源槽位的引用：{reference}"
        );
        if !reference.starts_with(r"wp\swo\swo019\") {
            continue;
        }
        assert!(
            target_paths
                .iter()
                .any(|path| path == &format!(r"nativePC\{reference}.tex")),
            "引用 {reference} 在计划里没有对应文件——重定向后会断链"
        );
    }
    assert_eq!(rewritten[0], r"wp\swo\swo019\mod\saya019ol_BML");
    assert_eq!(rewritten[2], r"wp\swo\Tamonowo\Tamonowo_BML");
}

#[test]
fn an_unregistered_part_prefix_is_carried_through_instead_of_failing_the_package() {
    /*
     * #343 的核心回归。改名只需要知道「源槽位数字 → 目标槽位数字」，**不需要事先登记
     * 部件前缀**。上一版从 role 推导目标部件名，因此前缀必须登记在
     * `WeaponFamily::secondary_part()` 里——而那张表只有三项，14 个族里 10 个为空，
     * 这些族的包只要带一个副件模型就判 `weapon_unknown_part` 并否决整包。
     *
     * 弓类就是其中之一：下面这个包在上一版是打不开的。
     */
    let paths = [
        "nativePC/wp/bow/bow013/mod/bow013.mod3",
        "nativePC/wp/bow/bow013/mod/bow013.mrl3",
        "nativePC/wp/bow/bow013/mod/bow013_BML.tex",
        // 前缀不在任何注册表里的副件模型
        "nativePC/wp/bow/bow013/mod/ya013.mod3",
        "nativePC/wp/bow/bow013/mod/ya013.mrl3",
        "nativePC/wp/bow/bow013/mod/ya013_BML.tex",
    ];
    let plan = plan_for(&paths, "bow019", &[r"wp\bow\bow013\mod\ya013_BML"])
        .expect("未登记前缀的副件不得否决整包");

    assert_eq!(plan.actions().len(), paths.len());
    for (source, target) in [
        ("bow013.mod3", "bow019.mod3"),
        ("ya013.mod3", "ya019.mod3"),
        ("ya013.mrl3", "ya019.mrl3"),
        ("ya013_BML.tex", "ya019_BML.tex"),
    ] {
        assert_eq!(
            target_of(&plan, &format!("nativePC/wp/bow/bow013/mod/{source}")),
            format!("nativePC/wp/bow/bow019/mod/{target}"),
            "{source} 的前缀必须逐字保留，只换槽位数字"
        );
    }
    // 两个模型对都要带上 MRL3 改写。
    assert_eq!(
        plan.actions()
            .iter()
            .filter(|action| action.content_transform().is_some())
            .count(),
        2
    );
}

#[test]
fn a_model_that_does_not_carry_the_source_slot_number_is_flagged_not_guessed() {
    /*
     * 放宽的是「前缀不必登记」，不是「什么都收」。源槽位目录内的模型若**不带本槽位的
     * 数字**，改名规则无从下手——把它当伴生文件搬运，它 MRL3 里指向源槽位的引用不会被
     * 改写，重定向后断链，而且是静默断链。所以它**不能**混进随行档。
     *
     * `#349`：但它也不该拖累整包。此前这里否决整包（`weapon_unknown_part`），于是一把
     * 完好的太刀因为多了个认不出名字的模型就整个不可用。现在它被单独标记、原样留在源
     * 路径，包照常可重定向——「不猜」与「不否决」是两件事。
     */
    for unknown in ["zzz999", "nodigits"] {
        let mut paths = fox_longsword_registered();
        let injected = format!("nativePC/wp/swo/swo035/mod/{unknown}.mod3");
        paths.push(&injected);

        let analysis = MhwReplacementAdapter
            .analyze_replacement_assets(ReplacementAnalysisRequest {
                game_id: GameId::mhw(),
                assets: assets(&paths),
            })
            .unwrap_or_else(|error| panic!("{unknown} 不该拒整包，实际 {error:?}"));

        assert_eq!(
            analysis.sources().len(),
            1,
            "{unknown} 仍然只有一个源槽位（太刀本体）"
        );

        // 分类器层面确认它落到了 `unresolved_models`，既没被猜、也没拖累整包。
        let unit = analyze_mhw_weapon_assets(&assets(&paths))
            .expect("分析应当成立")
            .sole_unit()
            .expect("恰好一个单元")
            .clone();
        assert_eq!(
            unit.unresolved_models()
                .iter()
                .map(|model| model.relative_path().as_str())
                .collect::<Vec<_>>(),
            vec![injected.as_str()],
            "{unknown} 必须被标为无法判断如何改写"
        );
        assert!(
            !unit
                .companions()
                .iter()
                .any(|companion| companion.relative_path().as_str() == injected),
            "{unknown} 绝不能混进随行档"
        );
    }
}

#[test]
fn a_digit_or_nested_part_id_after_the_part_prefix_is_not_treated_as_a_variant() {
    /*
     * 变体后缀的两条守卫，与 `rename_part_prefix` 逐字相同——两处必须对同一个文件名得出
     * 同一个结论，否则磁盘改名与 MRL3 引用改写会分叉。
     *
     * 守卫①：`saya0351` 是更长的数字串，不是 `saya035` + 变体 `1`。
     * 守卫②：`saya035saya035` 无法判断作者意图，且内层不会被改写。
     */
    // 基线：注入之前本体有几个模型对。注入不该改变这个数。
    let baseline_pairs = analyze_mhw_weapon_assets(&assets(&fox_longsword_registered()))
        .expect("基线分析")
        .sole_unit()
        .expect("恰好一个单元")
        .pairs()
        .len();

    for unknown in ["saya0351", "saya035saya035"] {
        let mut paths = fox_longsword_registered();
        let injected = format!("nativePC/wp/swo/swo035/mod/{unknown}.mod3");
        paths.push(&injected);

        // `#349`：守卫的语义不变（**不猜**），变的只是处置——标记那个文件，而不是否决整包。
        let unit = analyze_mhw_weapon_assets(&assets(&paths))
            .unwrap_or_else(|error| panic!("{unknown} 不该拒整包，实际 {error:?}"))
            .sole_unit()
            .expect("恰好一个单元")
            .clone();

        assert_eq!(
            unit.unresolved_models()
                .iter()
                .map(|model| model.relative_path().as_str())
                .collect::<Vec<_>>(),
            vec![injected.as_str()],
            "{unknown} 不得被当成变体后缀，必须标为无法判断如何改写"
        );
        assert!(
            !unit
                .companions()
                .iter()
                .any(|companion| companion.relative_path().as_str() == injected),
            "{unknown} 绝不能混进随行档"
        );
        // 太刀本体照常可重定向：模型对数与注入前一致。
        assert_eq!(
            unit.pairs().len(),
            baseline_pairs,
            "{unknown} 不该影响本体的模型对"
        );
    }
}

#[test]
fn files_outside_the_weapon_tree_never_enter_the_plan() {
    /*
     * 第二遍分类的「无关」档：`nativePC/wp/` 之外的东西与本武器无关，忽略即可。
     * 真实包几乎必然带 readme 与预览图，过去这类文件曾把整包判成混合包。
     */
    let mut paths = BLACK_KNIGHT_TWO003.to_vec();
    paths.extend([
        "readme.txt",
        "预览图.png",
        "nativePC/pl/f_equip/pl078_0000/arm/mod/f_arm078_0000.mod3",
        "nativePC/sound/wwise/Windows/pl_act_vo_f_07_m.nbnk",
    ]);

    let plan = plan_for(&paths, "two019", &[r"wp\two\two003\mod\two003_BML"]).expect("计划");

    assert_eq!(
        plan.actions().len(),
        BLACK_KNIGHT_TWO003.len() - 1,
        "武器树之外的文件不产出动作（减掉的 1 是拒绝清单命中的 .exe）"
    );
    for ignored in ["readme.txt", "预览图.png"] {
        assert!(
            !plan
                .actions()
                .iter()
                .any(|action| action.source_relative_path().as_str() == ignored),
            "{ignored} 不该进入重定向计划"
        );
    }
}

#[test]
fn a_companion_filename_that_cannot_be_renamed_safely_fails_closed() {
    /*
     * 守卫②：部件 ID 在文件名里出现两次，无法判断作者意图。
     *
     * 这里刻意**不**降级（留在源路径）：降级的完整语义要求 MRL3 那一侧同步不改写并计入
     * 告警，而告警变体在切片⑤。只做磁盘侧降级会静默产出「引用指向目标槽位、文件却留在
     * 源槽位」的安装——静默产出坏结果比失败关闭更糟。
     *
     * 报的是引用侧的码而非 `weapon_unsupported_resource`（「只支持 .mod3 与 .mrl3」）：
     * 磁盘改名与引用改写共用同一张对照表和同两条守卫，是同一个根因的两个出口。
     */
    let mut paths = BLACK_KNIGHT_TWO003.to_vec();
    paths.push("nativePC/wp/two/two003/mod/two003_two003_BML.tex");

    let error = plan_for(&paths, "two019", &[r"wp\two\two003\mod\two003_BML"])
        .expect_err("重复部件 ID 的文件名不得被猜");

    assert!(
        matches!(
            error,
            ReplacementAdapterError::AnalysisRejected {
                code: "weapon_binary_reference_ambiguous"
            }
        ),
        "实际是 {error:?}"
    );
}

#[test]
fn a_longer_digit_run_in_a_companion_filename_is_not_mistaken_for_the_part_id() {
    /*
     * 守卫①：部件 ID 是 `<prefix><3 位数字>`。若不检查命中后的下一个字符，
     * `two0031_x.tex` 会被当成 `two003` + `1_x.tex` 而错改成 `two0191_x.tex`。
     */
    let mut paths = BLACK_KNIGHT_TWO003.to_vec();
    paths.push("nativePC/wp/two/two003/mod/two0031_x.tex");

    let plan = plan_for(&paths, "two019", &[r"wp\two\two003\mod\two003_BML"]).expect("计划");

    assert_eq!(
        target_of(&plan, "nativePC/wp/two/two003/mod/two0031_x.tex"),
        "nativePC/wp/two/two019/mod/two0031_x.tex",
        "槽位段照常改写，但文件名段不得被误替换"
    );
}

/// 计划里有没有为某个包内文件产出动作。
fn has_action_for(plan: &RetargetPlan, source_path: &str) -> bool {
    plan.actions()
        .iter()
        .any(|action| action.source_relative_path().as_str() == source_path)
}

/// 计划的所有目标路径末段。断言「不会写出什么」时看目标而不是来源——真正决定
/// 落盘文件名的是目标路径。
fn target_file_names(plan: &RetargetPlan) -> Vec<&str> {
    plan.actions()
        .iter()
        .map(|action| {
            action
                .target_relative_path()
                .as_str()
                .rsplit_once('/')
                .map(|(_, name)| name)
                .unwrap_or_default()
        })
        .collect()
}

#[test]
fn the_authors_texture_converter_executable_never_enters_the_plan() {
    /*
     * #336 切片③ 的核心负测。真机实验 B 的快照比对显示
     * `MHWTexConverter_by_Jodo.exe`（30208 B）落进了游戏目录；两个真实黑骑士包都带着它。
     *
     * 「不产出动作」就是「不落盘」：`RetargetPlan.actions` 是重定向安装唯一的写入来源，
     * staging 与 commit 都只遍历它。
     */
    for (paths, target, source_slot) in [
        (BLACK_KNIGHT_TWO003, "two019", "two003"),
        (BLACK_KNIGHT_BS_TWO012, "two020", "bs_two012"),
    ] {
        let reference = format!(r"wp\two\{source_slot}\mod\{source_slot}_BML");
        let plan = plan_for(paths, target, &[reference.as_str()]).expect("计划");
        let executable = format!("nativePC/wp/two/{source_slot}/mod/MHWTexConverter_by_Jodo.exe");

        assert!(
            !has_action_for(&plan, &executable),
            "{executable} 不得产出动作"
        );
        assert_eq!(plan.actions().len(), paths.len() - 1);
        assert_eq!(
            plan.adapter_facts()
                .expect("sealed adapter facts")
                .excluded_file_count(),
            1
        );
        // 更强的不变量：不看来源，直接确认计划**写不出**任何可执行文件。
        assert!(
            !target_file_names(&plan)
                .iter()
                .any(|name| is_rejected_executable_file_name(name)),
            "计划的目标路径里不得出现可执行文件"
        );
    }
}

#[test]
fn every_extension_on_the_reject_list_is_dropped_from_the_plan() {
    /*
     * 逐条验证清单本身，而不是只验 `.exe`：往源槽位塞进清单里的每一个扩展名，
     * 断言无一产出动作、模型对不受影响、计数与清单长度一致。
     *
     * 遍历导出的常量而不是在测试里另抄一份——两份清单会漂移，而漂移的方向必然是
     * 「代码里少了一项而测试仍然全绿」。
     */
    let mut paths = BLACK_KNIGHT_TWO003
        .iter()
        .filter(|path| !path.ends_with(".exe"))
        .copied()
        .collect::<Vec<_>>();
    let injected = MHW_EXECUTABLE_REJECT_EXTENSIONS
        .iter()
        .map(|extension| format!("nativePC/wp/two/two003/mod/two003_tool.{extension}"))
        .collect::<Vec<_>>();
    paths.extend(injected.iter().map(String::as_str));

    let plan = plan_for(&paths, "two019", &[r"wp\two\two003\mod\two003_BML"]).expect("计划");

    for path in &injected {
        assert!(!has_action_for(&plan, path), "{path} 不得产出动作");
    }
    assert_eq!(
        plan.actions().len(),
        BLACK_KNIGHT_TWO003.len() - 1,
        "注入的可执行文件一个都不该进计划"
    );
    assert_eq!(
        plan.adapter_facts()
            .expect("sealed adapter facts")
            .excluded_file_count(),
        MHW_EXECUTABLE_REJECT_EXTENSIONS.len() as u32
    );
}

#[test]
fn the_reject_list_survives_case_and_windows_trailing_dot_spellings() {
    /*
     * 只做 `ends_with(".exe")` 的检查有两个现实中可用的绕法：
     *
     * - `X.EXE`——NTFS 大小写不敏感，落到磁盘上就是同一个文件；
     * - `x.exe.`——Win32 创建文件时剥掉最后一段的尾随点，磁盘上同样是 `x.exe`。
     *
     * 两种写法都必须被拒绝清单挡下。尾随**空格**（`x.exe `）走不到这里：整条路径的
     * 首尾空白在 `parse_safe_relative_path` 就 fail closed，见下一条测试。
     */
    let mut paths = BLACK_KNIGHT_TWO003
        .iter()
        .filter(|path| !path.ends_with(".exe"))
        .copied()
        .collect::<Vec<_>>();
    let evasions = [
        "nativePC/wp/two/two003/mod/UPPER.EXE",
        "nativePC/wp/two/two003/mod/Mixed.Dll",
        "nativePC/wp/two/two003/mod/trailing_dot.exe.",
        "nativePC/wp/two/two003/mod/trailing_dots.bat...",
    ];
    paths.extend(evasions);

    let plan = plan_for(&paths, "two019", &[r"wp\two\two003\mod\two003_BML"]).expect("计划");

    for path in evasions {
        assert!(!has_action_for(&plan, path), "{path} 不得产出动作");
    }
    assert_eq!(plan.actions().len(), BLACK_KNIGHT_TWO003.len() - 1);
    assert_eq!(
        plan.adapter_facts()
            .expect("sealed adapter facts")
            .excluded_file_count(),
        evasions.len() as u32
    );
}

#[test]
fn a_trailing_space_filename_fails_closed_before_classification() {
    /*
     * `x.exe ` 在 Windows 上落盘同样是 `x.exe`，但它压根到不了拒绝清单：整条相对路径
     * 的首尾空白在 `parse_safe_relative_path` 就被判 `weapon_unsafe_path`。
     *
     * 钉住这条是因为**拒绝清单依赖它**：清单只剥尾随点与空格来还原 Win32 语义，
     * 若哪天有人放宽了路径校验的空白规则，这里会转红，提醒同步加固清单侧。
     */
    let mut paths = BLACK_KNIGHT_TWO003.to_vec();
    paths.push("nativePC/wp/two/two003/mod/trailing_space.exe ");

    let error = plan_for(&paths, "two019", &[r"wp\two\two003\mod\two003_BML"])
        .expect_err("尾随空格的路径必须失败关闭");

    assert!(
        matches!(
            error,
            ReplacementAdapterError::AnalysisRejected {
                code: "weapon_unsafe_path"
            }
        ),
        "实际是 {error:?}"
    );
}

#[test]
fn real_game_asset_extensions_are_never_caught_by_the_reject_list() {
    /*
     * 反向的一半：拒绝清单不得误伤任何正常的 Mod 资源。这里逐条走三个真实包里
     * 实际出现过的扩展名——它们全部来自沙箱实测的路径清单，不是我编的。
     *
     * 误伤的后果比漏放更直接：重定向后游戏里贴图或特效缺失，而计划仍显示成功。
     */
    for name in [
        "two003.mod3",
        "two003.mrl3",
        "two003.evwp",
        "two003_BML.tex",
        "two003_BML.dds",
        "two003_BML.PNG",
        "swo035_off_deco.ctc",
        "swo035.epv3",
        "hm_wp03_82.epvsp",
        "petals.efx",
        "1 RMT.dds",
        "131072_2599467785140006031 BML.dds",
    ] {
        assert!(
            !is_rejected_executable_file_name(name),
            "{name} 是正常的游戏资源，不得被拒绝清单挡下"
        );
    }
}

/// 泡狐太刀包的族级随行文件：作者自建目录 `Tamonowo/` + 族级 `epv/` `sound/`。
///
/// 它们在 `nativePC/wp/swo/` 下、但不在任何槽位目录内，所以属于**包**。
const FOX_LONGSWORD_PACKAGE_COMPANIONS: &[&str] = &[
    "nativePC/wp/swo/Tamonowo/PetalTama_BML.tex",
    "nativePC/wp/swo/Tamonowo/TamoRing_NM.tex",
    "nativePC/wp/swo/Tamonowo/Tamonowo_BML.tex",
    "nativePC/wp/swo/Tamonowo/helmsplitter.efx",
    "nativePC/wp/swo/Tamonowo/petals.efx",
    "nativePC/wp/swo/epv/hm_wp03_82.epv3",
    "nativePC/wp/swo/sound/hm_wp03_82.epvsp",
];

/// 泡狐包再加一把同族太刀，构造真实形态的多槽位包：两个槽位共享族级随行文件。
fn fox_longsword_with_second_slot() -> Vec<&'static str> {
    let mut paths = FOX_LONGSWORD_SWO035.to_vec();
    paths.extend([
        "nativePC/wp/swo/swo040/mod/swo040.mod3",
        "nativePC/wp/swo/swo040/mod/swo040.mrl3",
    ]);
    paths
}

/// `#349` 切片③b：族级随行文件属于**包**，分析层不再把它们挂到某个槽位上。
#[test]
fn family_scoped_companions_belong_to_the_package_not_to_any_slot() {
    let analysis =
        analyze_mhw_weapon_assets(&assets(&fox_longsword_with_second_slot())).expect("多槽位分析");

    assert_eq!(analysis.units().len(), 2, "两把太刀 = 两个单元");
    assert_eq!(
        analysis
            .package_companions()
            .iter()
            .map(|companion| companion.relative_path().as_str())
            .collect::<Vec<_>>(),
        FOX_LONGSWORD_PACKAGE_COMPANIONS,
        "族级作者目录与族级 epv/ sound/ 必须报在包级"
    );
    for unit in analysis.units() {
        for companion in unit.companions() {
            assert!(
                companion
                    .relative_path()
                    .as_str()
                    .contains(unit.root().main_id().as_str()),
                "单元 {} 里只该有本槽位目录内的伴生文件，实际有 {}",
                unit.root().main_id().as_str(),
                companion.relative_path().as_str()
            );
        }
    }
}

/// 一个包只装一次：承载者带上包级随行文件，非承载者一个都不带。
///
/// 少了这道区分，多槽位包会让同一个族级贴图被两个绑定各产出一次，在 `InstallPlan` 里
/// 撞成阻断冲突——两把武器一起装直接装不上。
#[test]
fn only_the_designated_carrier_puts_package_companions_into_its_plan() {
    let paths = fox_longsword_with_second_slot();
    let references = [r"wp\swo\Tamonowo\Tamonowo_BML"];

    let carrier = plan_for_source(&paths, "swo035", "swo019", &references, true)
        .expect("承载者必须能产出计划");
    let passenger = plan_for_source(&paths, "swo040", "swo029", &references, false)
        .expect("非承载者必须能产出计划");

    let carried = |plan: &RetargetPlan| {
        FOX_LONGSWORD_PACKAGE_COMPANIONS
            .iter()
            .filter(|path| {
                plan.actions()
                    .iter()
                    .any(|action| action.source_relative_path().as_str() == **path)
            })
            .copied()
            .collect::<Vec<_>>()
    };

    assert_eq!(
        carried(&carrier),
        FOX_LONGSWORD_PACKAGE_COMPANIONS,
        "承载者必须带上全部包级随行文件"
    );
    assert_eq!(
        carried(&passenger),
        Vec::<&str>::new(),
        "非承载者一个包级随行文件都不该带"
    );

    // 包级文件的处置是「原路径保留」：承载者也不得改写它们的路径。
    for path in FOX_LONGSWORD_PACKAGE_COMPANIONS {
        assert_eq!(target_of(&carrier, path), *path);
    }

    // 两个计划的目标路径无交集——这是「两把武器能一起装」的静态前提。
    let carrier_targets = carrier
        .actions()
        .iter()
        .map(|action| action.target_relative_path().as_str())
        .collect::<std::collections::BTreeSet<_>>();
    let passenger_targets = passenger
        .actions()
        .iter()
        .map(|action| action.target_relative_path().as_str())
        .collect::<std::collections::BTreeSet<_>>();
    assert!(
        carrier_targets.is_disjoint(&passenger_targets),
        "两个绑定的产出不得撞车"
    );
}

/// 修掉的 bug：切片① 把族级随行文件挂在「排序第一」的单元上，于是玩家只装排序靠后的
/// 那把武器时，族级贴图与特效**整批丢失**——计划仍然显示成功，游戏里缺件。
///
/// 现在承载者由组装方指定，装哪个槽位都带得上。
#[test]
fn package_companions_travel_with_whichever_slot_the_player_installs() {
    let paths = fox_longsword_with_second_slot();
    let references = [r"wp\swo\Tamonowo\Tamonowo_BML"];

    // `swo040` 在按槽位根排序里排在 `swo035` 之后，正是旧口径下会丢文件的那一侧。
    let later_slot = plan_for_source(&paths, "swo040", "swo029", &references, true)
        .expect("装排序靠后的槽位也必须能产出计划");

    for path in FOX_LONGSWORD_PACKAGE_COMPANIONS {
        assert_eq!(
            target_of(&later_slot, path),
            *path,
            "只装 swo040 时族级文件也必须进计划、且留原路径"
        );
    }
}
