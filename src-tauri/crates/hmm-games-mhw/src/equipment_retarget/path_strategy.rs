use super::inventory::{is_texture, EquipmentRoot, PackageResources, Resource};
use crate::{
    ArmorResourcePath, MhwReplacementCatalog, WeaponAnalysisError, WeaponMainId, WeaponPathError,
};
use hmm_core::{
    InstallTargetPath, ReplacementAdapterFacts, ReplacementWarning, RetargetAction, RetargetPlan,
    REPLACEMENT_ADAPTER_FACTS_SCHEMA_VERSION,
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
    let mut kept_unmapped = false;
    let mut moved = false;
    for resource in &unit.resources {
        let (destination, unmapped) = if identity || is_texture(&resource.path) {
            (resource.path.clone(), false)
        } else {
            destination(&unit.root, resource, target.internal_id())?
        };
        moved |= destination != resource.path;
        kept_unmapped |= unmapped;
        actions.push(action(
            resource,
            destination,
            &unit.source,
            target.internal_id(),
        )?);
    }
    if !identity && !moved {
        return Err(ReplacementAdapterError::AnalysisRejected {
            code: WeaponAnalysisError::NoRelocatableResources.code(),
        });
    }
    if request.carries_package_companions {
        for resource in &package.companions {
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
        || actions.iter().any(|action| {
            !targets.insert(action.target_relative_path().as_str().to_ascii_lowercase())
        })
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
    let plan = RetargetPlan::new(request.binding, unit.source.clone(), actions, warnings)
        .map_err(|_| ReplacementAdapterError::InvalidRetargetPlan)?;
    let closure = digest(plan.actions().iter().flat_map(|action| {
        [
            action.package_file_id().as_str(),
            action.source_relative_path().as_str(),
            action.target_relative_path().as_str(),
        ]
    }));
    let source = digest([
        unit.source.source_type().as_str(),
        unit.source.path_family(),
        unit.source.internal_id(),
    ]);
    let facts = ReplacementAdapterFacts::new(
        REPLACEMENT_ADAPTER_FACTS_SCHEMA_VERSION,
        "mhw.equipment",
        "path-only-resource-preserving",
        1,
        closure,
        source,
        plan.content_transform_set_sha256(),
    )
    .map_err(|_| ReplacementAdapterError::InvalidRetargetPlan)?
    .with_excluded_file_count(if request.carries_package_companions {
        package.excluded_count
    } else {
        0
    });
    plan.with_adapter_facts(facts)
        .map_err(|_| ReplacementAdapterError::InvalidRetargetPlan)
}

fn destination(
    root: &EquipmentRoot,
    resource: &Resource,
    target: &str,
) -> ReplacementAdapterResult<(InstallTargetPath, bool)> {
    match root {
        EquipmentRoot::Weapon(root) => {
            let target = WeaponMainId::parse(target)
                .map_err(|_| ReplacementAdapterError::UnsupportedReplacementTarget)?;
            match root.relocate_matching_resource(&resource.path, &target) {
                Ok(Some(path)) => Ok((path, false)),
                Ok(None) | Err(WeaponPathError::UnsupportedResource) => {
                    Ok((resource.path.clone(), true))
                }
                Err(_) => Err(ReplacementAdapterError::UnsafeRetargetPath),
            }
        }
        EquipmentRoot::Armor(_) => {
            let path = ArmorResourcePath::parse(resource.path.as_str())
                .map_err(|_| ReplacementAdapterError::UnsafeRetargetPath)?;
            path.retarget(target)
                .map(|path| (path, false))
                .map_err(|_| ReplacementAdapterError::UnsafeRetargetPath)
        }
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
