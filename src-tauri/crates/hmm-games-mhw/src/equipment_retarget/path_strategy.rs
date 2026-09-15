use super::inventory::{EquipmentRoot, PackageResources, Resource};
use super::numbered_identity::{NumberedId, NumberedRoot};
use super::resource_path::{NumberedResourceMapper, NumberedResourceMapping};
use crate::{
    ArmorResourcePath, KinsectId, MhwReplacementCatalog, WeaponAnalysisError, WeaponMainId,
};
use hmm_core::{
    InstallTargetPath, ReplacementAdapterFacts, ReplacementWarning, RetargetAction,
    RetargetFileReason, RetargetPlan, REPLACEMENT_ADAPTER_FACTS_SCHEMA_VERSION,
};
use hmm_ports::{
    ReplacementAdapterError, ReplacementAdapterResult, ReplacementCatalogProvider,
    RetargetPlanRequest,
};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

pub(super) fn build_plan(request: RetargetPlanRequest) -> ReplacementAdapterResult<RetargetPlan> {
    let package = PackageResources::classify(&request.assets)?;
    let unit = package
        .sources
        .get(request.binding.source_id())
        .ok_or(ReplacementAdapterError::SourceBindingMismatch)?;
    let target = MhwReplacementCatalog
        .find_replacement_target(request.binding.target_id())
        .or_else(|error| {
            MhwReplacementCatalog
                .original_target_for_source(&unit.source)
                .and_then(|target| {
                    if target.id() == request.binding.target_id() {
                        Ok(target)
                    } else {
                        Err(error)
                    }
                })
        })
        .map_err(|_| ReplacementAdapterError::TargetCatalogMissing {
            target_id: request.binding.target_id().clone(),
        })?;
    if target.target_type() != unit.source.source_type()
        || target
            .metadata()
            .get("path_family")
            .and_then(serde_json::Value::as_str)
            != Some(unit.source.path_family())
    {
        return Err(ReplacementAdapterError::UnsupportedReplacementTarget);
    }
    let identity = target.internal_id() == unit.source.internal_id();
    if !identity && !unit.source.is_supported() {
        return Err(ReplacementAdapterError::SourceHasNoAvailableTargets);
    }
    let mut actions = Vec::new();
    let mut effects = Vec::new();
    let mut kept_unmapped = false;
    let mut moved = false;
    let numbered_mapper = match &unit.root {
        EquipmentRoot::Weapon(root) => Some(NumberedResourceMapper::new(
            NumberedRoot::Weapon(root),
            unit.resources.iter().map(|resource| &resource.path),
        )),
        EquipmentRoot::Kinsect(root) => Some(NumberedResourceMapper::new(
            NumberedRoot::Kinsect(root),
            unit.resources.iter().map(|resource| &resource.path),
        )),
        EquipmentRoot::Armor(_) => None,
    };
    for resource in &unit.resources {
        let (destination, unmapped) = if identity {
            (resource.path.clone(), None)
        } else {
            destination(
                &unit.root,
                numbered_mapper.as_ref(),
                resource,
                target.internal_id(),
            )?
        };
        moved |= destination != resource.path;
        kept_unmapped |= unmapped.is_some() && destination == resource.path;
        effects.push(super::file_effects::resource_effect(
            resource,
            &destination,
            Some(unit.source.id().clone()),
            identity,
            unmapped,
        ));
        actions.push(action(
            resource,
            destination,
            &unit.source,
            target.internal_id(),
        )?);
    }
    if !identity && !moved {
        return Err(ReplacementAdapterError::SourceAnalysisRejected {
            source_id: unit.source.id().clone(),
            code: WeaponAnalysisError::NoRelocatableResources.code(),
        });
    }
    if request.carries_package_companions {
        for resource in &package.companions {
            effects.push(super::file_effects::resource_effect(
                resource,
                &resource.path,
                None,
                true,
                None,
            ));
            actions.push(action(
                resource,
                resource.path.clone(),
                &unit.source,
                target.internal_id(),
            )?);
        }
    }
    let expected = unit.resources.len()
        + if request.carries_package_companions {
            package.companions.len()
        } else {
            0
        };
    let mut targets = BTreeSet::new();
    if actions.len() != expected
        || actions
            .iter()
            .any(|action| !targets.insert(action.target_relative_path().windows_key()))
    {
        return Err(ReplacementAdapterError::InvalidRetargetPlan);
    }
    let mut warnings = Vec::new();
    if identity {
        warnings.push(ReplacementWarning::SourceMatchesTarget);
    }
    if kept_unmapped {
        warnings.push(ReplacementWarning::UnmappedResourcesKept);
    }
    if request.carries_package_companions && package.excluded_count > 0 {
        warnings.push(ReplacementWarning::PolicyExcludedResources);
    }
    let plan = RetargetPlan::new(request.binding, unit.source.clone(), actions, warnings)
        .map_err(|_| ReplacementAdapterError::InvalidRetargetPlan)?;
    if request.carries_package_companions {
        effects.extend(super::file_effects::excluded_effects(&package)?);
    }
    let plan = plan
        .with_policy_exclusions(if request.carries_package_companions {
            package.excluded_files
        } else {
            Vec::new()
        })
        .map_err(|_| ReplacementAdapterError::InvalidRetargetPlan)?;
    let plan = plan
        .with_file_effects(effects)
        .map_err(|_| ReplacementAdapterError::InvalidRetargetPlan)?;
    with_facts(
        plan,
        if request.carries_package_companions {
            package.excluded_count
        } else {
            0
        },
    )
}

