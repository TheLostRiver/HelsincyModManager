use crate::mod_library::ModLibraryComposition;
use hmm_app::{
    is_identity_replacement_binding, AppSettingsService, AuditLogDiagnosticsExportService,
    CategoryService, CommitInstallPlanRequest, GameLaunchService, GamePrerequisiteDecision,
    GamePrerequisiteDecisionProvider, GameProfileWriteLockRegistry, GameSetupService,
    ImportedModInstallCommitRequest, ImportedModInstallPreflightService,
    InitialRetargetInstallPlan, InitialRetargetInstallPlanner,
    InitialRetargetInstallPreflightService, InitialRetargetInstallStatusError,
    InitialRetargetInstallStatusReader, InstallCommitError, InstallCommitPhase,
    InstallCommitResult, InstallCommitService, InstallManifestQueryService, InstallPlanCommitter,
    InstallPlanningService, InstallRecoveryActionError, InstallRecoveryActionExecutor,
    InstallRecoveryActionPreview, InstallRecoveryActionPreviewError,
    InstallRecoveryActionPreviewRequest, InstallRecoveryActionPreviewService,
    InstallRecoveryActionRequest, InstallRecoveryActionResult, InstallRecoveryActionService,
    InstallRecoveryScanError, InstallRecoveryScanRequest, InstallRecoveryScanService,
    InstallRecoverySummary, InstallTaskRunner, InstallTaskService, InstallWriteAdmission,
    InstallWriteAdmissionError, InstalledReplacementReinstallResolution,
    LimitedPreviewImageProcessor, ModDependencyGraphService, ModImportAnalysisService,
    ModImportPrepareService, ModImportTaskRunner, ModImportTaskService, ModLibraryQueryService,
    ModLibraryService, ModMetadataService, PlannedInitialRetargetInstall, PreparedReinstall,
    PreviewImageCandidateListService, PreviewImageCandidateSelectionService,
    PreviewImageDetailService, PreviewImageDiagnosticsExportService, PreviewImageService,
    PreviewInitialRetargetInstallRequest, PreviewRetargetReinstallRequest,
    ProfileSaveDirectoryDiscoveryService, ProfileService, RecoveryActionTaskRunner,
    RecoveryActionTaskService, ReinstallCandidateSourceReader, ReinstallCommitError,
    ReinstallCommitResult, ReinstallCommitService, ReinstallPlanPreview, ReinstallPreparation,
    ReinstallPreviewError, ReinstallPreviewRequest, ReinstallPreviewService,
    ReinstallRecoveryWriteAdmission, ReinstallTargetCounts, ReinstallTaskAuditContext,
    ReinstallTaskExecutor, ReinstallTaskExecutorService, ReinstallTaskPrepareError,
    ReinstallTaskPrepared, ReinstallTaskRunner, ReinstallTaskService, ReplacementWorkflowError,
    ReplacementWorkflowService, RetargetInstallTaskRunner, RetargetInstallTaskService,
    RetargetReinstallRequest, RetargetReinstallTaskExecutor, SaveBackupAutoSchedulerService,
    SaveBackupBackgroundService, SaveBackupBackgroundWorker, SaveBackupExecutor,
    SaveBackupExitGuard, SaveBackupService, SaveBackupTaskRunner, SaveBackupTaskScopeRegistry,
    SaveBackupTaskService, StartRecoveryActionTaskRequest, StartRetargetInstallTaskRequest,
    SupportDiagnosticsExportService, TaskManager, ThumbnailCacheMaintenanceScheduler,
    UninstallTaskRunner, UninstallTaskService, DEFAULT_PREVIEW_IMAGE_PROCESSING_CONCURRENCY,
    DEFAULT_THUMBNAIL_CACHE_MAINTENANCE_INTERVAL,
};
use hmm_core::{GameId, GameInstance, PackageFileId, PreviewImagePolicy, ReplacementBindingId};
use hmm_games_mhw::{
    MhwArmorCatalog, MhwArmorReplacementAdapter, MonsterHunterWorldAdapter,
    MonsterHunterWorldLauncher, MonsterHunterWorldSaveDirectoryRule,
};
#[cfg(not(target_os = "windows"))]
use hmm_infra::PgrepGameRunningDetector;
#[cfg(target_os = "windows")]
use hmm_infra::TasklistGameRunningDetector;
#[cfg(not(target_os = "windows"))]
use hmm_infra::UnsupportedSaveBackupBackgroundRegistry;
#[cfg(target_os = "windows")]
use hmm_infra::WindowsScheduledTaskRegistry;
use hmm_infra::{
    emit_safe_app_log, AppLogEvent, DebugLogController, DebugLogEvent,
    DiagnosticsEvidenceHealthState, FileSystemAuditLogWriter, FileSystemDiagnosticPackageExporter,
    FileSystemInstallBackupStore, FileSystemInstallGameFileSystem,
    FileSystemInstallSourceFileReader, FileSystemLogRetention, FileSystemLogStorageBudget,
    FileSystemRetargetStagingMaterializer, FileSystemSaveBackupWriter, FileSystemTaskLogWriter,
    FileSystemTextLogReader, FileSystemThumbnailStore, ImageCratePreviewImageProcessor,
    InMemoryPendingSaveDirectoryCandidateStore, JsonAppSettingsRepository,
    JsonGameConfigRepository, JsonGamePrerequisiteRuleRepository, JsonInstallManifestRepository,
    JsonInstallRecoveryRecordRepository, JsonReinstallRecoveryTransactionRepository,
    LogStorageBudgetOutcome, LogStorageBudgetReport, PlatformSteamRootProvider,
    RealGameDirectoryProbeFactory, ReqwestSteamProfileHttpTransport,
    RetargetStagingInstallSourceFileReader, SandboxModPackageInstallFileScanner,
    SandboxModPackageMetadataAnalyzer, SandboxPackagePreviewScanner, SqliteProfileRepository,
    SqliteSaveBackupBackgroundSettingsRepository, SqliteSaveBackupRepository,
    SqliteSaveBackupSchedulerStateRepository, SteamCommunityProfileClient,
    SteamGameDiscoveryService, SteamUserdataSaveDirectoryScanner, SystemClock,
    SystemDiagnosticsEnvironmentProvider, SystemGameLaunchRunner,
    TaskScopedModImportSandboxLocator, ZipModImportPackagePreparer, DEFAULT_LOG_STORAGE_MAX_BYTES,
};
use hmm_ports::{
    AppClock, AppSettingsRepository, AuditLogEvent, AuditLogReader, AuditLogWriter,
    AuditWriteFailurePolicy, DebugLogControl, DiagnosticPackageExporter,
    DiagnosticsEnvironmentProvider, DiagnosticsEvidenceHealth, GameAdapter, GameConfigRepository,
    GameLauncher, GamePrerequisiteRuleRepository, GameRunningDetector, InstallGameFileSystem,
    InstallManifestRepository, InstallSourceFileReader, ModImportResultRepository,
    ModImportSandboxLocator, ModPackageInstallFileScanner, ProfileRepository,
    ProfileSaveDirectoryValidator, ProfileSaveSettingsRepository,
    ReinstallRecoveryTransactionRepository, ReplacementAdapter, ReplacementCatalogProvider,
    SaveBackupBackgroundRegistry, SaveBackupBackgroundSettingsRepository, SaveBackupRepository,
    SaveBackupSchedulerStateRepository, SaveBackupWriter, StoredModRevision, TaskLogWriter,
    TextLogReader, ThumbnailCacheMaintenance, MIN_LOG_STORAGE_MAX_BYTES,
};
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

pub struct HmmRuntimeBuilder {
    app_data_dir: PathBuf,
    install_manifest_repository: Option<Arc<dyn InstallManifestRepository>>,
    sandbox_write_admission: Option<Arc<dyn InstallWriteAdmission>>,
}

impl HmmRuntimeBuilder {
    pub fn new(app_data_dir: PathBuf) -> Self {
        Self {
            app_data_dir,
            install_manifest_repository: None,
            sandbox_write_admission: None,
        }
    }

    #[doc(hidden)]
    pub fn with_install_manifest_repository(
        mut self,
        repository: Arc<dyn InstallManifestRepository>,
    ) -> Self {
        self.install_manifest_repository = Some(repository);
        self
    }

    pub(crate) fn with_sandbox_write_admission(
        mut self,
        admission: Arc<dyn InstallWriteAdmission>,
    ) -> Self {
        self.sandbox_write_admission = Some(admission);
        self
    }

    pub fn build(self) -> Result<HmmRuntime, String> {
        HmmRuntime::from_builder(self)
    }
}

pub struct HmmRuntime {
    pub game_setup: Arc<GameSetupService>,
    pub game_launch: Arc<GameLaunchService>,
    pub mod_library: Arc<ModLibraryService>,
    pub mod_library_query: Arc<ModLibraryQueryService>,
    pub mod_dependency_graph: Arc<ModDependencyGraphService>,
    pub preview_image_candidates: Arc<PreviewImageCandidateListService>,
    pub preview_image_selection: Arc<PreviewImageCandidateSelectionService>,
    pub preview_image_detail: Arc<PreviewImageDetailService>,
    pub preview_image_diagnostics_export: Arc<PreviewImageDiagnosticsExportService>,
    pub audit_log_diagnostics_export: Arc<AuditLogDiagnosticsExportService>,
    pub support_diagnostics_export: Arc<SupportDiagnosticsExportService>,
    pub task_log_writer: Arc<dyn TaskLogWriter>,
    pub(crate) audit_log_writer: Arc<dyn AuditLogWriter>,
    pub install_planning: Arc<InstallPlanningService>,
    pub install_preflight: Arc<ImportedModInstallPreflightService>,
    pub install_manifest_query: Arc<InstallManifestQueryService>,
    pub install_recovery_scanner: Arc<ConfiguredInstallRecoveryScanner>,
    pub install_recovery_action_previewer: Arc<ConfiguredInstallRecoveryActionPreviewer>,
    pub reinstall_executor: Arc<ConfiguredReinstallExecutor>,
    pub reinstall_task_runner: Arc<ReinstallTaskRunner<ConfiguredReinstallExecutor>>,
    pub reinstall_tasks: Arc<ReinstallTaskService>,
    #[allow(
        dead_code,
        reason = "retains the shared recovery repository for composition verification"
    )]
    pub reinstall_recovery_repository: Arc<dyn ReinstallRecoveryTransactionRepository>,
    pub install_task_runner: Arc<InstallTaskRunner>,
    pub install_tasks: Arc<InstallTaskService>,
    pub replacement_workflow: Arc<ReplacementWorkflowService>,
    pub initial_retarget_install_preflight: Arc<InitialRetargetInstallPreflightService>,
    pub retarget_install_task_runner: Arc<RetargetInstallTaskRunner>,
    pub retarget_install_tasks: Arc<RetargetInstallTaskService>,
    pub recovery_action_task_runner: Arc<RecoveryActionTaskRunner>,
    pub recovery_action_tasks: Arc<RecoveryActionTaskService>,
    pub uninstall_task_runner: Arc<UninstallTaskRunner>,
    pub uninstall_tasks: Arc<UninstallTaskService>,
    pub mod_import_task_runner: Arc<ModImportTaskRunner>,
    pub mod_import_tasks: Arc<ModImportTaskService>,
    pub external_import: crate::external_import::ExternalImportComposition,
    pub app_settings: Arc<AppSettingsService>,
    pub debug_log: Arc<DebugLogController>,
    pub mod_metadata: Arc<ModMetadataService>,
    pub categories: Arc<CategoryService>,
    pub profiles: Arc<ProfileService>,
    pub save_directory_discovery: Arc<ProfileSaveDirectoryDiscoveryService>,
    pub save_backups: Arc<SaveBackupService>,
    pub save_backup_auto_scheduler: Arc<SaveBackupAutoSchedulerService>,
    pub save_backup_background: Arc<SaveBackupBackgroundService>,
    pub save_backup_background_worker: Arc<SaveBackupBackgroundWorker>,
    pub save_backup_exit_guard: Arc<SaveBackupExitGuard>,
    pub save_backup_task_runner: Arc<SaveBackupTaskRunner>,
    pub save_backup_tasks: Arc<SaveBackupTaskService>,
    pub task_manager: Arc<TaskManager>,
    db: Arc<Mutex<rusqlite::Connection>>,
}

