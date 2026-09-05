use hmm_core::InstallTargetPath;
use thiserror::Error;

use super::slot_rename::{retarget_within_slot, slot_token};
use crate::package_path::{parse_safe_package_path, strip_leading_package_dirs, NATIVE_PC_ROOT};

/// `nativePC/pl/<equip>/<slot>` —— 槽位目录本身的段数。
const ARMOR_SLOT_ROOT_SEGMENT_COUNT: usize = 4;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ArmorPathError {
    #[error("not an MHW armor resource path")]
    NotArmorPath,
    /// 保留但不再产生（#342）：旧实现要求槽位之后必须是 `arm/mod` 且恰好 7 段，
    /// 真实套装的 `body` `helm` `leg` `wst` 与作者自建子目录全部撞这一条并否决整包。
    /// 部位与目录深度现在不再参与判定，错误码与前端文案保留，存量日志仍可解析。
    #[error("malformed MHW armor resource path")]
    MalformedArmorPath,
    #[error("invalid MHW armor slot")]
    InvalidSlot,
    #[error("unsafe MHW armor resource path")]
    UnsafePath,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum ArmorEquipFamily {
    Female,
    Male,
}

impl ArmorEquipFamily {
    fn from_segment(segment: &str) -> Option<Self> {
        match segment {
            "f_equip" => Some(Self::Female),
            "m_equip" => Some(Self::Male),
            _ => None,
        }
    }

    pub(super) fn path_family(self) -> &'static str {
        match self {
            Self::Female => "pl/f_equip",
            Self::Male => "pl/m_equip",
        }
    }

    /// 只有女性装备有 catalog 目标。这是 **catalog 覆盖范围**的限制，不是路径语法限制——
    /// 分类器照常识别 `m_equip`，由上层按「没有可选目标」处理。
    pub(super) fn is_supported(self) -> bool {
        self == Self::Female
    }
}

/// 一条落在**某个槽位目录之内**的防具资源路径。
///
/// #342 起这里**不再约束部位段、目录深度和扩展名**。判据只有两条结构事实：
/// 前四段是 `nativePC/pl/<equip>/<slot>` 且 `<slot>` 符合槽位语法；槽位之后至少还有一段。
/// 之后是 `arm/mod/f_arm078_0000.mod3` 还是 `cloak/tex/custom/skin.whatever`，一视同仁。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArmorResourcePath {
    normalized_path: InstallTargetPath,
    family: ArmorEquipFamily,
    slot: String,
}

impl ArmorResourcePath {
    pub fn parse(value: &str) -> Result<Self, ArmorPathError> {
        match classify_armor_asset(value)? {
            ArmorAsset::InSlot(path) => Ok(path),
            ArmorAsset::SlotIndependent { .. } => Err(ArmorPathError::InvalidSlot),
            ArmorAsset::Unrelated => Err(ArmorPathError::NotArmorPath),
        }
    }

    pub fn normalized_path(&self) -> &InstallTargetPath {
        &self.normalized_path
    }

    pub fn slot(&self) -> &str {
        &self.slot
    }

    pub fn path_family(&self) -> &'static str {
        self.family.path_family()
    }

    pub fn is_supported(&self) -> bool {
        self.family.is_supported()
    }

    /// 把本文件改写到目标槽位：在整条相对路径上替换槽位编号段。
    ///
    /// 目录段与文件名段一次同时改完，规则见 [`super::slot_rename`]。
    pub fn retarget(&self, target_slot: &str) -> Result<InstallTargetPath, ArmorPathError> {
        if !is_valid_armor_slot(target_slot) {
            return Err(ArmorPathError::InvalidSlot);
        }
        let (source_token, target_token) = (
            slot_token(&self.slot).ok_or(ArmorPathError::InvalidSlot)?,
            slot_token(target_slot).ok_or(ArmorPathError::InvalidSlot)?,
        );
        retarget_within_slot(&self.normalized_path, source_token, target_token)
            .ok_or(ArmorPathError::UnsafePath)
    }
}

/// 包内一个文件相对于防具重定向的归属。
///
/// 三档没有「包坏了」这一档——#342 的病根正是把「不符合我的语法」当成「包损坏」。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum ArmorAsset {
    /// 在某个槽位目录之内：跟着走，并按编号段改名。
    InSlot(ArmorResourcePath),
    /// 在 `nativePC/pl/<equip>/` 下但不属于任何槽位目录（作者自建目录，
    /// 如 `mod_pl_rosedress/`）。真机实验 A 观测到它们**原样留在原地**——
    /// 它们被 MRL3 按原路径引用，搬走反而断链。
    ///
    /// 带上归一化后的路径：调用方不能再拿原始字符串重新解析，否则小写根与外层目录
    /// 会在第二次解析时又被打回原形（#345）。
    SlotIndependent {
        family: ArmorEquipFamily,
        normalized_path: InstallTargetPath,
    },
    /// 与防具重定向无关（readme、预览图、武器资源、音效……）。忽略。
    Unrelated,
}

/// 唯一的失败是 `UnsafePath`：路径穿越、绝对路径这类真实安全信号。
/// 其余一切都归档，不再有「形态不对所以整包拒绝」。
pub(super) fn classify_armor_asset(value: &str) -> Result<ArmorAsset, ArmorPathError> {
    /*
     * 先做通用安全校验，再定位游戏根（#345）。两步都改自「直接要求首段等于 `nativePC`」：
     *
     * - 游戏根按**大小写不敏感**定位并归一化。真实包里 `nativepc` / `NativePC` 很常见，
     *   而它们在 Windows 上与 `nativePC` 是同一个目录；旧写法让这类包整包不可重定向。
     * - 顺带支持作者在 `nativePC` 外再包一层目录（`MyArmorMod/nativePC/pl/...`）。武器侧
     *   一直支持，防具侧此前会把这类包整包忽略——同一个入口两种行为没有道理。
     */
    let safe_path = parse_safe_package_path(value).map_err(|()| ArmorPathError::UnsafePath)?;
    let Some(normalized_path) = strip_leading_package_dirs(&safe_path) else {
        return Ok(ArmorAsset::Unrelated);
    };

    let parts = normalized_path.as_str().split('/').collect::<Vec<_>>();
    let Some(family) = (parts.first() == Some(&NATIVE_PC_ROOT) && parts.get(1) == Some(&"pl"))
        .then(|| parts.get(2).and_then(|s| ArmorEquipFamily::from_segment(s)))
        .flatten()
    else {
        return Ok(ArmorAsset::Unrelated);
    };

    // 槽位目录之内，且槽位之后至少还有一段（否则它是目录本身，不是文件）。
    let in_slot = parts.len() > ARMOR_SLOT_ROOT_SEGMENT_COUNT
        && parts
            .get(ARMOR_SLOT_ROOT_SEGMENT_COUNT - 1)
            .is_some_and(|slot| is_valid_armor_slot(slot));
    if !in_slot {
        return Ok(ArmorAsset::SlotIndependent {
            family,
            normalized_path,
        });
    }

    let slot = parts[ARMOR_SLOT_ROOT_SEGMENT_COUNT - 1].to_owned();
    Ok(ArmorAsset::InSlot(ArmorResourcePath {
        normalized_path,
        family,
        slot,
    }))
}

pub(crate) fn is_valid_armor_slot(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() == 10
        && &bytes[..2] == b"pl"
        && bytes[2..5].iter().all(u8::is_ascii_digit)
        && bytes[5] == b'_'
        && bytes[6..].iter().all(u8::is_ascii_digit)
}
