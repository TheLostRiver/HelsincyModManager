//! **仅测试用**的 RAR 语料生成器（store-only）。
//!
//! ## 为什么必须现场合成
//!
//! 两条硬约束叠在一起，别的路都堵死了：
//!
//! 1. `policy/project-policy.json` 的 `forbiddenFiles.extensions` 含 `.rar`
//!    ——二进制语料**提交不进仓库**。
//! 2. 开发机上没有任何 RAR 压缩器（WinRAR / `rar.exe` 都没有，7-Zip 也写不了 RAR）
//!    ——「先造一个再嵌成字节数组」这条路同样走不通。
//!
//! ## 为什么它不会造成假绿
//!
//! 生成器写错的话，**unrar 直接打不开，测试硬失败**。断言的形态是「unrar 成功解出
//! 预期字节」，而读取器不是我们写的——不存在「生成器与读取器一起错」的可能。
//! 这跟「自己写编码器再自己写解码器验证」是两回事。
//!
//! ## 只写 store，不写压缩
//!
//! 一是够用：门禁与判别都不关心熵编码。二是 UnRAR 许可明文禁止「重建 RAR 压缩算法」
//! ——store 不触及任何压缩算法，只是把字节原样放进容器。
//!
//! 字段布局逐条取自 vendor 源码而不是二手规格：`headers5.hpp` 的 `HFL_*` / `FHFL_*` /
//! `FHEXTRA_*`，`arcread.cpp:ReadHeader50` 的读取顺序，`rawread.cpp:GetV` 的 vint 编码，
//! `rawread.cpp:GetCRC50` 的 CRC 覆盖范围。

/// RAR5 签名（`Rar!\x1a\x07\x01\x00`）。
pub const RAR5_SIGNATURE: &[u8] = b"Rar!\x1a\x07\x01\x00";
/// RAR4 签名（`Rar!\x1a\x07\x00`）。两者前 7 字节相同，第 8 字节区分世代。
pub const RAR4_SIGNATURE: &[u8] = b"Rar!\x1a\x07\x00";

// headers.hpp:77 的 HEADER_TYPE。
const HEAD_MAIN: u64 = 0x01;
const HEAD_FILE: u64 = 0x02;
const HEAD_ENDARC: u64 = 0x05;

// headers5.hpp:10-18 的 HFL_*。
const HFL_EXTRA: u64 = 0x0001;
const HFL_DATA: u64 = 0x0002;

// headers5.hpp:25-27 的 MHFL_*。
const MHFL_VOLUME: u64 = 0x0001;
const MHFL_SOLID: u64 = 0x0004;

// headers5.hpp:32-35 的 FHFL_*。
const FHFL_DIRECTORY: u64 = 0x0001;
const FHFL_CRC32: u64 = 0x0004;
const FHFL_UNPUNKNOWN: u64 = 0x0008;

// headers5.hpp:82-86 的 FHEXTRA_*。
const FHEXTRA_CRYPT: u64 = 0x01;
const FHEXTRA_REDIR: u64 = 0x05;

/// `arcread.cpp:768` —— `Method=(CompInfo>>7)&7`，0 即 store；低 6 位是版本，0 = RAR 5.0。
const COMP_INFO_STORE_V50: u64 = 0;

/// `HOST5_WINDOWS` / `HOST5_UNIX`（headers.hpp:93）。
const HOST5_UNIX: u64 = 1;

/// unrar 在 `FHFL_UNPUNKNOWN` 时把 `UnpSize` 置成 `INT64NDF`
/// （`rartypes.hpp:30`，即 `0x7fffffff_7fffffff`），再由 `dll.cpp:287-288` 拆成
/// `UnpSize` / `UnpSizeHigh` 报出来。
///
/// **适配器必须把这个哨兵翻译成「未知」而不是「一个天文数字」**：否则声明值预检会
/// 直接以「超出单文件上限」拒掉，字节流配额根本轮不到运行。本常量导出，是为了让
/// 适配器与测试引用同一个来源，而不是各写一个魔数。
pub const UNRAR_UNKNOWN_UNPACKED_SIZE: u64 = 0x7fff_ffff_7fff_ffff;

