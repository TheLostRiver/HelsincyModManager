//! vendor 进来的 UnRAR 7.23 的最小 FFI 面。
//!
//! **这一层只做声明，不做策略。** 解包的七条安全门禁在 `hmm-infra` 的
//! `archive_extraction` 外壳里，对所有格式一视同仁；这里连一条判断都不该有。
//!
//! ## 只声明 6 个函数
//!
//! 官方 `dll.def` 导出 12 个，我们用 6 个：
//! `RAROpenArchiveEx` / `RARCloseArchive` / `RARReadHeaderEx` / `RARProcessFileW` /
//! `RARSetCallback` / `RARGetDllVersion`。
//!
//! 两处与设计文档所列清单的偏差，都是缩小攻击面：
//!
//! - 用 `RARProcessFileW` 而不是 `RARProcessFile`：我们只用 `RAR_TEST`，两者的
//!   `DestPath` / `DestName` 都会被忽略，行为等价；W 变体不经过 ANSI 代码页转换。
//! - **不声明 `RARSetPassword`**：加密包不做密码交互（设计的非目标）。不设密码时
//!   unrar 自己会返回 `ERAR_MISSING_PASSWORD`，那正是我们要的「落到明确档位」。
//!   声明一个永不调用的函数只是多一块无人看管的表面。
//!
//! ## 结构体必须是 `packed`
//!
//! `dll.hpp` 顶部是 `#pragma pack(push, 1)`。x64 上这不是无关紧要的细节：
//! `RARHeaderDataEx::CmtBuf` 落在偏移 6188，不是 8 对齐的位置。少写 `packed`
//! 会让编译器把它挪到 6192，此后每个字段都错位，而且**没有任何编译期或运行期报错**。
//!
//! 所以布局不是「照着头文件抄一遍就算数」——`layout_probe.cpp` 让 C++ 自己报出
//! `sizeof` 与 `offsetof`，本文件底部的测试逐条核对。核对不上就红，不留给运行期。

#![allow(non_snake_case)]
#![allow(non_camel_case_types)]

use std::ffi::{c_int, c_uint, c_void};

/// `wchar_t`。**两个平台不一样宽**：Windows 2 字节（UTF-16），Linux 4 字节（UCS-4）。
/// 因此 `RARHeaderDataEx` 在两个平台上的大小根本不同——这也是布局必须由 C++
/// 自己报、不能手算的原因之一。
#[cfg(windows)]
pub type WcharT = u16;
#[cfg(not(windows))]
pub type WcharT = i32;

/// `dll.hpp` 里的 `LPARAM`：Windows 上是 `LONG_PTR`，`_UNIX` 分支里是 `long`。
/// 两者在 LP64 / LLP64 下都与指针同宽。
pub type Lparam = isize;

pub type HANDLE = *mut c_void;

// ---- 错误码（dll.hpp:6-22）----
pub const ERAR_SUCCESS: c_int = 0;
pub const ERAR_END_ARCHIVE: c_int = 10;
pub const ERAR_NO_MEMORY: c_int = 11;
pub const ERAR_BAD_DATA: c_int = 12;
pub const ERAR_BAD_ARCHIVE: c_int = 13;
pub const ERAR_UNKNOWN_FORMAT: c_int = 14;
pub const ERAR_EOPEN: c_int = 15;
pub const ERAR_ECREATE: c_int = 16;
pub const ERAR_ECLOSE: c_int = 17;
pub const ERAR_EREAD: c_int = 18;
pub const ERAR_EWRITE: c_int = 19;
pub const ERAR_SMALL_BUF: c_int = 20;
pub const ERAR_UNKNOWN: c_int = 21;
pub const ERAR_MISSING_PASSWORD: c_int = 22;
pub const ERAR_EREFERENCE: c_int = 23;
pub const ERAR_BAD_PASSWORD: c_int = 24;
pub const ERAR_LARGE_DICT: c_int = 25;

// ---- 打开模式（dll.hpp:24-26）----
pub const RAR_OM_LIST: c_uint = 0;
/// 解压模式。**必须用它**：`RAR_OM_LIST` 下 `RARProcessFile` 不会产出数据。
pub const RAR_OM_EXTRACT: c_uint = 1;
pub const RAR_OM_LIST_INCSPLIT: c_uint = 2;

// ---- 操作（dll.hpp:28-30）----
pub const RAR_SKIP: c_int = 0;
/// **只用它。** `RAR_TEST` 不产生任何文件系统写入，字节全部经 `UCM_PROCESSDATA`
/// 回调交给外壳。用 `RAR_EXTRACT` 会让 unrar 自己决定往哪写，外壳对 rar 就完全失效
/// ——那一整类目录遍历 / symlink 绕过 CVE 正是这么发生的。
pub const RAR_TEST: c_int = 1;
pub const RAR_EXTRACT: c_int = 2;

