//! rar 的格式适配器（T21-C，#348）。
//!
//! **只做三件事**：按顺序产出条目头、把字节推给 sink、把 unrar 的错误码翻译成语义。
//! 七条安全门禁一条都不在这里——它们在 `archive_extraction` 外壳里，对所有格式一视同仁。
//!
//! ## unrar 全程不碰文件系统
//!
//! `RAR_OM_EXTRACT` 打开 ＋ `RAR_TEST` 操作 ＋ `UCM_PROCESSDATA` 回调收字节。
//! 落盘位置一律由外壳决定。**这不是风格问题**：目录遍历与 symlink 绕过那一整类
//! CVE 都发生在 unrar 自己决定往哪写的时候，而它不再决定。
//!
//! ## 为什么要自己记住中止原因
//!
//! 回调返回 `-1` 中止后，`RARProcessFileW` 返回 `ERAR_UNKNOWN`——因为
//! `RARX_USERBREAK` 不在 `dll.cpp:497` 的映射表里，落到了 `default`。
//! 也就是说「我们主动中止」与「unrar 自己出了不认识的错」**返回码完全相同**。
//! 不自己记的话，外壳那些精确文案（「超出单文件上限」「已取消」）会被一句泛化错误盖掉。

use crate::archive_extraction::{
    ArchiveChunkSink, ArchiveEntryHeader, ArchiveEntryKind, ArchiveSource,
};
use anyhow::Result;
use hmm_ports::{ModImportPrepareError, UnsupportedArchiveFeature};
use hmm_unrar_sys as unrar;
use std::ffi::{c_int, c_uint};
use std::path::Path;

/// unrar 在「解压大小未知」时报的哨兵，见 `fixture::UNRAR_UNKNOWN_UNPACKED_SIZE`。
/// 必须翻译成「未知」而不是一个天文数字，否则声明值预检会把好包直接拒掉。
const UNKNOWN_UNPACKED_SIZE: u64 = 0x7fff_ffff_7fff_ffff;

/// 回调与适配器之间的共享状态。**地址必须稳定**（装在 `Box` 里），
/// 因为它的裸指针要作为 `UserData` 交给 C 侧。
struct CallbackState {
    /// 只在 `write_current_to` 期间非空。
    ///
    /// 这里的 `'static` 是**擦除过的**：sink 的真实生存期只覆盖一次
    /// `write_current_to`。之所以要擦，是因为本结构体的地址要作为 `UserData`
    /// 交给 C 侧，不能带生命周期参数。安全性由「指针只在那一次同步调用期间非空」
    /// 保证，见 `write_current_to` 里的 SAFETY。
    sink: Option<*mut (dyn ArchiveChunkSink + 'static)>,
    /// 我们主动中止时的真实原因。见模块文档。
    abort: Option<anyhow::Error>,
}

/// **不得 unwind 出 FFI 边界。** sink 的实现（`BudgetedSink`）不会 panic，
/// 但底层 `write_all` 之类仍有理论可能，所以整体包一层 `catch_unwind`：
/// 把 panic 变成一次干净的中止，而不是让它穿过 C++ 栈。
unsafe extern "system" fn rar_callback(
    msg: c_uint,
    user_data: unrar::Lparam,
    p1: unrar::Lparam,
    p2: unrar::Lparam,
) -> c_int {
    // 只处理数据块。要密码、换卷等一律返回 -1 中止——它们各自由错误码落到明确档位。
    if msg != unrar::UCM_PROCESSDATA {
        return -1;
    }
    let state = user_data as *mut CallbackState;
    if state.is_null() || p1 == 0 || p2 <= 0 {
        return -1;
    }
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let state = &mut *state;
        let Some(sink) = state.sink else {
            return -1;
        };
        let chunk = std::slice::from_raw_parts(p1 as *const u8, p2 as usize);
        match (*sink).write_chunk(chunk) {
            Ok(()) => 1,
            Err(error) => {
                state.abort = Some(error);
                -1
            }
        }
    }));
    result.unwrap_or(-1)
}

pub(crate) struct RarArchiveSource {
    archive: unrar::Archive,
    state: Box<CallbackState>,
    /// 已读出头、但还没 `process` 掉的条目。
    ///
    /// unrar 要求**每读一个头就必须 process 一次**才能前进到下一个头。外壳对目录
    /// 是「不调 `write_current_to` 直接 continue」的，所以这里要在读下一个头之前
    /// 补一次 `RAR_SKIP`，否则头指针原地不动 —— 表现为无限循环，不是报错。
    pending_entry: bool,
    /// 需要投影成语义档位的失败。见 `take_structured_failure`。
    structured_failure: Option<ModImportPrepareError>,
}

