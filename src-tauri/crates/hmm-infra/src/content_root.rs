//! 解析导入包的「内容根」。
//!
//! 第三方压缩包普遍在 `nativePC` 外面套一层甚至多层包装目录
//! （`黑骑士大剑/nativePC/...`），而安装路径过滤要求目标路径以 `nativePC` 打头。
//! 因此必须先把真正的 `nativePC` 找出来，以它**所在的目录**作为内容根，
//! 再据此计算目标路径——否则整个包都会被过滤掉，装成一个空计划（见 #285）。
//!
//! 预览图扫描与安装文件扫描**共用**这里的解析，避免两处对「内容根在哪」
//! 产生不同判断（那正是 #284 的现象：图能显示、却装不上）。

use anyhow::{Context, Result};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// `nativePC` 目录名。比较时**大小写不敏感**——实测第三方包里出现过
/// `NATIVEpc` 这类写法。
pub const NATIVE_PC_DIR_NAME: &str = "nativepc";

/// 内容根搜索的深度上限。
///
/// 这不是「允许几层包装目录」的语义限制，而是**防御性**约束：防止恶意包用
/// 超深目录把遍历变成无界递归。现实分布是 1 层最常见、2 层偶尔、3 层以上罕见，
/// 因此 4 已给足余量。
pub const MAX_CONTENT_ROOT_SEARCH_DEPTH: usize = 4;

/// 内容根解析结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContentRootResolution {
    /// 沙箱内没有 `nativePC`：回退为沙箱根本身。
    ///
    /// 与预览图扫描的既有行为保持一致；至于有没有可安装文件，交给安装路径
    /// 过滤与 #285 的空计划检查去决定，这里不抢先下结论。
    Fallback(PathBuf),
    /// 恰好一个 `nativePC`：内容根是它所在的目录。
    Single(PathBuf),
    /// 多个 `nativePC`（合集包）。
    ///
    /// **不替用户挑一个**——静默合并会写入玩家没预期的文件。调用方必须拒绝，
    /// 与 #278 否决「按历史绑定自动重定向」是同一条原则。
    Ambiguous(Vec<PathBuf>),
}

impl ContentRootResolution {
    /// 安装时使用的内容根；合集包返回 `None`，调用方应拒绝安装。
    pub fn install_root(&self) -> Option<&Path> {
        match self {
            Self::Fallback(root) | Self::Single(root) => Some(root.as_path()),
            Self::Ambiguous(_) => None,
        }
    }
}

/// 解析导入包的内容根。
pub fn resolve_content_root(sandbox_root: &Path) -> Result<ContentRootResolution> {
    let parents = native_pc_parents(sandbox_root)?;

    Ok(match parents.len() {
        0 => ContentRootResolution::Fallback(sandbox_root.to_path_buf()),
        1 => ContentRootResolution::Single(
            parents
                .into_iter()
                .next()
                .expect("length is checked to be 1"),
        ),
        _ => ContentRootResolution::Ambiguous(parents.into_iter().collect()),
    })
}

/// 找出沙箱内所有 `nativePC` 目录的**父目录**。
///
/// 供预览图扫描复用：它可能接受多个候选根（遇到多个 `nativePC` 时都会收图），
/// 与安装侧「多个即拒绝」的严格语义不同。
pub fn native_pc_parents(sandbox_root: &Path) -> Result<BTreeSet<PathBuf>> {
    let mut parents = BTreeSet::new();
    collect_native_pc_parents(sandbox_root, 0, &mut parents)?;
    Ok(parents)
}

