//! 用 unrar 自己去读我们合成的语料——**这是语料生成器唯一的正确性凭据**。
//!
//! 断言的形态是「unrar 成功解出预期字节」。生成器若把任何一个字段写错，
//! unrar 直接打不开或解出别的东西，测试硬失败。读取器不是我们写的，
//! 所以不存在「编码器与解码器一起错、互相掩盖」那种自证循环。
//!
//! 顺带它也是 FFI 层的端到端冒烟：打开 → 逐条读头 → 回调收字节 → 关闭，
//! 整条路走通一次。C3 的适配器建立在这条路上。
//!
//! ## 为什么是单元测试模块而不是 `tests/`
//!
//! `fixture` 模块的门是 `#[cfg(any(test, feature = "test-fixtures"))]`。集成测试
//! （`tests/*.rs`）编译的是**不带 `cfg(test)`** 的那份 lib，于是没开 feature 时
//! 整个模块不存在——CI 跑的正是不带 feature 的 `cargo test --workspace`，
//! 结果就是编译失败（或者更糟：如果当初写成按 feature 跳过，就变成 CI 里**一条都不跑**
//! 的假绿）。放在 `src/` 下作单元测试模块，`cfg(test)` 天然成立，无需任何 feature。

use crate::fixture::{Rar5Archive, Rar5Entry, UNRAR_UNKNOWN_UNPACKED_SIZE};
use crate::*;
use std::ffi::{c_int, c_uint};

/// 读到的一个条目。
#[derive(Debug)]
struct ReadEntry {
    name: String,
    flags: c_uint,
    redirect_type: c_uint,
    unpacked_size: u64,
    data: Vec<u8>,
}

/// 回调收到的字节。裸指针要穿过 FFI，所以单独一个结构体而不是直接用 `Vec`。
struct Collector {
    bytes: Vec<u8>,
    /// 收满这么多字节就中止，用来走中止路径。`None` = 不中止。
    abort_after: Option<usize>,
}

/// **不得 unwind 出去**：panic 穿过 C++ 栈是未定义行为。这里只做一次
/// `extend_from_slice`，不做任何可能 panic 的判断。
unsafe extern "system" fn collect_callback(
    msg: c_uint,
    user_data: Lparam,
    p1: Lparam,
    p2: Lparam,
) -> c_int {
    if msg != UCM_PROCESSDATA {
        // 要密码、换卷等一律拒绝。返回 -1 即中止
        // （`rdwrfn.cpp:162`：只有 -1 中止，其他值继续）。
        return -1;
    }
    let collector = user_data as *mut Collector;
    if collector.is_null() || p1 == 0 || p2 <= 0 {
        return -1;
    }
    let chunk = std::slice::from_raw_parts(p1 as *const u8, p2 as usize);
    (*collector).bytes.extend_from_slice(chunk);
    match (*collector).abort_after {
        Some(limit) if (*collector).bytes.len() > limit => -1,
        _ => 1,
    }
}

