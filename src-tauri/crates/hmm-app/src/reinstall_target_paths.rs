use super::{InstallManifest, InstallPlan, ModId, ReinstallBlockingReason};
use std::collections::BTreeMap;

/// Windows 大小写别名仍是原有受管文件，不能被分类成一次新增加一次旧文件删除。
pub(super) fn preserve_installed_spelling(
    mod_id: &ModId,
    manifest: &InstallManifest,
    plan: &mut InstallPlan,
) -> Result<(), ReinstallBlockingReason> {
    let mut installed = BTreeMap::new();
    for entry in manifest
        .entries
        .iter()
        .filter(|entry| &entry.mod_id == mod_id)
    {
        let previous = installed.insert(entry.target_path.windows_key(), &entry.target_path);
        if previous.is_some_and(|previous| previous != &entry.target_path) {
            return Err(ReinstallBlockingReason::ManifestStateUnsafe);
        }
    }
    for action in &mut plan.actions {
        let Some(previous) = installed.get(&action.target_path.windows_key()) else {
            continue;
        };
        // NFKC 键用于保守冲突检查，不能据此把两种不同 Unicode 拼写当成同一个实际文件。
        if !previous
            .as_str()
            .eq_ignore_ascii_case(action.target_path.as_str())
        {
            return Err(ReinstallBlockingReason::PlanConflict);
        }
        action.target_path = (*previous).clone();
        action.provider.target_path = (*previous).clone();
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use hmm_core::{
        FileLayer, InstallFileProvider, InstallManifestEntry, InstallTargetPath, PackageFileId,
        ProfileId,
    };

    fn manifest(paths: &[&str]) -> InstallManifest {
        InstallManifest::completed(
            ProfileId::new("profile"),
            paths
                .iter()
                .map(|path| InstallManifestEntry {
                    target_path: InstallTargetPath::parse(*path, ["nativePC"]).unwrap(),
                    mod_id: ModId::new("mod"),
                    revision_id: None,
                    package_file_id: PackageFileId::new(*path),
                    layer: FileLayer::new("base", 0),
                    backup_ref: None,
                    installed_file: None,
                    adopted: false,
                })
                .collect(),
        )
    }

    fn candidate(path: &str) -> InstallPlan {
        InstallPlan::from_providers([InstallFileProvider::new(
            ModId::new("mod"),
            PackageFileId::new("new-revision-file"),
            InstallTargetPath::parse(path, ["nativePC"]).unwrap(),
            FileLayer::new("base", 0),
        )])
    }

    #[test]
    fn case_only_changes_keep_the_installed_target_and_new_source_identity() {
        let manifest = manifest(&["nativePC/Weapons/MODEL.mod3"]);
        let mut plan = candidate("nativePC/weapons/model.MOD3");
        preserve_installed_spelling(&ModId::new("mod"), &manifest, &mut plan).unwrap();
        assert_eq!(plan.actions[0].target_path, manifest.entries[0].target_path);
        assert_eq!(
            plan.actions[0].provider.target_path,
            manifest.entries[0].target_path
        );
        assert_eq!(
            plan.actions[0].provider.package_file_id.as_str(),
            "new-revision-file"
        );
    }

    #[test]
    fn conservative_unicode_collision_keys_never_relocate_an_unrelated_file() {
        let manifest = manifest(&["nativePC/model.mod3"]);
        let mut plan = candidate("nativePC/ｍodel.mod3");
        assert_eq!(
            preserve_installed_spelling(&ModId::new("mod"), &manifest, &mut plan),
            Err(ReinstallBlockingReason::PlanConflict)
        );
    }

    #[test]
    fn ambiguous_old_paths_are_not_silently_merged() {
        let manifest = manifest(&["nativePC/model.mod3", "nativePC/MODEL.mod3"]);
        let mut plan = candidate("nativePC/model.mod3");
        assert_eq!(
            preserve_installed_spelling(&ModId::new("mod"), &manifest, &mut plan),
            Err(ReinstallBlockingReason::ManifestStateUnsafe)
        );
    }
}
