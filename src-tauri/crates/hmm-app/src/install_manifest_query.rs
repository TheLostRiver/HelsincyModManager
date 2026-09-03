use hmm_core::{
    InstallManifest, InstallManifestStatusConsumption, ModId, ModRevisionId, ProfileId,
    ReplacementTargetId,
};
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
    CommittedCleanupPending,
    CleanupPending,
    RollbackRequired,
    RepairRequired,
    Unknown,
}

impl InstallManifestStatus {
    pub fn from_recovery_status(status: crate::InstallRecoveryStatus) -> Self {
        match status {
            crate::InstallRecoveryStatus::NotInstalled => Self::NotInstalled,
            crate::InstallRecoveryStatus::Completed => Self::Installed,
            crate::InstallRecoveryStatus::CommittedCleanupPending => Self::CommittedCleanupPending,
            crate::InstallRecoveryStatus::CleanupPending => Self::CleanupPending,
            crate::InstallRecoveryStatus::RollbackRequired => Self::RollbackRequired,
            crate::InstallRecoveryStatus::RepairRequired => Self::RepairRequired,
            crate::InstallRecoveryStatus::Unknown => Self::Unknown,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallManifestStatusSummary {
    pub profile_id: ProfileId,
    pub mod_id: ModId,
    pub status: InstallManifestStatus,
    pub managed_file_count: usize,
    pub backup_count: usize,
    /// The exact installed revision when the manifest records revisioned facts (schema v2);
    /// `None` for legacy manifests, not-installed mods and recovery-derived summaries.
    pub installed_revision_id: Option<ModRevisionId>,
    /// Entries claimed from an external installation (#286 adopt): no `backup_ref`, so
    /// uninstalling deletes them with nothing to restore. `None` when the summary comes
    /// from a source that does not carry the fact (the library projection); manifest and
    /// recovery-scan reads always report a count.
    pub adopted_file_count: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplacementTargetOccupancy {
    pub target_id: ReplacementTargetId,
    pub mod_id: ModId,
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

    pub fn query_installed_replacement_target(
        &self,
        profile_id: &ProfileId,
        mod_id: &ModId,
    ) -> Result<Option<ReplacementTargetId>, InstallManifestQueryError> {
        let manifest = self
            .manifest_repository
            .load_manifest(profile_id)
            .map_err(|_| InstallManifestQueryError::ManifestUnavailable)?;
        let Some(manifest) = manifest else {
            return Ok(None);
        };
        if manifest.profile_id != *profile_id {
            return Err(InstallManifestQueryError::ManifestUnavailable);
        }
        manifest
            .validate()
            .map_err(|_| InstallManifestQueryError::ManifestUnavailable)?;
        if summary_for_mod(profile_id, mod_id, Some(&manifest)).status
            != InstallManifestStatus::Installed
        {
            return Ok(None);
        }

        let mut bindings = manifest
            .replacement_bindings
            .iter()
            .filter(|snapshot| snapshot.mod_id() == mod_id);
        let Some(binding) = bindings.next() else {
            return Ok(None);
        };
        if binding.profile_id() != profile_id || bindings.next().is_some() {
            return Err(InstallManifestQueryError::ManifestUnavailable);
        }

        Ok(Some(binding.binding().target_id().clone()))
    }

    /// 列出该 profile 下**其他 Mod** 已占用的替换目标。
    ///
    /// 只用于前端提示（选中被占用目标时禁用预览/安装）。硬门禁在预览、任务、
    /// commit 三层，不依赖本查询，所以这里对不可信状态一律 fail-open 返回空。
    pub fn query_replacement_target_occupancy(
        &self,
        profile_id: &ProfileId,
        exclude_mod_id: &ModId,
    ) -> Result<Vec<ReplacementTargetOccupancy>, InstallManifestQueryError> {
        let manifest = self
            .manifest_repository
            .load_manifest(profile_id)
            .map_err(|_| InstallManifestQueryError::ManifestUnavailable)?;
        let Some(manifest) = manifest else {
            return Ok(Vec::new());
        };
        if manifest.profile_id != *profile_id {
            return Err(InstallManifestQueryError::ManifestUnavailable);
        }
        manifest
            .validate()
            .map_err(|_| InstallManifestQueryError::ManifestUnavailable)?;

        let mut occupancy: Vec<ReplacementTargetOccupancy> = Vec::new();
        for snapshot in &manifest.replacement_bindings {
            // 自身占用不算占用：用户重选自己已装的目标走的是 target switch。
            if snapshot.mod_id() == exclude_mod_id || snapshot.profile_id() != profile_id {
                continue;
            }
            // summary_for_mod 的 Installed 判定等价于清单状态为 TrustEntries：
            // InFlight / RollbackRequired / RepairRequired 的清单不能作为占用依据。
            if summary_for_mod(profile_id, snapshot.mod_id(), Some(&manifest)).status
                != InstallManifestStatus::Installed
            {
                continue;
            }
            let target_id = snapshot.binding().target_id().clone();
            if occupancy.iter().any(|item| item.target_id == target_id) {
                continue;
            }
            occupancy.push(ReplacementTargetOccupancy {
                target_id,
                mod_id: snapshot.mod_id().clone(),
            });
        }
        Ok(occupancy)
    }
}

fn summary_for_mod(
    profile_id: &ProfileId,
    mod_id: &ModId,
    manifest: Option<&InstallManifest>,
) -> InstallManifestStatusSummary {
    let (managed_file_count, backup_count, adopted_file_count, installed_revision_id) = manifest
        .map(|manifest| {
            let entries = manifest
                .entries
                .iter()
                .filter(|entry| entry.mod_id == *mod_id)
                .collect::<Vec<_>>();
            // The write path enforces a single revision per Mod (MultipleRevisionSet), but
            // stay defensive: only report a revision when every entry agrees on it.
            let mut revision = None;
            let mut revision_consistent = true;
            for entry in &entries {
                match (&revision, &entry.revision_id) {
                    (None, Some(candidate)) => revision = Some(candidate.clone()),
                    (Some(expected), Some(candidate)) if expected == candidate => {}
                    (Some(_), Some(_)) => {
                        revision_consistent = false;
                        break;
                    }
                    _ => {}
                }
            }
            (
                entries.len(),
                entries
                    .iter()
                    .filter(|entry| entry.backup_ref.is_some())
                    .count(),
                entries.iter().filter(|entry| entry.adopted).count(),
                revision_consistent.then_some(revision).flatten(),
            )
        })
        .unwrap_or((0, 0, 0, None));

    let status = if managed_file_count == 0 {
        InstallManifestStatus::NotInstalled
    } else {
        let manifest_status = manifest.map(|manifest| manifest.status).unwrap_or_default();
        match manifest_status.consumption() {
            InstallManifestStatusConsumption::TrustEntries => InstallManifestStatus::Installed,
            InstallManifestStatusConsumption::InFlight => InstallManifestStatus::Unknown,
            InstallManifestStatusConsumption::RollbackRequired => {
                InstallManifestStatus::RollbackRequired
            }
            InstallManifestStatusConsumption::RepairRequired => {
                InstallManifestStatus::RepairRequired
            }
        }
    };

    InstallManifestStatusSummary {
        profile_id: profile_id.clone(),
        mod_id: mod_id.clone(),
        status,
        managed_file_count,
        backup_count,
        installed_revision_id,
        adopted_file_count: Some(adopted_file_count),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hmm_core::{
        FileLayer, InstallManifest, InstallManifestEntry,
        InstallManifestStatus as CoreManifestStatus, InstallTargetPath, ModId, ModRevisionId,
        PackageFileId, ProfileId, ReplacementBinding, ReplacementBindingId,
        ReplacementBindingSnapshot, ReplacementSourceId, ReplacementTargetId,
        ReplacementTargetKind,
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
        assert_eq!(summaries[0].installed_revision_id, None);
        assert_eq!(summaries[1].mod_id.as_str(), "mod-b");
        assert_eq!(summaries[1].status, InstallManifestStatus::NotInstalled);
        assert_eq!(summaries[1].installed_revision_id, None);
    }

    #[test]
    fn query_returns_installed_revision_id_from_revisioned_entries() {
        let manifest = InstallManifest::completed(
            ProfileId::new("default"),
            vec![
                manifest_entry("mod-a", "nativePC/a.mod3", None),
                manifest_entry("mod-b", "nativePC/b.mod3", None),
            ],
        );
        let mut manifest = manifest;
        for entry in &mut manifest.entries {
            if entry.mod_id.as_str() == "mod-a" {
                entry.revision_id = Some(ModRevisionId::new("revision-v2"));
            }
        }
        let service = InstallManifestQueryService::new(Arc::new(FakeInstallManifestRepository {
            manifest: Some(manifest),
        }));

        let summaries = service
            .query_statuses(InstallManifestQueryRequest {
                profile_id: ProfileId::new("default"),
                mod_ids: vec![ModId::new("mod-a"), ModId::new("mod-b")],
            })
            .expect("manifest query should succeed");

        assert_eq!(
            summaries[0]
                .installed_revision_id
                .as_ref()
                .map(ModRevisionId::as_str),
            Some("revision-v2")
        );
        assert_eq!(summaries[1].installed_revision_id, None);
    }

    #[test]
    fn query_returns_installed_summary_without_exposing_target_paths() {
        let manifest = InstallManifest::completed(
            ProfileId::new("default"),
            vec![
                manifest_entry("mod-a", "nativePC/a.mod3", Some("backup-original-a")),
                manifest_entry("mod-a", "nativePC/b.mod3", None),
                manifest_entry("mod-b", "nativePC/c.mod3", Some("backup-original-b")),
            ],
        );
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
                installed_revision_id: None,
                adopted_file_count: Some(0),
            }]
        );
    }

    #[test]
    fn query_counts_adopted_entries_per_mod_and_reports_zero_without_a_manifest() {
        let mut manifest = InstallManifest::completed(
            ProfileId::new("default"),
            vec![
                manifest_entry("mod-a", "nativePC/a.mod3", None),
                manifest_entry("mod-a", "nativePC/b.mod3", Some("backup-original-b")),
                manifest_entry("mod-a", "nativePC/c.mod3", None),
                manifest_entry("mod-b", "nativePC/d.mod3", None),
            ],
        );
        for entry in &mut manifest.entries {
            entry.adopted = matches!(
                entry.target_path.as_str(),
                "nativePC/a.mod3" | "nativePC/c.mod3" | "nativePC/d.mod3"
            );
        }
        let service = InstallManifestQueryService::new(Arc::new(FakeInstallManifestRepository {
            manifest: Some(manifest),
        }));

        let summaries = service
            .query_statuses(InstallManifestQueryRequest {
                profile_id: ProfileId::new("default"),
                mod_ids: vec![
                    ModId::new("mod-a"),
                    ModId::new("mod-b"),
                    ModId::new("mod-c"),
                ],
            })
            .expect("manifest query should succeed");

        assert_eq!(summaries[0].managed_file_count, 3);
        assert_eq!(summaries[0].backup_count, 1);
        assert_eq!(summaries[0].adopted_file_count, Some(2));
        assert_eq!(summaries[1].adopted_file_count, Some(1));
        assert_eq!(summaries[2].status, InstallManifestStatus::NotInstalled);
        assert_eq!(summaries[2].adopted_file_count, Some(0));

        let without_manifest =
            InstallManifestQueryService::new(Arc::new(FakeInstallManifestRepository {
                manifest: None,
            }))
            .query_statuses(InstallManifestQueryRequest {
                profile_id: ProfileId::new("default"),
                mod_ids: vec![ModId::new("mod-a")],
            })
            .expect("missing manifest is a valid empty install state");
        assert_eq!(without_manifest[0].adopted_file_count, Some(0));
    }

    #[test]
    fn query_returns_only_the_installed_replacement_target_id() {
        let profile_id = ProfileId::new("default");
        let mod_id = ModId::new("mod-a");
        let mut manifest = InstallManifest::completed(
            profile_id.clone(),
            vec![manifest_entry("mod-a", "nativePC/a.mod3", None)],
        );
        manifest.replacement_bindings = vec![replacement_snapshot(
            "mod-a",
            "default",
            "mhw:armor:fatalis-beta",
        )];
        let service = InstallManifestQueryService::new(Arc::new(FakeInstallManifestRepository {
            manifest: Some(manifest),
        }));

        let target_id = service
            .query_installed_replacement_target(&profile_id, &mod_id)
            .expect("installed replacement target query should succeed")
            .expect("installed replacement target");

        assert_eq!(target_id.as_str(), "mhw:armor:fatalis-beta");
    }

    #[test]
    fn query_fails_closed_for_ambiguous_installed_replacement_targets() {
        let profile_id = ProfileId::new("default");
        let mod_id = ModId::new("mod-a");
        let mut manifest = InstallManifest::completed(
            profile_id.clone(),
            vec![manifest_entry("mod-a", "nativePC/a.mod3", None)],
        );
        manifest.replacement_bindings = vec![
            replacement_snapshot("mod-a", "default", "mhw:armor:fatalis-alpha"),
            replacement_snapshot("mod-a", "default", "mhw:armor:fatalis-beta"),
        ];
        let service = InstallManifestQueryService::new(Arc::new(FakeInstallManifestRepository {
            manifest: Some(manifest),
        }));

        assert_eq!(
            service.query_installed_replacement_target(&profile_id, &mod_id),
            Err(InstallManifestQueryError::ManifestUnavailable)
        );
    }

    #[test]
    fn query_fails_closed_for_invalid_completed_manifest() {
        let profile_id = ProfileId::new("default");
        let mod_id = ModId::new("mod-a");
        let mut manifest = InstallManifest::completed(
            profile_id.clone(),
            vec![manifest_entry("mod-a", "nativePC/a.mod3", None)],
        );
        manifest.entries[0].revision_id = Some(ModRevisionId::new("revision-a"));
        manifest.replacement_bindings = vec![replacement_snapshot(
            "mod-a",
            "default",
            "mhw:armor:fatalis-beta",
        )];
        let service = InstallManifestQueryService::new(Arc::new(FakeInstallManifestRepository {
            manifest: Some(manifest),
        }));

        assert_eq!(
            service.query_installed_replacement_target(&profile_id, &mod_id),
            Err(InstallManifestQueryError::ManifestUnavailable)
        );
    }

    #[test]
    fn query_hides_replacement_target_while_manifest_state_is_unsafe() {
        let profile_id = ProfileId::new("default");
        let mod_id = ModId::new("mod-a");
        let mut manifest = InstallManifest::completed(
            profile_id.clone(),
            vec![manifest_entry("mod-a", "nativePC/a.mod3", None)],
        );
        manifest.status = CoreManifestStatus::Committing;
        manifest.replacement_bindings = vec![replacement_snapshot(
            "mod-a",
            "default",
            "mhw:armor:fatalis-beta",
        )];
        let service = InstallManifestQueryService::new(Arc::new(FakeInstallManifestRepository {
            manifest: Some(manifest),
        }));

        assert_eq!(
            service
                .query_installed_replacement_target(&profile_id, &mod_id)
                .expect("unsafe status is represented without leaking a target"),
            None
        );
    }

    #[test]
    fn query_fails_closed_when_repository_returns_another_profiles_manifest() {
        let profile_id = ProfileId::new("default");
        let mod_id = ModId::new("mod-a");
        let mut manifest = InstallManifest::completed(
            ProfileId::new("other-profile"),
            vec![manifest_entry("mod-a", "nativePC/a.mod3", None)],
        );
        manifest.replacement_bindings = vec![replacement_snapshot(
            "mod-a",
            "default",
            "mhw:armor:fatalis-beta",
        )];
        let service = InstallManifestQueryService::new(Arc::new(FakeInstallManifestRepository {
            manifest: Some(manifest),
        }));

        assert_eq!(
            service.query_installed_replacement_target(&profile_id, &mod_id),
            Err(InstallManifestQueryError::ManifestUnavailable)
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
            revision_id: None,
            package_file_id: PackageFileId::new(target_path),
            layer: FileLayer::new("base", 0),
            backup_ref: backup_ref.map(str::to_owned),
            installed_file: None,
            adopted: false,
        }
    }

    fn replacement_snapshot(
        mod_id: &str,
        profile_id: &str,
        target_id: &str,
    ) -> ReplacementBindingSnapshot {
        ReplacementBindingSnapshot::new(
            ReplacementBinding::new(
                ReplacementBindingId::parse(format!("binding-{target_id}")).expect("binding id"),
                ModId::new(mod_id),
                ProfileId::new(profile_id),
                ReplacementSourceId::parse("mhw:armor:f_equip:pl121_0000").expect("source id"),
                ReplacementTargetId::parse(target_id).expect("target id"),
                1,
            )
            .expect("binding"),
            None,
            "pl121_0000",
            target_id,
            "pl/f_equip",
            "pl/f_equip",
            ReplacementTargetKind::parse("armor").expect("replacement kind"),
        )
        .expect("replacement snapshot")
    }

    fn manifest_with_status(status: CoreManifestStatus) -> InstallManifest {
        let mut manifest = InstallManifest::completed(
            ProfileId::new("default"),
            vec![manifest_entry("mod-a", "nativePC/a.mod3", None)],
        );
        manifest.status = status;
        manifest
    }

    fn query_status_for_mod(manifest: InstallManifest, mod_id: &str) -> InstallManifestStatus {
        let service = InstallManifestQueryService::new(Arc::new(FakeInstallManifestRepository {
            manifest: Some(manifest),
        }));

        let summaries = service
            .query_statuses(InstallManifestQueryRequest {
                profile_id: ProfileId::new("default"),
                mod_ids: vec![ModId::new(mod_id)],
            })
            .expect("manifest query should succeed");

        summaries[0].status
    }

    #[test]
    fn query_reports_rollback_required_when_manifest_status_requires_rollback() {
        assert_eq!(
            query_status_for_mod(
                manifest_with_status(CoreManifestStatus::RollbackRequired),
                "mod-a"
            ),
            InstallManifestStatus::RollbackRequired
        );
    }

    #[test]
    fn query_reports_repair_required_when_manifest_status_requires_repair() {
        assert_eq!(
            query_status_for_mod(
                manifest_with_status(CoreManifestStatus::RepairRequired),
                "mod-a"
            ),
            InstallManifestStatus::RepairRequired
        );
    }

    #[test]
    fn query_reports_unknown_while_manifest_commit_is_in_flight() {
        assert_eq!(
            query_status_for_mod(manifest_with_status(CoreManifestStatus::Planned), "mod-a"),
            InstallManifestStatus::Unknown
        );
        assert_eq!(
            query_status_for_mod(
                manifest_with_status(CoreManifestStatus::Committing),
                "mod-a"
            ),
            InstallManifestStatus::Unknown
        );
    }

    #[test]
    fn query_keeps_installed_for_remaining_mods_when_manifest_was_rolled_back() {
        assert_eq!(
            query_status_for_mod(
                manifest_with_status(CoreManifestStatus::RolledBack),
                "mod-a"
            ),
            InstallManifestStatus::Installed
        );
    }

    #[test]
    fn query_keeps_not_installed_for_unmanaged_mods_when_manifest_status_is_failure() {
        assert_eq!(
            query_status_for_mod(
                manifest_with_status(CoreManifestStatus::RollbackRequired),
                "mod-b"
            ),
            InstallManifestStatus::NotInstalled
        );
    }

    #[test]
    fn recovery_pending_states_keep_distinct_app_statuses() {
        assert_eq!(
            InstallManifestStatus::from_recovery_status(
                crate::InstallRecoveryStatus::CommittedCleanupPending
            ),
            InstallManifestStatus::CommittedCleanupPending
        );
        assert_eq!(
            InstallManifestStatus::from_recovery_status(
                crate::InstallRecoveryStatus::CleanupPending
            ),
            InstallManifestStatus::CleanupPending
        );
    }
}
