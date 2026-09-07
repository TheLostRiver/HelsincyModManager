//! 编译 vendor 进来的 UnRAR 7.23 静态库。
//!
//! 文件清单与宏定义**逐条对应上游自己的构建配方**，不是猜的：
//! `vendor/unrar/makefile` 的 `lib:` 目标（`OBJECTS` ＋ `LIB_OBJ`，`WHAT=RARDLL`），
//! 以及 `vendor/unrar/UnRARDll.vcxproj` 的 `RARDLL;UNRAR;SILENT`。
//!
//! ## 为什么两个平台都要编
//!
//! CI 跑在 ubuntu-latest（`.github/workflows/verify.yml`），发版跑 windows-latest。
//! 若把这个 crate 限定成 `cfg(windows)`，CI 就永远编不到它、rar 的测试一条也不会跑
//! ——那是本仓库反复吃过亏的那种假绿：本机绿、CI 绿、而 CI 根本没执行这部分。
//! 所以宁可让 CI 多花几分钟编 48 个 C++ 文件。

use std::path::{Path, PathBuf};

/// `makefile` 的 `OBJECTS` ＋ `LIB_OBJ`（`lib:` 目标）。
///
/// **不是** vendor 目录里全部 85 个 `.cpp`：unrar 用「一个 .cpp include 另一个 .cpp」
/// 的组织方式（例如 `crypt.cpp` 会 include `crypt1/2/3/5.cpp`），
/// 单独编译被 include 的那些会重复定义。多编的那些还会把命令行工具、
/// 恢复卷、SFX 等我们用不到的代码拖进来。
const LIB_SOURCES: &[&str] = &[
    // OBJECTS
    "rar",
    "strlist",
    "strfn",
    "pathfn",
    "smallfn",
    "global",
    "file",
    "filefn",
    "filcreat",
    "archive",
    "arcread",
    "unicode",
    "system",
    "crypt",
    "crc",
    "rawread",
    "encname",
    "resource",
    "match",
    "timefn",
    "rdwrfn",
    "consio",
    "options",
    "errhnd",
    "rarvm",
    "secpassword",
    "rijndael",
    "getbits",
    "sha1",
    "sha256",
    "blake2s",
    "hash",
    "extinfo",
    "extract",
    "volume",
    "list",
    "find",
    "unpack",
    "headers",
    "threadpool",
    "rs16",
    "cmddata",
    "ui",
    "largepage",
    // LIB_OBJ
    "filestr",
    "scantree",
    "dll",
    "qopen",
];

/// Windows 独有的翻译单元，来自 `vendor/unrar/UnRARDll.vcxproj` 的 `ClCompile` 清单。
///
/// **它们不在 POSIX 的 `lib:` 目标里**，因为实现的是 Windows 专有行为：
/// `isnt` 是系统版本判定（`WinNT()` / `IsWindows11OrGreater()`），
/// `motw` 是 Mark-of-the-Web（`Zone.Identifier` 备用流），`rs` 是恢复卷用的
/// Reed-Solomon。少了它们，MSVC 链接期会报 `MarkOfTheWeb::` 与 `WinNT` 未解析
/// ——这不是配置问题，是 Windows 上真的需要这三个单元。
///
/// 上游清单里还有 `rarpch.cpp`，那是给 MSVC 预编译头用的桩（内容只有
/// `#include "rar.hpp"`）。我们不开 PCH，编它产不出任何符号，故不编。
#[cfg(target_os = "windows")]
const WINDOWS_ONLY_SOURCES: &[&str] = &["isnt", "motw", "rs"];

/// `UnRARDll.vcxproj` 没写 `AdditionalDependencies`——它吃的是 Visual Studio 对
/// DLL 工程的默认库集合。从 Rust 链接时没有那份默认集合，所以必须显式列。
///
/// 清单是**按链接错误逐条对出来的**，不是照着「常见系统库」抄的：
/// `advapi32` 提供 `Reg*`（`pathfn.cpp` 读注册表找 RAR 数据目录）、
/// `Crypt*`（`crypt.cpp` 的 `GetRnd`）、令牌与 `SetFileSecurityW`；
/// `shell32` 提供 `SHGetMalloc` / `SHGetSpecialFolderLocation`（`pathfn.cpp` 找 AppData）。
///
/// `Shlwapi` / `PowrProf` / `Psapi` 不在这里：`os.hpp` 用 `#pragma comment(lib, …)`
/// 把它们写进了 obj，MSVC 链接器会自己认。
#[cfg(target_os = "windows")]
const WINDOWS_SYSTEM_LIBRARIES: &[&str] = &["advapi32", "shell32"];

