use super::family::{WeaponFamily, WeaponFamilyError, WeaponMainId, WeaponPartId};
use super::part_rename::{part_mappings, rename_part_prefix, PartRename};
use hmm_core::{InstallTargetPath, InstallTargetPathError};
use thiserror::Error;

const NATIVE_PC_ROOT: &str = "nativePC";
const RESOURCE_ROOT_SEGMENT_COUNT: usize = 4;
const MODEL_ASSET_SEGMENT_COUNT: usize = 6;

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum WeaponPathError {
    #[error("unsafe MHW weapon resource path")]
    UnsafePath,
    #[error("not an MHW weapon resource path")]
    NotWeaponPath,
    #[error("unknown MHW weapon family")]
    UnknownFamily,
    #[error("invalid MHW weapon main id")]
    InvalidMainId,
    #[error("unknown MHW weapon part")]
    UnknownPart,
    #[error("unsupported MHW weapon resource")]
    UnsupportedResource,
    #[error("MHW weapon target belongs to another family")]
    CrossFamilyTarget,
}

impl WeaponPathError {
    pub fn code(self) -> &'static str {
        match self {
            Self::UnsafePath => "weapon_unsafe_path",
            Self::NotWeaponPath => "weapon_source_not_found",
            Self::UnknownFamily => "weapon_unknown_family",
            Self::InvalidMainId => "weapon_invalid_main_id",
            Self::UnknownPart => "weapon_unknown_part",
            Self::UnsupportedResource => "weapon_unsupported_resource",
            Self::CrossFamilyTarget => "weapon_cross_family_target",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WeaponResourceRoot {
    normalized_path: InstallTargetPath,
    family: WeaponFamily,
    main_id: WeaponMainId,
}

impl WeaponResourceRoot {
    pub fn parse(value: &str) -> Result<Self, WeaponPathError> {
        let normalized_path = parse_safe_relative_path(value)?;
        let segments = normalized_path.as_str().split('/').collect::<Vec<_>>();

        if segments.first() != Some(&NATIVE_PC_ROOT) || segments.get(1) != Some(&"wp") {
            return Err(WeaponPathError::NotWeaponPath);
        }
        let family = segments
            .get(2)
            .ok_or(WeaponPathError::UnsupportedResource)
            .and_then(|segment| {
                WeaponFamily::parse(segment).map_err(|_| WeaponPathError::UnknownFamily)
            })?;
        if segments.len() != RESOURCE_ROOT_SEGMENT_COUNT {
            return Err(WeaponPathError::UnsupportedResource);
        }
        let main_id =
            WeaponMainId::parse_for_family(segments[3], family).map_err(map_family_error)?;

        Ok(Self {
            normalized_path,
            family,
            main_id,
        })
    }

    pub fn normalized_path(&self) -> &InstallTargetPath {
        &self.normalized_path
    }

    pub fn family(&self) -> WeaponFamily {
        self.family
    }

    pub fn main_id(&self) -> &WeaponMainId {
        &self.main_id
    }

    pub fn path_family(&self) -> &'static str {
        self.family.path_family()
    }

    /// 该路径是否落在本槽位目录**之内**（`nativePC/wp/<family>/<main_id>/…`）。
    ///
    /// 用于把包内文件分成「随行·需重定位」（在内）与「随行·原样」（在外但仍在
    /// `nativePC/wp/` 下，如作者自建贴图目录 `wp/swo/Tamonowo/`）。
    pub fn contains(&self, path: &InstallTargetPath) -> bool {
        let prefix = format!("{}/", self.normalized_path.as_str());
        path.as_str().starts_with(&prefix)
    }

    /// 把本槽位目录内的任意伴生文件重定位到目标槽位。
    ///
    /// 与 MRL3 引用改写共用 [`part_rename`] 的同一套规则：替换主 ID 段，再按部件 ID 前缀
    /// 改写文件名段。两处必须一致，否则改写后的引用会指向不存在的文件。
    ///
    /// `Ambiguous`（部件 ID 在文件名里出现多次）返回 `UnsupportedResource`——调用方应把
    /// 这一项按「原样保留在源路径」处置并计入告警，不要猜。
    pub fn relocate_within(
        &self,
        path: &InstallTargetPath,
        target_main_id: &WeaponMainId,
    ) -> Result<InstallTargetPath, WeaponPathError> {
        if target_main_id.family() != self.family {
            return Err(WeaponPathError::CrossFamilyTarget);
        }
        if !self.contains(path) {
            return Err(WeaponPathError::NotWeaponPath);
        }

        let mut segments = path
            .as_str()
            .split('/')
            .map(str::to_owned)
            .collect::<Vec<_>>();
        // 槽位段固定在索引 3（nativePC / wp / <family> / <main_id>）。
        segments[RESOURCE_ROOT_SEGMENT_COUNT - 1] = target_main_id.as_str().to_owned();

        let mappings = part_mappings(&self.main_id, target_main_id)
            .map_err(|_| WeaponPathError::CrossFamilyTarget)?;
        let last = segments.len() - 1;
        segments[last] = match rename_part_prefix(&segments[last], &mappings) {
            PartRename::Renamed(renamed) => renamed,
            PartRename::Unrelated => segments[last].clone(),
            PartRename::Ambiguous => return Err(WeaponPathError::UnsupportedResource),
        };

        InstallTargetPath::parse(segments.join("/"), [NATIVE_PC_ROOT])
            .map_err(|_| WeaponPathError::UnsafePath)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum WeaponModelAssetKind {
    Mod3,
    Mrl3,
}

impl WeaponModelAssetKind {
    pub fn extension(self) -> &'static str {
        match self {
            Self::Mod3 => "mod3",
            Self::Mrl3 => "mrl3",
        }
    }

    fn parse(value: &str) -> Result<Self, WeaponPathError> {
        match value {
            "mod3" => Ok(Self::Mod3),
            "mrl3" => Ok(Self::Mrl3),
            _ => Err(WeaponPathError::UnsupportedResource),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WeaponModelAssetPath {
    normalized_path: InstallTargetPath,
    segments: [String; MODEL_ASSET_SEGMENT_COUNT],
    root: WeaponResourceRoot,
    part_id: WeaponPartId,
    kind: WeaponModelAssetKind,
}

impl WeaponModelAssetPath {
    pub fn parse(value: &str) -> Result<Self, WeaponPathError> {
        let normalized_path = parse_safe_relative_path(value)?;
        let parts = normalized_path.as_str().split('/').collect::<Vec<_>>();
        if parts.first() != Some(&NATIVE_PC_ROOT) || parts.get(1) != Some(&"wp") {
            return Err(WeaponPathError::NotWeaponPath);
        }
        let family = parts
            .get(2)
            .ok_or(WeaponPathError::UnsupportedResource)
            .and_then(|segment| {
                WeaponFamily::parse(segment).map_err(|_| WeaponPathError::UnknownFamily)
            })?;
        /*
         * 先判形状再解析主 ID（#336 的 L2）。反过来的话，`nativePC/wp/swo/epv/x.epv3`
         * 这种压根不是模型资源的路径会先在主 ID 解析上失败，被报成
         * 「该武器资源编号不符合游戏规范，请重新下载该 Mod」——把用户引导去重下一个
         * 完好的 Mod。形状不对就该说「不是模型资源」，而不是「编号不合规」。
         */
        if parts.len() != MODEL_ASSET_SEGMENT_COUNT || parts[4] != "mod" {
            return Err(WeaponPathError::UnsupportedResource);
        }
        let main_id = parts
            .get(3)
            .ok_or(WeaponPathError::UnsupportedResource)
            .and_then(|segment| {
                WeaponMainId::parse_for_family(segment, family).map_err(map_family_error)
            })?;

        let (part_stem, extension) = parts[5]
            .rsplit_once('.')
            .ok_or(WeaponPathError::UnsupportedResource)?;
        let kind = WeaponModelAssetKind::parse(extension)?;
        let part_id = WeaponPartId::parse_for_main(part_stem, &main_id)
            .map_err(|_| WeaponPathError::UnknownPart)?;
        let root = WeaponResourceRoot::parse(&parts[..RESOURCE_ROOT_SEGMENT_COUNT].join("/"))?;
        let segments = parts
            .into_iter()
            .map(str::to_owned)
            .collect::<Vec<_>>()
            .try_into()
            .expect("weapon model asset segment count was checked");

        Ok(Self {
            normalized_path,
            segments,
            root,
            part_id,
            kind,
        })
    }

    pub fn normalized_path(&self) -> &InstallTargetPath {
        &self.normalized_path
    }

    pub fn root(&self) -> &WeaponResourceRoot {
        &self.root
    }

    pub fn part_id(&self) -> &WeaponPartId {
        &self.part_id
    }

    pub fn kind(&self) -> WeaponModelAssetKind {
        self.kind
    }

    pub fn retarget(
        &self,
        target_main_id: &WeaponMainId,
    ) -> Result<InstallTargetPath, WeaponPathError> {
        if target_main_id.family() != self.root.family() {
            return Err(WeaponPathError::CrossFamilyTarget);
        }

        let target_part = target_main_id
            .part_for_role(self.part_id.role())
            .map_err(|_| WeaponPathError::UnknownPart)?;
        let mut target_segments = self.segments.clone();
        target_segments[3] = target_main_id.as_str().to_owned();
        target_segments[5] = format!("{}.{}", target_part.as_str(), self.kind.extension());
        InstallTargetPath::parse(target_segments.join("/"), [NATIVE_PC_ROOT])
            .map_err(|_| WeaponPathError::UnsafePath)
    }
}

pub(crate) fn parse_safe_relative_path(value: &str) -> Result<InstallTargetPath, WeaponPathError> {
    if value.trim() != value {
        return Err(WeaponPathError::UnsafePath);
    }

    // The dynamic root only invokes generic relative-path validation. Public parsers still
    // enforce the exact nativePC/wp weapon grammar after this step.
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
        | InstallTargetPathError::InvalidSegment => WeaponPathError::UnsafePath,
    })
}

/// 真实 Mod 压缩包常在 `nativePC` 之外包一层作者自建目录
/// （`MyWeaponMod/nativePC/wp/two/001/mod/two001.mod3`），而武器语法要求首段
/// 即 `nativePC`。这里把外层目录剥离掉，让这类最常见的包形态能被识别。
///
/// **只能在已经通过 `parse_safe_relative_path` 校验的路径上调用**——先校验、
/// 后剥离。顺序一旦颠倒，`a/../../evil/nativePC/wp/...` 就能借剥离绕过父目录
/// 遍历检测。
///
/// 路径中不含 `nativePC` 段时返回 `None`，交由调用方按"与武器无关的文件"处理。
pub(crate) fn strip_leading_package_dirs(
    normalized: &InstallTargetPath,
) -> Option<InstallTargetPath> {
    let segments = normalized.as_str().split('/').collect::<Vec<_>>();
    let start = segments
        .iter()
        .position(|segment| *segment == NATIVE_PC_ROOT)?;
    if start == 0 {
        return Some(normalized.clone());
    }
    InstallTargetPath::parse(segments[start..].join("/"), [NATIVE_PC_ROOT]).ok()
}

fn map_family_error(error: WeaponFamilyError) -> WeaponPathError {
    match error {
        WeaponFamilyError::UnknownFamily => WeaponPathError::UnknownFamily,
        WeaponFamilyError::InvalidMainId | WeaponFamilyError::FamilyMismatch => {
            WeaponPathError::InvalidMainId
        }
        WeaponFamilyError::UnknownPart => WeaponPathError::UnknownPart,
    }
}
