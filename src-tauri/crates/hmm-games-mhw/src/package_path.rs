//! Mod 包内相对路径的通用整形，武器侧与防具侧共用。

use hmm_core::{InstallTargetPath, InstallTargetPathError};

pub(crate) const NATIVE_PC_ROOT: &str = "nativePC";

/// 包内相对路径的安全校验：父目录穿越、绝对路径、盘符前缀、空段/`.` 段、首尾空白。
///
/// 允许的 root 取路径自己的首段——此处**只**校验安全性，不校验游戏语法；后者由各适配器
/// 在 [`strip_leading_package_dirs`] 之后自行判定。两步顺序不可颠倒：先校验、后剥离，
/// 否则 `a/../../evil/nativePC/wp/...` 能借剥离绕过父目录遍历检测。
pub(crate) fn parse_safe_package_path(value: &str) -> Result<InstallTargetPath, ()> {
    if value.trim() != value {
        return Err(());
    }
    let root = value
        .replace('\\', "/")
        .split('/')
        .next()
        .unwrap_or_default()
        .to_owned();
    InstallTargetPath::parse(value, [root]).map_err(|error| match error {
        InstallTargetPathError::TargetRootNotAllowed { .. }
        | InstallTargetPathError::Empty
        | InstallTargetPathError::Absolute
        | InstallTargetPathError::ParentTraversal
        | InstallTargetPathError::WindowsDrivePrefix
        | InstallTargetPathError::InvalidSegment => (),
    })
}

/// 定位包内的 `nativePC` 段，剥掉它之前的一切，并把该段**规范化为 `nativePC`**。
///
/// # 为什么要按大小写不敏感定位（#345）
///
/// 真实 Mod 压缩包里 `nativepc` / `NativePC` 等写法很常见（作者手打的目录名）。Windows
/// 文件系统大小写不敏感，所以这类包**安装完全正常**——坏的只有重定向：整条链路上有八处
/// 按段做大小写敏感比较，第一处（适配器路由）就把整包判成「不是武器包」，最终报
/// 「该 Mod 不是当前可自动处理的单源外观包」，而真实原因是路径大小写。错误信息完全指不到
/// 原因，与 `#336` 的「请重新下载该 Mod」是同一类误导。
///
/// 修法是**在入口归一化一次**，而不是把那八处逐一放宽：
///
/// - 逐处放宽要改八个地方，漏一个就仍然失败，且失败方式更隐蔽（识别通过了但改写用错大小写）
/// - 归一化之后，下游继续按字节比较 `nativePC`，那是一条**内部不变量**而非到处放宽的比较
///
/// 顺带的好处：重定向产出的目标路径统一为规范大小写，不再继承包内的写法。
///
/// # 为什么还要剥掉外层目录
///
/// 真实压缩包常在 `nativePC` 之外包一层作者自建目录
/// （`MyWeaponMod/nativePC/wp/two/two001/mod/two001.mod3`），而资源语法要求首段即 `nativePC`。
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
        .position(|segment| segment.eq_ignore_ascii_case(NATIVE_PC_ROOT))?;

    /*
     * 只规范化根段本身，其余段逐字保留。
     *
     * 往下的段（`wp`、族名、槽位、文件名）**不能**跟着归一化：族名与部件 ID 的大小写是
     * 语法的一部分（`WeaponFamily::from_token` 只认小写），文件名的大小写更是要逐字带到
     * 目标路径上（真实包里有 `two003_BML.PNG` 这种混合大小写）。这里放宽的只有游戏根这一段。
     */
    let mut rebuilt = Vec::with_capacity(segments.len() - start);
    rebuilt.push(NATIVE_PC_ROOT);
    rebuilt.extend_from_slice(&segments[start + 1..]);

    InstallTargetPath::parse(rebuilt.join("/"), [NATIVE_PC_ROOT]).ok()
}

/// 这条包内路径是否落在游戏根之下（大小写不敏感），落在的话它后面紧跟的是哪一段。
///
/// 供适配器路由做粗筛：真正的语法校验在各自的分析器里。
pub(crate) fn segment_after_native_pc_root(path: &str) -> Option<String> {
    path.replace('\\', "/")
        .split('/')
        .skip_while(|segment| !segment.eq_ignore_ascii_case(NATIVE_PC_ROOT))
        .nth(1)
        .map(str::to_owned)
}
