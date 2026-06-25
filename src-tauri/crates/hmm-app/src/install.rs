use hmm_core::{
    FileLayer, GameId, InstallFileProvider, InstallPlan, InstallTargetPath, InstallTargetPathError,
    ModId, PackageFileId,
};
use hmm_ports::{
    GameAdapter, ModImportResultRepository, ModImportSandboxLocator,
    ModPackageInstallFileScanRequest, ModPackageInstallFileScanner,
};
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
    };
    use hmm_ports::{
        GameAdapter, GameDirectoryProbe, ModImportResultRepository, ModImportSandboxLocator,
        ModPackageInstallFile, ModPackageInstallFileScanRequest, ModPackageInstallFileScanner,
        StoredImportPreviewImage, StoredModImportAnalysis, StoredModPackageMetadata,
    };
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
}
