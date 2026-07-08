mod app_settings_repository;
mod audit_log;
mod diagnostics_environment;
mod game_config_repository;
mod game_directory_probe;
mod game_discovery;
mod game_launcher;
mod game_running_detector;
mod install_commit;
mod mod_import;
mod mod_import_install_files;
mod prerequisite_rules_repository;
mod preview_image;
mod save_backup;
mod save_directory_pending_store;
mod save_directory_scanner;
pub mod sqlite;
pub mod steam_discovery;
mod steam_profile;
mod text_log;

use anyhow::Result;
use hmm_ports::AppClock;
use std::time::{SystemTime, UNIX_EPOCH};

pub use app_settings_repository::JsonAppSettingsRepository;
pub use audit_log::FileSystemAuditLogWriter;
pub use diagnostics_environment::SystemDiagnosticsEnvironmentProvider;
pub use game_config_repository::JsonGameConfigRepository;
pub use game_directory_probe::{RealGameDirectoryProbe, RealGameDirectoryProbeFactory};
pub use game_discovery::{NoopGameDiscoveryService, SteamGameDiscoveryService};
pub use game_launcher::SystemGameLaunchRunner;
pub use game_running_detector::TasklistGameRunningDetector;
pub use install_commit::{
    FileSystemInstallBackupStore, FileSystemInstallGameFileSystem,
    FileSystemInstallSourceFileReader, JsonInstallManifestRepository,
    JsonInstallRecoveryRecordRepository,
};
pub use mod_import::{
    FileSystemDiagnosticPackageExporter, JsonModImportResultRepository,
    SandboxModPackageMetadataAnalyzer, TaskScopedModImportSandboxLocator,
    ZipModImportPackagePreparer,
};
pub use mod_import_install_files::SandboxModPackageInstallFileScanner;
pub use prerequisite_rules_repository::JsonGamePrerequisiteRuleRepository;
pub use preview_image::{
    FileSystemThumbnailStore, ImageCratePreviewImageProcessor, SandboxPackagePreviewScanner,
    ThumbnailPruneReport, ThumbnailSizePruneReport,
};
pub use save_backup::FileSystemSaveBackupWriter;
pub use save_directory_pending_store::InMemoryPendingSaveDirectoryCandidateStore;
pub use save_directory_scanner::SteamUserdataSaveDirectoryScanner;
pub use sqlite::open_database;
pub use sqlite::SqliteCategoryRepository;
pub use sqlite::SqliteModMetadataRepository;
pub use sqlite::SqliteProfileRepository;
pub use sqlite::SqliteSaveBackupRepository;
pub use sqlite::SqliteSaveBackupSchedulerStateRepository;
pub use steam_discovery::PlatformSteamRootProvider;
pub use steam_profile::{
    parse_steam_profile_xml, ReqwestSteamProfileHttpTransport, SteamCommunityProfileClient,
    SteamProfileHttpTransport,
};
pub use text_log::FileSystemTextLogReader;

pub struct SystemClock;

impl AppClock for SystemClock {
    fn now_unix_millis(&self) -> Result<u128> {
        Ok(SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis())
    }
}
