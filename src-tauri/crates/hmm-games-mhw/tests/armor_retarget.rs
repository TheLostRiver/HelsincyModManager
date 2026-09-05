//! #342：防具侧重定向。
//!
//! 夹具是真实包「玫瑰礼服（替换盛装）」`external-import-package-afc682a0` 的**完整路径
//! 清单**（纯字符串，逐条来自沙箱实测）。真实包本身不入库。
//!
//! 这批测试此前用的是 `f_body.mod3`、`f_121_0000_extra.mod3`、`m_body.mod3` 一类**编造的
//! 文件名**——真实防具文件名是 `f_<部位><槽位令牌>.<ext>`，编造名不含槽位令牌，于是
//! 「文件名必须跟着改名」这条规则在测试里根本不可能被发现。代码、文档、测试三方自洽地错
//! （#337 的病根）就是这么来的。夹具必须来自真实数据的形态。

use hmm_core::{
    GameId, ModId, PackageFileId, ProfileId, ReplacementBinding, ReplacementBindingId,
    ReplacementSourceId, ReplacementTargetId, ReplacementWarning, RetargetPlan,
};
use hmm_games_mhw::{ArmorPathError, ArmorResourcePath, MhwArmorReplacementAdapter};
use hmm_ports::{
    ReplacementAdapter, ReplacementAdapterError, ReplacementAnalysisRequest, ReplacementAsset,
    RetargetPlanRequest,
};

/// 玫瑰礼服，源槽位 `pl078_0000`。28 个文件：槽位内 18（五个部位）+ 作者自建目录 10。
///
/// 槽位内的 18 个正是 #336 真机实验 A 的「新增 18 / 删除 18 / 内容变化 0」。
const ROSE_DRESS: &[&str] = &[
    "nativePC/pl/f_equip/mod_pl_rosedress/hand000_BM.tex",
    "nativePC/pl/f_equip/mod_pl_rosedress/hand000_CMM.tex",
    "nativePC/pl/f_equip/mod_pl_rosedress/npc002_110_BM.tex",
    "nativePC/pl/f_equip/mod_pl_rosedress/npc002_110_CMM.tex",
    "nativePC/pl/f_equip/mod_pl_rosedress/npc002_111_BM.tex",
    "nativePC/pl/f_equip/mod_pl_rosedress/npc002_111_CMM.tex",
    "nativePC/pl/f_equip/mod_pl_rosedress/npc046_001_BM.tex",
    "nativePC/pl/f_equip/mod_pl_rosedress/npc046_001_CMM.tex",
    "nativePC/pl/f_equip/mod_pl_rosedress/npc046_002_BML.tex",
    "nativePC/pl/f_equip/mod_pl_rosedress/npc046_002_CMM.tex",
    "nativePC/pl/f_equip/pl078_0000/arm/mod/f_arm078_0000.ctc",
    "nativePC/pl/f_equip/pl078_0000/arm/mod/f_arm078_0000.mod3",
    "nativePC/pl/f_equip/pl078_0000/arm/mod/f_arm078_0000.mrl3",
    "nativePC/pl/f_equip/pl078_0000/body/mod/f_body078_0000.ccl",
    "nativePC/pl/f_equip/pl078_0000/body/mod/f_body078_0000.ctc",
    "nativePC/pl/f_equip/pl078_0000/body/mod/f_body078_0000.mod3",
    "nativePC/pl/f_equip/pl078_0000/body/mod/f_body078_0000.mrl3",
    "nativePC/pl/f_equip/pl078_0000/helm/mod/f_helm078_0000.ctc",
    "nativePC/pl/f_equip/pl078_0000/helm/mod/f_helm078_0000.evhl",
    "nativePC/pl/f_equip/pl078_0000/helm/mod/f_helm078_0000.mod3",
    "nativePC/pl/f_equip/pl078_0000/helm/mod/f_helm078_0000.mrl3",
    "nativePC/pl/f_equip/pl078_0000/leg/mod/f_leg078_0000.ccl",
    "nativePC/pl/f_equip/pl078_0000/leg/mod/f_leg078_0000.ctc",
    "nativePC/pl/f_equip/pl078_0000/leg/mod/f_leg078_0000.mod3",
    "nativePC/pl/f_equip/pl078_0000/leg/mod/f_leg078_0000.mrl3",
    "nativePC/pl/f_equip/pl078_0000/wst/mod/f_wst078_0000.ctc",
    "nativePC/pl/f_equip/pl078_0000/wst/mod/f_wst078_0000.mod3",
    "nativePC/pl/f_equip/pl078_0000/wst/mod/f_wst078_0000.mrl3",
];

