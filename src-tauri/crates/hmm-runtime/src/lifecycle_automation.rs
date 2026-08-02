use crate::{
    HmmRuntime, ReadOnlyInstallAutomation, ReadOnlyInstallRecoveryAction, RuntimeEnvironment,
    RuntimeEnvironmentKind, SandboxWriteCapability, SandboxWriteRoots,
};
use hmm_app::{
    GamePrerequisiteDecision, InstallRecoveryActionAvailability, InstallRecoveryActionBlockReason,
    InstallRecoveryActionKind, InstallRecoveryActionPreview, InstallRecoveryStatus,
    InstallRecoverySummary, InstallWriteAdmission, InstallWriteAdmissionError,
    ReinstallBlockingReason, ReinstallPlanPreview, ReinstallPreviewStatus, StartInstallTaskRequest,
    StartRecoveryActionTaskRequest, StartReinstallTaskRequest, StartUninstallTaskRequest,
    TaskManager, TaskProgressEvent, TaskProgressObserver,
};
use hmm_core::{FileLayer, GameId, InstallPlan, ModId, ModRevisionId, ProfileId};
use hmm_infra::JsonGameConfigRepository;
use hmm_ports::GameConfigRepository;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fmt;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

const LIFECYCLE_PLAN_TOKEN_PREFIX: &str = "hmm-lifecycle-plan-v1:";
const LIFECYCLE_PLAN_TOKEN_PAYLOAD_HEX_LENGTH: usize = 80;

#[cfg(test)]
use std::{fs, path::Path};

#[cfg(test)]
/// Builds a disposable Sandbox fixture with one configured game and one importable Mod
/// (`mod-a` / `package-a`), including prerequisite files so install preflight passes.
pub(crate) fn write_install_fixture(sandbox: &Path) -> PathBuf {
    fs::write(
        sandbox.join(crate::SANDBOX_MARKER_FILE_NAME),
        crate::SANDBOX_MARKER_SCHEMA,
    )
    .expect("sandbox marker");
    let game_root = sandbox.join("fixtures/games/mhw-minimal");
    fs::create_dir_all(game_root.join("nativePC/models")).expect("game fixture");
    fs::create_dir_all(game_root.join("nativePC/plugins")).expect("prerequisite fixture directory");
    fs::write(game_root.join("MonsterHunterWorld.exe"), b"fixture").expect("game executable");
    for relative_path in [
        "dinput8.dll",
        "loader.dll",
        "nativePC/plugins/MonsterLoader.dll",
        "nativePC/plugins/QuestLoader.dll",
        "nativePC/plugins/!CRCBypass.dll",
    ] {
        fs::write(game_root.join(relative_path), b"artificial-prerequisite")
            .expect("write prerequisite fixture");
    }
    fs::write(
        game_root.join("loader-config.json"),
        br#"{"enablePluginLoader":true}"#,
    )
    .expect("write prerequisite config");
    let config_root = sandbox.join("config");
    fs::create_dir_all(&config_root).expect("config root");
    fs::write(
        config_root.join("games.json"),
        serde_json::json!({
            "version": 1,
            "games": [{
                "id": "mhw-default",
                "game_id": "mhw",
                "display_name": "MHW fixture",
                "root_dir": game_root,
                "status": "configured",
                "configured_at_unix_millis": 42
            }]
        })
        .to_string(),
    )
    .expect("game config");
    let catalog_root = sandbox.join("mod-import");
    fs::create_dir_all(&catalog_root).expect("catalog root");
    fs::write(
        catalog_root.join("results.json"),
        r#"{
  "version": 1,
  "records": [{
    "mod_id": "mod-a",
    "task_id": "task-a",
    "package_id": "package-a",
    "display_name": "Fixture Mod"
  }]
}"#,
    )
    .expect("Mod catalog");
    let package_root = catalog_root.join("sandboxes/package-a/nativePC/models");
    fs::create_dir_all(&package_root).expect("package root");
    fs::write(package_root.join("player.mod3"), b"fixture").expect("package file");
    game_root
}
const LIFECYCLE_PLAN_TOKEN_TTL_MILLIS: u128 = 5 * 60 * 1000;
const INSTALL_APPLY_COMMAND: &str = "install.apply";
const INSTALL_UNINSTALL_COMMAND: &str = "install.uninstall";
const INSTALL_REINSTALL_COMMAND: &str = "install.reinstall";
const INSTALL_RECOVERY_APPLY_COMMAND: &str = "install.recovery.apply";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LifecycleTaskOutcome {
    pub task_id: String,
    pub events: Vec<TaskProgressEvent>,
}

#[derive(Clone)]
pub struct LifecycleTaskCancellationHandle {
    task_manager: Arc<TaskManager>,
}

impl LifecycleTaskCancellationHandle {
    pub fn cancel_task(&self, task_id: &str) -> bool {
        self.task_manager.cancel_task(task_id).is_ok()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SandboxLifecycleAutomationError {
    ProductionForbidden,
    PlanBlocked,
    PlanUnavailable,
    PlanTokenExpired,
    PlanTokenInvalid,
    RecoveryBlocked,
    ReinstallBlocked,
    RuntimeUnavailable,
    TaskFailed { task_id: String, code: &'static str },
    TaskUnavailable,
    UninstallBlocked,
    WriteRejected,
}

impl SandboxLifecycleAutomationError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::ProductionForbidden => "sandbox_lifecycle_production_forbidden",
            Self::PlanBlocked => "install_plan_blocked",
            Self::PlanUnavailable => "install_plan_unavailable",
            Self::PlanTokenExpired => "plan_token_expired",
            Self::PlanTokenInvalid => "plan_token_invalid",
            Self::RecoveryBlocked => "recovery_action_not_available",
            Self::ReinstallBlocked => "reinstall_not_available",
            Self::RuntimeUnavailable => "sandbox_lifecycle_runtime_unavailable",
            Self::TaskFailed { code, .. } => code,
            Self::TaskUnavailable => "install_task_unavailable",
            Self::UninstallBlocked => "uninstall_not_available",
            Self::WriteRejected => "sandbox_write_rejected",
        }
    }

    pub fn task_id(&self) -> Option<&str> {
        match self {
            Self::TaskFailed { task_id, .. } => Some(task_id),
            _ => None,
        }
    }
}

impl fmt::Display for SandboxLifecycleAutomationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for SandboxLifecycleAutomationError {}

pub struct SandboxLifecycleAutomation {
    runtime: HmmRuntime,
    install_request: Option<StartInstallTaskRequest>,
    uninstall_request: Option<StartUninstallTaskRequest>,
    reinstall_request: Option<StartReinstallTaskRequest>,
    recovery_request: Option<StartRecoveryActionTaskRequest>,
}

impl SandboxLifecycleAutomation {
    pub fn prepare_install(
        environment: &RuntimeEnvironment,
        game_id: &str,
        profile_id: &str,
        mod_id: &str,
        plan_token: &str,
    ) -> Result<Self, SandboxLifecycleAutomationError> {
        if environment.kind() != RuntimeEnvironmentKind::Sandbox {
            return Err(SandboxLifecycleAutomationError::ProductionForbidden);
        }
        let sandbox_root = environment
            .sandbox_data_dir()
            .ok_or(SandboxLifecycleAutomationError::ProductionForbidden)?
            .to_path_buf();
        let read_only = Arc::new(
            ReadOnlyInstallAutomation::from_environment(environment)
                .map_err(|_| SandboxLifecycleAutomationError::PlanUnavailable)?,
        );
        let (game_id, profile_id, mod_id, plan, prerequisite_decision) = read_only
            .build_install_plan(game_id, profile_id, mod_id)
            .map_err(|_| SandboxLifecycleAutomationError::PlanUnavailable)?;
        if prerequisite_decision.is_blocked() || plan.has_blocking_conflicts() {
            return Err(SandboxLifecycleAutomationError::PlanBlocked);
        }
        validate_install_plan_token(
            plan_token,
            &game_id,
            &profile_id,
            &mod_id,
            &plan,
            &prerequisite_decision,
        )?;

        let capability = Arc::new(
            environment
                .acquire_sandbox_write_capability()
                .map_err(|_| SandboxLifecycleAutomationError::WriteRejected)?,
        );
        let game_config_repository: Arc<dyn GameConfigRepository> = Arc::new(
            JsonGameConfigRepository::new(sandbox_root.join("config").join("games.json")),
        );
        let write_admission: Arc<dyn InstallWriteAdmission> =
            Arc::new(SandboxInstallWriteAdmission {
                capability,
                sandbox_root: sandbox_root.clone(),
                game_config_repository,
                expected_game_id: game_id.clone(),
                expected_profile_id: profile_id.clone(),
                expected_mod_id: mod_id.clone(),
                plan_token: plan_token.to_owned(),
            });
        let runtime = HmmRuntime::builder(sandbox_root)
            .with_sandbox_write_admission(write_admission)
            .build()
            .map_err(|_| SandboxLifecycleAutomationError::RuntimeUnavailable)?;

        Ok(Self {
            runtime,
            install_request: Some(StartInstallTaskRequest {
                game_id,
                mod_id,
                profile_id,
                layer: FileLayer::new("base", 0),
            }),
            uninstall_request: None,
            reinstall_request: None,
            recovery_request: None,
        })
    }

