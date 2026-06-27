use hmm_app::{
    AppSettingsService, AuditLogDiagnosticsExportService, CommitInstallPlanRequest,
    GameProfileWriteLockRegistry, GameSetupService, ImportedModInstallCommitRequest,
    InstallCommitError, InstallCommitPhase, InstallCommitResult, InstallCommitService,
    InstallManifestQueryService, InstallPlanCommitter, InstallPlanningService,
    InstallRecoveryActionPreview, InstallRecoveryActionPreviewError,
    InstallRecoveryActionPreviewRequest, InstallRecoveryActionPreviewService,
    InstallRecoveryScanError, InstallRecoveryScanRequest, InstallRecoveryScanService,
    InstallRecoverySummary, InstallTaskRunner, InstallTaskService, LimitedPreviewImageProcessor,
    ModDependencyGraphService, ModImportAnalysisService, ModImportPrepareService,
    ModImportTaskRunner, ModImportTaskService, ModLibraryService, ModUninstaller,
    PreviewImageCandidateListService, PreviewImageCandidateSelectionService,
    PreviewImageDetailService, PreviewImageDiagnosticsExportService, PreviewImageService,
    StartUninstallTaskRequest, SupportDiagnosticsExportService, TaskManager,
    ThumbnailCacheMaintenanceScheduler, UninstallModError, UninstallModRequest, UninstallModResult,
    UninstallModService, UninstallTaskRunner, UninstallTaskService,
    DEFAULT_PREVIEW_IMAGE_PROCESSING_CONCURRENCY, DEFAULT_THUMBNAIL_CACHE_MAINTENANCE_INTERVAL,
};
use hmm_core::{GameId, PreviewImagePolicy};
use hmm_games_mhw::MonsterHunterWorldAdapter;
use hmm_infra::{
    FileSystemAuditLogWriter, FileSystemDiagnosticPackageExporter, FileSystemInstallBackupStore,
    FileSystemInstallGameFileSystem, FileSystemInstallSourceFileReader, FileSystemTextLogReader,
    FileSystemThumbnailStore, ImageCratePreviewImageProcessor, JsonAppSettingsRepository,
    JsonGameConfigRepository, JsonInstallManifestRepository, JsonInstallRecoveryRecordRepository,
    JsonModImportResultRepository, PlatformSteamRootProvider, RealGameDirectoryProbeFactory,
    SandboxModPackageInstallFileScanner, SandboxModPackageMetadataAnalyzer,
    SandboxPackagePreviewScanner, SteamGameDiscoveryService, SystemClock,
    SystemDiagnosticsEnvironmentProvider, TaskScopedModImportSandboxLocator,
    ZipModImportPackagePreparer,
};
use hmm_ports::{
    AppSettingsRepository, AuditLogReader, AuditLogWriter, DiagnosticPackageExporter,
    DiagnosticsEnvironmentProvider, GameAdapter, GameConfigRepository, ModImportResultRepository,
    ModImportSandboxLocator, TextLogReader, ThumbnailCacheMaintenance,
};
use std::fmt::Display;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::{AppHandle, Manager};

pub struct AppState {
    pub game_setup: Arc<GameSetupService>,
    pub mod_library: Arc<ModLibraryService>,
    pub mod_dependency_graph: Arc<ModDependencyGraphService>,
    pub preview_image_candidates: Arc<PreviewImageCandidateListService>,
    pub preview_image_selection: Arc<PreviewImageCandidateSelectionService>,
    pub preview_image_detail: Arc<PreviewImageDetailService>,
    pub preview_image_diagnostics_export: Arc<PreviewImageDiagnosticsExportService>,
    pub audit_log_diagnostics_export: Arc<AuditLogDiagnosticsExportService>,
    pub support_diagnostics_export: Arc<SupportDiagnosticsExportService>,
    pub install_planning: Arc<InstallPlanningService>,
    pub install_manifest_query: Arc<InstallManifestQueryService>,
    pub(crate) install_recovery_scanner: Arc<ConfiguredInstallRecoveryScanner>,
    pub(crate) install_recovery_action_previewer: Arc<ConfiguredInstallRecoveryActionPreviewer>,
    pub install_task_runner: Arc<InstallTaskRunner>,
    pub install_tasks: Arc<InstallTaskService>,
    pub uninstall_task_runner: Arc<UninstallTaskRunner>,
    pub uninstall_tasks: Arc<UninstallTaskService>,
    pub mod_import_task_runner: Arc<ModImportTaskRunner>,
    pub mod_import_tasks: Arc<ModImportTaskService>,
    pub app_settings: Arc<AppSettingsService>,
    pub task_manager: Arc<TaskManager>,
}

