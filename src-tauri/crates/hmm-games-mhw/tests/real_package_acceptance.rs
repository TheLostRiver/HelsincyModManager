//! `#349` 切片⑤：真实 Mod 包的**接受率回归**。
//!
//! 夹具是真实第三方包的**完整路径清单**（纯字符串，逐条来自实测导出）。真实包本身不入库
//! （`自动测试不得使用第三方 Mod 包`），但路径清单必须原样保留——`#337` 的教训是夹具一旦
//! 用编造的文件名，代码、文档、测试就会三方自洽地错。
//!
//! **这个文件存在的理由**：此前每次改分类器都靠「写一次性探针 → 跑 → 删掉」来确认接受率，
//! 一轮下来重复了四次，而且探针一删，下一轮就没有基线可比。这里把接受率钉成断言——任何
//! 改动只要降低接受率、或改变了划分结果，就立刻转红。
//!
//! 判据是**逐字相等**而不是「没报错」：只断言 `is_ok()` 会漏掉「接受了但分档错了」这类更
//! 隐蔽的回归（例如伴生文件从 `Verbatim` 漂成 `Relocated`，那会让它被搬走并改名）。
//!
//! | 夹具 | 形态出处 |
//! | --- | --- |
//! | [`BIANCA_BOW_TYPE1`] | 包内游戏根是**小写** `nativepc/`（`#345`）；未登记前缀 `ya017`（`#343`）；族级作者目录 `PGR/`；四种没登记过的扩展名 |
//! | [`ROSE_DRESS_PL123`] | 防具五部位齐全 + 作者自建目录 + `.ctc` / `.ccl` / `.evhl` 伴生 |
//! | [`WEAPON_COLLISION_DATA`] | 对照组：路径里有 `/wp/` 但不在 `nativePC/wp/` 下，**必须**判定为没有可重定向资源 |
//!
//! 武器侧另有三个真实包的清单在 `tests/weapon_package_classifier.rs`（那里侧重分档细节，
//! 这里侧重「整包会不会被拒」）。

use hmm_core::{GameId, PackageFileId};
use hmm_games_mhw::{
    analyze_mhw_weapon_assets, MhwArmorReplacementAdapter, WeaponAnalysisError,
    WeaponCompanionPlacement,
};
use hmm_ports::{ReplacementAdapter, ReplacementAnalysisRequest, ReplacementAsset};

/// 「Bianca 弓」`mod-import-1788599257454-4`，源槽位 `bow017`。
///
/// 压缩包根就是**小写** `nativepc/`——2022 年打包的真实包，沙箱原样保留。它同时是 `#343`
/// 未登记前缀（`ya017`）唯一的真实来源，以及 `#345` / `#346` 的触发包。
const BIANCA_BOW_TYPE1: &[&str] = &[
    "nativepc/wp/bow/PGR/BML.tex",
    "nativepc/wp/bow/PGR/BML2.tex",
    "nativepc/wp/bow/PGR/EM.tex",
    "nativepc/wp/bow/PGR/EM2.tex",
    "nativepc/wp/bow/PGR/NM.tex",
    "nativepc/wp/bow/PGR/NM1.tex",
    "nativepc/wp/bow/PGR/NM2.tex",
    "nativepc/wp/bow/PGR/RMT.tex",
    "nativepc/wp/bow/PGR/RMT1.tex",
    "nativepc/wp/bow/PGR/RMT2.tex",
    "nativepc/wp/bow/bow017/epv/bow017.epv3",
    "nativepc/wp/bow/bow017/mod/bow017.evwp",
    "nativepc/wp/bow/bow017/mod/bow017.mod3",
    "nativepc/wp/bow/bow017/mod/bow017.mrl3",
    "nativepc/wp/bow/bow017/mod/ya017.evwp",
    "nativepc/wp/bow/bow017/mod/ya017.mod3",
    "nativepc/wp/bow/bow017/mod/ya017.mrl3",
    "nativepc/wp/bow/bow017/mod/ya017_off_deco.ctc",
    "nativepc/wp/bow/bow017/mod/ya017_on_deco.ctc",
    "nativepc/wp/bow/bow017/sound/snd_bow017_bk.wwbk",
];