const ROSE_DRESS_SOURCE_ID: &str = "mhw:armor:f_equip:pl078_0000";
/// catalog 里 `mhw:armor:fatalis-alpha` 对应的槽位。
const FATALIS_ALPHA_SLOT: &str = "pl129_0000";

fn asset(id: &str, relative_path: &str) -> ReplacementAsset {
    ReplacementAsset::new(PackageFileId::new(id), relative_path)
}

/// 包内路径同时用作 `package_file_id`——真实链路里两者一一对应。
fn assets(paths: &[&str]) -> Vec<ReplacementAsset> {
    paths.iter().map(|path| asset(path, path)).collect()
}

fn binding(source_id: &str, target_id: &str) -> ReplacementBinding {
    ReplacementBinding::new(
        ReplacementBindingId::parse("binding-1").expect("binding id"),
        ModId::new("mod-1"),
        ProfileId::new("profile-1"),
        ReplacementSourceId::parse(source_id).expect("source id"),
        ReplacementTargetId::parse(target_id).expect("target id"),
        42,
    )
    .expect("binding")
}

fn plan_for(paths: &[&str]) -> Result<RetargetPlan, ReplacementAdapterError> {
    MhwArmorReplacementAdapter.build_retarget_plan(RetargetPlanRequest {
        game_id: GameId::mhw(),
        binding: binding(ROSE_DRESS_SOURCE_ID, "mhw:armor:fatalis-alpha"),
        assets: assets(paths),
    })
}

/// 某个包内文件在计划里的目标路径。找不到就 panic——「随行文件被丢掉」正是 #342 要防的
/// 失败模式，不能退化成 `None` 被断言悄悄放过。
fn target_of<'a>(plan: &'a RetargetPlan, source_path: &str) -> &'a str {
    plan.actions()
        .iter()
        .find(|action| action.source_relative_path().as_str() == source_path)
        .unwrap_or_else(|| panic!("计划必须为 {source_path} 产出动作"))
        .target_relative_path()
        .as_str()
}

#[test]
fn a_real_armor_package_carries_every_part_and_companion_into_the_plan() {
    /*
     * #342 的核心回归：旧实现要求槽位之后必须是 `arm/mod`，于是 `body` `helm` `leg` `wst`
     * 四个部位撞上「路径畸形」并**否决整包**，用户看到的是
     * 「该 Mod 不是当前可自动处理的单源外观包」。库里的防具包 100% 不可重定向。
     */
    let plan = plan_for(ROSE_DRESS).expect("真实玫瑰礼服包必须能产出计划");

    // 一个包内文件都不能丢：28 个文件 = 28 条动作。
    assert_eq!(plan.actions().len(), ROSE_DRESS.len());
    for path in ROSE_DRESS {
        target_of(&plan, path);
    }
}

#[test]
fn filenames_are_rewritten_not_just_the_slot_directory() {
    /*
     * #342 的 C。真机实验 A 观测到 `f_arm078_0000.mod3` 落成 `f_arm123_0000.mod3`
     * ——哈希证明字节完全相同，**只有名字变了**。
     *
     * 旧实现只换目录段、不碰文件名。所以就算放开部位段，装出来也是「目录对、文件名错」
     * 的坏结果——比打不开更糟，因为它看起来成功了。
     */
    for (part, extension) in [
        ("arm", "mod3"),
        ("body", "mrl3"),
        ("helm", "evhl"),
        ("leg", "ccl"),
        ("wst", "ctc"),
    ] {
        let plan = plan_for(ROSE_DRESS).expect("计划");
        assert_eq!(
            target_of(
                &plan,
                &format!("nativePC/pl/f_equip/pl078_0000/{part}/mod/f_{part}078_0000.{extension}")
            ),
            format!(
                "nativePC/pl/f_equip/{FATALIS_ALPHA_SLOT}/{part}/mod/f_{part}129_0000.{extension}"
            ),
            "{part} 的目录段与文件名段必须同时改写"
        );
    }
}