impl AppState {
    pub fn new(app_handle: &AppHandle) -> Result<Self, String> {
        let app_data_dir = app_handle
            .path()
            .app_data_dir()
            .map_err(|error| format!("failed to resolve app data dir: {error}"))?;
        let config_path = app_data_dir.join("config").join("games.json");
        let settings_path = app_data_dir.join("config").join("settings.json");
        let mod_import_results_path = app_data_dir.join("mod-import").join("results.json");
        let mod_import_sandbox_root = app_data_dir.join("mod-import").join("sandboxes");

        let task_manager = Arc::new(TaskManager::new());
        let mhw_adapter: Arc<dyn GameAdapter> = Arc::new(MonsterHunterWorldAdapter);
        let game_config_repository: Arc<dyn GameConfigRepository> =
            Arc::new(JsonGameConfigRepository::new(config_path));
        let mod_import_result_repository: Arc<dyn ModImportResultRepository> =
            Arc::new(JsonModImportResultRepository::new(mod_import_results_path));
        let mod_import_sandbox_locator: Arc<dyn ModImportSandboxLocator> = Arc::new(
            TaskScopedModImportSandboxLocator::new(mod_import_sandbox_root.clone()),
        );
        let thumbnail_cache_maintenance: Arc<dyn ThumbnailCacheMaintenance> =
            Arc::new(FileSystemThumbnailStore::new(app_data_dir.clone()));
        let diagnostic_package_exporter: Arc<dyn DiagnosticPackageExporter> = Arc::new(
            FileSystemDiagnosticPackageExporter::new(app_data_dir.clone()),
        );
        let text_log_reader: Arc<dyn TextLogReader> =
            Arc::new(FileSystemTextLogReader::new(app_data_dir.clone()));
        let diagnostics_environment_provider: Arc<dyn DiagnosticsEnvironmentProvider> =
            Arc::new(SystemDiagnosticsEnvironmentProvider::new(
                env!("CARGO_PKG_VERSION").to_owned(),
                vec![mhw_adapter.game_id().as_str().to_owned()],
            ));
        let file_system_audit_log = Arc::new(FileSystemAuditLogWriter::new(app_data_dir.clone()));
        let audit_log_writer: Arc<dyn AuditLogWriter> = file_system_audit_log.clone();
        let audit_log_reader: Arc<dyn AuditLogReader> = file_system_audit_log;
        let app_settings_repository: Arc<dyn AppSettingsRepository> =
            Arc::new(JsonAppSettingsRepository::new(settings_path));
        let install_manifest_repository = Arc::new(JsonInstallManifestRepository::new(
            app_data_dir.join("install").join("manifests"),
        ));
        let app_settings = Arc::new(AppSettingsService::new(Arc::clone(
            &app_settings_repository,
        )));
        let preview_image_service = PreviewImageService::new(
            PreviewImagePolicy::default(),
            Box::new(SandboxPackagePreviewScanner),
            Box::new(LimitedPreviewImageProcessor::new(
                Box::new(ImageCratePreviewImageProcessor::new(Box::new(
                    FileSystemThumbnailStore::new(app_data_dir.clone()),
                ))),
                DEFAULT_PREVIEW_IMAGE_PROCESSING_CONCURRENCY,
            )),
        );
        let mod_import_prepare_service = Arc::new(ModImportPrepareService::new(
            Box::new(ZipModImportPackagePreparer::new(mod_import_sandbox_root)),
            ModImportAnalysisService::new(
                Box::new(preview_image_service),
                Box::new(FileSystemThumbnailStore::new(app_data_dir.clone())),
                Box::new(SandboxModPackageMetadataAnalyzer),
            ),
        ));
        let mod_library = Arc::new(ModLibraryService::new(Arc::clone(
            &mod_import_result_repository,
        )));
        let mod_dependency_graph = Arc::new(ModDependencyGraphService::new(Arc::clone(
            &mod_import_result_repository,
        )));
        let preview_image_candidates = Arc::new(PreviewImageCandidateListService::new(
            PreviewImagePolicy::default(),
            Arc::clone(&mod_import_result_repository),
            Arc::clone(&mod_import_sandbox_locator),
            Box::new(SandboxPackagePreviewScanner),
        ));
        let preview_image_selection = Arc::new(PreviewImageCandidateSelectionService::new(
            PreviewImagePolicy::default(),
            Arc::clone(&mod_import_result_repository),
            Arc::clone(&mod_import_sandbox_locator),
            Box::new(SandboxPackagePreviewScanner),
            Box::new(LimitedPreviewImageProcessor::new(
                Box::new(ImageCratePreviewImageProcessor::new(Box::new(
                    FileSystemThumbnailStore::new(app_data_dir.clone()),
                ))),
                DEFAULT_PREVIEW_IMAGE_PROCESSING_CONCURRENCY,
            )),
            Box::new(FileSystemThumbnailStore::new(app_data_dir.clone())),
        ));
        let preview_image_detail = Arc::new(PreviewImageDetailService::new(
            PreviewImagePolicy {
                output_max_edge_px: 1024,
                ..PreviewImagePolicy::default()
            },
            Arc::clone(&mod_import_result_repository),
            Arc::clone(&mod_import_sandbox_locator),
            Box::new(SandboxPackagePreviewScanner),
            Box::new(LimitedPreviewImageProcessor::new(
                Box::new(ImageCratePreviewImageProcessor::new(Box::new(
                    FileSystemThumbnailStore::new(app_data_dir.clone()),
                ))),
                DEFAULT_PREVIEW_IMAGE_PROCESSING_CONCURRENCY,
            )),
            Box::new(FileSystemThumbnailStore::new(app_data_dir.clone())),
        ));
        let preview_image_diagnostics_export = Arc::new(PreviewImageDiagnosticsExportService::new(
            Arc::clone(&mod_import_result_repository),
            Arc::clone(&diagnostic_package_exporter),
            Arc::clone(&audit_log_writer),
            Arc::new(SystemClock),
        ));
        let audit_log_diagnostics_export = Arc::new(AuditLogDiagnosticsExportService::new(
            Arc::clone(&audit_log_reader),
            Arc::clone(&diagnostic_package_exporter),
            Arc::clone(&audit_log_writer),
            Arc::new(SystemClock),
        ));
        let support_diagnostics_export = Arc::new(SupportDiagnosticsExportService::new(
            text_log_reader,
            audit_log_reader,
            diagnostics_environment_provider,
            diagnostic_package_exporter,
            Arc::clone(&audit_log_writer),
            Arc::new(SystemClock),
        ));
        let install_planning = Arc::new(InstallPlanningService::with_imported_mod_sources(
            Arc::clone(&mod_import_result_repository),
            Arc::clone(&mod_import_sandbox_locator),
            Arc::new(SandboxModPackageInstallFileScanner),
            vec![Arc::clone(&mhw_adapter)],
        ));
        let install_manifest_query = Arc::new(InstallManifestQueryService::new(
            install_manifest_repository.clone(),
        ));
        let install_write_locks = Arc::new(GameProfileWriteLockRegistry::default());
        let install_recovery_scanner = Arc::new(ConfiguredInstallRecoveryScanner::new(
            Arc::clone(&game_config_repository),
            app_data_dir.clone(),
            Arc::clone(&install_write_locks),
        ));
        let install_recovery_action_previewer =
            Arc::new(ConfiguredInstallRecoveryActionPreviewer::new(
                Arc::clone(&game_config_repository),
                app_data_dir.clone(),
                Arc::clone(&install_write_locks),
            ));
        let install_committer: Arc<dyn InstallPlanCommitter> =
            Arc::new(ConfiguredInstallCommitter::new(
                Arc::clone(&game_config_repository),
                Arc::clone(&mod_import_result_repository),
                Arc::clone(&mod_import_sandbox_locator),
                app_data_dir.clone(),
            ));
        let mod_uninstaller: Arc<dyn ModUninstaller> = Arc::new(ConfiguredModUninstaller::new(
            Arc::clone(&game_config_repository),
            app_data_dir.clone(),
        ));
        let install_task_runner = Arc::new(InstallTaskRunner::with_write_locks(
            Arc::clone(&task_manager),
            install_planning.clone(),
            install_committer,
            Arc::clone(&audit_log_writer),
            Arc::new(SystemClock),
            Arc::clone(&install_write_locks),
        ));
        let uninstall_task_runner = Arc::new(UninstallTaskRunner::with_write_locks(
            Arc::clone(&task_manager),
            mod_uninstaller,
            Arc::clone(&audit_log_writer),
            Arc::new(SystemClock),
            install_write_locks,
        ));
        let mod_import_task_runner = Arc::new(
            ModImportTaskRunner::new(
                Arc::clone(&task_manager),
                mod_import_prepare_service,
                Arc::clone(&mod_import_result_repository),
            )
            .with_thumbnail_cache_maintenance(thumbnail_cache_maintenance)
            .with_app_settings_repository(app_settings_repository),
        );
        let thumbnail_cache_scheduler = ThumbnailCacheMaintenanceScheduler::new(
            Arc::clone(&mod_import_task_runner),
            DEFAULT_THUMBNAIL_CACHE_MAINTENANCE_INTERVAL,
        );
        start_best_effort_background_task("thumbnail-cache-maintenance", || {
            std::thread::Builder::new()
                .name("thumbnail-cache-maintenance".to_owned())
                .spawn(move || thumbnail_cache_scheduler.run_forever())
                .map(|_| ())
        });

        Ok(Self {
            game_setup: Arc::new(GameSetupService::new(
                vec![mhw_adapter],
                Arc::clone(&game_config_repository),
                Arc::new(RealGameDirectoryProbeFactory),
                Arc::new(SteamGameDiscoveryService::new(Arc::new(
                    PlatformSteamRootProvider,
                ))),
                Arc::new(SystemClock),
            )),
            mod_library,
            mod_dependency_graph,
            preview_image_candidates,
            preview_image_selection,
            preview_image_detail,
            preview_image_diagnostics_export,
            audit_log_diagnostics_export,
            support_diagnostics_export,
            install_planning,
            install_manifest_query,
            install_recovery_scanner,
            install_recovery_action_previewer,
            install_task_runner,
            install_tasks: Arc::new(InstallTaskService::new(Arc::clone(&task_manager))),
            uninstall_task_runner,
            uninstall_tasks: Arc::new(UninstallTaskService::new(Arc::clone(&task_manager))),
            mod_import_task_runner,
            mod_import_tasks: Arc::new(ModImportTaskService::new(Arc::clone(&task_manager))),
            app_settings,
            task_manager,
        })
    }
}

