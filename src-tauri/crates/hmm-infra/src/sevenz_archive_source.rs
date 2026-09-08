//! 7z 的格式适配器（T21-D，#348）。
//!
//! **门禁一条都不在这里**，全部委托给 `archive_extraction::ArchiveGate`。
//!
//! ## 它是「推驱动」的，与 rar / zip 不同
//!
//! `sevenz-rust2` 只提供 `for_each_entries(|entry, reader| …)`——`ArchiveReader`
//! 与 `BlockDecoder` 都只有回调版，没有任何 pull 接口（逐个查过）。
//! 也就是说**它要当驱动方**，而 B 定的 `ArchiveSource` 假定驱动方是外壳。
//!
//! 所以这里不实现 `ArchiveSource`，而是在别人的回调里调 `ArchiveGate::accept_entry`。
//! 门禁仍然是同一份实现——这正是 B 的目的；但如实说明：**这是外壳接口的一次扩充，
//! B 当初没预见到「格式自己要当驱动方」这种形态。**
//!
//! ## 与 rar 不同，7z 不需要落盘暂存
//!
//! `ArchiveReader::new(source, password)` 直接吃 `Read + Seek`，所以原始包保持只读、
//! 不多一份拷贝。unrar 那边要暂存是因为它的 DLL API 只认路径。

use crate::archive_extraction::{
    pump_reader_into_sink, ArchiveEntryHeader, ArchiveEntryKind, ArchiveGate,
};
use hmm_ports::{ModImportPrepareError, UnsupportedArchiveFeature};
use std::io::{Read, Seek};

/// 7-Zip 用 Windows 属性的 `0x8000` 位表示「高 16 位放的是 Unix mode」。
/// 这是 p7zip / 7-Zip 之间的既有约定，不是我们发明的。
const FILE_ATTRIBUTE_UNIX_EXTENSION: u32 = 0x8000;
/// Windows 重解析点（符号链接、目录联接）。
const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0400;
/// Unix 文件类型掩码与 symlink 值（`S_IFMT` / `S_IFLNK`）。
const UNIX_FILE_TYPE_MASK: u32 = 0xF000;
const UNIX_FILE_TYPE_SYMLINK: u32 = 0xA000;

/// 条目属性 → 外壳的条目类型。
///
/// **7z 能存 symlink**，而且是两种表达方式：Unix 侧把 mode 塞进属性高 16 位，
/// Windows 侧用重解析点位。两条都要认——只认一条等于对另一半平台的包不设防。
fn entry_kind(is_directory: bool, windows_attributes: u32) -> ArchiveEntryKind {
    if windows_attributes & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
        return ArchiveEntryKind::Symlink;
    }
    if windows_attributes & FILE_ATTRIBUTE_UNIX_EXTENSION != 0 {
        let unix_mode = windows_attributes >> 16;
        if unix_mode & UNIX_FILE_TYPE_MASK == UNIX_FILE_TYPE_SYMLINK {
            return ArchiveEntryKind::Symlink;
        }
    }
    if is_directory {
        ArchiveEntryKind::Directory
    } else {
        ArchiveEntryKind::File
    }
}

/// 打开 7z 并把全部条目过一遍门禁。
///
/// 返回 `Err(ModImportPrepareError)`：
/// - `UnsupportedArchiveFeature(Encrypted)`：加密包（含头加密）
/// - `Other`：打不开、损坏，或门禁拒绝
///
/// **打不开与门禁拒绝要由调用方区分**：前者应当继续往下嗅探，后者必须直接报。
/// 所以打开与解包分成两步，见 [`SevenZipArchive`]。
pub(crate) struct SevenZipArchive<R: Read + Seek> {
    reader: sevenz_rust2::ArchiveReader<R>,
}

impl<R: Read + Seek> SevenZipArchive<R> {
    /// 尝试打开。**不传密码**——加密包不做密码交互（设计的非目标），
    /// 让它以「要密码」的形态失败，正是我们要的明确档位。
    pub(crate) fn open(source: R) -> std::result::Result<Self, ModImportPrepareError> {
        match sevenz_rust2::ArchiveReader::new(source, sevenz_rust2::Password::empty()) {
            Ok(reader) => Ok(Self { reader }),
            Err(error) => Err(open_error_to_prepare_error(&error)),
        }
    }

