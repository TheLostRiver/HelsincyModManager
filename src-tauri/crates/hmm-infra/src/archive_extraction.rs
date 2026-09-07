//! 与格式无关的解包安全外壳（T21-B，#348）。
//!
//! 条目数上限、单文件上限、总解压上限、`..` 逃逸拒绝、条目类型拒绝、大小写重名冲突、
//! sandbox 约束——**这七条一律在这里施加**。格式实现只负责「按顺序产出条目」。
//!
//! 存在的唯一理由：这七条原本全长在 zip reader 里面，每加一个格式复制一遍，早晚有一份
//! 漏掉其中一条。外壳让那件事不可能发生。
//!
//! ## 接口为什么是推模式而不是 `Read`
//!
//! 因为 rar 的 C API 是**回调式**的：解压器每产出一块就回调一次，回调返回中止信号即停。
//! 若把接口做成 pull（`Read`），rar 适配器就只能先把整个条目缓进内存再对外提供
//! ——那正是本设计拒绝的形态（大条目内存放大）。zip 从 pull 转 push 是平凡的，
//! 反过来不是。**接口形状必须迁就更受限的那一方。**
//!
//! ## 两条允许「未知」
//!
//! 条目总数与声明大小都允许未知：zip 能从中央目录预取，rar 与 tar 必须顺序读头。
//! 已知时外壳做写盘前的快速失败；未知时降级为边读边数、超了立即中止。
//! 而且**即使已知也不可信**——#367 实测过声明值会说谎，承重的永远是实际字节。

use crate::controlled_fs::{create_new_regular_file, open_or_create_child_directory};
use anyhow::{Context, Result};
use cap_std::fs::Dir;
use hmm_ports::CancellationToken;
use std::collections::HashSet;
use std::path::{Component, Path, PathBuf};

const COPY_BUFFER_BYTES: usize = 64 * 1024;

#[derive(Debug, Clone, Copy)]
pub(crate) struct ArchiveExtractionLimits {
    pub max_entries: usize,
    pub max_single_file_bytes: u64,
    pub max_total_uncompressed_bytes: u64,
}

/// 条目类型。**除 `File` / `Directory` 外一律拒绝。**
///
/// zip 里 symlink 是靠 Unix 权限位表达的边缘特性；RAR5 与 tar 把 symlink / hardlink 做成
/// 一等条目类型，tar 还有字符/块设备与 FIFO。枚举必须能表达它们——否则新格式接进来时
/// 只能在各自的适配器里再写一遍判断，外壳就白抽了。
/// 下面四个变体目前没有**生产**格式会产出——zip 只表达 File / Directory / Symlink。
/// 抑制放在变体级而不是整个 enum 上，这样将来真有变体死掉时仍然会被 lint 抓到。
///
/// 它们现在就存在，是因为接口若不先容纳，新格式接进来时只能在各自适配器里再写一遍
/// 判断，外壳就白抽了。**T21-C 接入 rar 时 `Symlink` / `HardLink` 会有真实产出方，
/// 届时这两条抑制必须删掉**；`Device` / `Fifo` 等 tar 家族恢复。
/// 测试里那个「有形状的假格式」会构造全部变体并逐个断言被拒。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ArchiveEntryKind {
    File,
    Directory,
    Symlink,
    #[allow(dead_code)]
    HardLink,
    #[allow(dead_code)]
    Device,
    #[allow(dead_code)]
    Fifo,
    /// 认得出不是普通文件/目录，但说不上是哪一种。同样拒绝。
    #[allow(dead_code)]
    Other,
}