    pub fn prepare_uninstall(
        environment: &RuntimeEnvironment,
        game_id: &str,
        profile_id: &str,
        mod_id: &str,
        plan_token: &str,
    ) -> Result<Self, SandboxLifecycleAutomationError> {
        if environment.kind() != RuntimeEnvironmentKind::Sandbox {
            return Err(SandboxLifecycleAutomationError::ProductionForbidden);
        }
        let sandbox_root = environment
            .sandbox_data_dir()
            .ok_or(SandboxLifecycleAutomationError::ProductionForbidden)?
            .to_path_buf();
        let read_only = Arc::new(
            ReadOnlyInstallAutomation::from_environment(environment)
                .map_err(|_| SandboxLifecycleAutomationError::PlanUnavailable)?,
        );
        let (game_id, profile_id, mod_id, summary, state_binding) = read_only
            .build_uninstall_facts(game_id, profile_id, mod_id)
            .map_err(|_| SandboxLifecycleAutomationError::PlanUnavailable)?;
        if summary.status != InstallRecoveryStatus::Completed {
            return Err(SandboxLifecycleAutomationError::UninstallBlocked);
        }
        validate_uninstall_plan_token(
            plan_token,
            &game_id,
            &profile_id,
            &mod_id,
            &summary,
            &state_binding,
        )?;

        let capability = Arc::new(
            environment
                .acquire_sandbox_write_capability()
                .map_err(|_| SandboxLifecycleAutomationError::WriteRejected)?,
        );
        let game_config_repository: Arc<dyn GameConfigRepository> = Arc::new(
            JsonGameConfigRepository::new(sandbox_root.join("config").join("games.json")),
        );
        let write_admission: Arc<dyn InstallWriteAdmission> =
            Arc::new(SandboxUninstallWriteAdmission {
                capability,
                sandbox_root: sandbox_root.clone(),
                game_config_repository,
                state_reader: Arc::clone(&read_only),
                expected_game_id: game_id.clone(),
                expected_profile_id: profile_id.clone(),
                expected_mod_id: mod_id.clone(),
                expected_summary: summary,
                expected_state_binding: state_binding,
                plan_token: plan_token.to_owned(),
            });
        let runtime = HmmRuntime::builder(sandbox_root)
            .with_sandbox_write_admission(write_admission)
            .build()
            .map_err(|_| SandboxLifecycleAutomationError::RuntimeUnavailable)?;

        Ok(Self {
            runtime,
            install_request: None,
            uninstall_request: Some(StartUninstallTaskRequest {
                game_id,
                mod_id,
                profile_id,
            }),
            reinstall_request: None,
            recovery_request: None,
        })
    }

    pub fn prepare_recovery(
        environment: &RuntimeEnvironment,
        game_id: &str,
        profile_id: &str,
        mod_id: &str,
        action: ReadOnlyInstallRecoveryAction,
        plan_token: &str,
    ) -> Result<Self, SandboxLifecycleAutomationError> {
        if environment.kind() != RuntimeEnvironmentKind::Sandbox {
            return Err(SandboxLifecycleAutomationError::ProductionForbidden);
        }
        let sandbox_root = environment
            .sandbox_data_dir()
            .ok_or(SandboxLifecycleAutomationError::ProductionForbidden)?
            .to_path_buf();
        let read_only = Arc::new(
            ReadOnlyInstallAutomation::from_environment(environment)
                .map_err(|_| SandboxLifecycleAutomationError::PlanUnavailable)?,
        );
        let (game_id, profile_id, mod_id, preview, state_binding) = read_only
            .build_recovery_preview_facts(game_id, profile_id, mod_id, action)
            .map_err(|_| SandboxLifecycleAutomationError::PlanUnavailable)?;
        if preview.availability != InstallRecoveryActionAvailability::Available {
            return Err(SandboxLifecycleAutomationError::RecoveryBlocked);
        }
        validate_recovery_plan_token(
            plan_token,
            &game_id,
            &profile_id,
            &mod_id,
            &preview,
            &state_binding,
        )?;

        let capability = Arc::new(
            environment
                .acquire_sandbox_write_capability()
                .map_err(|_| SandboxLifecycleAutomationError::WriteRejected)?,
        );
        let game_config_repository: Arc<dyn GameConfigRepository> = Arc::new(
            JsonGameConfigRepository::new(sandbox_root.join("config").join("games.json")),
        );
        let action_kind = preview.action_kind;
        let write_admission: Arc<dyn InstallWriteAdmission> =
            Arc::new(SandboxRecoveryWriteAdmission {
                capability,
                sandbox_root: sandbox_root.clone(),
                game_config_repository,
                state_reader: Arc::clone(&read_only),
                expected_game_id: game_id.clone(),
                expected_profile_id: profile_id.clone(),
                expected_mod_id: mod_id.clone(),
                expected_preview: preview,
                expected_state_binding: state_binding,
                plan_token: plan_token.to_owned(),
            });
        let runtime = HmmRuntime::builder(sandbox_root)
            .with_sandbox_write_admission(write_admission)
            .build()
            .map_err(|_| SandboxLifecycleAutomationError::RuntimeUnavailable)?;

        Ok(Self {
            runtime,
            install_request: None,
            uninstall_request: None,
            reinstall_request: None,
            recovery_request: Some(StartRecoveryActionTaskRequest {
                game_id,
                mod_id,
                profile_id,
                action_kind,
            }),
        })
    }

    pub fn prepare_reinstall(
        environment: &RuntimeEnvironment,
        game_id: &str,
        profile_id: &str,
        mod_id: &str,
        candidate_revision_id: &str,
        plan_token: &str,
    ) -> Result<Self, SandboxLifecycleAutomationError> {
        if environment.kind() != RuntimeEnvironmentKind::Sandbox {
            return Err(SandboxLifecycleAutomationError::ProductionForbidden);
        }
        let sandbox_root = environment
            .sandbox_data_dir()
            .ok_or(SandboxLifecycleAutomationError::ProductionForbidden)?
            .to_path_buf();
        let read_only = Arc::new(
            ReadOnlyInstallAutomation::from_environment(environment)
                .map_err(|_| SandboxLifecycleAutomationError::PlanUnavailable)?,
        );
        let (game_id, profile_id, mod_id, candidate_revision_id, preview) = read_only
            .build_reinstall_facts(game_id, profile_id, mod_id, candidate_revision_id)
            .map_err(|_| SandboxLifecycleAutomationError::PlanUnavailable)?;
        if preview.status != ReinstallPreviewStatus::Ready {
            return Err(SandboxLifecycleAutomationError::ReinstallBlocked);
        }
        let internal_plan_token = preview
            .plan_token
            .clone()
            .ok_or(SandboxLifecycleAutomationError::ReinstallBlocked)?;
        validate_reinstall_plan_token(
            plan_token,
            &game_id,
            &profile_id,
            &mod_id,
            &candidate_revision_id,
            &preview,
        )?;

        let capability = Arc::new(
            environment
                .acquire_sandbox_write_capability()
                .map_err(|_| SandboxLifecycleAutomationError::WriteRejected)?,
        );
        let game_config_repository: Arc<dyn GameConfigRepository> = Arc::new(
            JsonGameConfigRepository::new(sandbox_root.join("config").join("games.json")),
        );
        let write_admission: Arc<dyn InstallWriteAdmission> =
            Arc::new(SandboxReinstallWriteAdmission {
                capability,
                sandbox_root: sandbox_root.clone(),
                game_config_repository,
                expected_game_id: game_id.clone(),
                expected_profile_id: profile_id.clone(),
                expected_mod_id: mod_id.clone(),
                expected_candidate_revision_id: candidate_revision_id.clone(),
                expected_internal_plan_token: internal_plan_token.clone(),
                expected_preview: preview,
                plan_token: plan_token.to_owned(),
            });
        let runtime = HmmRuntime::builder(sandbox_root)
            .with_sandbox_write_admission(write_admission)
            .build()
            .map_err(|_| SandboxLifecycleAutomationError::RuntimeUnavailable)?;

        Ok(Self {
            runtime,
            install_request: None,
            uninstall_request: None,
            reinstall_request: Some(StartReinstallTaskRequest {
                game_id,
                profile_id,
                mod_id,
                candidate_revision_id,
                layer: FileLayer::new("base", 0),
                plan_token: internal_plan_token,
            }),
            recovery_request: None,
        })
    }

