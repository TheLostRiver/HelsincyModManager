use crate::{
    lifecycle_automation::revalidate_sandbox_write_roots, HmmRuntime, ReadOnlyInstallAutomation,
    RuntimeEnvironment, RuntimeEnvironmentKind, SandboxWriteCapability,
};
use hmm_app::{
    BatchInstallItemExecutor, BatchInstallRetryError, BatchInstallRetryResult,
    BatchInstallRetryService, BatchInstallRunError, BatchInstallRunResult, BatchInstallTaskRunner,
    BatchPlanPreview, BatchPlanPreviewError, BatchPlanSealError, BatchPlanSealResult,
    BatchPlanService, BatchTokenCodec, InstallTaskBatchItemExecutor, InstallWriteAdmission,
    Sha256BatchTokenCodec,
};
use hmm_core::{
    BatchActionSummary, BatchAttempt, BatchAttemptStatus, BatchId, BatchItemFacts, BatchItemInput,
    BatchItemResult, BatchPlanFacts, BatchPlanRequest, BatchPreflightDecision,
    BatchPreflightStatus, BatchTargetClaim, BatchTargetWriteKind, InstallPlan,
    NormalizedBatchPlanRequest, SealedBatch,
};
use hmm_infra::{
    open_database_read_only, JsonGameConfigRepository, SqliteBatchLifecycleRepository, SystemClock,
};
use hmm_ports::{BatchLifecycleRepository, BatchPlanFactsProvider, BatchSealRepository};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

const BATCH_TOKEN_SECRET_PREFIX: &str = "hmm-sandbox-batch-token-v2";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SandboxBatchAutomationError {
    code: &'static str,
}

impl SandboxBatchAutomationError {
    pub const fn code(&self) -> &'static str {
        self.code
    }

    const fn new(code: &'static str) -> Self {
        Self { code }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchAttemptSnapshot {
    pub batch_id: String,
    pub attempt_number: u32,
    pub status: BatchAttemptStatus,
    pub task_id: Option<String>,
    pub evidence_health_degraded: bool,
    pub summary: hmm_core::BatchResultSummary,
    pub items: Vec<BatchItemResult>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AllowedBatchPlan {
    revision_id: hmm_core::ModRevisionId,
    digest: String,
}

struct SandboxBatchFactsProvider {
    environment: RuntimeEnvironment,
}

impl BatchPlanFactsProvider for SandboxBatchFactsProvider {
    fn read_batch_plan_facts(
        &self,
        request: &NormalizedBatchPlanRequest,
    ) -> anyhow::Result<BatchPlanFacts> {
        if request.operation != hmm_core::BatchOperation::Install {
            anyhow::bail!("batch operation is not install");
        }

        let read_only = ReadOnlyInstallAutomation::from_environment(&self.environment)
            .map_err(|error| anyhow::anyhow!(error.code()))?;
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
                )
                .map_err(|error| anyhow::anyhow!(error.code()))?;
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
            items,
        })
    }
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
    format!(
        "sha256:{}",
        digest
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    )
}

struct ReadOnlyBatchSealRepository;

impl BatchSealRepository for ReadOnlyBatchSealRepository {
    fn seal_batch(&self, _request: hmm_ports::BatchSealRequest<'_>) -> anyhow::Result<()> {
        anyhow::bail!("read-only batch plan cannot seal")
    }
}

struct SandboxBatchWriteAdmission {
    capability: Arc<SandboxWriteCapability>,
    sandbox_root: PathBuf,
    game_config_repository: Arc<dyn hmm_ports::GameConfigRepository>,
    expected: Mutex<Option<ExpectedBatchPlans>>,
}

type ExpectedBatchPlans = (
    hmm_core::GameId,
    hmm_core::ProfileId,
    BTreeMap<hmm_core::ModId, AllowedBatchPlan>,
);