/// 条目在归档里的形态。
#[derive(Debug, Clone)]
pub enum Rar5Body {
    /// 普通文件，内容原样存储。
    Stored(Vec<u8>),
    /// 目录条目（`FHFL_DIRECTORY`，无数据区）。
    Directory,
    /// 链接类条目：通过 `FHEXTRA_REDIR` 记录表达。
    /// `redirect_kind` 取 `FSREDIR_*`（`headers.hpp:110`）。
    Redirect { redirect_kind: u32, target: String },
}

#[derive(Debug, Clone)]
pub struct Rar5Entry {
    pub name: String,
    pub body: Rar5Body,
    /// 置 `FHFL_UNPUNKNOWN`：声明的解压大小「未知」。
    /// 这是 rar 表达「声明值不可信」的形态——store 结构上没法声明得比实际小
    /// （`extract.cpp:902` 的 `UnstoreFile` 以 `UnpSize` 为界），但可以声明不知道。
    pub unknown_unpacked_size: bool,
    /// 置 `FHEXTRA_CRYPT` 记录：让 unrar 认为该条目已加密。
    /// 记录内容是占位字节——我们不实现 RAR 加密，只需要 unrar 走到「要密码」这条路。
    pub encrypted: bool,
}

impl Rar5Entry {
    pub fn file(name: &str, data: impl Into<Vec<u8>>) -> Self {
        Self {
            name: name.to_owned(),
            body: Rar5Body::Stored(data.into()),
            unknown_unpacked_size: false,
            encrypted: false,
        }
    }

    pub fn directory(name: &str) -> Self {
        Self {
            name: name.to_owned(),
            body: Rar5Body::Directory,
            unknown_unpacked_size: false,
            encrypted: false,
        }
    }

    pub fn redirect(name: &str, redirect_kind: u32, target: &str) -> Self {
        Self {
            name: name.to_owned(),
            body: Rar5Body::Redirect {
                redirect_kind,
                target: target.to_owned(),
            },
            unknown_unpacked_size: false,
            encrypted: false,
        }
    }

    pub fn with_unknown_unpacked_size(mut self) -> Self {
        self.unknown_unpacked_size = true;
        self
    }

    pub fn with_encrypted_flag(mut self) -> Self {
        self.encrypted = true;
        self
    }
}

#[derive(Debug, Clone, Default)]
pub struct Rar5Archive {
    pub entries: Vec<Rar5Entry>,
    /// 追加在签名之前的任意字节，用来造自解压（SFX）形态。
    /// `archive.cpp` 的 `IsArchive` 会在前 `MAXSFXSIZE` 字节里找签名。
    pub sfx_prefix: Vec<u8>,
    /// 主头置 `MHFL_VOLUME`：分卷归档。
    pub volume: bool,
    /// 主头置 `MHFL_SOLID`：固实归档。
    pub solid: bool,
}

impl Rar5Archive {
    pub fn new(entries: Vec<Rar5Entry>) -> Self {
        Self {
            entries,
            ..Default::default()
        }
    }

    pub fn with_sfx_prefix(mut self, prefix: impl Into<Vec<u8>>) -> Self {
        self.sfx_prefix = prefix.into();
        self
    }

    pub fn as_volume(mut self) -> Self {
        self.volume = true;
        self
    }

    pub fn as_solid(mut self) -> Self {
        self.solid = true;
        self
    }

    pub fn build(&self) -> Vec<u8> {
        let mut out = self.sfx_prefix.clone();
        out.extend_from_slice(RAR5_SIGNATURE);

        let mut archive_flags = 0_u64;
        if self.volume {
            archive_flags |= MHFL_VOLUME;
        }
        if self.solid {
            archive_flags |= MHFL_SOLID;
        }
        let mut main_body = Vec::new();
        push_vint(&mut main_body, archive_flags);
        out.extend_from_slice(&build_block(HEAD_MAIN, 0, None, &main_body, &[]));

        for entry in &self.entries {
            out.extend_from_slice(&build_file_block(entry));
        }

        // 结束块：EndFlags = 0（最后一卷）。
        let mut end_body = Vec::new();
        push_vint(&mut end_body, 0);
        out.extend_from_slice(&build_block(HEAD_ENDARC, 0, None, &end_body, &[]));
        out
    }
}

