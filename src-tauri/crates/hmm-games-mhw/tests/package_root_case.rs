//! #345：包内游戏根目录段的大小写与外层包裹目录。
//!
//! 真实 Mod 压缩包里 `nativepc` / `NativePC` 等写法很常见（作者手打的目录名）。Windows
//! 文件系统大小写不敏感，所以这类包**安装完全正常**——坏的只有重定向：路由按段做大小写
//! 敏感比较，整包被判「不是武器包」，最终报「该 Mod 不是当前可自动处理的单源外观包」，
//! 而真实原因是路径大小写。错误信息完全指不到原因。
//!
//! 判据是**等价性**而不是「小写也能跑」：三种写法必须产出**逐字相同**的计划。只断言
//! 「不报错」会漏掉「识别通过但目标路径继承了包内的错误大小写」这一类更隐蔽的失败。

use hmm_core::{
    GameId, ModId, PackageFileId, ProfileId, ReplacementBinding, ReplacementBindingId, RetargetPlan,
};
use hmm_games_mhw::{MhwReplacementAdapter, MhwReplacementCatalog};
use hmm_ports::{
    ReplacementAdapter, ReplacementAdapterError, ReplacementAdapterResult,
    ReplacementAnalysisRequest, ReplacementAsset, ReplacementAssetContentReader,
    ReplacementCatalogProvider, RetargetPlanRequest,
};

/// 「黑骑士特大」的真实路径清单（源槽位 `two003`），根段留作占位符。
const WEAPON_PACKAGE: &[&str] = &[
    "{root}/wp/two/two003/mod/two003.evwp",
    "{root}/wp/two/two003/mod/two003.mod3",
    "{root}/wp/two/two003/mod/two003.mrl3",
    "{root}/wp/two/two003/mod/two003_BML.tex",
    "{root}/wp/two/two003/mod/two003_NM.tex",
];

/// 「玫瑰礼服」的真实路径清单（源槽位 `pl078_0000`），根段留作占位符。
const ARMOR_PACKAGE: &[&str] = &[
    "{root}/pl/f_equip/mod_pl_rosedress/npc046_002_BML.tex",
    "{root}/pl/f_equip/pl078_0000/arm/mod/f_arm078_0000.mod3",
    "{root}/pl/f_equip/pl078_0000/arm/mod/f_arm078_0000.mrl3",
    "{root}/pl/f_equip/pl078_0000/body/mod/f_body078_0000.mod3",
    "{root}/pl/f_equip/pl078_0000/body/mod/f_body078_0000.mrl3",
];

/// 真实包里出现过的三种根段写法，外加作者在游戏根之外再包一层目录的形态。
const ROOT_SPELLINGS: [&str; 4] = ["nativePC", "nativepc", "NativePC", "Cool Mod v1.2/nativePC"];

const MOD3_HEADER_SIZE: usize = 320;
const MOD3_MATERIAL_ENTRY_SIZE: usize = 128;
const MOD3_MESH_ENTRY_SIZE: usize = 80;
const MRL3_HEADER_SIZE: usize = 40;
const MRL3_TEXTURE_ENTRY_SIZE: usize = 272;
const MRL3_MATERIAL_ENTRY_SIZE: usize = 56;
const MRL3_TEXTURE_PATH_OFFSET: usize = 16;
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

fn artificial_mod3() -> Vec<u8> {
    let material_offset = MOD3_HEADER_SIZE;
    let mesh_offset = material_offset + MOD3_MATERIAL_ENTRY_SIZE;
    let vertex_offset = mesh_offset + MOD3_MESH_ENTRY_SIZE + 4;
    let vertex_buffer_size = 36usize;
    let face_offset = vertex_offset + vertex_buffer_size;
    let vertex_remap_offset = face_offset + 8;
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
    bytes[material_offset..material_offset + ARTIFICIAL_MATERIAL.len()]
        .copy_from_slice(ARTIFICIAL_MATERIAL.as_bytes());
    write_u16(&mut bytes, mesh_offset + 2, 3);
    write_u16(&mut bytes, mesh_offset + 6, 0);
    write_u16(&mut bytes, mesh_offset + 8, 1);
    bytes[mesh_offset + 14] = 12;
    write_u32(&mut bytes, mesh_offset + 32, 3);
    write_u32(&mut bytes, vertex_remap_offset, 4);
    bytes
}