impl ArchiveEntryKind {
    /// symlink 那条文案与重构前逐字一致——既有测试断言它，重构不该顺手改判据文本。
    /// 其余几种沿用同一句式。
    fn rejection_message(self) -> Option<&'static str> {
        match self {
            Self::File | Self::Directory => None,
            Self::Symlink => Some("unsafe archive path: symlink entries are not allowed"),
            Self::HardLink => Some("unsafe archive path: hard link entries are not allowed"),
            Self::Device => Some("unsafe archive path: device entries are not allowed"),
            Self::Fifo => Some("unsafe archive path: fifo entries are not allowed"),
            Self::Other => Some("unsafe archive path: unsupported entry type"),
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ArchiveEntryHeader {
    pub name: String,
    pub kind: ArchiveEntryKind,
    /// 声明的解压大小。允许未知；即使已知也**不可信**（#367）。
    /// 只用于写盘前的快速失败，配额的承重判据是实际写入的字节。
    pub declared_size: Option<u64>,
}

/// 外壳提供给格式实现的写出口。返回 `Err` 表示中止——配额超限或已取消。
/// 实现**必须立即停止并把错误原样返回**，不得吞掉后继续解压。
pub(crate) trait ArchiveChunkSink {
    fn write_chunk(&mut self, chunk: &[u8]) -> Result<()>;
}

/// 格式实现只需提供这三件事。七条门禁一概不在这里。
pub(crate) trait ArchiveSource {
    /// 条目总数，允许未知。已知时外壳能在写第一个字节之前就拒。
    fn declared_entry_count(&self) -> Option<usize>;

    /// 推进到下一个条目头。`None` = 结束。
    fn next_entry(&mut self) -> Result<Option<ArchiveEntryHeader>>;

    /// 把**当前条目**的字节分块推给 `sink`。只有 `File` 会被调用。
    fn write_current_to(&mut self, sink: &mut dyn ArchiveChunkSink) -> Result<()>;
}

/// 施加全部七条门禁并落盘。
pub(crate) fn extract_archive(
    source: &mut dyn ArchiveSource,
    sandbox_root: &Dir,
    cancellation_token: &dyn CancellationToken,
    limits: ArchiveExtractionLimits,
) -> Result<()> {
    // ① 条目数：已知就先拒（写盘前快速失败），未知则由循环里的计数兜底。
    if let Some(declared) = source.declared_entry_count() {
        reject_too_many_entries(declared, limits.max_entries)?;
    }

    let mut seen_paths = HashSet::new();
    let mut entry_count = 0_usize;
    // 两个计数器各管一头：声明值用于写盘前的快速失败，实际写入量才是承重判据（#367）。
    let mut total_declared_bytes = 0_u64;
    let mut total_written_bytes = 0_u64;

    while let Some(header) = source.next_entry()? {
        ensure_not_cancelled(cancellation_token)?;

        // ① 续：总数未知时，边读边数。语义从「预检拒绝」降级为「中途中止」——
        // 这个降级是显式接受的，rar 与 tar 都拿不到可信的预取总数。
        entry_count += 1;
        reject_too_many_entries(entry_count, limits.max_entries)?;

        // ② 条目类型：只有普通文件与目录能过。
        if let Some(message) = header.kind.rejection_message() {
            anyhow::bail!("{message}");
        }

        // ③ 路径安全：`..` 逃逸、绝对路径、盘符、非法组件。
        let relative_path = safe_archive_entry_path(&header.name)?;
        // ④ 大小写重名冲突。
        reject_case_insensitive_collision(&mut seen_paths, &relative_path)?;

        if header.kind == ArchiveEntryKind::Directory {
            // ⑤ sandbox 约束：目录逐级在 cap-std 能力下创建，不走操作系统全路径。
            let _ = open_or_create_archive_directory(sandbox_root, &relative_path)?;
            continue;
        }

        // ⑥ 单文件与总量的**声明值**预检：诚实声明超大的包在写第一个字节前就被拒。
        if let Some(declared) = header.declared_size {
            if declared > limits.max_single_file_bytes {
                anyhow::bail!("unsafe archive: archive file size limit exceeded");
            }
            total_declared_bytes = total_declared_bytes.saturating_add(declared);
            if total_declared_bytes > limits.max_total_uncompressed_bytes {
                anyhow::bail!("unsafe archive: archive total size limit exceeded");
            }
        }

        let parent = relative_path.parent().unwrap_or_else(|| Path::new(""));
        let parent = open_or_create_archive_directory(sandbox_root, parent)?;
        let file_name = relative_path
            .file_name()
            .context("unsafe archive path: missing file name")?;
        let mut target_file =
            create_new_regular_file(&parent, file_name, "extracted archive file")?;

        // ⑦ 字节流配额：声明可以说谎（#367 实测），所以真正承重的是这里。
        // 判定放在**写之前**，会越线的那一块直接中止，一个字节都不越界。
        let mut sink = BudgetedSink {
            writer: &mut target_file,
            cancellation_token,
            written: 0,
            max_single_file_bytes: limits.max_single_file_bytes,
            remaining_total_bytes: limits
                .max_total_uncompressed_bytes
                .saturating_sub(total_written_bytes),
        };
        source
            .write_current_to(&mut sink)
            .context("failed to extract archive file")?;
        total_written_bytes = total_written_bytes.saturating_add(sink.written);
    }

    Ok(())
}

struct BudgetedSink<'a, W> {
    writer: &'a mut W,
    cancellation_token: &'a dyn CancellationToken,
    written: u64,
    max_single_file_bytes: u64,
    remaining_total_bytes: u64,
}

impl<W: std::io::Write> ArchiveChunkSink for BudgetedSink<'_, W> {
    fn write_chunk(&mut self, chunk: &[u8]) -> Result<()> {
        ensure_not_cancelled(self.cancellation_token)?;
        let next_total = self.written.saturating_add(chunk.len() as u64);
        if next_total > self.max_single_file_bytes {
            anyhow::bail!("unsafe archive: archive file size limit exceeded");
        }
        if next_total > self.remaining_total_bytes {
            anyhow::bail!("unsafe archive: archive total size limit exceeded");
        }
        self.writer.write_all(chunk)?;
        self.written = next_total;
        Ok(())
    }
}

