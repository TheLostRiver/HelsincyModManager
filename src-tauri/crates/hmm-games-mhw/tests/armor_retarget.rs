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
use hmm_games_mhw::{
    ArmorPathError, ArmorResourcePath, MhwArmorCatalog, MhwArmorReplacementAdapter,
};
use hmm_ports::{
    ReplacementAdapter, ReplacementAdapterError, ReplacementAnalysisRequest, ReplacementAsset,
    ReplacementCatalogProvider, RetargetPlanRequest,
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

/// 按 `(internal_id, path_family)` 取目标 ID。
///
/// `#356` 起 `internal_id` **不再唯一**——同一件装备的两套模型各占一条，而 aggregate
/// catalog 按 stable_id 排序，只按 `internal_id` 查会拿到不确定的那一条。
fn target_id_in(internal_id: &str, path_family: &str) -> String {
    MhwArmorCatalog
        .replacement_catalog()
        .expect("armor catalog")
        .targets()
        .iter()
        .find(|target| {
            target.internal_id() == internal_id
                && target
                    .metadata()
                    .get("path_family")
                    .and_then(|value| value.as_str())
                    == Some(path_family)
        })
        .unwrap_or_else(|| panic!("catalog 里没有 {internal_id} 的 {path_family} 变体"))
        .id()
        .as_str()
        .to_owned()
}

fn male_target_id(internal_id: &str) -> String {
    target_id_in(internal_id, "pl/m_equip")
}

fn female_target_id(internal_id: &str) -> String {
    target_id_in(internal_id, "pl/f_equip")
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
        carries_package_companions: true,
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

/*
 * `#356`：男性模型变体现在也有 catalog 目标。
 *
 * 此前 `is_supported()` 硬编码只认 `f_equip`，那不是路径判定，是 catalog 只转录了一半。
 * 后果比「男装包不能重定向」严重得多：`single_source()` 要求 `is_supported()`，所以
 * **男角玩家改任何防具外观都拿不到目标列表**——`f_equip`/`m_equip` 不是「女装／男装」，
 * 是同一件装备的两套模型，玩家的角色性别决定游戏加载哪一套。
 */
#[test]
fn both_model_variants_are_recognized_and_have_catalog_targets() {
    let female =
        ArmorResourcePath::parse("nativePC/pl/f_equip/pl078_0000/body/mod/f_body078_0000.mod3")
            .expect("female path is recognized");
    let male =
        ArmorResourcePath::parse("nativePC/pl/m_equip/pl078_0000/body/mod/m_body078_0000.mod3")
            .expect("male path is recognized");

    assert_eq!(female.path_family(), "pl/f_equip");
    assert_eq!(male.path_family(), "pl/m_equip");
    assert!(female.is_supported());
    assert!(male.is_supported(), "男性模型变体必须能拿到目标列表");
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
    /*
     * `#356`：不再有 `UnsupportedSource`。
     *
     * 这个包同时带 `f_equip` 与 `m_equip` 的同一槽位，两个变体现在都有 catalog 目标，
     * 所以「不受支持」不再成立。挡住它的仍然是 `MultipleSources`——包里有两个源，
     * 得逐槽位绑定（`#349` ②），而不是因为某个变体没目标。
     */
    assert!(
        !mixed
            .warnings()
            .contains(&ReplacementWarning::UnsupportedSource),
        "两套模型都有目标了，不该再报「源不受支持」"
    );
}

/*
 * `#349` 切片②：包里同时有女装槽位和男装槽位时，**女装那件照常可重定向**。
 *
 * 这条曾经断言 `AmbiguousSourceSlot`（拒整包）。那是 `#349` 的病根——判定粒度错了：
 * 「作者一次发布多件装备」是正常的发布习惯，不是坏包。绑定点名了哪个槽位，就按那个槽位建
 * 计划；包里还有别的槽位与它无关。
 */
#[test]
fn a_female_slot_is_still_retargetable_when_the_package_also_carries_a_male_slot() {
    let plan = MhwArmorReplacementAdapter
        .build_retarget_plan(RetargetPlanRequest {
            game_id: GameId::mhw(),
            binding: binding(ROSE_DRESS_SOURCE_ID, "mhw:armor:fatalis-alpha"),
            assets: assets(&[
                "nativePC/pl/f_equip/pl078_0000/body/mod/f_body078_0000.mod3",
                "nativePC/pl/m_equip/pl078_0000/body/mod/m_body078_0000.mod3",
            ]),
            carries_package_companions: true,
        })
        .expect("女装槽位不该被同包里的男装槽位拖累");

    // 只带自己那个槽位的文件：男装槽位既不属于本绑定，也没有可选目标。
    assert_eq!(
        plan.actions()
            .iter()
            .map(|action| action.source_relative_path().as_str())
            .collect::<Vec<_>>(),
        vec!["nativePC/pl/f_equip/pl078_0000/body/mod/f_body078_0000.mod3"]
    );
    assert_eq!(
        target_of(
            &plan,
            "nativePC/pl/f_equip/pl078_0000/body/mod/f_body078_0000.mod3"
        ),
        "nativePC/pl/f_equip/pl129_0000/body/mod/f_body129_0000.mod3"
    );
}

/*
 * `#356`：男装包必须报「没有可选目标」，不是「源槽位有歧义」。
 *
 * 包里只有一个槽位，报歧义与事实相反，而且把玩家引向「换个包」——真正的成因是 catalog 只
 * 覆盖女装（实测 269 条目标全是 `pl/f_equip`），包本身没有任何问题。
 */
#[test]
fn a_male_model_package_retargets_within_its_own_variant() {
    /*
     * `#356` 的核心验收：男性模型的包能重定向，且**留在自己的变体里**。
     *
     * 此前这条测试断言的是「男装报没有可选目标」——那是 catalog 只转录了一半时的行为。
     * 现在两套模型都有目标，男角玩家改防具外观不再装到游戏不读的路径去。
     *
     * 断言逐字的目标路径而不是 `is_ok()`：只断言成功会漏掉「重定向到了 f_equip」，
     * 而那正是修复前的失败形态——安装成功、游戏不生效、没有任何诊断线索。
     */
    let target = male_target_id("pl129_0000");
    let plan = MhwArmorReplacementAdapter
        .build_retarget_plan(RetargetPlanRequest {
            game_id: GameId::mhw(),
            binding: binding("mhw:armor:m_equip:pl078_0000", target.as_str()),
            assets: assets(&["nativePC/pl/m_equip/pl078_0000/body/mod/m_body078_0000.mod3"]),
            carries_package_companions: true,
        })
        .expect("男性模型包必须能重定向");

    let targets: Vec<_> = plan
        .actions()
        .iter()
        .map(|action| action.target_relative_path().as_str())
        .collect();
    assert_eq!(
        targets,
        vec!["nativePC/pl/m_equip/pl129_0000/body/mod/m_body129_0000.mod3"],
        "必须落在 m_equip，不能跨模型变体"
    );
}

/// 跨模型变体的绑定必须被拒绝：两套模型的骨骼不同，换过去本来就不成立。
#[test]
fn a_binding_across_model_variants_is_rejected() {
    let error = MhwArmorReplacementAdapter
        .build_retarget_plan(RetargetPlanRequest {
            game_id: GameId::mhw(),
            binding: binding(
                "mhw:armor:m_equip:pl078_0000",
                female_target_id("pl129_0000").as_str(),
            ),
            assets: assets(&["nativePC/pl/m_equip/pl078_0000/body/mod/m_body078_0000.mod3"]),
            carries_package_companions: true,
        })
        .expect_err("男性模型的源不得绑到女性模型的目标");

    assert_eq!(error, ReplacementAdapterError::UnsupportedReplacementTarget);
}

#[test]
fn armor_retarget_plan_rejects_unknown_target_and_binding_mismatch() {
    let adapter = MhwArmorReplacementAdapter;
    let missing_id = ReplacementTargetId::parse("mhw:armor:missing").expect("target id");
    let missing = adapter
        .build_retarget_plan(RetargetPlanRequest {
            game_id: GameId::mhw(),
            binding: binding(ROSE_DRESS_SOURCE_ID, missing_id.as_str()),
            assets: assets(ROSE_DRESS),
            carries_package_companions: true,
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
            carries_package_companions: true,
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
            carries_package_companions: true,
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
            carries_package_companions: true,
        })
        .expect_err("duplicate final target");

    assert_eq!(error, ReplacementAdapterError::InvalidRetargetPlan);
}

/*
 * 多槽位夹具（`#355`）。
 *
 * 语料库里 11 个真实外观包**全是单槽位**，一个多槽位样本都没有（`#349` 正文第三节已如实
 * 承认这个限制），所以这份清单是构造的。构造必须复刻真实命名约定——两个源槽位号
 * （`pl078_0000` / `pl123_0000`）与作者自建目录的形态都取自真实包，只是把它们组合进同一个
 * 包。用编造的文件名会让代码、文档、测试三方自洽地错（`#337` 的病根）。
 */
const TWO_ARMOR_SETS: &[&str] = &[
    // 作者自建目录：属于**包**，与哪个槽位装到哪都无关，一个包只装一次。
    "nativePC/pl/f_equip/mod_pl_twosets/shared_BM.tex",
    "nativePC/pl/f_equip/mod_pl_twosets/shared_CMM.tex",
    // 槽位 A。`_BM.tex` 是**槽位级**原路径文件：只有装 A 的绑定才该带它。
    "nativePC/pl/f_equip/pl078_0000/body/mod/f_body078_0000.mod3",
    "nativePC/pl/f_equip/pl078_0000/body/mod/f_body078_0000.mrl3",
    "nativePC/pl/f_equip/pl078_0000/body/mod/f_body078_0000_BM.tex",
    // 槽位 B。
    "nativePC/pl/f_equip/pl123_0000/body/mod/f_body123_0000.mod3",
    "nativePC/pl/f_equip/pl123_0000/body/mod/f_body123_0000.mrl3",
    "nativePC/pl/f_equip/pl123_0000/body/mod/f_body123_0000_BM.tex",
];

const SLOT_B_SOURCE_ID: &str = "mhw:armor:f_equip:pl123_0000";
/// catalog 里 `pl001_0000` 的 `pl/f_equip` 目标 id（`data/armor/` 分片实测）。
const LEATHER_TARGET_ID: &str =
    "mhw:armor:67663de427bb57b42d289ea193d8e865bb949ffaeee8a9e9caecdc1ee54662eb";

fn two_set_plan(source_id: &str, target_id: &str, carries: bool) -> RetargetPlan {
    MhwArmorReplacementAdapter
        .build_retarget_plan(RetargetPlanRequest {
            game_id: GameId::mhw(),
            binding: binding(source_id, target_id),
            assets: assets(TWO_ARMOR_SETS),
            carries_package_companions: carries,
        })
        .expect("多槽位包的每个槽位都该能各自建计划")
}

fn source_paths(plan: &RetargetPlan) -> Vec<&str> {
    let mut paths = plan
        .actions()
        .iter()
        .map(|action| action.source_relative_path().as_str())
        .collect::<Vec<_>>();
    paths.sort_unstable();
    paths
}

/*
 * `#349` 切片② 的核心：一个包里两套防具，各自绑定、各自建计划。
 *
 * 断言到**逐字的路径集合**而不是「没报错」：只断言 `is_ok()` 会漏掉「建出来了但带错了文件」
 * ——而那正是下面两条 latent bug 的表现形态。
 */
#[test]
fn each_armor_slot_in_a_multi_slot_package_builds_its_own_plan() {
    let plan_a = two_set_plan(ROSE_DRESS_SOURCE_ID, "mhw:armor:fatalis-alpha", true);

    assert_eq!(
        target_of(
            &plan_a,
            "nativePC/pl/f_equip/pl078_0000/body/mod/f_body078_0000.mod3"
        ),
        "nativePC/pl/f_equip/pl129_0000/body/mod/f_body129_0000.mod3"
    );

    let plan_b = two_set_plan(SLOT_B_SOURCE_ID, LEATHER_TARGET_ID, false);

    assert_eq!(
        target_of(
            &plan_b,
            "nativePC/pl/f_equip/pl123_0000/body/mod/f_body123_0000.mod3"
        ),
        "nativePC/pl/f_equip/pl001_0000/body/mod/f_body001_0000.mod3"
    );
}

/*
 * 槽位内的 `.tex` 属于**那个槽位**，不属于包。
 *
 * 旧实现把它和作者自建目录混在同一个 `kept_in_place` 里、且不按槽位过滤，于是槽位 A 的计划
 * 会把槽位 B 的贴图按原路径一起装进去——玩家只想换第一套，第二套的贴图也落了盘。
 */
#[test]
fn a_slots_plan_does_not_carry_another_slots_in_place_textures() {
    let plan_a = two_set_plan(ROSE_DRESS_SOURCE_ID, "mhw:armor:fatalis-alpha", true);

    assert_eq!(
        source_paths(&plan_a),
        vec![
            "nativePC/pl/f_equip/mod_pl_twosets/shared_BM.tex",
            "nativePC/pl/f_equip/mod_pl_twosets/shared_CMM.tex",
            "nativePC/pl/f_equip/pl078_0000/body/mod/f_body078_0000.mod3",
            "nativePC/pl/f_equip/pl078_0000/body/mod/f_body078_0000.mrl3",
            "nativePC/pl/f_equip/pl078_0000/body/mod/f_body078_0000_BM.tex",
        ],
        "槽位 A 的计划不得出现任何 pl123_0000 的文件"
    );

    // 槽位级 `.tex` 仍然留在**原路径**（#342：防具侧零二进制改写，搬走就是静默断链）。
    assert_eq!(
        target_of(
            &plan_a,
            "nativePC/pl/f_equip/pl078_0000/body/mod/f_body078_0000_BM.tex"
        ),
        "nativePC/pl/f_equip/pl078_0000/body/mod/f_body078_0000_BM.tex"
    );
}

/*
 * 包级随行文件由**恰好一个**绑定承载。
 *
 * 旧实现无条件把它们塞进每个计划。多绑定一次提交时，同一个 `target_path` 会出现多个
 * provider，在 `InstallPlan` 里撞成阻断冲突——`#349` ③b 给这条路径具名过
 * （`DuplicateSlotTarget`）。承载者是谁不影响正确性，承重的只有「恰好一个」。
 */
#[test]
fn package_level_companions_ride_only_with_the_designated_carrier() {
    let carrier = two_set_plan(ROSE_DRESS_SOURCE_ID, "mhw:armor:fatalis-alpha", true);
    let passenger = two_set_plan(SLOT_B_SOURCE_ID, LEATHER_TARGET_ID, false);

    assert!(
        source_paths(&carrier).contains(&"nativePC/pl/f_equip/mod_pl_twosets/shared_BM.tex"),
        "承载者必须带上包级随行文件"
    );
    assert_eq!(
        source_paths(&passenger),
        vec![
            "nativePC/pl/f_equip/pl123_0000/body/mod/f_body123_0000.mod3",
            "nativePC/pl/f_equip/pl123_0000/body/mod/f_body123_0000.mrl3",
            "nativePC/pl/f_equip/pl123_0000/body/mod/f_body123_0000_BM.tex",
        ],
        "非承载者一个包级随行文件都不该带，否则两个绑定撞同一个 target_path"
    );
}

/// `#363` 的真实包夹具：一个 `pl/m_equip` 外观包，**作者自建贴图目录放在 family 旁边**
/// （`nativePC/pl/<作者目录>/`）而不是 family 之内。37 个文件：槽位内 14（五个部位）
/// ＋ 包级随行 23（22 个 `.tex` ＋ 1 个 `.efx`）。
///
/// 这 23 个此前被静默丢弃：分类器只把 `nativePC/pl/<family>/<作者目录>/` 认作随行，
/// 放在 family 旁边就归「无关」。而那 22 个贴图**被槽位里的 MRL3 按路径引用**（按 MRL3
/// 贴图表逐条解开该包的 5 个 `.mrl3`，指向该目录的引用恰好 22 条，与文件一一对应，
/// 且引用里不含槽位编号——所以它们不需要改名，只需要原样安装）。丢掉的后果是
/// 模型装上了、贴图没有：安装成功、外观是坏的、无诊断线索。
const GUTS_MALE_SET: &[&str] = &[
    "nativePC/pl/Guts/BerserkEyes.efx",
    "nativePC/pl/Guts/arm/Gauntlet_BML.tex",
    "nativePC/pl/Guts/arm/Gauntlet_NM.tex",
    "nativePC/pl/Guts/arm/Gauntlet_RMT.tex",
    "nativePC/pl/Guts/body/Shoulder_BML.tex",
    "nativePC/pl/Guts/body/Shoulder_NM.tex",
    "nativePC/pl/Guts/body/Shoulder_RMT.tex",
    "nativePC/pl/Guts/body/Ub_BML.tex",
    "nativePC/pl/Guts/body/Ub_NM.tex",
    "nativePC/pl/Guts/body/Ub_RMT.tex",
    "nativePC/pl/Guts/helm/Helm_BML.tex",
    "nativePC/pl/Guts/helm/Helm_EM.tex",
    "nativePC/pl/Guts/helm/Helm_NM.tex",
    "nativePC/pl/Guts/helm/Helm_RMT.tex",
    "nativePC/pl/Guts/leg/Foot_BML.tex",
    "nativePC/pl/Guts/leg/Foot_NM.tex",
    "nativePC/pl/Guts/leg/Foot_RMT.tex",
    "nativePC/pl/Guts/leg/Skin_BML.tex",
    "nativePC/pl/Guts/leg/Skin_NM.tex",
    "nativePC/pl/Guts/leg/Skin_RMT.tex",
    "nativePC/pl/Guts/wst/Cloak_BML.tex",
    "nativePC/pl/Guts/wst/Cloak_NM.tex",
    "nativePC/pl/Guts/wst/Cloak_RMT.tex",
    "nativePC/pl/m_equip/pl071_0000/arm/mod/m_arm071_0000.mod3",
    "nativePC/pl/m_equip/pl071_0000/arm/mod/m_arm071_0000.mrl3",
    "nativePC/pl/m_equip/pl071_0000/body/epv/m_body071.epv3",
    "nativePC/pl/m_equip/pl071_0000/body/mod/m_body071_0000.mod3",
    "nativePC/pl/m_equip/pl071_0000/body/mod/m_body071_0000.mrl3",
    "nativePC/pl/m_equip/pl071_0000/helm/mod/m_helm071_0000.evhl",
    "nativePC/pl/m_equip/pl071_0000/helm/mod/m_helm071_0000.mod3",
    "nativePC/pl/m_equip/pl071_0000/helm/mod/m_helm071_0000.mrl3",
    "nativePC/pl/m_equip/pl071_0000/leg/mod/m_leg071_0000.mod3",
    "nativePC/pl/m_equip/pl071_0000/leg/mod/m_leg071_0000.mrl3",
    "nativePC/pl/m_equip/pl071_0000/wst/mod/m_wst071_0000.ctc",
    "nativePC/pl/m_equip/pl071_0000/wst/mod/m_wst071_0000.evbd",
    "nativePC/pl/m_equip/pl071_0000/wst/mod/m_wst071_0000.mod3",
    "nativePC/pl/m_equip/pl071_0000/wst/mod/m_wst071_0000.mrl3",
];

const GUTS_MALE_SOURCE_ID: &str = "mhw:armor:m_equip:pl071_0000";
/// 包级随行那 23 个的包内路径前缀。
const GUTS_COMPANION_PREFIX: &str = "nativePC/pl/Guts/";

fn male_plan_for(
    paths: &[&str],
    target_slot: &str,
    carries_package_companions: bool,
) -> Result<RetargetPlan, ReplacementAdapterError> {
    MhwArmorReplacementAdapter.build_retarget_plan(RetargetPlanRequest {
        game_id: GameId::mhw(),
        binding: binding(GUTS_MALE_SOURCE_ID, &male_target_id(target_slot)),
        assets: assets(paths),
        carries_package_companions,
    })
}

fn analysis_of(paths: &[&str]) -> hmm_core::ReplacementAnalysis {
    MhwArmorReplacementAdapter
        .analyze_replacement_assets(ReplacementAnalysisRequest {
            game_id: GameId::mhw(),
            assets: assets(paths),
        })
        .expect("分析必须成功")
}

/// `#363`：包内每一个可安装文件都必须进计划，认不出来的原样安装。
///
/// 判据是**与普通安装同一口径**：普通安装的过滤只有「在不在游戏根之下」
/// （`hmm-app` 的 `is_installable_target_path`），零内容知识。重定向没有理由更严——
/// 否则「用了重定向反而少装文件」。修复前这个包普通安装写 37 个、重定向只写 14 个。
#[test]
fn every_installable_file_reaches_the_plan_even_when_the_adapter_cannot_classify_it() {
    let analysis = analysis_of(GUTS_MALE_SET);
    assert_eq!(
        analysis.matched_asset_count(),
        GUTS_MALE_SET.len(),
        "包内 {} 个文件必须全部有归属，实际只认了 {}",
        GUTS_MALE_SET.len(),
        analysis.matched_asset_count()
    );

    let plan = male_plan_for(GUTS_MALE_SET, "pl069_0000", true).expect("计划必须生成");
    assert_eq!(
        plan.actions().len(),
        GUTS_MALE_SET.len(),
        "计划动作数必须覆盖包内全部文件"
    );

    // 槽位内的 14 个按编号段改名，`m_` 前缀与扩展名逐字保留。
    assert_eq!(
        target_of(
            &plan,
            "nativePC/pl/m_equip/pl071_0000/arm/mod/m_arm071_0000.mod3"
        ),
        "nativePC/pl/m_equip/pl069_0000/arm/mod/m_arm069_0000.mod3"
    );

    // 包级随行那 23 个**原样安装**：目标路径与来源逐字相同，一个都不能少。
    let verbatim = plan
        .actions()
        .iter()
        .filter(|action| {
            action
                .source_relative_path()
                .as_str()
                .starts_with(GUTS_COMPANION_PREFIX)
        })
        .collect::<Vec<_>>();
    assert_eq!(verbatim.len(), 23, "包级随行必须全部进计划");
    for action in verbatim {
        assert_eq!(
            action.source_relative_path().as_str(),
            action.target_relative_path().as_str(),
            "{} 是按路径被引用的随行文件，不得改路径",
            action.source_relative_path().as_str()
        );
    }

    // family 之外的随行同样受承载者开关约束（「恰好一个承载者」的双槽位用例另见
    // `package_level_companions_ride_only_with_the_designated_carrier`）。
    let passenger = male_plan_for(GUTS_MALE_SET, "pl069_0000", false).expect("非承载者计划");
    assert_eq!(
        passenger.actions().len(),
        14,
        "非承载者只带自己槽位的文件，不带包级随行"
    );
}

/// 作者把随行目录放在 family **之内**还是**旁边**，处置必须等价（`#363`）。
///
/// 两种都是真实作者习惯：库里的女装包放在 `pl/f_equip/<作者目录>/`，这个男装包放在
/// `pl/<作者目录>/`。此前前者会安装、后者被丢弃——那个区分本身就是缺陷，没有任何
/// 结构理由。断言的是**等价性**而不是「都不报错」。
#[test]
fn companion_directories_are_equivalent_inside_and_beside_the_family_dir() {
    let beside = ["nativePC/pl/Guts/arm/Gauntlet_BML.tex"];
    let inside = ["nativePC/pl/m_equip/Guts/arm/Gauntlet_BML.tex"];

    for paths in [beside.as_slice(), inside.as_slice()] {
        let mut with_slot = vec!["nativePC/pl/m_equip/pl071_0000/arm/mod/m_arm071_0000.mod3"];
        with_slot.extend_from_slice(paths);

        let analysis = analysis_of(&with_slot);
        assert_eq!(
            analysis.matched_asset_count(),
            with_slot.len(),
            "{paths:?} 必须与槽位文件一起被完整计入"
        );

        let plan = male_plan_for(&with_slot, "pl069_0000", true).expect("计划必须生成");
        assert_eq!(plan.actions().len(), with_slot.len());
        assert_eq!(
            target_of(&plan, paths[0]),
            paths[0],
            "随行文件原样安装，路径不变"
        );
    }
}

/// 游戏根之外的文件不安装，但**必须有归属**（`#363`）。
///
/// 与「不认识就丢」的区别在于判据：这里是「有没有游戏根」这个结构事实，
/// 普通安装同样按它过滤（`allowed_install_roots`），两条路径口径一致。
/// 分档对账因此仍然成立——它们没有丢，是被明确判为不可安装。
#[test]
fn files_outside_the_game_root_are_not_installed_and_do_not_break_accounting() {
    let paths = [
        "nativePC/pl/m_equip/pl071_0000/arm/mod/m_arm071_0000.mod3",
        "nativePC/pl/Guts/arm/Gauntlet_BML.tex",
        "readme.txt",
        "preview/screenshot.png",
    ];

    let analysis = analysis_of(&paths);
    assert_eq!(
        analysis.matched_asset_count(),
        2,
        "只有游戏根之下的两个文件可安装"
    );

    let plan = male_plan_for(&paths, "pl069_0000", true).expect("计划必须生成");
    assert_eq!(plan.actions().len(), 2);
    assert!(plan.actions().iter().all(|action| action
        .source_relative_path()
        .as_str()
        .starts_with("nativePC/")));
}