impl RarArchiveSource {
    /// 打开归档。**归档级不支持的特性在这里就拒掉**，不等到读条目。
    pub(crate) fn open(path: &Path) -> std::result::Result<Self, ModImportPrepareError> {
        let archive = match unrar::Archive::open(path, unrar::RAR_OM_EXTRACT) {
            Ok(archive) => archive,
            Err(code) => return Err(open_error_to_prepare_error(code)),
        };

        // 分卷：我们只拿到玩家选中的那一个文件，续卷不在视野内。
        // 打开时就报得出来，不必等解到一半才发现 —— 早拒的提示更准。
        if archive.flags() & unrar::ROADF_VOLUME != 0 {
            return Err(ModImportPrepareError::UnsupportedArchiveFeature(
                UnsupportedArchiveFeature::MultiVolume,
            ));
        }
        // 头加密的包在 open 阶段就会失败（走上面的分支），这里兜住理论上的漏网。
        if archive.flags() & unrar::ROADF_ENCHEADERS != 0 {
            return Err(ModImportPrepareError::UnsupportedArchiveFeature(
                UnsupportedArchiveFeature::Encrypted,
            ));
        }

        let mut source = Self {
            archive,
            state: Box::new(CallbackState {
                sink: None,
                abort: None,
            }),
            pending_entry: false,
            structured_failure: None,
        };
        let user_data = source.state.as_mut() as *mut CallbackState as unrar::Lparam;
        // SAFETY: `state` 在 `Box` 里，地址稳定且活到 `source` 被 drop；
        // 回调自身包了 `catch_unwind`，不会 unwind 出边界。
        unsafe {
            source.archive.set_callback(Some(rar_callback), user_data);
        }
        Ok(source)
    }

    /// 取走需要投影成语义档位的失败（加密包等）。
    ///
    /// **不走 `downcast`。** 外壳的接口是 `anyhow::Result`，语义没法从里面还原；
    /// 与其在外面猜，不如让适配器把它单独存着 —— 这与端口错误类型化是同一条原则。
    pub(crate) fn take_structured_failure(&mut self) -> Option<ModImportPrepareError> {
        self.structured_failure.take()
    }

    /// 把 unrar 的错误码翻成 `anyhow`，并在需要时记下语义档位。
    fn record_error(&mut self, code: c_int, context: &'static str) -> anyhow::Error {
        if matches!(
            code,
            unrar::ERAR_MISSING_PASSWORD | unrar::ERAR_BAD_PASSWORD
        ) {
            self.structured_failure = Some(ModImportPrepareError::UnsupportedArchiveFeature(
                UnsupportedArchiveFeature::Encrypted,
            ));
        }
        anyhow::anyhow!("{context}: unrar error {code}")
    }
}

/// 打开失败的错误码 → 语义。
///
/// 只有**确凿**对应到某个档位的码才映射；其余一律退回 `Other`（既有的
/// retry-hint）。宁可少说，也不要把「损坏」说成「加密」——那只是把误导换个方向。
fn open_error_to_prepare_error(code: c_int) -> ModImportPrepareError {
    match code {
        unrar::ERAR_MISSING_PASSWORD | unrar::ERAR_BAD_PASSWORD => {
            ModImportPrepareError::UnsupportedArchiveFeature(UnsupportedArchiveFeature::Encrypted)
        }
        other => ModImportPrepareError::Other(anyhow::anyhow!(
            "failed to open rar archive: unrar error {other}"
        )),
    }
}

/// `RedirType`（`headers.hpp:110`）→ 外壳的条目类型。
///
/// symlink / junction 都归到 `Symlink`：对外壳而言它们是同一件事（都要拒），
/// 而分得更细只会让拒绝文案多出没有意义的分支。
/// `FSREDIR_FILECOPY` 归到 `Other`：它是「引用归档内另一个文件」，
/// 我们既不展开也不落盘，拒掉即可。
fn redirect_to_entry_kind(redirect_type: c_uint) -> Option<ArchiveEntryKind> {
    match redirect_type {
        unrar::FSREDIR_NONE => None,
        unrar::FSREDIR_UNIXSYMLINK | unrar::FSREDIR_WINSYMLINK | unrar::FSREDIR_JUNCTION => {
            Some(ArchiveEntryKind::Symlink)
        }
        unrar::FSREDIR_HARDLINK => Some(ArchiveEntryKind::HardLink),
        _ => Some(ArchiveEntryKind::Other),
    }
}

impl ArchiveSource for RarArchiveSource {
    /// rar 拿不到可信的预取总数——头是顺序读的。
    /// 外壳会降级成「边读边数、超了立即中止」，这个降级是设计里显式接受的。
    fn declared_entry_count(&self) -> Option<usize> {
        None
    }

