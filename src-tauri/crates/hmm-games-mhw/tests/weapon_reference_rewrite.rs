//! #336 切片①：文件名前缀改写规则 + 引用定向前缀匹配。
//!
//! 夹具字节是合成的，但**引用串全部复刻真实数据的形态**。防具侧曾因为夹具用了
//! `f_121_0000_extra.mod3` 这种不符合真实命名约定的合成名，让代码、文档、测试三方自洽地错
//! （#337）；这里每条引用都标出实测出处：
//!
//! | 引用形态 | 出处 | 旧版行为 |
//! | --- | --- | --- |
//! | `wp\two\two003\mod\two003_BML` | 沙箱包 two003.mrl3 | `ReferenceAmbiguous` |
//! | `wp\two\bs_two012\mod\bs_two012_BML` | 沙箱包 bs_two012.mrl3 | `ReferenceAmbiguous` |
//! | `wp\swo\Tamonowo\Tamonowo_BML` | 沙箱包 swo035.mrl3 | `ReferenceUnsafe` |
//! | `wp\Sakurad\Sakurad_BML` | 游戏目录 two029.mrl3（只 3 段） | `ReferenceUnsafe` |
//! | `wp\two\DARKMOON\DARKMOON_BML` | 游戏目录 two018.mrl3 | `ReferenceUnsafe` |
//! | `wp\two\textures\opulent_BML` | 游戏目录 bs_two012.mrl3 | `ReferenceUnsafe` |
//! | `pl\f_equip\mangie\goldfish\skin_BM` | 游戏目录 two018.mrl3（跨类引用） | 原样（正确） |
//! | `Assets\default_tex\CM\country_road_hor[1]_CM-00` | 原版共享贴图，**名字带方括号** | `ReferenceUnsafe` |
//! | `wp\swo\swo026\mod\swo026_BML` | 沙箱包 saya035ol.mrl3（**别的**槽位） | 原样（正确） |

use hmm_core::PackageFileId;
use hmm_games_mhw::{
    analyze_mhw_weapon_assets, transform_mhw_weapon_mrl3_texture_paths, WeaponMainId,
    WeaponModelPair,
};
use hmm_ports::ReplacementAsset;

// 以下常量与三个合成构造器与 tests/weapon_binary.rs 保持一致（Rust 集成测试各自独立
// 编译，无法共享 helper）。改动其中任何一个都要同步另一处。
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