fn artificial_mrl3(paths: &[&str]) -> Vec<u8> {
    let texture_offset = MRL3_HEADER_SIZE;
    let material_offset = texture_offset + paths.len() * MRL3_TEXTURE_ENTRY_SIZE;
    let resource_offset = (material_offset + MRL3_MATERIAL_ENTRY_SIZE + 15) & !15;
    let mut bytes = vec![0u8; resource_offset + 16];
    write_u32(&mut bytes, 0, 0x004c_524d);
    write_u32(&mut bytes, 4, 12);
    write_u32(&mut bytes, 16, 1);
    write_u32(&mut bytes, 20, u32::try_from(paths.len()).expect("count"));
    write_u64(&mut bytes, 24, texture_offset as u64);
    write_u64(&mut bytes, 32, material_offset as u64);
    for (index, path) in paths.iter().enumerate() {
        let record = texture_offset + index * MRL3_TEXTURE_ENTRY_SIZE;
        write_u32(&mut bytes, record, 0x241f_5deb);
        let start = record + MRL3_TEXTURE_PATH_OFFSET;
        bytes[start..start + path.len()].copy_from_slice(path.as_bytes());
    }
    write_u32(&mut bytes, material_offset, 0x4516_e7ab);
    write_u32(&mut bytes, material_offset + 4, ARTIFICIAL_MATERIAL_HASH);
    write_u32(&mut bytes, material_offset + 16, 16);
    write_u16(&mut bytes, material_offset + 22, 2);
    write_u64(&mut bytes, material_offset + 48, resource_offset as u64);
    bytes
}

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

fn spell(paths: &[&str], root: &str) -> Vec<String> {
    paths
        .iter()
        .map(|path| path.replace("{root}", root))
        .collect()
}