    fn next_entry(&mut self) -> Result<Option<ArchiveEntryHeader>> {
        if self.pending_entry {
            // 上一条没被消费（目录，或外壳提前 continue）。不 SKIP 的话头指针不动。
            let code = self.archive.process(unrar::RAR_SKIP);
            self.pending_entry = false;
            if code != unrar::ERAR_SUCCESS {
                return Err(self.record_error(code, "failed to skip rar archive entry"));
            }
        }

        let mut header = Box::new(unrar::RARHeaderDataEx::default());
        let code = self.archive.read_header(&mut header);
        if code == unrar::ERAR_END_ARCHIVE {
            return Ok(None);
        }
        if code != unrar::ERAR_SUCCESS {
            return Err(self.record_error(code, "failed to read rar archive entry header"));
        }
        self.pending_entry = true;

        // packed 结构体：标量按值拷出来，数组字段走访问器。
        let flags = header.Flags;
        let redirect_type = header.RedirType;
        let declared = header.unpacked_size();

        // 名字可能被静默截断（`dll.cpp:265` 的 `wcsncpyz`）。截断后的路径是错的数据
        // ——可能指向别处，也可能与另一个条目撞名。**拒绝，不将就。**
        //
        // **不做分隔符归一。** 实测（见 hmm-unrar-sys 的
        // `unrar_emits_host_native_separators_and_sanitises_illegal_ones`）：unrar 报出的
        // 已经是宿主平台的原生形态——Windows 上正斜杠被转成反斜杠，而字面反斜杠与冒号
        // 作为非法文件名字符被消毒成下划线；Linux 上一律原样。外壳用的
        // `Path::components()` 读的正是宿主原生分隔符，两边天然对齐，`..` 在两个平台上
        // 都会被识别成上级目录并拒绝。
        //
        // 反过来强行归一才会制造平台差异：Linux 上一个存着反斜杠的条目会被解成两层目录，
        // 而 Windows 上它是一个下划线文件。（第一版就是这么写错的。）
        let Some(name) = header.file_name() else {
            anyhow::bail!("unsafe archive path: rar entry name is too long");
        };

        // 条目级加密：`RARProcessFile` 会返回 ERAR_MISSING_PASSWORD，但读头阶段
        // 就能看出来，早一步给出准确档位。
        if flags & unrar::RHDF_ENCRYPTED != 0 {
            self.structured_failure = Some(ModImportPrepareError::UnsupportedArchiveFeature(
                UnsupportedArchiveFeature::Encrypted,
            ));
            anyhow::bail!("rar archive entry is encrypted");
        }
        // 跨卷条目：单个文件里拿不到它的另一半。
        if flags & (unrar::RHDF_SPLITBEFORE | unrar::RHDF_SPLITAFTER) != 0 {
            self.structured_failure = Some(ModImportPrepareError::UnsupportedArchiveFeature(
                UnsupportedArchiveFeature::MultiVolume,
            ));
            anyhow::bail!("rar archive entry spans multiple volumes");
        }

        let kind = match redirect_to_entry_kind(redirect_type) {
            Some(kind) => kind,
            None if flags & unrar::RHDF_DIRECTORY != 0 => ArchiveEntryKind::Directory,
            None => ArchiveEntryKind::File,
        };

        Ok(Some(ArchiveEntryHeader {
            name,
            kind,
            declared_size: (declared != UNKNOWN_UNPACKED_SIZE).then_some(declared),
        }))
    }

    fn write_current_to(&mut self, sink: &mut dyn ArchiveChunkSink) -> Result<()> {
        // SAFETY: 只擦生命周期，不改指针的其他任何部分。指针在下面**同一个函数体内**
        // 置空，而 `process` 是同步调用——回调只可能在它执行期间发生。也就是说
        // sink 的真实生存期完整覆盖了这个指针的全部有效期。
        let erased: *mut (dyn ArchiveChunkSink + 'static) =
            unsafe { std::mem::transmute(sink as *mut dyn ArchiveChunkSink) };
        self.state.sink = Some(erased);
        self.state.abort = None;
        // `RAR_TEST` 而不是 `RAR_EXTRACT`：前者不产生任何文件系统写入。
        let code = self.archive.process(unrar::RAR_TEST);
        self.state.sink = None;
        self.pending_entry = false;

        // 顺序要紧：先看我们自己的中止原因。unrar 对「被中止」报的是 ERAR_UNKNOWN，
        // 与「它自己出了不认识的错」无法区分，先看返回码就会把真实原因盖掉。
        if let Some(error) = self.state.abort.take() {
            return Err(error);
        }
        if code != unrar::ERAR_SUCCESS {
            return Err(self.record_error(code, "failed to extract rar archive entry"));
        }
        Ok(())
    }
}