pub(super) fn with_facts(
    plan: RetargetPlan,
    excluded_count: u32,
) -> ReplacementAdapterResult<RetargetPlan> {
    let closure = digest(plan.actions().iter().flat_map(|action| {
        [
            action.package_file_id().as_str(),
            action.source_relative_path().as_str(),
            action.target_relative_path().as_str(),
        ]
    }));
    let source = digest([
        plan.source().source_type().as_str(),
        plan.source().path_family(),
        plan.source().internal_id(),
    ]);
    let mut facts = ReplacementAdapterFacts::new(
        REPLACEMENT_ADAPTER_FACTS_SCHEMA_VERSION,
        "mhw.equipment",
        "resource-reference-migration",
        4,
        closure,
        source,
        plan.content_transform_set_sha256(),
    )
    .map_err(|_| ReplacementAdapterError::InvalidRetargetPlan)?
    .with_excluded_file_count(excluded_count);
    let transform_count = plan
        .actions()
        .iter()
        .filter(|action| action.content_transform().is_some())
        .count();
    if transform_count > 0 {
        facts = facts
            .with_transformers(
                plan.content_transformer_identities(),
                u32::try_from(transform_count)
                    .map_err(|_| ReplacementAdapterError::InvalidRetargetPlan)?,
                u32::try_from(plan.actions().len())
                    .map_err(|_| ReplacementAdapterError::InvalidRetargetPlan)?,
            )
            .map_err(|_| ReplacementAdapterError::InvalidRetargetPlan)?;
    }
    plan.with_adapter_facts(facts)
        .map_err(|_| ReplacementAdapterError::InvalidRetargetPlan)
}

fn destination(
    root: &EquipmentRoot,
    mapper: Option<&NumberedResourceMapper<'_>>,
    resource: &Resource,
    target: &str,
) -> ReplacementAdapterResult<(InstallTargetPath, Option<RetargetFileReason>)> {
    match root {
        EquipmentRoot::Weapon(_) => {
            let target = WeaponMainId::parse(target)
                .map_err(|_| ReplacementAdapterError::UnsupportedReplacementTarget)?;
            map_numbered(mapper, resource, NumberedId::Weapon(&target))
        }
        EquipmentRoot::Kinsect(_) => {
            let target = KinsectId::parse(target)
                .map_err(|_| ReplacementAdapterError::UnsupportedReplacementTarget)?;
            map_numbered(mapper, resource, NumberedId::Kinsect(&target))
        }
        EquipmentRoot::Armor(_) => {
            let path = ArmorResourcePath::parse(resource.path.as_str())
                .map_err(|_| ReplacementAdapterError::UnsafeRetargetPath)?;
            path.retarget(target)
                .map(|path| (path, None))
                .map_err(|_| ReplacementAdapterError::UnsafeRetargetPath)
        }
    }
}

fn map_numbered(
    mapper: Option<&NumberedResourceMapper<'_>>,
    resource: &Resource,
    target: NumberedId<'_>,
) -> ReplacementAdapterResult<(InstallTargetPath, Option<RetargetFileReason>)> {
    match mapper
        .ok_or(ReplacementAdapterError::InvalidRetargetPlan)?
        .map(&resource.path, target)
    {
        Ok(NumberedResourceMapping::Relocated(path)) => Ok((path, None)),
        Ok(NumberedResourceMapping::Kept(reason)) => {
            // 装备根已经证明归属。无法证明内部编号时只迁移根，不能留下旧装备覆盖。
            let mut parts = resource.path.as_str().split('/').collect::<Vec<_>>();
            parts[3] = target.as_str();
            let path = InstallTargetPath::parse(parts.join("/"), ["nativePC"])
                .map_err(|_| ReplacementAdapterError::UnsafeRetargetPath)?;
            Ok((path, Some(reason)))
        }
        Err(_) => Err(ReplacementAdapterError::UnsafeRetargetPath),
    }
}

fn action(
    resource: &Resource,
    target: InstallTargetPath,
    source: &hmm_core::ReplacementSource,
    target_id: &str,
) -> ReplacementAdapterResult<RetargetAction> {
    RetargetAction::new(
        resource.id.clone(),
        resource.path.clone(),
        target,
        source.id().clone(),
        source.internal_id(),
        target_id,
        source.path_family(),
        source.path_family(),
    )
    .map_err(|_| ReplacementAdapterError::InvalidRetargetPlan)
}

fn digest<'a>(values: impl IntoIterator<Item = &'a str>) -> String {
    let mut hasher = Sha256::new();
    for value in values {
        hasher.update((value.len() as u64).to_le_bytes());
        hasher.update(value.as_bytes());
    }
    format!("{:x}", hasher.finalize())
}