pub(crate) struct ConfiguredInstallRecoveryScanner {
    game_config_repository: Arc<dyn GameConfigRepository>,
    app_data_dir: PathBuf,
    write_locks: Arc<GameProfileWriteLockRegistry>,
}

impl ConfiguredInstallRecoveryScanner {
    fn new(
        game_config_repository: Arc<dyn GameConfigRepository>,
        app_data_dir: PathBuf,
        write_locks: Arc<GameProfileWriteLockRegistry>,
    ) -> Self {
        Self {
            game_config_repository,
            app_data_dir,
            write_locks,
        }
    }

    pub(crate) fn scan(
        &self,
        game_id: GameId,
        request: InstallRecoveryScanRequest,
    ) -> Result<Vec<InstallRecoverySummary>, InstallRecoveryScanError> {
        let write_lock = self.write_locks.lock_for(&game_id, &request.profile_id);
        let _guard = write_lock
            .lock()
            .map_err(|_| InstallRecoveryScanError::ManifestUnavailable)?;
        let game_instance = self
            .game_config_repository
            .load_game_instance(&game_id)
            .map_err(|_| InstallRecoveryScanError::GameInstanceUnavailable)?
            .ok_or(InstallRecoveryScanError::GameInstanceUnavailable)?;
        let service = InstallRecoveryScanService::new_with_recovery_records(
            Arc::new(FileSystemInstallGameFileSystem::new(game_instance.root_dir)),
            Arc::new(FileSystemInstallBackupStore::new(
                self.app_data_dir.join("install").join("backups"),
            )),
            Arc::new(JsonInstallManifestRepository::new(
                self.app_data_dir.join("install").join("manifests"),
            )),
            Arc::new(JsonInstallRecoveryRecordRepository::new(
                self.app_data_dir.join("install").join("recovery"),
            )),
        );

        service.scan(request)
    }
}