/// 「玫瑰礼服（替换盛装）」，源槽位 `pl123_0000`（女性装备）。
///
/// 防具侧唯一的完整真实样本：五个部位齐全、每个部位的文件名都带槽位令牌
/// （`f_<部位>123_0000`），另有作者自建目录 `mod_pl_rosedress/` 与三种伴生扩展名。
const ROSE_DRESS_PL123: &[&str] = &[
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
    "nativePC/pl/f_equip/pl123_0000/arm/mod/f_arm123_0000.ctc",
    "nativePC/pl/f_equip/pl123_0000/arm/mod/f_arm123_0000.mod3",
    "nativePC/pl/f_equip/pl123_0000/arm/mod/f_arm123_0000.mrl3",
    "nativePC/pl/f_equip/pl123_0000/body/mod/f_body123_0000.ccl",
    "nativePC/pl/f_equip/pl123_0000/body/mod/f_body123_0000.ctc",
    "nativePC/pl/f_equip/pl123_0000/body/mod/f_body123_0000.mod3",
    "nativePC/pl/f_equip/pl123_0000/body/mod/f_body123_0000.mrl3",
    "nativePC/pl/f_equip/pl123_0000/helm/mod/f_helm123_0000.ctc",
    "nativePC/pl/f_equip/pl123_0000/helm/mod/f_helm123_0000.evhl",
    "nativePC/pl/f_equip/pl123_0000/helm/mod/f_helm123_0000.mod3",
    "nativePC/pl/f_equip/pl123_0000/helm/mod/f_helm123_0000.mrl3",
    "nativePC/pl/f_equip/pl123_0000/leg/mod/f_leg123_0000.ccl",
    "nativePC/pl/f_equip/pl123_0000/leg/mod/f_leg123_0000.ctc",
    "nativePC/pl/f_equip/pl123_0000/leg/mod/f_leg123_0000.mod3",
    "nativePC/pl/f_equip/pl123_0000/leg/mod/f_leg123_0000.mrl3",
    "nativePC/pl/f_equip/pl123_0000/wst/mod/f_wst123_0000.ctc",
    "nativePC/pl/f_equip/pl123_0000/wst/mod/f_wst123_0000.mod3",
    "nativePC/pl/f_equip/pl123_0000/wst/mod/f_wst123_0000.mrl3",
];

/// 对照组：武器**碰撞体**数据包。
///
/// 路径里有 `/wp/`，但落在 `nativePC/hm/wp/` 下、不是 `nativePC/wp/`；没有槽位、没有模型，
/// 「重定向」对它没有意义。它必须被判定为「没有可重定向资源」——这条是防止把接受率做过头：
/// 放宽不等于什么都收。
const WEAPON_COLLISION_DATA: &[&str] = &[
    "nativePC/hm/wp/wp00/collision/wp00.col",
    "nativePC/hm/wp/wp00/collision/wp00_01.col",
    "nativePC/hm/wp/wp01/collision/wp01.col",
    "nativePC/hm/wp/wp01/collision/wp01_01.col",
    "nativePC/hm/wp/wp01/collision/wp01_02.col",
    "nativePC/hm/wp/wp02/collision/wp02.col",
    "nativePC/hm/wp/wp03/collision/wp03.col",
    "nativePC/hm/wp/wp03/shell/collision/object.col",
    "nativePC/hm/wp/wp04/collision/wp04.col",
    "nativePC/hm/wp/wp05/collision/wp05.col",
    "nativePC/hm/wp/wp05/shell/collision/object.col",
    "nativePC/hm/wp/wp06/collision/wp06.col",
    "nativePC/hm/wp/wp06/collision/wp06_01.col",
    "nativePC/hm/wp/wp07/collision/wp07.col",
    "nativePC/hm/wp/wp07/collision/wp07_01.col",
    "nativePC/hm/wp/wp07/shell/collision/object.col",
    "nativePC/hm/wp/wp08/collision/wp08.col",
    "nativePC/hm/wp/wp08/shell/collision/object.col",
    "nativePC/hm/wp/wp09/collision/wp09.col",
    "nativePC/hm/wp/wp09/collision/wp09_01.col",
    "nativePC/hm/wp/wp09/shell/collision/object.col",
    "nativePC/hm/wp/wp10/collision/wp10.col",
    "nativePC/hm/wp/wp10/collision/wp10_01.col",
    "nativePC/hm/wp/wp10/shell/collision/object.col",
    "nativePC/hm/wp/wp11/collision/wp11.col",
    "nativePC/hm/wp/wp11/collision/wp11_01.col",
    "nativePC/hm/wp/wp11/shell/collision/object.col",
    "nativePC/hm/wp/wp12/collision/wp12.col",
    "nativePC/hm/wp/wp12/shell/collision/object.col",
    "nativePC/hm/wp/wp13/collision/wp13.col",
];