impl HmmRuntime {
    pub fn builder(app_data_dir: PathBuf) -> HmmRuntimeBuilder {
        HmmRuntimeBuilder::new(app_data_dir)
    }

    pub fn from_app_data_dir(app_data_dir: PathBuf) -> Result<Self, String> {
        Self::builder(app_data_dir).build()
    }

    /// Returns the process-local database handle for consumers that must observe the same
    /// SQLite WAL connection as the GUI. Callers still go through repositories; this only
    /// avoids opening an immutable snapshot while the GUI owns an active WAL.
    pub fn database_handle(&self) -> Arc<Mutex<rusqlite::Connection>> {
        Arc::clone(&self.db)
    }

    pub(crate) fn audit_log_writer(&self) -> Arc<dyn AuditLogWriter> {
        Arc::clone(&self.audit_log_writer)
    }

    fn from_builder(builder: HmmRuntimeBuilder) -> Result<Self, String> {
        let HmmRuntimeBuilder {
            app_data_dir,
            install_manifest_repository,
            sandbox_write_admission,
        } = builder;
        let config_path = app_data_dir.join("config").join("games.json");
        let settings_path = app_data_dir.join("config").join("settings.json");
        let mod_import_results_path = app_data_dir.join("mod-import").join("results.json");

        let db_path = app_data_dir.join("hmm.db");
        let db = hmm_infra::open_database(&db_path)
            .map_err(|error| format!("failed to open database: {error}"))?;
        let db = Arc::new(Mutex::new(db));
        let mod_library_composition = ModLibraryComposition::new(&db, mod_import_results_path)?;
        let mod_metadata_repository = mod_library_composition.mod_metadata_repository();
        let category_repository = mod_library_composition.category_repository();
        let mod_import_result_repository = mod_library_composition.mod_import_result_repository();
        let profile_repository = Arc::new(SqliteProfileRepository::new(Arc::clone(&db)));
        let profile_repository_for_profiles: Arc<dyn ProfileRepository> =
            profile_repository.clone();
        let profile_repository_for_save_directory_discovery: Arc<dyn ProfileRepository> =
            profile_repository.clone();
        let profile_repository_for_save_backups: Arc<dyn ProfileRepository> =
            profile_repository.clone();
        let profile_repository_for_save_backup_auto_scheduler: Arc<dyn ProfileRepository> =
            profile_repository.clone();
        let profile_repository_for_save_backup_background_worker: Arc<dyn ProfileRepository> =
            profile_repository.clone();
        let profile_repository_for_save_backup_exit_guard: Arc<dyn ProfileRepository> =
            profile_repository.clone();
        let profile_save_settings_repository: Arc<dyn ProfileSaveSettingsRepository> =
            profile_repository.clone();
        let profile_save_settings_repository_for_save_directory_discovery: Arc<
            dyn ProfileSaveSettingsRepository,
        > = profile_repository.clone();
        let profile_save_settings_repository_for_save_backups: Arc<
            dyn ProfileSaveSettingsRepository,
        > = profile_repository.clone();
        let profile_save_settings_repository_for_save_backup_auto_scheduler: Arc<
            dyn ProfileSaveSettingsRepository,
        > = profile_repository.clone();
        let profile_save_settings_repository_for_save_backup_background_worker: Arc<
            dyn ProfileSaveSettingsRepository,
        > = profile_repository.clone();
        let profile_save_settings_repository_for_save_backup_exit_guard: Arc<
            dyn ProfileSaveSettingsRepository,
        > = profile_repository.clone();
        let profile_save_directory_validator: Arc<dyn ProfileSaveDirectoryValidator> =
            profile_repository.clone();
        let profile_save_directory_validator_for_save_directory_discovery: Arc<
            dyn ProfileSaveDirectoryValidator,
        > = profile_repository.clone();
        let profile_save_directory_validator_for_save_backups: Arc<
            dyn ProfileSaveDirectoryValidator,
        > = profile_repository.clone();
        let save_backup_repository: Arc<dyn SaveBackupRepository> =
            Arc::new(SqliteSaveBackupRepository::new(Arc::clone(&db)));
        let save_backup_scheduler_state_repository: Arc<dyn SaveBackupSchedulerStateRepository> =
            Arc::new(SqliteSaveBackupSchedulerStateRepository::new(Arc::clone(
                &db,
            )));
        let save_backup_background_settings_repository = Arc::new(
            SqliteSaveBackupBackgroundSettingsRepository::new(Arc::clone(&db)),
        );
        let settings_for_service: Arc<dyn SaveBackupBackgroundSettingsRepository> =
            save_backup_background_settings_repository.clone();
        let settings_for_worker: Arc<dyn SaveBackupBackgroundSettingsRepository> =
            save_backup_background_settings_repository;
        let save_backup_writer: Arc<dyn SaveBackupWriter> =
            Arc::new(FileSystemSaveBackupWriter::new(app_data_dir.clone()));

        let task_manager = Arc::new(TaskManager::new());
        let mhw_prerequisite_rules: Arc<dyn GamePrerequisiteRuleRepository> =
            Arc::new(JsonGamePrerequisiteRuleRepository::new(
                app_data_dir
                    .join("config")
                    .join("prerequisite-rules")
                    .join("mhw.json"),
            ));
        let mhw_adapter: Arc<dyn GameAdapter> =
            Arc::new(MonsterHunterWorldAdapter::new(mhw_prerequisite_rules));
        let game_adapters: Vec<Arc<dyn GameAdapter>> = vec![Arc::clone(&mhw_adapter)];
        let game_ids = game_ids_from_adapters(&game_adapters);
        let mhw_launcher: Arc<dyn GameLauncher> = Arc::new(MonsterHunterWorldLauncher::new(
            Arc::new(SystemGameLaunchRunner),
        ));
        let game_config_repository: Arc<dyn GameConfigRepository> =
            Arc::new(JsonGameConfigRepository::new(config_path));
        let game_setup = Arc::new(GameSetupService::new(
            clone_game_adapters(&game_adapters),
            Arc::clone(&game_config_repository),
            Arc::new(RealGameDirectoryProbeFactory),
            Arc::new(SteamGameDiscoveryService::new(Arc::new(
                PlatformSteamRootProvider,
            ))),
            Arc::new(SystemClock),
        ));
        let mod_import_sandbox_locator: Arc<dyn ModImportSandboxLocator> = Arc::new(
            TaskScopedModImportSandboxLocator::new_in_app_data(app_data_dir.clone()),
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
                game_ids
                    .iter()
                    .map(|game_id| game_id.as_str().to_owned())
                    .collect(),
            ));
        let evidence_health: Arc<dyn DiagnosticsEvidenceHealth> =
            Arc::new(DiagnosticsEvidenceHealthState::default());
        let app_settings_repository: Arc<dyn AppSettingsRepository> =
            Arc::new(JsonAppSettingsRepository::new(settings_path));
        let (log_storage_max_bytes, log_storage_settings_degraded, debug_log_enabled) =
            resolve_log_settings(app_settings_repository.as_ref(), evidence_health.as_ref());
        let debug_log = Arc::new(DebugLogController::new(
            app_data_dir.clone(),
            debug_log_enabled,
            Arc::clone(&evidence_health),
        ));
        let log_retention =
            FileSystemLogRetention::new(app_data_dir.clone(), Arc::clone(&evidence_health));
        let log_storage_budget =
            FileSystemLogStorageBudget::new(app_data_dir.clone(), Arc::clone(&evidence_health));
        let log_storage_maintenance = match SystemClock.now_unix_millis() {
            Ok(timestamp_unix_millis) => {
                log_retention.run_at(timestamp_unix_millis);
                Some((
                    timestamp_unix_millis,
                    log_storage_budget.run_at(
                        timestamp_unix_millis,
                        log_storage_max_bytes,
                        log_storage_settings_degraded,
                    ),
                ))
            }
            Err(_) => {
                evidence_health.record_debug_log_retention_failure();
                evidence_health.record_task_log_retention_failure();
                evidence_health.record_audit_log_retention_failure();
                evidence_health.record_log_storage_budget_failure();
                None
            }
        };
        let task_log_writer: Arc<dyn TaskLogWriter> = Arc::new(FileSystemTaskLogWriter::new(
            app_data_dir.clone(),
            Arc::clone(&evidence_health),
        ));
        let file_system_audit_log = Arc::new(FileSystemAuditLogWriter::with_health(
            app_data_dir.clone(),
            Arc::clone(&evidence_health),
        ));
        let audit_log_writer: Arc<dyn AuditLogWriter> = file_system_audit_log.clone();
        let audit_log_reader: Arc<dyn AuditLogReader> = file_system_audit_log;
        if let Some((timestamp_unix_millis, report)) = log_storage_maintenance {
            record_log_storage_budget_maintenance(
                audit_log_writer.as_ref(),
                timestamp_unix_millis,
                report,
                log_storage_settings_degraded,
            );
        }
        let save_backup_background_clock: Arc<dyn AppClock> = Arc::new(SystemClock);
        let save_backup_background_registry: Arc<dyn SaveBackupBackgroundRegistry> = {
            #[cfg(target_os = "windows")]
            {
                Arc::new(WindowsScheduledTaskRegistry::from_current_exe())
            }
            #[cfg(not(target_os = "windows"))]
            {
                Arc::new(UnsupportedSaveBackupBackgroundRegistry)
            }
        };
        let save_backup_background =
            Arc::new(SaveBackupBackgroundService::new_with_settings_repository(
                save_backup_background_registry,
                Arc::clone(&save_backup_scheduler_state_repository),
                settings_for_service,
                Arc::clone(&audit_log_writer),
                Arc::clone(&save_backup_background_clock),
            ));
        let save_backup_exit_guard = Arc::new(SaveBackupExitGuard::new(
            profile_repository_for_save_backup_exit_guard,
            profile_save_settings_repository_for_save_backup_exit_guard,
            Arc::clone(&save_backup_background),
            Arc::clone(&audit_log_writer),
            Arc::clone(&save_backup_background_clock),
        ));
        let install_manifest_repository = mod_library_composition.install_manifest_repository(
            install_manifest_repository_for(&app_data_dir, install_manifest_repository),
        );
        let reinstall_recovery_repository: Arc<dyn ReinstallRecoveryTransactionRepository> =
            Arc::new(JsonReinstallRecoveryTransactionRepository::new(
                app_data_dir.join("install").join("reinstall-recovery"),
            ));
        let debug_log_control: Arc<dyn DebugLogControl> = debug_log.clone();
        let app_settings = Arc::new(AppSettingsService::new_with_debug_log_control(
            Arc::clone(&app_settings_repository),
            debug_log_control,
        ));
        let _ = debug_log.record(
            DebugLogEvent::new("runtime.initialized")
                .with_component("runtime")
                .with_operation("composition")
                .with_result("success"),
        );
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
            Box::new(ZipModImportPackagePreparer::new_in_app_data(
                app_data_dir.clone(),
            )),
            ModImportAnalysisService::new(
                Box::new(preview_image_service),
                Box::new(FileSystemThumbnailStore::new(app_data_dir.clone())),
                Box::new(SandboxModPackageMetadataAnalyzer),
            ),
        ));
        let external_import = crate::external_import::compose(
            &app_data_dir,
            &db,
            &task_manager,
            Arc::clone(&mod_import_result_repository),
            Arc::clone(&category_repository),
            Arc::clone(&mod_import_sandbox_locator),
            Arc::clone(&mod_import_prepare_service),
        )?;
        let mod_library = mod_library_composition.library_service();
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
        let support_diagnostics_export =
            Arc::new(SupportDiagnosticsExportService::new_with_health(
                text_log_reader,
                audit_log_reader,
                diagnostics_environment_provider,
                diagnostic_package_exporter,
                Arc::clone(&audit_log_writer),
                Arc::new(SystemClock),
                evidence_health,
            ));
        let save_backups = Arc::new(SaveBackupService::new(
            profile_repository_for_save_backups,
            profile_save_settings_repository_for_save_backups,
            profile_save_directory_validator_for_save_backups,
            Arc::clone(&save_backup_repository),
            save_backup_writer,
            Arc::new(SystemClock),
        ));
        let save_backup_auto_scheduler = Arc::new(SaveBackupAutoSchedulerService::new(
            profile_repository_for_save_backup_auto_scheduler,
            profile_save_settings_repository_for_save_backup_auto_scheduler,
            Arc::clone(&save_backup_repository),
            Arc::clone(&save_backup_scheduler_state_repository),
            game_running_detector_for_platform(&game_adapters),
            Arc::clone(&audit_log_writer),
            Arc::new(SystemClock),
        ));
        let save_directory_discovery = Arc::new(ProfileSaveDirectoryDiscoveryService::new(
            Arc::clone(&game_config_repository),
            profile_repository_for_save_directory_discovery,
            profile_save_settings_repository_for_save_directory_discovery,
            profile_save_directory_validator_for_save_directory_discovery,
            vec![Arc::new(MonsterHunterWorldSaveDirectoryRule)],
            Arc::new(SteamUserdataSaveDirectoryScanner::new(Arc::new(
                PlatformSteamRootProvider,
            ))),
            Arc::new(SteamCommunityProfileClient::new(Box::new(
                ReqwestSteamProfileHttpTransport,
            ))),
            Arc::new(InMemoryPendingSaveDirectoryCandidateStore::default()),
            Arc::new(SystemClock),
        ));
        let save_backup_executor: Arc<dyn SaveBackupExecutor> = save_backups.clone();
        let save_backup_task_scopes = Arc::new(SaveBackupTaskScopeRegistry::default());
        let save_backup_task_runner = Arc::new(
            SaveBackupTaskRunner::with_scope_registry_and_scheduler_state(
                Arc::clone(&task_manager),
                save_backup_executor,
                Arc::clone(&audit_log_writer),
                Arc::new(SystemClock),
                Arc::clone(&save_backup_task_scopes),
                Arc::clone(&save_backup_scheduler_state_repository),
            ),
        );
        let save_backup_tasks = Arc::new(SaveBackupTaskService::with_scope_registry(
            Arc::clone(&task_manager),
            save_backup_task_scopes,
        ));
        let save_backup_background_worker =
            Arc::new(SaveBackupBackgroundWorker::new_with_settings_repository(
                game_ids,
                profile_repository_for_save_backup_background_worker,
                profile_save_settings_repository_for_save_backup_background_worker,
                Arc::clone(&save_backup_auto_scheduler),
                Arc::clone(&save_backup_tasks),
                Arc::clone(&save_backup_task_runner),
                Arc::clone(&save_backup_scheduler_state_repository),
                settings_for_worker,
                Arc::clone(&audit_log_writer),
                save_backup_background_clock,
            ));
        let install_file_scanner: Arc<dyn ModPackageInstallFileScanner> =
            Arc::new(SandboxModPackageInstallFileScanner);
        let install_planning = Arc::new(InstallPlanningService::with_imported_mod_sources(
            Arc::clone(&mod_import_result_repository),
            Arc::clone(&mod_import_sandbox_locator),
            Arc::clone(&install_file_scanner),
            clone_game_adapters(&game_adapters),
        ));
        let prerequisites: Arc<dyn GamePrerequisiteDecisionProvider> = game_setup.clone();
        let install_preflight = Arc::new(ImportedModInstallPreflightService::new(
            Arc::clone(&install_planning),
            Arc::clone(&prerequisites),
        ));
        let install_manifest_query = Arc::new(InstallManifestQueryService::new(Arc::clone(
            &install_manifest_repository,
        )));
        let mod_library_query = mod_library_composition
            .query_service(Arc::clone(&mod_library), install_manifest_query.clone());
        let install_write_locks = Arc::new(GameProfileWriteLockRegistry::default());
        let install_recovery_scanner = Arc::new(ConfiguredInstallRecoveryScanner::new(
            Arc::clone(&game_config_repository),
            app_data_dir.clone(),
            Arc::clone(&install_write_locks),
            Arc::clone(&reinstall_recovery_repository),
        ));
        let initial_retarget_install_status: Arc<dyn InitialRetargetInstallStatusReader> =
            install_recovery_scanner.clone();
        let replacement_adapters: Vec<Arc<dyn ReplacementAdapter>> =
            vec![Arc::new(MhwArmorReplacementAdapter)];
        let replacement_catalogs: Vec<Arc<dyn ReplacementCatalogProvider>> =
            vec![Arc::new(MhwArmorCatalog)];
        let replacement_workflow = Arc::new(ReplacementWorkflowService::new(
            replacement_adapters,
            replacement_catalogs,
            Arc::clone(&mod_import_result_repository),
            Arc::clone(&mod_import_sandbox_locator),
            install_file_scanner,
            initial_retarget_install_status,
            Arc::new(SystemClock),
        ));
        let initial_retarget_install_preflight =
            Arc::new(InitialRetargetInstallPreflightService::new(
                Arc::clone(&replacement_workflow),
                Arc::clone(&prerequisites),
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
        let mod_uninstaller = crate::uninstall::mod_uninstaller(
            Arc::clone(&game_config_repository),
            app_data_dir.clone(),
        );
        let recovery_action_executor: Arc<dyn InstallRecoveryActionExecutor> =
            Arc::new(ConfiguredInstallRecoveryActionExecutor::new(
                Arc::clone(&game_config_repository),
                app_data_dir.clone(),
                Arc::clone(&reinstall_recovery_repository),
            ));
        let sandbox_write_admission: Arc<dyn InstallWriteAdmission> =
            sandbox_write_admission.unwrap_or_else(|| Arc::new(AllowRuntimeWriteAdmission));
        let reinstall_write_admission: Arc<dyn InstallWriteAdmission> = Arc::new(
            ReinstallRecoveryWriteAdmission::new(Arc::clone(&reinstall_recovery_repository)),
        );
        let lifecycle_write_admission: Arc<dyn InstallWriteAdmission> =
            Arc::new(ChainedInstallWriteAdmission::new(
                reinstall_write_admission,
                Arc::clone(&sandbox_write_admission),
            ));
        let install_task_runner = Arc::new(InstallTaskRunner::with_write_coordination(
            Arc::clone(&task_manager),
            install_preflight.clone(),
            Arc::clone(&install_committer),
            Arc::clone(&audit_log_writer),
            Arc::new(SystemClock),
            Arc::clone(&install_write_locks),
            Arc::clone(&lifecycle_write_admission),
        ));
        let retarget_install_planner: Arc<dyn InitialRetargetInstallPlanner> =
            Arc::new(ConfiguredInitialRetargetInstallPlanner::new(
                Arc::clone(&replacement_workflow),
                Arc::clone(&prerequisites),
                Arc::clone(&mod_import_sandbox_locator),
                Arc::clone(&install_recovery_scanner),
                app_data_dir.clone(),
            ));
        let retarget_install_task_runner =
            Arc::new(RetargetInstallTaskRunner::with_write_coordination(
                Arc::clone(&task_manager),
                retarget_install_planner,
                install_committer,
                Arc::clone(&audit_log_writer),
                Arc::new(SystemClock),
                Arc::clone(&install_write_locks),
                Arc::clone(&lifecycle_write_admission),
            ));
        let recovery_action_task_runner =
            Arc::new(RecoveryActionTaskRunner::with_write_coordination(
                Arc::clone(&task_manager),
                recovery_action_executor,
                Arc::clone(&audit_log_writer),
                Arc::new(SystemClock),
                Arc::clone(&install_write_locks),
                Arc::clone(&sandbox_write_admission),
            ));
        let uninstall_task_runner = Arc::new(UninstallTaskRunner::with_write_coordination(
            Arc::clone(&task_manager),
            mod_uninstaller,
            Arc::clone(&audit_log_writer),
            Arc::new(SystemClock),
            Arc::clone(&install_write_locks),
            lifecycle_write_admission,
        ));
        let reinstall_executor = Arc::new(ConfiguredReinstallExecutor::new(
            Arc::clone(&game_config_repository),
            Arc::clone(&prerequisites),
            Arc::clone(&mod_import_result_repository),
            Arc::clone(&mod_import_sandbox_locator),
            install_planning.clone(),
            Arc::clone(&install_manifest_repository),
            Arc::clone(&reinstall_recovery_repository),
            Arc::clone(&replacement_workflow),
            app_data_dir.clone(),
        ));
        let reinstall_task_runner = Arc::new(ReinstallTaskRunner::with_write_coordination(
            Arc::clone(&task_manager),
            Arc::clone(&reinstall_executor),
            Arc::clone(&audit_log_writer),
            Arc::new(SystemClock),
            install_write_locks,
            sandbox_write_admission,
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
        let state = Self {
            game_setup,
            game_launch: Arc::new(GameLaunchService::new(
                vec![mhw_launcher],
                Arc::clone(&game_config_repository),
            )),
            mod_library,
            mod_library_query,
            mod_dependency_graph,
            preview_image_candidates,
            preview_image_selection,
            preview_image_detail,
            preview_image_diagnostics_export,
            audit_log_diagnostics_export,
            support_diagnostics_export,
            task_log_writer,
            audit_log_writer: Arc::clone(&audit_log_writer),
            install_planning,
            install_preflight,
            install_manifest_query,
            install_recovery_scanner,
            install_recovery_action_previewer,
            reinstall_executor,
            reinstall_task_runner,
            reinstall_tasks: Arc::new(ReinstallTaskService::new(Arc::clone(&task_manager))),
            reinstall_recovery_repository,
            install_task_runner,
            install_tasks: Arc::new(InstallTaskService::new(Arc::clone(&task_manager))),
            replacement_workflow,
            initial_retarget_install_preflight,
            retarget_install_task_runner,
            retarget_install_tasks: Arc::new(RetargetInstallTaskService::new(Arc::clone(
                &task_manager,
            ))),
            recovery_action_task_runner,
            recovery_action_tasks: Arc::new(RecoveryActionTaskService::new(Arc::clone(
                &task_manager,
            ))),
            uninstall_task_runner,
            uninstall_tasks: Arc::new(UninstallTaskService::new(Arc::clone(&task_manager))),
            mod_import_task_runner,
            mod_import_tasks: Arc::new(ModImportTaskService::new(Arc::clone(&task_manager))),
            external_import,
            app_settings,
            debug_log,
            mod_metadata: Arc::new(ModMetadataService::new(
                mod_metadata_repository,
                Arc::new(SystemClock),
            )),
            categories: Arc::new(CategoryService::new(
                category_repository,
                Arc::new(SystemClock),
            )),
            profiles: Arc::new(ProfileService::new(
                profile_repository_for_profiles,
                profile_save_settings_repository,
                profile_save_directory_validator,
                Arc::new(SystemClock),
            )),
            save_directory_discovery,
            save_backup_task_runner,
            save_backup_tasks,
            save_backup_auto_scheduler,
            save_backup_background,
            save_backup_background_worker,
            save_backup_exit_guard,
            save_backups,
            task_manager,
            db,
        };

        Ok(state)
    }

    pub fn start_thumbnail_cache_maintenance(&self) {
        let thumbnail_cache_scheduler = ThumbnailCacheMaintenanceScheduler::new(
            Arc::clone(&self.mod_import_task_runner),
            DEFAULT_THUMBNAIL_CACHE_MAINTENANCE_INTERVAL,
        );
        start_best_effort_background_task("thumbnail-cache-maintenance", || {
            std::thread::Builder::new()
                .name("thumbnail-cache-maintenance".to_owned())
                .spawn(move || thumbnail_cache_scheduler.run_forever())
                .map(|_| ())
        });
    }
}

struct ChainedInstallWriteAdmission {
    first: Arc<dyn InstallWriteAdmission>,
    second: Arc<dyn InstallWriteAdmission>,
}

impl ChainedInstallWriteAdmission {
    fn new(first: Arc<dyn InstallWriteAdmission>, second: Arc<dyn InstallWriteAdmission>) -> Self {
        Self { first, second }
    }
}

impl InstallWriteAdmission for ChainedInstallWriteAdmission {
    fn ensure_write_allowed(
        &self,
        game_id: &GameId,
        profile_id: &hmm_core::ProfileId,
    ) -> Result<(), InstallWriteAdmissionError> {
        self.first.ensure_write_allowed(game_id, profile_id)?;
        self.second.ensure_write_allowed(game_id, profile_id)
    }

    fn ensure_install_plan_allowed(
        &self,
        game_id: &GameId,
        profile_id: &hmm_core::ProfileId,
        mod_id: &hmm_core::ModId,
        plan: &hmm_core::InstallPlan,
        prerequisite_decision: &GamePrerequisiteDecision,
    ) -> Result<(), InstallWriteAdmissionError> {
        self.first.ensure_install_plan_allowed(
            game_id,
            profile_id,
            mod_id,
            plan,
            prerequisite_decision,
        )?;
        self.second.ensure_install_plan_allowed(
            game_id,
            profile_id,
            mod_id,
            plan,
            prerequisite_decision,
        )
    }
}

struct AllowRuntimeWriteAdmission;

impl InstallWriteAdmission for AllowRuntimeWriteAdmission {
    fn ensure_write_allowed(
        &self,
        _game_id: &GameId,
        _profile_id: &hmm_core::ProfileId,
    ) -> Result<(), InstallWriteAdmissionError> {
        Ok(())
    }
}

fn resolve_log_settings(
    repository: &dyn AppSettingsRepository,
    health: &dyn DiagnosticsEvidenceHealth,
) -> (u64, bool, bool) {
    match repository.load_settings() {
        Ok(settings) => {
            let (max_bytes, degraded) = match settings.log_storage_max_bytes {
                None => (DEFAULT_LOG_STORAGE_MAX_BYTES, false),
                Some(max_bytes) if max_bytes >= MIN_LOG_STORAGE_MAX_BYTES => (max_bytes, false),
                Some(_) => {
                    health.record_log_storage_settings_failure();
                    (DEFAULT_LOG_STORAGE_MAX_BYTES, true)
                }
            };
            (max_bytes, degraded, settings.debug_log_enabled)
        }
        Err(_) => {
            health.record_log_storage_settings_failure();
            (DEFAULT_LOG_STORAGE_MAX_BYTES, true, false)
        }
    }
}

fn record_log_storage_budget_maintenance(
    audit_log_writer: &dyn AuditLogWriter,
    timestamp_unix_millis: u128,
    report: LogStorageBudgetReport,
    settings_degraded: bool,
) {
    let should_record = settings_degraded
        || report.deleted_file_count > 0
        || matches!(
            report.outcome,
            LogStorageBudgetOutcome::Unsatisfied | LogStorageBudgetOutcome::Failed
        );
    if !should_record {
        return;
    }

    let result = match report.outcome {
        LogStorageBudgetOutcome::Failed => "failed",
        _ if settings_degraded => "degraded",
        LogStorageBudgetOutcome::Unsatisfied => "degraded",
        LogStorageBudgetOutcome::WithinBudget | LogStorageBudgetOutcome::ReducedToBudget => {
            "success"
        }
    };
    let fields = BTreeMap::from([
        ("outcome".to_owned(), report.outcome.code().to_owned()),
        ("max_bytes".to_owned(), report.max_bytes.to_string()),
        (
            "owned_bytes_after".to_owned(),
            report.owned_bytes_after.to_string(),
        ),
        (
            "deleted_file_count".to_owned(),
            report.deleted_file_count.to_string(),
        ),
        ("deleted_bytes".to_owned(), report.deleted_bytes.to_string()),
        (
            "failed_category_count".to_owned(),
            report.failed_category_count.to_string(),
        ),
        (
            "settings_status".to_owned(),
            if settings_degraded { "degraded" } else { "ok" }.to_owned(),
        ),
    ]);
    let _ = audit_log_writer.record_with_policy(
        AuditLogEvent {
            timestamp_unix_millis,
            category: "log_storage".to_owned(),
            operation: "log_storage_budget_maintenance".to_owned(),
            result: result.to_owned(),
            fields,
        },
        AuditWriteFailurePolicy::BestEffort,
    );
}

fn install_manifest_repository_for(
    app_data_dir: &Path,
    override_repository: Option<Arc<dyn InstallManifestRepository>>,
) -> Arc<dyn InstallManifestRepository> {
    override_repository.unwrap_or_else(|| {
        Arc::new(JsonInstallManifestRepository::new(
            app_data_dir.join("install").join("manifests"),
        ))
    })
}

struct ConfiguredReinstallCandidateSourceReader {
    sandbox_locator: Arc<dyn ModImportSandboxLocator>,
}

struct RetargetStagingReinstallCandidateSourceReader {
    reader: RetargetStagingInstallSourceFileReader,
}

impl ReinstallCandidateSourceReader for RetargetStagingReinstallCandidateSourceReader {
    fn read_candidate_source_file(
        &self,
        _candidate: &StoredModRevision,
        package_file_id: &PackageFileId,
    ) -> anyhow::Result<Vec<u8>> {
        self.reader.read_source_file(package_file_id)
    }
}

impl ConfiguredReinstallCandidateSourceReader {
    fn new(sandbox_locator: Arc<dyn ModImportSandboxLocator>) -> Self {
        Self { sandbox_locator }
    }
}

impl ReinstallCandidateSourceReader for ConfiguredReinstallCandidateSourceReader {
    fn read_candidate_source_file(
        &self,
        candidate: &StoredModRevision,
        package_file_id: &PackageFileId,
    ) -> anyhow::Result<Vec<u8>> {
        let source_root = self
            .sandbox_locator
            .sandbox_root_for_package(&candidate.package_id)?;
        FileSystemInstallSourceFileReader::new(source_root).read_source_file(package_file_id)
    }
}

struct ConfiguredReinstallServices {
    game_instance: GameInstance,
    preview: Arc<ReinstallPreviewService>,
    executor: ReinstallTaskExecutorService,
}

pub struct ConfiguredPreparedReinstall {
    prepared: PreparedReinstall,
    game_instance: GameInstance,
    source: Arc<dyn ReinstallCandidateSourceReader>,
    staging_cleanup: RetargetStagingCleanup,
}

impl ReinstallTaskPrepared for ConfiguredPreparedReinstall {
    fn audit_context(&self) -> ReinstallTaskAuditContext {
        self.prepared.audit_context()
    }

    fn plan_token(&self) -> &str {
        self.prepared.plan_token()
    }

    fn batch_plan_digest(&self) -> String {
        self.prepared.batch_plan_digest()
    }
}

pub struct ConfiguredReinstallExecutor {
    game_config_repository: Arc<dyn GameConfigRepository>,
    prerequisites: Arc<dyn GamePrerequisiteDecisionProvider>,
    catalog: Arc<dyn ModImportResultRepository>,
    planner: Arc<InstallPlanningService>,
    source: Arc<dyn ReinstallCandidateSourceReader>,
    sandbox_locator: Arc<dyn ModImportSandboxLocator>,
    manifest_repository: Arc<dyn InstallManifestRepository>,
    recovery_repository: Arc<dyn ReinstallRecoveryTransactionRepository>,
    replacement_workflow: Arc<ReplacementWorkflowService>,
    app_data_dir: PathBuf,
}

#[derive(Debug)]
pub enum ConfiguredRetargetReinstallError {
    Reinstall(ReinstallPreviewError),
    Replacement(ReplacementWorkflowError),
}

struct ConfiguredRetargetReinstallPreparation {
    preparation: ReinstallPreparation,
    game_instance: GameInstance,
    source: Arc<dyn ReinstallCandidateSourceReader>,
    staging_cleanup: RetargetStagingCleanup,
}

#[derive(Default)]
struct RetargetStagingCleanup {
    staging_root: Option<PathBuf>,
}

impl RetargetStagingCleanup {
    fn armed(staging_root: PathBuf) -> Self {
        Self {
            staging_root: Some(staging_root),
        }
    }
}

impl Drop for RetargetStagingCleanup {
    fn drop(&mut self) {
        if let Some(staging_root) = self.staging_root.take() {
            discard_retarget_staging(&staging_root);
        }
    }
}

impl ConfiguredReinstallExecutor {
    #[allow(clippy::too_many_arguments)]
    fn new(
        game_config_repository: Arc<dyn GameConfigRepository>,
        prerequisites: Arc<dyn GamePrerequisiteDecisionProvider>,
        catalog: Arc<dyn ModImportResultRepository>,
        sandbox_locator: Arc<dyn ModImportSandboxLocator>,
        planner: Arc<InstallPlanningService>,
        manifest_repository: Arc<dyn InstallManifestRepository>,
        recovery_repository: Arc<dyn ReinstallRecoveryTransactionRepository>,
        replacement_workflow: Arc<ReplacementWorkflowService>,
        app_data_dir: PathBuf,
    ) -> Self {
        let source = Arc::new(ConfiguredReinstallCandidateSourceReader::new(Arc::clone(
            &sandbox_locator,
        )));
        Self {
            game_config_repository,
            prerequisites,
            catalog,
            planner,
            source,
            sandbox_locator,
            manifest_repository,
            recovery_repository,
            replacement_workflow,
            app_data_dir,
        }
    }

    pub fn preview(
        &self,
        request: ReinstallPreviewRequest,
    ) -> Result<ReinstallPlanPreview, ReinstallPreviewError> {
        let services = self.services_for(&request.game_id)?;
        services.preview.preview(request)
    }

    pub fn preview_retarget_reinstall(
        &self,
        request: RetargetReinstallRequest,
    ) -> Result<ReinstallPlanPreview, ConfiguredRetargetReinstallError> {
        let prepared = self.prepare_retarget_reinstall(request)?;
        Ok(prepared.preparation.into_preview())
    }

    fn prepare_retarget_reinstall(
        &self,
        request: RetargetReinstallRequest,
    ) -> Result<ConfiguredRetargetReinstallPreparation, ConfiguredRetargetReinstallError> {
        let services = self
            .services_for(&request.game_id)
            .map_err(ConfiguredRetargetReinstallError::Reinstall)?;
        let context = services
            .preview
            .resolve_installed_replacement_context(
                &request.game_id,
                &request.profile_id,
                &request.mod_id,
            )
            .map_err(ConfiguredRetargetReinstallError::Reinstall)?;
        let context = match context {
            InstalledReplacementReinstallResolution::Ready(context) => context,
            InstalledReplacementReinstallResolution::Blocked(preview) => {
                return Ok(ConfiguredRetargetReinstallPreparation {
                    preparation: ReinstallPreparation::Blocked(preview),
                    game_instance: services.game_instance,
                    source: Arc::clone(&self.source),
                    staging_cleanup: RetargetStagingCleanup::default(),
                });
            }
        };
        let planned = self
            .replacement_workflow
            .preview_reinstall_target(PreviewRetargetReinstallRequest {
                game_id: request.game_id.clone(),
                profile_id: request.profile_id.clone(),
                mod_id: request.mod_id.clone(),
                installed_revision_id: context.installed_revision_id.clone(),
                installed_binding: context.installed_binding,
                target_id: request.target_id,
                layer: request.layer.clone(),
            })
            .map_err(ConfiguredRetargetReinstallError::Replacement)?;
        let source_root = self
            .sandbox_locator
            .sandbox_root_for_package(planned.package_id())
            .map_err(|_| {
                ConfiguredRetargetReinstallError::Replacement(
                    ReplacementWorkflowError::SandboxUnavailable,
                )
            })?;
        let staging_root = retarget_reinstall_staging_root(&self.app_data_dir);
        let materializer = FileSystemRetargetStagingMaterializer::new(
            staging_root.clone(),
            Arc::new(FileSystemInstallSourceFileReader::new(source_root)),
        );
        let plan = self
            .replacement_workflow
            .materialize_reinstall_target(&materializer, planned)
            .map_err(ConfiguredRetargetReinstallError::Replacement)?;
        let staging_cleanup = RetargetStagingCleanup::armed(staging_root.clone());
        let source: Arc<dyn ReinstallCandidateSourceReader> =
            match RetargetStagingInstallSourceFileReader::from_install_plan(
                staging_root.clone(),
                &plan,
            ) {
                Ok(reader) => Arc::new(RetargetStagingReinstallCandidateSourceReader { reader }),
                Err(_) => {
                    return Err(ConfiguredRetargetReinstallError::Replacement(
                        ReplacementWorkflowError::PlanUnavailable,
                    ));
                }
            };
        let candidate_request = ReinstallPreviewRequest {
            game_id: request.game_id,
            profile_id: request.profile_id,
            mod_id: request.mod_id,
            candidate_revision_id: context.installed_revision_id,
            layer: request.layer,
        };
        let candidate_services = self.services_for_game_instance_with_source(
            services.game_instance.clone(),
            Arc::clone(&source),
        );
        let preparation = candidate_services
            .preview
            .prepare_replacement_target_switch(candidate_request, plan)
            .map_err(ConfiguredRetargetReinstallError::Reinstall)?;
        Ok(ConfiguredRetargetReinstallPreparation {
            preparation,
            game_instance: services.game_instance,
            source,
            staging_cleanup,
        })
    }

    fn services_for(
        &self,
        game_id: &GameId,
    ) -> Result<ConfiguredReinstallServices, ReinstallPreviewError> {
        let game_instance = self
            .game_config_repository
            .load_game_instance(game_id)
            .map_err(|_| ReinstallPreviewError::CandidatePlanUnavailable)?
            .ok_or(ReinstallPreviewError::CandidatePlanUnavailable)?;
        Ok(self.services_for_game_instance(game_instance))
    }

    fn services_for_game_instance(
        &self,
        game_instance: GameInstance,
    ) -> ConfiguredReinstallServices {
        self.services_for_game_instance_with_source(game_instance, Arc::clone(&self.source))
    }

    fn services_for_game_instance_with_source(
        &self,
        game_instance: GameInstance,
        source: Arc<dyn ReinstallCandidateSourceReader>,
    ) -> ConfiguredReinstallServices {
        let game_files: Arc<dyn InstallGameFileSystem> = Arc::new(
            FileSystemInstallGameFileSystem::new(game_instance.root_dir.clone()),
        );
        let backup_store = Arc::new(FileSystemInstallBackupStore::new(
            self.app_data_dir.join("install").join("backups"),
        ));
        let preview = Arc::new(ReinstallPreviewService::new(
            Arc::clone(&self.prerequisites),
            Arc::clone(&self.catalog),
            self.planner.clone(),
            Arc::clone(&source),
            Arc::clone(&game_files),
            backup_store.clone(),
            Arc::clone(&self.manifest_repository),
            Arc::clone(&self.recovery_repository),
        ));
        let commit = Arc::new(ReinstallCommitService::new(
            Arc::clone(&self.catalog),
            source,
            game_files,
            backup_store.clone(),
            Arc::clone(&self.manifest_repository),
            Arc::clone(&self.recovery_repository),
            backup_store,
        ));
        let executor = ReinstallTaskExecutorService::new(Arc::clone(&preview), commit);

        ConfiguredReinstallServices {
            game_instance,
            preview,
            executor,
        }
    }
}

fn load_reinstall_game_instance_for_commit(
    repository: &dyn GameConfigRepository,
    prepared_instance: &GameInstance,
) -> Result<GameInstance, ReinstallCommitError> {
    let current_instance = repository
        .load_game_instance(&prepared_instance.game_id)
        .map_err(|_| ReinstallCommitError::Failed {
            phase: hmm_app::ReinstallCommitPhase::Revalidation,
        })?
        .ok_or(ReinstallCommitError::PreviewStale)?;
    if current_instance != *prepared_instance {
        return Err(ReinstallCommitError::PreviewStale);
    }
    Ok(current_instance)
}

impl ReinstallTaskExecutor for ConfiguredReinstallExecutor {
    type Prepared = ConfiguredPreparedReinstall;

    fn prepare(
        &self,
        request: ReinstallPreviewRequest,
    ) -> Result<Self::Prepared, ReinstallTaskPrepareError> {
        let fallback = ReinstallTaskAuditContext {
            previous_revision_id: None,
            candidate_revision_id: request.candidate_revision_id.clone(),
            counts: ReinstallTargetCounts::default(),
        };
        let services = self
            .services_for(&request.game_id)
            .map_err(|_| ReinstallTaskPrepareError::Planning(fallback))?;
        let prepared = services.executor.prepare(request)?;
        Ok(ConfiguredPreparedReinstall {
            prepared,
            game_instance: services.game_instance,
            source: Arc::clone(&self.source),
            staging_cleanup: RetargetStagingCleanup::default(),
        })
    }

    fn revalidate(&self, prepared: &Self::Prepared) -> Result<(), ReinstallCommitError> {
        self.services_for_game_instance_with_source(
            prepared.game_instance.clone(),
            Arc::clone(&prepared.source),
        )
        .executor
        .revalidate(&prepared.prepared)
    }

    fn commit(
        &self,
        prepared: Self::Prepared,
        expected_plan_token: &str,
    ) -> Result<ReinstallCommitResult, ReinstallCommitError> {
        let ConfiguredPreparedReinstall {
            prepared,
            game_instance,
            source,
            staging_cleanup: _cleanup_guard,
        } = prepared;
        let result = load_reinstall_game_instance_for_commit(
            self.game_config_repository.as_ref(),
            &game_instance,
        )
        .and_then(|current_instance| {
            self.services_for_game_instance_with_source(current_instance, source)
                .executor
                .commit(prepared, expected_plan_token)
        });
        result
    }
}

impl RetargetReinstallTaskExecutor for ConfiguredReinstallExecutor {
    fn prepare_retarget_reinstall(
        &self,
        request: RetargetReinstallRequest,
    ) -> Result<Self::Prepared, ReinstallTaskPrepareError> {
        let fallback = ReinstallTaskAuditContext {
            previous_revision_id: None,
            candidate_revision_id: hmm_core::ModRevisionId::new("unresolved"),
            counts: ReinstallTargetCounts::default(),
        };
        let prepared = match ConfiguredReinstallExecutor::prepare_retarget_reinstall(self, request)
        {
            Ok(prepared) => prepared,
            Err(ConfiguredRetargetReinstallError::Replacement(_)) => {
                return Err(ReinstallTaskPrepareError::Planning(fallback));
            }
            Err(ConfiguredRetargetReinstallError::Reinstall(
                ReinstallPreviewError::CatalogUnavailable
                | ReinstallPreviewError::CandidatePlanUnavailable,
            )) => return Err(ReinstallTaskPrepareError::Planning(fallback)),
            Err(ConfiguredRetargetReinstallError::Reinstall(
                ReinstallPreviewError::ManifestUnavailable
                | ReinstallPreviewError::RecoveryUnavailable,
            )) => return Err(ReinstallTaskPrepareError::Preflight(fallback)),
        };
        let ConfiguredRetargetReinstallPreparation {
            preparation,
            game_instance,
            source,
            staging_cleanup,
        } = prepared;
        match preparation {
            ReinstallPreparation::Ready(reinstall) => Ok(ConfiguredPreparedReinstall {
                prepared: *reinstall,
                game_instance,
                source,
                staging_cleanup,
            }),
            ReinstallPreparation::Blocked(preview) => Err(ReinstallTaskPrepareError::Preflight(
                ReinstallTaskAuditContext {
                    previous_revision_id: preview
                        .installed_revision
                        .as_ref()
                        .map(|revision| revision.revision_id.clone()),
                    candidate_revision_id: preview
                        .candidate_revision
                        .map(|revision| revision.revision_id)
                        .or_else(|| {
                            preview
                                .installed_revision
                                .map(|revision| revision.revision_id)
                        })
                        .unwrap_or(fallback.candidate_revision_id),
                    counts: preview.counts,
                },
            )),
        }
    }
}

pub struct ConfiguredInstallRecoveryScanner {
    game_config_repository: Arc<dyn GameConfigRepository>,
    app_data_dir: PathBuf,
    write_locks: Arc<GameProfileWriteLockRegistry>,
    reinstall_recovery_repository: Arc<dyn ReinstallRecoveryTransactionRepository>,
}

impl ConfiguredInstallRecoveryScanner {
    fn new(
        game_config_repository: Arc<dyn GameConfigRepository>,
        app_data_dir: PathBuf,
        write_locks: Arc<GameProfileWriteLockRegistry>,
        reinstall_recovery_repository: Arc<dyn ReinstallRecoveryTransactionRepository>,
    ) -> Self {
        Self {
            game_config_repository,
            app_data_dir,
            write_locks,
            reinstall_recovery_repository,
        }
    }

    pub fn scan(
        &self,
        game_id: GameId,
        request: InstallRecoveryScanRequest,
    ) -> Result<Vec<InstallRecoverySummary>, InstallRecoveryScanError> {
        let write_lock = self.write_locks.lock_for(&game_id, &request.profile_id);
        let _guard = write_lock
            .lock()
            .map_err(|_| InstallRecoveryScanError::ManifestUnavailable)?;
        self.scan_without_lock(game_id, request)
    }

    fn scan_without_lock(
        &self,
        game_id: GameId,
        request: InstallRecoveryScanRequest,
    ) -> Result<Vec<InstallRecoverySummary>, InstallRecoveryScanError> {
        let game_instance = self
            .game_config_repository
            .load_game_instance(&game_id)
            .map_err(|_| InstallRecoveryScanError::GameInstanceUnavailable)?
            .ok_or(InstallRecoveryScanError::GameInstanceUnavailable)?;
        let backup_store = Arc::new(FileSystemInstallBackupStore::new(
            self.app_data_dir.join("install").join("backups"),
        ));
        let service = InstallRecoveryScanService::new_with_recovery_records(
            Arc::new(FileSystemInstallGameFileSystem::new(game_instance.root_dir)),
            backup_store.clone(),
            Arc::new(JsonInstallManifestRepository::new(
                self.app_data_dir.join("install").join("manifests"),
            )),
            Arc::new(JsonInstallRecoveryRecordRepository::new(
                self.app_data_dir.join("install").join("recovery"),
            )),
        )
        .with_reinstall_recovery_transactions(
            Arc::clone(&self.reinstall_recovery_repository),
            backup_store,
        );

        service.scan(request)
    }
}

impl InitialRetargetInstallStatusReader for ConfiguredInstallRecoveryScanner {
    fn recovery_status(
        &self,
        game_id: &GameId,
        profile_id: &hmm_core::ProfileId,
        mod_id: &hmm_core::ModId,
    ) -> Result<hmm_app::InstallRecoveryStatus, InitialRetargetInstallStatusError> {
        let summaries = self
            .scan(
                game_id.clone(),
                InstallRecoveryScanRequest {
                    profile_id: profile_id.clone(),
                    mod_ids: Vec::new(),
                },
            )
            .map_err(|_| InitialRetargetInstallStatusError::Unavailable)?;
        Ok(initial_retarget_status(mod_id, &summaries))
    }
}

fn initial_retarget_status(
    mod_id: &hmm_core::ModId,
    summaries: &[InstallRecoverySummary],
) -> hmm_app::InstallRecoveryStatus {
    summaries
        .iter()
        .find(|summary| {
            !matches!(
                summary.status,
                hmm_app::InstallRecoveryStatus::NotInstalled
                    | hmm_app::InstallRecoveryStatus::Completed
            )
        })
        .map(|summary| summary.status)
        .or_else(|| {
            summaries
                .iter()
                .find(|summary| summary.mod_id == *mod_id)
                .map(|summary| summary.status)
        })
        .unwrap_or(hmm_app::InstallRecoveryStatus::NotInstalled)
}

pub struct ConfiguredInstallRecoveryActionPreviewer {
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

    pub fn preview(
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

struct ConfiguredInstallRecoveryActionExecutor {
    game_config_repository: Arc<dyn GameConfigRepository>,
    app_data_dir: PathBuf,
    reinstall_recovery_repository: Arc<dyn ReinstallRecoveryTransactionRepository>,
}

impl ConfiguredInstallRecoveryActionExecutor {
    fn new(
        game_config_repository: Arc<dyn GameConfigRepository>,
        app_data_dir: PathBuf,
        reinstall_recovery_repository: Arc<dyn ReinstallRecoveryTransactionRepository>,
    ) -> Self {
        Self {
            game_config_repository,
            app_data_dir,
            reinstall_recovery_repository,
        }
    }
}

impl InstallRecoveryActionExecutor for ConfiguredInstallRecoveryActionExecutor {
    fn run_recovery_action(
        &self,
        request: StartRecoveryActionTaskRequest,
    ) -> Result<InstallRecoveryActionResult, InstallRecoveryActionError> {
        let game_instance = self
            .game_config_repository
            .load_game_instance(&request.game_id)
            .map_err(|_| InstallRecoveryActionError::ActionUnavailable)?
            .ok_or(InstallRecoveryActionError::ActionUnavailable)?;
        let backup_store = Arc::new(FileSystemInstallBackupStore::new(
            self.app_data_dir.join("install").join("backups"),
        ));
        let service = InstallRecoveryActionService::new_with_manifest(
            Arc::new(FileSystemInstallGameFileSystem::new(game_instance.root_dir)),
            backup_store.clone(),
            Arc::new(JsonInstallRecoveryRecordRepository::new(
                self.app_data_dir.join("install").join("recovery"),
            )),
            Arc::new(JsonInstallManifestRepository::new(
                self.app_data_dir.join("install").join("manifests"),
            )),
        )
        .with_reinstall_reconciliation(
            Arc::clone(&self.reinstall_recovery_repository),
            backup_store,
        );

        service.run(InstallRecoveryActionRequest {
            profile_id: request.profile_id,
            mod_id: request.mod_id,
            action_kind: request.action_kind,
        })
    }
}

struct ConfiguredInstallCommitter {
    game_config_repository: Arc<dyn GameConfigRepository>,
    mod_import_result_repository: Arc<dyn ModImportResultRepository>,
    mod_import_sandbox_locator: Arc<dyn ModImportSandboxLocator>,
    app_data_dir: PathBuf,
}

struct ConfiguredInitialRetargetInstallPlanner {
    workflow: Arc<ReplacementWorkflowService>,
    prerequisites: Arc<dyn GamePrerequisiteDecisionProvider>,
    sandbox_locator: Arc<dyn ModImportSandboxLocator>,
    install_recovery_scanner: Arc<ConfiguredInstallRecoveryScanner>,
    app_data_dir: PathBuf,
}

impl ConfiguredInitialRetargetInstallPlanner {
    fn new(
        workflow: Arc<ReplacementWorkflowService>,
        prerequisites: Arc<dyn GamePrerequisiteDecisionProvider>,
        sandbox_locator: Arc<dyn ModImportSandboxLocator>,
        install_recovery_scanner: Arc<ConfiguredInstallRecoveryScanner>,
        app_data_dir: PathBuf,
    ) -> Self {
        Self {
            workflow,
            prerequisites,
            sandbox_locator,
            install_recovery_scanner,
            app_data_dir,
        }
    }

    fn materializer_for(
        &self,
        planned: &PlannedInitialRetargetInstall,
    ) -> Result<FileSystemRetargetStagingMaterializer, ReplacementWorkflowError> {
        let source_root = self
            .sandbox_locator
            .sandbox_root_for_package(planned.package_id())
            .map_err(|_| ReplacementWorkflowError::SandboxUnavailable)?;
        let staging_root = retarget_staging_root(&self.app_data_dir, planned.binding_id())
            .ok_or(ReplacementWorkflowError::PlanUnavailable)?;
        Ok(FileSystemRetargetStagingMaterializer::new(
            staging_root,
            Arc::new(FileSystemInstallSourceFileReader::new(source_root)),
        ))
    }
}

impl InitialRetargetInstallPlanner for ConfiguredInitialRetargetInstallPlanner {
    fn build_initial_retarget_install_plan(
        &self,
        request: StartRetargetInstallTaskRequest,
    ) -> Result<InitialRetargetInstallPlan, ReplacementWorkflowError> {
        let planned =
            self.workflow
                .preview_initial_install(PreviewInitialRetargetInstallRequest {
                    game_id: request.game_id,
                    profile_id: request.profile_id,
                    mod_id: request.mod_id,
                    target_id: request.target_id,
                    layer: request.layer,
                })?;
        let materializer = self.materializer_for(&planned)?;
        let revision_id = planned.revision_id().clone();
        let plan = self
            .workflow
            .materialize_initial_install(&materializer, planned)?;
        Ok(InitialRetargetInstallPlan { plan, revision_id })
    }

    fn revalidate_initial_install(
        &self,
        request: &StartRetargetInstallTaskRequest,
    ) -> Result<(), ReplacementWorkflowError> {
        let summaries = self
            .install_recovery_scanner
            .scan_without_lock(
                request.game_id.clone(),
                InstallRecoveryScanRequest {
                    profile_id: request.profile_id.clone(),
                    mod_ids: Vec::new(),
                },
            )
            .map_err(|_| ReplacementWorkflowError::InstallStatusUnavailable)?;
        let status = initial_retarget_status(&request.mod_id, &summaries);
        if status == hmm_app::InstallRecoveryStatus::NotInstalled {
            Ok(())
        } else {
            Err(ReplacementWorkflowError::InitialInstallBlocked { status })
        }
    }

    fn prerequisite_decision(&self, game_id: &GameId) -> hmm_app::GamePrerequisiteDecision {
        self.prerequisites.prerequisite_decision(game_id)
    }

    fn discard_initial_retarget_install(&self, plan: &hmm_core::InstallPlan) {
        let [snapshot] = plan.replacement_bindings.as_slice() else {
            return;
        };
        let Some(staging_root) = retarget_staging_root(&self.app_data_dir, snapshot.binding_id())
        else {
            return;
        };
        discard_retarget_staging(&staging_root);
    }
}

fn retarget_staging_root(
    app_data_dir: &Path,
    binding_id: &ReplacementBindingId,
) -> Option<PathBuf> {
    let id = binding_id.as_str().strip_prefix("binding-")?;
    let id = uuid::Uuid::parse_str(id).ok()?;
    Some(
        app_data_dir
            .join("install")
            .join("retarget-staging")
            .join(id.to_string()),
    )
}

fn retarget_reinstall_staging_root(app_data_dir: &Path) -> PathBuf {
    app_data_dir
        .join("install")
        .join("retarget-staging")
        .join(uuid::Uuid::new_v4().to_string())
}

fn discard_retarget_staging(staging_root: &Path) {
    if std::fs::remove_dir_all(staging_root).is_err() && staging_root.exists() {
        record_runtime_warning(
            "retarget.staging_cleanup_failed",
            "discard",
            "retarget_staging_cleanup_failed",
        );
    }
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
        let source_error = || InstallCommitError::Failed {
            phase: InstallCommitPhase::SourceRead,
        };
        let imported_source_files = || {
            let package_id = match request.revision_id.as_ref() {
                Some(revision_id) => self
                    .mod_import_result_repository
                    .get_revision(revision_id)
                    .map_err(|_| source_error())?
                    .filter(|revision| revision.mod_id == request.mod_id)
                    .map(|revision| revision.package_id)
                    .ok_or_else(source_error)?,
                None => self
                    .mod_import_result_repository
                    .get_analysis(request.mod_id.as_str())
                    .map_err(|_| source_error())?
                    .map(|analysis| analysis.package_id)
                    .ok_or_else(source_error)?,
            };
            let source_root = self
                .mod_import_sandbox_locator
                .sandbox_root_for_package(&package_id)
                .map_err(|_| source_error())?;
            Ok::<Arc<dyn InstallSourceFileReader>, InstallCommitError>(Arc::new(
                FileSystemInstallSourceFileReader::new(source_root),
            ))
        };
        let (source_files, staging_root): (Arc<dyn InstallSourceFileReader>, Option<PathBuf>) =
            match request.plan.replacement_bindings.as_slice() {
                [] => (imported_source_files()?, None),
                [snapshot] if is_identity_replacement_binding(snapshot) => {
                    (imported_source_files()?, None)
                }
                [snapshot] => {
                    let staging_root =
                        retarget_staging_root(&self.app_data_dir, snapshot.binding_id())
                            .ok_or_else(source_error)?;
                    let reader = RetargetStagingInstallSourceFileReader::from_install_plan(
                        staging_root.clone(),
                        &request.plan,
                    )
                    .map_err(|_| source_error())?;
                    (Arc::new(reader), Some(staging_root))
                }
                _ => return Err(source_error()),
            };
        let service = InstallCommitService::new_with_recovery_records(
            source_files,
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

        let commit_request = CommitInstallPlanRequest {
            profile_id: request.profile_id,
            plan: request.plan,
        };
        let result = match request.revision_id {
            Some(revision_id) => {
                service.commit_plan_for_revision(commit_request, request.mod_id, revision_id)
            }
            None => service.commit_plan(commit_request),
        };
        if let Some(staging_root) = staging_root {
            if std::fs::remove_dir_all(&staging_root).is_err() && staging_root.exists() {
                record_runtime_warning(
                    "retarget.staging_cleanup_failed",
                    "post_commit",
                    "retarget_staging_cleanup_failed",
                );
            }
        }
        result
    }
}

fn clone_game_adapters(adapters: &[Arc<dyn GameAdapter>]) -> Vec<Arc<dyn GameAdapter>> {
    adapters.to_vec()
}

fn game_ids_from_adapters(adapters: &[Arc<dyn GameAdapter>]) -> Vec<GameId> {
    adapters.iter().map(|adapter| adapter.game_id()).collect()
}

fn game_process_names_by_game(adapters: &[Arc<dyn GameAdapter>]) -> HashMap<GameId, Vec<String>> {
    adapters
        .iter()
        .map(|adapter| (adapter.game_id(), adapter.process_image_names()))
        .collect()
}

fn game_running_detector_for_platform(
    adapters: &[Arc<dyn GameAdapter>],
) -> Arc<dyn GameRunningDetector> {
    let process_names = game_process_names_by_game(adapters);
    #[cfg(target_os = "windows")]
    {
        Arc::new(TasklistGameRunningDetector::new(process_names))
    }
    #[cfg(not(target_os = "windows"))]
    {
        Arc::new(PgrepGameRunningDetector::new(process_names))
    }
}

fn start_best_effort_background_task<E>(
    task_name: &'static str,
    spawn: impl FnOnce() -> Result<(), E>,
) {
    if spawn().is_err() {
        record_runtime_warning(
            "background_task.spawn_failed",
            task_name,
            "background_task_spawn_failed",
        );
    }
}

fn record_runtime_warning(
    event_name: &'static str,
    operation: &'static str,
    error_code: &'static str,
) {
    emit_safe_app_log(
        AppLogEvent::warning(event_name)
            .with_operation(operation)
            .with_result("failed")
            .with_error_code(error_code),
    );
}

#[cfg(test)]
#[path = "runtime_core_mod_lifecycle_tests.rs"]
mod core_mod_lifecycle_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use hmm_core::{GameDirectoryStatus, GameId, GameInstance, ModId, ProfileId};
    use hmm_ports::{
        DebugLogControl, GameConfigRepositoryError, GameConfigRepositoryResult,
        SaveBackupBackgroundSettingsRepository,
    };
    use std::fs::{self, File, FileTimes};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Barrier, Mutex};
    use std::time::{Duration, Instant, SystemTime};

    fn recovery_summary(
        mod_id: &str,
        status: hmm_app::InstallRecoveryStatus,
    ) -> InstallRecoverySummary {
        InstallRecoverySummary {
            profile_id: ProfileId::new("profile-a"),
            mod_id: ModId::new(mod_id),
            status,
            managed_file_count: usize::from(status == hmm_app::InstallRecoveryStatus::Completed),
            backup_count: 0,
            issue_count: 0,
            issues: Vec::new(),
        }
    }

    #[derive(Default)]
    struct CapturingAuditLogWriter {
        policy: Mutex<Option<AuditWriteFailurePolicy>>,
        event: Mutex<Option<AuditLogEvent>>,
    }

    impl AuditLogWriter for CapturingAuditLogWriter {
        fn record(&self, _event: AuditLogEvent) -> anyhow::Result<()> {
            panic!("maintenance audit must select an explicit failure policy")
        }

        fn record_with_policy(
            &self,
            event: AuditLogEvent,
            policy: AuditWriteFailurePolicy,
        ) -> anyhow::Result<()> {
            *self.policy.lock().expect("audit policy lock") = Some(policy);
            *self.event.lock().expect("audit event lock") = Some(event);
            Ok(())
        }
    }

    #[test]
    fn initial_retarget_status_blocks_on_another_mods_unsafe_profile_state() {
        let summaries = vec![
            recovery_summary("mod-a", hmm_app::InstallRecoveryStatus::Completed),
            recovery_summary(
                "mod-b",
                hmm_app::InstallRecoveryStatus::CommittedCleanupPending,
            ),
        ];

        assert_eq!(
            initial_retarget_status(&ModId::new("new-mod"), &summaries),
            hmm_app::InstallRecoveryStatus::CommittedCleanupPending
        );
    }

    #[test]
    fn initial_retarget_status_distinguishes_installed_and_absent_mods_in_safe_profile() {
        let summaries = vec![recovery_summary(
            "mod-a",
            hmm_app::InstallRecoveryStatus::Completed,
        )];

        assert_eq!(
            initial_retarget_status(&ModId::new("mod-a"), &summaries),
            hmm_app::InstallRecoveryStatus::Completed
        );
        assert_eq!(
            initial_retarget_status(&ModId::new("mod-b"), &summaries),
            hmm_app::InstallRecoveryStatus::NotInstalled
        );
    }

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

    struct StaticGameConfigRepository {
        instance: Option<GameInstance>,
    }

    impl GameConfigRepository for StaticGameConfigRepository {
        fn load_game_instance(
            &self,
            _game_id: &GameId,
        ) -> GameConfigRepositoryResult<Option<GameInstance>> {
            Ok(self.instance.clone())
        }

        fn save_game_instance(&self, _instance: &GameInstance) -> GameConfigRepositoryResult<()> {
            panic!("reinstall game-instance revalidation test must not save game config")
        }
    }

    fn configured_game_instance(root_dir: &str, configured_at_unix_millis: u128) -> GameInstance {
        GameInstance {
            id: "mhw-default".to_owned(),
            game_id: GameId::mhw(),
            display_name: "Monster Hunter: World - Iceborne".to_owned(),
            root_dir: PathBuf::from(root_dir),
            status: GameDirectoryStatus::Configured,
            configured_at_unix_millis,
        }
    }

    struct TestGameAdapter {
        process_names: Vec<String>,
    }

    impl GameAdapter for TestGameAdapter {
        fn game_id(&self) -> GameId {
            GameId::mhw()
        }

        fn display_name(&self) -> &'static str {
            "Test Game"
        }

        fn validate_directory(
            &self,
            probe: &dyn hmm_ports::GameDirectoryProbe,
        ) -> hmm_core::GameDirectoryValidation {
            hmm_core::GameDirectoryValidation::new(GameId::mhw(), probe.root_dir().to_path_buf())
        }

        fn inspect_prerequisites(
            &self,
            _probe: &dyn hmm_ports::GameDirectoryProbe,
        ) -> hmm_ports::GamePrerequisiteReport {
            hmm_ports::GamePrerequisiteReport::not_configured(GameId::mhw())
        }

        fn process_image_names(&self) -> Vec<String> {
            self.process_names.clone()
        }
    }

    #[test]
    fn game_process_names_are_derived_from_registered_adapters() {
        let adapter: Arc<dyn GameAdapter> = Arc::new(TestGameAdapter {
            process_names: vec!["ExampleGame.exe".to_owned()],
        });

        let process_names = game_process_names_by_game(&[adapter]);

        assert_eq!(
            process_names.get(&GameId::mhw()),
            Some(&vec!["ExampleGame.exe".to_owned()])
        );
    }

    #[test]
    fn best_effort_background_task_start_ignores_spawn_failure() {
        start_best_effort_background_task("thumbnail-cache-maintenance", || -> Result<(), &str> {
            Err("spawn failed")
        });
    }

    #[test]
    fn shared_runtime_composition_applies_task_and_audit_retention_on_startup() {
        let temp = tempfile::tempdir().expect("temporary app data directory");
        let app_data_dir = temp.path().to_path_buf();
        let task_dir = app_data_dir.join("logs").join("tasks");
        let audit_dir = app_data_dir.join("logs").join("audit");
        fs::create_dir_all(&task_dir).expect("create task log directory");
        fs::create_dir_all(&audit_dir).expect("create audit log directory");
        let expired_task = task_dir.join("task-install-expired.log");
        let unknown_task = task_dir.join("notes.txt");
        let expired_audit = audit_dir.join("audit-1970-01-01.log");
        fs::write(&expired_task, "expired\n").expect("write expired task log");
        fs::write(&unknown_task, "unmanaged\n").expect("write unmanaged task file");
        fs::write(&expired_audit, "expired\n").expect("write expired audit log");
        File::options()
            .write(true)
            .open(&expired_task)
            .expect("open expired task log")
            .set_times(FileTimes::new().set_modified(SystemTime::UNIX_EPOCH))
            .expect("age expired task log");

        let state = HmmRuntime::from_app_data_dir(app_data_dir)
            .expect("shared runtime composition succeeds");

        assert!(!expired_task.exists());
        assert!(!expired_audit.exists());
        assert!(unknown_task.exists());
        let health = state
            .support_diagnostics_export
            .read_page_snapshot()
            .evidence_health;
        assert_eq!(health.task_log_status, "ok");
        assert_eq!(health.audit_log_status, "ok");
        assert_eq!(health.task_log_retention_failure_count, 0);
        assert_eq!(health.audit_log_retention_failure_count, 0);
    }

    #[test]
    fn shared_runtime_initializes_debug_log_from_persisted_settings() {
        let temp = tempfile::tempdir().expect("temporary app data directory");
        let app_data_dir = temp.path().to_path_buf();
        let config_dir = app_data_dir.join("config");
        fs::create_dir_all(&config_dir).expect("create config directory");
        fs::write(
            config_dir.join("settings.json"),
            r#"{"version":1,"debugLogEnabled":true}"#,
        )
        .expect("write enabled debug settings");

        let state = HmmRuntime::from_app_data_dir(app_data_dir.clone())
            .expect("shared runtime composition succeeds");

        assert!(state.debug_log.is_enabled());
        let debug_log = app_data_dir.join("logs").join("debug");
        assert!(debug_log.is_dir());
        assert_eq!(
            fs::read_dir(debug_log)
                .expect("read debug log directory")
                .count(),
            1
        );
    }

    #[test]
    fn shared_runtime_defaults_debug_log_to_disabled_when_settings_are_corrupt() {
        let temp = tempfile::tempdir().expect("temporary app data directory");
        let app_data_dir = temp.path().to_path_buf();
        let config_dir = app_data_dir.join("config");
        fs::create_dir_all(&config_dir).expect("create config directory");
        fs::write(config_dir.join("settings.json"), b"{not-json").expect("write corrupt settings");

        let state = HmmRuntime::from_app_data_dir(app_data_dir.clone())
            .expect("corrupt settings should fail closed");

        assert!(!state.debug_log.is_enabled());
        assert!(!app_data_dir.join("logs").join("debug").exists());
    }

    #[test]
    fn shared_runtime_applies_custom_log_budget_and_records_one_maintenance_audit() {
        let temp = tempfile::tempdir().expect("temporary app data directory");
        let app_data_dir = temp.path().to_path_buf();
        let config_dir = app_data_dir.join("config");
        let task_dir = app_data_dir.join("logs").join("tasks");
        fs::create_dir_all(&config_dir).expect("create config directory");
        fs::create_dir_all(&task_dir).expect("create task log directory");
        fs::write(
            config_dir.join("settings.json"),
            r#"{
                "version": 1,
                "logStorageMaxBytes": 1048576
            }"#,
        )
        .expect("write log storage settings");
        let oversized_task = task_dir.join("task-install-budget.log");
        fs::write(&oversized_task, vec![b'x'; 1_100_000]).expect("write oversized task log");

        let state = HmmRuntime::from_app_data_dir(app_data_dir)
            .expect("shared runtime composition succeeds");

        assert!(!oversized_task.exists());
        let snapshot = state.support_diagnostics_export.read_page_snapshot();
        let maintenance_events = snapshot
            .audit_events
            .iter()
            .filter(|event| event.operation == "log_storage_budget_maintenance")
            .collect::<Vec<_>>();
        assert_eq!(maintenance_events.len(), 1);
        assert_eq!(maintenance_events[0].result, "success");
        assert_eq!(maintenance_events[0].fields["outcome"], "reduced_to_budget");
        assert_eq!(maintenance_events[0].fields["deleted_file_count"], "1");
        assert_eq!(snapshot.evidence_health.log_storage_status, "ok");
    }

    #[test]
    fn log_storage_maintenance_audit_is_not_classified_as_a_player_commit() {
        let writer = CapturingAuditLogWriter::default();
        record_log_storage_budget_maintenance(
            &writer,
            1,
            LogStorageBudgetReport {
                outcome: LogStorageBudgetOutcome::ReducedToBudget,
                max_bytes: 1024 * 1024,
                cleanup_target_bytes: 1024 * 1024 - 16 * 1024,
                owned_bytes_before: 1024 * 1024 + 1,
                owned_bytes_after: 512 * 1024,
                deleted_file_count: 1,
                deleted_bytes: 512 * 1024 + 1,
                failed_category_count: 0,
            },
            false,
        );

        assert_eq!(
            *writer.policy.lock().expect("audit policy lock"),
            Some(AuditWriteFailurePolicy::BestEffort)
        );
        assert_eq!(
            writer
                .event
                .lock()
                .expect("audit event lock")
                .as_ref()
                .expect("maintenance audit event")
                .result,
            "success"
        );
    }

    #[test]
    fn invalid_persisted_log_budget_falls_back_and_degrades_health_without_failing_startup() {
        let temp = tempfile::tempdir().expect("temporary app data directory");
        let app_data_dir = temp.path().to_path_buf();
        let config_dir = app_data_dir.join("config");
        fs::create_dir_all(&config_dir).expect("create config directory");
        fs::write(
            config_dir.join("settings.json"),
            r#"{
                "version": 1,
                "logStorageMaxBytes": 1024
            }"#,
        )
        .expect("write invalid log storage settings");

        let state = HmmRuntime::from_app_data_dir(app_data_dir)
            .expect("runtime falls back to the default log budget");

        let snapshot = state.support_diagnostics_export.read_page_snapshot();
        assert_eq!(
            snapshot.evidence_health.log_storage_status,
            "log_storage_settings_unavailable"
        );
        assert_eq!(
            snapshot.evidence_health.log_storage_settings_failure_count,
            1
        );
        let maintenance = snapshot
            .audit_events
            .iter()
            .find(|event| event.operation == "log_storage_budget_maintenance")
            .expect("settings degradation is audited once");
        assert_eq!(maintenance.result, "degraded");
        assert_eq!(maintenance.fields["settings_status"], "degraded");
    }

    #[test]
    fn corrupted_log_settings_fall_back_without_blocking_runtime_composition() {
        let temp = tempfile::tempdir().expect("temporary app data directory");
        let app_data_dir = temp.path().to_path_buf();
        let config_dir = app_data_dir.join("config");
        fs::create_dir_all(&config_dir).expect("create config directory");
        fs::write(config_dir.join("settings.json"), "{not json").expect("write corrupted settings");

        let state = HmmRuntime::from_app_data_dir(app_data_dir)
            .expect("runtime falls back when settings are unavailable");
        let health = state
            .support_diagnostics_export
            .read_page_snapshot()
            .evidence_health;

        assert_eq!(
            health.log_storage_status,
            "log_storage_settings_unavailable"
        );
        assert_eq!(health.log_storage_settings_failure_count, 1);
    }

    #[test]
    fn state_composes_shared_background_settings_for_service_and_worker() {
        let temp = tempfile::tempdir().expect("temporary app data directory");
        let app_data_dir = temp.path().to_path_buf();
        let state = HmmRuntime::from_app_data_dir(app_data_dir.clone())
            .expect("headless state composition succeeds");

        let control = state
            .save_backup_background
            .control_status()
            .expect("default background control status");
        assert_eq!(
            control.status,
            hmm_core::SaveBackupBackgroundProtectionStatus::NotEnabled
        );

        let settings_db = Arc::new(Mutex::new(
            hmm_infra::open_database(&app_data_dir.join("hmm.db"))
                .expect("open background settings test database"),
        ));
        let settings =
            hmm_infra::SqliteSaveBackupBackgroundSettingsRepository::new(Arc::clone(&settings_db));
        settings.begin_enable(1).expect("enable background intent");
        state
            .save_backup_background_worker
            .run_once("worker-composition-test")
            .expect("settings-aware worker cycle");
        assert!(settings
            .load()
            .expect("load background settings")
            .last_worker_heartbeat_at
            .is_some());

        drop(settings);
        drop(settings_db);
        drop(state);
    }

    #[test]
    fn headless_state_composes_reinstall_tasks_with_shared_task_manager() {
        let temp = tempfile::tempdir().expect("temporary app data directory");
        let app_data_dir = temp.path().to_path_buf();
        let state = HmmRuntime::from_app_data_dir(app_data_dir.clone())
            .expect("headless state composition succeeds");
        let request = hmm_app::StartReinstallTaskRequest {
            game_id: GameId::mhw(),
            profile_id: ProfileId::new("default"),
            mod_id: ModId::new("mod-a"),
            candidate_revision_id: hmm_core::ModRevisionId::new("revision-v2"),
            layer: hmm_core::FileLayer::new("base", 0),
            plan_token: "reinstall-preview-v1:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_owned(),
        };

        let task = state
            .reinstall_tasks
            .start_reinstall_task(request)
            .expect("reinstall task starts");

        assert_eq!(task.kind, hmm_app::TaskKind::Install);
        assert_eq!(task.status, hmm_app::TaskStatus::Queued);
        assert_eq!(
            state.task_manager.task_status(&task.task_id),
            Some(hmm_app::TaskStatus::Queued)
        );
        assert!(Arc::ptr_eq(
            &state.reinstall_recovery_repository,
            &state.reinstall_executor.recovery_repository
        ));

        drop(state);
    }

    #[test]
    fn reinstall_commit_rejects_game_instance_changed_after_prepare() {
        let prepared_instance = configured_game_instance("C:/fixture/mhw-v1", 1);
        let repository = StaticGameConfigRepository {
            instance: Some(configured_game_instance("C:/fixture/mhw-v2", 2)),
        };

        let error = load_reinstall_game_instance_for_commit(&repository, &prepared_instance)
            .expect_err("changed game instance must invalidate prepared reinstall");

        assert_eq!(error, ReinstallCommitError::PreviewStale);
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
            Arc::new(JsonReinstallRecoveryTransactionRepository::new(
                std::env::temp_dir()
                    .join("hmm-recovery-lock-test")
                    .join("install")
                    .join("reinstall-recovery"),
            )),
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

    #[test]
    fn recovery_action_task_waits_for_shared_game_profile_write_lock() {
        let game_id = GameId::mhw();
        let profile_id = ProfileId::new("default");
        let write_locks = Arc::new(GameProfileWriteLockRegistry::default());
        let write_lock = write_locks.lock_for(&game_id, &profile_id);
        let guard = write_lock.lock().expect("hold shared write lock");
        let load_called = Arc::new(AtomicBool::new(false));
        let task_manager = Arc::new(TaskManager::new());
        let task = task_manager
            .create_task(hmm_app::TaskKind::Install)
            .expect("task can be created");
        let runner = Arc::new(hmm_app::RecoveryActionTaskRunner::with_write_locks(
            Arc::clone(&task_manager),
            Arc::new(ConfiguredInstallRecoveryActionExecutor::new(
                Arc::new(NotifyingGameConfigRepository {
                    load_called: Arc::clone(&load_called),
                }),
                std::env::temp_dir().join("hmm-recovery-action-task-lock-test"),
                Arc::new(JsonReinstallRecoveryTransactionRepository::new(
                    std::env::temp_dir()
                        .join("hmm-recovery-action-task-lock-test")
                        .join("install")
                        .join("reinstall-recovery"),
                )),
            )),
            Arc::new(FileSystemAuditLogWriter::new(
                std::env::temp_dir().join("hmm-recovery-action-task-lock-test"),
            )),
            Arc::new(SystemClock),
            Arc::clone(&write_locks),
        ));
        let barrier = Arc::new(Barrier::new(2));
        let task_barrier = Arc::clone(&barrier);
        let runner_for_thread = Arc::clone(&runner);
        let task_id = task.task_id.clone();
        let request_game_id = game_id.clone();
        let request_profile_id = profile_id.clone();

        let handle = std::thread::spawn(move || {
            task_barrier.wait();
            runner_for_thread.run_recovery_action_task(
                &task_id,
                hmm_app::StartRecoveryActionTaskRequest {
                    game_id: request_game_id,
                    profile_id: request_profile_id,
                    mod_id: ModId::new("mod-a"),
                    action_kind: hmm_app::InstallRecoveryActionKind::RollbackInstall,
                },
            )
        });

        barrier.wait();
        wait_for_task_status(&task_manager, &task.task_id, hmm_app::TaskStatus::Running);
        let deadline = Instant::now() + Duration::from_millis(150);
        while Instant::now() < deadline {
            if load_called.load(Ordering::SeqCst) {
                break;
            }
            std::thread::yield_now();
        }

        assert!(
            !load_called.load(Ordering::SeqCst),
            "recovery action task entered filesystem/config work while the shared write lock was held"
        );

        drop(guard);
        let result = handle
            .join()
            .expect("recovery action task thread should not panic");
        assert!(result.is_err());
        assert!(load_called.load(Ordering::SeqCst));
    }

    fn wait_for_task_status(
        task_manager: &TaskManager,
        task_id: &str,
        expected: hmm_app::TaskStatus,
    ) {
        let deadline = Instant::now() + Duration::from_secs(2);
        while Instant::now() < deadline {
            if task_manager.task_status(task_id) == Some(expected) {
                return;
            }
            std::thread::sleep(Duration::from_millis(10));
        }

        panic!("task {task_id} did not reach expected status {expected:?}");
    }
}