/// 把语料写盘、用 unrar 读一遍。
///
/// 锁由 `Archive` 自己持有到 drop——测试不必（也无法）记得取锁。
fn read_back_with(bytes: &[u8], abort_after: Option<usize>) -> Result<Vec<ReadEntry>, c_int> {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("fixture.rar");
    std::fs::write(&path, bytes).expect("write fixture");

    let mut archive = Archive::open(&path, RAR_OM_EXTRACT)?;
    let mut collector = Box::new(Collector {
        bytes: Vec::new(),
        abort_after,
    });
    // SAFETY: `collector` 装在 Box 里，地址在整个会话期间稳定；回调不 unwind。
    unsafe {
        archive.set_callback(
            Some(collect_callback),
            collector.as_mut() as *mut Collector as Lparam,
        );
    }

    let mut entries = Vec::new();
    loop {
        let mut header = Box::new(RARHeaderDataEx::default());
        let read = archive.read_header(&mut header);
        if read == ERAR_END_ARCHIVE {
            return Ok(entries);
        }
        if read != ERAR_SUCCESS {
            return Err(read);
        }

        // packed 结构体：标量字段按值拷出来即可，数组字段取引用是硬编译错误
        // ——所以名字走 `file_name()` 访问器。
        let flags = header.Flags;
        let redirect_type = header.RedirType;
        let unpacked_size = header.unpacked_size();
        // **不做分隔符归一**——unrar 报的已经是宿主平台的原生形态（见
        // `unrar_emits_host_native_separators_and_sanitises_illegal_ones`），
        // 而 `Path::components()` 读的正是宿主原生形态，两边天然对齐。
        let name = header.file_name().expect("条目名不应被截断");

        collector.bytes.clear();
        let operation = if flags & RHDF_DIRECTORY != 0 {
            RAR_SKIP
        } else {
            RAR_TEST
        };
        let processed = archive.process(operation);
        if processed != ERAR_SUCCESS {
            return Err(processed);
        }

        entries.push(ReadEntry {
            name,
            flags,
            redirect_type,
            unpacked_size,
            data: std::mem::take(&mut collector.bytes),
        });
    }
}

