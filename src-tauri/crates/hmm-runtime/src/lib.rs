use std::fmt;
use std::path::{Component, Path, PathBuf};

mod backup_automation;
mod composition;
mod diagnostics_automation;
mod external_import;
mod game_automation;
mod install_automation;
mod mod_library;
mod sandbox_write;
mod uninstall;

pub use backup_automation::{
    BackupBackgroundStatusSnapshot, BackupListItemSnapshot, BackupListSnapshot,
    ReadOnlyBackupAutomation, ReadOnlyBackupAutomationError,
};
pub use composition::{
    ConfiguredInstallRecoveryActionPreviewer, ConfiguredInstallRecoveryScanner,
    ConfiguredReinstallExecutor, ConfiguredRetargetReinstallError, HmmRuntime, HmmRuntimeBuilder,
};
pub use diagnostics_automation::{
    DiagnosticsPlatformSnapshot, DiagnosticsSnapshot, ReadOnlyDiagnosticsAutomation,
    ReadOnlyDiagnosticsAutomationError,
};
pub use external_import::ExternalImportComposition;
pub use game_automation::{
    GamePrerequisiteItemSnapshot, GamePrerequisiteSnapshot, GameScanSnapshot, GameStatusSnapshot,
    GameValidationSnapshot, GameValidationState, ReadOnlyGameAutomation,
    ReadOnlyGameAutomationError,
};
pub use hmm_app::{TaskKind, TaskProgressEvent, TaskProgressObserver, TaskStatus};
pub use install_automation::{
    InstallPlanActionSnapshot, InstallPlanConflictSnapshot, InstallPlanSnapshot,
    InstallRecoveryBlockReasonSnapshot, InstallRecoveryIssueSnapshot, InstallRecoveryItemSnapshot,
    InstallRecoveryPreviewSnapshot, InstallRecoveryScanSnapshot, InstallStatusItemSnapshot,
    InstallStatusSnapshot, ReadOnlyInstallAutomation, ReadOnlyInstallAutomationError,
    ReadOnlyInstallRecoveryAction,
};
pub use sandbox_write::{
    SandboxWriteAdmission, SandboxWriteCapability, SandboxWriteCapabilityError, SandboxWriteRoots,
    SANDBOX_MARKER_FILE_NAME, SANDBOX_MARKER_SCHEMA,
};

pub const APP_IDENTIFIER: &str = "dev.helsincy.modmanager";

