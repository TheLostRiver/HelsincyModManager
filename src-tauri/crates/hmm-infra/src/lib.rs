mod app_log;
mod app_settings_repository;
mod audit_log;
mod content_root;
mod controlled_fs;
mod cross_process_write_admission;
mod debug_log;
mod diagnostics_environment;
mod diagnostics_health;
mod external_import_materializer;
mod external_import_scanner;
mod external_import_source_registry;
mod game_config_repository;
mod game_directory_probe;
mod game_discovery;
mod game_launcher;
mod game_running_detector;
mod install_commit;
mod log_retention;
mod log_storage_budget;
mod managed_log;
mod mod_import;
mod mod_import_archive_consumer;
mod mod_import_install_files;
#[cfg(test)]
mod mod_library_projection_tests;
mod mod_revision_catalog;
#[cfg(test)]
mod mod_revision_catalog_tests;
mod mod_storage_inspector;
mod mod_storage_migrator;
mod package_content_root;
mod prerequisite_rules_repository;
mod preview_image;
mod reinstall;
mod release_update;
mod replacement_selection;
mod save_backup;
mod save_backup_background_registry;
mod save_directory_pending_store;
mod save_directory_scanner;
mod save_path;
mod save_restore;
pub mod sqlite;
mod staging;
pub mod steam_discovery;
mod steam_profile;
mod system_directory_opener;
mod task_log;
mod text_log;
#[cfg(windows)]
mod windows_identity;

use anyhow::Result;
use hmm_ports::AppClock;
use std::time::{SystemTime, UNIX_EPOCH};

pub use app_log::{
    emit_safe_app_log, initialize_app_logging, redact_sensitive_text, AppLogEvent, AppLogHealth,
    AppLogLevel,
};
pub use app_settings_repository::JsonAppSettingsRepository;
pub use audit_log::{FileSystemAuditLogReader, FileSystemAuditLogWriter};
pub use content_root::{
    native_pc_parents, resolve_content_root, ContentRootResolution, MAX_CONTENT_ROOT_SEARCH_DEPTH,
    NATIVE_PC_DIR_NAME,
};
pub use cross_process_write_admission::{
    PlatformCrossProcessWriteAdmission, PlatformCrossProcessWriteAdmissionInitError,
};
pub use debug_log::{DebugLogController, DebugLogEvent, DebugLogWriteOutcome};
pub use diagnostics_environment::SystemDiagnosticsEnvironmentProvider;
pub use diagnostics_health::DiagnosticsEvidenceHealthState;
pub use external_import_materializer::HuntingBoxDirectoryMaterializer;
pub use external_import_scanner::HuntingBoxDirectoryScanner;
pub use external_import_source_registry::{
    HuntingBoxDirectorySourceRegistry, HUNTING_BOX_DIRECTORY_V1_ADAPTER_ID,
};
pub use game_config_repository::JsonGameConfigRepository;
pub use game_directory_probe::{RealGameDirectoryProbe, RealGameDirectoryProbeFactory};
pub use game_discovery::{NoopGameDiscoveryService, SteamGameDiscoveryService};
pub use game_launcher::SystemGameLaunchRunner;
pub use game_running_detector::{PgrepGameRunningDetector, TasklistGameRunningDetector};
pub use install_commit::{
    FileSystemInstallBackupStore, FileSystemInstallGameFileSystem,
    FileSystemInstallSourceFileReader, JsonInstallManifestRepository,
    JsonInstallRecoveryRecordRepository,
};
pub use log_retention::{
    FileSystemLogRetention, LogRetentionReport, DEFAULT_AUDIT_LOG_RETENTION_DAYS,
    DEFAULT_DEBUG_LOG_RETENTION_DAYS, DEFAULT_TASK_LOG_RETENTION_DAYS,
};
pub use log_storage_budget::{
    FileSystemLogStorageBudget, LogStorageBudgetOutcome, LogStorageBudgetReport,
    DEFAULT_LOG_STORAGE_MAX_BYTES, LOG_STORAGE_AUDIT_RESERVE_BYTES, MIN_AUDIT_LOG_RETENTION_DAYS,
};
pub use mod_import::{
    default_mod_storage_root, FileSystemDiagnosticPackageExporter,
    SandboxModPackageMetadataAnalyzer, TaskScopedModImportSandboxLocator,
    ZipModImportPackagePreparer,
};
pub use mod_import_archive_consumer::FileSystemModImportArchiveConsumer;
pub use mod_import_install_files::SandboxModPackageInstallFileScanner;
pub use mod_revision_catalog::JsonModImportResultRepository;
pub use mod_storage_inspector::FileSystemModStorageDirectoryInspector;
pub use mod_storage_migrator::{
    FileSystemModStorageMigrator, JsonModStorageMigrationJournalRepository,
};
pub use package_content_root::JsonModPackageContentRootRepository;
pub use prerequisite_rules_repository::{
    JsonGamePrerequisiteRuleRepository, ReadOnlyJsonGamePrerequisiteRuleRepository,
};
pub use preview_image::{
    FileSystemThumbnailStore, ImageCratePreviewImageProcessor, SandboxPackagePreviewScanner,
    ThumbnailPruneReport, ThumbnailSizePruneReport,
};
pub use reinstall::JsonReinstallRecoveryTransactionRepository;
pub use release_update::{GitHubLatestReleaseSource, ReqwestReleaseFeedHttpTransport};
pub use replacement_selection::JsonReplacementSelectionRepository;
pub use save_backup::{FileSystemSaveBackupDirectoryLocator, FileSystemSaveBackupWriter};
#[cfg(windows)]
pub use save_backup_background_registry::WindowsScheduledTaskRegistry;
pub use save_backup_background_registry::{
    cleanup_owned_save_backup_task_for_installer, InstallerCleanupOutcome,
    UnsupportedSaveBackupBackgroundRegistry,
};
pub use save_directory_pending_store::InMemoryPendingSaveDirectoryCandidateStore;
pub use save_directory_scanner::SteamUserdataSaveDirectoryScanner;
pub use save_restore::{FileSystemSaveRestoreFileSystem, FileSystemSaveRestoreSourceValidator};
pub use sqlite::SqliteBatchLifecycleRepository;
pub use sqlite::SqliteCategoryRepository;
pub use sqlite::SqliteExternalImportBatchRepository;
pub use sqlite::SqliteProfileRepository;
pub use sqlite::SqliteSaveBackupBackgroundSettingsRepository;
pub use sqlite::SqliteSaveBackupRepository;
pub use sqlite::SqliteSaveBackupSchedulerStateRepository;
pub use sqlite::SqliteSaveRestoreTransactionRepository;
pub use sqlite::{open_database, open_database_read_only};
pub use sqlite::{SqliteModLibraryProjectionRepository, SqliteModMetadataRepository};
pub use staging::{FileSystemRetargetStagingMaterializer, RetargetStagingInstallSourceFileReader};
pub use steam_discovery::PlatformSteamRootProvider;
pub use steam_profile::{
    parse_steam_profile_xml, ReqwestSteamProfileHttpTransport, SteamCommunityProfileClient,
    SteamProfileHttpTransport,
};
pub use system_directory_opener::SystemShellDirectoryOpener;
pub use task_log::FileSystemTaskLogWriter;
pub use text_log::FileSystemTextLogReader;

pub struct SystemClock;

impl AppClock for SystemClock {
    fn now_unix_millis(&self) -> Result<u128> {
        Ok(SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis())
    }
}