pub(crate) struct ConfiguredInstallRecoveryActionPreviewer {
    game_config_repository: Arc<dyn GameConfigRepository>,
    app_data_dir: PathBuf,
    write_locks: Arc<GameProfileWriteLockRegistry>,
}

impl ConfiguredInstallRecoveryActionPreviewer {
    fn new(
        game_config_repository: Arc<dyn GameConfigRepository>,
        app_data_dir: PathBuf,
        write_locks: Arc<GameProfileWriteLockRegistry>,
    ) -> Self {
        Self {
            game_config_repository,
            app_data_dir,
            write_locks,
        }
    }

    pub(crate) fn preview(
        &self,
        game_id: GameId,
        request: InstallRecoveryActionPreviewRequest,
    ) -> Result<InstallRecoveryActionPreview, InstallRecoveryActionPreviewError> {
        let write_lock = self.write_locks.lock_for(&game_id, &request.profile_id);
        let _guard = write_lock
            .lock()
            .map_err(|_| InstallRecoveryActionPreviewError::PreviewUnavailable)?;
        let game_instance = self
            .game_config_repository
            .load_game_instance(&game_id)
            .map_err(|_| InstallRecoveryActionPreviewError::GameInstanceUnavailable)?
            .ok_or(InstallRecoveryActionPreviewError::GameInstanceUnavailable)?;
        let service = InstallRecoveryActionPreviewService::new(
            Arc::new(FileSystemInstallGameFileSystem::new(game_instance.root_dir)),
            Arc::new(FileSystemInstallBackupStore::new(
                self.app_data_dir.join("install").join("backups"),
            )),
            Arc::new(JsonInstallRecoveryRecordRepository::new(
                self.app_data_dir.join("install").join("recovery"),
            )),
        );

        service.preview(request)
    }
}