// ---- 条目头标志（dll.hpp:51-55）----
pub const RHDF_SPLITBEFORE: c_uint = 0x01;
pub const RHDF_SPLITAFTER: c_uint = 0x02;
pub const RHDF_ENCRYPTED: c_uint = 0x04;
pub const RHDF_SOLID: c_uint = 0x10;
pub const RHDF_DIRECTORY: c_uint = 0x20;

// ---- 归档标志（dll.hpp:133-141）----
pub const ROADF_VOLUME: c_uint = 0x0001;
pub const ROADF_SOLID: c_uint = 0x0008;
pub const ROADF_ENCHEADERS: c_uint = 0x0080;
pub const ROADF_FIRSTVOLUME: c_uint = 0x0100;

// ---- 回调消息（dll.hpp:165-168）----
pub const UCM_CHANGEVOLUME: c_uint = 0;
pub const UCM_PROCESSDATA: c_uint = 1;
pub const UCM_NEEDPASSWORD: c_uint = 2;
pub const UCM_CHANGEVOLUMEW: c_uint = 3;
pub const UCM_NEEDPASSWORDW: c_uint = 4;
pub const UCM_LARGEDICT: c_uint = 5;

// ---- 重定向类型（headers.hpp:110-111 的 FSREDIR_*）----
pub const FSREDIR_NONE: c_uint = 0;
pub const FSREDIR_UNIXSYMLINK: c_uint = 1;
pub const FSREDIR_WINSYMLINK: c_uint = 2;
pub const FSREDIR_JUNCTION: c_uint = 3;
pub const FSREDIR_HARDLINK: c_uint = 4;
pub const FSREDIR_FILECOPY: c_uint = 5;

/// `dll.hpp:131` 的 `UNRARCALLBACK`。
///
/// `extern "system"` 覆盖 `CALLBACK`/`PASCAL` 在两个平台上的展开：Windows x86 是
/// `__stdcall`、x64 只有一种约定，`_UNIX` 分支里这两个宏展开为空。
///
/// **回调不得 unwind 出 FFI 边界**——panic 穿过 C++ 栈是未定义行为。实现方必须
/// 把错误变成返回 `-1`（`RARX_USERBREAK`，即中止解压）。
pub type UnrarCallback = Option<
    unsafe extern "system" fn(msg: c_uint, user_data: Lparam, p1: Lparam, p2: Lparam) -> c_int,
>;

/// `dll.hpp:78-117`。字段顺序与名字逐条对应，`Reserved` 一并保留——它参与 `sizeof`，
/// 少写就会让 unrar 写出结构体尾部。
#[repr(C, packed)]
pub struct RARHeaderDataEx {
    pub ArcName: [u8; 1024],
    pub ArcNameW: [WcharT; 1024],
    pub FileName: [u8; 1024],
    pub FileNameW: [WcharT; 1024],
    pub Flags: c_uint,
    pub PackSize: c_uint,
    pub PackSizeHigh: c_uint,
    pub UnpSize: c_uint,
    pub UnpSizeHigh: c_uint,
    pub HostOS: c_uint,
    pub FileCRC: c_uint,
    pub FileTime: c_uint,
    pub UnpVer: c_uint,
    pub Method: c_uint,
    pub FileAttr: c_uint,
    pub CmtBuf: *mut u8,
    pub CmtBufSize: c_uint,
    pub CmtSize: c_uint,
    pub CmtState: c_uint,
    pub DictSize: c_uint,
    pub HashType: c_uint,
    pub Hash: [u8; 32],
    pub RedirType: c_uint,
    pub RedirName: *mut WcharT,
    pub RedirNameSize: c_uint,
    pub DirTarget: c_uint,
    pub MtimeLow: c_uint,
    pub MtimeHigh: c_uint,
    pub CtimeLow: c_uint,
    pub CtimeHigh: c_uint,
    pub AtimeLow: c_uint,
    pub AtimeHigh: c_uint,
    pub ArcNameEx: *mut WcharT,
    pub ArcNameExSize: c_uint,
    pub FileNameEx: *mut WcharT,
    pub FileNameExSize: c_uint,
    pub Reserved: [c_uint; 982],
}

