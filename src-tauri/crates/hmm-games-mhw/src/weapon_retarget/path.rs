use super::family::{WeaponFamily, WeaponFamilyError, WeaponMainId, WeaponPartId};
use super::part_rename::{rename_weapon_stem, PartRename};
use hmm_core::InstallTargetPath;
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

    /// 从**完整资源路径**里截出它所属的槽位根，认不出就返回 `None`。
    ///
    /// `#349`：分类器需要给「解析成模型失败」的文件也定位槽位——例如源槽位目录里有个
    /// `arrow.mod3`，部件名认不出（`UnknownPart`），但它显然属于 `nativePC/wp/bow/bow017`。
    /// 定位到槽位之后它才能被记进那个单元的「无法判断如何改写」清单，而不是拖累整包。
    ///
    /// 注意 `WeaponModelAssetPath::parse` 里部件名的解析**在**槽位根之前，所以那条路径上
    /// 拿不到根；这里单独走一遍前 4 段。
    pub fn of_resource_path(value: &str) -> Option<Self> {
        let segments = value.split('/').collect::<Vec<_>>();
        if segments.len() < RESOURCE_ROOT_SEGMENT_COUNT {
            return None;
        }
        Self::parse(&segments[..RESOURCE_ROOT_SEGMENT_COUNT].join("/")).ok()
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

        let last = segments.len() - 1;
        segments[last] = match rename_weapon_stem(&segments[last], &self.main_id, target_main_id) {
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

        /*
         * 模型文件名走与伴生文件**完全相同**的改写函数（#343）。
         *
         * 上一版从 role 推导目标部件名，因此丢得掉信息：`saya035ol` 会落成 `saya019`
         * 而与同族的 `saya035` 互相覆盖；`ya013` 这类未登记前缀更是在识别阶段就被否决。
         * 现在只替换槽位数字，前缀与余部逐字保留，两个问题一起消失。
         */
        let renamed =
            match rename_weapon_stem(self.part_id.as_str(), self.root.main_id(), target_main_id) {
                PartRename::Renamed(renamed) => renamed,
                // 部件 ID 是由同一个 `split_weapon_stem` 从本路径解析出来的，因此这两档
                // 结构上不可达；保留分支是为了让「识别与改名共用一份实现」这条不变量
                // 一旦被破坏就立刻失败关闭，而不是产出一个错名字。
                PartRename::Unrelated | PartRename::Ambiguous => {
                    return Err(WeaponPathError::UnknownPart)
                }
            };
        let mut target_segments = self.segments.clone();
        target_segments[3] = target_main_id.as_str().to_owned();
        target_segments[5] = format!("{renamed}.{}", self.kind.extension());
        InstallTargetPath::parse(target_segments.join("/"), [NATIVE_PC_ROOT])
            .map_err(|_| WeaponPathError::UnsafePath)
    }
}

pub(crate) fn parse_safe_relative_path(value: &str) -> Result<InstallTargetPath, WeaponPathError> {
    // 通用安全校验在 `package_path`，两侧适配器共用；武器语法在本模块的各 parser 里。
    crate::package_path::parse_safe_package_path(value).map_err(|()| WeaponPathError::UnsafePath)
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