struct ConfiguredModUninstaller {
    game_config_repository: Arc<dyn GameConfigRepository>,
    app_data_dir: PathBuf,
}

impl ConfiguredModUninstaller {
    fn new(game_config_repository: Arc<dyn GameConfigRepository>, app_data_dir: PathBuf) -> Self {
        Self {
            game_config_repository,
            app_data_dir,
        }
    }
}

impl ModUninstaller for ConfiguredModUninstaller {
    fn uninstall_mod(
        &self,
        request: StartUninstallTaskRequest,
    ) -> Result<UninstallModResult, UninstallModError> {
        let game_instance = self
            .game_config_repository
            .load_game_instance(&request.game_id)
            .map_err(|_| UninstallModError::GameInstanceUnavailable)?
            .ok_or(UninstallModError::GameInstanceUnavailable)?;
        let service = UninstallModService::new(
            Arc::new(FileSystemInstallGameFileSystem::new(game_instance.root_dir)),
            Arc::new(FileSystemInstallBackupStore::new(
                self.app_data_dir.join("install").join("backups"),
            )),
            Arc::new(JsonInstallManifestRepository::new(
                self.app_data_dir.join("install").join("manifests"),
            )),
        );

        service.uninstall_mod(UninstallModRequest {
            profile_id: request.profile_id,
            mod_id: request.mod_id,
        })
    }
}

