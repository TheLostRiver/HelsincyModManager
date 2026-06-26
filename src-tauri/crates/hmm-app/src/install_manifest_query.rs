use hmm_core::{InstallManifest, ModId, ProfileId};
use hmm_ports::InstallManifestRepository;
use std::sync::Arc;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallManifestQueryRequest {
    pub profile_id: ProfileId,
    pub mod_ids: Vec<ModId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallManifestStatus {
    NotInstalled,
    Installed,
    RepairRequired,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallManifestStatusSummary {
    pub profile_id: ProfileId,
    pub mod_id: ModId,
    pub status: InstallManifestStatus,
    pub managed_file_count: usize,
    pub backup_count: usize,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum InstallManifestQueryError {
    #[error("install manifest query failed")]
    ManifestUnavailable,
}

#[derive(Clone)]
pub struct InstallManifestQueryService {
    manifest_repository: Arc<dyn InstallManifestRepository>,
}

impl InstallManifestQueryService {
    pub fn new(manifest_repository: Arc<dyn InstallManifestRepository>) -> Self {
        Self {
            manifest_repository,
        }
    }

    pub fn query_statuses(
        &self,
        request: InstallManifestQueryRequest,
    ) -> Result<Vec<InstallManifestStatusSummary>, InstallManifestQueryError> {
        let manifest = self
            .manifest_repository
            .load_manifest(&request.profile_id)
            .map_err(|_| InstallManifestQueryError::ManifestUnavailable)?;

        Ok(request
            .mod_ids
            .into_iter()
            .map(|mod_id| summary_for_mod(&request.profile_id, &mod_id, manifest.as_ref()))
            .collect())
    }
}

fn summary_for_mod(
    profile_id: &ProfileId,
    mod_id: &ModId,
    manifest: Option<&InstallManifest>,
) -> InstallManifestStatusSummary {
    let (managed_file_count, backup_count) = manifest
        .map(|manifest| {
            manifest
                .entries
                .iter()
                .filter(|entry| entry.mod_id == *mod_id)
                .fold((0_usize, 0_usize), |(managed, backups), entry| {
                    (managed + 1, backups + usize::from(entry.backup_ref.is_some()))
                })
        })
        .unwrap_or((0, 0));

    let status = if managed_file_count == 0 {
        InstallManifestStatus::NotInstalled
    } else {
        InstallManifestStatus::Installed
    };

    InstallManifestStatusSummary {
        profile_id: profile_id.clone(),
        mod_id: mod_id.clone(),
        status,
        managed_file_count,
        backup_count,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hmm_core::{
        FileLayer, InstallManifest, InstallManifestEntry, InstallTargetPath, ModId, PackageFileId,
        ProfileId,
    };
    use hmm_ports::InstallManifestRepository;
    use std::sync::Arc;

    #[derive(Clone)]
    struct FakeInstallManifestRepository {
        manifest: Option<InstallManifest>,
    }

    impl InstallManifestRepository for FakeInstallManifestRepository {
        fn load_manifest(
            &self,
            _profile_id: &ProfileId,
        ) -> anyhow::Result<Option<InstallManifest>> {
            Ok(self.manifest.clone())
        }

        fn save_manifest(&self, _manifest: &InstallManifest) -> anyhow::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn query_returns_not_installed_for_requested_mods_when_manifest_is_missing() {
        let service = InstallManifestQueryService::new(Arc::new(FakeInstallManifestRepository {
            manifest: None,
        }));

        let summaries = service
            .query_statuses(InstallManifestQueryRequest {
                profile_id: ProfileId::new("default"),
                mod_ids: vec![ModId::new("mod-a"), ModId::new("mod-b")],
            })
            .expect("missing manifest is a valid empty install state");

        assert_eq!(summaries.len(), 2);
        assert_eq!(summaries[0].mod_id.as_str(), "mod-a");
        assert_eq!(summaries[0].status, InstallManifestStatus::NotInstalled);
        assert_eq!(summaries[0].managed_file_count, 0);
        assert_eq!(summaries[0].backup_count, 0);
        assert_eq!(summaries[1].mod_id.as_str(), "mod-b");
        assert_eq!(summaries[1].status, InstallManifestStatus::NotInstalled);
    }

    #[test]
    fn query_returns_installed_summary_without_exposing_target_paths() {
        let manifest = InstallManifest {
            profile_id: ProfileId::new("default"),
            entries: vec![
                manifest_entry("mod-a", "nativePC/a.mod3", Some("backup-original-a")),
                manifest_entry("mod-a", "nativePC/b.mod3", None),
                manifest_entry("mod-b", "nativePC/c.mod3", Some("backup-original-b")),
            ],
        };
        let service = InstallManifestQueryService::new(Arc::new(FakeInstallManifestRepository {
            manifest: Some(manifest),
        }));

        let summaries = service
            .query_statuses(InstallManifestQueryRequest {
                profile_id: ProfileId::new("default"),
                mod_ids: vec![ModId::new("mod-a")],
            })
            .expect("manifest query should succeed");

        assert_eq!(
            summaries,
            vec![InstallManifestStatusSummary {
                profile_id: ProfileId::new("default"),
                mod_id: ModId::new("mod-a"),
                status: InstallManifestStatus::Installed,
                managed_file_count: 2,
                backup_count: 1,
            }]
        );
    }

    fn manifest_entry(
        mod_id: &str,
        target_path: &str,
        backup_ref: Option<&str>,
    ) -> InstallManifestEntry {
        InstallManifestEntry {
            target_path: InstallTargetPath::parse(target_path, ["nativePC"]).expect("target path"),
            mod_id: ModId::new(mod_id),
            package_file_id: PackageFileId::new(target_path),
            layer: FileLayer::new("base", 0),
            backup_ref: backup_ref.map(str::to_owned),
        }
    }
}