#[test]
fn the_authors_own_directory_stays_exactly_where_it_is() {
    /*
     * 真机实验 A 观测：作者自建目录 `mod_pl_rosedress/` 的 10 个文件**一字节没动**，也没换路径。
     * 它们被 MRL3 按原路径引用（`pl\f_equip\mod_pl_rosedress\*`），搬走反而断链。
     */
    let plan = plan_for(ROSE_DRESS).expect("计划");
    for path in ROSE_DRESS
        .iter()
        .filter(|path| path.contains("mod_pl_rosedress"))
    {
        assert_eq!(
            target_of(&plan, path),
            *path,
            "与槽位无关的作者目录必须留在原路径"
        );
    }
}

#[test]
fn textures_inside_the_source_slot_stay_at_their_original_path() {
    /*
     * 防具侧**不做二进制改写**，所以被 MRL3 按路径引用的文件绝不能搬——搬走就是静默断链：
     * 计划成功、装完游戏里贴图直接没了。
     *
     * 玫瑰礼服抓不到这个 bug：它的 10 个 `.tex` 全在槽位**外**的 `mod_pl_rosedress/`。
     * 这条夹具刻意把贴图放进槽位目录内，那是另一种同样常见的作者布局。
     *
     * 判据是扩展名，不是「有没有被 MRL3 引用」。#336 正文由实验 B 推断出的后者不可靠：
     * 那次观测里两者恰好重合，但没有因果关系；一个当前没被引用的 `.tex` 同样不该搬。
     */
    let mut paths = ROSE_DRESS.to_vec();
    paths.extend([
        "nativePC/pl/f_equip/pl078_0000/body/mod/f_body078_0000_BM.tex",
        "nativePC/pl/f_equip/pl078_0000/body/mod/f_body078_0000_NM.TEX",
        // 作者的中间产物不是 .tex，游戏不加载它，照常跟着走。
        "nativePC/pl/f_equip/pl078_0000/body/mod/f_body078_0000_BM.dds",
    ]);

    let plan = plan_for(&paths).expect("计划");

    for texture in [
        "nativePC/pl/f_equip/pl078_0000/body/mod/f_body078_0000_BM.tex",
        "nativePC/pl/f_equip/pl078_0000/body/mod/f_body078_0000_NM.TEX",
    ] {
        assert_eq!(
            target_of(&plan, texture),
            texture,
            "槽位内的贴图必须留在原路径，扩展名大小写不参与判定"
        );
    }
    assert_eq!(
        target_of(
            &plan,
            "nativePC/pl/f_equip/pl078_0000/body/mod/f_body078_0000_BM.dds"
        ),
        format!("nativePC/pl/f_equip/{FATALIS_ALPHA_SLOT}/body/mod/f_body129_0000_BM.dds"),
        ".dds 是作者中间产物，不被引用，照常重定位"
    );
    // 模型本身仍然搬走——只有 .tex 例外。
    assert_eq!(
        target_of(
            &plan,
            "nativePC/pl/f_equip/pl078_0000/body/mod/f_body078_0000.mod3"
        ),
        format!("nativePC/pl/f_equip/{FATALIS_ALPHA_SLOT}/body/mod/f_body129_0000.mod3")
    );
}