fn assets(paths: &[String]) -> Vec<ReplacementAsset> {
    paths
        .iter()
        .map(|path| ReplacementAsset::new(PackageFileId::new(path.as_str()), path.as_str()))
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

/// 计划的可比较投影：来源 → 目标。来源用**归一化后**的路径，因此三种写法应完全一致。
fn plan_shape(plan: &RetargetPlan) -> Vec<(String, String)> {
    let mut shape = plan
        .actions()
        .iter()
        .map(|action| {
            (
                action.source_relative_path().as_str().to_owned(),
                action.target_relative_path().as_str().to_owned(),
            )
        })
        .collect::<Vec<_>>();
    shape.sort();
    shape
}

fn plan_for(paths: &[String], target_internal_id: &str) -> RetargetPlan {
    let adapter = MhwReplacementAdapter;
    let package = assets(paths);
    let analysis = adapter
        .analyze_replacement_assets(ReplacementAnalysisRequest {
            game_id: GameId::mhw(),
            assets: package.clone(),
        })
        .expect("分析必须成功");
    let source = analysis.single_source().expect("single source");
    let binding = ReplacementBinding::new(
        ReplacementBindingId::parse("binding-case").expect("binding id"),
        ModId::new("case-mod"),
        ProfileId::new("default"),
        source.id().clone(),
        target_id(target_internal_id),
        1,
    )
    .expect("binding");

    adapter
        .build_retarget_plan_with_content(
            RetargetPlanRequest {
                game_id: GameId::mhw(),
                binding,
                assets: package,
            },
            &SyntheticContentReader {
                mod3: artificial_mod3(),
                mrl3: artificial_mrl3(&[r"wp\two\two003\mod\two003_BML"]),
            },
        )
        .expect("计划必须成功")
}

#[test]
fn a_weapon_package_plans_identically_under_every_root_spelling() {
    let mut baseline: Option<Vec<(String, String)>> = None;
    for root in ROOT_SPELLINGS {
        let paths = spell(WEAPON_PACKAGE, root);
        let shape = plan_shape(&plan_for(&paths, "two019"));

        // 目标路径必须是规范大小写，而不是继承包内的写法。
        for (source, target) in &shape {
            assert!(
                source.starts_with("nativePC/") && target.starts_with("nativePC/"),
                "根段必须归一化：{root} → {source} → {target}"
            );
        }
        match &baseline {
            None => baseline = Some(shape),
            Some(expected) => assert_eq!(&shape, expected, "根段写法 {root} 产出了不同的计划"),
        }
    }
    assert_eq!(baseline.expect("baseline").len(), WEAPON_PACKAGE.len());
}

#[test]
fn an_armor_package_plans_identically_under_every_root_spelling() {
    /*
     * 防具侧此前连外层包裹目录都不支持（武器侧一直支持），所以这条同时钉住两件事：
     * 根段大小写归一化，以及 `Cool Mod v1.2/nativePC/...` 这种形态被识别。
     */
    let mut baseline: Option<Vec<(String, String)>> = None;
    for root in ROOT_SPELLINGS {
        let paths = spell(ARMOR_PACKAGE, root);
        let adapter = MhwReplacementAdapter;
        let package = assets(&paths);
        let analysis = adapter
            .analyze_replacement_assets(ReplacementAnalysisRequest {
                game_id: GameId::mhw(),
                assets: package.clone(),
            })
            .unwrap_or_else(|error| panic!("{root} 的分析必须成功，实际 {error:?}"));
        let source = analysis.single_source().expect("single source");
        let binding = ReplacementBinding::new(
            ReplacementBindingId::parse("binding-case-armor").expect("binding id"),
            ModId::new("case-armor-mod"),
            ProfileId::new("default"),
            source.id().clone(),
            target_id("pl123_0000"),
            1,
        )
        .expect("binding");
        let plan = adapter
            .build_retarget_plan(RetargetPlanRequest {
                game_id: GameId::mhw(),
                binding,
                assets: package,
            })
            .unwrap_or_else(|error| panic!("{root} 的计划必须成功，实际 {error:?}"));
        let shape = plan_shape(&plan);

        for (source, target) in &shape {
            assert!(
                source.starts_with("nativePC/") && target.starts_with("nativePC/"),
                "根段必须归一化：{root} → {source} → {target}"
            );
        }
        match &baseline {
            None => baseline = Some(shape),
            Some(expected) => assert_eq!(&shape, expected, "根段写法 {root} 产出了不同的计划"),
        }
    }
    assert_eq!(baseline.expect("baseline").len(), ARMOR_PACKAGE.len());
}

#[test]
fn only_the_root_segment_is_case_normalized() {
    /*
     * 放宽的**只有游戏根这一段**。往下的段不能跟着归一化：族名与部件 ID 的大小写是语法的
     * 一部分，文件名的大小写更要逐字带到目标路径上（真实包里有 `two003_BML.PNG` 这种）。
     */
    let paths = spell(&["{root}/wp/two/two003/mod/two003_BML.PNG"], "nativepc");
    let mut all = spell(WEAPON_PACKAGE, "nativepc");
    all.extend(paths);
    let plan = plan_for(&all, "two019");

    let shape = plan_shape(&plan);
    assert!(
        shape.iter().any(|(source, target)| {
            source == "nativePC/wp/two/two003/mod/two003_BML.PNG"
                && target == "nativePC/wp/two/two019/mod/two019_BML.PNG"
        }),
        "文件名的大小写必须逐字保留，实际 {shape:?}"
    );

    /*
     * 大写的族段不是合法语法，不能因为根段放宽了就跟着放宽。
     *
     * 这里给的是**完整的模型对**：若 `WP` 被误当成 `wp`，分析会成功并产出一个源。所以
     * 判据必须是「恰好 0 个源」，不能写成 `is_err() || sources().is_empty()`——那样
     * 「缺 .mrl3 导致的 IncompleteBinaryPair」也会让断言通过，两种情况都过等于没测。
     */
    let bogus_family = spell(
        &[
            "{root}/WP/two/two003/mod/two003.mod3",
            "{root}/WP/two/two003/mod/two003.mrl3",
        ],
        "nativePC",
    );
    let analysis = MhwReplacementAdapter
        .analyze_replacement_assets(ReplacementAnalysisRequest {
            game_id: GameId::mhw(),
            assets: assets(&bogus_family),
        })
        .expect("与武器树无关的文件应被忽略，而不是报错");
    assert!(
        analysis.sources().is_empty(),
        "`WP` 不是 `wp`，不得被当成武器树，实际 {:?}",
        analysis.sources()
    );
}

#[test]
fn mixed_case_duplicates_are_still_rejected_instead_of_silently_merged() {
    /*
     * 归一化不能把「同一个包里同时存在 `nativePC/a` 与 `nativepc/a`」悄悄合并成一个文件——
     * 那是真实的大小写碰撞信号，既有的碰撞检测必须仍然拦住它。
     */
    let mut paths = spell(WEAPON_PACKAGE, "nativePC");
    paths.extend(spell(WEAPON_PACKAGE, "nativepc"));

    let error = MhwReplacementAdapter
        .analyze_replacement_assets(ReplacementAnalysisRequest {
            game_id: GameId::mhw(),
            assets: assets(&paths),
        })
        .expect_err("大小写碰撞必须失败关闭");

    assert!(
        matches!(
            error,
            ReplacementAdapterError::AnalysisRejected {
                code: "weapon_duplicate_asset_path" | "weapon_case_insensitive_path_collision"
            }
        ),
        "实际是 {error:?}"
    );
}