fn pair(main_id: &str, part_id: &str) -> WeaponModelPair {
    let family = main_id
        .strip_prefix("bs_")
        .unwrap_or(main_id)
        .trim_end_matches(|character: char| character.is_ascii_digit());
    let assets = [
        ReplacementAsset::new(
            PackageFileId::new("artificial-mod3"),
            format!("nativePC/wp/{family}/{main_id}/mod/{part_id}.mod3"),
        ),
        ReplacementAsset::new(
            PackageFileId::new("artificial-mrl3"),
            format!("nativePC/wp/{family}/{main_id}/mod/{part_id}.mrl3"),
        ),
    ];
    analyze_mhw_weapon_assets(&assets)
        .expect("artificial pair closure")
        .sole_unit()
        .expect("恰好一个可重定向单元")
        .clone()
        .pairs()[0]
        .clone()
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

/// 跑一次改写，返回 (改写后的引用列表, 报告)。
fn rewrite(
    main_id: &str,
    target_main_id: &str,
    references: &[&str],
) -> (Vec<String>, hmm_games_mhw::WeaponMrl3TransformReport) {
    let model_pair = pair(main_id, main_id);
    let mod3 = artificial_mod3(&[ARTIFICIAL_MATERIAL]);
    let mrl3 = artificial_mrl3(references, &[ARTIFICIAL_MATERIAL_HASH]);
    let target = WeaponMainId::parse(target_main_id).expect("target main id");

    let output = transform_mhw_weapon_mrl3_texture_paths(&model_pair, &target, &mod3, &mrl3)
        .expect("transform must not fail");
    let paths = mrl3_paths(output.bytes(), references.len());
    (paths, output.report().clone())
}

#[test]
fn textures_named_after_the_source_part_are_rewritten_by_prefix() {
    /*
     * L5：真实贴图叫 `two003_BML`，只「包含」部件 ID。旧规则只认整段相等或去扩展名后
     * 相等，于是判 ReferenceAmbiguous，整个改写失败（已用真实 MRL3 字节实测）。
     */
    let (paths, report) = rewrite(
        "two003",
        "two019",
        &[
            r"wp\two\two003\mod\two003_BML",
            r"wp\two\two003\mod\two003_NM",
            r"wp\two\two003\mod\two003_RMT",
        ],
    );

    assert_eq!(
        paths,
        vec![
            r"wp\two\two019\mod\two019_BML".to_owned(),
            r"wp\two\two019\mod\two019_NM".to_owned(),
            r"wp\two\two019\mod\two019_RMT".to_owned(),
        ],
    );
    assert_eq!(report.rewritten_reference_count(), 3);
}

#[test]
fn the_bs_prefix_is_part_of_the_replaced_part_id() {
    /*
     * 参照实现把 `bs_two012_BML.dds` 改名为 `two020_BML.dds`——源含 `bs_`、目标不含，
     * **整个部件 ID 前缀**被替换。这条边界由真机实验独立验证。
     */
    let (paths, _) = rewrite(
        "bs_two012",
        "two020",
        &[r"wp\two\bs_two012\mod\bs_two012_BML"],
    );

    assert_eq!(paths, vec![r"wp\two\two020\mod\two020_BML".to_owned()]);
}

#[test]
fn slot_independent_author_directories_are_left_untouched() {
    /*
     * L3：想让 Mod 可重定向的作者本来就把贴图放在与槽位无关的目录。这些引用旧版一律
     * 判 ReferenceUnsafe，导致整个 Mod 不可重定向——而它们是**主流形态**，不是边缘情况。
     */
    let references = [
        r"wp\swo\Tamonowo\Tamonowo_BML",
        r"wp\Sakurad\Sakurad_BML",
        r"wp\two\DARKMOON\DARKMOON_BML",
        r"wp\two\textures\opulent_BML",
        r"pl\f_equip\mangie\goldfish\skin_BM",
        r"vfx\mod\wp\wp03\md_wp03_000_BML",
    ];
    let (paths, report) = rewrite("swo035", "swo026", &references);

    assert_eq!(
        paths,
        references
            .iter()
            .map(|r| (*r).to_owned())
            .collect::<Vec<_>>(),
        "与槽位无关的引用必须逐字保留",
    );
    assert_eq!(report.rewritten_reference_count(), 0);
    assert_eq!(report.changed_byte_count(), 0, "一个字节都不该被写");
}

#[test]
fn stock_texture_names_containing_square_brackets_are_accepted() {
    /*
     * L4：`Assets\default_tex\CM\country_road_hor[1]_CM-00` 是**原版**贴图名，
     * 旧版的字节白名单不允许 `[` `]`，整个 MRL3 解析失败。
     */
    let (paths, report) = rewrite(
        "two003",
        "two019",
        &[
            r"Assets\default_tex\CM\country_road_hor[1]_CM-00",
            r"wp\two\two003\mod\two003_BML",
        ],
    );

    // 带方括号的原版名原样保留，同一文件里我们自己的引用照常改写。
    assert_eq!(paths[0], r"Assets\default_tex\CM\country_road_hor[1]_CM-00");
    assert_eq!(paths[1], r"wp\two\two019\mod\two019_BML");
    assert_eq!(report.rewritten_reference_count(), 1);
}

#[test]
fn references_rooted_at_a_different_slot_are_left_untouched() {
    // 真实案例：saya035ol.mrl3 引用 `wp\swo\swo026\mod\swo026_BML`，是**别的**槽位。
    let (paths, report) = rewrite("swo035", "swo019", &[r"wp\swo\swo026\mod\swo026_BML"]);

    assert_eq!(paths, vec![r"wp\swo\swo026\mod\swo026_BML".to_owned()]);
    assert_eq!(report.rewritten_reference_count(), 0);
}

#[test]
fn a_filename_that_cannot_be_renamed_safely_still_fails_closed() {
    /*
     * 守卫②：部件 ID 在文件名里出现两次，无法判断作者意图，仍必须失败关闭。
     *
     * 本切片**刻意不做降级**。「改不动就保留原引用」只有在源槽位贴图真的被留在原路径时
     * 才是正确结果（参照实现的策略 A），而那是切片② 分类器的职责。若在这里就降级，
     * 重定向会「成功」但贴图缺失——静默产出坏结果，比现在的失败关闭更糟。
     */
    let model_pair = pair("two003", "two003");
    let mod3 = artificial_mod3(&[ARTIFICIAL_MATERIAL]);
    let mrl3 = artificial_mrl3(
        &[r"wp\two\two003\mod\two003_two003_BML"],
        &[ARTIFICIAL_MATERIAL_HASH],
    );
    let target = WeaponMainId::parse("two019").expect("target main id");

    let error = transform_mhw_weapon_mrl3_texture_paths(&model_pair, &target, &mod3, &mrl3)
        .expect_err("a doubled part id must not be guessed at");
    assert_eq!(error.code(), "weapon_binary_reference_ambiguous");
}

#[test]
fn a_longer_digit_run_is_not_mistaken_for_the_part_id() {
    /*
     * 守卫①：部件 ID 是 `<prefix><3 位数字>`。若不检查命中后的下一个字符，
     * `two0031_x` 会被当成 `two003` + `1_x` 而错改成 `two0191_x`。
     */
    let (paths, _) = rewrite("two003", "two019", &[r"wp\two\two003\mod\two0031_x"]);

    assert_eq!(
        paths,
        vec![r"wp\two\two019\mod\two0031_x".to_owned()],
        "主 ID 段照常改写，但文件名段不得被误替换",
    );
}