fn build_file_block(entry: &Rar5Entry) -> Vec<u8> {
    let data: &[u8] = match &entry.body {
        Rar5Body::Stored(bytes) => bytes,
        Rar5Body::Directory | Rar5Body::Redirect { .. } => &[],
    };

    let mut file_flags = 0_u64;
    if matches!(entry.body, Rar5Body::Directory) {
        file_flags |= FHFL_DIRECTORY;
    }
    if entry.unknown_unpacked_size {
        file_flags |= FHFL_UNPUNKNOWN;
    } else {
        file_flags |= FHFL_CRC32;
    }

    let mut body = Vec::new();
    push_vint(&mut body, file_flags);
    // UnpSize：`FHFL_UNPUNKNOWN` 时 unrar 忽略它，写 0 即可。
    push_vint(
        &mut body,
        if entry.unknown_unpacked_size {
            0
        } else {
            data.len() as u64
        },
    );
    push_vint(&mut body, 0); // FileAttr
    if file_flags & FHFL_CRC32 != 0 {
        body.extend_from_slice(&crc32(data).to_le_bytes());
    }
    push_vint(&mut body, COMP_INFO_STORE_V50);
    push_vint(&mut body, HOST5_UNIX);
    let name = entry.name.as_bytes();
    push_vint(&mut body, name.len() as u64);
    body.extend_from_slice(name);

    let mut extra = Vec::new();
    if let Rar5Body::Redirect {
        redirect_kind,
        target,
    } = &entry.body
    {
        let mut record = Vec::new();
        push_vint(&mut record, FHEXTRA_REDIR);
        push_vint(&mut record, u64::from(*redirect_kind));
        push_vint(&mut record, 0); // RedirFlags：0 = 目标不是目录
        push_vint(&mut record, target.len() as u64);
        record.extend_from_slice(target.as_bytes());
        push_extra_record(&mut extra, &record);
    }
    if entry.encrypted {
        let mut record = Vec::new();
        push_vint(&mut record, FHEXTRA_CRYPT);
        push_vint(&mut record, 0); // CryptVersion
        push_vint(&mut record, 0); // EncFlags：不带密码校验数据
        record.push(0); // Lg2Count
        record.extend_from_slice(&[0_u8; 16]); // SIZE_SALT50
        record.extend_from_slice(&[0_u8; 8]); // IV
        push_extra_record(&mut extra, &record);
    }

    let data_size = if data.is_empty() {
        None
    } else {
        Some(data.len() as u64)
    };
    let mut block = build_block(HEAD_FILE, 0, data_size, &body, &extra);
    block.extend_from_slice(data);
    block
}

/// 一条 extra 记录的外层是「`Size`（vint）＋ 负载」，而 `Size` **不含它自己**、
/// 但**含记录内的类型字段**（`arcread.cpp:ProcessExtra50` 用
/// `NextPos = GetPos() + FieldSize` 定位下一条）。
fn push_extra_record(extra: &mut Vec<u8>, record: &[u8]) {
    push_vint(extra, record.len() as u64);
    extra.extend_from_slice(record);
}