struct ConfiguredInstallCommitter {
    game_config_repository: Arc<dyn GameConfigRepository>,
    mod_import_result_repository: Arc<dyn ModImportResultRepository>,
    mod_import_sandbox_locator: Arc<dyn ModImportSandboxLocator>,
    app_data_dir: PathBuf,
}

impl ConfiguredInstallCommitter {
    fn new(
        game_config_repository: Arc<dyn GameConfigRepository>,
        mod_import_result_repository: Arc<dyn ModImportResultRepository>,
        mod_import_sandbox_locator: Arc<dyn ModImportSandboxLocator>,
        app_data_dir: PathBuf,
    ) -> Self {
        Self {
            game_config_repository,
            mod_import_result_repository,
            mod_import_sandbox_locator,
            app_data_dir,
        }
    }
}

impl InstallPlanCommitter for ConfiguredInstallCommitter {
    fn commit_install_plan(
        &self,
        request: ImportedModInstallCommitRequest,
    ) -> Result<InstallCommitResult, InstallCommitError> {
        let game_instance = self
            .game_config_repository
            .load_game_instance(&request.game_id)
            .map_err(|_| InstallCommitError::Failed {
                phase: InstallCommitPhase::TargetRead,
            })?
            .ok_or(InstallCommitError::Failed {
                phase: InstallCommitPhase::TargetRead,
            })?;
        let analysis = self
            .mod_import_result_repository
            .get_analysis(request.mod_id.as_str())
            .map_err(|_| InstallCommitError::Failed {
                phase: InstallCommitPhase::SourceRead,
            })?
            .ok_or(InstallCommitError::Failed {
                phase: InstallCommitPhase::SourceRead,
            })?;
        let source_root = self
            .mod_import_sandbox_locator
            .sandbox_root_for_package(&analysis.package_id)
            .map_err(|_| InstallCommitError::Failed {
                phase: InstallCommitPhase::SourceRead,
            })?;
        let service = InstallCommitService::new_with_recovery_records(
            Arc::new(FileSystemInstallSourceFileReader::new(source_root)),
            Arc::new(FileSystemInstallGameFileSystem::new(game_instance.root_dir)),
            Arc::new(FileSystemInstallBackupStore::new(
                self.app_data_dir.join("install").join("backups"),
            )),
            Arc::new(JsonInstallManifestRepository::new(
                self.app_data_dir.join("install").join("manifests"),
            )),
            Arc::new(JsonInstallRecoveryRecordRepository::new(
                self.app_data_dir.join("install").join("recovery"),
            )),
        );

        service.commit_plan(CommitInstallPlanRequest {
            profile_id: request.profile_id,
            plan: request.plan,
        })
    }
}