pub fn production_app_data_dir() -> Option<PathBuf> {
    dirs::data_dir().map(|path| path.join(APP_IDENTIFIER))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeEnvironmentKind {
    Production,
    Sandbox,
}

impl RuntimeEnvironmentKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Production => "production",
            Self::Sandbox => "sandbox",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeDataRootMode {
    System,
    ExplicitSandbox,
}

impl RuntimeDataRootMode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::ExplicitSandbox => "explicit_sandbox",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CliWriteCommandPolicy {
    Disabled,
    SandboxOnly,
}

impl CliWriteCommandPolicy {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::SandboxOnly => "sandbox_only",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeEnvironment {
    kind: RuntimeEnvironmentKind,
    sandbox_data_dir: Option<PathBuf>,
}

impl RuntimeEnvironment {
    pub fn from_options(
        kind: RuntimeEnvironmentKind,
        data_dir: Option<PathBuf>,
    ) -> Result<Self, RuntimeEnvironmentError> {
        match (kind, data_dir) {
            (RuntimeEnvironmentKind::Production, None) => Ok(Self {
                kind: RuntimeEnvironmentKind::Production,
                sandbox_data_dir: None,
            }),
            (RuntimeEnvironmentKind::Production, Some(_)) => {
                Err(RuntimeEnvironmentError::ProductionDataDirForbidden)
            }
            (RuntimeEnvironmentKind::Sandbox, None) => {
                Err(RuntimeEnvironmentError::SandboxDataDirRequired)
            }
            (RuntimeEnvironmentKind::Sandbox, Some(data_dir)) => Self::sandbox(data_dir),
        }
    }

    pub fn sandbox(data_dir: PathBuf) -> Result<Self, RuntimeEnvironmentError> {
        validate_sandbox_data_dir(&data_dir)?;
        Ok(Self {
            kind: RuntimeEnvironmentKind::Sandbox,
            sandbox_data_dir: Some(data_dir),
        })
    }

    pub const fn kind(&self) -> RuntimeEnvironmentKind {
        self.kind
    }

    pub const fn data_root_mode(&self) -> RuntimeDataRootMode {
        match self.kind {
            RuntimeEnvironmentKind::Production => RuntimeDataRootMode::System,
            RuntimeEnvironmentKind::Sandbox => RuntimeDataRootMode::ExplicitSandbox,
        }
    }

    pub const fn cli_write_command_policy(&self) -> CliWriteCommandPolicy {
        match self.kind {
            RuntimeEnvironmentKind::Production => CliWriteCommandPolicy::Disabled,
            RuntimeEnvironmentKind::Sandbox => CliWriteCommandPolicy::SandboxOnly,
        }
    }

    pub fn sandbox_data_dir(&self) -> Option<&Path> {
        self.sandbox_data_dir.as_deref()
    }

    /// Acquires the process-local write capability for a validated Sandbox root.
    ///
    /// Production deliberately has no construction path for this capability. The marker is
    /// created only when a future Sandbox write command explicitly requests admission; read-only
    /// commands and `runtime status` never initialize it.
    pub fn acquire_sandbox_write_capability(
        &self,
    ) -> Result<SandboxWriteCapability, SandboxWriteCapabilityError> {
        SandboxWriteCapability::acquire(self)
    }
}

fn validate_sandbox_data_dir(data_dir: &Path) -> Result<(), RuntimeEnvironmentError> {
    if !data_dir.is_absolute() {
        return Err(RuntimeEnvironmentError::SandboxDataDirMustBeAbsolute);
    }
    if data_dir.file_name().is_none()
        || has_unsafe_lexical_component(data_dir)
        || data_dir
            .components()
            .any(|component| matches!(component, Component::CurDir | Component::ParentDir))
    {
        return Err(RuntimeEnvironmentError::SandboxDataDirUnsafe);
    }
    Ok(())
}

fn has_unsafe_lexical_component(path: &Path) -> bool {
    path.as_os_str()
        .to_string_lossy()
        .split(['/', '\\'])
        .any(|component| matches!(component, "." | ".."))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeEnvironmentError {
    ProductionDataDirForbidden,
    SandboxDataDirRequired,
    SandboxDataDirMustBeAbsolute,
    SandboxDataDirUnsafe,
}

impl RuntimeEnvironmentError {
    pub const fn code(self) -> &'static str {
        match self {
            Self::ProductionDataDirForbidden => "production_data_dir_forbidden",
            Self::SandboxDataDirRequired => "sandbox_data_dir_required",
            Self::SandboxDataDirMustBeAbsolute => "sandbox_data_dir_must_be_absolute",
            Self::SandboxDataDirUnsafe => "sandbox_data_dir_unsafe",
        }
    }
}

impl fmt::Display for RuntimeEnvironmentError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for RuntimeEnvironmentError {}

#[cfg(test)]
mod tests {
    use super::*;
    use hmm_app::{TaskKind, TaskStatus};
    use std::convert::Infallible;
    use std::sync::Mutex;

    #[derive(Default)]
    struct RecordingTaskProgressObserver {
        events: Mutex<Vec<TaskProgressEvent>>,
    }

    impl TaskProgressObserver for RecordingTaskProgressObserver {
        type Error = Infallible;

        fn observe(&self, event: &TaskProgressEvent) -> Result<(), Self::Error> {
            self.events
                .lock()
                .expect("recording observer lock")
                .push(event.clone());
            Ok(())
        }
    }

    fn safe_absolute_sandbox_path() -> PathBuf {
        std::env::current_dir()
            .expect("current directory")
            .join("target")
            .join("hmm-runtime-sandbox")
    }

    #[test]
    fn task_progress_observer_uses_transport_neutral_domain_events() {
        let observer = RecordingTaskProgressObserver::default();
        let event = TaskProgressEvent::new(
            "install-123",
            TaskKind::Install,
            TaskStatus::Running,
            "install.commit",
        );

        observer.observe(&event).expect("record progress event");

        assert_eq!(
            observer
                .events
                .lock()
                .expect("recording observer lock")
                .as_slice(),
            [event]
        );
    }

    #[test]
    fn production_is_read_only_and_does_not_accept_data_dir_override() {
        let environment =
            RuntimeEnvironment::from_options(RuntimeEnvironmentKind::Production, None)
                .expect("production environment");

        assert_eq!(environment.kind(), RuntimeEnvironmentKind::Production);
        assert_eq!(environment.data_root_mode(), RuntimeDataRootMode::System);
        assert_eq!(
            environment.cli_write_command_policy(),
            CliWriteCommandPolicy::Disabled
        );
        assert_eq!(environment.sandbox_data_dir(), None);
        assert_eq!(
            RuntimeEnvironment::from_options(
                RuntimeEnvironmentKind::Production,
                Some(safe_absolute_sandbox_path()),
            ),
            Err(RuntimeEnvironmentError::ProductionDataDirForbidden)
        );
    }

    #[test]
    fn sandbox_requires_a_safe_explicit_absolute_data_dir() {
        assert_eq!(
            RuntimeEnvironment::from_options(RuntimeEnvironmentKind::Sandbox, None),
            Err(RuntimeEnvironmentError::SandboxDataDirRequired)
        );
        assert_eq!(
            RuntimeEnvironment::sandbox(PathBuf::from("relative")),
            Err(RuntimeEnvironmentError::SandboxDataDirMustBeAbsolute)
        );

        let mut parent_dir = safe_absolute_sandbox_path();
        parent_dir.push("..");
        parent_dir.push("escaped");
        assert_eq!(
            RuntimeEnvironment::sandbox(parent_dir),
            Err(RuntimeEnvironmentError::SandboxDataDirUnsafe)
        );

        let mut current_dir = safe_absolute_sandbox_path().into_os_string();
        current_dir.push(r"\.\nested");
        assert_eq!(
            RuntimeEnvironment::sandbox(PathBuf::from(current_dir)),
            Err(RuntimeEnvironmentError::SandboxDataDirUnsafe)
        );
    }

    #[test]
    fn sandbox_exposes_only_an_explicit_sandbox_write_policy() {
        let data_dir = safe_absolute_sandbox_path();
        let environment =
            RuntimeEnvironment::sandbox(data_dir.clone()).expect("sandbox environment");

        assert_eq!(environment.kind(), RuntimeEnvironmentKind::Sandbox);
        assert_eq!(
            environment.data_root_mode(),
            RuntimeDataRootMode::ExplicitSandbox
        );
        assert_eq!(
            environment.cli_write_command_policy(),
            CliWriteCommandPolicy::SandboxOnly
        );
        assert_eq!(environment.sandbox_data_dir(), Some(data_dir.as_path()));
    }

    #[test]
    fn filesystem_root_is_not_a_valid_sandbox_data_dir() {
        let root = std::env::current_dir()
            .expect("current directory")
            .ancestors()
            .last()
            .expect("filesystem root")
            .to_path_buf();

        assert_eq!(
            RuntimeEnvironment::sandbox(root),
            Err(RuntimeEnvironmentError::SandboxDataDirUnsafe)
        );
    }

    #[test]
    fn rust_app_metadata_matches_tauri_configuration() {
        let config: serde_json::Value =
            serde_json::from_str(include_str!("../../../tauri.conf.json")).expect("tauri config");

        assert_eq!(config["identifier"], APP_IDENTIFIER);
        assert_eq!(config["version"], env!("CARGO_PKG_VERSION"));
    }
}
