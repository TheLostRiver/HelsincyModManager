use super::path::{parse_safe_relative_path, strip_leading_package_dirs};
use super::{
    WeaponFamily, WeaponModelAssetKind, WeaponModelAssetPath, WeaponPartId, WeaponPartRole,
    WeaponPathError, WeaponResourceRoot,
};
use crate::{generate_mhw_equipment_stable_id, EquipmentCandidateTargetKind};
use hmm_core::{InstallTargetPath, PackageFileId, ReplacementSourceId};
use hmm_ports::ReplacementAsset;
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum WeaponAnalysisError {
    #[error("weapon package file id is invalid")]
    InvalidPackageFileId,
    #[error("weapon package contains a duplicate package file id")]
    DuplicatePackageFileId,
    #[error("weapon package contains an unsafe path")]
    UnsafePath,
    #[error("weapon package contains a duplicate asset path")]
    DuplicateAssetPath,
    #[error("weapon package contains a case-insensitive path collision")]
    CaseInsensitivePathCollision,
    #[error("weapon source was not found")]
    SourceNotFound,
    #[error("weapon package contains multiple source roots")]
    MultipleSourceRoots,
    #[error("weapon package contains mixed weapon families")]
    MixedFamily,
    #[error("weapon package contains an unknown family")]
    UnknownFamily,
    #[error("weapon package contains an invalid main id")]
    InvalidMainId,
    #[error("weapon package contains an unknown part")]
    UnknownPart,
    #[error("weapon package contains an incomplete MOD3/MRL3 pair")]
    IncompleteBinaryPair,
    /// 保留但不再产生：真实武器包几乎必然携带 readme、预览图等非武器文件，
    /// 以"混合包"为由拒绝会让绝大多数 Mod 不可用。杂项文件现在被忽略，
    /// 门禁下限收敛到 `SourceNotFound`（一件武器资源都没有）。
    /// 错误码与前端文案保留，存量 manifest 与日志中的历史记录仍可解析。
    #[error("weapon package contains mixed install payload")]
    MixedInstallPayload,
    #[error("weapon package contains an unsupported resource")]
    UnsupportedResource,
    #[error("weapon source identity is invalid")]
    IdentityInvalid,
}