impl Default for RARHeaderDataEx {
    /// 全零。C 侧要求调用方把可选的 out 指针（`CmtBuf` / `FileNameEx` / …）置空，
    /// 全零正好表达「都不要」。结构体里没有任何不接受零位模式的类型。
    fn default() -> Self {
        // SAFETY: 全部字段是整数、裸指针与它们的数组；空指针与零整数都是合法位模式。
        unsafe { std::mem::zeroed() }
    }
}

/// `dll.hpp:146-163`。
#[repr(C, packed)]
pub struct RAROpenArchiveDataEx {
    pub ArcName: *const u8,
    pub ArcNameW: *const WcharT,
    pub OpenMode: c_uint,
    pub OpenResult: c_uint,
    pub CmtBuf: *mut u8,
    pub CmtBufSize: c_uint,
    pub CmtSize: c_uint,
    pub CmtState: c_uint,
    pub Flags: c_uint,
    pub Callback: UnrarCallback,
    pub UserData: Lparam,
    pub OpFlags: c_uint,
    pub CmtBufW: *mut WcharT,
    pub MarkOfTheWeb: *mut WcharT,
    pub Reserved: [c_uint; 23],
}

impl Default for RAROpenArchiveDataEx {
    fn default() -> Self {
        // SAFETY: 同 `RARHeaderDataEx`——只有整数、裸指针与函数指针的 `Option`，
        // 后者的全零位模式就是 `None`。
        unsafe { std::mem::zeroed() }
    }
}

extern "system" {
    pub fn RAROpenArchiveEx(ArchiveData: *mut RAROpenArchiveDataEx) -> HANDLE;
    pub fn RARCloseArchive(hArcData: HANDLE) -> c_int;
    pub fn RARReadHeaderEx(hArcData: HANDLE, HeaderData: *mut RARHeaderDataEx) -> c_int;
    pub fn RARProcessFileW(
        hArcData: HANDLE,
        Operation: c_int,
        DestPath: *mut WcharT,
        DestName: *mut WcharT,
    ) -> c_int;
    pub fn RARSetCallback(hArcData: HANDLE, Callback: UnrarCallback, UserData: Lparam);
    pub fn RARGetDllVersion() -> c_int;
}

/// 把路径编成 unrar 要的宽字符串（NUL 结尾）。平台差异在这里收口，调用方不必知道
/// `wchar_t` 有多宽。
///
/// Windows 走 `encode_wide`，与操作系统内部表示逐字相同，不经任何代码页。
/// 其余平台按 UTF-8 解码后转 UCS-4；路径若不是合法 UTF-8 则返回 `None`
/// ——**宁可明确失败，也不猜编码**。
pub fn to_wide_path(path: &std::path::Path) -> Option<Vec<WcharT>> {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        let mut wide: Vec<WcharT> = path.as_os_str().encode_wide().collect();
        if wide.contains(&0) {
            return None;
        }
        wide.push(0);
        Some(wide)
    }
    #[cfg(not(windows))]
    {
        let text = path.to_str()?;
        if text.contains('\0') {
            return None;
        }
        let mut wide: Vec<WcharT> = text.chars().map(|value| value as WcharT).collect();
        wide.push(0);
        Some(wide)
    }
}

