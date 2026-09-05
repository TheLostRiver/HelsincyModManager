//! 可执行 / 脚本类型的拒绝清单（#336 洞见 5，切片③）。
//!
//! 起因是真机实验 B（重定向前后对游戏目录做全量快照，逐文件比对 path + size + SHA-256）：
//! 「黑骑士特大」包内作者自带的 `MHWTexConverter_by_Jodo.exe`（30208 B）**原样落进了游戏
//! 目录**。HMM 切片② 让伴生文件进入重定向计划后，同一条路径在 HMM 这边也从「理论缺口」
//! 变成了「可触发」。
//!
//! 处置是丢弃而不是整包拒绝：作者把转换工具塞进 Mod 包是普遍习惯，包本身没有问题，
//! 因这个原因让整个 Mod 不可用会重演 #336 的病根（「不符合我的语法」被当成「包损坏」）。
//! 丢弃的数量记进 `ReplacementAdapterFacts::excluded_file_count` 与 Audit Log，
//! 不是无痕的静默丢弃。
//!
//! 边界说明：清单只作用在重定向计划产出的文件上。普通安装链路目前同样没有拒绝清单，
//! 那是独立的缺口，不在 #336 范围内。

/// 拒绝清单，取自 #336 正文洞见 5 的规范列表。
///
/// 全部是 Windows 上的可执行体或脚本宿主入口，且**没有一个**是 MHW 的游戏资源扩展名——
/// 游戏侧只会加载 `.mod3` `.mrl3` `.tex` `.dds` `.efx` `.epv3` `.epvsp` `.ctc` `.evwp` 这类。
/// 因此这条清单不会误伤任何正常的 Mod 资源。
///
/// 已知取舍：这是**枚举**而不是「所有可执行类型」的完备刻画，同类里的 `.hta` `.wsf`
/// `.vbe` `.jse` `.pif` 等不在其中。清单沿用 issue 的规范列表，扩充是维护者的口径决定；
/// 就本条路径的实际风险而言，`nativePC/wp/` 下的文件不在任何 Windows 或 MHW loader 的
/// 搜索路径上，拒绝清单属于纵深防御与目录卫生，不是唯一防线。
pub const MHW_EXECUTABLE_REJECT_EXTENSIONS: [&str; 12] = [
    "bat", "cmd", "com", "dll", "exe", "jar", "js", "lnk", "msi", "ps1", "scr", "vbs",
];

/// 文件名（**只能是路径的最后一段**）是否命中拒绝清单。
///
/// 两处归一化，都对应真实的 Windows 行为：
///
/// 1. **尾随点与空格**。Win32 在创建文件时会剥掉最后一段的尾随 `.` 和空格，于是
///    `x.exe.` 和 `x.exe ` 落到磁盘上都是 `x.exe`。只做 `ends_with(".exe")` 的检查
///    会被这两种写法直接绕过，所以先剥再取扩展名。
/// 2. **大小写**。NTFS 大小写不敏感，`X.EXE` 与 `x.exe` 是同一个文件。
pub fn is_rejected_executable_file_name(file_name: &str) -> bool {
    let normalized = file_name.trim_end_matches(['.', ' ']);
    let Some((_, extension)) = normalized.rsplit_once('.') else {
        return false;
    };
    let extension = extension.to_ascii_lowercase();
    MHW_EXECUTABLE_REJECT_EXTENSIONS.contains(&extension.as_str())
}
