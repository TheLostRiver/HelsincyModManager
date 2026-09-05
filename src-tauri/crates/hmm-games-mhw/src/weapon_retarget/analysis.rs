use super::path::parse_safe_relative_path;
use super::{
    WeaponFamily, WeaponModelAssetKind, WeaponModelAssetPath, WeaponPartId, WeaponPartRole,
    WeaponPathError, WeaponResourceRoot,
};
use crate::package_path::strip_leading_package_dirs;
use crate::{
    generate_mhw_equipment_stable_id, is_rejected_executable_file_name,
    EquipmentCandidateTargetKind,
};
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

/// 一个被拒绝清单挡下的包内文件：适配器看见了它，但**不会**为它产出任何动作。
///
/// 保留路径而不是只记数量，是为了让切片⑤ 的 UI 能列出具体被排除了什么——用户看到
/// 「已排除 3 个非游戏资源文件」却不知道是哪三个，同样属于「错误信息不指向可操作的
/// 下一步」。投影到日志、CLI 或诊断包时必须只出计数，路径属于第三方 Mod 内容。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WeaponExcludedAsset {
    package_file_id: PackageFileId,
    relative_path: InstallTargetPath,
}

impl WeaponExcludedAsset {
    pub fn package_file_id(&self) -> &PackageFileId {
        &self.package_file_id
    }

    pub fn relative_path(&self) -> &InstallTargetPath {
        &self.relative_path
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WeaponSourceClosure {
    source_id: ReplacementSourceId,
    root: WeaponResourceRoot,
    pairs: Vec<WeaponModelPair>,
    companions: Vec<WeaponCompanionAsset>,
    excluded: Vec<WeaponExcludedAsset>,
    unresolved_models: Vec<WeaponUnresolvedModel>,
    asset_count: usize,
    warnings: Vec<WeaponAnalysisWarning>,
}

/// 属于某个槽位、但**无法判断如何改写**的模型文件。
///
/// `#349`：此前这一档会否决整包（`weapon_unknown_part`）。可是同一个包里名字认不出的
/// `.dds` 走随行档、`.mod3` 就拖累整包——同一个「认不出」两种待遇，纯粹是内部不一致。
/// 现在它留在所属单元里，重定向时**原样保留在源路径**，单元本身照常可重定向。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WeaponUnresolvedModel {
    package_file_id: PackageFileId,
    relative_path: InstallTargetPath,
    reason: WeaponUnresolvedModelReason,
}

impl WeaponUnresolvedModel {
    pub fn package_file_id(&self) -> &PackageFileId {
        &self.package_file_id
    }

    pub fn relative_path(&self) -> &InstallTargetPath {
        &self.relative_path
    }

    pub fn reason(&self) -> WeaponUnresolvedModelReason {
        self.reason
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WeaponUnresolvedModelReason {
    /// 文件名里认不出「部件前缀 + 槽位编号」的结构，判断不出它对应哪个部件。
    UnrecognizedPartName,
    /// 认出了部件，但配套的另一半（`.mod3` 或 `.mrl3`）不在包里，无法安全改写引用。
    IncompleteModelPair,
}

impl WeaponUnresolvedModelReason {
    pub fn code(self) -> &'static str {
        match self {
            Self::UnrecognizedPartName => "weapon_unknown_part",
            Self::IncompleteModelPair => "weapon_incomplete_binary_pair",
        }
    }
}

/// 一个包的武器侧分析结果：**N 个各自独立的可重定向单元**。
///
/// `#349`：此前这里是「一个包 → 一个源槽位 → 接受或否决」，于是包里有两把武器
/// （`MultipleSourceRoots`）、跨族（`MixedFamily`）、模型不成对
/// （`IncompleteBinaryPair`）、部件名认不出（`UnknownPart`）四种形态都会**拒整包**——
/// 哪怕其中一把武器完全正常。判定粒度错了：分档做在文件级，否决做在包级。
///
/// 现在每个槽位是一个独立单元，逐个判定；包级只剩一条下限
/// （`SourceNotFound`：一个可重定向单元都没有）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WeaponPackageAnalysis {
    units: Vec<WeaponSourceClosure>,
}

impl WeaponPackageAnalysis {
    pub fn units(&self) -> &[WeaponSourceClosure] {
        &self.units
    }

    /// 全部单元加起来会被计划处理的资源数。
    pub fn asset_count(&self) -> usize {
        self.units
            .iter()
            .map(WeaponSourceClosure::asset_count)
            .sum()
    }

    /// 恰好一个可重定向单元时返回它，否则 `None`。
    ///
    /// 给「只支持单槽位」的调用方用（例如内容变换器：它的输入本来就是一对 MOD3/MRL3）。
    /// 需要按 binding 挑单元的调用方应当直接遍历 [`Self::units`]，不要用这个。
    pub fn sole_unit(&self) -> Option<&WeaponSourceClosure> {
        match self.units.as_slice() {
            [unit] => Some(unit),
            _ => None,
        }
    }
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

    pub fn excluded(&self) -> &[WeaponExcludedAsset] {
        &self.excluded
    }