    /// 归档里有没有用到 AES——即**内容加密**。
    ///
    /// 只看编解码链，**不解码任何数据**。之所以需要它：`ArchiveReader::new` 对
    /// 内容加密的包是**能打开的**（只有头加密才在 open 阶段失败），
    /// 要密码这件事要等真去解内容时才暴露。拖拽清单的预检不解内容，
    /// 光靠 open 就会把加密包显示成「可导入」，玩家确认之后才发现被骗
    /// ——这是 `probe_and_import_agree_on_every_fixture` 逼出来的。
    pub(crate) fn is_content_encrypted(&self) -> bool {
        self.reader.archive().blocks.iter().any(|block| {
            block.coders.iter().any(|coder| {
                coder.encoder_method_id() == sevenz_rust2::EncoderMethod::ID_AES256_SHA256
            })
        })
    }

    /// 内容层旁证：条目名里有没有本游戏的内容目录。
    ///
    /// 只读头部条目表，**不解码任何数据**——与 `is_content_encrypted` 同一个理由：
    /// 拖拽清单的预检不解内容。
    pub(crate) fn declares_game_content_root(&self) -> bool {
        self.reader.archive().files.iter().any(|entry| {
            crate::mod_import::entry_declares_game_content_root(entry.name(), entry.is_directory())
        })
    }

    /// 逐条目过门禁并落盘。
    pub(crate) fn extract_into(
        &mut self,
        gate: &mut ArchiveGate<'_>,
    ) -> std::result::Result<(), ModImportPrepareError> {
        // 7z 的条目总数从头部就能拿到——与 zip 一样能做写盘前的快速失败。
        let declared = self.reader.archive().files.len();
        if let Err(error) = gate.declare_entry_count(Some(declared)) {
            return Err(ModImportPrepareError::Other(error));
        }

        // 门禁的拒绝原因必须原样带出去。`for_each_entries` 的错误类型是 crate 自己的
        // `Error`，把 anyhow 塞进去会丢掉上下文（外壳那些精确文案正是靠它），
        // 所以单独存一份——与 rar 适配器同一个理由、同一个做法。
        let mut gate_failure: Option<anyhow::Error> = None;

        let outcome = self.reader.for_each_entries(|entry, reader| {
            let header = ArchiveEntryHeader {
                name: entry.name().to_owned(),
                kind: entry_kind(entry.is_directory(), entry.windows_attributes()),
                // 7z 头里就带解压大小。它同样**不可信**（#367 的教训与格式无关），
                // 只用于写盘前的快速失败；承重的是外壳按实际字节扣的配额。
                declared_size: Some(entry.size()),
            };
            match gate.accept_entry(&header, &mut |sink| pump_reader_into_sink(reader, sink)) {
                Ok(()) => Ok(true),
                Err(error) => {
                    gate_failure = Some(error);
                    // 返回 false 让它停下来。真正的原因在 `gate_failure` 里。
                    Ok(false)
                }
            }
        });

        if let Some(error) = gate_failure {
            return Err(ModImportPrepareError::Other(error));
        }
        match outcome {
            Ok(()) => Ok(()),
            Err(error) => Err(extract_error_to_prepare_error(&error)),
        }
    }
}

/// 打开失败 → 语义。
///
/// 只有**确凿**对应到某个档位的才映射，其余退回 `Other`（既有的 retry-hint）：
/// 宁可少说，也不要把「损坏」说成「加密」——那只是把误导换个方向。
fn open_error_to_prepare_error(error: &sevenz_rust2::Error) -> ModImportPrepareError {
    if is_password_error(error) {
        return ModImportPrepareError::UnsupportedArchiveFeature(
            UnsupportedArchiveFeature::Encrypted,
        );
    }
    ModImportPrepareError::Other(anyhow::anyhow!("failed to open 7z archive: {error}"))
}

fn extract_error_to_prepare_error(error: &sevenz_rust2::Error) -> ModImportPrepareError {
    if is_password_error(error) {
        return ModImportPrepareError::UnsupportedArchiveFeature(
            UnsupportedArchiveFeature::Encrypted,
        );
    }
    ModImportPrepareError::Other(anyhow::anyhow!("failed to extract 7z archive: {error}"))
}

/// 「这个包要密码」的判定。
///
/// 走 `Error` 的具名变体，不做字符串匹配——字符串会随上游改文案而静默失效，
/// 那正是查不出来的假绿。
fn is_password_error(error: &sevenz_rust2::Error) -> bool {
    matches!(
        error,
        sevenz_rust2::Error::PasswordRequired | sevenz_rust2::Error::MaybeBadPassword(_)
    )
}