impl SandboxBatchWriteAdmission {
    fn register_batch(&self, batch: &SealedBatch) {
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
        if let Ok(mut expected) = self.expected.lock() {
            *expected = Some((
                batch.plan.game_id.clone(),
                batch.plan.profile_id.clone(),
                allowed,
            ));
        }
    }
}

impl InstallWriteAdmission for SandboxBatchWriteAdmission {
    fn ensure_write_allowed(
        &self,
        game_id: &hmm_core::GameId,
        profile_id: &hmm_core::ProfileId,
    ) -> Result<(), hmm_app::InstallWriteAdmissionError> {
        let expected = self
            .expected
            .lock()
            .map_err(|_| hmm_app::InstallWriteAdmissionError::SafetyRejected)?;
        let Some((expected_game, expected_profile, _)) = expected.as_ref() else {
            return Err(hmm_app::InstallWriteAdmissionError::SafetyRejected);
        };
        revalidate_sandbox_write_roots(
            self.capability.as_ref(),
            &self.sandbox_root,
            self.game_config_repository.as_ref(),
            expected_game,
            expected_profile,
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
        let Some((expected_game, expected_profile, allowed)) = expected.as_ref() else {
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
        revalidate_sandbox_write_roots(
            self.capability.as_ref(),
            &self.sandbox_root,
            self.game_config_repository.as_ref(),
            expected_game,
            expected_profile,
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
    admission: Arc<SandboxBatchWriteAdmission>,
}

pub struct SandboxBatchInstallAutomation;

impl SandboxBatchInstallAutomation {
    pub fn preview(
        environment: &RuntimeEnvironment,
        request: BatchPlanRequest,
    ) -> Result<BatchPlanPreview, SandboxBatchAutomationError> {
        build_read_only_plan_service(environment)?
            .preview(request)
            .map_err(map_preview_error)
    }

    pub fn apply(
        environment: &RuntimeEnvironment,
        request: BatchPlanRequest,
        preview_token: &str,
    ) -> Result<(BatchPlanSealResult, BatchInstallRunResult), SandboxBatchAutomationError> {
        // Reject stale input before constructing HmmRuntime, whose initialization creates the
        // sandbox journal. The write path repeats this validation inside `seal` to close the
        // validation-to-persistence TOCTOU window.
        build_read_only_plan_service(environment)?
            .validate_preview(request.clone(), preview_token)
            .map_err(map_seal_error)?;
        let context = build_write_context(environment)?;
        let sealed = context
            .plan_service
            .seal(request, preview_token)
            .map_err(map_seal_error)?;
        let batch_id = BatchId::new(sealed.batch_id.clone());
        let batch = context
            .repository
            .load_batch(&batch_id)
            .map_err(|_| SandboxBatchAutomationError::new("batch_journal_unavailable"))?
            .ok_or_else(|| SandboxBatchAutomationError::new("batch_unavailable"))?;
        context.admission.register_batch(&batch);
        let run = context
            .runner
            .run(&batch_id, &sealed.plan_token)
            .map_err(map_run_error)?;
        Ok((sealed, run))
    }

    pub fn retry(
        environment: &RuntimeEnvironment,
        batch_id: &str,
        attempt_number: u32,
    ) -> Result<(BatchInstallRetryResult, BatchInstallRunResult), SandboxBatchAutomationError> {
        let context = build_write_context(environment)?;
        let batch_id = parse_batch_id(batch_id)?;
        let retry = context
            .retry
            .retry(&batch_id, attempt_number)
            .map_err(map_retry_error)?;
        let batch = context
            .repository
            .load_batch(&batch_id)
            .map_err(|_| SandboxBatchAutomationError::new("batch_journal_unavailable"))?
            .ok_or_else(|| SandboxBatchAutomationError::new("batch_unavailable"))?;
        context.admission.register_batch(&batch);
        let run = context
            .runner
            .run_attempt(&batch_id, retry.attempt_number, &retry.plan_token)
            .map_err(map_run_error)?;
        Ok((retry, run))
    }

    pub fn result(
        environment: &RuntimeEnvironment,
        batch_id: &str,
        attempt_number: u32,
    ) -> Result<BatchAttemptSnapshot, SandboxBatchAutomationError> {
        ensure_sandbox(environment)?;
        let root = environment
            .sandbox_data_dir()
            .ok_or_else(|| SandboxBatchAutomationError::new("sandbox_data_dir_required"))?;
        let connection = open_database_read_only(&root.join("hmm.db"))
            .map_err(|_| SandboxBatchAutomationError::new("batch_result_unavailable"))?;
        let repository: Arc<dyn BatchLifecycleRepository> = Arc::new(
            SqliteBatchLifecycleRepository::new(Arc::new(Mutex::new(connection))),
        );
        let batch_id = parse_batch_id(batch_id)?;
        let batch = repository
            .load_batch(&batch_id)
            .map_err(|_| SandboxBatchAutomationError::new("batch_result_unavailable"))?
            .ok_or_else(|| SandboxBatchAutomationError::new("batch_result_unavailable"))?;
        let attempt = repository
            .load_attempt(&batch_id, attempt_number)
            .map_err(|_| SandboxBatchAutomationError::new("batch_result_unavailable"))?
            .ok_or_else(|| SandboxBatchAutomationError::new("batch_result_unavailable"))?;
        let items = repository
            .list_item_results(&batch_id, attempt_number)
            .map_err(|_| SandboxBatchAutomationError::new("batch_result_unavailable"))?;
        Ok(snapshot(batch, attempt, items))
    }
}

fn build_read_only_plan_service(
    environment: &RuntimeEnvironment,
) -> Result<BatchPlanService, SandboxBatchAutomationError> {
    ensure_sandbox(environment)?;
    let facts = Arc::new(SandboxBatchFactsProvider {
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
) -> Result<WriteContext, SandboxBatchAutomationError> {
    ensure_sandbox(environment)?;
    let sandbox_root = environment
        .sandbox_data_dir()
        .ok_or_else(|| SandboxBatchAutomationError::new("sandbox_data_dir_required"))?
        .to_path_buf();
    let capability = Arc::new(
        environment
            .acquire_sandbox_write_capability()
            .map_err(|_| SandboxBatchAutomationError::new("batch_write_admission_unavailable"))?,
    );
    let game_config_repository: Arc<dyn hmm_ports::GameConfigRepository> = Arc::new(
        JsonGameConfigRepository::new(sandbox_root.join("config").join("games.json")),
    );
    let admission = Arc::new(SandboxBatchWriteAdmission {
        capability,
        sandbox_root: sandbox_root.clone(),
        game_config_repository,
        expected: Mutex::new(None),
    });
    let runtime = HmmRuntime::builder(sandbox_root.clone())
        .with_sandbox_write_admission(admission.clone())
        .build()
        .map_err(|_| SandboxBatchAutomationError::new("batch_runtime_unavailable"))?;
    let repository_impl = Arc::new(SqliteBatchLifecycleRepository::new(
        runtime.database_handle(),
    ));
    let repository: Arc<dyn BatchLifecycleRepository> = repository_impl.clone();
    let seal_repository: Arc<dyn BatchSealRepository> = repository_impl;
    let facts = Arc::new(SandboxBatchFactsProvider {
        environment: environment.clone(),
    });
    let token_codec = batch_token_codec(environment)?;
    let plan_service = BatchPlanService::new(
        facts.clone(),
        seal_repository,
        Arc::new(SystemClock),
        Arc::clone(&token_codec),
    );
    let executor: Arc<dyn BatchInstallItemExecutor> = Arc::new(InstallTaskBatchItemExecutor::new(
        Arc::clone(&runtime.install_task_runner),
        Arc::clone(&runtime.task_manager),
    ));
    let runner = BatchInstallTaskRunner::new(
        Arc::clone(&runtime.task_manager),
        Arc::clone(&repository),
        executor,
        facts,
        runtime.audit_log_writer(),
        Arc::new(SystemClock),
        Arc::clone(&token_codec),
    );
    let retry =
        BatchInstallRetryService::new(Arc::clone(&repository), Arc::new(SystemClock), token_codec);
    Ok(WriteContext {
        repository,
        plan_service,
        runner,
        retry,
        admission,
    })
}

fn batch_token_codec(
    environment: &RuntimeEnvironment,
) -> Result<Arc<dyn BatchTokenCodec>, SandboxBatchAutomationError> {
    let root = environment
        .sandbox_data_dir()
        .ok_or_else(|| SandboxBatchAutomationError::new("sandbox_data_dir_required"))?;
    let secret = format!("{BATCH_TOKEN_SECRET_PREFIX}\0{}", root.display());
    let codec = Sha256BatchTokenCodec::new(secret)
        .map_err(|_| SandboxBatchAutomationError::new("batch_token_unavailable"))?;
    Ok(Arc::new(codec))
}

fn ensure_sandbox(environment: &RuntimeEnvironment) -> Result<(), SandboxBatchAutomationError> {
    if environment.kind() == RuntimeEnvironmentKind::Sandbox {
        Ok(())
    } else {
        Err(SandboxBatchAutomationError::new(
            "sandbox_batch_production_forbidden",
        ))
    }
}

fn parse_batch_id(value: &str) -> Result<BatchId, SandboxBatchAutomationError> {
    let value = value.trim();
    if value.len() > 128
        || value.is_empty()
        || !value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
    {
        return Err(SandboxBatchAutomationError::new("batch_id_invalid"));
    }
    Ok(BatchId::new(value))
}

fn snapshot(
    batch: SealedBatch,
    attempt: BatchAttempt,
    items: Vec<BatchItemResult>,
) -> BatchAttemptSnapshot {
    BatchAttemptSnapshot {
        batch_id: batch.batch_id.as_str().to_owned(),
        attempt_number: attempt.attempt_number,
        status: attempt.status,
        task_id: attempt.task_id,
        evidence_health_degraded: attempt.evidence_health_degraded,
        summary: hmm_core::BatchResultSummary::from_item_results(attempt.item_ids.len(), &items),
        items,
    }
}

fn map_preview_error(error: BatchPlanPreviewError) -> SandboxBatchAutomationError {
    SandboxBatchAutomationError::new(error.code())
}

fn map_seal_error(error: BatchPlanSealError) -> SandboxBatchAutomationError {
    SandboxBatchAutomationError::new(error.code())
}

fn map_run_error(error: BatchInstallRunError) -> SandboxBatchAutomationError {
    let code = match error {
        BatchInstallRunError::BatchUnavailable => "batch_unavailable",
        BatchInstallRunError::InvalidToken => "batch_token_invalid",
        BatchInstallRunError::PlanBlocked => "batch_plan_blocked",
        BatchInstallRunError::OperationMismatch => "batch_operation_mismatch",
        BatchInstallRunError::AdmissionRejected => "batch_admission_rejected",
        BatchInstallRunError::JournalUnavailable => "batch_journal_unavailable",
        BatchInstallRunError::TaskUnavailable => "batch_task_unavailable",
    };
    SandboxBatchAutomationError::new(code)
}

fn map_retry_error(error: BatchInstallRetryError) -> SandboxBatchAutomationError {
    let code = match error {
        BatchInstallRetryError::BatchUnavailable => "batch_unavailable",
        BatchInstallRetryError::RetryUnavailable => "batch_retry_unavailable",
        BatchInstallRetryError::AttemptStale => "batch_attempt_stale",
        BatchInstallRetryError::ClockUnavailable => "batch_internal_error",
        BatchInstallRetryError::TokenIssueFailed => "batch_internal_error",
        BatchInstallRetryError::JournalUnavailable => "batch_journal_unavailable",
    };
    SandboxBatchAutomationError::new(code)
}
