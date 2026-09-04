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
    /// 同样保留但不再产生（#336 两遍分类法）：`UnknownFamily` / `InvalidMainId` /
    /// `UnsupportedResource` 描述的都是「`nativePC/wp/` 下有形态不符的文件」，
    /// 而真实包里那正是伴生文件的常态，现在由分类器归档而非否决整包。
    /// 错误码与前端文案保留，存量 manifest 与日志仍可解析。
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

/// 随行文件的落位方式。
///
/// #336：真实 Mod 在 `nativePC/wp/` 下必然携带模型之外的伴生文件——贴图 `.tex/.dds`、
/// 特效 `.efx/.epv3`、附件 `.evwp/.ctc`、作者自建目录。旧版把它们一律当作包结构错误
/// 否决整包，导致库里 4/4 真实包不可重定向。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WeaponCompanionPlacement {
    /// 在源槽位目录**之内**：按主 ID + 文件名前缀规则重定位到目标槽位。
    /// 例：`wp/swo/swo035/epv/swo035.epv3`、`wp/two/two003/mod/two003_BML.tex`。
    Relocated,
    /// 在 `nativePC/wp/` 下但与槽位**无关**：原路径保留，换任何目标槽位都仍有效。
    /// 例：`wp/swo/Tamonowo/*`、`wp/two/DARKMOON/*`、`wp/Sakurad/*`、`wp/swo/epv/*`。
    /// 参照实现同样原样留在原地（真机实验实证）。
    Verbatim,
}

/// 一个随行文件：不是可重定向模型，但必须跟着走，否则重定向出来的装备缺资源。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WeaponCompanionAsset {
    package_file_id: PackageFileId,
    relative_path: InstallTargetPath,
    placement: WeaponCompanionPlacement,
}

impl WeaponCompanionAsset {
    pub fn package_file_id(&self) -> &PackageFileId {
        &self.package_file_id
    }

    pub fn relative_path(&self) -> &InstallTargetPath {
        &self.relative_path
    }

    pub fn placement(&self) -> WeaponCompanionPlacement {
        self.placement
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WeaponSourceClosure {
    source_id: ReplacementSourceId,
    root: WeaponResourceRoot,
    pairs: Vec<WeaponModelPair>,
    companions: Vec<WeaponCompanionAsset>,
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

    pub fn companions(&self) -> &[WeaponCompanionAsset] {
        &self.companions
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

    /*
     * 第一遍：只用严格语法收集可重定向模型，据此定唯一源根。
     *
     * #336：旧版是一遍二分——解析成功就收下，解析失败（除 NotWeaponPath）就否决整包。
     * 而 `NotWeaponPath` 只在路径不以 `nativePC/wp` 开头时返回，于是 `nativePC/wp/` 内
     * 一切伴生文件都会否决整包。现在这一遍**只挑模型**，其余留给第二遍分类，不再是错误。
     */
    let mut parsed_assets = Vec::new();
    let mut unclassified = Vec::new();
    for asset in prepared {
        match WeaponModelAssetPath::parse(asset.relative_path.as_str()) {
            Ok(model_path) => parsed_assets.push(WeaponSourceAsset {
                package_file_id: asset.package_file_id,
                model_path,
            }),
            Err(error) => unclassified.push((asset, error)),
        }
    }

    // 门禁下限：一件武器模型都没有就必须失败，否则纯杂物包会被当成合法（空）武器包放过。
    if parsed_assets.is_empty() {
        /*
         * 一个模型都没认出来时还没有源根可供分类，但诊断码要挑更可操作的那个：包里若有
         * `mod3`/`mrl3` 只是部件名不认识，报 `UnknownPart` 比笼统的「未找到武器资源」有用。
         */
        let has_unknown_part_model = unclassified.iter().any(|(asset, error)| {
            matches!(error, WeaponPathError::UnknownPart)
                && asset
                    .relative_path
                    .as_str()
                    .rsplit_once('.')
                    .is_some_and(|(_, extension)| matches!(extension, "mod3" | "mrl3"))
        });
        return Err(if has_unknown_part_model {
            WeaponAnalysisError::UnknownPart
        } else {
            WeaponAnalysisError::SourceNotFound
        });
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

    /*
     * 第二遍：源根已定，把剩下的文件分档。
     *
     * - 不在 `nativePC/wp/` 下（防具、NPC、readme、预览图…）→ 忽略，与本武器无关
     * - 在源槽位目录**之内** → 随行·需重定位（贴图、特效、`.evwp`、`.ctc`…）
     * - 在 `nativePC/wp/` 下但与槽位无关 → 随行·原样（作者自建目录、族级 `epv/`、`sound/`）
     *
     * 这一遍**不产生任何错误**：真实包里这三类都是常态。危险类型（可执行等）的拒绝
     * 属 hmm-install-safety 边界，见 #336 切片③。
     */
    let mut companions = Vec::with_capacity(unclassified.len());
    for (asset, error) in unclassified {
        let segments = asset.relative_path.as_str().split('/').collect::<Vec<_>>();
        let under_weapon_tree =
            segments.first() == Some(&"nativePC") && segments.get(1) == Some(&"wp");
        if !under_weapon_tree {
            continue;
        }
        let inside_root = root.contains(&asset.relative_path);

        /*
         * 唯一仍然硬失败的一档：源槽位目录内、扩展名是 mod3/mrl3、但部件名不在注册表里
         * （如太刀的 `saya035ol`）。它是**模型**，不是伴生文件——若当伴生文件搬运，它的
         * MRL3 里指向源槽位的贴图引用就不会被改写，重定向后断链。
         *
         * 正确做法是让未登记部件走正常的配对 + MRL3 改写管线（#336 正文洞见 2 的推论），
         * 但那要改 `WeaponPartId` 模型，而部件注册表是 `WEAPON_RETARGET_DESIGN.md:167`
         * 明文冻结的口径，需独立的设计变更。故本切片保持失败关闭、不猜。
         */
        if inside_root
            && matches!(error, WeaponPathError::UnknownPart)
            && segments
                .last()
                .and_then(|name| name.rsplit_once('.'))
                .is_some_and(|(_, extension)| matches!(extension, "mod3" | "mrl3"))
        {
            return Err(WeaponAnalysisError::UnknownPart);
        }

        companions.push(WeaponCompanionAsset {
            package_file_id: asset.package_file_id,
            relative_path: asset.relative_path,
            placement: if inside_root {
                WeaponCompanionPlacement::Relocated
            } else {
                WeaponCompanionPlacement::Verbatim
            },
        });
    }

    let asset_count = parsed_assets.len() + companions.len();
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
        companions,
        asset_count,
        warnings,
    })
}