    pub fn run_install(&self) -> Result<LifecycleTaskOutcome, SandboxLifecycleAutomationError> {
        self.run_install_with_observer(&NoopLifecycleObserver)
    }

    pub fn run_install_with_observer<O: TaskProgressObserver + ?Sized>(
        &self,
        observer: &O,
    ) -> Result<LifecycleTaskOutcome, SandboxLifecycleAutomationError> {
        let request = self
            .install_request
            .as_ref()
            .ok_or(SandboxLifecycleAutomationError::TaskUnavailable)?;
        let started = self
            .runtime
            .install_tasks
            .start_install_task(request.clone())
            .map_err(|_| SandboxLifecycleAutomationError::TaskUnavailable)?;
        let _ = observer.observe(&TaskProgressEvent::new(
            started.task_id.clone(),
            started.kind,
            started.status,
            "install.queued",
        ));
        match self
            .runtime
            .install_task_runner
            .run_install_task_with_observer(&started.task_id, request.clone(), observer)
        {
            Ok(events) => Ok(LifecycleTaskOutcome {
                task_id: started.task_id,
                events,
            }),
            Err(_) => Err(SandboxLifecycleAutomationError::TaskFailed {
                task_id: started.task_id,
                code: "install_task_failed",
            }),
        }
    }

    pub fn run_uninstall(&self) -> Result<LifecycleTaskOutcome, SandboxLifecycleAutomationError> {
        self.run_uninstall_with_observer(&NoopLifecycleObserver)
    }

    pub fn run_uninstall_with_observer<O: TaskProgressObserver + ?Sized>(
        &self,
        observer: &O,
    ) -> Result<LifecycleTaskOutcome, SandboxLifecycleAutomationError> {
        let request = self
            .uninstall_request
            .as_ref()
            .ok_or(SandboxLifecycleAutomationError::TaskUnavailable)?;
        let started = self
            .runtime
            .uninstall_tasks
            .start_uninstall_task(request.clone())
            .map_err(|_| SandboxLifecycleAutomationError::TaskUnavailable)?;
        let _ = observer.observe(&TaskProgressEvent::new(
            started.task_id.clone(),
            started.kind,
            started.status,
            "install.uninstall.queued",
        ));
        match self
            .runtime
            .uninstall_task_runner
            .run_uninstall_task_with_observer(&started.task_id, request.clone(), observer)
        {
            Ok(events) => Ok(LifecycleTaskOutcome {
                task_id: started.task_id,
                events,
            }),
            Err(_) => Err(SandboxLifecycleAutomationError::TaskFailed {
                task_id: started.task_id,
                code: "install_uninstall_task_failed",
            }),
        }
    }

    pub fn run_recovery(&self) -> Result<LifecycleTaskOutcome, SandboxLifecycleAutomationError> {
        self.run_recovery_with_observer(&NoopLifecycleObserver)
    }

    pub fn run_reinstall(&self) -> Result<LifecycleTaskOutcome, SandboxLifecycleAutomationError> {
        self.run_reinstall_with_observer(&NoopLifecycleObserver)
    }

    pub fn run_reinstall_with_observer<O: TaskProgressObserver + ?Sized>(
        &self,
        observer: &O,
    ) -> Result<LifecycleTaskOutcome, SandboxLifecycleAutomationError> {
        let request = self
            .reinstall_request
            .as_ref()
            .ok_or(SandboxLifecycleAutomationError::TaskUnavailable)?;
        let started = self
            .runtime
            .reinstall_tasks
            .start_reinstall_task(request.clone())
            .map_err(|_| SandboxLifecycleAutomationError::TaskUnavailable)?;
        let _ = observer.observe(&TaskProgressEvent::new(
            started.task_id.clone(),
            started.kind,
            started.status,
            "install.reinstall.queued",
        ));
        match self
            .runtime
            .reinstall_task_runner
            .run_reinstall_task_with_observer(&started.task_id, request.clone(), observer)
        {
            Ok(events) => Ok(LifecycleTaskOutcome {
                task_id: started.task_id,
                events,
            }),
            Err(_) => Err(SandboxLifecycleAutomationError::TaskFailed {
                task_id: started.task_id,
                code: "install_reinstall_task_failed",
            }),
        }
    }

    pub fn run_recovery_with_observer<O: TaskProgressObserver + ?Sized>(
        &self,
        observer: &O,
    ) -> Result<LifecycleTaskOutcome, SandboxLifecycleAutomationError> {
        let request = self
            .recovery_request
            .as_ref()
            .ok_or(SandboxLifecycleAutomationError::TaskUnavailable)?;
        let started = self
            .runtime
            .recovery_action_tasks
            .start_recovery_action_task(request.clone())
            .map_err(|_| SandboxLifecycleAutomationError::TaskUnavailable)?;
        let _ = observer.observe(&TaskProgressEvent::new(
            started.task_id.clone(),
            started.kind,
            started.status,
            "install.recovery.queued",
        ));
        match self
            .runtime
            .recovery_action_task_runner
            .run_recovery_action_task_with_observer(&started.task_id, request.clone(), observer)
        {
            Ok(events) => Ok(LifecycleTaskOutcome {
                task_id: started.task_id,
                events,
            }),
            Err(_) => Err(SandboxLifecycleAutomationError::TaskFailed {
                task_id: started.task_id,
                code: "install_recovery_task_failed",
            }),
        }
    }

    pub fn task_log_writer(&self) -> Arc<dyn hmm_ports::TaskLogWriter> {
        Arc::clone(&self.runtime.task_log_writer)
    }

    pub fn cancellation_handle(&self) -> LifecycleTaskCancellationHandle {
        LifecycleTaskCancellationHandle {
            task_manager: Arc::clone(&self.runtime.task_manager),
        }
    }
}

struct SandboxInstallWriteAdmission {
    capability: Arc<SandboxWriteCapability>,
    sandbox_root: PathBuf,
    game_config_repository: Arc<dyn GameConfigRepository>,
    expected_game_id: GameId,
    expected_profile_id: ProfileId,
    expected_mod_id: ModId,
    plan_token: String,
}

impl InstallWriteAdmission for SandboxInstallWriteAdmission {
    fn ensure_write_allowed(
        &self,
        game_id: &GameId,
        profile_id: &ProfileId,
    ) -> Result<(), InstallWriteAdmissionError> {
        revalidate_sandbox_write_roots(
            self.capability.as_ref(),
            &self.sandbox_root,
            self.game_config_repository.as_ref(),
            &self.expected_game_id,
            &self.expected_profile_id,
            game_id,
            profile_id,
        )
    }

    fn ensure_install_plan_allowed(
        &self,
        game_id: &GameId,
        profile_id: &ProfileId,
        mod_id: &ModId,
        plan: &InstallPlan,
        prerequisite_decision: &GamePrerequisiteDecision,
    ) -> Result<(), InstallWriteAdmissionError> {
        if mod_id != &self.expected_mod_id {
            return Err(InstallWriteAdmissionError::SafetyRejected);
        }
        if prerequisite_decision.is_blocked() || plan.has_blocking_conflicts() {
            return Err(InstallWriteAdmissionError::SafetyRejected);
        }
        validate_install_plan_token(
            &self.plan_token,
            &self.expected_game_id,
            &self.expected_profile_id,
            &self.expected_mod_id,
            plan,
            prerequisite_decision,
        )
        .map_err(|_| InstallWriteAdmissionError::SafetyRejected)?;
        self.ensure_write_allowed(game_id, profile_id)
    }
}

