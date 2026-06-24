mod app_settings_repository;
mod audit_log;
mod game_config_repository;
mod game_directory_probe;
mod game_discovery;
mod mod_import;
mod preview_image;
pub mod steam_discovery;
mod text_log;

use anyhow::Result;
use hmm_ports::AppClock;
use std::time::{SystemTime, UNIX_EPOCH};

pub use app_settings_repository::JsonAppSettingsRepository;
pub use audit_log::FileSystemAuditLogWriter;
pub use game_config_repository::JsonGameConfigRepository;
pub use game_directory_probe::{RealGameDirectoryProbe, RealGameDirectoryProbeFactory};
pub use game_discovery::{NoopGameDiscoveryService, SteamGameDiscoveryService};
pub use mod_import::{
    FileSystemDiagnosticPackageExporter, JsonModImportResultRepository,
    SandboxModPackageMetadataAnalyzer, TaskScopedModImportSandboxLocator,
    ZipModImportPackagePreparer,
};
pub use preview_image::{
    FileSystemThumbnailStore, ImageCratePreviewImageProcessor, SandboxPackagePreviewScanner,
    ThumbnailPruneReport, ThumbnailSizePruneReport,
};
pub use steam_discovery::PlatformSteamRootProvider;
pub use text_log::FileSystemTextLogReader;

pub struct SystemClock;

impl AppClock for SystemClock {
    fn now_unix_millis(&self) -> Result<u128> {
        Ok(SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis())
    }
}
