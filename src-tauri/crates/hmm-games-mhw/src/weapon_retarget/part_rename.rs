//! 武器侧文件名改写规则：把主干里的**源槽位数字**换成**目标槽位数字**。
//!
//! # 为什么是结构规则而不是词表（#343）
//!
//! 上一版从 role 推导目标部件名，因此必须先在 `WeaponFamily::secondary_part()` 里登记这个
//! 部件的前缀。表里只有 `sld` / `sou_r` / `saya` 三项，**14 个武器族里 10 个返回 `None`**，
//! 于是这些族的包只要携带任何副件模型就判 `weapon_unknown_part` 并**否决整包**——弓类带
//! 副件模型的包即是一例。这与 #336 ②b 是同一个病：靠一张事先记录的表，没记录的就拒绝。
//!
//! 改名本身并不需要知道部件叫什么。它只需要知道两件事：源槽位的数字、目标槽位的数字。
//!
//! ```text
//! 主干 = <bs_?><前缀:[A-Za-z_]+><源槽位 3 位数字><余部>
//! 目标 = <目标槽位的 bs_?><同一前缀><目标槽位 3 位数字><余部>
//! ```
//!
//! `bs_` 必须归一化而不能当成前缀的一部分：catalog 里 601 个槽位有 176 个（29%）带 `bs_`，
//! 真机实验 B 观测到 `bs_two012_BML.dds` 落成 `two020_BML.dds`——源带 `bs_`、目标不带时
//! 整个 `bs_` 都要去掉。纯数字替换会得出 `bs_two020`，是错的。
//!
//! 规则覆盖既有全部形态，并额外免费支持任何未登记前缀：
//!
//! | 输入（源 `two003` / `swo035` / `bs_two012`） | 输出 | 分支 |
//! | --- | --- | --- |
//! | `two003.mod3` | `two019.mod3` | 主件 |
//! | `two003_BML.tex` | `two019_BML.tex` | 伴生 |
//! | `bs_two012_XM.tex`（目标 `two020`） | `two020_XM.tex` | `bs_` 归一化 |
//! | `saya035.mod3` | `saya019.mod3` | 副件 |
//! | `saya035ol.mod3` | `saya019ol.mod3` | 副件 + 变体后缀 |
//! | `swo035_off_deco.ctc` | `swo019_off_deco.ctc` | 伴生 |
//! | `two0031_x.tex` | 原样 | 守卫① |
//! | `two003_two003.tex` | ambiguous | 守卫② |
//! | `131072_2599467785140006031 BML.dds` | 原样 | 前缀为空 |
//! | `DARKMOON_BML.tex` | 原样 | 无匹配数字段 |
//!
//! **本模块是磁盘改名与 MRL3 引用改写的唯一实现**。两处必须对同一个文件名得出同一个结论，
//! 否则重定向会「成功」但游戏里贴图缺失。切片② 的反向验证已证明这是最有价值的不变量：
//! 停用本模块，两侧的测试**同时**转红。
//!
//! **与防具侧不可复用同一函数**：防具是槽位编号段 `<3位>_<4位>` 的整路径替换，没有 `bs_`
//! 这类前缀归一化，结构不同。

use super::family::WeaponMainId;

/// 一次文件名改写的结论。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum PartRename {
    /// 主干命中源槽位数字，已换成目标槽位数字。
    Renamed(String),
    /// 与本槽位无关，原样保留。
    Unrelated,
    /// 含源槽位数字但无法安全替换。调用方失败关闭，不猜。
    Ambiguous,
}

/// 主干拆解结果。`parse_for_main` 与本模块共用，保证识别与改名对同一个名字结论一致。
pub(super) struct WeaponStem<'a> {
    /// `bs_` 之后、数字之前的部件前缀，如 `two`、`saya`、`sou_r`、`ya`。
    pub(super) prefix: &'a str,
    /// 数字之后的剩余部分，含扩展名。
    pub(super) rest: &'a str,
}

/// 按结构拆解一个文件名主干，要求它属于 `source` 这个槽位。
///
/// 返回 `None` 表示与本槽位无关；`Some(Err(()))` 表示命中但撞守卫②，调用方应失败关闭。
#[allow(clippy::result_unit_err)]
pub(super) fn split_weapon_stem<'a>(
    stem: &'a str,
    source: &WeaponMainId,
) -> Option<Result<WeaponStem<'a>, ()>> {
    // `bs_` 是槽位身份的一部分，不是前缀的一部分：源带目标不带时整段都要换掉。
    let (stem_has_bs, body) = match stem.strip_prefix("bs_") {
        Some(body) => (true, body),
        None => (false, stem),
    };
    if stem_has_bs != source.has_bs_prefix() {
        return None;
    }

    // 前缀必须非空且不以数字开头，否则 `131072_...` 这类作者中间产物会被误判。
    let prefix_len = body
        .bytes()
        .take_while(|byte| byte.is_ascii_alphabetic() || *byte == b'_')
        .count();
    if prefix_len == 0 {
        return None;
    }
    let (prefix, after_prefix) = body.split_at(prefix_len);

    let digits = format!("{:03}", source.number());
    let rest = after_prefix.strip_prefix(digits.as_str())?;

    // 守卫①：数字段后面不能再跟数字。部件 ID 形如 `<前缀><3 位数字>`，否则
    // `two0031_x.tex` 会被读成 `two003` + 余部 `1_x`，错改成 `two0191_x.tex`。
    if rest.as_bytes().first().is_some_and(u8::is_ascii_digit) {
        return None;
    }

    // 守卫②：余部里不能再出现「字母/下划线 + 同一数字段」。`two003_two003.tex` 无法判断
    // 作者意图，且只改前一处会让后一处残留指向源槽位。
    let rest_bytes = rest.as_bytes();
    let ambiguous =
        rest.match_indices(digits.as_str())
            .any(|(index, _)| match index.checked_sub(1) {
                Some(previous) => {
                    rest_bytes[previous].is_ascii_alphabetic() || rest_bytes[previous] == b'_'
                }
                None => false,
            });
    if ambiguous {
        return Some(Err(()));
    }

    Some(Ok(WeaponStem { prefix, rest }))
}

/// 把文件名（含扩展名）从源槽位改写到目标槽位。
///
/// 扩展名只是 `rest` 的一部分，因此本函数对 `two003.mod3` 与 `two003_BML.tex` 一视同仁。
pub(super) fn rename_weapon_stem(
    stem: &str,
    source: &WeaponMainId,
    target: &WeaponMainId,
) -> PartRename {
    match split_weapon_stem(stem, source) {
        None => PartRename::Unrelated,
        Some(Err(())) => PartRename::Ambiguous,
        Some(Ok(parsed)) => {
            let bs_prefix = if target.has_bs_prefix() { "bs_" } else { "" };
            PartRename::Renamed(format!(
                "{bs_prefix}{}{:03}{}",
                parsed.prefix,
                target.number(),
                parsed.rest
            ))
        }
    }
}
