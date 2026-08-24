use crate::{
    lifecycle_automation::revalidate_sandbox_write_roots, HmmRuntime, ReadOnlyInstallAutomation,
    RuntimeEnvironment, SandboxWriteCapability,
};
use hmm_app::{
    BatchInstallItemExecutor, BatchInstallRetryError, BatchInstallRetryResult,
    BatchInstallRetryService, BatchInstallRunError, BatchInstallRunResult, BatchInstallTaskRunner,
    BatchPlanPreview, BatchPlanPreviewError, BatchPlanSealError, BatchPlanSealResult,
    BatchPlanService, BatchTokenCodec, InstallTaskBatchItemExecutor, InstallWriteAdmission,
    ReinstallTaskBatchItemExecutor, Sha256BatchTokenCodec, UninstallTaskBatchItemExecutor,
};
use hmm_core::{
    BatchActionSummary, BatchAttempt, BatchAttemptStatus, BatchId, BatchItemFacts, BatchItemInput,
    BatchItemResult, BatchOperation, BatchPlanFacts, BatchPlanRequest, BatchPreflightDecision,
    BatchPreflightStatus, BatchTargetClaim, BatchTargetWriteKind, InstallPlan, ModId,
    NormalizedBatchPlanRequest, ReplacementTargetId, SealedBatch,
};
use hmm_infra::{
    open_database_read_only, JsonGameConfigRepository, SqliteBatchLifecycleRepository, SystemClock,
};
use hmm_ports::{BatchLifecycleRepository, BatchPlanFactsProvider, BatchSealRepository};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fmt;
use std::fmt::Write as _;
use std::fs;
use std::io::ErrorKind;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

// This deterministic Sandbox tag binds previews to a fixture root and detects stale input. It is
// not an authentication secret; sandbox isolation carries its safety. Production tokens are keyed
// by the per-installation random secret instead (CLI-3C, see batch_token_secret.rs), so they
// cannot be forged offline, and cross-process admission still guards every item write.
const BATCH_TOKEN_TAG_PREFIX: &str = "hmm-sandbox-batch-token-v2";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BatchAutomationErrorClass {
    DataSafetyRisk,
    UserActionRequired,
    Recoverable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BatchAutomationError {
    code: &'static str,
}

impl BatchAutomationError {
    pub const fn code(&self) -> &'static str {
        self.code
    }

    pub fn class(&self) -> BatchAutomationErrorClass {
        match self.code {
            "sandbox_batch_production_forbidden"
            | "batch_write_admission_unavailable"
            | "batch_runtime_unavailable"
            | "batch_attempt_reconciliation_required"
            | "batch_operation_mismatch"
            | "batch_admission_rejected" => BatchAutomationErrorClass::DataSafetyRisk,
            "batch_input_invalid"
            | "batch_duplicate_item"
            | "batch_resource_limit_exceeded"
            | "batch_plan_blocked"
            | "batch_token_invalid"
            | "batch_plan_stale"
            | "batch_plan_expired"
            | "batch_retry_unavailable"
            | "batch_attempt_stale"
            | "batch_id_invalid" => BatchAutomationErrorClass::UserActionRequired,
            "sandbox_data_dir_required"
            | "batch_secret_unavailable"
            | "batch_token_unavailable"
            | "batch_unavailable"
            | "batch_journal_unavailable"
            | "batch_result_unavailable"
            | "batch_task_unavailable"
            | "batch_evidence_unavailable"
            | "batch_internal_error" => BatchAutomationErrorClass::Recoverable,
            _ => BatchAutomationErrorClass::DataSafetyRisk,
        }
    }

    pub fn retryable(&self) -> bool {
        matches!(self.class(), BatchAutomationErrorClass::Recoverable)
    }

    const fn new(code: &'static str) -> Self {
        Self { code }
    }
}

impl fmt::Display for BatchAutomationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code)
    }
}