struct SandboxUninstallWriteAdmission {
    capability: Arc<SandboxWriteCapability>,
    sandbox_root: PathBuf,
    game_config_repository: Arc<dyn GameConfigRepository>,
    state_reader: Arc<ReadOnlyInstallAutomation>,
    expected_game_id: GameId,
    expected_profile_id: ProfileId,
    expected_mod_id: ModId,
    expected_summary: InstallRecoverySummary,
    expected_state_binding: String,
    plan_token: String,
}

impl InstallWriteAdmission for SandboxUninstallWriteAdmission {
    fn ensure_write_allowed(
        &self,
        game_id: &GameId,
        profile_id: &ProfileId,
    ) -> Result<(), InstallWriteAdmissionError> {
        if self.expected_summary.status != InstallRecoveryStatus::Completed {
            return Err(InstallWriteAdmissionError::SafetyRejected);
        }
        validate_uninstall_plan_token(
            &self.plan_token,
            &self.expected_game_id,
            &self.expected_profile_id,
            &self.expected_mod_id,
            &self.expected_summary,
            &self.expected_state_binding,
        )
        .map_err(|_| InstallWriteAdmissionError::SafetyRejected)?;
        revalidate_sandbox_write_roots(
            self.capability.as_ref(),
            &self.sandbox_root,
            self.game_config_repository.as_ref(),
            &self.expected_game_id,
            &self.expected_profile_id,
            game_id,
            profile_id,
        )?;
        self.ensure_lifecycle_state_unchanged()
    }
}

impl SandboxUninstallWriteAdmission {
    fn ensure_lifecycle_state_unchanged(&self) -> Result<(), InstallWriteAdmissionError> {
        let current = self
            .state_reader
            .load_lifecycle_state_binding(&self.expected_profile_id, &self.expected_mod_id)
            .map_err(|_| InstallWriteAdmissionError::SafetyRejected)?;
        if current == self.expected_state_binding {
            Ok(())
        } else {
            Err(InstallWriteAdmissionError::SafetyRejected)
        }
    }
}

struct SandboxReinstallWriteAdmission {
    capability: Arc<SandboxWriteCapability>,
    sandbox_root: PathBuf,
    game_config_repository: Arc<dyn GameConfigRepository>,
    expected_game_id: GameId,
    expected_profile_id: ProfileId,
    expected_mod_id: ModId,
    expected_candidate_revision_id: ModRevisionId,
    expected_internal_plan_token: String,
    expected_preview: ReinstallPlanPreview,
    plan_token: String,
}

impl InstallWriteAdmission for SandboxReinstallWriteAdmission {
    fn ensure_write_allowed(
        &self,
        game_id: &GameId,
        profile_id: &ProfileId,
    ) -> Result<(), InstallWriteAdmissionError> {
        if self.expected_preview.status != ReinstallPreviewStatus::Ready
            || self.expected_preview.plan_token.as_deref()
                != Some(self.expected_internal_plan_token.as_str())
        {
            return Err(InstallWriteAdmissionError::SafetyRejected);
        }
        validate_reinstall_plan_token(
            &self.plan_token,
            &self.expected_game_id,
            &self.expected_profile_id,
            &self.expected_mod_id,
            &self.expected_candidate_revision_id,
            &self.expected_preview,
        )
        .map_err(|_| InstallWriteAdmissionError::SafetyRejected)?;
        revalidate_sandbox_write_roots(
            self.capability.as_ref(),
            &self.sandbox_root,
            self.game_config_repository.as_ref(),
            &self.expected_game_id,
            &self.expected_profile_id,
            game_id,
            profile_id,
        )
    }
}

struct SandboxRecoveryWriteAdmission {
    capability: Arc<SandboxWriteCapability>,
    sandbox_root: PathBuf,
    game_config_repository: Arc<dyn GameConfigRepository>,
    state_reader: Arc<ReadOnlyInstallAutomation>,
    expected_game_id: GameId,
    expected_profile_id: ProfileId,
    expected_mod_id: ModId,
    expected_preview: InstallRecoveryActionPreview,
    expected_state_binding: String,
    plan_token: String,
}

impl InstallWriteAdmission for SandboxRecoveryWriteAdmission {
    fn ensure_write_allowed(
        &self,
        game_id: &GameId,
        profile_id: &ProfileId,
    ) -> Result<(), InstallWriteAdmissionError> {
        if self.expected_preview.availability != InstallRecoveryActionAvailability::Available {
            return Err(InstallWriteAdmissionError::SafetyRejected);
        }
        validate_recovery_plan_token(
            &self.plan_token,
            &self.expected_game_id,
            &self.expected_profile_id,
            &self.expected_mod_id,
            &self.expected_preview,
            &self.expected_state_binding,
        )
        .map_err(|_| InstallWriteAdmissionError::SafetyRejected)?;
        revalidate_sandbox_write_roots(
            self.capability.as_ref(),
            &self.sandbox_root,
            self.game_config_repository.as_ref(),
            &self.expected_game_id,
            &self.expected_profile_id,
            game_id,
            profile_id,
        )?;
        self.ensure_lifecycle_state_unchanged()
    }
}