impl WeaponAnalysisError {
    pub fn code(self) -> &'static str {
        match self {
            Self::InvalidPackageFileId => "weapon_invalid_package_file_id",
            Self::DuplicatePackageFileId => "weapon_duplicate_package_file_id",
            Self::UnsafePath => "weapon_unsafe_path",
            Self::DuplicateAssetPath => "weapon_duplicate_asset_path",
            Self::CaseInsensitivePathCollision => "weapon_case_insensitive_path_collision",
            Self::SourceNotFound => "weapon_source_not_found",
            Self::MultipleSourceRoots => "weapon_multiple_source_roots",
            Self::MixedFamily => "weapon_mixed_family",
            Self::UnknownFamily => "weapon_unknown_family",
            Self::InvalidMainId => "weapon_invalid_main_id",
            Self::UnknownPart => "weapon_unknown_part",
            Self::IncompleteBinaryPair => "weapon_incomplete_binary_pair",
            Self::MixedInstallPayload => "weapon_mixed_install_payload",
            Self::UnsupportedResource => "weapon_unsupported_resource",
            Self::IdentityInvalid => "weapon_identity_invalid",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WeaponAnalysisWarning {
    PartialPartSet,
}

impl WeaponAnalysisWarning {
    pub fn code(self) -> &'static str {
        match self {
            Self::PartialPartSet => "weapon_partial_part_set",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WeaponSourceAsset {
    package_file_id: PackageFileId,
    model_path: WeaponModelAssetPath,
}

impl WeaponSourceAsset {
    pub fn package_file_id(&self) -> &PackageFileId {
        &self.package_file_id
    }

    pub fn model_path(&self) -> &WeaponModelAssetPath {
        &self.model_path
    }

    pub fn relative_path(&self) -> &InstallTargetPath {
        self.model_path.normalized_path()
    }

    pub fn kind(&self) -> WeaponModelAssetKind {
        self.model_path.kind()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WeaponModelPair {
    part_id: WeaponPartId,
    mod3: WeaponSourceAsset,
    mrl3: WeaponSourceAsset,
}

impl WeaponModelPair {
    pub fn part_id(&self) -> &WeaponPartId {
        &self.part_id
    }

    pub fn mod3(&self) -> &WeaponSourceAsset {
        &self.mod3
    }

    pub fn mrl3(&self) -> &WeaponSourceAsset {
        &self.mrl3
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WeaponSourceClosure {
    source_id: ReplacementSourceId,
    root: WeaponResourceRoot,
    pairs: Vec<WeaponModelPair>,
    asset_count: usize,
    warnings: Vec<WeaponAnalysisWarning>,
}

impl WeaponSourceClosure {
    pub fn source_id(&self) -> &ReplacementSourceId {
        &self.source_id
    }

    pub fn root(&self) -> &WeaponResourceRoot {
        &self.root
    }

    pub fn family(&self) -> WeaponFamily {
        self.root.family()
    }

    pub fn pairs(&self) -> &[WeaponModelPair] {
        &self.pairs
    }

    pub fn asset_count(&self) -> usize {
        self.asset_count
    }

    pub fn warnings(&self) -> &[WeaponAnalysisWarning] {
        &self.warnings
    }
}

#[derive(Debug)]
struct PreparedAsset {
    package_file_id: PackageFileId,
    relative_path: InstallTargetPath,
}

#[derive(Default)]
struct PairBuilder {
    mod3: Option<WeaponSourceAsset>,
    mrl3: Option<WeaponSourceAsset>,
}

pub fn analyze_mhw_weapon_assets(
    assets: &[ReplacementAsset],
) -> Result<WeaponSourceClosure, WeaponAnalysisError> {
    if assets.is_empty() {
        return Err(WeaponAnalysisError::SourceNotFound);
    }

    let mut package_file_ids = BTreeSet::new();
    let mut prepared = Vec::with_capacity(assets.len());
    let mut path_keys = BTreeMap::<String, String>::new();
    let mut has_duplicate_path = false;
    let mut has_case_collision = false;

    for asset in assets {
        let package_file_id = asset.package_file_id();
        if package_file_id.as_str().trim().is_empty() {
            return Err(WeaponAnalysisError::InvalidPackageFileId);
        }
        if !package_file_ids.insert(package_file_id.as_str().to_owned()) {
            return Err(WeaponAnalysisError::DuplicatePackageFileId);
        }

        // 先做安全校验再剥离外层目录：顺序颠倒会让 `a/../../nativePC/...`
        // 借剥离绕过父目录遍历检测。
        let safe_path = parse_safe_relative_path(asset.relative_path())
            .map_err(|_| WeaponAnalysisError::UnsafePath)?;
        // 与 nativePC 无关的文件（readme、预览图、可选贴图…）不参与武器闭包。
        // 真机上这类文件几乎必然存在，过去会把整个 Mod 判成混合包而拒绝；
        // 这里只忽略，若最后一件武器资源都没有再报 SourceNotFound。
        let Some(relative_path) = strip_leading_package_dirs(&safe_path) else {
            continue;
        };
        let canonical_path = relative_path.as_str().to_owned();
        let path_key = canonical_path.to_ascii_lowercase();
        if let Some(previous_path) = path_keys.insert(path_key, canonical_path.clone()) {
            if previous_path == canonical_path {
                has_duplicate_path = true;
            } else {
                has_case_collision = true;
            }
        }

        prepared.push(PreparedAsset {
            package_file_id: package_file_id.clone(),
            relative_path,
        });
    }

    if has_case_collision {
        return Err(WeaponAnalysisError::CaseInsensitivePathCollision);
    }
    if has_duplicate_path {
        return Err(WeaponAnalysisError::DuplicateAssetPath);
    }

    prepared.sort_by(|left, right| {
        left.relative_path
            .as_str()
            .cmp(right.relative_path.as_str())
            .then_with(|| {
                left.package_file_id
                    .as_str()
                    .cmp(right.package_file_id.as_str())
            })
    });

    let mut parsed_assets = Vec::new();
    for asset in prepared {
        // 剥离外层目录后仍不是武器资源（防具、NPC 之类的 nativePC 子树）
        // 同样只忽略。真正需要硬失败的是 `UnsupportedResource`——那是
        // `nativePC/wp/` 底下出现了形态不对的东西，属于明确的包结构错误信号。
        match WeaponModelAssetPath::parse(asset.relative_path.as_str()) {
            Ok(model_path) => parsed_assets.push(WeaponSourceAsset {
                package_file_id: asset.package_file_id,
                model_path,
            }),
            Err(WeaponPathError::NotWeaponPath) => continue,
            Err(error) => return Err(map_path_error(error)),
        }
    }

    // 放宽 mixed payload 后的门禁下限：一件武器资源都没有就必须失败，
    // 否则纯杂物包会被当成合法（空）武器包放过。
    if parsed_assets.is_empty() {
        return Err(WeaponAnalysisError::SourceNotFound);
    }

    let families = parsed_assets
        .iter()
        .map(|asset| asset.model_path.root().family())
        .collect::<BTreeSet<_>>();
    if families.len() > 1 {
        return Err(WeaponAnalysisError::MixedFamily);
    }

    let roots = parsed_assets
        .iter()
        .map(|asset| asset.model_path.root().clone())
        .collect::<BTreeSet<_>>();
    if roots.len() > 1 {
        return Err(WeaponAnalysisError::MultipleSourceRoots);
    }

    let root = roots
        .into_iter()
        .next()
        .expect("a parsed weapon asset always has a source root");
    let asset_count = parsed_assets.len();
    let mut pair_builders = BTreeMap::<String, (WeaponPartId, PairBuilder)>::new();
    for asset in parsed_assets {
        let part_id = asset.model_path.part_id().clone();
        let builder = &mut pair_builders
            .entry(part_id.as_str().to_owned())
            .or_insert_with(|| (part_id, PairBuilder::default()))
            .1;
        let destination = match asset.kind() {
            WeaponModelAssetKind::Mod3 => &mut builder.mod3,
            WeaponModelAssetKind::Mrl3 => &mut builder.mrl3,
        };
        if destination.replace(asset).is_some() {
            return Err(WeaponAnalysisError::DuplicateAssetPath);
        }
    }

    let mut pairs = Vec::with_capacity(pair_builders.len());
    for (_, (part_id, builder)) in pair_builders {
        let (Some(mod3), Some(mrl3)) = (builder.mod3, builder.mrl3) else {
            return Err(WeaponAnalysisError::IncompleteBinaryPair);
        };
        pairs.push(WeaponModelPair {
            part_id,
            mod3,
            mrl3,
        });
    }
    pairs.sort_by(|left, right| {
        left.part_id
            .role()
            .cmp(&right.part_id.role())
            .then_with(|| left.part_id.as_str().cmp(right.part_id.as_str()))
    });

    let mut warnings = Vec::new();
    if let Some(secondary) = root.family().secondary_part() {
        let roles = pairs
            .iter()
            .map(|pair| pair.part_id.role())
            .collect::<BTreeSet<_>>();
        if !roles.contains(&WeaponPartRole::Main) || !roles.contains(&secondary.role()) {
            warnings.push(WeaponAnalysisWarning::PartialPartSet);
        }
    }

    let stable_id = generate_mhw_equipment_stable_id(
        EquipmentCandidateTargetKind::Weapon,
        root.path_family(),
        root.normalized_path().as_str(),
    )
    .map_err(|_| WeaponAnalysisError::IdentityInvalid)?;
    let source_id =
        ReplacementSourceId::parse(stable_id).map_err(|_| WeaponAnalysisError::IdentityInvalid)?;

    Ok(WeaponSourceClosure {
        source_id,
        root,
        pairs,
        asset_count,
        warnings,
    })
}

fn map_path_error(error: WeaponPathError) -> WeaponAnalysisError {
    match error {
        WeaponPathError::UnsafePath => WeaponAnalysisError::UnsafePath,
        WeaponPathError::NotWeaponPath => WeaponAnalysisError::SourceNotFound,
        WeaponPathError::UnknownFamily => WeaponAnalysisError::UnknownFamily,
        WeaponPathError::InvalidMainId | WeaponPathError::CrossFamilyTarget => {
            WeaponAnalysisError::InvalidMainId
        }
        WeaponPathError::UnknownPart => WeaponAnalysisError::UnknownPart,
        WeaponPathError::UnsupportedResource => WeaponAnalysisError::UnsupportedResource,
    }
}
