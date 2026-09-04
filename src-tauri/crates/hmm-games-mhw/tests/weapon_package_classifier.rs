//! #336 切片②：武器包两遍分类器 + 随行文件进入重定向计划。
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
    analyze_mhw_weapon_assets, transform_mhw_weapon_mrl3_texture_paths, MhwReplacementAdapter,
    MhwReplacementCatalog, WeaponMainId,
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

    // 一个包内文件都不能丢：16 个文件 = 16 条动作。
    assert_eq!(plan.actions().len(), BLACK_KNIGHT_TWO003.len());
    for path in BLACK_KNIGHT_TWO003 {
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
        2,
        "随行文件进入计划改变了 action 集合，必须由 strategy_version 标记"
    );
    assert_eq!(facts.part_count(), 1);
    assert_eq!(
        facts.file_count(),
        BLACK_KNIGHT_TWO003.len() as u32,
        "file_count 现在含随行文件"
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

    assert_eq!(plan.actions().len(), BLACK_KNIGHT_BS_TWO012.len());
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

    let pair_closure = analyze_mhw_weapon_assets(&assets(BLACK_KNIGHT_TWO003)).expect("closure");
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
fn an_unregistered_part_inside_the_source_slot_still_fails_closed() {
    /*
     * ②b：`saya035ol` 是**模型**（`.mod3`/`.mrl3`），不是伴生文件。若当伴生文件搬运，
     * 它 MRL3 里指向源槽位的贴图引用不会被改写，重定向后断链。正确做法是让未登记部件
     * 走正常的配对 + 改写管线，但那要改 `WeaponPartId` 模型，而部件注册表是
     * `WEAPON_RETARGET_DESIGN.md:167` 明文冻结的口径，需独立设计变更。
     *
     * 因此泡狐太刀在本切片后仍不可重定向——**这是刻意的失败关闭，不是遗漏**。
     */
    let error = MhwReplacementAdapter
        .analyze_replacement_assets(ReplacementAnalysisRequest {
            game_id: GameId::mhw(),
            assets: assets(FOX_LONGSWORD_SWO035),
        })
        .expect_err("未登记部件不得被当成伴生文件搬运");

    assert!(matches!(
        error,
        ReplacementAdapterError::AnalysisRejected {
            code: "weapon_unknown_part"
        }
    ));
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
        BLACK_KNIGHT_TWO003.len(),
        "武器树之外的文件不产出动作"
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

#[test]
fn executables_still_travel_with_the_package_until_the_reject_list_lands() {
    /*
     * **已知缺口，刻意钉住。** 真实包里的 `MHWTexConverter_by_Jodo.exe` 是作者的贴图
     * 转换工具，参照实现真的把它写进了游戏目录（实测 30208 B）。#336 洞见 5 的拒绝清单
     * 是切片③，落地后这条测试会转红——那时应改成断言 `.exe` 不产出动作。
     *
     * 本切片不抢先做：普通安装链路目前同样没有可执行文件拒绝清单，重定向路径不引入新的
     * 暴露面类别；而拒绝清单要连审计留痕、UI 提示与 `SECURITY.md` 一起做才完整。
     */
    let plan = plan_for(
        BLACK_KNIGHT_TWO003,
        "two019",
        &[r"wp\two\two003\mod\two003_BML"],
    )
    .expect("计划");

    assert_eq!(
        target_of(
            &plan,
            "nativePC/wp/two/two003/mod/MHWTexConverter_by_Jodo.exe"
        ),
        "nativePC/wp/two/two019/mod/MHWTexConverter_by_Jodo.exe",
        "切片③ 落地后这里应改为断言不产出动作"
    );
}