impl SandboxRecoveryWriteAdmission {
    fn ensure_lifecycle_state_unchanged(&self) -> Result<(), InstallWriteAdmissionError> {
        let current = self
            .state_reader
            .load_lifecycle_state_binding(&self.expected_profile_id, &self.expected_mod_id)
            .map_err(|_| InstallWriteAdmissionError::SafetyRejected)?;
        if current == self.expected_state_binding {
            Ok(())
        } else {
            Err(InstallWriteAdmissionError::SafetyRejected)
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn revalidate_sandbox_write_roots(
    capability: &SandboxWriteCapability,
    sandbox_root: &std::path::Path,
    game_config_repository: &dyn GameConfigRepository,
    expected_game_id: &GameId,
    expected_profile_id: &ProfileId,
    game_id: &GameId,
    profile_id: &ProfileId,
) -> Result<(), InstallWriteAdmissionError> {
    if game_id != expected_game_id || profile_id != expected_profile_id {
        return Err(InstallWriteAdmissionError::SafetyRejected);
    }
    let game_instance = game_config_repository
        .load_game_instance(game_id)
        .map_err(|_| InstallWriteAdmissionError::SafetyRejected)?
        .ok_or(InstallWriteAdmissionError::SafetyRejected)?;
    capability
        .admit_roots(SandboxWriteRoots::new(
            sandbox_root.to_path_buf(),
            game_instance.root_dir,
        ))
        .map_err(|_| InstallWriteAdmissionError::SafetyRejected)?
        .revalidate()
        .map_err(|_| InstallWriteAdmissionError::SafetyRejected)
}

struct NoopLifecycleObserver;

impl TaskProgressObserver for NoopLifecycleObserver {
    type Error = std::convert::Infallible;

    fn observe(&self, _event: &TaskProgressEvent) -> Result<(), Self::Error> {
        Ok(())
    }
}

pub(crate) struct IssuedLifecyclePlanToken {
    pub token: String,
    pub expires_at_unix_millis: u128,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct InstallPlanTokenFacts<'a> {
    schema_version: &'static str,
    command: &'static str,
    environment: &'static str,
    game_id: &'a GameId,
    profile_id: &'a ProfileId,
    mod_id: &'a ModId,
    plan: &'a InstallPlan,
    prerequisite_status: &'static str,
    prerequisite_rules_version: Option<u32>,
    prerequisite_codes: Vec<&'static str>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct UninstallPlanTokenFacts<'a> {
    schema_version: &'static str,
    command: &'static str,
    environment: &'static str,
    game_id: &'a GameId,
    profile_id: &'a ProfileId,
    mod_id: &'a ModId,
    status: &'static str,
    managed_file_count: usize,
    backup_count: usize,
    issue_count: usize,
    state_binding: &'a str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RecoveryPlanTokenFacts<'a> {
    schema_version: &'static str,
    command: &'static str,
    environment: &'static str,
    game_id: &'a GameId,
    profile_id: &'a ProfileId,
    mod_id: &'a ModId,
    action: &'static str,
    availability: &'static str,
    remove_file_count: usize,
    restore_file_count: usize,
    backup_count: usize,
    blocking_issue_count: usize,
    blocking_reasons: Vec<RecoveryBlockReasonTokenFacts>,
    state_binding: &'a str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RecoveryBlockReasonTokenFacts {
    code: &'static str,
    count: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ReinstallPlanTokenFacts<'a> {
    schema_version: &'static str,
    command: &'static str,
    environment: &'static str,
    game_id: &'a GameId,
    profile_id: &'a ProfileId,
    mod_id: &'a ModId,
    candidate_revision_id: &'a ModRevisionId,
    status: &'static str,
    installed_revision_id: Option<&'a ModRevisionId>,
    preview_candidate_revision_id: Option<&'a ModRevisionId>,
    retained_count: usize,
    replaced_count: usize,
    added_count: usize,
    stale_count: usize,
    blocking_reasons: Vec<ReinstallBlockReasonTokenFacts>,
    internal_plan_token: Option<&'a str>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ReinstallBlockReasonTokenFacts {
    code: &'static str,
    count: usize,
}

pub(crate) fn issue_install_plan_token(
    game_id: &GameId,
    profile_id: &ProfileId,
    mod_id: &ModId,
    plan: &InstallPlan,
    prerequisite_decision: &GamePrerequisiteDecision,
) -> Result<IssuedLifecyclePlanToken, SandboxLifecycleAutomationError> {
    if plan.has_blocking_conflicts() || prerequisite_decision.is_blocked() {
        return Err(SandboxLifecycleAutomationError::PlanBlocked);
    }
    let now = now_unix_millis()?;
    let expires_at_unix_millis = now
        .checked_add(LIFECYCLE_PLAN_TOKEN_TTL_MILLIS)
        .ok_or(SandboxLifecycleAutomationError::PlanTokenInvalid)?;
    let token = build_install_plan_token(
        expires_at_unix_millis,
        game_id,
        profile_id,
        mod_id,
        plan,
        prerequisite_decision,
    )?;
    Ok(IssuedLifecyclePlanToken {
        token,
        expires_at_unix_millis,
    })
}

pub(crate) fn issue_uninstall_plan_token(
    game_id: &GameId,
    profile_id: &ProfileId,
    mod_id: &ModId,
    summary: &InstallRecoverySummary,
    state_binding: &str,
) -> Result<IssuedLifecyclePlanToken, SandboxLifecycleAutomationError> {
    let now = now_unix_millis()?;
    let expires_at_unix_millis = now
        .checked_add(LIFECYCLE_PLAN_TOKEN_TTL_MILLIS)
        .ok_or(SandboxLifecycleAutomationError::PlanTokenInvalid)?;
    let token = build_uninstall_plan_token(
        expires_at_unix_millis,
        game_id,
        profile_id,
        mod_id,
        summary,
        state_binding,
    )?;
    Ok(IssuedLifecyclePlanToken {
        token,
        expires_at_unix_millis,
    })
}

pub(crate) fn issue_recovery_plan_token(
    game_id: &GameId,
    profile_id: &ProfileId,
    mod_id: &ModId,
    preview: &InstallRecoveryActionPreview,
    state_binding: &str,
) -> Result<IssuedLifecyclePlanToken, SandboxLifecycleAutomationError> {
    let now = now_unix_millis()?;
    let expires_at_unix_millis = now
        .checked_add(LIFECYCLE_PLAN_TOKEN_TTL_MILLIS)
        .ok_or(SandboxLifecycleAutomationError::PlanTokenInvalid)?;
    let token = build_recovery_plan_token(
        expires_at_unix_millis,
        game_id,
        profile_id,
        mod_id,
        preview,
        state_binding,
    )?;
    Ok(IssuedLifecyclePlanToken {
        token,
        expires_at_unix_millis,
    })
}

pub(crate) fn issue_reinstall_plan_token(
    game_id: &GameId,
    profile_id: &ProfileId,
    mod_id: &ModId,
    candidate_revision_id: &ModRevisionId,
    preview: &ReinstallPlanPreview,
) -> Result<IssuedLifecyclePlanToken, SandboxLifecycleAutomationError> {
    let now = now_unix_millis()?;
    let expires_at_unix_millis = now
        .checked_add(LIFECYCLE_PLAN_TOKEN_TTL_MILLIS)
        .ok_or(SandboxLifecycleAutomationError::PlanTokenInvalid)?;
    let token = build_reinstall_plan_token(
        expires_at_unix_millis,
        game_id,
        profile_id,
        mod_id,
        candidate_revision_id,
        preview,
    )?;
    Ok(IssuedLifecyclePlanToken {
        token,
        expires_at_unix_millis,
    })
}

fn validate_install_plan_token(
    token: &str,
    game_id: &GameId,
    profile_id: &ProfileId,
    mod_id: &ModId,
    plan: &InstallPlan,
    prerequisite_decision: &GamePrerequisiteDecision,
) -> Result<(), SandboxLifecycleAutomationError> {
    let expires_at_unix_millis = parse_token_expiry(token)?;
    if now_unix_millis()? >= expires_at_unix_millis {
        return Err(SandboxLifecycleAutomationError::PlanTokenExpired);
    }
    let expected = build_install_plan_token(
        expires_at_unix_millis,
        game_id,
        profile_id,
        mod_id,
        plan,
        prerequisite_decision,
    )?;
    if token == expected {
        Ok(())
    } else {
        Err(SandboxLifecycleAutomationError::PlanTokenInvalid)
    }
}

fn validate_uninstall_plan_token(
    token: &str,
    game_id: &GameId,
    profile_id: &ProfileId,
    mod_id: &ModId,
    summary: &InstallRecoverySummary,
    state_binding: &str,
) -> Result<(), SandboxLifecycleAutomationError> {
    let expires_at_unix_millis = parse_token_expiry(token)?;
    if now_unix_millis()? >= expires_at_unix_millis {
        return Err(SandboxLifecycleAutomationError::PlanTokenExpired);
    }
    let expected = build_uninstall_plan_token(
        expires_at_unix_millis,
        game_id,
        profile_id,
        mod_id,
        summary,
        state_binding,
    )?;
    if token == expected {
        Ok(())
    } else {
        Err(SandboxLifecycleAutomationError::PlanTokenInvalid)
    }
}

fn validate_recovery_plan_token(
    token: &str,
    game_id: &GameId,
    profile_id: &ProfileId,
    mod_id: &ModId,
    preview: &InstallRecoveryActionPreview,
    state_binding: &str,
) -> Result<(), SandboxLifecycleAutomationError> {
    let expires_at_unix_millis = parse_token_expiry(token)?;
    if now_unix_millis()? >= expires_at_unix_millis {
        return Err(SandboxLifecycleAutomationError::PlanTokenExpired);
    }
    let expected = build_recovery_plan_token(
        expires_at_unix_millis,
        game_id,
        profile_id,
        mod_id,
        preview,
        state_binding,
    )?;
    if token == expected {
        Ok(())
    } else {
        Err(SandboxLifecycleAutomationError::PlanTokenInvalid)
    }
}

fn validate_reinstall_plan_token(
    token: &str,
    game_id: &GameId,
    profile_id: &ProfileId,
    mod_id: &ModId,
    candidate_revision_id: &ModRevisionId,
    preview: &ReinstallPlanPreview,
) -> Result<(), SandboxLifecycleAutomationError> {
    let expires_at_unix_millis = parse_token_expiry(token)?;
    if now_unix_millis()? >= expires_at_unix_millis {
        return Err(SandboxLifecycleAutomationError::PlanTokenExpired);
    }
    let expected = build_reinstall_plan_token(
        expires_at_unix_millis,
        game_id,
        profile_id,
        mod_id,
        candidate_revision_id,
        preview,
    )?;
    if token == expected {
        Ok(())
    } else {
        Err(SandboxLifecycleAutomationError::PlanTokenInvalid)
    }
}

fn build_install_plan_token(
    expires_at_unix_millis: u128,
    game_id: &GameId,
    profile_id: &ProfileId,
    mod_id: &ModId,
    plan: &InstallPlan,
    prerequisite_decision: &GamePrerequisiteDecision,
) -> Result<String, SandboxLifecycleAutomationError> {
    build_lifecycle_plan_token(
        expires_at_unix_millis,
        &InstallPlanTokenFacts {
            schema_version: "hmm.lifecycle-plan/v1",
            command: INSTALL_APPLY_COMMAND,
            environment: "sandbox",
            game_id,
            profile_id,
            mod_id,
            plan,
            prerequisite_status: prerequisite_decision.status.as_str(),
            prerequisite_rules_version: prerequisite_decision.rules_version,
            prerequisite_codes: prerequisite_decision
                .codes
                .iter()
                .map(|code| code.as_str())
                .collect(),
        },
    )
}

fn build_uninstall_plan_token(
    expires_at_unix_millis: u128,
    game_id: &GameId,
    profile_id: &ProfileId,
    mod_id: &ModId,
    summary: &InstallRecoverySummary,
    state_binding: &str,
) -> Result<String, SandboxLifecycleAutomationError> {
    build_lifecycle_plan_token(
        expires_at_unix_millis,
        &UninstallPlanTokenFacts {
            schema_version: "hmm.lifecycle-plan/v1",
            command: INSTALL_UNINSTALL_COMMAND,
            environment: "sandbox",
            game_id,
            profile_id,
            mod_id,
            status: recovery_status_token_code(summary.status),
            managed_file_count: summary.managed_file_count,
            backup_count: summary.backup_count,
            issue_count: summary.issue_count,
            state_binding,
        },
    )
}

fn build_recovery_plan_token(
    expires_at_unix_millis: u128,
    game_id: &GameId,
    profile_id: &ProfileId,
    mod_id: &ModId,
    preview: &InstallRecoveryActionPreview,
    state_binding: &str,
) -> Result<String, SandboxLifecycleAutomationError> {
    build_lifecycle_plan_token(
        expires_at_unix_millis,
        &RecoveryPlanTokenFacts {
            schema_version: "hmm.lifecycle-plan/v1",
            command: INSTALL_RECOVERY_APPLY_COMMAND,
            environment: "sandbox",
            game_id,
            profile_id,
            mod_id,
            action: recovery_action_token_code(preview.action_kind),
            availability: recovery_availability_token_code(preview.availability),
            remove_file_count: preview.remove_file_count,
            restore_file_count: preview.restore_file_count,
            backup_count: preview.backup_count,
            blocking_issue_count: preview.blocking_issue_count,
            blocking_reasons: preview
                .blocking_reasons
                .iter()
                .map(|reason| RecoveryBlockReasonTokenFacts {
                    code: recovery_block_reason_token_code(reason.reason),
                    count: reason.count,
                })
                .collect(),
            state_binding,
        },
    )
}

fn build_reinstall_plan_token(
    expires_at_unix_millis: u128,
    game_id: &GameId,
    profile_id: &ProfileId,
    mod_id: &ModId,
    candidate_revision_id: &ModRevisionId,
    preview: &ReinstallPlanPreview,
) -> Result<String, SandboxLifecycleAutomationError> {
    build_lifecycle_plan_token(
        expires_at_unix_millis,
        &ReinstallPlanTokenFacts {
            schema_version: "hmm.lifecycle-plan/v1",
            command: INSTALL_REINSTALL_COMMAND,
            environment: "sandbox",
            game_id,
            profile_id,
            mod_id,
            candidate_revision_id,
            status: reinstall_status_token_code(preview.status),
            installed_revision_id: preview
                .installed_revision
                .as_ref()
                .map(|revision| &revision.revision_id),
            preview_candidate_revision_id: preview
                .candidate_revision
                .as_ref()
                .map(|revision| &revision.revision_id),
            retained_count: preview.counts.retained,
            replaced_count: preview.counts.replaced,
            added_count: preview.counts.added,
            stale_count: preview.counts.stale,
            blocking_reasons: preview
                .blocking_reasons
                .iter()
                .map(|reason| ReinstallBlockReasonTokenFacts {
                    code: reinstall_block_reason_token_code(reason.reason),
                    count: reason.count,
                })
                .collect(),
            internal_plan_token: preview.plan_token.as_deref(),
        },
    )
}

fn build_lifecycle_plan_token(
    expires_at_unix_millis: u128,
    facts: &impl Serialize,
) -> Result<String, SandboxLifecycleAutomationError> {
    let expiry = u64::try_from(expires_at_unix_millis)
        .map_err(|_| SandboxLifecycleAutomationError::PlanTokenInvalid)?;
    let facts =
        serde_json::to_vec(facts).map_err(|_| SandboxLifecycleAutomationError::PlanTokenInvalid)?;
    let mut hasher = Sha256::new();
    hasher.update(b"hmm.lifecycle-plan/v1");
    hasher.update(expiry.to_be_bytes());
    hasher.update((facts.len() as u64).to_be_bytes());
    hasher.update(facts);
    let digest = hasher.finalize();

    Ok(format!(
        "{LIFECYCLE_PLAN_TOKEN_PREFIX}{}{}",
        encode_hex(&expiry.to_be_bytes()),
        encode_hex(&digest)
    ))
}

pub(crate) fn lifecycle_state_binding(
    state: &impl Serialize,
) -> Result<String, SandboxLifecycleAutomationError> {
    let state =
        serde_json::to_vec(state).map_err(|_| SandboxLifecycleAutomationError::PlanTokenInvalid)?;
    let mut hasher = Sha256::new();
    hasher.update(b"hmm.lifecycle-state-binding/v1");
    hasher.update((state.len() as u64).to_be_bytes());
    hasher.update(state);
    Ok(encode_hex(&hasher.finalize()))
}

fn recovery_status_token_code(status: InstallRecoveryStatus) -> &'static str {
    match status {
        InstallRecoveryStatus::NotInstalled => "not_installed",
        InstallRecoveryStatus::Completed => "completed",
        InstallRecoveryStatus::CommittedCleanupPending => "committed_cleanup_pending",
        InstallRecoveryStatus::CleanupPending => "cleanup_pending",
        InstallRecoveryStatus::RollbackRequired => "rollback_required",
        InstallRecoveryStatus::RepairRequired => "repair_required",
        InstallRecoveryStatus::Unknown => "unknown",
    }
}

fn recovery_action_token_code(action: InstallRecoveryActionKind) -> &'static str {
    match action {
        InstallRecoveryActionKind::RollbackInstall => "rollback_install",
        InstallRecoveryActionKind::ReconcileReinstall => "reconcile_reinstall",
    }
}

fn recovery_availability_token_code(
    availability: InstallRecoveryActionAvailability,
) -> &'static str {
    match availability {
        InstallRecoveryActionAvailability::Available => "available",
        InstallRecoveryActionAvailability::Blocked => "blocked",
    }
}

fn recovery_block_reason_token_code(reason: InstallRecoveryActionBlockReason) -> &'static str {
    match reason {
        InstallRecoveryActionBlockReason::RollbackStateMissing => "rollback_state_missing",
        InstallRecoveryActionBlockReason::MissingInstalledFileSummary => {
            "missing_installed_file_summary"
        }
        InstallRecoveryActionBlockReason::TargetMissing => "target_missing",
        InstallRecoveryActionBlockReason::TargetChanged => "target_changed",
        InstallRecoveryActionBlockReason::TargetReadFailed => "target_read_failed",
        InstallRecoveryActionBlockReason::BackupMissing => "backup_missing",
        InstallRecoveryActionBlockReason::BackupReadFailed => "backup_read_failed",
    }
}

fn reinstall_status_token_code(status: ReinstallPreviewStatus) -> &'static str {
    match status {
        ReinstallPreviewStatus::Ready => "ready",
        ReinstallPreviewStatus::Blocked => "blocked",
    }
}