impl std::error::Error for BatchAutomationError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BatchAttemptSnapshot {
    pub batch_id: String,
    pub operation: BatchOperation,
    pub attempt_number: u32,
    pub status: BatchAttemptStatus,
    pub task_id: Option<String>,
    pub evidence_health_degraded: bool,
    pub summary: hmm_core::BatchResultSummary,
    pub items: Vec<BatchItemResult>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BatchLifecyclePlanRequest {
    pub plan: BatchPlanRequest,
    pub replacement_targets: BTreeMap<ModId, ReplacementTargetId>,
}

impl From<BatchPlanRequest> for BatchLifecyclePlanRequest {
    fn from(plan: BatchPlanRequest) -> Self {
        Self {
            plan,
            replacement_targets: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AllowedBatchPlan {
    revision_id: hmm_core::ModRevisionId,
    digest: String,
}

struct BatchFactsProvider {
    environment: RuntimeEnvironment,
}

impl BatchPlanFactsProvider for BatchFactsProvider {
    fn read_batch_plan_facts(
        &self,
        request: &NormalizedBatchPlanRequest,
    ) -> anyhow::Result<BatchPlanFacts> {
        let read_only = ReadOnlyInstallAutomation::from_environment(&self.environment)
            .map_err(|error| anyhow::anyhow!(error.code()))?;
        match request.operation {
            BatchOperation::Install => facts_for_install_batch(&read_only, request),
            BatchOperation::Uninstall => read_only.read_batch_uninstall_facts(
                request,
                operation_environment_digest(&self.environment, request),
            ),
            BatchOperation::Reinstall => read_only.read_batch_reinstall_facts(
                request,
                operation_environment_digest(&self.environment, request),
            ),
        }
    }
}

fn facts_for_install_batch(
    read_only: &ReadOnlyInstallAutomation,
    request: &NormalizedBatchPlanRequest,
) -> anyhow::Result<BatchPlanFacts> {
    let mut items = Vec::with_capacity(request.items.len());
    for item in &request.items {
        let BatchItemInput::Install(input) = item else {
            anyhow::bail!("batch operation is not install");
        };
        let (_, _, mod_id, revision_id, plan, prerequisite) = read_only
            .build_install_plan_for_revision(
                request.game_id.as_str(),
                request.profile_id.as_str(),
                input.mod_id.as_str(),
                input.revision_id.as_str(),
                &input.layer,
            )
            .map_err(|error| anyhow::anyhow!(error.code()))?;
        let current_binding = match plan.replacement_bindings.as_slice() {
            [] => None,
            [binding] => Some(binding),
            _ => anyhow::bail!("batch install plan has multiple replacement bindings"),
        };
        anyhow::ensure!(
            current_binding == input.replacement_binding_snapshot.as_ref(),
            "batch install canonical source binding changed"
        );
        items.push(facts_for_install(
            &mod_id,
            &revision_id,
            &plan,
            &prerequisite,
        ));
    }

    let environment_digest = digest_json(&(
        "hmm-batch-environment-v1",
        request.game_id.as_str(),
        request.profile_id.as_str(),
        items
            .iter()
            .map(|item| &item.fact_digest)
            .collect::<Vec<_>>(),
    ));
    let prerequisite_rules_version = items
        .iter()
        .find_map(|item| item.prerequisite.rules_version);

    Ok(BatchPlanFacts {
        environment_digest,
        prerequisite_rules_version,
        global_blocking_reasons: Vec::new(),
        items,
    })
}

fn operation_environment_digest(
    environment: &RuntimeEnvironment,
    request: &NormalizedBatchPlanRequest,
) -> String {
    digest_json(&(
        "hmm-batch-environment-v2",
        environment
            .sandbox_data_dir()
            .map(|path| path.to_string_lossy().into_owned()),
        request.operation,
        request.game_id.as_str(),
        request.profile_id.as_str(),
    ))
}

fn facts_for_install(
    mod_id: &hmm_core::ModId,
    revision_id: &hmm_core::ModRevisionId,
    plan: &InstallPlan,
    prerequisite: &hmm_app::GamePrerequisiteDecision,
) -> BatchItemFacts {
    let prerequisite = project_prerequisite(prerequisite);
    let mut target_claims = plan
        .actions
        .iter()
        .map(|action| BatchTargetClaim {
            target_path: action.target_path.clone(),
            kind: BatchTargetWriteKind::Install,
        })
        .collect::<Vec<_>>();
    target_claims.extend(plan.conflicts.iter().flat_map(|conflict| {
        conflict.providers.iter().map(|_| BatchTargetClaim {
            target_path: conflict.target_path.clone(),
            kind: BatchTargetWriteKind::Install,
        })
    }));
    target_claims.sort_by(|left, right| {
        left.windows_key()
            .cmp(&right.windows_key())
            .then_with(|| left.kind.as_str().cmp(right.kind.as_str()))
    });

    let action_count = plan.actions.len()
        + plan
            .conflicts
            .iter()
            .map(|conflict| conflict.providers.len())
            .sum::<usize>();
    let action_summary = BatchActionSummary {
        actions: action_count,
        added: action_count,
        ..BatchActionSummary::default()
    };
    let blocking_reasons = if !plan.conflicts.is_empty() {
        vec!["install_plan_conflict".to_owned()]
    } else {
        Vec::new()
    };
    let warning_codes = if prerequisite.status == BatchPreflightStatus::Warning {
        prerequisite.codes.clone()
    } else {
        Vec::new()
    };
    let fact_digest = digest_json(&(
        "hmm-batch-fact-v1",
        mod_id.as_str(),
        revision_id.as_str(),
        plan,
        &prerequisite,
    ));
    let single_plan_digest = digest_json(&(
        "hmm-batch-single-plan-v1",
        mod_id.as_str(),
        revision_id.as_str(),
        plan,
        &prerequisite,
    ));

    BatchItemFacts {
        mod_id: mod_id.clone(),
        source_revision_id: Some(revision_id.clone()),
        installed_revision_id: None,
        fact_digest,
        single_plan_digest,
        target_claims,
        action_summary,
        prerequisite,
        blocking_reasons,
        warning_codes,
    }
}

fn project_prerequisite(decision: &hmm_app::GamePrerequisiteDecision) -> BatchPreflightDecision {
    let status = match decision.status {
        hmm_app::GamePrerequisiteDecisionStatus::Ready => BatchPreflightStatus::Ready,
        hmm_app::GamePrerequisiteDecisionStatus::Warning => BatchPreflightStatus::Warning,
        hmm_app::GamePrerequisiteDecisionStatus::Blocked => BatchPreflightStatus::Blocked,
    };
    BatchPreflightDecision {
        status,
        rules_version: decision.rules_version,
        codes: decision
            .codes
            .iter()
            .map(|code| code.as_str().to_owned())
            .collect(),
    }
}

fn digest_json<T: Serialize>(value: &T) -> String {
    let bytes = serde_json::to_vec(value).expect("batch digest input is serializable");
    let digest = Sha256::digest(bytes);
    let mut value = String::with_capacity("sha256:".len() + digest.len() * 2);
    value.push_str("sha256:");
    for byte in digest {
        write!(&mut value, "{byte:02x}").expect("writing to String cannot fail");
    }
    value
}

struct ReadOnlyBatchSealRepository;

impl BatchSealRepository for ReadOnlyBatchSealRepository {
    fn seal_batch(&self, _request: hmm_ports::BatchSealRequest<'_>) -> anyhow::Result<()> {
        anyhow::bail!("read-only batch plan cannot seal")
    }
}

/// 写根守卫：Sandbox 重验 marker/containment capability；Production 的根事实是
/// register 时（锁外）从已保存配置读取的游戏根，锁内重载比较，配置漂移 fail closed
/// （与 CLI-3B 单项 lifecycle 的 Production 语义一致）。
enum BatchRootGuard {
    Sandbox {
        capability: Arc<SandboxWriteCapability>,
        sandbox_root: PathBuf,
    },
    Production,
}

struct BatchWriteAdmission {
    root_guard: BatchRootGuard,
    game_config_repository: Arc<dyn hmm_ports::GameConfigRepository>,
    expected: Mutex<Option<ExpectedBatchPlans>>,
}

type ExpectedBatchPlans = (
    hmm_core::GameId,
    hmm_core::ProfileId,
    BTreeMap<hmm_core::ModId, AllowedBatchPlan>,
    Option<PathBuf>,
);

impl BatchWriteAdmission {
    fn register_batch(&self, batch: &SealedBatch) -> Result<(), BatchAutomationError> {
        // Production：锁外记录已保存配置的游戏根，供锁内一致性重验。
        let expected_game_root = match &self.root_guard {
            BatchRootGuard::Production => {
                let instance = self
                    .game_config_repository
                    .load_game_instance(&batch.plan.game_id)
                    .map_err(|_| BatchAutomationError::new("batch_write_admission_unavailable"))?
                    .ok_or_else(|| {
                        BatchAutomationError::new("batch_write_admission_unavailable")
                    })?;
                if !instance.root_dir.is_dir() {
                    return Err(BatchAutomationError::new(
                        "batch_write_admission_unavailable",
                    ));
                }
                Some(instance.root_dir)
            }
            BatchRootGuard::Sandbox { .. } => None,
        };
        let allowed = batch
            .plan
            .items
            .iter()
            .filter_map(|item| match &item.input_snapshot {
                BatchItemInput::Install(input) => Some((
                    input.mod_id.clone(),
                    AllowedBatchPlan {
                        revision_id: input.revision_id.clone(),
                        digest: item.single_plan_digest.clone(),
                    },
                )),
                _ => None,
            })
            .collect::<BTreeMap<_, _>>();
        let mut expected = self
            .expected
            .lock()
            .map_err(|_| BatchAutomationError::new("batch_write_admission_unavailable"))?;
        *expected = Some((
            batch.plan.game_id.clone(),
            batch.plan.profile_id.clone(),
            allowed,
            expected_game_root,
        ));
        Ok(())
    }

    fn revalidate_roots(
        &self,
        expected_game: &hmm_core::GameId,
        expected_profile: &hmm_core::ProfileId,
        expected_game_root: Option<&PathBuf>,
        game_id: &hmm_core::GameId,
        profile_id: &hmm_core::ProfileId,
    ) -> Result<(), hmm_app::InstallWriteAdmissionError> {
        match &self.root_guard {
            BatchRootGuard::Sandbox {
                capability,
                sandbox_root,
            } => revalidate_sandbox_write_roots(
                capability.as_ref(),
                sandbox_root,
                self.game_config_repository.as_ref(),
                expected_game,
                expected_profile,
                game_id,
                profile_id,
            ),
            BatchRootGuard::Production => {
                if game_id != expected_game || profile_id != expected_profile {
                    return Err(hmm_app::InstallWriteAdmissionError::SafetyRejected);
                }
                let expected_root = expected_game_root
                    .ok_or(hmm_app::InstallWriteAdmissionError::SafetyRejected)?;
                let instance = self
                    .game_config_repository
                    .load_game_instance(game_id)
                    .map_err(|_| hmm_app::InstallWriteAdmissionError::SafetyRejected)?
                    .ok_or(hmm_app::InstallWriteAdmissionError::SafetyRejected)?;
                if &instance.root_dir != expected_root || !instance.root_dir.is_dir() {
                    return Err(hmm_app::InstallWriteAdmissionError::SafetyRejected);
                }
                Ok(())
            }
        }
    }
}

impl InstallWriteAdmission for BatchWriteAdmission {
    fn ensure_write_allowed(
        &self,
        game_id: &hmm_core::GameId,
        profile_id: &hmm_core::ProfileId,
    ) -> Result<(), hmm_app::InstallWriteAdmissionError> {
        let expected = self
            .expected
            .lock()
            .map_err(|_| hmm_app::InstallWriteAdmissionError::SafetyRejected)?;
        let Some((expected_game, expected_profile, _, expected_root)) = expected.as_ref() else {
            return Err(hmm_app::InstallWriteAdmissionError::SafetyRejected);
        };
        self.revalidate_roots(
            expected_game,
            expected_profile,
            expected_root.as_ref(),
            game_id,
            profile_id,
        )
    }

    fn ensure_install_plan_allowed(
        &self,
        game_id: &hmm_core::GameId,
        profile_id: &hmm_core::ProfileId,
        mod_id: &hmm_core::ModId,
        plan: &InstallPlan,
        prerequisite_decision: &hmm_app::GamePrerequisiteDecision,
    ) -> Result<(), hmm_app::InstallWriteAdmissionError> {
        if prerequisite_decision.is_blocked() || plan.has_blocking_conflicts() {
            return Err(hmm_app::InstallWriteAdmissionError::SafetyRejected);
        }
        let expected = self
            .expected
            .lock()
            .map_err(|_| hmm_app::InstallWriteAdmissionError::SafetyRejected)?;
        let Some((expected_game, expected_profile, allowed, expected_root)) = expected.as_ref()
        else {
            return Err(hmm_app::InstallWriteAdmissionError::SafetyRejected);
        };
        let Some(item) = allowed.get(mod_id) else {
            return Err(hmm_app::InstallWriteAdmissionError::SafetyRejected);
        };
        let digest = digest_json(&(
            "hmm-batch-single-plan-v1",
            mod_id.as_str(),
            item.revision_id.as_str(),
            plan,
            project_prerequisite(prerequisite_decision),
        ));
        if digest != item.digest {
            return Err(hmm_app::InstallWriteAdmissionError::SafetyRejected);
        }
        self.revalidate_roots(
            expected_game,
            expected_profile,
            expected_root.as_ref(),
            game_id,
            profile_id,
        )
    }
}

struct WriteContext {
    repository: Arc<dyn BatchLifecycleRepository>,
    plan_service: BatchPlanService,
    runner: BatchInstallTaskRunner,
    retry: BatchInstallRetryService,
    admission: Arc<BatchWriteAdmission>,
}

pub struct BatchLifecycleAutomation;

impl BatchLifecycleAutomation {
    pub fn preview_request(
        environment: &RuntimeEnvironment,
        request: BatchLifecyclePlanRequest,
    ) -> Result<BatchPlanPreview, BatchAutomationError> {
        let request = resolve_batch_plan_request(environment, request, "batch_input_invalid")?;
        Self::preview(environment, request)
    }

    pub fn preview(
        environment: &RuntimeEnvironment,
        request: BatchPlanRequest,
    ) -> Result<BatchPlanPreview, BatchAutomationError> {
        build_read_only_plan_service(environment)?
            .preview(request)
            .map_err(map_preview_error)
    }

    pub fn apply(
        environment: &RuntimeEnvironment,
        request: BatchPlanRequest,
        preview_token: &str,
    ) -> Result<(BatchPlanSealResult, BatchInstallRunResult), BatchAutomationError> {
        // Reject stale input before constructing HmmRuntime, whose initialization creates the
        // sandbox journal. The write path repeats this validation inside `seal` to close the
        // validation-to-persistence TOCTOU window.
        precheck_batch_token(preview_token, "preview")?;
        build_read_only_plan_service(environment)?
            .validate_preview(request.clone(), preview_token)
            .map_err(map_seal_error)?;
        ensure_scope_reconciled(environment, &request.game_id, &request.profile_id, None)?;
        let context = build_write_context(environment, request.operation)?;
        let sealed = context
            .plan_service
            .seal(request, preview_token)
            .map_err(map_seal_error)?;
        let batch_id = BatchId::new(sealed.batch_id.clone());
        let batch = context
            .repository
            .load_batch(&batch_id)
            .map_err(|_| BatchAutomationError::new("batch_journal_unavailable"))?
            .ok_or_else(|| BatchAutomationError::new("batch_unavailable"))?;
        context.admission.register_batch(&batch)?;
        let run = context
            .runner
            .run(&batch_id, &sealed.plan_token)
            .map_err(map_run_error)?;
        Ok((sealed, run))
    }

    pub fn apply_request(
        environment: &RuntimeEnvironment,
        request: BatchLifecyclePlanRequest,
        preview_token: &str,
    ) -> Result<(BatchOperation, BatchPlanSealResult, BatchInstallRunResult), BatchAutomationError>
    {
        precheck_batch_token(preview_token, "preview")?;
        let request = resolve_batch_plan_request(environment, request, "batch_plan_stale")?;
        let operation = request.operation;
        Self::apply(environment, request, preview_token)
            .map(|(sealed, run)| (operation, sealed, run))
    }

    /// Seals a previously previewed request without starting execution. This is the Tauri
    /// counterpart of the CLI `apply` path: it revalidates the preview token and current facts,
    /// persists the sealed batch journal (attempt 0) and returns the opaque `planToken`.
    pub fn seal_request(
        environment: &RuntimeEnvironment,
        request: BatchLifecyclePlanRequest,
        preview_token: &str,
    ) -> Result<(BatchOperation, BatchPlanSealResult), BatchAutomationError> {
        Self::seal_request_internal(environment, request, preview_token, None)
    }

    /// Seals a batch from a GUI process that already owns the app database connection. The
    /// shared handle is used only for journal reads; CLI callers keep the immutable snapshot
    /// path and its fail-closed WAL checks.
    pub fn seal_request_with_database(
        environment: &RuntimeEnvironment,
        request: BatchLifecyclePlanRequest,
        preview_token: &str,
        database: Arc<Mutex<rusqlite::Connection>>,
    ) -> Result<(BatchOperation, BatchPlanSealResult), BatchAutomationError> {
        Self::seal_request_internal(environment, request, preview_token, Some(database))
    }

    fn seal_request_internal(
        environment: &RuntimeEnvironment,
        request: BatchLifecyclePlanRequest,
        preview_token: &str,
        database: Option<SharedBatchDatabase>,
    ) -> Result<(BatchOperation, BatchPlanSealResult), BatchAutomationError> {
        precheck_batch_token(preview_token, "preview")?;
        let request = resolve_batch_plan_request(environment, request, "batch_plan_stale")?;
        let operation = request.operation;
        build_read_only_plan_service(environment)?
            .validate_preview(request.clone(), preview_token)
            .map_err(map_seal_error)?;
        ensure_scope_reconciled(
            environment,
            &request.game_id,
            &request.profile_id,
            database.as_ref(),
        )?;
        let context = build_write_context(environment, request.operation)?;
        let sealed = context
            .plan_service
            .seal(request, preview_token)
            .map_err(map_seal_error)?;
        Ok((operation, sealed))
    }

    /// Starts the sealed attempt 0 of a batch identified by `batch_id`, consuming the opaque
    /// `planToken` returned by `seal_request`. Admission is the same CAS used by `apply`; a
    /// repeated start for an already-admitted attempt returns the same task id.
    pub fn start_request(
        environment: &RuntimeEnvironment,
        batch_id: &str,
        plan_token: &str,
    ) -> Result<(BatchOperation, BatchInstallRunResult), BatchAutomationError> {
        Self::start_request_internal(environment, batch_id, plan_token, None)
    }

    /// Starts a batch while reading its sealed identity through the GUI-owned database handle.
    pub fn start_request_with_database(
        environment: &RuntimeEnvironment,
        batch_id: &str,
        plan_token: &str,
        database: Arc<Mutex<rusqlite::Connection>>,
    ) -> Result<(BatchOperation, BatchInstallRunResult), BatchAutomationError> {
        Self::start_request_internal(environment, batch_id, plan_token, Some(database))
    }

    fn start_request_internal(
        environment: &RuntimeEnvironment,
        batch_id: &str,
        plan_token: &str,
        database: Option<SharedBatchDatabase>,
    ) -> Result<(BatchOperation, BatchInstallRunResult), BatchAutomationError> {
        let batch_id = parse_batch_id(batch_id)?;
        precheck_batch_token(plan_token, "plan")?;
        let operation = {
            let repository = open_batch_repository_read_only(
                environment,
                false,
                "batch_unavailable",
                database.as_ref(),
            )?
            .ok_or_else(|| BatchAutomationError::new("batch_unavailable"))?;
            let batch = repository
                .load_batch(&batch_id)
                .map_err(|_| BatchAutomationError::new("batch_unavailable"))?
                .ok_or_else(|| BatchAutomationError::new("batch_unavailable"))?;
            batch.plan.operation
        };
        let context = build_write_context(environment, operation)?;
        let batch = context
            .repository
            .load_batch(&batch_id)
            .map_err(|_| BatchAutomationError::new("batch_journal_unavailable"))?
            .ok_or_else(|| BatchAutomationError::new("batch_unavailable"))?;
        if batch.plan.operation != operation {
            return Err(BatchAutomationError::new("batch_operation_mismatch"));
        }
        context.admission.register_batch(&batch)?;
        let run = context
            .runner
            .run_attempt(&batch_id, 0, plan_token)
            .map_err(map_run_error)?;
        Ok((operation, run))
    }

    pub fn retry(
        environment: &RuntimeEnvironment,
        batch_id: &str,
        attempt_number: u32,
    ) -> Result<(BatchInstallRetryResult, BatchInstallRunResult), BatchAutomationError> {
        Self::retry_with_operation(environment, batch_id, attempt_number)
            .map(|(_, retry, run)| (retry, run))
    }

    pub fn retry_with_operation(
        environment: &RuntimeEnvironment,
        batch_id: &str,
        attempt_number: u32,
    ) -> Result<
        (
            BatchOperation,
            BatchInstallRetryResult,
            BatchInstallRunResult,
        ),
        BatchAutomationError,
    > {
        Self::retry_with_operation_internal(environment, batch_id, attempt_number, None)
    }

    /// Retries a batch while reconciling its journal through the GUI-owned database handle.
    pub fn retry_with_operation_with_database(
        environment: &RuntimeEnvironment,
        batch_id: &str,
        attempt_number: u32,
        database: Arc<Mutex<rusqlite::Connection>>,
    ) -> Result<
        (
            BatchOperation,
            BatchInstallRetryResult,
            BatchInstallRunResult,
        ),
        BatchAutomationError,
    > {
        Self::retry_with_operation_internal(environment, batch_id, attempt_number, Some(database))
    }

    fn retry_with_operation_internal(
        environment: &RuntimeEnvironment,
        batch_id: &str,
        attempt_number: u32,
        database: Option<SharedBatchDatabase>,
    ) -> Result<
        (
            BatchOperation,
            BatchInstallRetryResult,
            BatchInstallRunResult,
        ),
        BatchAutomationError,
    > {
        let batch_id = parse_batch_id(batch_id)?;
        let (_, reconciled_batch) = ensure_batch_reconciled(
            environment,
            &batch_id,
            "batch_unavailable",
            database.as_ref(),
        )?;
        let operation = reconciled_batch.plan.operation;
        let context = build_write_context(environment, operation)?;
        let retry = context
            .retry
            .retry(&batch_id, attempt_number)
            .map_err(map_retry_error)?;
        let batch = context
            .repository
            .load_batch(&batch_id)
            .map_err(|_| BatchAutomationError::new("batch_journal_unavailable"))?
            .ok_or_else(|| BatchAutomationError::new("batch_unavailable"))?;
        if batch.plan.operation != operation {
            return Err(BatchAutomationError::new("batch_operation_mismatch"));
        }
        context.admission.register_batch(&batch)?;
        let run = context
            .runner
            .run_attempt(&batch_id, retry.attempt_number, &retry.plan_token)
            .map_err(map_run_error)?;
        Ok((operation, retry, run))
    }

    pub fn result(
        environment: &RuntimeEnvironment,
        batch_id: &str,
        attempt_number: u32,
    ) -> Result<BatchAttemptSnapshot, BatchAutomationError> {
        Self::result_internal(environment, batch_id, attempt_number, None)
    }

    /// Reads a batch result through the GUI-owned database handle so committed WAL rows remain
    /// visible without weakening the immutable snapshot contract used by CLI diagnostics.
    pub fn result_with_database(
        environment: &RuntimeEnvironment,
        batch_id: &str,
        attempt_number: u32,
        database: Arc<Mutex<rusqlite::Connection>>,
    ) -> Result<BatchAttemptSnapshot, BatchAutomationError> {
        Self::result_internal(environment, batch_id, attempt_number, Some(database))
    }

    fn result_internal(
        environment: &RuntimeEnvironment,
        batch_id: &str,
        attempt_number: u32,
        database: Option<SharedBatchDatabase>,
    ) -> Result<BatchAttemptSnapshot, BatchAutomationError> {
        let batch_id = parse_batch_id(batch_id)?;
        let repository = open_batch_repository_read_only(
            environment,
            false,
            "batch_result_unavailable",
            database.as_ref(),
        )?
        .ok_or_else(|| BatchAutomationError::new("batch_result_unavailable"))?;
        let batch = repository
            .load_batch(&batch_id)
            .map_err(|_| BatchAutomationError::new("batch_result_unavailable"))?
            .ok_or_else(|| BatchAutomationError::new("batch_result_unavailable"))?;
        let attempt = repository
            .load_attempt(&batch_id, attempt_number)
            .map_err(|_| BatchAutomationError::new("batch_result_unavailable"))?
            .ok_or_else(|| BatchAutomationError::new("batch_result_unavailable"))?;
        let items = repository
            .list_item_results(&batch_id, attempt_number)
            .map_err(|_| BatchAutomationError::new("batch_result_unavailable"))?;
        Ok(snapshot(batch, attempt, items))
    }
}

fn resolve_batch_plan_request(
    environment: &RuntimeEnvironment,
    request: BatchLifecyclePlanRequest,
    unavailable_code: &'static str,
) -> Result<BatchPlanRequest, BatchAutomationError> {
    let BatchLifecyclePlanRequest {
        mut plan,
        mut replacement_targets,
    } = request;
    if plan.operation != BatchOperation::Reinstall && !replacement_targets.is_empty() {
        return Err(BatchAutomationError::new("batch_input_invalid"));
    }

    let requires_resolution = plan.operation == BatchOperation::Install
        || plan.items.iter().any(|item| {
            matches!(
                item,
                BatchItemInput::Reinstall(input)
                    if input.installed_revision_id == input.candidate_revision_id
            )
        });
    let read_only = requires_resolution
        .then(|| ReadOnlyInstallAutomation::from_environment(environment))
        .transpose()
        .map_err(|_| BatchAutomationError::new(unavailable_code))?;

    for item in &mut plan.items {
        match item {
            BatchItemInput::Install(input) => {
                if input.replacement_binding_snapshot.is_some() {
                    return Err(BatchAutomationError::new("batch_input_invalid"));
                }
                let (_, _, _, _, install_plan, _) = read_only
                    .as_ref()
                    .expect("batch install initialized read-only automation")
                    .build_install_plan_for_revision(
                        plan.game_id.as_str(),
                        plan.profile_id.as_str(),
                        input.mod_id.as_str(),
                        input.revision_id.as_str(),
                        &input.layer,
                    )
                    .map_err(|_| BatchAutomationError::new(unavailable_code))?;
                input.replacement_binding_snapshot =
                    match install_plan.replacement_bindings.as_slice() {
                        [] => None,
                        [binding] => Some(binding.clone()),
                        _ => return Err(BatchAutomationError::new(unavailable_code)),
                    };
            }
            BatchItemInput::Reinstall(input) => {
                if input.installed_revision_id == input.candidate_revision_id {
                    if input.replacement_binding_snapshot.is_some() {
                        return Err(BatchAutomationError::new("batch_input_invalid"));
                    }
                    let target_id = replacement_targets
                        .remove(&input.mod_id)
                        .ok_or_else(|| BatchAutomationError::new("batch_input_invalid"))?;
                    let binding = read_only
                        .as_ref()
                        .expect("same-revision reinstall initialized read-only automation")
                        .resolve_batch_replacement_binding(
                            &plan.game_id,
                            &plan.profile_id,
                            input,
                            &target_id,
                        )
                        .map_err(|_| BatchAutomationError::new(unavailable_code))?;
                    input.replacement_binding_snapshot = Some(binding);
                } else if input.replacement_binding_snapshot.is_some()
                    || replacement_targets.contains_key(&input.mod_id)
                {
                    return Err(BatchAutomationError::new("batch_input_invalid"));
                }
            }
            BatchItemInput::Uninstall(_) => {}
        }
    }
    if !replacement_targets.is_empty() {
        return Err(BatchAutomationError::new("batch_input_invalid"));
    }
    Ok(plan)
}

fn build_read_only_plan_service(
    environment: &RuntimeEnvironment,
) -> Result<BatchPlanService, BatchAutomationError> {
    let facts = Arc::new(BatchFactsProvider {
        environment: environment.clone(),
    });
    let token_codec = batch_token_codec(environment)?;
    Ok(BatchPlanService::new(
        facts,
        Arc::new(ReadOnlyBatchSealRepository),
        Arc::new(SystemClock),
        token_codec,
    ))
}

fn build_write_context(
    environment: &RuntimeEnvironment,
    operation: BatchOperation,
) -> Result<WriteContext, BatchAutomationError> {
    let context = BatchEnvironmentContext::resolve(environment)?;
    let root_guard = match &context.sandbox_root {
        Some(sandbox_root) => BatchRootGuard::Sandbox {
            capability: Arc::new(
                environment
                    .acquire_sandbox_write_capability()
                    .map_err(|_| BatchAutomationError::new("batch_write_admission_unavailable"))?,
            ),
            sandbox_root: sandbox_root.clone(),
        },
        None => BatchRootGuard::Production,
    };
    let game_config_repository: Arc<dyn hmm_ports::GameConfigRepository> = Arc::new(
        JsonGameConfigRepository::new(context.data_root.join("config").join("games.json")),
    );
    let admission = Arc::new(BatchWriteAdmission {
        root_guard,
        game_config_repository,
        expected: Mutex::new(None),
    });
    let runtime = HmmRuntime::builder(context.data_root.clone())
        .with_install_write_admission(admission.clone())
        .build()
        .map_err(|_| BatchAutomationError::new("batch_runtime_unavailable"))?;
    let repository_impl = Arc::new(SqliteBatchLifecycleRepository::new(
        runtime.database_handle(),
    ));
    let repository: Arc<dyn BatchLifecycleRepository> = repository_impl.clone();
    let seal_repository: Arc<dyn BatchSealRepository> = repository_impl;
    let facts = Arc::new(BatchFactsProvider {
        environment: environment.clone(),
    });
    let token_codec = batch_token_codec(environment)?;
    let plan_service = BatchPlanService::new(
        facts.clone(),
        seal_repository,
        Arc::new(SystemClock),
        Arc::clone(&token_codec),
    );
    let executor: Arc<dyn BatchInstallItemExecutor> = match operation {
        BatchOperation::Install => Arc::new(InstallTaskBatchItemExecutor::new(
            Arc::clone(&runtime.install_task_runner),
            Arc::clone(&runtime.task_manager),
        )),
        BatchOperation::Uninstall => Arc::new(UninstallTaskBatchItemExecutor::new(
            Arc::clone(&runtime.uninstall_task_runner),
            Arc::clone(&runtime.task_manager),
        )),
        BatchOperation::Reinstall => Arc::new(ReinstallTaskBatchItemExecutor::new(
            Arc::clone(&runtime.reinstall_task_runner),
            Arc::clone(&runtime.task_manager),
        )),
    };
    let runner = BatchInstallTaskRunner::for_operation(
        operation,
        Arc::clone(&runtime.task_manager),
        Arc::clone(&repository),
        executor,
        facts,
        runtime.audit_log_writer(),
        Arc::new(SystemClock),
        Arc::clone(&token_codec),
    );
    let retry = BatchInstallRetryService::for_operation(
        operation,
        Arc::clone(&repository),
        Arc::new(SystemClock),
        token_codec,
    );
    Ok(WriteContext {
        repository,
        plan_service,
        runner,
        retry,
        admission,
    })
}

/// 批量链路的环境事实：数据根与 Sandbox 根。Production 数据根仅由操作系统解析
/// （或 crate 内测试注入），与单项 lifecycle 的 CLI-3B 约束一致，没有 CLI 注入面。
struct BatchEnvironmentContext {
    data_root: PathBuf,
    sandbox_root: Option<PathBuf>,
}

impl BatchEnvironmentContext {
    fn resolve(environment: &RuntimeEnvironment) -> Result<Self, BatchAutomationError> {
        match environment.sandbox_data_dir() {
            Some(root) => Ok(Self {
                data_root: root.to_path_buf(),
                sandbox_root: Some(root.to_path_buf()),
            }),
            None => Ok(Self {
                data_root: environment
                    .resolved_production_app_data_dir()
                    .ok_or_else(|| BatchAutomationError::new("batch_runtime_unavailable"))?,
                sandbox_root: None,
            }),
        }
    }
}

fn batch_token_codec(
    environment: &RuntimeEnvironment,
) -> Result<Arc<dyn BatchTokenCodec>, BatchAutomationError> {
    let context = BatchEnvironmentContext::resolve(environment)?;
    // Sandbox key 是隔离根派生的可推导 stale tag（其安全性由 sandbox 隔离承担）；
    // Production key 是 per-installation 随机 secret，token 不可离线伪造（CLI-3C）。
    let secret: Vec<u8> = match &context.sandbox_root {
        Some(root) => format!("{BATCH_TOKEN_TAG_PREFIX}\0{}", root.display()).into_bytes(),
        None => crate::batch_token_secret::load_or_create_batch_token_secret(&context.data_root)
            .map_err(|_| BatchAutomationError::new("batch_secret_unavailable"))?,
    };
    let codec = Sha256BatchTokenCodec::new(secret)
        .map_err(|_| BatchAutomationError::new("batch_token_unavailable"))?;
    Ok(Arc::new(codec))
}

/// 纯语法 token 预检：格式与有效期，不读取任何数据根。放在会触达文件系统的
/// 入口最前，让明显无效的 token 在 Production 下也不触发真实数据读取。
fn precheck_batch_token(token: &str, expected_kind: &str) -> Result<(), BatchAutomationError> {
    let parts = token.split('.').collect::<Vec<_>>();
    if parts.len() != 5 || parts[0] != "hmm-batch-v2" || parts[1] != expected_kind {
        return Err(BatchAutomationError::new("batch_token_invalid"));
    }
    let expires_at: u128 = parts[3]
        .parse()
        .map_err(|_| BatchAutomationError::new("batch_token_invalid"))?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| BatchAutomationError::new("batch_internal_error"))?
        .as_millis();
    if now >= expires_at {
        return Err(BatchAutomationError::new("batch_plan_expired"));
    }
    Ok(())
}

fn parse_batch_id(value: &str) -> Result<BatchId, BatchAutomationError> {
    let value = value.trim();
    if value.len() > 128
        || value.is_empty()
        || !value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
    {
        return Err(BatchAutomationError::new("batch_id_invalid"));
    }
    Ok(BatchId::new(value))
}

type ReadOnlyBatchRepository = Arc<dyn BatchLifecycleRepository>;
type SharedBatchDatabase = Arc<Mutex<rusqlite::Connection>>;

fn open_batch_repository_read_only(
    environment: &RuntimeEnvironment,
    missing_is_empty: bool,
    unavailable_code: &'static str,
    database: Option<&SharedBatchDatabase>,
) -> Result<Option<ReadOnlyBatchRepository>, BatchAutomationError> {
    if let Some(database) = database {
        return Ok(Some(Arc::new(SqliteBatchLifecycleRepository::new(
            Arc::clone(database),
        ))));
    }
    // Production 下 GUI 持有活跃 WAL 时，immutable 快照打开会 fail closed
    // （与 backup 只读 facade 同一行为）：需要一致结果时先关闭桌面端。
    let context = BatchEnvironmentContext::resolve(environment)?;
    let database_path = context.data_root.join("hmm.db");
    match fs::symlink_metadata(&database_path) {
        Ok(metadata) if metadata.file_type().is_file() => {}
        Ok(_) => return Err(BatchAutomationError::new(unavailable_code)),
        Err(error) if error.kind() == ErrorKind::NotFound && missing_is_empty => return Ok(None),
        Err(_) => return Err(BatchAutomationError::new(unavailable_code)),
    }
    let connection = open_database_read_only(&database_path)
        .map_err(|_| BatchAutomationError::new(unavailable_code))?;
    Ok(Some(Arc::new(SqliteBatchLifecycleRepository::new(
        Arc::new(Mutex::new(connection)),
    ))))
}

fn ensure_scope_reconciled(
    environment: &RuntimeEnvironment,
    game_id: &hmm_core::GameId,
    profile_id: &hmm_core::ProfileId,
    database: Option<&SharedBatchDatabase>,
) -> Result<(), BatchAutomationError> {
    let Some(repository) =
        open_batch_repository_read_only(environment, true, "batch_journal_unavailable", database)?
    else {
        return Ok(());
    };
    let active = repository
        .find_active_attempt_for_scope(game_id, profile_id)
        .map_err(|_| BatchAutomationError::new("batch_journal_unavailable"))?;
    if active.is_some() {
        return Err(BatchAutomationError::new(
            "batch_attempt_reconciliation_required",
        ));
    }
    Ok(())
}

fn ensure_batch_reconciled(
    environment: &RuntimeEnvironment,
    batch_id: &BatchId,
    unavailable_code: &'static str,
    database: Option<&SharedBatchDatabase>,
) -> Result<(ReadOnlyBatchRepository, SealedBatch), BatchAutomationError> {
    let repository =
        open_batch_repository_read_only(environment, false, unavailable_code, database)?
            .ok_or_else(|| BatchAutomationError::new(unavailable_code))?;
    let batch = repository
        .load_batch(batch_id)
        .map_err(|_| BatchAutomationError::new(unavailable_code))?
        .ok_or_else(|| BatchAutomationError::new(unavailable_code))?;
    let active = repository
        .find_active_attempt_for_scope(&batch.plan.game_id, &batch.plan.profile_id)
        .map_err(|_| BatchAutomationError::new(unavailable_code))?;
    if active.is_some() {
        return Err(BatchAutomationError::new(
            "batch_attempt_reconciliation_required",
        ));
    }
    Ok((repository, batch))
}

fn snapshot(
    batch: SealedBatch,
    attempt: BatchAttempt,
    items: Vec<BatchItemResult>,
) -> BatchAttemptSnapshot {
    BatchAttemptSnapshot {
        batch_id: batch.batch_id.as_str().to_owned(),
        operation: batch.plan.operation,
        attempt_number: attempt.attempt_number,
        status: attempt.status,
        task_id: attempt.task_id,
        evidence_health_degraded: attempt.evidence_health_degraded,
        summary: hmm_core::BatchResultSummary::from_item_results(attempt.item_ids.len(), &items),
        items,
    }
}

fn map_preview_error(error: BatchPlanPreviewError) -> BatchAutomationError {
    BatchAutomationError::new(error.code())
}

fn map_seal_error(error: BatchPlanSealError) -> BatchAutomationError {
    BatchAutomationError::new(error.code())
}

fn map_run_error(error: BatchInstallRunError) -> BatchAutomationError {
    let code = match error {
        BatchInstallRunError::BatchUnavailable => "batch_unavailable",
        BatchInstallRunError::InvalidToken => "batch_token_invalid",
        BatchInstallRunError::PlanBlocked => "batch_plan_blocked",
        BatchInstallRunError::OperationMismatch => "batch_operation_mismatch",
        BatchInstallRunError::AdmissionRejected => "batch_admission_rejected",
        BatchInstallRunError::ScopeReconciliationRequired => {
            "batch_attempt_reconciliation_required"
        }
        BatchInstallRunError::JournalUnavailable => "batch_journal_unavailable",
        BatchInstallRunError::TaskUnavailable => "batch_task_unavailable",
    };
    BatchAutomationError::new(code)
}

fn map_retry_error(error: BatchInstallRetryError) -> BatchAutomationError {
    let code = match error {
        BatchInstallRetryError::BatchUnavailable => "batch_unavailable",
        BatchInstallRetryError::RetryUnavailable => "batch_retry_unavailable",
        BatchInstallRetryError::AttemptStale => "batch_attempt_stale",
        BatchInstallRetryError::ClockUnavailable => "batch_internal_error",
        BatchInstallRetryError::TokenIssueFailed => "batch_internal_error",
        BatchInstallRetryError::JournalUnavailable => "batch_journal_unavailable",
    };
    BatchAutomationError::new(code)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lifecycle_automation::write_install_fixture;
    use hmm_core::{
        BatchAttemptStatus, BatchExecutionPolicy, BatchItemInput, BatchPlanRequest, FileLayer,
        GameId, InstallBatchItemInput, InstallManifest, ModId, ModRevisionId, ProfileId,
        ReinstallBatchItemInput, BATCH_PLAN_SCHEMA_VERSION,
    };
    use hmm_games_mhw::MhwArmorCatalog;
    use hmm_ports::ReplacementCatalogProvider;
    use std::collections::BTreeMap;

    #[test]
    fn batch_automation_errors_have_stable_display_and_classification() {
        let cases = [
            (
                "sandbox_batch_production_forbidden",
                BatchAutomationErrorClass::DataSafetyRisk,
                false,
            ),
            (
                "batch_attempt_reconciliation_required",
                BatchAutomationErrorClass::DataSafetyRisk,
                false,
            ),
            (
                "batch_token_invalid",
                BatchAutomationErrorClass::UserActionRequired,
                false,
            ),
            (
                "batch_resource_limit_exceeded",
                BatchAutomationErrorClass::UserActionRequired,
                false,
            ),
            (
                "batch_id_invalid",
                BatchAutomationErrorClass::UserActionRequired,
                false,
            ),
            (
                "batch_result_unavailable",
                BatchAutomationErrorClass::Recoverable,
                true,
            ),
        ];
        for (code, class, retryable) in cases {
            let error = BatchAutomationError::new(code);
            assert_eq!(error.to_string(), code);
            assert_eq!(error.class(), class);
            assert_eq!(error.retryable(), retryable);
            fn assert_error<E: std::error::Error>() {}
            assert_error::<BatchAutomationError>();
        }
    }

    fn batch_install_request() -> BatchLifecyclePlanRequest {
        BatchLifecyclePlanRequest {
            plan: BatchPlanRequest {
                schema_version: BATCH_PLAN_SCHEMA_VERSION,
                operation: BatchOperation::Install,
                game_id: GameId::mhw(),
                profile_id: ProfileId::new("default"),
                execution_policy: BatchExecutionPolicy::StopOnFailure,
                items: vec![BatchItemInput::Install(InstallBatchItemInput {
                    mod_id: ModId::new("mod-a"),
                    revision_id: ModRevisionId::new("package-a"),
                    layer: FileLayer::new("base", 0),
                    replacement_binding_snapshot: None,
                })],
            },
            replacement_targets: BTreeMap::new(),
        }
    }

    fn write_batch_armor_fixture(sandbox: &std::path::Path) -> PathBuf {
        let game_root = write_install_fixture(sandbox);
        fs::write(
            sandbox.join("mod-import/results.json"),
            r#"{
  "version": 1,
  "records": [{
    "mod_id": "mod-armor",
    "task_id": "task-armor",
    "package_id": "package-armor",
    "display_name": "Armor Retarget Fixture"
  }]
}"#,
        )
        .expect("armor Mod catalog");
        let package_root = sandbox
            .join("mod-import/sandboxes/package-armor")
            .join("nativePC/pl/f_equip/pl121_0000/arm/mod");
        fs::create_dir_all(&package_root).expect("armor package root");
        fs::write(package_root.join("f_body.mod3"), b"synthetic armor fixture")
            .expect("armor package file");
        game_root
    }

    fn armor_install_request() -> BatchLifecyclePlanRequest {
        BatchLifecyclePlanRequest {
            plan: BatchPlanRequest {
                schema_version: BATCH_PLAN_SCHEMA_VERSION,
                operation: BatchOperation::Install,
                game_id: GameId::mhw(),
                profile_id: ProfileId::new("default"),
                execution_policy: BatchExecutionPolicy::StopOnFailure,
                items: vec![BatchItemInput::Install(InstallBatchItemInput {
                    mod_id: ModId::new("mod-armor"),
                    revision_id: ModRevisionId::new("package-armor"),
                    layer: FileLayer::new("base", 0),
                    replacement_binding_snapshot: None,
                })],
            },
            replacement_targets: BTreeMap::new(),
        }
    }

    fn armor_target_switch_request() -> BatchLifecyclePlanRequest {
        BatchLifecyclePlanRequest {
            plan: BatchPlanRequest {
                schema_version: BATCH_PLAN_SCHEMA_VERSION,
                operation: BatchOperation::Reinstall,
                game_id: GameId::mhw(),
                profile_id: ProfileId::new("default"),
                execution_policy: BatchExecutionPolicy::StopOnFailure,
                items: vec![BatchItemInput::Reinstall(ReinstallBatchItemInput {
                    mod_id: ModId::new("mod-armor"),
                    installed_revision_id: ModRevisionId::new("package-armor"),
                    candidate_revision_id: ModRevisionId::new("package-armor"),
                    layer: FileLayer::new("base", 0),
                    replacement_binding_snapshot: None,
                })],
            },
            replacement_targets: BTreeMap::from([(
                ModId::new("mod-armor"),
                ReplacementTargetId::parse("mhw:armor:fatalis-alpha").expect("target id"),
            )]),
        }
    }

    #[test]
    fn gui_database_handle_reads_batch_journal_while_wal_is_active() {
        let sandbox = tempfile::tempdir().expect("sandbox");
        let game_root = write_install_fixture(sandbox.path());
        let environment =
            RuntimeEnvironment::sandbox(sandbox.path().to_path_buf()).expect("environment");
        let gui_runtime =
            HmmRuntime::from_app_data_dir(sandbox.path().to_path_buf()).expect("GUI runtime");
        let gui_database = gui_runtime.database_handle();

        let wal_path = sandbox.path().join("hmm.db-wal");
        let shm_path = sandbox.path().join("hmm.db-shm");
        assert!(
            wal_path.exists() || shm_path.exists(),
            "GUI runtime should keep SQLite WAL state active"
        );

        let preview =
            BatchLifecycleAutomation::preview_request(&environment, batch_install_request())
                .expect("sandbox preview");
        let preview_token = preview.preview_token.expect("ready preview token");
        let (_, sealed) = BatchLifecycleAutomation::seal_request_with_database(
            &environment,
            batch_install_request(),
            &preview_token,
            Arc::clone(&gui_database),
        )
        .expect("sandbox seal through GUI database");

        let (_, run) = BatchLifecycleAutomation::start_request_with_database(
            &environment,
            &sealed.batch_id,
            &sealed.plan_token,
            Arc::clone(&gui_database),
        )
        .expect("sandbox start through GUI database");
        let snapshot = BatchLifecycleAutomation::result_with_database(
            &environment,
            &sealed.batch_id,
            run.attempt_number,
            Arc::clone(&gui_database),
        )
        .expect("attempt result through GUI database");

        assert_eq!(snapshot.status, BatchAttemptStatus::Completed);
        assert_eq!(snapshot.summary.succeeded_count, 1);
        assert_eq!(
            fs::read(game_root.join("nativePC/models/player.mod3")).expect("installed target"),
            b"fixture"
        );

        let snapshot_error =
            BatchLifecycleAutomation::result(&environment, &sealed.batch_id, run.attempt_number)
                .expect_err("immutable snapshot path must remain fail-closed while WAL is active");
        assert_eq!(snapshot_error.code(), "batch_result_unavailable");
    }

    #[test]
    fn seal_then_start_runs_attempt_and_repeated_start_is_idempotent() {
        let sandbox = tempfile::tempdir().expect("sandbox");
        let game_root = write_install_fixture(sandbox.path());
        let environment =
            RuntimeEnvironment::sandbox(sandbox.path().to_path_buf()).expect("environment");

        let preview =
            BatchLifecycleAutomation::preview_request(&environment, batch_install_request())
                .expect("sandbox preview");
        assert_eq!(preview.plan.status(), hmm_core::BatchPlanStatus::Ready);
        let preview_token = preview.preview_token.expect("ready preview token");

        let (operation, sealed) = BatchLifecycleAutomation::seal_request(
            &environment,
            batch_install_request(),
            &preview_token,
        )
        .expect("sandbox seal");
        assert_eq!(operation, BatchOperation::Install);
        assert_eq!(sealed.status, "sealed");
        assert!(!sealed.plan_token.is_empty());
        assert!(sealed.expires_at_unix_millis > 0);

        let (operation, run) = BatchLifecycleAutomation::start_request(
            &environment,
            &sealed.batch_id,
            &sealed.plan_token,
        )
        .expect("sandbox start");
        assert_eq!(operation, BatchOperation::Install);
        assert_eq!(run.status, BatchAttemptStatus::Completed);
        assert!(run.task_id.starts_with("install-"));
        assert_eq!(
            fs::read(game_root.join("nativePC/models/player.mod3")).expect("installed target"),
            b"fixture"
        );

        let snapshot =
            BatchLifecycleAutomation::result(&environment, &sealed.batch_id, run.attempt_number)
                .expect("attempt result");
        assert_eq!(snapshot.status, BatchAttemptStatus::Completed);
        assert_eq!(snapshot.task_id.as_deref(), Some(run.task_id.as_str()));
        assert_eq!(snapshot.summary.succeeded_count, 1);
        assert_eq!(snapshot.items.len(), 1);
        assert_eq!(
            snapshot.items[0].status,
            hmm_core::BatchItemStatus::Succeeded
        );

        let (_, repeated) = BatchLifecycleAutomation::start_request(
            &environment,
            &sealed.batch_id,
            &sealed.plan_token,
        )
        .expect("repeated start is idempotent");
        assert_eq!(repeated.task_id, run.task_id);
        assert_eq!(repeated.status, BatchAttemptStatus::Completed);
    }

    #[test]
    fn source_slot_batch_install_persists_binding_for_same_revision_target_switch() {
        let sandbox = tempfile::tempdir().expect("sandbox");
        let game_root = write_batch_armor_fixture(sandbox.path());
        let environment =
            RuntimeEnvironment::sandbox(sandbox.path().to_path_buf()).expect("environment");

        let install_preview =
            BatchLifecycleAutomation::preview_request(&environment, armor_install_request())
                .expect("armor batch install preview");
        let BatchItemInput::Install(install_input) = &install_preview.plan.items[0].input_snapshot
        else {
            panic!("install input");
        };
        let source_binding = install_input
            .replacement_binding_snapshot
            .as_ref()
            .expect("canonical source binding");
        // AR6 之后 binding 记录的是 catalog 的规范 hash ID；请求里用的旧 slug
        // 经 legacy_ids 解析到同一槽位。这里断言解析关系而不是硬编码 hash：
        // 硬编码既不可读，catalog 一动就得改一遍。
        let resolved_source_target = MhwArmorCatalog
            .find_replacement_target(
                &ReplacementTargetId::parse("mhw:armor:guardian-alpha").expect("legacy target id"),
            )
            .expect("legacy slug must resolve after AR6 expansion");
        assert_eq!(
            source_binding.binding().target_id(),
            resolved_source_target.id()
        );
        assert_eq!(resolved_source_target.internal_id(), "pl121_0000");
        assert_eq!(source_binding.source_internal_id(), "pl121_0000");
        assert_eq!(source_binding.target_internal_id(), "pl121_0000");

        let (_, sealed_install) = BatchLifecycleAutomation::seal_request(
            &environment,
            armor_install_request(),
            install_preview
                .preview_token
                .as_deref()
                .expect("preview token"),
        )
        .expect("seal armor install");
        let (_, install_run) = BatchLifecycleAutomation::start_request(
            &environment,
            &sealed_install.batch_id,
            &sealed_install.plan_token,
        )
        .expect("run armor install");
        assert_eq!(install_run.status, BatchAttemptStatus::Completed);

        let manifest_path = sandbox.path().join("install/manifests/default.json");
        let installed_manifest: InstallManifest =
            serde_json::from_slice(&fs::read(&manifest_path).expect("read installed manifest"))
                .expect("parse installed manifest");
        assert_eq!(installed_manifest.replacement_bindings.len(), 1);
        // 落盘 manifest 同样记录规范 ID，与上面 binding snapshot 一致。
        assert_eq!(
            installed_manifest.replacement_bindings[0]
                .binding()
                .target_id(),
            resolved_source_target.id()
        );

        let switch_preview =
            BatchLifecycleAutomation::preview_request(&environment, armor_target_switch_request())
                .expect("same-revision target-switch preview");
        assert_eq!(
            switch_preview.plan.status(),
            hmm_core::BatchPlanStatus::Ready
        );
        let (_, sealed_switch) = BatchLifecycleAutomation::seal_request(
            &environment,
            armor_target_switch_request(),
            switch_preview
                .preview_token
                .as_deref()
                .expect("switch token"),
        )
        .expect("seal target switch");
        let (_, switch_run) = BatchLifecycleAutomation::start_request(
            &environment,
            &sealed_switch.batch_id,
            &sealed_switch.plan_token,
        )
        .expect("run target switch");
        assert_eq!(switch_run.status, BatchAttemptStatus::Completed);

        let source_path = game_root.join("nativePC/pl/f_equip/pl121_0000/arm/mod/f_body.mod3");
        let target_path = game_root.join("nativePC/pl/f_equip/pl129_0000/arm/mod/f_body.mod3");
        assert!(!source_path.exists());
        assert_eq!(
            fs::read(target_path).expect("read switched target"),
            b"synthetic armor fixture"
        );
        let switched_manifest: InstallManifest =
            serde_json::from_slice(&fs::read(manifest_path).expect("read switched manifest"))
                .expect("parse switched manifest");
        assert_eq!(switched_manifest.replacement_bindings.len(), 1);
        // 切换后的绑定同样记录规范 ID。
        let resolved_switch_target = MhwArmorCatalog
            .find_replacement_target(
                &ReplacementTargetId::parse("mhw:armor:fatalis-alpha").expect("legacy target id"),
            )
            .expect("legacy slug must resolve after AR6 expansion");
        assert_eq!(
            switched_manifest.replacement_bindings[0]
                .binding()
                .target_id(),
            resolved_switch_target.id()
        );
        assert_eq!(resolved_switch_target.internal_id(), "pl129_0000");
    }

    #[test]
    fn start_rejects_invalid_plan_token_before_any_game_write() {
        let sandbox = tempfile::tempdir().expect("sandbox");
        let game_root = write_install_fixture(sandbox.path());
        let game_before = fs::read(game_root.join("nativePC/models/player.mod3")).ok();
        let environment =
            RuntimeEnvironment::sandbox(sandbox.path().to_path_buf()).expect("environment");

        let preview =
            BatchLifecycleAutomation::preview_request(&environment, batch_install_request())
                .expect("sandbox preview");
        let preview_token = preview.preview_token.expect("ready preview token");
        let (_, sealed) = BatchLifecycleAutomation::seal_request(
            &environment,
            batch_install_request(),
            &preview_token,
        )
        .expect("sandbox seal");

        let error = BatchLifecycleAutomation::start_request(
            &environment,
            &sealed.batch_id,
            "forged-plan-token",
        )
        .expect_err("forged plan token must be rejected");
        assert_eq!(error.code(), "batch_token_invalid");

        let snapshot = BatchLifecycleAutomation::result(&environment, &sealed.batch_id, 0)
            .expect("attempt remains readable");
        assert_eq!(snapshot.status, BatchAttemptStatus::Sealed);
        assert_eq!(snapshot.task_id, None);
        assert_eq!(
            fs::read(game_root.join("nativePC/models/player.mod3")).ok(),
            game_before,
            "no game file may be written before token validation"
        );
    }

    #[test]
    fn production_batch_runs_end_to_end_with_keyed_tokens_in_a_test_root() {
        // CLI-3C：production batch 走 per-installation secret 签名的 token，
        // 在 temp 根上完成 preview -> seal -> start -> result 全链路。
        let data_root = tempfile::tempdir().expect("production app data root");
        write_install_fixture(data_root.path());
        // production 根不需要（也不该要求）sandbox marker。
        fs::remove_file(data_root.path().join(crate::SANDBOX_MARKER_FILE_NAME))
            .expect("remove sandbox marker");
        let environment = RuntimeEnvironment::production_with_app_data_root_for_tests(
            data_root.path().to_path_buf(),
        );

        let preview =
            BatchLifecycleAutomation::preview_request(&environment, batch_install_request())
                .expect("production preview");
        let preview_token = preview
            .preview_token
            .expect("ready production preview token");
        assert!(data_root
            .path()
            .join("secrets/batch-token-secret-v1")
            .is_file());

        let (operation, sealed) = BatchLifecycleAutomation::seal_request(
            &environment,
            batch_install_request(),
            &preview_token,
        )
        .expect("production seal");
        assert_eq!(operation, BatchOperation::Install);
        let (_, run) = BatchLifecycleAutomation::start_request(
            &environment,
            &sealed.batch_id,
            &sealed.plan_token,
        )
        .expect("production start");
        assert_eq!(run.status, BatchAttemptStatus::Completed);

        let snapshot = BatchLifecycleAutomation::result(&environment, &sealed.batch_id, 0)
            .expect("production result");
        assert_eq!(snapshot.status, BatchAttemptStatus::Completed);
        assert_eq!(snapshot.summary.succeeded_count, 1);
    }

    #[test]
    fn batch_tokens_do_not_cross_environments_and_forged_tokens_fail_before_io() {
        // 同一份数据布局分别以两种环境读取：sandbox token 不能在 production
        // seal，production token 也不能回流 sandbox；语法非法/过期 token 在
        // 触达数据根之前拒绝。
        let root = tempfile::tempdir().expect("shared root");
        write_install_fixture(root.path());
        let sandbox = RuntimeEnvironment::sandbox(root.path().to_path_buf()).expect("sandbox");
        let production =
            RuntimeEnvironment::production_with_app_data_root_for_tests(root.path().to_path_buf());

        let sandbox_token =
            BatchLifecycleAutomation::preview_request(&sandbox, batch_install_request())
                .expect("sandbox preview")
                .preview_token
                .expect("sandbox preview token");
        let production_token =
            BatchLifecycleAutomation::preview_request(&production, batch_install_request())
                .expect("production preview")
                .preview_token
                .expect("production preview token");
        assert_ne!(sandbox_token, production_token);

        let cross = BatchLifecycleAutomation::seal_request(
            &production,
            batch_install_request(),
            &sandbox_token,
        )
        .expect_err("sandbox token must not seal a production batch");
        assert_eq!(cross.code(), "batch_plan_stale");
        let cross_back = BatchLifecycleAutomation::seal_request(
            &sandbox,
            batch_install_request(),
            &production_token,
        )
        .expect_err("production token must not seal a sandbox batch");
        assert_eq!(cross_back.code(), "batch_plan_stale");

        // 纯语法预检：格式非法与已过期 token 不触达数据根。
        let malformed = BatchLifecycleAutomation::seal_request(
            &production,
            batch_install_request(),
            "forged-preview-token",
        )
        .expect_err("malformed token fails before io");
        assert_eq!(malformed.code(), "batch_token_invalid");
        let expired = BatchLifecycleAutomation::seal_request(
            &production,
            batch_install_request(),
            "hmm-batch-v2.preview.1.2.deadbeef",
        )
        .expect_err("expired token fails before io");
        assert_eq!(expired.code(), "batch_plan_expired");
    }

    #[test]
    fn production_secret_rotation_invalidates_previously_issued_tokens() {
        let data_root = tempfile::tempdir().expect("production app data root");
        write_install_fixture(data_root.path());
        fs::remove_file(data_root.path().join(crate::SANDBOX_MARKER_FILE_NAME))
            .expect("remove sandbox marker");
        let environment = RuntimeEnvironment::production_with_app_data_root_for_tests(
            data_root.path().to_path_buf(),
        );

        let preview_token =
            BatchLifecycleAutomation::preview_request(&environment, batch_install_request())
                .expect("production preview")
                .preview_token
                .expect("production preview token");

        fs::write(
            data_root.path().join("secrets/batch-token-secret-v1"),
            "corrupted",
        )
        .expect("corrupt secret to force rotation");

        let error = BatchLifecycleAutomation::seal_request(
            &environment,
            batch_install_request(),
            &preview_token,
        )
        .expect_err("token issued under the old secret must fail after rotation");
        assert_eq!(error.code(), "batch_plan_stale");
    }
}
