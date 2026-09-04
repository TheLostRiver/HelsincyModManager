//! 武器侧文件名改写规则：把文件名开头的源部件 ID 换成目标部件 ID。
//!
//! 为什么需要它（#336）：旧规则只认三种情况——整段等于部件 ID、去扩展名后等于部件 ID、
//! 其余只要「包含」部件 ID 就判 ambiguous。而真实 Mod 的贴图叫 `two003_BML.tex`、
//! `bs_two012_XM.tex`，全部落进第三条，导致二进制改写阶段必然失败
//! （`weapon_binary_reference_ambiguous`，已用真实 MRL3 字节实测）。
//!
//! 规则由参照实现独立验证：第三方管理器把 `bs_two012_BML.dds` 改名为 `two020_BML.dds`，
//! 即**整个部件 ID 前缀**被替换（注意源含 `bs_` 而目标不含）。

/// 一次文件名改写的结论。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum PartRename {
    /// 文件名以某个源部件 ID 开头，已替换为对应的目标部件 ID。
    Renamed(String),
    /// 文件名与任何源部件 ID 无关，原样保留。
    Unrelated,
    /// 含源部件 ID 但无法安全替换（不在开头，或替换后仍残留）。
    /// 调用方应降级：不改写这一项并计入告警，而不是让整个操作失败。
    Ambiguous,
}

/// 按 `mappings`（源部件 ID → 目标部件 ID）改写文件名段。
///
/// 判定顺序与两条守卫：
/// 1. 取**最长**的、作为前缀命中的源部件 ID。取最长是因为同族部件 ID 可能互为前缀
///    （`two003` 与 `bs_two003`）。
/// 2. **守卫①**：命中后的下一个字符不能是数字。部件 ID 形如 `<prefix><3 位数字>`，
///    若不设此守卫，`two0031_x.tex` 会被当成 `two003` + `1_x.tex` 而错改成 `two0191_x.tex`。
/// 3. **守卫②**：剩余部分不得再次出现任何源部件 ID。`two003_two003.tex` 这类无法判断
///    作者意图，判 `Ambiguous` 由调用方降级。
pub(super) fn rename_part_prefix(segment: &str, mappings: &[(String, String)]) -> PartRename {
    let matched = mappings
        .iter()
        .filter(|(source, _)| segment.starts_with(source.as_str()))
        .max_by_key(|(source, _)| source.len());

    let Some((source, target)) = matched else {
        // 没有前缀命中；但若部件 ID 出现在别处，说明这个名字与部件有关而我们改不动它。
        if mappings
            .iter()
            .any(|(source, _)| segment.contains(source.as_str()))
        {
            return PartRename::Ambiguous;
        }
        return PartRename::Unrelated;
    };

    let remainder = &segment[source.len()..];

    // 守卫①
    if remainder
        .as_bytes()
        .first()
        .is_some_and(u8::is_ascii_digit)
    {
        return PartRename::Unrelated;
    }

    // 守卫②
    if mappings
        .iter()
        .any(|(source, _)| remainder.contains(source.as_str()))
    {
        return PartRename::Ambiguous;
    }

    PartRename::Renamed(format!("{target}{remainder}"))
}