/// 把 unrar 填回来的定长宽字符缓冲区读成 `String`，读到第一个 NUL 为止。
///
/// 无法表示的码位一律用替换字符，**不报错**：条目名的合法性由外壳的路径安全判据
/// 负责，不是这一层的事。
pub fn wide_buffer_to_string(buffer: &[WcharT]) -> String {
    let length = buffer
        .iter()
        .position(|value| *value == 0)
        .unwrap_or(buffer.len());
    let filled = &buffer[..length];
    #[cfg(windows)]
    {
        String::from_utf16_lossy(filled)
    }
    #[cfg(not(windows))]
    {
        filled
            .iter()
            .map(|value| char::from_u32(*value as u32).unwrap_or(char::REPLACEMENT_CHARACTER))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // C++ 侧自报布局。参见 `src/layout_probe.cpp` 的注释。
    extern "C" {
        fn hmm_unrar_sizeof_header_data_ex() -> usize;
        fn hmm_unrar_sizeof_open_archive_data_ex() -> usize;
        fn hmm_unrar_sizeof_wchar() -> usize;
        fn hmm_unrar_offsetof_header_file_name_w() -> usize;
        fn hmm_unrar_offsetof_header_flags() -> usize;
        fn hmm_unrar_offsetof_header_unp_size() -> usize;
        fn hmm_unrar_offsetof_header_unp_size_high() -> usize;
        fn hmm_unrar_offsetof_header_cmt_buf() -> usize;
        fn hmm_unrar_offsetof_header_redir_type() -> usize;
        fn hmm_unrar_offsetof_header_file_attr() -> usize;
        fn hmm_unrar_offsetof_open_open_mode() -> usize;
        fn hmm_unrar_offsetof_open_open_result() -> usize;
        fn hmm_unrar_offsetof_open_flags() -> usize;
        fn hmm_unrar_offsetof_open_callback() -> usize;
        fn hmm_unrar_offsetof_open_user_data() -> usize;
        fn hmm_unrar_offsetof_open_op_flags() -> usize;
        fn hmm_unrar_offsetof_open_cmt_buf_w() -> usize;
        fn hmm_unrar_offsetof_open_mark_of_the_web() -> usize;
    }

    /// 取 packed 结构体字段的偏移。`offset_of!` 不需要构造实例，也不会像
    /// `&value.field` 那样在 packed 结构体上直接编译失败。
    macro_rules! offset {
        ($type:ty, $field:ident) => {
            std::mem::offset_of!($type, $field)
        };
    }

    #[test]
    fn wchar_width_matches_the_c_compiler() {
        // 这一条先跑：它若不成立，下面每一条偏移都会跟着错，而错因就是它。
        assert_eq!(
            std::mem::size_of::<WcharT>(),
            unsafe { hmm_unrar_sizeof_wchar() },
            "WcharT 与 C++ 的 wchar_t 不同宽——两个平台上它分别是 2 和 4 字节"
        );
    }

    /// 逐条比对并**把所有不一致一次报全**。
    ///
    /// 不用 `assert_eq!` 逐条断言，是因为那样会停在第一条：拿掉 `packed` 时先炸的是
    /// 总大小，于是「哪些字段错位了」这个真正有用的信息一条都看不到。布局出错时
    /// 人要的是全貌，不是第一处。
    fn assert_layout_matches(
        struct_name: &str,
        rust_size: usize,
        c_size: usize,
        fields: &[(&str, usize, usize)],
    ) {
        let mut mismatches = Vec::new();
        if rust_size != c_size {
            mismatches.push(format!("sizeof: rust={rust_size} c={c_size}"));
        }
        for (label, rust, c) in fields {
            if rust != c {
                mismatches.push(format!("{label}: rust={rust} c={c}"));
            }
        }
        assert!(
            mismatches.is_empty(),
            "{struct_name} 与 C 侧布局不一致（{} 处）：\n  {}",
            mismatches.len(),
            mismatches.join("\n  ")
        );
    }

    /// `RARHeaderDataEx` 的布局逐字段核对。**这是 packed 假设的唯一凭据。**
    ///
    /// 已反向验证：把 `#[repr(C, packed)]` 改成 `#[repr(C)]`，本条转红，报出
    /// `sizeof 10256≠10244`、`CmtBuf 6192≠6188`、`RedirType 6252≠6248`
    /// ——`CmtBuf` 正是 x64 上对齐填充插进去的位置。
    #[test]
    fn header_layout_matches_the_c_struct() {
        unsafe {
            assert_layout_matches(
                "RARHeaderDataEx",
                std::mem::size_of::<RARHeaderDataEx>(),
                hmm_unrar_sizeof_header_data_ex(),
                &[
                    (
                        "FileNameW",
                        offset!(RARHeaderDataEx, FileNameW),
                        hmm_unrar_offsetof_header_file_name_w(),
                    ),
                    (
                        "Flags",
                        offset!(RARHeaderDataEx, Flags),
                        hmm_unrar_offsetof_header_flags(),
                    ),
                    (
                        "UnpSize",
                        offset!(RARHeaderDataEx, UnpSize),
                        hmm_unrar_offsetof_header_unp_size(),
                    ),
                    (
                        "UnpSizeHigh",
                        offset!(RARHeaderDataEx, UnpSizeHigh),
                        hmm_unrar_offsetof_header_unp_size_high(),
                    ),
                    (
                        "FileAttr",
                        offset!(RARHeaderDataEx, FileAttr),
                        hmm_unrar_offsetof_header_file_attr(),
                    ),
                    // 这一条是 packed 与否的分水岭：x64 上差 4 字节。
                    (
                        "CmtBuf",
                        offset!(RARHeaderDataEx, CmtBuf),
                        hmm_unrar_offsetof_header_cmt_buf(),
                    ),
                    (
                        "RedirType",
                        offset!(RARHeaderDataEx, RedirType),
                        hmm_unrar_offsetof_header_redir_type(),
                    ),
                ],
            );
        }
    }

    /// 已反向验证：`RAROpenArchiveDataEx` 去掉 `packed` 后本条转红，报出
    /// `sizeof 184≠176`、`CmtBufW 72≠68`、`MarkOfTheWeb 80≠76`。
    #[test]
    fn open_archive_layout_matches_the_c_struct() {
        unsafe {
            assert_layout_matches(
                "RAROpenArchiveDataEx",
                std::mem::size_of::<RAROpenArchiveDataEx>(),
                hmm_unrar_sizeof_open_archive_data_ex(),
                &[
                    (
                        "OpenMode",
                        offset!(RAROpenArchiveDataEx, OpenMode),
                        hmm_unrar_offsetof_open_open_mode(),
                    ),
                    (
                        "OpenResult",
                        offset!(RAROpenArchiveDataEx, OpenResult),
                        hmm_unrar_offsetof_open_open_result(),
                    ),
                    (
                        "Flags",
                        offset!(RAROpenArchiveDataEx, Flags),
                        hmm_unrar_offsetof_open_flags(),
                    ),
                    (
                        "Callback",
                        offset!(RAROpenArchiveDataEx, Callback),
                        hmm_unrar_offsetof_open_callback(),
                    ),
                    (
                        "UserData",
                        offset!(RAROpenArchiveDataEx, UserData),
                        hmm_unrar_offsetof_open_user_data(),
                    ),
                    (
                        "OpFlags",
                        offset!(RAROpenArchiveDataEx, OpFlags),
                        hmm_unrar_offsetof_open_op_flags(),
                    ),
                    // 本结构体里 packed 与否唯一分道的字段：它前面的都天然对齐。
                    (
                        "CmtBufW",
                        offset!(RAROpenArchiveDataEx, CmtBufW),
                        hmm_unrar_offsetof_open_cmt_buf_w(),
                    ),
                    (
                        "MarkOfTheWeb",
                        offset!(RAROpenArchiveDataEx, MarkOfTheWeb),
                        hmm_unrar_offsetof_open_mark_of_the_web(),
                    ),
                ],
            );
        }
    }

    /// 库真的被链进来了，且 ABI 版本是我们按着写声明的那一版。
    #[test]
    fn the_vendored_library_links_and_reports_its_abi_version() {
        // `dll.hpp:35` 的 `RAR_DLL_VERSION`。变了就说明 vendor 升级动了 ABI，
        // 此时上面那些布局断言必须重新核对，而不是想当然照旧。
        assert_eq!(unsafe { RARGetDllVersion() }, 10);
    }

    /// 打不开的归档要走到 `OpenResult`，而不是崩掉。
    ///
    /// **这一条不能替代上面的布局断言。** 反向验证时把 `RAROpenArchiveDataEx` 的
    /// `packed` 去掉，本条依旧**通过**——因为 `OpenMode` / `OpenResult` 都在错位
    /// 发生点（`CmtBufW`）之前。也就是说功能性冒烟测试对这类布局错误是瞎的，
    /// 它只会在真正解压时以莫名其妙的方式发作。布局要单独钉。
    #[test]
    fn opening_a_missing_archive_reports_eopen_instead_of_crashing() {
        let missing = std::path::Path::new("hmm-unrar-sys-no-such-archive-9d3f1a.rar");
        let mut wide = to_wide_path(missing).expect("path must encode");
        let mut data = RAROpenArchiveDataEx {
            ArcNameW: wide.as_mut_ptr(),
            OpenMode: RAR_OM_EXTRACT,
            ..Default::default()
        };
        let handle = unsafe { RAROpenArchiveEx(&mut data) };
        let open_result = data.OpenResult;
        assert!(handle.is_null(), "缺失的归档不该返回句柄");
        assert_eq!(open_result as c_int, ERAR_EOPEN);
    }

    #[test]
    fn wide_round_trip_keeps_non_ascii_names() {
        // 条目名带非 ASCII 是常态（真实素材里中文、日文包名很多）。
        let original = "素材/プレビュー.png";
        let mut wide = to_wide_path(std::path::Path::new(original)).expect("encode");
        assert_eq!(wide.pop(), Some(0), "必须 NUL 结尾");
        assert_eq!(wide_buffer_to_string(&wide), original);
    }

    #[test]
    fn wide_buffer_stops_at_the_first_nul() {
        let mut buffer = [0 as WcharT; 8];
        buffer[0] = 'a' as WcharT;
        buffer[1] = 'b' as WcharT;
        // buffer[2] 是 0；后面的内容必须被忽略，而不是拼进来。
        buffer[3] = 'z' as WcharT;
        assert_eq!(wide_buffer_to_string(&buffer), "ab");
    }
}
