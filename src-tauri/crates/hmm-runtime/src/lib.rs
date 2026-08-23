use std::fmt;
use std::path::{Component, Path, PathBuf};

mod backup_automation;
mod batch_automation;
mod composition;
mod diagnostics_automation;
mod external_import;
mod game_automation;
mod install_automation;
mod lifecycle_automation;
mod mod_library;
mod sandbox_write;
mod uninstall;

pub use backup_automation::{
    BackupBackgroundStatusSnapshot, BackupListItemSnapshot, BackupListSnapshot,
    ReadOnlyBackupAutomation, ReadOnlyBackupAutomationError,
};
pub use batch_automation::{
    BatchAttemptSnapshot, SandboxBatchAutomationError, SandboxBatchAutomationErrorClass,
    SandboxBatchInstallAutomation, SandboxBatchPlanRequest,
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
    ReadOnlyInstallRecoveryAction, ReinstallBlockingReasonSnapshot, ReinstallPlanSnapshot,
    UninstallPlanSnapshot,
};
pub use lifecycle_automation::{
    LifecycleTaskCancellationHandle, LifecycleTaskOutcome, CliLifecycleAutomation,
    CliLifecycleAutomationError,
};
pub use sandbox_write::{
    SandboxWriteAdmission, SandboxWriteCapability, SandboxWriteCapabilityError, SandboxWriteRoots,
    SANDBOX_MARKER_FILE_NAME, SANDBOX_MARKER_SCHEMA,
};

pub const APP_IDENTIFIER: &str = "dev.helsincy.modmanager";

/// 玩家可见的数据目录名。默认存档备份放在这个目录下，与自定义备份根的布局一致。
pub const USER_DATA_DIRECTORY_NAME: &str = "HelsincyModManager";

pub fn production_app_data_dir() -> Option<PathBuf> {
    dirs::data_dir().map(|path| path.join(APP_IDENTIFIER))
}

/// 默认存档备份根目录。
///
/// **不能**放在 app data 目录下。NSIS 卸载器带一个"删除应用数据"复选框，勾选后执行
/// `RmDir /r $APPDATA\dev.helsincy.modmanager`，会把该目录下的一切连同玩家的全部
/// 存档备份一起删掉；而那个选项的措辞完全看不出包含存档备份。存档是不可恢复数据，
/// 不能挂在一个一键清除的位置上。
///
/// 选文档目录的理由：卸载器不碰它、玩家能自己找到并复制走、无需新增依赖。
/// 若文档目录被重定向到云盘，备份会顺带获得一份异地副本——对存档来说是好事；
/// 不希望如此的玩家可以在界面上改成自定义备份目录。
///
/// 回退顺序：文档目录 → 用户主目录下的 Documents → app data。最后一档是保底，
/// 此时会退回可被卸载清除的位置，但总好过完全无法备份。
pub fn default_save_backup_root(app_data_dir: &Path) -> PathBuf {
    dirs::document_dir()
        .or_else(|| dirs::home_dir().map(|home| home.join("Documents")))
        .unwrap_or_else(|| app_data_dir.to_path_buf())
        .join(USER_DATA_DIRECTORY_NAME)
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
    /// 仅供 crate 内测试把 Production 链路指向临时根。没有任何 CLI 参数、环境变量或
    /// 公开构造器能设置它：`from_options` 与 `sandbox` 恒置 `None`，因此运行时的
    /// Production 数据根始终由操作系统解析。
    production_app_data_override: Option<PathBuf>,
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
                production_app_data_override: None,
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
            production_app_data_override: None,
        })
    }

    /// 测试专用：Production 语义 + 显式临时 app-data 根。crate 私有，CLI 与外部
    /// crate 均不可达，不构成生产写入的注入面。
    #[cfg(test)]
    pub(crate) fn production_with_app_data_root_for_tests(app_data_dir: PathBuf) -> Self {
        Self {
            kind: RuntimeEnvironmentKind::Production,
            sandbox_data_dir: None,
            production_app_data_override: Some(app_data_dir),
        }
    }

    /// Production 数据根解析的唯一入口：正常路径走操作系统解析，测试 override 仅在
    /// crate 内测试构造器下存在。
    pub(crate) fn resolved_production_app_data_dir(&self) -> Option<PathBuf> {
        self.production_app_data_override
            .clone()
            .or_else(production_app_data_dir)
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
    fn default_save_backup_root_stays_outside_the_app_data_directory() {
        let app_data_dir = production_app_data_dir().expect("app data dir");

        let backup_root = default_save_backup_root(&app_data_dir);

        // 这是本函数存在的全部理由：卸载器的"删除应用数据"选项会执行
        // RmDir /r 到 app data 目录，落在它下面的备份会被一并删光。
        // 除非连文档目录和用户主目录都拿不到（此时已无处可放），否则不得落入其中。
        let fell_back_to_app_data = dirs::document_dir().is_none() && dirs::home_dir().is_none();
        if !fell_back_to_app_data {
            assert!(
                !backup_root.starts_with(&app_data_dir),
                "备份根 {backup_root:?} 不得位于 app data 目录 {app_data_dir:?} 之下"
            );
        }
        assert!(backup_root.ends_with(USER_DATA_DIRECTORY_NAME));
    }

    #[test]
    fn default_save_backup_root_falls_back_without_losing_the_directory_name() {
        // 回退到 app data 是保底档：此时备份仍可写，只是失去卸载存活性。
        // 返回值不能是空路径，否则备份会写到进程当前目录。
        let fallback = default_save_backup_root(Path::new("/nonexistent-app-data"));

        assert!(fallback.is_absolute() || fallback.starts_with("/nonexistent-app-data"));
        assert!(fallback.ends_with(USER_DATA_DIRECTORY_NAME));
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