#[test]
fn unknown_part_directories_and_extensions_do_not_fail_the_package() {
    /*
     * #342 最重要的一条：规则是**结构性的，不是词表**。
     *
     * 补一张 `arm/body/helm/leg/wst` 的白名单同样是死路——Mod 作者的目录结构和扩展名
     * 不可能被穷举，每遇到一个没记录过的形态就拒绝，这个功能等于没有。正确做法是对源槽位
     * 目录下的一切一视同仁，只按编号段改写。
     *
     * 下面每一条都是词表里不存在的形态，必须全部照常进计划。
     */
    let mut paths = ROSE_DRESS.to_vec();
    paths.extend([
        // 作者自造的部位目录
        "nativePC/pl/f_equip/pl078_0000/cloak/mod/f_cloak078_0000.mod3",
        // 没见过的扩展名
        "nativePC/pl/f_equip/pl078_0000/body/mod/f_body078_0000.whatever",
        // 更深的嵌套子目录
        "nativePC/pl/f_equip/pl078_0000/body/mod/tex/custom/skin_BM.tex",
        // 直接挂在槽位根下，连部位段都没有
        "nativePC/pl/f_equip/pl078_0000/f_078_0000.ctc",
        // 文件名完全不含编号
        "nativePC/pl/f_equip/pl078_0000/body/mod/authors_note.txt",
    ]);

    let plan = plan_for(&paths).expect("没见过的形态不得让整包失败");

    assert_eq!(plan.actions().len(), paths.len());
    assert_eq!(
        target_of(
            &plan,
            "nativePC/pl/f_equip/pl078_0000/cloak/mod/f_cloak078_0000.mod3"
        ),
        format!("nativePC/pl/f_equip/{FATALIS_ALPHA_SLOT}/cloak/mod/f_cloak129_0000.mod3")
    );
    assert_eq!(
        target_of(
            &plan,
            "nativePC/pl/f_equip/pl078_0000/body/mod/tex/custom/skin_BM.tex"
        ),
        "nativePC/pl/f_equip/pl078_0000/body/mod/tex/custom/skin_BM.tex",
        "槽位内的 .tex 留在原路径，见 textures_inside_the_source_slot_stay_at_their_original_path"
    );
    assert_eq!(
        target_of(
            &plan,
            "nativePC/pl/f_equip/pl078_0000/body/mod/authors_note.txt"
        ),
        format!("nativePC/pl/f_equip/{FATALIS_ALPHA_SLOT}/body/mod/authors_note.txt"),
        "非贴图且文件名不含编号时，只换目录段"
    );
    assert_eq!(
        target_of(&plan, "nativePC/pl/f_equip/pl078_0000/f_078_0000.ctc"),
        format!("nativePC/pl/f_equip/{FATALIS_ALPHA_SLOT}/f_129_0000.ctc")
    );
}

#[test]
fn an_author_directory_that_is_not_a_slot_no_longer_fails_the_package() {
    /*
     * `pl/<equip>/` 下不符合槽位语法的目录（作者自建目录就是这种）过去报
     * `UnrecognizedSourceSlot` 否决整包。现在归到「随行·原样」，原路径安装。
     */
    let mut paths = ROSE_DRESS.to_vec();
    paths.push("nativePC/pl/f_equip/not-a-slot/whatever.tex");

    let plan = plan_for(&paths).expect("非槽位目录不得否决整包");

    assert_eq!(
        target_of(&plan, "nativePC/pl/f_equip/not-a-slot/whatever.tex"),
        "nativePC/pl/f_equip/not-a-slot/whatever.tex"
    );
}

#[test]
fn executables_never_enter_the_armor_plan() {
    /*
     * #336 切片③ 的拒绝清单同样作用于防具侧——⑥ 让随行文件开始搬运，这条暴露面
     * 是随之新增的，不能只在武器侧关上。
     */
    let mut paths = ROSE_DRESS.to_vec();
    paths.extend([
        "nativePC/pl/f_equip/pl078_0000/body/mod/TexConverter.exe",
        "nativePC/pl/f_equip/mod_pl_rosedress/installer.bat",
    ]);

    let plan = plan_for(&paths).expect("计划");

    assert_eq!(
        plan.actions().len(),
        ROSE_DRESS.len(),
        "两个可执行文件被排除"
    );
    for excluded in [
        "nativePC/pl/f_equip/pl078_0000/body/mod/TexConverter.exe",
        "nativePC/pl/f_equip/mod_pl_rosedress/installer.bat",
    ] {
        assert!(
            !plan
                .actions()
                .iter()
                .any(|action| action.source_relative_path().as_str() == excluded),
            "{excluded} 不得产出动作"
        );
    }
    assert_eq!(
        plan.adapter_facts()
            .expect("排除文件后必须留下 facts")
            .excluded_file_count(),
        2,
        "丢弃必须留痕"
    );
}

