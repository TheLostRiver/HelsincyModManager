use crate::{
    HmmRuntime, ReadOnlyInstallAutomation, ReadOnlyInstallRecoveryAction, RuntimeEnvironment,
    RuntimeEnvironmentKind, SandboxWriteCapability, SandboxWriteRoots,
};
use hmm_app::{
    InstallRecoveryActionAvailability, InstallRecoveryActionBlockReason, InstallRecoveryActionKind,
    InstallRecoveryActionPreview, InstallRecoveryStatus, InstallRecoverySummary,
    InstallWriteAdmission, InstallWriteAdmissionError, ReinstallBlockingReason,
    ReinstallPlanPreview, ReinstallPreviewStatus, StartInstallTaskRequest,
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
        let (game_id, profile_id, mod_id, plan) = read_only
            .build_install_plan(game_id, profile_id, mod_id)
            .map_err(|_| SandboxLifecycleAutomationError::PlanUnavailable)?;
        if plan.has_blocking_conflicts() {
            return Err(SandboxLifecycleAutomationError::PlanBlocked);
        }
        validate_install_plan_token(plan_token, &game_id, &profile_id, &mod_id, &plan)?;

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
                read_only,
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
                read_only,
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
        let write_admission: Arc<dyn InstallWriteAdmission> =
            Arc::new(SandboxRecoveryWriteAdmission {
                capability,
                sandbox_root: sandbox_root.clone(),
                game_config_repository,
                read_only,
                expected_game_id: game_id.clone(),
                expected_profile_id: profile_id.clone(),
                expected_mod_id: mod_id.clone(),
                expected_action: action,
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
                action_kind: preview.action_kind,
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
                read_only,
                expected_game_id: game_id.clone(),
                expected_profile_id: profile_id.clone(),
                expected_mod_id: mod_id.clone(),
                expected_candidate_revision_id: candidate_revision_id.clone(),
                expected_internal_plan_token: internal_plan_token.clone(),
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
    read_only: Arc<ReadOnlyInstallAutomation>,
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
        if game_id != &self.expected_game_id || profile_id != &self.expected_profile_id {
            return Err(InstallWriteAdmissionError::SafetyRejected);
        }
        let game_instance = self
            .game_config_repository
            .load_game_instance(game_id)
            .map_err(|_| InstallWriteAdmissionError::SafetyRejected)?
            .ok_or(InstallWriteAdmissionError::SafetyRejected)?;
        let admission = self
            .capability
            .admit_roots(SandboxWriteRoots::new(
                self.sandbox_root.clone(),
                game_instance.root_dir,
            ))
            .map_err(|_| InstallWriteAdmissionError::SafetyRejected)?;
        let (_, _, _, plan) = self
            .read_only
            .build_install_plan(
                self.expected_game_id.as_str(),
                self.expected_profile_id.as_str(),
                self.expected_mod_id.as_str(),
            )
            .map_err(|_| InstallWriteAdmissionError::SafetyRejected)?;
        validate_install_plan_token(
            &self.plan_token,
            &self.expected_game_id,
            &self.expected_profile_id,
            &self.expected_mod_id,
            &plan,
        )
        .map_err(|_| InstallWriteAdmissionError::SafetyRejected)?;
        admission
            .revalidate()
            .map_err(|_| InstallWriteAdmissionError::SafetyRejected)
    }
}

struct SandboxUninstallWriteAdmission {
    capability: Arc<SandboxWriteCapability>,
    sandbox_root: PathBuf,
    game_config_repository: Arc<dyn GameConfigRepository>,
    read_only: Arc<ReadOnlyInstallAutomation>,
    expected_game_id: GameId,
    expected_profile_id: ProfileId,
    expected_mod_id: ModId,
    plan_token: String,
}

impl InstallWriteAdmission for SandboxUninstallWriteAdmission {
    fn ensure_write_allowed(
        &self,
        game_id: &GameId,
        profile_id: &ProfileId,
    ) -> Result<(), InstallWriteAdmissionError> {
        if game_id != &self.expected_game_id || profile_id != &self.expected_profile_id {
            return Err(InstallWriteAdmissionError::SafetyRejected);
        }
        let game_instance = self
            .game_config_repository
            .load_game_instance(game_id)
            .map_err(|_| InstallWriteAdmissionError::SafetyRejected)?
            .ok_or(InstallWriteAdmissionError::SafetyRejected)?;
        let admission = self
            .capability
            .admit_roots(SandboxWriteRoots::new(
                self.sandbox_root.clone(),
                game_instance.root_dir,
            ))
            .map_err(|_| InstallWriteAdmissionError::SafetyRejected)?;
        let (_, _, _, summary, state_binding) = self
            .read_only
            .build_uninstall_facts(
                self.expected_game_id.as_str(),
                self.expected_profile_id.as_str(),
                self.expected_mod_id.as_str(),
            )
            .map_err(|_| InstallWriteAdmissionError::SafetyRejected)?;
        if summary.status != InstallRecoveryStatus::Completed {
            return Err(InstallWriteAdmissionError::SafetyRejected);
        }
        validate_uninstall_plan_token(
            &self.plan_token,
            &self.expected_game_id,
            &self.expected_profile_id,
            &self.expected_mod_id,
            &summary,
            &state_binding,
        )
        .map_err(|_| InstallWriteAdmissionError::SafetyRejected)?;
        admission
            .revalidate()
            .map_err(|_| InstallWriteAdmissionError::SafetyRejected)
    }
}

struct SandboxReinstallWriteAdmission {
    capability: Arc<SandboxWriteCapability>,
    sandbox_root: PathBuf,
    game_config_repository: Arc<dyn GameConfigRepository>,
    read_only: Arc<ReadOnlyInstallAutomation>,
    expected_game_id: GameId,
    expected_profile_id: ProfileId,
    expected_mod_id: ModId,
    expected_candidate_revision_id: ModRevisionId,
    expected_internal_plan_token: String,
    plan_token: String,
}

impl InstallWriteAdmission for SandboxReinstallWriteAdmission {
    fn ensure_write_allowed(
        &self,
        game_id: &GameId,
        profile_id: &ProfileId,
    ) -> Result<(), InstallWriteAdmissionError> {
        if game_id != &self.expected_game_id || profile_id != &self.expected_profile_id {
            return Err(InstallWriteAdmissionError::SafetyRejected);
        }
        let game_instance = self
            .game_config_repository
            .load_game_instance(game_id)
            .map_err(|_| InstallWriteAdmissionError::SafetyRejected)?
            .ok_or(InstallWriteAdmissionError::SafetyRejected)?;
        let admission = self
            .capability
            .admit_roots(SandboxWriteRoots::new(
                self.sandbox_root.clone(),
                game_instance.root_dir,
            ))
            .map_err(|_| InstallWriteAdmissionError::SafetyRejected)?;
        let (_, _, _, _, preview) = self
            .read_only
            .build_reinstall_facts(
                self.expected_game_id.as_str(),
                self.expected_profile_id.as_str(),
                self.expected_mod_id.as_str(),
                self.expected_candidate_revision_id.as_str(),
            )
            .map_err(|_| InstallWriteAdmissionError::SafetyRejected)?;
        if preview.status != ReinstallPreviewStatus::Ready
            || preview.plan_token.as_deref() != Some(self.expected_internal_plan_token.as_str())
        {
            return Err(InstallWriteAdmissionError::SafetyRejected);
        }
        validate_reinstall_plan_token(
            &self.plan_token,
            &self.expected_game_id,
            &self.expected_profile_id,
            &self.expected_mod_id,
            &self.expected_candidate_revision_id,
            &preview,
        )
        .map_err(|_| InstallWriteAdmissionError::SafetyRejected)?;
        admission
            .revalidate()
            .map_err(|_| InstallWriteAdmissionError::SafetyRejected)
    }
}

struct SandboxRecoveryWriteAdmission {
    capability: Arc<SandboxWriteCapability>,
    sandbox_root: PathBuf,
    game_config_repository: Arc<dyn GameConfigRepository>,
    read_only: Arc<ReadOnlyInstallAutomation>,
    expected_game_id: GameId,
    expected_profile_id: ProfileId,
    expected_mod_id: ModId,
    expected_action: ReadOnlyInstallRecoveryAction,
    plan_token: String,
}

impl InstallWriteAdmission for SandboxRecoveryWriteAdmission {
    fn ensure_write_allowed(
        &self,
        game_id: &GameId,
        profile_id: &ProfileId,
    ) -> Result<(), InstallWriteAdmissionError> {
        if game_id != &self.expected_game_id || profile_id != &self.expected_profile_id {
            return Err(InstallWriteAdmissionError::SafetyRejected);
        }
        let game_instance = self
            .game_config_repository
            .load_game_instance(game_id)
            .map_err(|_| InstallWriteAdmissionError::SafetyRejected)?
            .ok_or(InstallWriteAdmissionError::SafetyRejected)?;
        let admission = self
            .capability
            .admit_roots(SandboxWriteRoots::new(
                self.sandbox_root.clone(),
                game_instance.root_dir,
            ))
            .map_err(|_| InstallWriteAdmissionError::SafetyRejected)?;
        let (_, _, _, preview, state_binding) = self
            .read_only
            .build_recovery_preview_facts(
                self.expected_game_id.as_str(),
                self.expected_profile_id.as_str(),
                self.expected_mod_id.as_str(),
                self.expected_action,
            )
            .map_err(|_| InstallWriteAdmissionError::SafetyRejected)?;
        if preview.availability != InstallRecoveryActionAvailability::Available {
            return Err(InstallWriteAdmissionError::SafetyRejected);
        }
        validate_recovery_plan_token(
            &self.plan_token,
            &self.expected_game_id,
            &self.expected_profile_id,
            &self.expected_mod_id,
            &preview,
            &state_binding,
        )
        .map_err(|_| InstallWriteAdmissionError::SafetyRejected)?;
        admission
            .revalidate()
            .map_err(|_| InstallWriteAdmissionError::SafetyRejected)
    }
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
) -> Result<IssuedLifecyclePlanToken, SandboxLifecycleAutomationError> {
    let now = now_unix_millis()?;
    let expires_at_unix_millis = now
        .checked_add(LIFECYCLE_PLAN_TOKEN_TTL_MILLIS)
        .ok_or(SandboxLifecycleAutomationError::PlanTokenInvalid)?;
    let token =
        build_install_plan_token(expires_at_unix_millis, game_id, profile_id, mod_id, plan)?;
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
) -> Result<(), SandboxLifecycleAutomationError> {
    let expires_at_unix_millis = parse_token_expiry(token)?;
    if now_unix_millis()? >= expires_at_unix_millis {
        return Err(SandboxLifecycleAutomationError::PlanTokenExpired);
    }
    let expected =
        build_install_plan_token(expires_at_unix_millis, game_id, profile_id, mod_id, plan)?;
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
    use crate::{SANDBOX_MARKER_FILE_NAME, SANDBOX_MARKER_SCHEMA};
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
    fn install_plan_token_rejects_wrong_profile_expiry_and_malformed_payload() {
        let sandbox = tempfile::tempdir().expect("sandbox");
        write_install_fixture(sandbox.path());
        let environment =
            RuntimeEnvironment::sandbox(sandbox.path().to_path_buf()).expect("environment");
        let read_only =
            ReadOnlyInstallAutomation::from_environment(&environment).expect("automation");
        let (game_id, profile_id, mod_id, plan) = read_only
            .build_install_plan("mhw", "default", "mod-a")
            .expect("plan facts");
        let issued =
            issue_install_plan_token(&game_id, &profile_id, &mod_id, &plan).expect("token");

        assert_eq!(
            validate_install_plan_token(
                &issued.token,
                &game_id,
                &ProfileId::new("other"),
                &mod_id,
                &plan,
            ),
            Err(SandboxLifecycleAutomationError::PlanTokenInvalid)
        );
        let expired =
            build_install_plan_token(1, &game_id, &profile_id, &mod_id, &plan).expect("expired");
        assert_eq!(
            validate_install_plan_token(&expired, &game_id, &profile_id, &mod_id, &plan),
            Err(SandboxLifecycleAutomationError::PlanTokenExpired)
        );
        assert_eq!(
            validate_install_plan_token("not-a-token", &game_id, &profile_id, &mod_id, &plan),
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

    fn write_install_fixture(sandbox: &Path) -> PathBuf {
        fs::write(
            sandbox.join(SANDBOX_MARKER_FILE_NAME),
            SANDBOX_MARKER_SCHEMA,
        )
        .expect("sandbox marker");
        let game_root = sandbox.join("fixtures/games/mhw-minimal");
        fs::create_dir_all(game_root.join("nativePC/models")).expect("game fixture");
        fs::write(game_root.join("MonsterHunterWorld.exe"), b"fixture").expect("game executable");
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
}