fn reinstall_block_reason_token_code(reason: ReinstallBlockingReason) -> &'static str {
    match reason {
        ReinstallBlockingReason::PrerequisitesBlocked => "prerequisites_blocked",
        ReinstallBlockingReason::NotInstalled => "not_installed",
        ReinstallBlockingReason::CandidateNotFound => "candidate_not_found",
        ReinstallBlockingReason::CandidateNotReady => "candidate_not_ready",
        ReinstallBlockingReason::CandidateOwnerMismatch => "candidate_owner_mismatch",
        ReinstallBlockingReason::CandidateAlreadyInstalled => "candidate_already_installed",
        ReinstallBlockingReason::ManifestStateUnsafe => "manifest_state_unsafe",
        ReinstallBlockingReason::InstalledRevisionUnknown => "installed_revision_unknown",
        ReinstallBlockingReason::SourceUnavailable => "source_unavailable",
        ReinstallBlockingReason::TargetMissing => "target_missing",
        ReinstallBlockingReason::TargetChanged => "target_changed",
        ReinstallBlockingReason::TargetReadFailed => "target_read_failed",
        ReinstallBlockingReason::BackupMissing => "backup_missing",
        ReinstallBlockingReason::BackupReadFailed => "backup_read_failed",
        ReinstallBlockingReason::PlanConflict => "plan_conflict",
        ReinstallBlockingReason::CrossModTargetConflict => "cross_mod_target_conflict",
    }
}

