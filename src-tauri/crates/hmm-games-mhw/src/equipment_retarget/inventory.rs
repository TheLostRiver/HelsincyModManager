use crate::package_path::{parse_safe_package_path, strip_leading_package_dirs, NATIVE_PC_ROOT};
use crate::{
    generate_mhw_equipment_stable_id, is_rejected_executable_file_name, ArmorResourcePath,
    EquipmentCandidateTargetKind, WeaponResourceRoot,
};
use hmm_core::{
    GameId, InstallTargetPath, PackageFileId, ReplacementAnalysis, ReplacementSource,
    ReplacementSourceId, ReplacementTargetKind, ReplacementWarning,
};
use hmm_ports::{ReplacementAdapterError, ReplacementAdapterResult, ReplacementAsset};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone)]
pub(super) struct Resource {
    pub id: PackageFileId,
    pub path: InstallTargetPath,
}

#[derive(Clone)]
pub(super) enum EquipmentRoot {
    Weapon(WeaponResourceRoot),
    Armor(ArmorResourcePath),
}

impl EquipmentRoot {
    fn from_path(path: &InstallTargetPath) -> Option<Self> {
        if let Some(root) =
            WeaponResourceRoot::of_resource_path(path.as_str()).filter(|root| root.contains(path))
        {
            return Some(Self::Weapon(root));
        }
        ArmorResourcePath::parse(path.as_str())
            .ok()
            .map(Self::Armor)
    }

    fn source(&self) -> ReplacementAdapterResult<ReplacementSource> {
        let (id, kind, internal, family, supported) = match self {
            Self::Weapon(root) => (
                generate_mhw_equipment_stable_id(
                    EquipmentCandidateTargetKind::Weapon,
                    root.path_family(),
                    root.normalized_path().as_str(),
                )
                .map_err(|_| ReplacementAdapterError::InvalidRetargetPlan)?,
                "weapon",
                root.main_id().as_str(),
                root.path_family(),
                true,
            ),
            Self::Armor(path) => (
                format!(
                    "mhw:armor:{}:{}",
                    path.path_family().rsplit('/').next().unwrap_or_default(),
                    path.slot()
                ),
                "armor",
                path.slot(),
                path.path_family(),
                path.is_supported(),
            ),
        };
        ReplacementSource::new(
            ReplacementSourceId::parse(id)
                .map_err(|_| ReplacementAdapterError::InvalidRetargetPlan)?,
            GameId::mhw(),
            ReplacementTargetKind::parse(kind)
                .map_err(|_| ReplacementAdapterError::InvalidRetargetPlan)?,
            internal,
            family,
            supported,
        )
        .map_err(|_| ReplacementAdapterError::InvalidRetargetPlan)
    }
}

pub(super) struct SourceResources {
    pub root: EquipmentRoot,
    pub source: ReplacementSource,
    pub resources: Vec<Resource>,
}

pub(super) struct PackageResources {
    pub sources: BTreeMap<ReplacementSourceId, SourceResources>,
    pub companions: Vec<Resource>,
    pub excluded_count: u32,
}