/// 给 pull 型格式（zip）用的适配助手：把 `Read` 转成对 sink 的分块推送。
///
/// 注意这是**格式适配器的便利函数**，不是外壳的一部分——push 型格式（rar）不经过它。
pub(crate) fn pump_reader_into_sink<R: std::io::Read + ?Sized>(
    reader: &mut R,
    sink: &mut dyn ArchiveChunkSink,
) -> Result<()> {
    let mut buffer = [0_u8; COPY_BUFFER_BYTES];
    loop {
        let read = match reader.read(&mut buffer) {
            Ok(0) => break,
            Ok(read) => read,
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error.into()),
        };
        sink.write_chunk(&buffer[..read])?;
    }
    Ok(())
}

fn reject_too_many_entries(actual_entries: usize, max_entries: usize) -> Result<()> {
    if actual_entries > max_entries {
        anyhow::bail!("unsafe archive: archive entry limit exceeded");
    }
    Ok(())
}

fn ensure_not_cancelled(cancellation_token: &dyn CancellationToken) -> Result<()> {
    if cancellation_token.is_cancelled() {
        anyhow::bail!("mod import prepare cancelled");
    }
    Ok(())
}

pub(crate) fn open_or_create_archive_directory(root: &Dir, relative_path: &Path) -> Result<Dir> {
    let mut current = root
        .try_clone()
        .context("failed to clone archive sandbox directory handle")?;
    for component in relative_path.components() {
        let Component::Normal(name) = component else {
            anyhow::bail!("unsafe archive path component");
        };
        current = open_or_create_child_directory(&current, name, "archive sandbox directory")?;
    }
    Ok(current)
}

/// 归档内相对路径的安全化。拒绝绝对路径、盘符前缀、`..`，跳过 `.`。
///
/// **与重构前逐字一致**：这是安全判据，重构切片不改它的语义，也不改它的错误文本
/// （既有测试断言这些字符串）。
///
/// 路径分隔符归一化**不在这里**：zip 用 `/`，rar 历史上用 `\`，tar 用 `/`。
/// 那是格式特有的事，由各自的适配器在交给外壳之前处理干净——外壳只认一种形态。
pub(crate) fn safe_archive_entry_path(entry_name: &str) -> Result<PathBuf> {
    let path = Path::new(entry_name);
    let mut safe = PathBuf::new();

    for component in path.components() {
        match component {
            Component::Normal(value) => safe.push(value),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                anyhow::bail!("unsafe archive path: {entry_name}");
            }
        }
    }

    if safe.as_os_str().is_empty() {
        anyhow::bail!("unsafe archive path: {entry_name}");
    }

    Ok(safe)
}

/// Windows 大小写不敏感，`A.txt` 与 `a.txt` 会互相覆盖。归档里同时出现两者时拒绝整包，
/// 而不是让后者悄悄盖掉前者。**与重构前逐字一致。**
pub(crate) fn reject_case_insensitive_collision(
    seen_paths: &mut HashSet<String>,
    relative_path: &Path,
) -> Result<()> {
    let key = case_insensitive_path_key(relative_path);

    if !seen_paths.insert(key) {
        anyhow::bail!("unsafe archive path: case-insensitive path collision");
    }

    Ok(())
}