    /// 属于本槽位但无法判断如何改写的模型文件。重定向时原样保留在源路径。
    pub fn unresolved_models(&self) -> &[WeaponUnresolvedModel] {
        &self.unresolved_models
    }

    /// 会被计划处理的资源数。**不含**被拒绝的文件——它们不会落盘，算进「已匹配」
    /// 会让前端的「本次影响 N 个文件」比实际多。
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
) -> Result<WeaponPackageAnalysis, WeaponAnalysisError> {
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

    /*
     * 门禁下限：一件武器模型都没有就必须失败，否则纯杂物包会被当成合法（空）武器包放过。
     *
     * `#349`：这是包级**唯一**保留的否决。原先这里还会在「有 mod3/mrl3 但部件名认不出」
     * 时报 `UnknownPart`，那个分支已经没用了——只要能定位到槽位根，它就会成为该单元的
     * `unresolved_models`；定位不到根的（族名不认识、层级不对）本来就不属于任何槽位。
     */
    if parsed_assets.is_empty() {
        return Err(WeaponAnalysisError::SourceNotFound);
    }

    /*
     * `#349`：按槽位根**分组**，而不是要求全包恰好一个。
     *
     * 删掉的两条包级否决：
     * - `MixedFamily`（包里既有大剑又有太刀）
     * - `MultipleSourceRoots`（包里两把同族武器）
     *
     * 两者描述的都是「作者一次发布多件装备」，那是正常的发布习惯，不是坏包。每个槽位
     * 各自成为一个独立单元、各自选目标；错误码按 `#342` / `#343` 的先例保留枚举与前端
     * 文案、不再产生，存量 manifest 与日志仍可解析。
     */
    let mut grouped = BTreeMap::<WeaponResourceRoot, Vec<WeaponSourceAsset>>::new();
    for asset in parsed_assets {
        grouped
            .entry(asset.model_path.root().clone())
            .or_default()
            .push(asset);
    }

    /*
     * 第二遍：槽位已分组，把剩下的文件分档。
     *
     * - 不在 `nativePC/wp/` 下（防具、NPC、readme、预览图…）→ 忽略，与武器无关
     * - 命中可执行 / 脚本拒绝清单 → 拒绝（`#336` 切片③），不产出动作、计数
     * - 落在**某个**槽位目录之内 → 那个单元的随行·需重定位（贴图、特效、`.evwp`、`.ctc`…）
     *   - 其中扩展名是 `mod3`/`mrl3` 但部件名认不出的 → 那个单元的 `unresolved_models`
     * - 在 `nativePC/wp/` 下但不属于任何槽位 → 随行·原样（作者自建目录、族级 `epv/`、`sound/`）
     *
     * 这一遍**不产生任何错误**：真实包里这几类全是常态。`#349` 之前还有一条
     * 「槽位内的 mod3/mrl3 部件名认不出就否决整包」，已降级为单元内的 `unresolved_models`。
     */
    let mut excluded = Vec::new();
    let mut per_root_companions = BTreeMap::<WeaponResourceRoot, Vec<WeaponCompanionAsset>>::new();
    let mut per_root_unresolved = BTreeMap::<WeaponResourceRoot, Vec<WeaponUnresolvedModel>>::new();
    let mut verbatim_companions = Vec::new();

    for (asset, error) in unclassified {
        let segments = asset.relative_path.as_str().split('/').collect::<Vec<_>>();
        let under_weapon_tree =
            segments.first() == Some(&"nativePC") && segments.get(1) == Some(&"wp");
        if !under_weapon_tree {
            continue;
        }

        /*
         * 拒绝清单排在所有归档分支之前（#336 切片③）。
         *
         * 顺序是有意的：命中拒绝清单的文件不该有机会落进任何一档随行处置，也不该因为
         * 恰好长得像别的东西而改变结论。当前 `.mod3`/`.mrl3` 都不在清单里，两条分支
         * 事实上不重叠；把拒绝放在前面是为了让「以后往清单里加扩展名」不会意外地
         * 被后面的分支抢先。
         */
        if segments
            .last()
            .is_some_and(|name| is_rejected_executable_file_name(name))
        {
            excluded.push(WeaponExcludedAsset {
                package_file_id: asset.package_file_id,
                relative_path: asset.relative_path,
            });
            continue;
        }

        let owning_root = grouped
            .keys()
            .find(|root| root.contains(&asset.relative_path))
            .cloned();

        let Some(owning_root) = owning_root else {
            verbatim_companions.push(WeaponCompanionAsset {
                package_file_id: asset.package_file_id,
                relative_path: asset.relative_path,
                placement: WeaponCompanionPlacement::Verbatim,
            });
            continue;
        };

        let is_model_extension = segments
            .last()
            .and_then(|name| name.rsplit_once('.'))
            .is_some_and(|(_, extension)| matches!(extension, "mod3" | "mrl3"));

        if is_model_extension && matches!(error, WeaponPathError::UnknownPart) {
            /*
             * `#349`：它是**模型**，当伴生文件搬运会让 MRL3 里指向源槽位的贴图引用断链，
             * 所以不能混进随行档；但它也不该拖累整包——单独记下来，重定向时原样留在源路径，
             * 由调用方按「这个文件无法判断如何改写」呈现。
             */
            per_root_unresolved
                .entry(owning_root)
                .or_default()
                .push(WeaponUnresolvedModel {
                    package_file_id: asset.package_file_id,
                    relative_path: asset.relative_path,
                    reason: WeaponUnresolvedModelReason::UnrecognizedPartName,
                });
            continue;
        }

        per_root_companions
            .entry(owning_root)
            .or_default()
            .push(WeaponCompanionAsset {
                package_file_id: asset.package_file_id,
                relative_path: asset.relative_path,
                placement: WeaponCompanionPlacement::Relocated,
            });
    }

    /*
     * 族级随行·原样文件（`wp/<族>/<作者目录>/`、族级 `epv/` `sound/`）属于**包**，
     * 不属于任何槽位——它们的处置是「原路径保留」，装到哪个单元都是同一个结果。
     *
     * 单槽位包（真实语料库里 11 个外观包全是）：归入那个唯一的单元，行为与 `#349` 之前
     * **逐字相同**。多槽位包：暂归第一个单元（按槽位根排序，确定），避免同一路径被多个
     * 单元重复产出而在 `InstallPlan` 里撞成冲突。把它提到包级是切片③（绑定模型）的事。
     */
    let first_root = grouped
        .keys()
        .next()
        .expect("parsed assets are non-empty, so at least one root exists")
        .clone();
    per_root_companions
        .entry(first_root)
        .or_default()
        .extend(verbatim_companions);

    let mut units = Vec::with_capacity(grouped.len());
    for (root, assets) in grouped {
        let mut companions = per_root_companions.remove(&root).unwrap_or_default();
        let mut unresolved_models = per_root_unresolved.remove(&root).unwrap_or_default();
        companions.sort_by(|left, right| {
            left.relative_path
                .as_str()
                .cmp(right.relative_path.as_str())
        });
        unresolved_models.sort_by(|left, right| {
            left.relative_path
                .as_str()
                .cmp(right.relative_path.as_str())
        });

        let mut pair_builders = BTreeMap::<String, (WeaponPartId, PairBuilder)>::new();
        let model_count = assets.len();
        for asset in assets {
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

        /*
         * `#349`：模型对不完整**不再否决整包**。
         *
         * 此前「主件成对 + 副件只有 `.mod3`」会拒整包——主件完全正常、完全可重定向，
         * 只因为副件缺了配套的 `.mrl3`。现在缺一半的那个部件进 `unresolved_models`
         * （原样留在源路径），其余成对的部件照常重定向。
         *
         * 一个槽位里**没有任何**完整对时，`pairs` 为空，该单元不进 `units`——包级下限
         * `SourceNotFound` 会在全部单元都为空时兜住。
         */
        let mut pairs = Vec::with_capacity(pair_builders.len());
        for (_, (part_id, builder)) in pair_builders {
            match (builder.mod3, builder.mrl3) {
                (Some(mod3), Some(mrl3)) => pairs.push(WeaponModelPair {
                    part_id,
                    mod3,
                    mrl3,
                }),
                (Some(only), None) | (None, Some(only)) => {
                    unresolved_models.push(WeaponUnresolvedModel {
                        package_file_id: only.package_file_id,
                        relative_path: only.model_path.normalized_path().clone(),
                        reason: WeaponUnresolvedModelReason::IncompleteModelPair,
                    });
                }
                (None, None) => unreachable!("a pair builder is only created from an asset"),
            }
        }
        if pairs.is_empty() {
            continue;
        }
        pairs.sort_by(|left, right| {
            left.part_id
                .role()
                .cmp(&right.part_id.role())
                .then_with(|| left.part_id.as_str().cmp(right.part_id.as_str()))
        });
        unresolved_models.sort_by(|left, right| {
            left.relative_path
                .as_str()
                .cmp(right.relative_path.as_str())
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
        let source_id = ReplacementSourceId::parse(stable_id)
            .map_err(|_| WeaponAnalysisError::IdentityInvalid)?;

        // 与 `#349` 之前同口径：**不含**被拒绝的文件——它们不落盘，算进「已匹配」会让
        // 前端的「本次影响 N 个文件」比实际多。无法改写的模型同样不计入。
        let asset_count = model_count + companions.len();

        units.push(WeaponSourceClosure {
            source_id,
            root,
            pairs,
            companions,
            excluded: Vec::new(),
            unresolved_models,
            asset_count,
            warnings,
        });
    }

    // 一个槽位都没有完整模型对：包级下限。
    let Some(first) = units.first_mut() else {
        return Err(WeaponAnalysisError::SourceNotFound);
    };
    // 被拒绝的文件是包级事实（拒绝清单不按槽位分），挂在第一个单元上保持与旧行为一致。
    first.excluded = excluded;

    Ok(WeaponPackageAnalysis { units })
}