impl PackageResources {
    pub fn classify(assets: &[ReplacementAsset]) -> ReplacementAdapterResult<Self> {
        let mut identities = BTreeSet::new();
        let mut paths = BTreeSet::new();
        let mut prepared = Vec::new();
        let mut sources = BTreeMap::new();
        let mut outside_count = 0usize;
        let mut excluded_count = 0u32;
        for asset in assets {
            if asset.package_file_id().as_str().trim().is_empty()
                || !identities.insert(asset.package_file_id().clone())
            {
                return Err(ReplacementAdapterError::InvalidRetargetPlan);
            }
            // 先验证原路径再剥离外层目录，不能把越界路径截成看似安全的 nativePC 路径。
            let safe = parse_safe_package_path(asset.relative_path())
                .map_err(|_| ReplacementAdapterError::UnsafeRetargetPath)?;
            let Some(path) = strip_leading_package_dirs(&safe) else {
                outside_count += 1;
                continue;
            };
            let path = normalize_equipment_root(path)?;
            if !paths.insert(path.as_str().to_ascii_lowercase()) {
                return Err(ReplacementAdapterError::UnsafeRetargetPath);
            }
            let filename = path.as_str().rsplit('/').next().unwrap_or_default();
            if is_rejected_executable_file_name(filename) {
                excluded_count = excluded_count
                    .checked_add(1)
                    .ok_or(ReplacementAdapterError::InvalidRetargetPlan)?;
                continue;
            }
            let root = EquipmentRoot::from_path(&path);
            let owner = root.as_ref().map(EquipmentRoot::source).transpose()?;
            // 单独一张共享贴图不决定包的装备类型；模型、材质或其他非贴图资源才建立源单元。
            if !is_texture(&path) {
                if let (Some(root), Some(source)) = (&root, &owner) {
                    sources
                        .entry(source.id().clone())
                        .or_insert_with(|| SourceResources {
                            root: root.clone(),
                            source: source.clone(),
                            resources: Vec::new(),
                        });
                }
            }
            prepared.push((
                Resource {
                    id: asset.package_file_id().clone(),
                    path,
                },
                owner.map(|source| source.id().clone()),
            ));
        }
        let mut companions = Vec::new();
        for (resource, owner) in prepared {
            if let Some(unit) = owner.as_ref().and_then(|owner| sources.get_mut(owner)) {
                unit.resources.push(resource);
            } else {
                companions.push(resource);
            }
        }
        let order = |left: &Resource, right: &Resource| left.path.as_str().cmp(right.path.as_str());
        companions.sort_by(order);
        for unit in sources.values_mut() {
            unit.resources.sort_by(order);
        }
        let result = Self {
            sources,
            companions,
            excluded_count,
        };
        if result.installable_count() + outside_count + excluded_count as usize != assets.len() {
            return Err(ReplacementAdapterError::InvalidRetargetPlan);
        }
        Ok(result)
    }

    fn installable_count(&self) -> usize {
        self.companions.len()
            + self
                .sources
                .values()
                .map(|unit| unit.resources.len())
                .sum::<usize>()
    }

    pub fn analysis(&self) -> ReplacementAdapterResult<ReplacementAnalysis> {
        let mut warnings = Vec::new();
        if self.sources.is_empty() {
            warnings.push(ReplacementWarning::NoSupportedAssets);
        }
        if self.sources.len() > 1 {
            warnings.push(ReplacementWarning::MultipleSources);
        }
        if self
            .sources
            .values()
            .any(|unit| !unit.source.is_supported())
        {
            warnings.push(ReplacementWarning::UnsupportedSource);
        }
        ReplacementAnalysis::new(
            GameId::mhw(),
            self.sources
                .values()
                .map(|unit| unit.source.clone())
                .collect(),
            self.installable_count(),
            warnings,
        )
        .map_err(|_| ReplacementAdapterError::InvalidRetargetPlan)
    }
}

pub(super) fn is_texture(path: &InstallTargetPath) -> bool {
    path.as_str()
        .rsplit_once('.')
        .is_some_and(|(_, extension)| extension.eq_ignore_ascii_case("tex"))
}

fn normalize_equipment_root(
    path: InstallTargetPath,
) -> ReplacementAdapterResult<InstallTargetPath> {
    let mut parts = path
        .as_str()
        .split('/')
        .map(str::to_owned)
        .collect::<Vec<_>>();
    if parts.len() <= 4 {
        return Ok(path);
    }
    for part in &mut parts[1..4] {
        part.make_ascii_lowercase();
    }
    let candidate = parts.join("/");
    if WeaponResourceRoot::of_resource_path(&candidate).is_none()
        && ArmorResourcePath::parse(&candidate).is_err()
    {
        return Ok(path);
    }
    InstallTargetPath::parse(candidate, [NATIVE_PC_ROOT])
        .map_err(|_| ReplacementAdapterError::UnsafeRetargetPath)
}
