//! Mod 包内相对路径的通用整形，武器侧与防具侧共用。

use hmm_core::InstallTargetPath;

pub(crate) const NATIVE_PC_ROOT: &str = "nativePC";

/// 真实 Mod 压缩包常在 `nativePC` 之外包一层作者自建目录
/// （`MyWeaponMod/nativePC/wp/two/two001/mod/two001.mod3`），而资源语法要求首段
/// 即 `nativePC`。这里把外层目录剥离掉，让这类最常见的包形态能被识别。
///
/// **只能在已经通过安全校验的路径上调用**——先校验、后剥离。顺序一旦颠倒，
/// `a/../../evil/nativePC/wp/...` 就能借剥离绕过父目录遍历检测。
///
/// 路径中不含 `nativePC` 段时返回 `None`，交由调用方按「与本适配器无关的文件」处理。
pub(crate) fn strip_leading_package_dirs(
    normalized: &InstallTargetPath,
) -> Option<InstallTargetPath> {
    let segments = normalized.as_str().split('/').collect::<Vec<_>>();
    let start = segments
        .iter()
        .position(|segment| *segment == NATIVE_PC_ROOT)?;
    if start == 0 {
        return Some(normalized.clone());
    }
    InstallTargetPath::parse(segments[start..].join("/"), [NATIVE_PC_ROOT]).ok()
}