fn parse_token_expiry(token: &str) -> Result<u128, SandboxLifecycleAutomationError> {
    let payload = token
        .strip_prefix(LIFECYCLE_PLAN_TOKEN_PREFIX)
        .ok_or(SandboxLifecycleAutomationError::PlanTokenInvalid)?;
    if payload.len() != LIFECYCLE_PLAN_TOKEN_PAYLOAD_HEX_LENGTH
        || !payload.bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        return Err(SandboxLifecycleAutomationError::PlanTokenInvalid);
    }
    let expiry_bytes = decode_hex_8(&payload[..16])?;
    Ok(u64::from_be_bytes(expiry_bytes) as u128)
}

fn encode_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn decode_hex_8(value: &str) -> Result<[u8; 8], SandboxLifecycleAutomationError> {
    let mut bytes = [0_u8; 8];
    for (index, slot) in bytes.iter_mut().enumerate() {
        let offset = index * 2;
        *slot = u8::from_str_radix(&value[offset..offset + 2], 16)
            .map_err(|_| SandboxLifecycleAutomationError::PlanTokenInvalid)?;
    }
    Ok(bytes)
}

fn now_unix_millis() -> Result<u128, SandboxLifecycleAutomationError> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .map_err(|_| SandboxLifecycleAutomationError::RuntimeUnavailable)
}

#[cfg(test)]
mod tests {
    use super::*;
    use hmm_core::{
        InstallRecoveryRecord, InstallRecoveryRecordEntry, InstallRecoveryRecordStatus,
        InstallTargetPath, InstalledFileSummary, PackageFileId,
    };
    use hmm_infra::{JsonInstallManifestRepository, JsonInstallRecoveryRecordRepository};
    use hmm_ports::{InstallManifestRepository, InstallRecoveryRecordRepository};
    use std::fs;
    use std::path::Path;

    #[test]
    fn sandbox_install_and_uninstall_tokens_drive_real_runners_and_preserve_external_sentinel() {
        let sandbox = tempfile::tempdir().expect("sandbox");
        let outside = tempfile::tempdir().expect("outside");
        let sentinel = outside.path().join("sentinel.bin");
        fs::write(&sentinel, b"outside").expect("sentinel");
        let game_root = write_install_fixture(sandbox.path());
        let environment =
            RuntimeEnvironment::sandbox(sandbox.path().to_path_buf()).expect("environment");
        let preview = ReadOnlyInstallAutomation::from_environment(&environment)
            .expect("read-only automation")
            .plan_for_profile("mhw", "default", "mod-a")
            .expect("install preview");
        let token = preview.plan_token.expect("sandbox plan token");

        let automation = SandboxLifecycleAutomation::prepare_install(
            &environment,
            "mhw",
            "default",
            "mod-a",
            &token,
        )
        .expect("prepare sandbox install");
        let outcome = automation.run_install().expect("install succeeds");

        assert!(outcome.task_id.starts_with("install-"));
        assert_eq!(
            outcome
                .events
                .iter()
                .map(|event| event.phase.as_str())
                .collect::<Vec<_>>(),
            [
                "install.plan.building",
                "install.commit.processing",
                "install.completed",
            ]
        );
        assert_eq!(
            fs::read(game_root.join("nativePC/models/player.mod3")).expect("installed target"),
            b"fixture"
        );
        assert!(sandbox
            .path()
            .join("install/manifests/default.json")
            .exists());

        let uninstall_preview = ReadOnlyInstallAutomation::from_environment(&environment)
            .expect("restarted read-only automation")
            .uninstall_preview("mhw", "default", "mod-a")
            .expect("uninstall preview");
        assert!(uninstall_preview.available);
        assert_eq!(uninstall_preview.status, "installed");
        let uninstall_token = uninstall_preview.plan_token.expect("uninstall plan token");
        let uninstall = SandboxLifecycleAutomation::prepare_uninstall(
            &environment,
            "mhw",
            "default",
            "mod-a",
            &uninstall_token,
        )
        .expect("prepare sandbox uninstall");
        let uninstall_outcome = uninstall.run_uninstall().expect("uninstall succeeds");

        assert!(uninstall_outcome.task_id.starts_with("install-"));
        assert_eq!(
            uninstall_outcome
                .events
                .iter()
                .map(|event| event.phase.as_str())
                .collect::<Vec<_>>(),
            [
                "install.uninstall.processing",
                "install.uninstall.completed",
            ]
        );
        assert!(!game_root.join("nativePC/models/player.mod3").exists());
        let status = ReadOnlyInstallAutomation::from_environment(&environment)
            .expect("post-uninstall read-only automation")
            .status(Some("mhw"), "default", &["mod-a".to_owned()])
            .expect("post-uninstall status");
        assert_eq!(status.items[0].status, "not_installed");
        assert_eq!(fs::read(&sentinel).expect("sentinel remains"), b"outside");
    }

    #[test]
    fn install_rejects_package_plan_drift_before_any_game_write() {
        let sandbox = tempfile::tempdir().expect("sandbox");
        let game_root = write_install_fixture(sandbox.path());
        let environment =
            RuntimeEnvironment::sandbox(sandbox.path().to_path_buf()).expect("environment");
        let preview = ReadOnlyInstallAutomation::from_environment(&environment)
            .expect("read-only automation")
            .plan_for_profile("mhw", "default", "mod-a")
            .expect("install preview");
        let token = preview.plan_token.expect("sandbox plan token");
        let automation = SandboxLifecycleAutomation::prepare_install(
            &environment,
            "mhw",
            "default",
            "mod-a",
            &token,
        )
        .expect("prepare sandbox install");

        fs::write(
            sandbox
                .path()
                .join("mod-import/sandboxes/package-a/nativePC/models/extra.mod3"),
            b"drift",
        )
        .expect("mutate package after confirmation");

        let error = automation
            .run_install()
            .expect_err("changed plan must be rejected");

        assert_eq!(error.code(), "install_task_failed");
        assert!(error.task_id().is_some());
        assert!(!game_root.join("nativePC/models/player.mod3").exists());
        assert!(!game_root.join("nativePC/models/extra.mod3").exists());
    }

    #[test]
    fn uninstall_rejects_manifest_drift_after_prepare_before_any_game_write() {
        let sandbox = tempfile::tempdir().expect("sandbox");
        let game_root = write_install_fixture(sandbox.path());
        let environment =
            RuntimeEnvironment::sandbox(sandbox.path().to_path_buf()).expect("environment");
        let install_preview = ReadOnlyInstallAutomation::from_environment(&environment)
            .expect("read-only automation")
            .plan_for_profile("mhw", "default", "mod-a")
            .expect("install preview");
        SandboxLifecycleAutomation::prepare_install(
            &environment,
            "mhw",
            "default",
            "mod-a",
            install_preview
                .plan_token
                .as_deref()
                .expect("install token"),
        )
        .expect("prepare install")
        .run_install()
        .expect("install fixture");

        let uninstall_preview = ReadOnlyInstallAutomation::from_environment(&environment)
            .expect("read-only automation")
            .uninstall_preview("mhw", "default", "mod-a")
            .expect("uninstall preview");
        let automation = SandboxLifecycleAutomation::prepare_uninstall(
            &environment,
            "mhw",
            "default",
            "mod-a",
            uninstall_preview
                .plan_token
                .as_deref()
                .expect("uninstall token"),
        )
        .expect("prepare uninstall");

        let original_target = game_root.join("nativePC/models/player.mod3");
        let unconfirmed_target = game_root.join("nativePC/models/unconfirmed.mod3");
        fs::write(&unconfirmed_target, b"fixture").expect("write unconfirmed target");
        let repository =
            JsonInstallManifestRepository::new(sandbox.path().join("install/manifests"));
        let mut changed_manifest = repository
            .load_manifest(&ProfileId::new("default"))
            .expect("load manifest")
            .expect("installed manifest");
        changed_manifest.entries[0].target_path =
            InstallTargetPath::parse("nativePC/models/unconfirmed.mod3", ["nativePC"])
                .expect("unconfirmed target path");
        changed_manifest.entries[0].package_file_id =
            PackageFileId::new("nativePC/models/unconfirmed.mod3");
        repository
            .save_manifest(&changed_manifest)
            .expect("save changed manifest");

        let error = automation
            .run_uninstall()
            .expect_err("changed manifest must be rejected");

        assert_eq!(error.code(), "install_uninstall_task_failed");
        assert!(fs::read(&original_target).is_ok());
        assert_eq!(
            fs::read(&unconfirmed_target).expect("unconfirmed target remains"),
            b"fixture"
        );
        assert_eq!(
            repository
                .load_manifest(&ProfileId::new("default"))
                .expect("reload manifest"),
            Some(changed_manifest)
        );
    }