fn collect_native_pc_parents(
    current_dir: &Path,
    depth: usize,
    out: &mut BTreeSet<PathBuf>,
) -> Result<()> {
    if depth >= MAX_CONTENT_ROOT_SEARCH_DEPTH {
        return Ok(());
    }

    for entry in
        std::fs::read_dir(current_dir).context("failed to read imported mod sandbox directory")?
    {
        let entry = entry.context("failed to read imported mod sandbox entry")?;
        let file_type = entry
            .file_type()
            .context("failed to inspect imported mod sandbox entry")?;

        // 符号链接与目录联接一律跳过：内容根必须是真实目录，否则会被用来
        // 指到沙箱外面。
        if file_type.is_symlink() || !file_type.is_dir() {
            continue;
        }

        if entry
            .file_name()
            .to_string_lossy()
            .eq_ignore_ascii_case(NATIVE_PC_DIR_NAME)
        {
            out.insert(current_dir.to_path_buf());
            // 不再深入 `nativePC` 内部：内容根是它的父目录，里面不可能有
            // 另一个「更该被当作内容根」的层级。
            continue;
        }

        collect_native_pc_parents(&entry.path(), depth + 1, out)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn write(path: &Path) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create parent");
        }
        fs::write(path, b"fixture").expect("write fixture");
    }

    #[test]
    fn native_pc_directly_under_the_sandbox_root_resolves_to_the_root() {
        let temp = tempfile::tempdir().expect("temp dir");
        write(&temp.path().join("nativePC/models/player.mod3"));

        let parents = native_pc_parents(temp.path()).expect("scan");
        assert_eq!(parents.len(), 1);
        assert_eq!(
            parents.iter().next().map(|path| path.as_path()),
            Some(temp.path())
        );
    }

    #[test]
    fn a_single_wrapper_directory_is_resolved_as_the_content_root() {
        let temp = tempfile::tempdir().expect("temp dir");
        let sandbox = temp.path().join("package-a");
        write(&sandbox.join("黑骑士大剑/nativePC/models/player.mod3"));

        assert_eq!(
            resolve_content_root(&sandbox)
                .expect("resolve")
                .install_root(),
            Some(sandbox.join("黑骑士大剑").as_path())
        );
    }

    #[test]
    fn two_wrapper_directories_are_still_resolved() {
        let temp = tempfile::tempdir().expect("temp dir");
        let sandbox = temp.path().join("package-a");
        write(&sandbox.join("outer/inner/nativePC/models/player.mod3"));

        assert_eq!(
            resolve_content_root(&sandbox)
                .expect("resolve")
                .install_root(),
            Some(sandbox.join("outer/inner").as_path())
        );
    }

    #[test]
    fn native_pc_discovery_is_case_insensitive() {
        let temp = tempfile::tempdir().expect("temp dir");
        let sandbox = temp.path().join("package-a");
        write(&sandbox.join("wrapper/NATIVEpc/models/player.mod3"));

        assert_eq!(
            resolve_content_root(&sandbox)
                .expect("resolve")
                .install_root(),
            Some(sandbox.join("wrapper").as_path())
        );
    }

    #[test]
    fn several_native_pc_directories_are_ambiguous_and_unusable_for_install() {
        let temp = tempfile::tempdir().expect("temp dir");
        let sandbox = temp.path().join("package-a");
        write(&sandbox.join("mod-a/nativePC/models/a.mod3"));
        write(&sandbox.join("mod-b/nativePC/models/b.mod3"));

        let resolution = resolve_content_root(&sandbox).expect("resolve");
        assert!(matches!(resolution, ContentRootResolution::Ambiguous(_)));
        // 关键：合集包不能替用户挑一个，安装侧必须拿到 None。
        assert_eq!(resolution.install_root(), None);
    }

    #[test]
    fn a_package_without_native_pc_falls_back_to_the_sandbox_root() {
        let temp = tempfile::tempdir().expect("temp dir");
        let sandbox = temp.path().join("package-a");
        write(&sandbox.join("dinput8.dll"));

        assert_eq!(
            resolve_content_root(&sandbox)
                .expect("resolve")
                .install_root(),
            Some(sandbox.as_path())
        );
    }

    #[test]
    fn native_pc_below_the_depth_limit_is_not_discovered() {
        let temp = tempfile::tempdir().expect("temp dir");
        let sandbox = temp.path().join("package-a");
        // 比上限再多一层：应被深度约束挡住。
        let mut deep = sandbox.clone();
        for index in 0..=MAX_CONTENT_ROOT_SEARCH_DEPTH {
            deep = deep.join(format!("level-{index}"));
        }
        write(&deep.join("nativePC/models/player.mod3"));

        assert_eq!(
            resolve_content_root(&sandbox)
                .expect("resolve")
                .install_root(),
            Some(sandbox.as_path()),
            "超出深度上限的 nativePC 不应被当作内容根"
        );
    }
}