/// 按**宿主平台的路径语义**把名字拆成组件再用 `/` 拼回来。
///
/// 断言用它而不是原始字符串，是因为原始字符串的分隔符随平台不同
/// （Windows 反斜杠、Linux 正斜杠），而**安全外壳消费的正是
/// `Path::components()`**——比较组件才是在比较真正会落盘的结构。
///
/// 这也顺带说明了为什么不做分隔符归一：两个平台的原始形态不同，
/// 但按各自平台解析出来的组件是同一个，天然就对齐了。
fn component_path(name: &str) -> String {
    std::path::Path::new(name)
        .components()
        .map(|component| component.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/")
}

fn read_back(bytes: &[u8]) -> Result<Vec<ReadEntry>, c_int> {
    read_back_with(bytes, None)
}

#[test]
fn unrar_reads_back_a_synthesized_store_only_archive() {
    let bytes = Rar5Archive::new(vec![
        Rar5Entry::file("readme.txt", b"hello rar".to_vec()),
        Rar5Entry::directory("nested"),
        Rar5Entry::file("nested/data.bin", vec![0xab; 5000]),
    ])
    .build();

    let entries = read_back(&bytes).expect("unrar must open the synthesized archive");
    assert_eq!(entries.len(), 3, "条目数");

    assert_eq!(component_path(&entries[0].name), "readme.txt");
    assert_eq!(entries[0].data, b"hello rar");
    assert_eq!(entries[0].unpacked_size, 9);

    assert_eq!(component_path(&entries[1].name), "nested");
    assert_ne!(
        entries[1].flags & RHDF_DIRECTORY,
        0,
        "目录条目必须带 RHDF_DIRECTORY"
    );

    // 5000 字节跨多次回调——推模式的分块形态在这里就成立了。
    assert_eq!(component_path(&entries[2].name), "nested/data.bin");
    assert_eq!(entries[2].data, vec![0xab; 5000]);
}

/// 非 ASCII 条目名要原样穿过 wchar_t 缓冲区。真实素材里中日文包名很常见。
#[test]
fn non_ascii_entry_names_survive_the_round_trip() {
    let bytes = Rar5Archive::new(vec![Rar5Entry::file(
        "素材/プレビュー.png",
        b"png".to_vec(),
    )])
    .build();
    let entries = read_back(&bytes).expect("open");
    assert_eq!(component_path(&entries[0].name), "素材/プレビュー.png");
}

/// unrar 报出的条目名已经是**宿主平台的原生形态**，而且非法字符会被就地消毒。
///
/// 落地 C3 时实测出来的，逐条记下来，因为它直接决定了「要不要做分隔符归一」：
///
/// | 归档里存的 | Windows 报出 | Linux 报出 |
/// | --- | --- | --- |
/// | 正斜杠分隔 | 转成**反斜杠**（宿主原生分隔符） | 原样 |
/// | 反斜杠分隔 | 反斜杠是 Windows 非法文件名字符，**消毒成下划线** | 原样保留 |
/// | `../escape.txt` | `..` 保留，分隔符换成反斜杠 | 原样 |
/// | 带盘符的绝对路径 | 冒号与反斜杠一并消毒成下划线 | 原样 |
///
/// **结论：不需要归一，也不应该归一。** `Path::components()` 读的就是宿主原生分隔符，
/// 与 unrar 的输出天然对齐；`..` 在两个平台上都会被识别成上级目录并被外壳拒绝。
/// 反过来，若强行把反斜杠换成正斜杠，Linux 上一个存着反斜杠的条目会被解成
/// **两层目录**，而 Windows 上它是一个下划线文件——那才是制造了平台差异。
///
/// 我第一版就是这么写错的（还在注释里把它说成安全问题）。留这条用例，
/// 是为了让「归一」这个念头下次冒出来时先看见真实数据。
#[test]
fn unrar_emits_host_native_separators_and_sanitises_illegal_ones() {
    let cases: &[(&str, &str, &str)] = &[
        ("dir/file.txt", r"dir\file.txt", "dir/file.txt"),
        (r"dir\file.txt", "dir_file.txt", r"dir\file.txt"),
        ("../escape.txt", r"..\escape.txt", "../escape.txt"),
        (
            r"C:\windows\evil.txt",
            "C__windows_evil.txt",
            r"C:\windows\evil.txt",
        ),
    ];
    for (stored, on_windows, elsewhere) in cases {
        let bytes = Rar5Archive::new(vec![Rar5Entry::file(stored, b"x".to_vec())]).build();
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("sep.rar");
        std::fs::write(&path, &bytes).expect("write");

        let mut archive = Archive::open(&path, RAR_OM_EXTRACT).expect("open");
        let mut header = Box::new(RARHeaderDataEx::default());
        assert_eq!(archive.read_header(&mut header), ERAR_SUCCESS);
        let raw = header.file_name().expect("name");

        let expected = if cfg!(windows) { on_windows } else { elsewhere };
        assert_eq!(&raw, expected, "stored={stored:?}");
    }
}

/// 自解压形态：签名不在首字节，unrar 照样能开。
///
/// **这条是「先开后嗅」全部论证的实证**：若按首字节预检，这个包会被判成
/// `MZ` 开头的可执行文件而拒掉，而它其实是能正常导入的。
#[test]
fn unrar_opens_an_sfx_archive_whose_signature_is_not_at_byte_zero() {
    let bytes = Rar5Archive::new(vec![Rar5Entry::file("a.txt", b"sfx".to_vec())])
        .with_sfx_prefix(b"MZ\x90\x00\x03\x00\x00\x00stub payload".to_vec())
        .build();
    assert!(bytes.starts_with(b"MZ"));
    let entries = read_back(&bytes).expect("unrar 必须能打开自解压形态");
    assert_eq!(entries[0].data, b"sfx");
}

/// 链接类条目经 `FHEXTRA_REDIR` 报出 `RedirType`。
///
/// 这是外壳拒绝 symlink / hardlink 的判据来源——若这里读不出来，
/// C3 的条目类型映射就是空的。
#[test]
fn redirect_entries_report_their_redirect_type() {
    for (kind, label) in [
        (FSREDIR_UNIXSYMLINK, "unix symlink"),
        (FSREDIR_WINSYMLINK, "windows symlink"),
        (FSREDIR_JUNCTION, "junction"),
        (FSREDIR_HARDLINK, "hard link"),
        (FSREDIR_FILECOPY, "file copy"),
    ] {
        let bytes = Rar5Archive::new(vec![Rar5Entry::redirect("link", kind, "../outside")]).build();
        let entries =
            read_back(&bytes).unwrap_or_else(|code| panic!("{label}: open failed {code}"));
        assert_eq!(entries.len(), 1, "{label}");
        assert_eq!(entries[0].redirect_type, kind, "{label}: RedirType");
    }
}

/// `FHFL_UNPUNKNOWN` 时 unrar 报的是哨兵值 `INT64NDF`，不是 0、也不是真实长度。
///
/// **这条钉住的是一个适配器必须知道的魔数。** 不把它翻译成「未知」的话，
/// 声明值预检会拿 9.2e18 去比单文件上限，直接以「超出上限」拒掉——
/// 字节流配额根本轮不到运行，而包其实是好的。
#[test]
fn an_unknown_unpacked_size_surfaces_as_the_documented_sentinel() {
    let bytes = Rar5Archive::new(vec![
        Rar5Entry::file("streamed.bin", vec![7_u8; 32]).with_unknown_unpacked_size()
    ])
    .build();
    let entries = read_back(&bytes).expect("open");
    assert_eq!(
        entries[0].unpacked_size, UNRAR_UNKNOWN_UNPACKED_SIZE,
        "unrar 的「未知解压大小」哨兵变了——适配器里的映射要跟着改"
    );
}

/// 中止路径：回调返回 -1 时 `RARProcessFileW` 返回 `ERAR_UNKNOWN`（21）。
///
/// **这条记录的是一个会坑到 C3 的事实。** `RARX_USERBREAK` 不在
/// `dll.cpp:497` 的映射表里，落到 `default` 上，所以「我们主动中止」与
/// 「unrar 自己出了不认识的错」返回码完全相同。适配器因此**必须自己记住中止原因**，
/// 否则外壳那些精确的文案（「超出单文件上限」等）会被一句泛化错误盖掉。
#[test]
fn aborting_from_the_callback_is_indistinguishable_by_return_code() {
    let bytes = Rar5Archive::new(vec![Rar5Entry::file("a.txt", vec![1_u8; 200_000])]).build();
    let error = read_back_with(&bytes, Some(0)).expect_err("中止必须报错");
    assert_eq!(
        error, ERAR_UNKNOWN,
        "中止路径的返回码变了——C3 里「靠自己记住中止原因」的理由要重新核"
    );
}

/// 加密条目在没有密码时落到 `ERAR_MISSING_PASSWORD`，而不是卡住或静默产出空文件。
#[test]
fn an_encrypted_entry_reports_a_missing_password() {
    let bytes = Rar5Archive::new(vec![
        Rar5Entry::file("secret.txt", b"cipher".to_vec()).with_encrypted_flag()
    ])
    .build();
    let error = read_back(&bytes).expect_err("加密条目必须报错，不能当成正常条目");
    assert_eq!(error, ERAR_MISSING_PASSWORD);
}

/// 分卷归档在**打开时**就带 `ROADF_VOLUME`——不必等读到缺失的下一卷才发现。
#[test]
fn a_volume_archive_is_flagged_at_open_time() {
    let bytes = Rar5Archive::new(vec![Rar5Entry::file("a.txt", b"vol".to_vec())])
        .as_volume()
        .build();
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("vol.rar");
    std::fs::write(&path, &bytes).expect("write");
    let archive = Archive::open(&path, RAR_OM_EXTRACT).expect("open");
    assert_ne!(archive.flags() & ROADF_VOLUME, 0, "必须报出分卷标志");
}

/// 目录条目不带数据区，`RAR_SKIP` 之后能继续读下一条。
#[test]
fn directories_do_not_stall_the_header_walk() {
    let bytes = Rar5Archive::new(vec![
        Rar5Entry::directory("a"),
        Rar5Entry::directory("a/b"),
        Rar5Entry::file("a/b/c.txt", b"deep".to_vec()),
    ])
    .build();
    let entries = read_back(&bytes).expect("open");
    assert_eq!(entries.len(), 3);
    assert_eq!(component_path(&entries[2].name), "a/b/c.txt");
    assert_eq!(entries[2].data, b"deep");
}