    #[test]
    fn recovery_rejects_record_drift_after_prepare_before_any_game_write() {
        let sandbox = tempfile::tempdir().expect("sandbox");
        let game_root = write_install_fixture(sandbox.path());
        let environment =
            RuntimeEnvironment::sandbox(sandbox.path().to_path_buf()).expect("environment");
        let original_target = write_rollback_recovery_fixture(sandbox.path(), &game_root);
        let preview = ReadOnlyInstallAutomation::from_environment(&environment)
            .expect("read-only automation")
            .recovery_preview(
                "mhw",
                "default",
                "mod-recovery",
                ReadOnlyInstallRecoveryAction::RollbackInstall,
            )
            .expect("recovery preview");
        let automation = SandboxLifecycleAutomation::prepare_recovery(
            &environment,
            "mhw",
            "default",
            "mod-recovery",
            ReadOnlyInstallRecoveryAction::RollbackInstall,
            preview.plan_token.as_deref().expect("recovery token"),
        )
        .expect("prepare recovery");

        let unconfirmed_target = game_root.join("nativePC/models/unconfirmed-recovery.mod3");
        fs::write(&unconfirmed_target, b"recovery-fixture").expect("write unconfirmed target");
        let repository =
            JsonInstallRecoveryRecordRepository::new(sandbox.path().join("install/recovery"));
        let mut changed_record = repository
            .load_record(&ProfileId::new("default"), &ModId::new("mod-recovery"))
            .expect("load recovery record")
            .expect("recovery record");
        changed_record.entries[0].target_path =
            InstallTargetPath::parse("nativePC/models/unconfirmed-recovery.mod3", ["nativePC"])
                .expect("unconfirmed recovery target path");
        changed_record.entries[0].package_file_id =
            PackageFileId::new("nativePC/models/unconfirmed-recovery.mod3");
        repository
            .save_record(&changed_record)
            .expect("save changed recovery record");

        let error = automation
            .run_recovery()
            .expect_err("changed recovery state must be rejected");

        assert_eq!(error.code(), "install_recovery_task_failed");
        assert_eq!(
            fs::read(&original_target).expect("original recovery target remains"),
            b"recovery-fixture"
        );
        assert_eq!(
            fs::read(&unconfirmed_target).expect("unconfirmed recovery target remains"),
            b"recovery-fixture"
        );
        assert_eq!(
            repository
                .load_record(&ProfileId::new("default"), &ModId::new("mod-recovery"))
                .expect("reload recovery record"),
            Some(changed_record)
        );
    }

    #[test]
    fn install_plan_token_rejects_wrong_profile_expiry_and_malformed_payload() {
        let sandbox = tempfile::tempdir().expect("sandbox");
        write_install_fixture(sandbox.path());
        let environment =
            RuntimeEnvironment::sandbox(sandbox.path().to_path_buf()).expect("environment");
        let read_only =
            ReadOnlyInstallAutomation::from_environment(&environment).expect("automation");
        let (game_id, profile_id, mod_id, plan, prerequisite_decision) = read_only
            .build_install_plan("mhw", "default", "mod-a")
            .expect("plan facts");
        let issued = issue_install_plan_token(
            &game_id,
            &profile_id,
            &mod_id,
            &plan,
            &prerequisite_decision,
        )
        .expect("token");

        assert_eq!(
            validate_install_plan_token(
                &issued.token,
                &game_id,
                &ProfileId::new("other"),
                &mod_id,
                &plan,
                &prerequisite_decision,
            ),
            Err(SandboxLifecycleAutomationError::PlanTokenInvalid)
        );
        let expired = build_install_plan_token(
            1,
            &game_id,
            &profile_id,
            &mod_id,
            &plan,
            &prerequisite_decision,
        )
        .expect("expired");
        assert_eq!(
            validate_install_plan_token(
                &expired,
                &game_id,
                &profile_id,
                &mod_id,
                &plan,
                &prerequisite_decision,
            ),
            Err(SandboxLifecycleAutomationError::PlanTokenExpired)
        );
        assert_eq!(
            validate_install_plan_token(
                "not-a-token",
                &game_id,
                &profile_id,
                &mod_id,
                &plan,
                &prerequisite_decision,
            ),
            Err(SandboxLifecycleAutomationError::PlanTokenInvalid)
        );
        let mut changed_prerequisite_decision = prerequisite_decision.clone();
        changed_prerequisite_decision.rules_version = Some(
            prerequisite_decision
                .rules_version
                .unwrap_or_default()
                .saturating_add(1),
        );
        assert_eq!(
            validate_install_plan_token(
                &issued.token,
                &game_id,
                &profile_id,
                &mod_id,
                &plan,
                &changed_prerequisite_decision,
            ),
            Err(SandboxLifecycleAutomationError::PlanTokenInvalid)
        );
        let serialized = serde_json::to_string(&issued.token).expect("serialize token");
        assert!(!serialized.contains(&sandbox.path().to_string_lossy().to_string()));
        assert!(!serialized.contains("default"));
        assert!(!serialized.contains("mod-a"));
    }

    #[test]
    fn production_cannot_prepare_install_even_with_a_well_formed_token() {
        let environment =
            RuntimeEnvironment::from_options(RuntimeEnvironmentKind::Production, None)
                .expect("production");
        let result = SandboxLifecycleAutomation::prepare_install(
            &environment,
            "mhw",
            "default",
            "mod-a",
            &format!("{LIFECYCLE_PLAN_TOKEN_PREFIX}{}", "0".repeat(80)),
        );
        let error = match result {
            Ok(_) => panic!("production write is unreachable"),
            Err(error) => error,
        };
        assert_eq!(error, SandboxLifecycleAutomationError::ProductionForbidden);

        let result = SandboxLifecycleAutomation::prepare_uninstall(
            &environment,
            "mhw",
            "default",
            "mod-a",
            &format!("{LIFECYCLE_PLAN_TOKEN_PREFIX}{}", "0".repeat(80)),
        );
        let error = match result {
            Ok(_) => panic!("production uninstall is unreachable"),
            Err(error) => error,
        };
        assert_eq!(error, SandboxLifecycleAutomationError::ProductionForbidden);

        let result = SandboxLifecycleAutomation::prepare_reinstall(
            &environment,
            "mhw",
            "default",
            "mod-a",
            "revision-v2",
            &format!("{LIFECYCLE_PLAN_TOKEN_PREFIX}{}", "0".repeat(80)),
        );
        let error = match result {
            Ok(_) => panic!("production reinstall is unreachable"),
            Err(error) => error,
        };
        assert_eq!(error, SandboxLifecycleAutomationError::ProductionForbidden);

        let result = SandboxLifecycleAutomation::prepare_recovery(
            &environment,
            "mhw",
            "default",
            "mod-a",
            ReadOnlyInstallRecoveryAction::RollbackInstall,
            &format!("{LIFECYCLE_PLAN_TOKEN_PREFIX}{}", "0".repeat(80)),
        );
        let error = match result {
            Ok(_) => panic!("production recovery is unreachable"),
            Err(error) => error,
        };
        assert_eq!(error, SandboxLifecycleAutomationError::ProductionForbidden);
    }

    fn write_rollback_recovery_fixture(sandbox: &Path, game_root: &Path) -> PathBuf {
        const CONTENT: &[u8] = b"recovery-fixture";
        const SHA256: &str = "f1889dda90864358c71d55bdf593bf568d7bde025635c248182721d319a2aeaf";

        let relative_target = "nativePC/models/recovery.mod3";
        let target = game_root.join(relative_target);
        fs::write(&target, CONTENT).expect("write recovery target");
        JsonInstallRecoveryRecordRepository::new(sandbox.join("install/recovery"))
            .save_record(&InstallRecoveryRecord {
                profile_id: ProfileId::new("default"),
                mod_id: ModId::new("mod-recovery"),
                status: InstallRecoveryRecordStatus::RollbackRequired,
                entries: vec![InstallRecoveryRecordEntry {
                    target_path: InstallTargetPath::parse(relative_target, ["nativePC"])
                        .expect("recovery target path"),
                    package_file_id: PackageFileId::new(relative_target),
                    backup_ref: None,
                    installed_file: Some(InstalledFileSummary {
                        size_bytes: CONTENT.len() as u64,
                        sha256: SHA256.to_owned(),
                    }),
                }],
            })
            .expect("write recovery record");
        target
    }
}
