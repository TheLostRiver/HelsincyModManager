use crate::install::ModId;
use std::path::Path;

/// Mod 展示名的字符上限。
///
/// 展示名会进入 SQLite 投影的 `display_name` 列与搜索键，来源包含压缩包内的
/// manifest/readme 文本与用户选择的压缩包文件名——都是外部可控文本。
/// 上限按字符而非字节计数，避免中文名被截断到无法辨认。
pub const MOD_METADATA_MAX_DISPLAY_NAME_CHARS: usize = 80;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModMetadataOverlay {
    pub mod_id: ModId,
    pub display_name: Option<String>,
    pub author: Option<String>,
    pub version: Option<String>,
    pub description: Option<String>,
    pub nexus_mod_id: Option<u64>,
    pub updated_at: u128,
}

/// 把外部来源的文本净化成可用作 Mod 展示名的字符串，无法产出有效名称时返回 `None`。
///
/// 折叠连续空白、滤掉控制字符、按字符截断到 [`MOD_METADATA_MAX_DISPLAY_NAME_CHARS`]。
/// 截断发生在 `trim` 之前，因此正好切在空白处时尾随空白仍会被清掉。
///
/// 所有展示名来源都必须经过这里。压缩包文件名是比 manifest 更现实的攻击面，
/// 若派生路径自己写一套规则就会绕过长度上限与控制字符过滤。
pub fn sanitize_mod_metadata_text(value: &str) -> Option<String> {
    let normalized = value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .filter(|character| !character.is_control())
        .take(MOD_METADATA_MAX_DISPLAY_NAME_CHARS)
        .collect::<String>();
    let normalized = normalized.trim();

    if normalized.is_empty() {
        None
    } else {
        Some(normalized.to_owned())
    }
}

/// 把展示名归一化成同名判定用的比较键。
///
/// 去首尾空白并统一大小写：玩家眼里 `黑骑士大剑 ` 与 `黑骑士大剑`、`BlackKnight`
/// 与 `blackknight` 是同一个名字，判重要跟着这个直觉走，否则"看起来一样却当成两个"
/// 会让去重和冲突提示都失去意义。
///
/// 所有同名判定都必须共用这一份规则。外部导入的准入检查与新建 Mod 的自动去重若各写
/// 一套，去重产出的名字仍可能撞上准入检查——两边各自"正确"却互相打架。
pub fn normalize_mod_display_name(value: &str) -> String {
    value.trim().to_lowercase()
}

/// 从压缩包路径派生展示名候选，无法产出有效名称时返回 `None`。
///
/// 只取 `file_stem`，即剥掉最后一级扩展名——`mod.v1.2.zip` 保留 `mod.v1.2`，
/// 因为版本号点号是名称的一部分。
///
/// 刻意用 `to_str()` 而不是 `to_string_lossy()`：非 UTF-8 文件名应当让调用方
/// 回落到内部标识，而不是把 U+FFFD 替换字符写进 catalog 与搜索键。
pub fn mod_display_name_from_archive_path(archive_path: &Path) -> Option<String> {
    archive_path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .and_then(sanitize_mod_metadata_text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collapses_interior_whitespace_and_trims_edges() {
        assert_eq!(
            sanitize_mod_metadata_text("  黑骑士   大剑\t\n改版  "),
            Some("黑骑士 大剑 改版".to_owned())
        );
    }

    #[test]
    fn filters_control_characters_without_rejecting_the_whole_value() {
        assert_eq!(
            sanitize_mod_metadata_text("Black\u{0}Knight\u{7}"),
            Some("BlackKnight".to_owned())
        );
    }

    #[test]
    fn truncates_by_characters_so_multibyte_names_stay_readable() {
        let long_name = "黑".repeat(MOD_METADATA_MAX_DISPLAY_NAME_CHARS + 20);

        let sanitized = sanitize_mod_metadata_text(&long_name).expect("non-empty");

        // 按字符截断：80 个汉字，而不是 80 字节（后者只有 26 个字）。
        assert_eq!(
            sanitized.chars().count(),
            MOD_METADATA_MAX_DISPLAY_NAME_CHARS
        );
    }

    #[test]
    fn returns_none_for_values_that_sanitize_to_nothing() {
        // 调用方必须自行回落，不能把空串写进投影——那会让整个投影写入硬失败。
        for value in ["", "   ", "\t\n", "\u{0}\u{7}"] {
            assert_eq!(sanitize_mod_metadata_text(value), None, "value: {value:?}");
        }
    }

    #[test]
    fn keeps_punctuation_and_mixed_scripts() {
        assert_eq!(
            sanitize_mod_metadata_text("Black Knight 大剑 v2.1 [HD]"),
            Some("Black Knight 大剑 v2.1 [HD]".to_owned())
        );
    }

    #[test]
    fn normalizes_case_and_edge_whitespace_for_name_comparison() {
        assert_eq!(
            normalize_mod_display_name("  BlackKnight  "),
            normalize_mod_display_name("blackknight")
        );
    }

    #[test]
    fn normalization_keeps_interior_whitespace_significant() {
        // "黑骑士 大剑" 与 "黑骑士大剑" 是两个名字：折叠内部空白是净化的职责，
        // 归一化再折叠一次会让净化后本就不同的名字被误判为重名。
        assert_ne!(
            normalize_mod_display_name("黑骑士 大剑"),
            normalize_mod_display_name("黑骑士大剑")
        );
    }

    #[test]
    fn derives_display_name_from_archive_file_stem() {
        assert_eq!(
            mod_display_name_from_archive_path(Path::new("C:/mods/黑骑士大剑.zip")),
            Some("黑骑士大剑".to_owned())
        );
    }

    #[test]
    fn keeps_interior_dots_when_stripping_the_extension() {
        // 只剥最后一级扩展名：版本号里的点属于名称。
        assert_eq!(
            mod_display_name_from_archive_path(Path::new("mod.v1.2.zip")),
            Some("mod.v1.2".to_owned())
        );
    }

    #[test]
    fn keeps_stems_that_look_odd_but_sanitize_to_something() {
        // `___` 是有效名称，不该被丢弃——调用方只在 None 时才回落内部标识。
        assert_eq!(
            mod_display_name_from_archive_path(Path::new("___.zip")),
            Some("___".to_owned())
        );
    }

    #[test]
    fn returns_none_when_the_stem_has_no_usable_characters() {
        for path in [" .zip", "  ", "\t.7z"] {
            assert_eq!(
                mod_display_name_from_archive_path(Path::new(path)),
                None,
                "path: {path:?}"
            );
        }
    }

    #[test]
    fn returns_none_for_paths_without_a_file_stem() {
        assert_eq!(mod_display_name_from_archive_path(Path::new("")), None);
        assert_eq!(mod_display_name_from_archive_path(Path::new("..")), None);
    }
}