#[test]
fn armor_paths_normalize_separators_and_carry_the_slot_identity() {
    let forward =
        ArmorResourcePath::parse("nativePC/pl/f_equip/pl078_0000/arm/mod/f_arm078_0000.mod3")
            .expect("forward path");
    let backward =
        ArmorResourcePath::parse(r"nativePC\pl\f_equip\pl078_0000\arm\mod\f_arm078_0000.mod3")
            .expect("backslash path");

    assert_eq!(forward, backward);
    assert_eq!(forward.slot(), "pl078_0000");
    assert_eq!(forward.path_family(), "pl/f_equip");
    assert!(forward.is_supported());
    assert_eq!(
        forward.retarget("pl129_0000").expect("target").as_str(),
        "nativePC/pl/f_equip/pl129_0000/arm/mod/f_arm129_0000.mod3"
    );
}

#[test]
fn male_equipment_is_recognized_but_has_no_catalog_target() {
    // `m_equip` 的路径语法照常识别；不可选是 **catalog 覆盖范围**的限制，不是路径判定。
    let male =
        ArmorResourcePath::parse("nativePC/pl/m_equip/pl078_0000/body/mod/m_body078_0000.mod3")
            .expect("male path is recognized for analysis");
    assert_eq!(male.path_family(), "pl/m_equip");
    assert!(!male.is_supported());
}

#[test]
fn only_genuine_path_safety_signals_still_fail() {
    // 父目录遍历仍然失败关闭；「形态不认识」不再是失败理由。
    assert_eq!(
        ArmorResourcePath::parse("../nativePC/pl/f_equip/pl078_0000/arm/mod/f_arm078_0000.mod3"),
        Err(ArmorPathError::UnsafePath)
    );

    let unsafe_error = MhwArmorReplacementAdapter
        .analyze_replacement_assets(ReplacementAnalysisRequest {
            game_id: GameId::mhw(),
            assets: vec![asset("escape.mod3", "../escape.mod3")],
        })
        .expect_err("unsafe asset path");
    assert_eq!(unsafe_error, ReplacementAdapterError::UnsafeRetargetPath);
}

#[test]
fn armor_analysis_accepts_one_source_and_ignores_unrelated_safe_assets() {
    let mut paths = ROSE_DRESS.to_vec();
    paths.extend(["readme.txt", "预览图.png"]);

    let analysis = MhwArmorReplacementAdapter
        .analyze_replacement_assets(ReplacementAnalysisRequest {
            game_id: GameId::mhw(),
            assets: assets(&paths),
        })
        .expect("analysis");

    assert!(analysis.is_retargetable());
    assert_eq!(
        analysis.matched_asset_count(),
        ROSE_DRESS.len(),
        "`nativePC/pl/<equip>/` 之外的文件不计入"
    );
    assert_eq!(analysis.sources().len(), 1);
    assert_eq!(analysis.sources()[0].id().as_str(), ROSE_DRESS_SOURCE_ID);
    assert!(analysis.warnings().is_empty());
}

#[test]
fn armor_analysis_reports_no_source_without_failing_normal_mod_imports() {
    let analysis = MhwArmorReplacementAdapter
        .analyze_replacement_assets(ReplacementAnalysisRequest {
            game_id: GameId::mhw(),
            assets: vec![asset("readme.txt", "readme.txt")],
        })
        .expect("non-armor analysis");

    assert!(!analysis.is_retargetable());
    assert!(analysis.sources().is_empty());
    assert_eq!(
        analysis.warnings(),
        &[ReplacementWarning::NoSupportedAssets]
    );
}

