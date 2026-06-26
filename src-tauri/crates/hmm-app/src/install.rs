use hmm_core::{
    FileLayer, GameId, InstallFileProvider, InstallManifest, InstallManifestEntry, InstallPlan,
    InstallTargetPath, InstallTargetPathError, InstalledFileSummary, ModId, PackageFileId,
    ProfileId,
};
use hmm_ports::{
    GameAdapter, InstallBackupStore, InstallGameFileSystem, InstallManifestRepository,
    InstallSourceFileReader, ModImportResultRepository, ModImportSandboxLocator,
    ModPackageInstallFileScanRequest, ModPackageInstallFileScanner,
};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::Arc;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildInstallPlanRequest {
    pub allowed_target_roots: Vec<String>,
    pub files: Vec<InstallPlanFile>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildImportedModInstallPlanRequest {
    pub game_id: GameId,
    pub mod_id: ModId,
    pub layer: FileLayer,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallPlanFile {
    pub mod_id: ModId,
    pub package_file_id: PackageFileId,
    pub target_path: String,
    pub layer: FileLayer,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitInstallPlanRequest {
    pub profile_id: ProfileId,
    pub plan: InstallPlan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallCommitResult {
    pub manifest: InstallManifest,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstallCommitPhase {
    ManifestRead,
    SourceRead,
    TargetRead,
    Backup,
    Write,
    Manifest,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum InstallCommitError {
    #[error("install plan has blocking conflicts")]
    PlanHasBlockingConflicts,
    #[error("install commit failed during {phase:?}")]
    Failed { phase: InstallCommitPhase },
    #[error("install commit failed during {failed_phase:?}; rollback succeeded")]
    RollbackSucceeded { failed_phase: InstallCommitPhase },
    #[error("install commit failed during {failed_phase:?}; rollback failed")]
    RollbackFailed { failed_phase: InstallCommitPhase },
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum InstallPlanningError {
    #[error("invalid install target path")]
    InvalidTargetPath {
        package_file_id: PackageFileId,
        source: InstallTargetPathError,
    },
    #[error("imported mod install planning sources are not configured")]
    ImportedModSourcesUnavailable,
    #[error("game adapter not found")]
    GameAdapterNotFound { game_id: GameId },
    #[error("imported mod was not found")]
    ImportedModNotFound { mod_id: ModId },
    #[error("failed to read imported mod analysis")]
    ImportedModAnalysisUnavailable,
    #[error("failed to locate imported mod sandbox")]
    ImportedModSandboxUnavailable,
    #[error("failed to scan imported mod files")]
    ImportedModFileScanUnavailable,
}

#[derive(Clone)]
struct ImportedModInstallPlanSources {
    result_repository: Arc<dyn ModImportResultRepository>,
    sandbox_locator: Arc<dyn ModImportSandboxLocator>,
    file_scanner: Arc<dyn ModPackageInstallFileScanner>,
    game_adapters: Vec<Arc<dyn GameAdapter>>,
}

#[derive(Default, Clone)]
pub struct InstallPlanningService {
    imported_mod_sources: Option<ImportedModInstallPlanSources>,
}

#[derive(Clone)]
pub struct InstallCommitService {
    source_files: Arc<dyn InstallSourceFileReader>,
    game_files: Arc<dyn InstallGameFileSystem>,
    backup_store: Arc<dyn InstallBackupStore>,
    manifest_repository: Arc<dyn InstallManifestRepository>,
}

#[derive(Clone)]
struct AppliedInstallChange {
    target_path: InstallTargetPath,
    previous_bytes: Option<Vec<u8>>,
    pending_backup_ref: Option<String>,
    entry: InstallManifestEntry,
}

impl InstallCommitService {
    pub fn new(
        source_files: Arc<dyn InstallSourceFileReader>,
        game_files: Arc<dyn InstallGameFileSystem>,
        backup_store: Arc<dyn InstallBackupStore>,
        manifest_repository: Arc<dyn InstallManifestRepository>,
    ) -> Self {
        Self {
            source_files,
            game_files,
            backup_store,
            manifest_repository,
        }
    }

    pub fn commit_plan(
        &self,
        request: CommitInstallPlanRequest,
    ) -> Result<InstallCommitResult, InstallCommitError> {
        let CommitInstallPlanRequest { profile_id, plan } = request;

        if plan.has_blocking_conflicts() {
            return Err(InstallCommitError::PlanHasBlockingConflicts);
        }

        let existing_manifest = self
            .manifest_repository
            .load_manifest(&profile_id)
            .map_err(|_| InstallCommitError::Failed {
                phase: InstallCommitPhase::ManifestRead,
            })?;
        let existing_backup_refs = manifest_backup_refs_by_target(existing_manifest.as_ref());
        let mut applied_changes = Vec::new();

        for action in plan.actions {
            let source_bytes = self
                .source_files
                .read_source_file(&action.provider.package_file_id)
                .map_err(|_| {
                    self.fail_or_rollback(&applied_changes, InstallCommitPhase::SourceRead)
                })?;
            let previous_bytes = self
                .game_files
                .read_game_file(&action.target_path)
                .map_err(|_| {
                    self.fail_or_rollback(&applied_changes, InstallCommitPhase::TargetRead)
                })?;
            let backup_ref = if let Some(bytes) = previous_bytes.as_deref() {
                Some(
                    self.backup_store
                        .store_backup(&action.target_path, bytes)
                        .map_err(|_| {
                            self.fail_or_rollback(&applied_changes, InstallCommitPhase::Backup)
                        })?,
                )
            } else {
                None
            };
            let manifest_backup_ref = match existing_backup_refs.get(&action.target_path) {
                Some(existing_backup_ref) => existing_backup_ref.clone(),
                None => backup_ref.clone(),
            };

            if self
                .game_files
                .write_game_file(&action.target_path, &source_bytes)
                .is_err()
            {
                return Err(self.fail_or_rollback_with_pending_backup(
                    &applied_changes,
                    backup_ref.as_deref(),
                    InstallCommitPhase::Write,
                ));
            }

            applied_changes.push(AppliedInstallChange {
                target_path: action.target_path.clone(),
                previous_bytes,
                pending_backup_ref: backup_ref,
                entry: InstallManifestEntry {
                    target_path: action.target_path,
                    mod_id: action.provider.mod_id,
                    package_file_id: action.provider.package_file_id,
                    layer: action.provider.layer,
                    backup_ref: manifest_backup_ref,
                    installed_file: Some(installed_file_summary(&source_bytes)),
                },
            });
        }

        let manifest = merge_install_manifest(
            profile_id,
            existing_manifest,
            applied_changes
                .iter()
                .map(|change| change.entry.clone())
                .collect(),
        );

        self.manifest_repository
            .save_manifest(&manifest)
            .map_err(|_| self.fail_or_rollback(&applied_changes, InstallCommitPhase::Manifest))?;
        self.remove_obsolete_pending_backups(&applied_changes);

        Ok(InstallCommitResult { manifest })
    }

    fn fail_or_rollback(
        &self,
        applied_changes: &[AppliedInstallChange],
        failed_phase: InstallCommitPhase,
    ) -> InstallCommitError {
        self.fail_or_rollback_with_pending_backup(applied_changes, None, failed_phase)
    }

    fn fail_or_rollback_with_pending_backup(
        &self,
        applied_changes: &[AppliedInstallChange],
        pending_backup_ref: Option<&str>,
        failed_phase: InstallCommitPhase,
    ) -> InstallCommitError {
        if applied_changes.is_empty() && pending_backup_ref.is_none() {
            return InstallCommitError::Failed {
                phase: failed_phase,
            };
        }

        let rollback_result = self.rollback(applied_changes);
        if let Some(backup_ref) = pending_backup_ref {
            let _ = self.backup_store.remove_backup(backup_ref);
        }

        if rollback_result.is_ok() {
            InstallCommitError::RollbackSucceeded { failed_phase }
        } else {
            InstallCommitError::RollbackFailed { failed_phase }
        }
    }

    fn rollback(&self, applied_changes: &[AppliedInstallChange]) -> anyhow::Result<()> {
        let mut rollback_error = None;

        for change in applied_changes.iter().rev() {
            let restore_result = if let Some(previous_bytes) = &change.previous_bytes {
                self.game_files
                    .write_game_file(&change.target_path, previous_bytes)
            } else {
                self.game_files.remove_game_file(&change.target_path)
            };

            if let Err(error) = restore_result {
                rollback_error.get_or_insert(error);
            }
        }

        for change in applied_changes.iter().rev() {
            if let Some(backup_ref) = &change.pending_backup_ref {
                let _ = self.backup_store.remove_backup(backup_ref);
            }
        }

        match rollback_error {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }

    fn remove_obsolete_pending_backups(&self, applied_changes: &[AppliedInstallChange]) {
        for change in applied_changes {
            if let Some(pending_backup_ref) = &change.pending_backup_ref {
                if change.entry.backup_ref.as_deref() != Some(pending_backup_ref.as_str()) {
                    let _ = self.backup_store.remove_backup(pending_backup_ref);
                }
            }
        }
    }
}

fn manifest_backup_refs_by_target(
    existing_manifest: Option<&InstallManifest>,
) -> HashMap<InstallTargetPath, Option<String>> {
    let mut backup_refs = HashMap::new();
    if let Some(manifest) = existing_manifest {
        for entry in &manifest.entries {
            backup_refs
                .entry(entry.target_path.clone())
                .or_insert_with(|| entry.backup_ref.clone());
        }
    }

    backup_refs
}

fn merge_install_manifest(
    profile_id: ProfileId,
    existing_manifest: Option<InstallManifest>,
    applied_entries: Vec<InstallManifestEntry>,
) -> InstallManifest {
    let mut entries = existing_manifest
        .map(|manifest| manifest.entries)
        .unwrap_or_default();

    entries.retain(|entry| {
        !applied_entries
            .iter()
            .any(|applied_entry| applied_entry.target_path == entry.target_path)
    });
    entries.extend(applied_entries);

    InstallManifest {
        profile_id,
        entries,
    }
}

fn installed_file_summary(bytes: &[u8]) -> InstalledFileSummary {
    InstalledFileSummary {
        size_bytes: bytes.len() as u64,
        sha256: sha256_hex(bytes),
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

impl InstallPlanningService {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_imported_mod_sources(
        result_repository: Arc<dyn ModImportResultRepository>,
        sandbox_locator: Arc<dyn ModImportSandboxLocator>,
        file_scanner: Arc<dyn ModPackageInstallFileScanner>,
        game_adapters: Vec<Arc<dyn GameAdapter>>,
    ) -> Self {
        Self {
            imported_mod_sources: Some(ImportedModInstallPlanSources {
                result_repository,
                sandbox_locator,
                file_scanner,
                game_adapters,
            }),
        }
    }

    pub fn build_plan(
        &self,
        request: BuildInstallPlanRequest,
    ) -> Result<InstallPlan, InstallPlanningError> {
        let mut providers = Vec::with_capacity(request.files.len());

        for file in request.files {
            let target_path =
                InstallTargetPath::parse(file.target_path, request.allowed_target_roots.iter())
                    .map_err(|source| InstallPlanningError::InvalidTargetPath {
                        package_file_id: file.package_file_id.clone(),
                        source,
                    })?;

            providers.push(InstallFileProvider::new(
                file.mod_id,
                file.package_file_id,
                target_path,
                file.layer,
            ));
        }

        Ok(InstallPlan::from_providers(providers))
    }

    pub fn build_plan_from_imported_mod(
        &self,
        request: BuildImportedModInstallPlanRequest,
    ) -> Result<InstallPlan, InstallPlanningError> {
        let sources = self
            .imported_mod_sources
            .as_ref()
            .ok_or(InstallPlanningError::ImportedModSourcesUnavailable)?;
        let adapter = sources
            .game_adapters
            .iter()
            .find(|adapter| adapter.game_id() == request.game_id)
            .ok_or_else(|| InstallPlanningError::GameAdapterNotFound {
                game_id: request.game_id.clone(),
            })?;
        let analysis = sources
            .result_repository
            .get_analysis(request.mod_id.as_str())
            .map_err(|_| InstallPlanningError::ImportedModAnalysisUnavailable)?
            .ok_or_else(|| InstallPlanningError::ImportedModNotFound {
                mod_id: request.mod_id.clone(),
            })?;
        let sandbox_root = sources
            .sandbox_locator
            .sandbox_root_for_package(&analysis.package_id)
            .map_err(|_| InstallPlanningError::ImportedModSandboxUnavailable)?;
        let files = sources
            .file_scanner
            .scan_install_files(ModPackageInstallFileScanRequest {
                package_id: &analysis.package_id,
                sandbox_root: &sandbox_root,
            })
            .map_err(|_| InstallPlanningError::ImportedModFileScanUnavailable)?;
        let allowed_target_roots = adapter.allowed_install_roots();

        self.build_plan(BuildInstallPlanRequest {
            allowed_target_roots: allowed_target_roots.clone(),
            files: files
                .into_iter()
                .filter(|file| is_installable_target_path(&file.target_path, &allowed_target_roots))
                .map(|file| InstallPlanFile {
                    mod_id: request.mod_id.clone(),
                    package_file_id: PackageFileId::new(file.package_file_id),
                    target_path: file.target_path,
                    layer: request.layer.clone(),
                })
                .collect(),
        })
    }
}

fn is_installable_target_path(target_path: &str, allowed_target_roots: &[String]) -> bool {
    InstallTargetPath::parse(target_path, allowed_target_roots).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use hmm_core::{
        FileLayer, GameDirectoryValidation, GameId, InstallTargetPathError, ModId, PackageFileId,
        ProfileId,
    };
    use hmm_ports::{
        GameAdapter, GameDirectoryProbe, InstallBackupStore, InstallGameFileSystem,
        InstallManifestRepository, InstallSourceFileReader, ModImportResultRepository,
        ModImportSandboxLocator, ModPackageInstallFile, ModPackageInstallFileScanRequest,
        ModPackageInstallFileScanner, StoredImportPreviewImage, StoredModImportAnalysis,
        StoredModPackageMetadata,
    };
    use std::collections::BTreeMap;
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex};

    fn install_file(
        mod_id: &str,
        package_file_id: &str,
        target_path: &str,
        priority: i32,
    ) -> InstallPlanFile {
        InstallPlanFile {
            mod_id: ModId::new(mod_id),
            package_file_id: PackageFileId::new(package_file_id),
            target_path: target_path.to_owned(),
            layer: FileLayer::new("test", priority),
        }
    }

    #[test]
    fn build_plan_parses_allowed_target_paths_into_core_plan() {
        let service = InstallPlanningService::new();
        let request = BuildInstallPlanRequest {
            allowed_target_roots: vec!["content".to_owned()],
            files: vec![install_file(
                "mod-a",
                "file-a",
                "content/models/player.mod3",
                0,
            )],
        };

        let plan = service
            .build_plan(request)
            .expect("valid request should build an install plan");

        assert!(!plan.has_blocking_conflicts());
        assert_eq!(plan.actions.len(), 1);
        assert_eq!(
            plan.actions[0].target_path.as_str(),
            "content/models/player.mod3"
        );
        assert_eq!(plan.actions[0].provider.mod_id.as_str(), "mod-a");
    }

    #[test]
    fn build_plan_reports_package_file_for_invalid_target_path() {
        let service = InstallPlanningService::new();
        let request = BuildInstallPlanRequest {
            allowed_target_roots: vec!["content".to_owned()],
            files: vec![install_file("mod-a", "file-a", "../outside.bin", 0)],
        };

        let error = service
            .build_plan(request)
            .expect_err("invalid target path should fail planning");

        assert_eq!(
            error,
            InstallPlanningError::InvalidTargetPath {
                package_file_id: PackageFileId::new("file-a"),
                source: InstallTargetPathError::ParentTraversal,
            }
        );
    }

    #[test]
    fn build_plan_preserves_core_conflicts() {
        let service = InstallPlanningService::new();
        let request = BuildInstallPlanRequest {
            allowed_target_roots: vec!["content".to_owned()],
            files: vec![
                install_file("mod-a", "file-a", "content/models/player.mod3", 0),
                install_file("mod-b", "file-b", "content/models/player.mod3", 0),
            ],
        };

        let plan = service
            .build_plan(request)
            .expect("valid paths should build a plan even when conflicts exist");

        assert!(plan.has_blocking_conflicts());
        assert_eq!(plan.conflicts.len(), 1);
        assert_eq!(plan.conflicts[0].providers.len(), 2);
    }

    #[test]
    fn build_plan_from_imported_mod_uses_sandbox_files_and_adapter_roots() {
        let repository = Arc::new(FakeModImportResultRepository::new(vec![stored_analysis(
            "mod-a",
            "package-a",
        )]));
        let locator = Arc::new(FakeSandboxLocator {
            root: PathBuf::from("controlled-sandbox/package-a"),
        });
        let scanner = Arc::new(FakeInstallFileScanner {
            files: vec![ModPackageInstallFile {
                package_file_id: "nativePC/models/player.mod3".to_owned(),
                target_path: "nativePC/models/player.mod3".to_owned(),
            }],
            seen_requests: Mutex::new(Vec::new()),
        });
        let service = InstallPlanningService::with_imported_mod_sources(
            repository,
            locator,
            scanner.clone(),
            vec![Arc::new(FakeGameAdapter {
                game_id: GameId::mhw(),
                allowed_roots: vec!["nativePC".to_owned()],
            })],
        );

        let plan = service
            .build_plan_from_imported_mod(BuildImportedModInstallPlanRequest {
                game_id: GameId::mhw(),
                mod_id: ModId::new("mod-a"),
                layer: FileLayer::new("base", 0),
            })
            .expect("imported mod should build a plan");

        assert_eq!(plan.actions.len(), 1);
        assert_eq!(
            plan.actions[0].target_path.as_str(),
            "nativePC/models/player.mod3"
        );
        assert_eq!(plan.actions[0].provider.mod_id.as_str(), "mod-a");
        assert_eq!(
            plan.actions[0].provider.package_file_id.as_str(),
            "nativePC/models/player.mod3"
        );
        assert_eq!(
            scanner.seen_requests.lock().expect("requests").as_slice(),
            &[(
                "package-a".to_owned(),
                PathBuf::from("controlled-sandbox/package-a")
            )]
        );
    }

    #[test]
    fn build_plan_from_imported_mod_ignores_files_outside_adapter_roots() {
        let repository = Arc::new(FakeModImportResultRepository::new(vec![stored_analysis(
            "mod-a",
            "package-a",
        )]));
        let locator = Arc::new(FakeSandboxLocator {
            root: PathBuf::from("controlled-sandbox/package-a"),
        });
        let scanner = Arc::new(FakeInstallFileScanner {
            files: vec![
                ModPackageInstallFile {
                    package_file_id: "readme.txt".to_owned(),
                    target_path: "readme.txt".to_owned(),
                },
                ModPackageInstallFile {
                    package_file_id: "nativePC/models/player.mod3".to_owned(),
                    target_path: "nativePC/models/player.mod3".to_owned(),
                },
            ],
            seen_requests: Mutex::new(Vec::new()),
        });
        let service = InstallPlanningService::with_imported_mod_sources(
            repository,
            locator,
            scanner,
            vec![Arc::new(FakeGameAdapter {
                game_id: GameId::mhw(),
                allowed_roots: vec!["nativePC".to_owned()],
            })],
        );

        let plan = service
            .build_plan_from_imported_mod(BuildImportedModInstallPlanRequest {
                game_id: GameId::mhw(),
                mod_id: ModId::new("mod-a"),
                layer: FileLayer::new("base", 0),
            })
            .expect("non-install files should be ignored");

        assert_eq!(plan.actions.len(), 1);
        assert_eq!(
            plan.actions[0].target_path.as_str(),
            "nativePC/models/player.mod3"
        );
    }

    #[test]
    fn commit_plan_writes_new_files_and_persists_manifest() {
        let target = InstallTargetPath::parse("nativePC/models/player.mod3", ["nativePC"])
            .expect("valid target");
        let plan = InstallPlan::from_providers(vec![InstallFileProvider::new(
            ModId::new("mod-a"),
            PackageFileId::new("nativePC/models/player.mod3"),
            target,
            FileLayer::new("base", 0),
        )]);
        let source_files = Arc::new(RecordingInstallSourceFileReader::new([(
            "nativePC/models/player.mod3",
            b"new model".as_slice(),
        )]));
        let game_files = Arc::new(RecordingInstallGameFileSystem::default());
        let backups = Arc::new(RecordingInstallBackupStore::default());
        let manifests = Arc::new(RecordingInstallManifestRepository::default());
        let service = InstallCommitService::new(
            source_files,
            game_files.clone(),
            backups.clone(),
            manifests.clone(),
        );

        let result = service
            .commit_plan(CommitInstallPlanRequest {
                profile_id: ProfileId::new("default"),
                plan,
            })
            .expect("commit should succeed");

        assert_eq!(
            game_files
                .file_bytes("nativePC/models/player.mod3")
                .as_deref(),
            Some(b"new model".as_slice())
        );
        assert_eq!(backups.records().len(), 0);
        let manifest = manifests.take_manifest().expect("manifest should be saved");
        assert_eq!(manifest.profile_id.as_str(), "default");
        assert_eq!(manifest.entries.len(), 1);
        assert_eq!(
            manifest.entries[0].target_path.as_str(),
            "nativePC/models/player.mod3"
        );
        assert_eq!(manifest.entries[0].backup_ref, None);
        let installed_file = manifest.entries[0]
            .installed_file
            .as_ref()
            .expect("manifest entry should record installed file summary");
        assert_eq!(installed_file.size_bytes, 9);
        assert_eq!(
            installed_file.sha256,
            "d556e02a85803b1d71c94a462432da55b16b443f7579c8bfdc4a44a4c7d6a17a"
        );
        assert_eq!(result.manifest, manifest);
    }

    #[test]
    fn commit_plan_merges_existing_manifest_by_target_path() {
        let target = InstallTargetPath::parse("nativePC/models/player.mod3", ["nativePC"])
            .expect("valid target");
        let plan = InstallPlan::from_providers(vec![InstallFileProvider::new(
            ModId::new("mod-new"),
            PackageFileId::new("nativePC/models/player.mod3"),
            target,
            FileLayer::new("base", 0),
        )]);
        let source_files = Arc::new(RecordingInstallSourceFileReader::new([(
            "nativePC/models/player.mod3",
            b"new model".as_slice(),
        )]));
        let game_files = Arc::new(RecordingInstallGameFileSystem::default());
        let backups = Arc::new(RecordingInstallBackupStore::default());
        let existing_manifest = InstallManifest {
            profile_id: ProfileId::new("default"),
            entries: vec![
                InstallManifestEntry {
                    target_path: InstallTargetPath::parse(
                        "nativePC/models/keep.mod3",
                        ["nativePC"],
                    )
                    .expect("valid target"),
                    mod_id: ModId::new("mod-new"),
                    package_file_id: PackageFileId::new("nativePC/models/keep.mod3"),
                    layer: FileLayer::new("base", 0),
                    backup_ref: None,
                    installed_file: None,
                },
                InstallManifestEntry {
                    target_path: InstallTargetPath::parse(
                        "nativePC/models/player.mod3",
                        ["nativePC"],
                    )
                    .expect("valid target"),
                    mod_id: ModId::new("mod-old"),
                    package_file_id: PackageFileId::new("nativePC/models/player-old.mod3"),
                    layer: FileLayer::new("base", 0),
                    backup_ref: Some("backup-old-player".to_owned()),
                    installed_file: None,
                },
            ],
        };
        let manifests = Arc::new(
            RecordingInstallManifestRepository::default().with_existing_manifest(existing_manifest),
        );
        let service =
            InstallCommitService::new(source_files, game_files, backups, manifests.clone());

        service
            .commit_plan(CommitInstallPlanRequest {
                profile_id: ProfileId::new("default"),
                plan,
            })
            .expect("commit should succeed");

        let manifest = manifests.take_manifest().expect("manifest should be saved");
        assert_eq!(manifest.profile_id.as_str(), "default");
        assert_eq!(
            manifest
                .entries
                .iter()
                .map(|entry| (
                    entry.target_path.as_str(),
                    entry.mod_id.as_str(),
                    entry.package_file_id.as_str()
                ))
                .collect::<Vec<_>>(),
            vec![
                (
                    "nativePC/models/keep.mod3",
                    "mod-new",
                    "nativePC/models/keep.mod3"
                ),
                (
                    "nativePC/models/player.mod3",
                    "mod-new",
                    "nativePC/models/player.mod3"
                ),
            ]
        );
    }

    #[test]
    fn commit_plan_preserves_existing_backup_ref_when_replacing_manifest_entry() {
        let target = InstallTargetPath::parse("nativePC/models/player.mod3", ["nativePC"])
            .expect("valid target");
        let plan = InstallPlan::from_providers(vec![InstallFileProvider::new(
            ModId::new("mod-new"),
            PackageFileId::new("nativePC/models/player-new.mod3"),
            target,
            FileLayer::new("base", 0),
        )]);
        let source_files = Arc::new(RecordingInstallSourceFileReader::new([(
            "nativePC/models/player-new.mod3",
            b"new model".as_slice(),
        )]));
        let game_files = Arc::new(RecordingInstallGameFileSystem::with_files([(
            "nativePC/models/player.mod3",
            b"old managed model".as_slice(),
        )]));
        let backups = Arc::new(RecordingInstallBackupStore::default());
        let existing_manifest = InstallManifest {
            profile_id: ProfileId::new("default"),
            entries: vec![InstallManifestEntry {
                target_path: InstallTargetPath::parse("nativePC/models/player.mod3", ["nativePC"])
                    .expect("valid target"),
                mod_id: ModId::new("mod-old"),
                package_file_id: PackageFileId::new("nativePC/models/player-old.mod3"),
                layer: FileLayer::new("base", 0),
                backup_ref: Some("backup-original-player".to_owned()),
                installed_file: None,
            }],
        };
        let manifests = Arc::new(
            RecordingInstallManifestRepository::default().with_existing_manifest(existing_manifest),
        );
        let service =
            InstallCommitService::new(source_files, game_files, backups.clone(), manifests.clone());

        service
            .commit_plan(CommitInstallPlanRequest {
                profile_id: ProfileId::new("default"),
                plan,
            })
            .expect("commit should succeed");

        let manifest = manifests.take_manifest().expect("manifest should be saved");
        assert_eq!(manifest.entries.len(), 1);
        assert_eq!(
            manifest.entries[0].package_file_id.as_str(),
            "nativePC/models/player-new.mod3"
        );
        assert_eq!(
            manifest.entries[0].backup_ref.as_deref(),
            Some("backup-original-player")
        );
        assert_eq!(
            backups.records(),
            vec![(
                "nativePC/models/player.mod3".to_owned(),
                b"old managed model".to_vec()
            )]
        );
        assert_eq!(
            backups.removed_refs(),
            vec!["backup-nativePC-models-player.mod3".to_owned()]
        );
    }

    #[test]
    fn commit_plan_keeps_absent_backup_ref_when_replacing_managed_new_file() {
        let target = InstallTargetPath::parse("nativePC/models/new-file.mod3", ["nativePC"])
            .expect("valid target");
        let plan = InstallPlan::from_providers(vec![InstallFileProvider::new(
            ModId::new("mod-new"),
            PackageFileId::new("nativePC/models/new-file-v2.mod3"),
            target,
            FileLayer::new("base", 0),
        )]);
        let source_files = Arc::new(RecordingInstallSourceFileReader::new([(
            "nativePC/models/new-file-v2.mod3",
            b"new model v2".as_slice(),
        )]));
        let game_files = Arc::new(RecordingInstallGameFileSystem::with_files([(
            "nativePC/models/new-file.mod3",
            b"old managed new file".as_slice(),
        )]));
        let backups = Arc::new(RecordingInstallBackupStore::default());
        let existing_manifest = InstallManifest {
            profile_id: ProfileId::new("default"),
            entries: vec![InstallManifestEntry {
                target_path: InstallTargetPath::parse(
                    "nativePC/models/new-file.mod3",
                    ["nativePC"],
                )
                .expect("valid target"),
                mod_id: ModId::new("mod-old"),
                package_file_id: PackageFileId::new("nativePC/models/new-file-v1.mod3"),
                layer: FileLayer::new("base", 0),
                backup_ref: None,
                installed_file: None,
            }],
        };
        let manifests = Arc::new(
            RecordingInstallManifestRepository::default().with_existing_manifest(existing_manifest),
        );
        let service =
            InstallCommitService::new(source_files, game_files, backups.clone(), manifests.clone());

        service
            .commit_plan(CommitInstallPlanRequest {
                profile_id: ProfileId::new("default"),
                plan,
            })
            .expect("commit should succeed");

        let manifest = manifests.take_manifest().expect("manifest should be saved");
        assert_eq!(manifest.entries.len(), 1);
        assert_eq!(
            manifest.entries[0].package_file_id.as_str(),
            "nativePC/models/new-file-v2.mod3"
        );
        assert_eq!(manifest.entries[0].backup_ref, None);
        assert_eq!(
            backups.removed_refs(),
            vec!["backup-nativePC-models-new-file.mod3".to_owned()]
        );
    }

    #[test]
    fn commit_plan_aborts_before_writes_when_manifest_load_fails() {
        let target = InstallTargetPath::parse("nativePC/models/player.mod3", ["nativePC"])
            .expect("valid target");
        let plan = InstallPlan::from_providers(vec![InstallFileProvider::new(
            ModId::new("mod-a"),
            PackageFileId::new("nativePC/models/player.mod3"),
            target,
            FileLayer::new("base", 0),
        )]);
        let source_files = Arc::new(RecordingInstallSourceFileReader::new([]));
        let game_files = Arc::new(RecordingInstallGameFileSystem::default());
        let backups = Arc::new(RecordingInstallBackupStore::default());
        let manifests = Arc::new(RecordingInstallManifestRepository::failing_load());
        let service =
            InstallCommitService::new(source_files, game_files.clone(), backups, manifests);

        let error = service
            .commit_plan(CommitInstallPlanRequest {
                profile_id: ProfileId::new("default"),
                plan,
            })
            .expect_err("manifest load failure should abort before file operations");

        assert_eq!(
            error,
            InstallCommitError::Failed {
                phase: InstallCommitPhase::ManifestRead
            }
        );
        assert_eq!(game_files.file_bytes("nativePC/models/player.mod3"), None);
    }

    #[test]
    fn commit_plan_backs_up_overwritten_files_before_writing_manifest() {
        let target = InstallTargetPath::parse("nativePC/models/player.mod3", ["nativePC"])
            .expect("valid target");
        let plan = InstallPlan::from_providers(vec![InstallFileProvider::new(
            ModId::new("mod-a"),
            PackageFileId::new("nativePC/models/player.mod3"),
            target,
            FileLayer::new("base", 0),
        )]);
        let source_files = Arc::new(RecordingInstallSourceFileReader::new([(
            "nativePC/models/player.mod3",
            b"new model".as_slice(),
        )]));
        let game_files = Arc::new(RecordingInstallGameFileSystem::with_files([(
            "nativePC/models/player.mod3",
            b"old model".as_slice(),
        )]));
        let backups = Arc::new(RecordingInstallBackupStore::default());
        let manifests = Arc::new(RecordingInstallManifestRepository::default());
        let service = InstallCommitService::new(
            source_files,
            game_files.clone(),
            backups.clone(),
            manifests.clone(),
        );

        service
            .commit_plan(CommitInstallPlanRequest {
                profile_id: ProfileId::new("default"),
                plan,
            })
            .expect("commit should succeed");

        assert_eq!(
            game_files
                .file_bytes("nativePC/models/player.mod3")
                .as_deref(),
            Some(b"new model".as_slice())
        );
        assert_eq!(
            backups.records(),
            vec![(
                "nativePC/models/player.mod3".to_owned(),
                b"old model".to_vec()
            )]
        );
        let manifest = manifests.take_manifest().expect("manifest should be saved");
        assert_eq!(
            manifest.entries[0].backup_ref.as_deref(),
            Some("backup-nativePC-models-player.mod3")
        );
    }

    #[test]
    fn commit_plan_applies_layered_same_target_actions_in_priority_order() {
        let target = InstallTargetPath::parse("nativePC/models/player.mod3", ["nativePC"])
            .expect("valid target");
        let plan = InstallPlan::from_providers(vec![
            InstallFileProvider::new(
                ModId::new("mod-low"),
                PackageFileId::new("nativePC/models/player-low.mod3"),
                target.clone(),
                FileLayer::new("low", 0),
            ),
            InstallFileProvider::new(
                ModId::new("mod-high"),
                PackageFileId::new("nativePC/models/player-high.mod3"),
                target,
                FileLayer::new("high", 10),
            ),
        ]);
        let source_files = Arc::new(RecordingInstallSourceFileReader::new([
            ("nativePC/models/player-low.mod3", b"low layer".as_slice()),
            ("nativePC/models/player-high.mod3", b"high layer".as_slice()),
        ]));
        let game_files = Arc::new(RecordingInstallGameFileSystem::with_files([(
            "nativePC/models/player.mod3",
            b"original".as_slice(),
        )]));
        let backups = Arc::new(RecordingInstallBackupStore::default());
        let manifests = Arc::new(RecordingInstallManifestRepository::default());
        let service = InstallCommitService::new(
            source_files,
            game_files.clone(),
            backups.clone(),
            manifests.clone(),
        );

        service
            .commit_plan(CommitInstallPlanRequest {
                profile_id: ProfileId::new("default"),
                plan,
            })
            .expect("commit should succeed");

        assert_eq!(
            game_files
                .file_bytes("nativePC/models/player.mod3")
                .as_deref(),
            Some(b"high layer".as_slice())
        );
        assert_eq!(
            backups.records(),
            vec![
                (
                    "nativePC/models/player.mod3".to_owned(),
                    b"original".to_vec()
                ),
                (
                    "nativePC/models/player.mod3".to_owned(),
                    b"low layer".to_vec()
                ),
            ]
        );
        let manifest = manifests.take_manifest().expect("manifest should be saved");
        assert_eq!(manifest.entries.len(), 2);
        assert_eq!(
            manifest.entries[0].package_file_id.as_str(),
            "nativePC/models/player-low.mod3"
        );
        assert_eq!(
            manifest.entries[1].package_file_id.as_str(),
            "nativePC/models/player-high.mod3"
        );
    }

    #[test]
    fn commit_plan_rolls_back_written_files_when_manifest_save_fails() {
        let new_target =
            InstallTargetPath::parse("nativePC/models/new.mod3", ["nativePC"]).expect("valid");
        let existing_target =
            InstallTargetPath::parse("nativePC/models/player.mod3", ["nativePC"]).expect("valid");
        let plan = InstallPlan::from_providers(vec![
            InstallFileProvider::new(
                ModId::new("mod-a"),
                PackageFileId::new("nativePC/models/new.mod3"),
                new_target,
                FileLayer::new("base", 0),
            ),
            InstallFileProvider::new(
                ModId::new("mod-a"),
                PackageFileId::new("nativePC/models/player.mod3"),
                existing_target,
                FileLayer::new("base", 0),
            ),
        ]);
        let source_files = Arc::new(RecordingInstallSourceFileReader::new([
            ("nativePC/models/new.mod3", b"new file".as_slice()),
            ("nativePC/models/player.mod3", b"new model".as_slice()),
        ]));
        let game_files = Arc::new(RecordingInstallGameFileSystem::with_files([(
            "nativePC/models/player.mod3",
            b"old model".as_slice(),
        )]));
        let backups = Arc::new(RecordingInstallBackupStore::default());
        let manifests = Arc::new(RecordingInstallManifestRepository::failing());
        let service =
            InstallCommitService::new(source_files, game_files.clone(), backups.clone(), manifests);

        let error = service
            .commit_plan(CommitInstallPlanRequest {
                profile_id: ProfileId::new("default"),
                plan,
            })
            .expect_err("manifest failure should abort commit");

        assert_eq!(
            error,
            InstallCommitError::RollbackSucceeded {
                failed_phase: InstallCommitPhase::Manifest
            }
        );
        assert_eq!(game_files.file_bytes("nativePC/models/new.mod3"), None);
        assert_eq!(
            game_files
                .file_bytes("nativePC/models/player.mod3")
                .as_deref(),
            Some(b"old model".as_slice())
        );
        assert_eq!(
            backups.removed_refs(),
            vec!["backup-nativePC-models-player.mod3".to_owned()]
        );
    }

    #[test]
    fn commit_plan_cleans_pending_backup_when_write_fails() {
        let target = InstallTargetPath::parse("nativePC/models/player.mod3", ["nativePC"])
            .expect("valid target");
        let plan = InstallPlan::from_providers(vec![InstallFileProvider::new(
            ModId::new("mod-a"),
            PackageFileId::new("nativePC/models/player.mod3"),
            target,
            FileLayer::new("base", 0),
        )]);
        let source_files = Arc::new(RecordingInstallSourceFileReader::new([(
            "nativePC/models/player.mod3",
            b"new model".as_slice(),
        )]));
        let game_files = Arc::new(RecordingInstallGameFileSystem::with_failing_writes([(
            "nativePC/models/player.mod3",
            b"old model".as_slice(),
        )]));
        let backups = Arc::new(RecordingInstallBackupStore::default());
        let manifests = Arc::new(RecordingInstallManifestRepository::default());
        let service = InstallCommitService::new(
            source_files,
            game_files.clone(),
            backups.clone(),
            manifests.clone(),
        );

        let error = service
            .commit_plan(CommitInstallPlanRequest {
                profile_id: ProfileId::new("default"),
                plan,
            })
            .expect_err("write failure should abort commit");

        assert_eq!(
            error,
            InstallCommitError::RollbackSucceeded {
                failed_phase: InstallCommitPhase::Write
            }
        );
        assert_eq!(
            game_files
                .file_bytes("nativePC/models/player.mod3")
                .as_deref(),
            Some(b"old model".as_slice())
        );
        assert_eq!(
            backups.removed_refs(),
            vec!["backup-nativePC-models-player.mod3".to_owned()]
        );
        assert!(manifests.take_manifest().is_none());
    }

    #[test]
    fn commit_plan_restores_all_files_even_when_backup_cleanup_fails() {
        let first_target =
            InstallTargetPath::parse("nativePC/models/first.mod3", ["nativePC"]).expect("valid");
        let second_target =
            InstallTargetPath::parse("nativePC/models/second.mod3", ["nativePC"]).expect("valid");
        let plan = InstallPlan::from_providers(vec![
            InstallFileProvider::new(
                ModId::new("mod-a"),
                PackageFileId::new("nativePC/models/first.mod3"),
                first_target,
                FileLayer::new("base", 0),
            ),
            InstallFileProvider::new(
                ModId::new("mod-a"),
                PackageFileId::new("nativePC/models/second.mod3"),
                second_target,
                FileLayer::new("base", 0),
            ),
        ]);
        let source_files = Arc::new(RecordingInstallSourceFileReader::new([
            ("nativePC/models/first.mod3", b"new first".as_slice()),
            ("nativePC/models/second.mod3", b"new second".as_slice()),
        ]));
        let game_files = Arc::new(RecordingInstallGameFileSystem::with_files([
            ("nativePC/models/first.mod3", b"old first".as_slice()),
            ("nativePC/models/second.mod3", b"old second".as_slice()),
        ]));
        let backups = Arc::new(RecordingInstallBackupStore::failing_removals());
        let manifests = Arc::new(RecordingInstallManifestRepository::failing());
        let service =
            InstallCommitService::new(source_files, game_files.clone(), backups.clone(), manifests);

        let error = service
            .commit_plan(CommitInstallPlanRequest {
                profile_id: ProfileId::new("default"),
                plan,
            })
            .expect_err("manifest failure should trigger rollback");

        assert_eq!(
            error,
            InstallCommitError::RollbackSucceeded {
                failed_phase: InstallCommitPhase::Manifest
            }
        );
        assert_eq!(
            game_files
                .file_bytes("nativePC/models/first.mod3")
                .as_deref(),
            Some(b"old first".as_slice())
        );
        assert_eq!(
            game_files
                .file_bytes("nativePC/models/second.mod3")
                .as_deref(),
            Some(b"old second".as_slice())
        );
        assert_eq!(
            backups.removed_refs(),
            vec![
                "backup-nativePC-models-second.mod3-1".to_owned(),
                "backup-nativePC-models-first.mod3".to_owned(),
            ]
        );
    }

    fn stored_analysis(mod_id: &str, package_id: &str) -> StoredModImportAnalysis {
        StoredModImportAnalysis {
            mod_id: mod_id.to_owned(),
            task_id: "task-a".to_owned(),
            package_id: package_id.to_owned(),
            display_name: "Test Mod".to_owned(),
            metadata: StoredModPackageMetadata::default(),
            preview_image: StoredImportPreviewImage::Fallback {
                reason: hmm_core::PreviewImageRejectionReason::Missing,
            },
        }
    }

    struct FakeModImportResultRepository {
        records: Vec<StoredModImportAnalysis>,
    }

    impl FakeModImportResultRepository {
        fn new(records: Vec<StoredModImportAnalysis>) -> Self {
            Self { records }
        }
    }

    impl ModImportResultRepository for FakeModImportResultRepository {
        fn save_analysis(&self, _analysis: &StoredModImportAnalysis) -> anyhow::Result<()> {
            unreachable!("install planning must not save import analysis")
        }

        fn list_analysis(&self) -> anyhow::Result<Vec<StoredModImportAnalysis>> {
            unreachable!("install planning should look up the requested mod directly")
        }

        fn get_analysis(&self, mod_id: &str) -> anyhow::Result<Option<StoredModImportAnalysis>> {
            Ok(self
                .records
                .iter()
                .find(|record| record.mod_id == mod_id)
                .cloned())
        }
    }

    struct FakeSandboxLocator {
        root: PathBuf,
    }

    impl ModImportSandboxLocator for FakeSandboxLocator {
        fn sandbox_root_for_package(&self, _package_id: &str) -> anyhow::Result<PathBuf> {
            Ok(self.root.clone())
        }
    }

    struct FakeInstallFileScanner {
        files: Vec<ModPackageInstallFile>,
        seen_requests: Mutex<Vec<(String, PathBuf)>>,
    }

    impl ModPackageInstallFileScanner for FakeInstallFileScanner {
        fn scan_install_files(
            &self,
            request: ModPackageInstallFileScanRequest<'_>,
        ) -> anyhow::Result<Vec<ModPackageInstallFile>> {
            self.seen_requests.lock().expect("requests").push((
                request.package_id.to_owned(),
                request.sandbox_root.to_path_buf(),
            ));
            Ok(self.files.clone())
        }
    }

    struct FakeGameAdapter {
        game_id: GameId,
        allowed_roots: Vec<String>,
    }

    impl GameAdapter for FakeGameAdapter {
        fn game_id(&self) -> GameId {
            self.game_id.clone()
        }

        fn display_name(&self) -> &'static str {
            "Fake Game"
        }

        fn validate_directory(&self, _probe: &dyn GameDirectoryProbe) -> GameDirectoryValidation {
            unreachable!("install planning must not probe game directories")
        }

        fn allowed_install_roots(&self) -> Vec<String> {
            self.allowed_roots.clone()
        }
    }

    struct RecordingInstallSourceFileReader {
        files: BTreeMap<String, Vec<u8>>,
    }

    impl RecordingInstallSourceFileReader {
        fn new<const N: usize>(files: [(&str, &[u8]); N]) -> Self {
            Self {
                files: files
                    .into_iter()
                    .map(|(path, bytes)| (path.to_owned(), bytes.to_vec()))
                    .collect(),
            }
        }
    }

    impl InstallSourceFileReader for RecordingInstallSourceFileReader {
        fn read_source_file(&self, package_file_id: &PackageFileId) -> anyhow::Result<Vec<u8>> {
            self.files
                .get(package_file_id.as_str())
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("missing source file"))
        }
    }

    #[derive(Default)]
    struct RecordingInstallGameFileSystem {
        files: Mutex<BTreeMap<String, Vec<u8>>>,
        fail_writes: bool,
    }

    impl RecordingInstallGameFileSystem {
        fn with_files<const N: usize>(files: [(&str, &[u8]); N]) -> Self {
            Self {
                files: Mutex::new(
                    files
                        .into_iter()
                        .map(|(path, bytes)| (path.to_owned(), bytes.to_vec()))
                        .collect(),
                ),
                fail_writes: false,
            }
        }

        fn with_failing_writes<const N: usize>(files: [(&str, &[u8]); N]) -> Self {
            Self {
                files: Mutex::new(
                    files
                        .into_iter()
                        .map(|(path, bytes)| (path.to_owned(), bytes.to_vec()))
                        .collect(),
                ),
                fail_writes: true,
            }
        }

        fn file_bytes(&self, target_path: &str) -> Option<Vec<u8>> {
            self.files.lock().expect("files").get(target_path).cloned()
        }
    }

    impl InstallGameFileSystem for RecordingInstallGameFileSystem {
        fn read_game_file(
            &self,
            target_path: &InstallTargetPath,
        ) -> anyhow::Result<Option<Vec<u8>>> {
            Ok(self
                .files
                .lock()
                .expect("files")
                .get(target_path.as_str())
                .cloned())
        }

        fn write_game_file(
            &self,
            target_path: &InstallTargetPath,
            bytes: &[u8],
        ) -> anyhow::Result<()> {
            if self.fail_writes {
                anyhow::bail!("write failed");
            }
            self.files
                .lock()
                .expect("files")
                .insert(target_path.as_str().to_owned(), bytes.to_vec());
            Ok(())
        }

        fn remove_game_file(&self, target_path: &InstallTargetPath) -> anyhow::Result<()> {
            self.files
                .lock()
                .expect("files")
                .remove(target_path.as_str());
            Ok(())
        }
    }

    #[derive(Default)]
    struct RecordingInstallBackupStore {
        records: Mutex<Vec<(String, String, Vec<u8>)>>,
        removed_refs: Mutex<Vec<String>>,
        fail_removals: bool,
    }

    impl RecordingInstallBackupStore {
        fn failing_removals() -> Self {
            Self {
                records: Mutex::new(Vec::new()),
                removed_refs: Mutex::new(Vec::new()),
                fail_removals: true,
            }
        }

        fn records(&self) -> Vec<(String, Vec<u8>)> {
            self.records
                .lock()
                .expect("records")
                .iter()
                .map(|(_, target_path, bytes)| (target_path.clone(), bytes.clone()))
                .collect()
        }

        fn removed_refs(&self) -> Vec<String> {
            self.removed_refs.lock().expect("removed refs").clone()
        }
    }

    impl InstallBackupStore for RecordingInstallBackupStore {
        fn store_backup(
            &self,
            target_path: &InstallTargetPath,
            bytes: &[u8],
        ) -> anyhow::Result<String> {
            let mut records = self.records.lock().expect("records");
            let base_ref = format!("backup-{}", target_path.as_str().replace('/', "-"));
            let backup_ref = if records.is_empty() {
                base_ref
            } else {
                format!("{base_ref}-{}", records.len())
            };
            records.push((
                backup_ref.clone(),
                target_path.as_str().to_owned(),
                bytes.to_vec(),
            ));
            Ok(backup_ref)
        }

        fn remove_backup(&self, backup_ref: &str) -> anyhow::Result<()> {
            self.removed_refs
                .lock()
                .expect("removed refs")
                .push(backup_ref.to_owned());
            if self.fail_removals {
                anyhow::bail!("backup cleanup failed");
            }
            Ok(())
        }
    }

    #[derive(Default)]
    struct RecordingInstallManifestRepository {
        existing_manifest: Option<InstallManifest>,
        saved_manifest: Mutex<Option<InstallManifest>>,
        fail_load: bool,
        fail_save: bool,
    }

    impl RecordingInstallManifestRepository {
        fn failing_load() -> Self {
            Self {
                existing_manifest: None,
                saved_manifest: Mutex::new(None),
                fail_load: true,
                fail_save: false,
            }
        }

        fn failing() -> Self {
            Self {
                existing_manifest: None,
                saved_manifest: Mutex::new(None),
                fail_load: false,
                fail_save: true,
            }
        }

        fn with_existing_manifest(mut self, manifest: InstallManifest) -> Self {
            self.existing_manifest = Some(manifest);
            self
        }

        fn take_manifest(&self) -> Option<InstallManifest> {
            self.saved_manifest.lock().expect("manifest").take()
        }
    }

    impl InstallManifestRepository for RecordingInstallManifestRepository {
        fn load_manifest(
            &self,
            _profile_id: &ProfileId,
        ) -> anyhow::Result<Option<InstallManifest>> {
            if self.fail_load {
                anyhow::bail!("manifest load failed");
            }
            Ok(self.existing_manifest.clone())
        }

        fn save_manifest(&self, manifest: &InstallManifest) -> anyhow::Result<()> {
            if self.fail_save {
                anyhow::bail!("manifest save failed");
            }
            *self.saved_manifest.lock().expect("manifest") = Some(manifest.clone());
            Ok(())
        }
    }
}