fn assets(paths: &[&str]) -> Vec<ReplacementAsset> {
    paths
        .iter()
        .map(|path| ReplacementAsset::new(PackageFileId::new(*path), *path))
        .collect()
}

/// 弓包的每一个文件都要有确定的归属，逐条钉住。
///
/// `#345` 之前这个包完全不可重定向（小写根段匹配不上）；`#346` 之前两个 MOD3 都被预检拒。
#[test]
fn the_bow_package_is_accepted_with_every_file_accounted_for() {
    let analysis = analyze_mhw_weapon_assets(&assets(BIANCA_BOW_TYPE1))
        .expect("小写 nativepc 的真实弓包必须可重定向");

    let [unit] = analysis.units() else {
        panic!("期望恰好一个源槽位，实际 {}", analysis.units().len());
    };

    assert_eq!(unit.root().main_id().as_str(), "bow017");

    // 两个模型对：主件 bow017 + 未登记前缀的副件 ya017（`#343` 的核心能力）。
    assert_eq!(
        unit.pairs()
            .iter()
            .map(|pair| pair.part_id().as_str())
            .collect::<Vec<_>>(),
        vec!["bow017", "ya017"],
        "主件与未登记前缀的副件都必须成对识别"
    );

    // 伴生文件的**落位**是承重的：`Relocated` 会被搬到目标槽位并改名，`Verbatim` 留原路径。
    // 判错方向会让族级贴图被搬走（游戏找不到）或槽位内文件留在原地（重定向后缺件）。
    let mut relocated = unit
        .companions()
        .iter()
        .filter(|companion| companion.placement() == WeaponCompanionPlacement::Relocated)
        .map(|companion| companion.relative_path().as_str())
        .collect::<Vec<_>>();
    let mut verbatim = unit
        .companions()
        .iter()
        .filter(|companion| companion.placement() == WeaponCompanionPlacement::Verbatim)
        .map(|companion| companion.relative_path().as_str())
        .collect::<Vec<_>>();
    relocated.sort_unstable();
    verbatim.sort_unstable();

    assert_eq!(
        relocated,
        vec![
            // 归一化之后一律是规范大小写 `nativePC`（`#345`）。
            "nativePC/wp/bow/bow017/epv/bow017.epv3",
            "nativePC/wp/bow/bow017/mod/bow017.evwp",
            "nativePC/wp/bow/bow017/mod/ya017.evwp",
            "nativePC/wp/bow/bow017/mod/ya017_off_deco.ctc",
            "nativePC/wp/bow/bow017/mod/ya017_on_deco.ctc",
            "nativePC/wp/bow/bow017/sound/snd_bow017_bk.wwbk",
        ],
        "源槽位目录内的伴生文件必须随行改名——包括没登记过的 .evwp / .ctc / .epv3 / .wwbk"
    );
    assert_eq!(
        verbatim,
        vec![
            "nativePC/wp/bow/PGR/BML.tex",
            "nativePC/wp/bow/PGR/BML2.tex",
            "nativePC/wp/bow/PGR/EM.tex",
            "nativePC/wp/bow/PGR/EM2.tex",
            "nativePC/wp/bow/PGR/NM.tex",
            "nativePC/wp/bow/PGR/NM1.tex",
            "nativePC/wp/bow/PGR/NM2.tex",
            "nativePC/wp/bow/PGR/RMT.tex",
            "nativePC/wp/bow/PGR/RMT1.tex",
            "nativePC/wp/bow/PGR/RMT2.tex",
        ],
        "族级作者目录 PGR/ 与槽位无关，必须留在原路径"
    );

    assert!(
        unit.unresolved_models().is_empty(),
        "这个包里每个模型都能判断如何改写"
    );
    assert!(unit.excluded().is_empty(), "包里没有危险类型");

    // 全部 20 个文件都有归属：2 对模型（4 个文件）+ 16 个伴生。
    assert_eq!(
        unit.pairs().len() * 2 + unit.companions().len(),
        BIANCA_BOW_TYPE1.len(),
        "不能有文件在分档中丢失"
    );
}