#[test]
fn armor_analysis_blocks_multiple_slots_and_male_or_mixed_sources() {
    let adapter = MhwArmorReplacementAdapter;
    let multiple = adapter
        .analyze_replacement_assets(ReplacementAnalysisRequest {
            game_id: GameId::mhw(),
            assets: assets(&[
                "nativePC/pl/f_equip/pl078_0000/body/mod/f_body078_0000.mod3",
                "nativePC/pl/f_equip/pl079_0000/body/mod/f_body079_0000.mod3",
            ]),
        })
        .expect("multiple analysis");
    assert!(!multiple.is_retargetable());
    assert!(multiple
        .warnings()
        .contains(&ReplacementWarning::MultipleSources));

    let mixed = adapter
        .analyze_replacement_assets(ReplacementAnalysisRequest {
            game_id: GameId::mhw(),
            assets: assets(&[
                "nativePC/pl/f_equip/pl078_0000/body/mod/f_body078_0000.mod3",
                "nativePC/pl/m_equip/pl078_0000/body/mod/m_body078_0000.mod3",
            ]),
        })
        .expect("mixed analysis");
    assert!(!mixed.is_retargetable());
    assert!(mixed
        .warnings()
        .contains(&ReplacementWarning::MultipleSources));
    assert!(mixed
        .warnings()
        .contains(&ReplacementWarning::UnsupportedSource));
}

#[test]
fn armor_retarget_plan_rejects_ambiguous_source_unknown_target_and_binding_mismatch() {
    let adapter = MhwArmorReplacementAdapter;
    let ambiguous = adapter
        .build_retarget_plan(RetargetPlanRequest {
            game_id: GameId::mhw(),
            binding: binding(ROSE_DRESS_SOURCE_ID, "mhw:armor:fatalis-alpha"),
            assets: assets(&[
                "nativePC/pl/f_equip/pl078_0000/body/mod/f_body078_0000.mod3",
                "nativePC/pl/m_equip/pl078_0000/body/mod/m_body078_0000.mod3",
            ]),
        })
        .expect_err("ambiguous source");
    assert_eq!(ambiguous, ReplacementAdapterError::AmbiguousSourceSlot);

    let missing_id = ReplacementTargetId::parse("mhw:armor:missing").expect("target id");
    let missing = adapter
        .build_retarget_plan(RetargetPlanRequest {
            game_id: GameId::mhw(),
            binding: binding(ROSE_DRESS_SOURCE_ID, missing_id.as_str()),
            assets: assets(ROSE_DRESS),
        })
        .expect_err("unknown target");
    assert_eq!(
        missing,
        ReplacementAdapterError::TargetCatalogMissing {
            target_id: missing_id
        }
    );

    let mismatch = adapter
        .build_retarget_plan(RetargetPlanRequest {
            game_id: GameId::mhw(),
            binding: binding("mhw:armor:f_equip:pl999_0000", "mhw:armor:fatalis-alpha"),
            assets: assets(ROSE_DRESS),
        })
        .expect_err("binding mismatch");
    assert_eq!(mismatch, ReplacementAdapterError::SourceBindingMismatch);
}

#[test]
fn armor_retarget_plan_warns_when_source_already_matches_target() {
    let plan = MhwArmorReplacementAdapter
        .build_retarget_plan(RetargetPlanRequest {
            game_id: GameId::mhw(),
            binding: binding("mhw:armor:f_equip:pl129_0000", "mhw:armor:fatalis-alpha"),
            assets: assets(&["nativePC/pl/f_equip/pl129_0000/body/mod/f_body129_0000.mod3"]),
        })
        .expect("same-target plan");

    assert_eq!(plan.warnings(), &[ReplacementWarning::SourceMatchesTarget]);
}

#[test]
fn armor_retarget_plan_rejects_duplicate_normalized_target_paths() {
    let error = MhwArmorReplacementAdapter
        .build_retarget_plan(RetargetPlanRequest {
            game_id: GameId::mhw(),
            binding: binding(ROSE_DRESS_SOURCE_ID, "mhw:armor:fatalis-alpha"),
            assets: vec![
                asset(
                    "first.mod3",
                    "nativePC/pl/f_equip/pl078_0000/body/mod/f_body078_0000.mod3",
                ),
                asset(
                    "second.mod3",
                    r"nativePC\pl\f_equip\pl078_0000\body\mod\f_body078_0000.mod3",
                ),
            ],
        })
        .expect_err("duplicate final target");

    assert_eq!(error, ReplacementAdapterError::InvalidRetargetPlan);
}