fn case_insensitive_path_key(relative_path: &Path) -> String {
    relative_path
        .components()
        .filter_map(|component| match component {
            Component::Normal(value) => Some(value.to_string_lossy().to_ascii_lowercase()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/")
}

/// 与格式无关的负测清单（T21-B 的完成定义本体）。
///
/// **同一组负测，任何格式接进来都跑同一份表。** 每个格式只负责「按自己的形态造出这个
/// 情形」，断言与期望文本一概共用——要为某个格式重写断言，就说明外壳没抽干净。
#[cfg(test)]
pub(crate) mod shared_negative_suite {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) enum NegativeCase {
        PathEscape,
        AbsolutePath,
        SymlinkEntry,
        CaseCollision,
        TooManyEntries,
        OversizedSingleFile,
        OversizedTotal,
        /// 声明大小说谎：声明很小、实际很大。承重的必须是实际字节（#367）。
        LyingDeclaredSize,
    }

    pub(crate) const ALL_CASES: &[NegativeCase] = &[
        NegativeCase::PathEscape,
        NegativeCase::AbsolutePath,
        NegativeCase::SymlinkEntry,
        NegativeCase::CaseCollision,
        NegativeCase::TooManyEntries,
        NegativeCase::OversizedSingleFile,
        NegativeCase::OversizedTotal,
        NegativeCase::LyingDeclaredSize,
    ];

    /// 期望出现在错误里的片段。共用，不按格式分叉。
    pub(crate) fn expected_message(case: NegativeCase) -> &'static str {
        match case {
            NegativeCase::PathEscape | NegativeCase::AbsolutePath => "unsafe archive path:",
            NegativeCase::SymlinkEntry => "symlink entries are not allowed",
            NegativeCase::CaseCollision => "case-insensitive path collision",
            NegativeCase::TooManyEntries => "archive entry limit exceeded",
            NegativeCase::OversizedSingleFile | NegativeCase::LyingDeclaredSize => {
                "archive file size limit exceeded"
            }
            NegativeCase::OversizedTotal => "archive total size limit exceeded",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::shared_negative_suite::*;
    use super::*;
    use hmm_ports::NeverCancelled;

    /// 刻意做成**与 zip 形状相反**的测试用格式。
    ///
    /// 它不是「故意不做任何检查的假格式」——那种只能证明外壳**会调用**检查。
    /// 这个把 rar 将会带来的每一个形状差异都提前摆出来：
    ///
    /// - 条目总数**事先不可知**（`declared_entry_count` 返回 `None`）
    /// - 声明大小**可以未知**，也**可以说谎**
    /// - 能产出 symlink / hardlink / device / fifo 条目
    /// - 字节是**推**给 sink 的，不是被 `Read` 拉走的
    ///
    /// 外壳的接口若偷偷假设了 zip，这个源接进来就会露馅。
    struct ShapedSource {
        entries: Vec<(ArchiveEntryHeader, Vec<u8>)>,
        cursor: usize,
    }

    impl ArchiveSource for ShapedSource {
        fn declared_entry_count(&self) -> Option<usize> {
            None // 与 zip 相反：拿不到预取总数
        }

        fn next_entry(&mut self) -> Result<Option<ArchiveEntryHeader>> {
            if self.cursor >= self.entries.len() {
                return Ok(None);
            }
            let header = self.entries[self.cursor].0.clone();
            self.cursor += 1;
            Ok(Some(header))
        }

        fn write_current_to(&mut self, sink: &mut dyn ArchiveChunkSink) -> Result<()> {
            let bytes = self.entries[self.cursor - 1].1.clone();
            // 分多块推，模拟解压器的回调形态。
            for chunk in bytes.chunks(7) {
                sink.write_chunk(chunk)?;
            }
            Ok(())
        }
    }

    type Entry = (ArchiveEntryHeader, Vec<u8>);

    fn file(name: &str, declared: Option<u64>, bytes: Vec<u8>) -> Entry {
        (
            ArchiveEntryHeader {
                name: name.to_owned(),
                kind: ArchiveEntryKind::File,
                declared_size: declared,
            },
            bytes,
        )
    }

    fn special(name: &str, kind: ArchiveEntryKind) -> Entry {
        (
            ArchiveEntryHeader {
                name: name.to_owned(),
                kind,
                declared_size: Some(0),
            },
            Vec::new(),
        )
    }

    fn directory(name: &str) -> Entry {
        (
            ArchiveEntryHeader {
                name: name.to_owned(),
                kind: ArchiveEntryKind::Directory,
                declared_size: None,
            },
            Vec::new(),
        )
    }

    fn limits(max_entries: usize, single: u64, total: u64) -> ArchiveExtractionLimits {
        ArchiveExtractionLimits {
            max_entries,
            max_single_file_bytes: single,
            max_total_uncompressed_bytes: total,
        }
    }

    fn run(entries: Vec<Entry>, limits: ArchiveExtractionLimits) -> Result<tempfile::TempDir> {
        let temp = tempfile::tempdir().expect("temp dir");
        let sandbox =
            Dir::open_ambient_dir(temp.path(), cap_std::ambient_authority()).expect("open sandbox");
        extract_archive(
            &mut ShapedSource { entries, cursor: 0 },
            &sandbox,
            &NeverCancelled,
            limits,
        )?;
        Ok(temp)
    }

    fn shaped_case(case: NegativeCase) -> (Vec<Entry>, ArchiveExtractionLimits) {
        let generous = limits(16, 1024, 4096);
        match case {
            NegativeCase::PathEscape => (
                vec![file("../escape.txt", Some(3), b"bad".into())],
                generous,
            ),
            NegativeCase::AbsolutePath => {
                (vec![file("/abs.txt", Some(3), b"bad".into())], generous)
            }
            NegativeCase::SymlinkEntry => {
                (vec![special("link", ArchiveEntryKind::Symlink)], generous)
            }
            NegativeCase::CaseCollision => (
                vec![
                    file("Same.txt", Some(1), b"a".into()),
                    file("same.txt", Some(1), b"b".into()),
                ],
                generous,
            ),
            NegativeCase::TooManyEntries => (
                (0..5)
                    .map(|i| file(&format!("f{i}.txt"), Some(1), b"x".into()))
                    .collect(),
                limits(3, 1024, 4096),
            ),
            NegativeCase::OversizedSingleFile => (
                vec![file("big.bin", Some(500), vec![0; 500])],
                limits(16, 64, 4096),
            ),
            NegativeCase::OversizedTotal => (
                vec![
                    file("a.bin", Some(40), vec![0; 40]),
                    file("b.bin", Some(40), vec![0; 40]),
                ],
                limits(16, 1024, 64),
            ),
            NegativeCase::LyingDeclaredSize => (
                // 声明 4 字节、实际 500 字节：预检放行，必须由字节流配额兜住。
                vec![file("liar.bin", Some(4), vec![0; 500])],
                limits(16, 64, 4096),
            ),
        }
    }

    /// **T21-B 的完成定义**：整组负测跑在一个与 zip 形状完全相反的格式上，
    /// 断言与期望文本一条都不为它重写。
    #[test]
    fn the_shell_enforces_every_gate_on_a_format_shaped_unlike_zip() {
        for case in ALL_CASES {
            let (entries, limits) = shaped_case(*case);
            let error = run(entries, limits)
                .err()
                .unwrap_or_else(|| panic!("{case:?} must be rejected"));
            let text = format!("{error:#}");
            assert!(
                text.contains(expected_message(*case)),
                "{case:?}: expected {:?}, got {text}",
                expected_message(*case)
            );
        }
    }

    /// 条目总数未知时，「预检拒绝」降级为「中途中止」——这个降级是显式接受的。
    #[test]
    fn an_unknown_entry_count_still_stops_once_the_limit_is_crossed() {
        let entries: Vec<_> = (0..10)
            .map(|i| file(&format!("f{i}.txt"), None, b"x".into()))
            .collect();
        let error = run(entries, limits(3, 1024, 4096)).expect_err("must stop mid-stream");
        assert!(format!("{error:#}").contains("archive entry limit exceeded"));
    }

    /// 声明大小全部未知时配额仍然生效——因为它挂在字节流上，不挂元数据。
    #[test]
    fn quotas_hold_when_every_declared_size_is_unknown() {
        let error = run(
            vec![file("unknown.bin", None, vec![0; 500])],
            limits(16, 64, 4096),
        )
        .expect_err("the byte budget must hold without any declared size");
        assert!(format!("{error:#}").contains("archive file size limit exceeded"));
    }

    /// 其余条目类型也必须被拒——枚举不是摆设。
    #[test]
    fn hard_links_devices_and_fifos_are_rejected_too() {
        for (kind, expected) in [
            (
                ArchiveEntryKind::HardLink,
                "hard link entries are not allowed",
            ),
            (ArchiveEntryKind::Device, "device entries are not allowed"),
            (ArchiveEntryKind::Fifo, "fifo entries are not allowed"),
            (ArchiveEntryKind::Other, "unsupported entry type"),
        ] {
            let error = run(vec![special("weird", kind)], limits(16, 1024, 4096))
                .err()
                .unwrap_or_else(|| panic!("{kind:?} must be rejected"));
            assert!(
                format!("{error:#}").contains(expected),
                "{kind:?}: expected {expected:?}, got {error:#}"
            );
        }
    }

    /// 正向：普通文件与目录照常落盘，内容逐字节不变。
    /// 断言等价性而不是「没报错」——只断言不报错的话，一个把内容丢光的实现也能过。
    #[test]
    fn ordinary_files_and_directories_still_extract_byte_for_byte() {
        let temp = run(
            vec![
                directory("nested"),
                file("nested/a.txt", None, b"hello world".into()),
            ],
            limits(16, 1024, 4096),
        )
        .expect("an ordinary package must extract");
        assert_eq!(
            std::fs::read(temp.path().join("nested").join("a.txt")).expect("read extracted"),
            b"hello world"
        );
    }
}
