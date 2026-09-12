use super::inventory::{is_texture, EquipmentRoot, PackageResources, Resource};
use hmm_core::{
    InstallTargetPath, ReplacementSourceId, RetargetFileDisposition as Disposition,
    RetargetFileEffect, RetargetFileReason as Reason,
};
use hmm_ports::ReplacementAdapterResult;

pub(super) fn resource_effect(
    resource: &Resource,
    destination: &InstallTargetPath,
    source_id: Option<ReplacementSourceId>,
    identity: bool,
    unmapped: Option<Reason>,
) -> RetargetFileEffect {
    let (disposition, reason) = if source_id.is_none() {
        (Disposition::PackageCompanion, Reason::PackageResource)
    } else if destination.windows_key() != resource.path.windows_key() {
        (Disposition::Relocated, Reason::TargetMapping)
    } else if is_texture(&resource.path) {
        (Disposition::KeptInPlace, Reason::TextureReference)
    } else if identity {
        (Disposition::KeptInPlace, Reason::OriginalTarget)
    } else {
        (
            Disposition::KeptInPlace,
            unmapped.expect("unchanged resources have a mapping reason"),
        )
    };
    RetargetFileEffect {
        package_file_id: resource.id.clone(),
        source_id,
        source_path: resource.path.clone(),
        target_path: Some(destination.clone()),
        disposition,
        reason,
    }
}

pub(super) fn excluded_effects(
    package: &PackageResources,
) -> ReplacementAdapterResult<Vec<RetargetFileEffect>> {
    package
        .excluded_files
        .iter()
        .map(|file| {
            let source_id = EquipmentRoot::from_path(file.original_path())
                .map(|root| root.source())
                .transpose()?
                .map(|source| source.id().clone())
                .filter(|id| package.sources.contains_key(id));
            let lower = file.original_path().as_str().to_ascii_lowercase();
            let plugin = lower.starts_with("nativepc/plugins/")
                && lower.trim_end_matches(['.', ' ']).ends_with(".dll");
            Ok(RetargetFileEffect {
                package_file_id: file.package_file_id().clone(),
                source_id,
                source_path: file.original_path().clone(),
                target_path: None,
                disposition: if plugin {
                    Disposition::PluginCandidate
                } else {
                    Disposition::PolicyExcluded
                },
                reason: if plugin {
                    Reason::PluginNotIncluded
                } else {
                    Reason::ExecutablePolicy
                },
            })
        })
        .collect()
}