fn main() {
    let vendor = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("vendor")
        .join("unrar");

    let mut build = cc::Build::new();
    build.cpp(true).include(&vendor);

    // 第三方源码的告警不是我们能修的（改 vendor 就触发许可的「修改需在源码注释里
    // 附第 2 条全文」义务），所以关掉，免得淹掉我们自己代码的告警。
    build.warnings(false).extra_warnings(false);

    // `RARDLL` 选中 DLL 入口（dll.cpp）。`SILENT` 掐掉控制台输出——GUI 进程里
    // 第三方库往 stdout 打字是不可接受的。（`os.hpp:6` 其实会在 RARDLL 下自动
    // 补上 SILENT，这里显式写出来是为了让意图留在配方里，而不是藏在头文件里。）
    build.define("RARDLL", None);
    build.define("SILENT", None);
    // 上游 MSVC 工程带着它。全仓搜过：`UNRAR` 没有被任何 `#if` 用作开关，
    // 定义与否不改变产物；跟着上游写，是为了不制造「我们和上游配方不一样」的疑问。
    build.define("UNRAR", None);

    if build.get_compiler().is_like_msvc() {
        // MSVC 没有 `/std:c++11`，且 `os.hpp` 在 `_WIN_ALL` 下自己 `#define UNICODE`，
        // 不需要我们从命令行给。
        build.flag_if_supported("/EHsc");
        // 没有它，MSVC 在中文 Windows 上按 GBK 读源文件。`layout_probe.cpp` 的中文注释
        // 里有 UTF-8 三字节序列，按 GBK 解会**把行尾的换行吃掉**，于是注释后面那行
        // `#include "rar.hpp"` 被吞进注释里——症状是「结构体未声明」，与编码毫无相似之处，
        // 排查成本极高。vendor 的 48 个 .cpp 全是纯 ASCII，加这个开关对它们零影响。
        build.flag_if_supported("/utf-8");
    } else {
        build.std("c++11");
        // makefile 的 DEFINES。32 位平台上没有它们，大文件会被截成 2 GiB。
        build.define("_FILE_OFFSET_BITS", "64");
        build.define("_LARGEFILE_SOURCE", None);
        // `os.hpp:44` 在 Windows 下**无条件**定义 RAR_SMP（不看编译参数）。
        // 所以 Linux 也必须定义，否则 CI 验证的配置和 Windows 上跑的不是同一个
        // ——那正是「CI 全绿但没测到真实形态」的做法。makefile 本来也带着它。
        build.define("RAR_SMP", None);
        build.flag("-pthread");
    }

    let mut sources: Vec<&str> = LIB_SOURCES.to_vec();
    #[cfg(target_os = "windows")]
    sources.extend_from_slice(WINDOWS_ONLY_SOURCES);

    for stem in sources {
        let source = vendor.join(format!("{stem}.cpp"));
        assert!(
            source.is_file(),
            "vendored UnRAR source is missing: {}",
            source.display()
        );
        build.file(source);
    }

    // 布局探针：让 C++ 自己报 sizeof / offsetof，供 Rust 单测核对。
    // 它是本项目的文件（`src/`，不在 vendor 内），所以不触及 vendor 的原样保留约束。
    let probe = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join("layout_probe.cpp");
    println!("cargo:rerun-if-changed={}", probe.display());
    build.file(probe);

    build.compile("unrar");

    #[cfg(target_os = "windows")]
    for library in WINDOWS_SYSTEM_LIBRARIES {
        println!("cargo:rustc-link-lib=dylib={library}");
    }

    // 只对**参与编译**的文件重跑没用：被 include 进去的 .cpp 与全部 .hpp 一样是输入。
    // 少一条就会出现「改了 vendor 却没重编」。
    rerun_if_vendor_changed(&vendor);
}

fn rerun_if_vendor_changed(vendor: &Path) {
    println!("cargo:rerun-if-changed=build.rs");
    let Ok(entries) = std::fs::read_dir(vendor) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let is_source = path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| extension == "cpp" || extension == "hpp");
        if is_source {
            println!("cargo:rerun-if-changed={}", path.display());
        }
    }
}