/// `#292` / `#345` 的等价性在真实包上再钉一次：根段大小写不影响任何产出。
#[test]
fn the_bow_package_analysis_is_identical_across_root_casings() {
    let lowercase = analyze_mhw_weapon_assets(&assets(BIANCA_BOW_TYPE1)).expect("小写根段");

    let canonical_paths = BIANCA_BOW_TYPE1
        .iter()
        .map(|path| path.replacen("nativepc/", "nativePC/", 1))
        .collect::<Vec<_>>();
    let canonical_refs = canonical_paths
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();
    let canonical = analyze_mhw_weapon_assets(&assets(&canonical_refs)).expect("规范根段");

    // `package_file_id` 用的是各自的原始路径，所以只比对分析出的目标侧结构。
    assert_eq!(
        lowercase
            .units()
            .iter()
            .map(|unit| unit.root().normalized_path().as_str())
            .collect::<Vec<_>>(),
        canonical
            .units()
            .iter()
            .map(|unit| unit.root().normalized_path().as_str())
            .collect::<Vec<_>>()
    );
    assert_eq!(
        lowercase
            .units()
            .iter()
            .flat_map(|unit| unit.companions())
            .map(|companion| (companion.relative_path().as_str(), companion.placement()))
            .collect::<Vec<_>>(),
        canonical
            .units()
            .iter()
            .flat_map(|unit| unit.companions())
            .map(|companion| (companion.relative_path().as_str(), companion.placement()))
            .collect::<Vec<_>>(),
        "两种根段写法必须产出逐字相同的伴生落位"
    );
    assert_eq!(
        lowercase.units()[0].source_id(),
        canonical.units()[0].source_id(),
        "源身份不能随包内大小写漂移"
    );
}

/// 防具侧：五部位齐全的真实套装必须识别出唯一且可用的源槽位。
#[test]
fn the_rose_dress_package_is_accepted_by_the_armor_adapter() {
    let analysis = MhwArmorReplacementAdapter
        .analyze_replacement_assets(ReplacementAnalysisRequest {
            game_id: GameId::mhw(),
            assets: assets(ROSE_DRESS_PL123),
        })
        .expect("五部位齐全的真实套装必须可重定向");

    let [source] = analysis.sources() else {
        panic!("期望恰好一个源槽位，实际 {}", analysis.sources().len());
    };
    assert_eq!(source.internal_id(), "pl123_0000");
    assert!(source.is_supported(), "女性装备槽位应当可用");
    assert_eq!(source.path_family(), "pl/f_equip");

    // 作者自建目录 `mod_pl_rosedress/` 的 10 个文件不构成第二个源槽位。
    assert_eq!(
        analysis.sources().len(),
        1,
        "作者自建目录不能被当成另一个槽位"
    );
}

/// 放宽不等于什么都收：没有可重定向资源的包必须如实报出来。
#[test]
fn a_package_without_any_retargetable_resource_is_reported_as_such() {
    assert_eq!(
        analyze_mhw_weapon_assets(&assets(WEAPON_COLLISION_DATA)),
        Err(WeaponAnalysisError::SourceNotFound),
        "碰撞体数据落在 nativePC/hm/wp/ 下，没有槽位也没有模型"
    );

    // 防具适配器同样不该从它里面认出源槽位。
    let analysis =
        MhwArmorReplacementAdapter.analyze_replacement_assets(ReplacementAnalysisRequest {
            game_id: GameId::mhw(),
            assets: assets(WEAPON_COLLISION_DATA),
        });
    match analysis {
        Ok(analysis) => assert!(
            analysis.sources().is_empty(),
            "碰撞体数据不含防具槽位，实际 {:?}",
            analysis.sources()
        ),
        Err(_) => { /* 报错也可接受：同样表示「没有可重定向的防具资源」 */ }
    }
}