/// 组一个 RAR5 块。
///
/// 布局（`arcread.cpp:ReadHeader50`）：
/// `CRC32(4) | BlockSize(vint) | HeaderType(vint) | Flags(vint) | [ExtraSize(vint)] |
///  [DataSize(vint)] | 类型专有字段 | extra 区`
///
/// `BlockSize` 是**从 `HeaderType` 起到 extra 区结束**的长度；
/// CRC32 覆盖的是**从 `BlockSize` 起到块尾**（`rawread.cpp:GetCRC50` 是
/// `CRC32(&Data[4], DataSize-4)`，跳过的正好是 CRC 自身那 4 字节）。
fn build_block(
    header_type: u64,
    extra_flags: u64,
    data_size: Option<u64>,
    body: &[u8],
    extra: &[u8],
) -> Vec<u8> {
    let mut flags = extra_flags;
    if !extra.is_empty() {
        flags |= HFL_EXTRA;
    }
    if data_size.is_some() {
        flags |= HFL_DATA;
    }

    let mut after_size = Vec::new();
    push_vint(&mut after_size, header_type);
    push_vint(&mut after_size, flags);
    if !extra.is_empty() {
        push_vint(&mut after_size, extra.len() as u64);
    }
    if let Some(size) = data_size {
        push_vint(&mut after_size, size);
    }
    after_size.extend_from_slice(body);
    after_size.extend_from_slice(extra);

    let mut crc_input = Vec::new();
    push_vint(&mut crc_input, after_size.len() as u64);
    crc_input.extend_from_slice(&after_size);

    let mut block = Vec::with_capacity(4 + crc_input.len());
    block.extend_from_slice(&crc32(&crc_input).to_le_bytes());
    block.extend_from_slice(&crc_input);
    block
}

/// RAR 的 vint：每字节 7 位有效，低位组在前，最高位为 1 表示后面还有
/// （`rawread.cpp:GetV`）。
fn push_vint(out: &mut Vec<u8>, mut value: u64) {
    loop {
        let byte = (value & 0x7f) as u8;
        value >>= 7;
        if value == 0 {
            out.push(byte);
            return;
        }
        out.push(byte | 0x80);
    }
}

/// 标准 CRC-32（IEEE，反射多项式 `0xEDB88320`）。
///
/// 自己写而不是引依赖：一个 `-sys` crate 不值得为测试语料多背一个 crate，
/// 而且它的正确性由 unrar 反向校验——算错了归档就打不开。
fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xffff_ffff_u32;
    for byte in data {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            let mask = 0_u32.wrapping_sub(crc & 1);
            crc = (crc >> 1) ^ (0xedb8_8320 & mask);
        }
    }
    !crc
}

#[cfg(test)]
mod tests {
    use super::*;

    /// CRC32 的独立锚点：`"123456789"` 的 CRC-32 是众所周知的 `0xCBF43926`。
    /// 不靠「unrar 能打开」间接证明——那样 CRC 与其他字段一起错时会互相掩盖。
    #[test]
    fn crc32_matches_the_standard_check_value() {
        assert_eq!(crc32(b"123456789"), 0xcbf4_3926);
    }

    /// vint 编码逐例锚点，含跨字节边界。
    #[test]
    fn vint_encoding_matches_the_rar_scheme() {
        for (value, expected) in [
            (0_u64, vec![0x00]),
            (1, vec![0x01]),
            (0x7f, vec![0x7f]),
            (0x80, vec![0x80, 0x01]),
            (0x3fff, vec![0xff, 0x7f]),
            (0x4000, vec![0x80, 0x80, 0x01]),
        ] {
            let mut out = Vec::new();
            push_vint(&mut out, value);
            assert_eq!(out, expected, "vint({value:#x})");
        }
    }

    #[test]
    fn a_built_archive_starts_with_the_rar5_signature() {
        let bytes = Rar5Archive::new(vec![Rar5Entry::file("a.txt", b"hi".to_vec())]).build();
        assert!(bytes.starts_with(RAR5_SIGNATURE));
    }

    /// 自解压形态：签名不在文件首字节。这条钉住的是「首字节不是 magic 推不出不是该格式」。
    #[test]
    fn an_sfx_prefix_pushes_the_signature_off_byte_zero() {
        let bytes = Rar5Archive::new(vec![Rar5Entry::file("a.txt", b"hi".to_vec())])
            .with_sfx_prefix(b"MZ\x90\x00stub".to_vec())
            .build();
        assert!(bytes.starts_with(b"MZ"));
        assert!(!bytes.starts_with(RAR5_SIGNATURE));
        assert!(bytes
            .windows(RAR5_SIGNATURE.len())
            .any(|window| window == RAR5_SIGNATURE));
    }
}