fn start_best_effort_background_task<E>(
    task_name: &'static str,
    spawn: impl FnOnce() -> Result<(), E>,
) where
    E: Display,
{
    if let Err(error) = spawn() {
        tracing::warn!(
            task = task_name,
            error = %error,
            "failed to start best-effort background task"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hmm_core::{GameId, GameInstance, ModId, ProfileId};
    use hmm_ports::{GameConfigRepositoryError, GameConfigRepositoryResult};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Barrier};
    use std::time::{Duration, Instant};

    struct NotifyingGameConfigRepository {
        load_called: Arc<AtomicBool>,
    }

    impl GameConfigRepository for NotifyingGameConfigRepository {
        fn load_game_instance(
            &self,
            _game_id: &GameId,
        ) -> GameConfigRepositoryResult<Option<GameInstance>> {
            self.load_called.store(true, Ordering::SeqCst);
            Err(GameConfigRepositoryError::StorageFailed(
                "not configured in test".to_owned(),
            ))
        }

        fn save_game_instance(&self, _instance: &GameInstance) -> GameConfigRepositoryResult<()> {
            panic!("recovery scanner lock test must not save game config")
        }
    }

    #[test]
    fn best_effort_background_task_start_ignores_spawn_failure() {
        start_best_effort_background_task("thumbnail-cache-maintenance", || -> Result<(), &str> {
            Err("spawn failed")
        });
    }

    #[test]
    fn recovery_scan_waits_for_shared_game_profile_write_lock() {
        let game_id = GameId::mhw();
        let profile_id = ProfileId::new("default");
        let write_locks = Arc::new(GameProfileWriteLockRegistry::default());
        let write_lock = write_locks.lock_for(&game_id, &profile_id);
        let guard = write_lock.lock().expect("hold shared write lock");
        let load_called = Arc::new(AtomicBool::new(false));
        let scanner = Arc::new(ConfiguredInstallRecoveryScanner::new(
            Arc::new(NotifyingGameConfigRepository {
                load_called: Arc::clone(&load_called),
            }),
            std::env::temp_dir().join("hmm-recovery-lock-test"),
            Arc::clone(&write_locks),
        ));
        let barrier = Arc::new(Barrier::new(2));
        let scan_barrier = Arc::clone(&barrier);
        let scan_scanner = Arc::clone(&scanner);
        let scan_game_id = game_id.clone();
        let scan_profile_id = profile_id.clone();

        let handle = std::thread::spawn(move || {
            scan_barrier.wait();
            scan_scanner.scan(
                scan_game_id,
                InstallRecoveryScanRequest {
                    profile_id: scan_profile_id,
                    mod_ids: vec![ModId::new("mod-a")],
                },
            )
        });

        barrier.wait();
        let deadline = Instant::now() + Duration::from_millis(150);
        while Instant::now() < deadline {
            if load_called.load(Ordering::SeqCst) {
                break;
            }
            std::thread::yield_now();
        }

        assert!(
            !load_called.load(Ordering::SeqCst),
            "recovery scan entered filesystem/config work while the shared write lock was held"
        );

        drop(guard);
        let result = handle
            .join()
            .expect("recovery scan thread should not panic");
        assert_eq!(
            result,
            Err(InstallRecoveryScanError::GameInstanceUnavailable)
        );
        assert!(load_called.load(Ordering::SeqCst));
    }

    #[test]
    fn recovery_action_preview_waits_for_shared_game_profile_write_lock() {
        let game_id = GameId::mhw();
        let profile_id = ProfileId::new("default");
        let write_locks = Arc::new(GameProfileWriteLockRegistry::default());
        let write_lock = write_locks.lock_for(&game_id, &profile_id);
        let guard = write_lock.lock().expect("hold shared write lock");
        let load_called = Arc::new(AtomicBool::new(false));
        let previewer = Arc::new(ConfiguredInstallRecoveryActionPreviewer::new(
            Arc::new(NotifyingGameConfigRepository {
                load_called: Arc::clone(&load_called),
            }),
            std::env::temp_dir().join("hmm-recovery-action-preview-lock-test"),
            Arc::clone(&write_locks),
        ));
        let barrier = Arc::new(Barrier::new(2));
        let preview_barrier = Arc::clone(&barrier);
        let previewer_for_thread = Arc::clone(&previewer);
        let preview_game_id = game_id.clone();
        let preview_profile_id = profile_id.clone();

        let handle = std::thread::spawn(move || {
            preview_barrier.wait();
            previewer_for_thread.preview(
                preview_game_id,
                InstallRecoveryActionPreviewRequest {
                    profile_id: preview_profile_id,
                    mod_id: ModId::new("mod-a"),
                    action_kind: hmm_app::InstallRecoveryActionKind::RollbackInstall,
                },
            )
        });

        barrier.wait();
        let deadline = Instant::now() + Duration::from_millis(150);
        while Instant::now() < deadline {
            if load_called.load(Ordering::SeqCst) {
                break;
            }
            std::thread::yield_now();
        }

        assert!(
            !load_called.load(Ordering::SeqCst),
            "recovery action preview entered filesystem/config work while the shared write lock was held"
        );

        drop(guard);
        let result = handle
            .join()
            .expect("recovery action preview thread should not panic");
        assert_eq!(
            result,
            Err(InstallRecoveryActionPreviewError::GameInstanceUnavailable)
        );
        assert!(load_called.load(Ordering::SeqCst));
    }
}
