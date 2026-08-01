use super::*;
use hmm_core::{
    BatchActionSummary, BatchItemInput, BatchOperation, BatchPlanRequest, BatchPreflightStatus,
    BatchResourceLimits, BatchTargetClaim, BatchTargetWriteKind, FileLayer, GameId,
    InstallBatchItemInput, InstallFileProvider, InstallManifest, InstallPlan,
    InstallRecoveryRecord, InstallRecoveryRecordStatus, InstallTargetPath, ModId, ModRevisionId,
    PackageFileId, ProfileId, SealedBatch,
};
use hmm_ports::{
    BatchAttemptAdmission, BatchPlanFactsProvider, BatchRetryAttemptCreation,
    BatchRetryAttemptRequest, BatchSealRepository, BatchSealRequest, InstallBackupStore,
    InstallGameFileSystem, InstallManifestRepository, InstallRecoveryRecordRepository,
    InstallSourceFileReader,
};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Mutex;

#[derive(Default)]
struct FakeRepository {
    batch: Mutex<Option<SealedBatch>>,
    attempts: Mutex<HashMap<u32, hmm_core::BatchAttempt>>,
    results: Mutex<HashMap<u32, Vec<BatchItemResult>>>,
    admission_task_ids: Mutex<Vec<String>>,
    fail_admission: AtomicBool,
    scope_active_admission: AtomicBool,
    fail_record_item_result: AtomicBool,
    fail_record_item_result_on_call: AtomicUsize,
    record_item_result_calls: AtomicUsize,
    fail_finish_attempt: AtomicBool,
    finish_attempt_failures_remaining: AtomicUsize,
}

impl BatchSealRepository for FakeRepository {
    fn seal_batch(&self, request: BatchSealRequest<'_>) -> anyhow::Result<()> {
        *self.batch.lock().expect("batch") = Some(request.sealed_batch.clone());
        self.attempts.lock().expect("attempts").insert(
            request.initial_attempt.attempt_number,
            request.initial_attempt.clone(),
        );
        Ok(())
    }
}

impl BatchLifecycleRepository for FakeRepository {
    fn load_batch(&self, _batch_id: &BatchId) -> anyhow::Result<Option<SealedBatch>> {
        Ok(self.batch.lock().expect("batch").clone())
    }

    fn load_attempt(
        &self,
        _batch_id: &BatchId,
        attempt_number: u32,
    ) -> anyhow::Result<Option<hmm_core::BatchAttempt>> {
        Ok(self
            .attempts
            .lock()
            .expect("attempts")
            .get(&attempt_number)
            .cloned())
    }

    fn find_active_attempt_for_scope(
        &self,
        game_id: &GameId,
        profile_id: &ProfileId,
    ) -> anyhow::Result<Option<hmm_core::BatchAttempt>> {
        let in_scope = self
            .batch
            .lock()
            .expect("batch")
            .as_ref()
            .is_some_and(|batch| {
                batch.plan.game_id == *game_id && batch.plan.profile_id == *profile_id
            });
        if !in_scope {
            return Ok(None);
        }
        Ok(self
            .attempts
            .lock()
            .expect("attempts")
            .values()
            .find(|attempt| {
                matches!(
                    attempt.status,
                    BatchAttemptStatus::Queued
                        | BatchAttemptStatus::Running
                        | BatchAttemptStatus::Stopping
                )
            })
            .cloned())
    }

    fn admit_attempt(
        &self,
        request: BatchAttemptAdmissionRequest<'_>,
    ) -> anyhow::Result<BatchAttemptAdmission> {
        self.admission_task_ids
            .lock()
            .expect("admission task ids")
            .push(request.task_id.to_owned());
        if self.fail_admission.load(Ordering::Relaxed) {
            anyhow::bail!("injected admission failure");
        }
        let mut attempts = self.attempts.lock().expect("attempts");
        let current = attempts.get_mut(&request.attempt_number).expect("attempt");
        if current.plan_token_verifier != request.presented_plan_token_verifier {
            return Ok(BatchAttemptAdmission::Rejected);
        }
        if current.status != BatchAttemptStatus::Sealed {
            return Ok(BatchAttemptAdmission::AlreadyAdmitted(current.clone()));
        }
        if self.scope_active_admission.load(Ordering::Relaxed) {
            return Ok(BatchAttemptAdmission::ScopeActive);
        }
        current.status = BatchAttemptStatus::Queued;
        current.task_id = Some(request.task_id.to_owned());
        Ok(BatchAttemptAdmission::Admitted(current.clone()))
    }

    fn discard_unadmitted_retry_attempt(
        &self,
        _batch_id: &BatchId,
        attempt_number: u32,
        presented_plan_token_verifier: &str,
    ) -> anyhow::Result<bool> {
        if attempt_number == 0 {
            return Ok(false);
        }
        let mut attempts = self.attempts.lock().expect("attempts");
        let Some(attempt) = attempts.get(&attempt_number) else {
            return Ok(false);
        };
        let has_results = self
            .results
            .lock()
            .expect("results")
            .get(&attempt_number)
            .is_some_and(|results| !results.is_empty());
        let latest = attempts.keys().max().copied();
        if attempt.status != BatchAttemptStatus::Sealed
            || attempt.task_id.is_some()
            || attempt.plan_token_verifier != presented_plan_token_verifier
            || has_results
            || latest != Some(attempt_number)
        {
            return Ok(false);
        }
        attempts.remove(&attempt_number);
        Ok(true)
    }

    fn mark_attempt_running(
        &self,
        _batch_id: &BatchId,
        attempt_number: u32,
        now: u128,
    ) -> anyhow::Result<hmm_core::BatchAttempt> {
        let mut attempts = self.attempts.lock().expect("attempts");
        let current = attempts.get_mut(&attempt_number).expect("attempt");
        current.status = BatchAttemptStatus::Running;
        current.started_at_unix_millis = Some(now);
        Ok(current.clone())
    }

    fn mark_item_running(
        &self,
        _batch_id: &BatchId,
        _attempt_number: u32,
        _item_id: &hmm_core::BatchItemId,
    ) -> anyhow::Result<()> {
        Ok(())
    }

    fn record_item_result(&self, result: &BatchItemResult) -> anyhow::Result<()> {
        let call_number = self
            .record_item_result_calls
            .fetch_add(1, Ordering::Relaxed)
            .saturating_add(1);
        if self.fail_record_item_result.load(Ordering::Relaxed)
            || self.fail_record_item_result_on_call.load(Ordering::Relaxed) == call_number
        {
            anyhow::bail!("injected item result failure");
        }
        self.results
            .lock()
            .expect("results")
            .entry(result.attempt_number)
            .or_default()
            .push(result.clone());
        Ok(())
    }

    fn list_item_results(
        &self,
        _batch_id: &BatchId,
        attempt_number: u32,
    ) -> anyhow::Result<Vec<BatchItemResult>> {
        Ok(self
            .results
            .lock()
            .expect("results")
            .get(&attempt_number)
            .cloned()
            .unwrap_or_default())
    }

    fn finish_attempt(
        &self,
        _batch_id: &BatchId,
        attempt_number: u32,
        status: BatchAttemptStatus,
        degraded: bool,
        completed: u128,
    ) -> anyhow::Result<hmm_core::BatchAttempt> {
        if self.fail_finish_attempt.load(Ordering::Relaxed)
            || self
                .finish_attempt_failures_remaining
                .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |remaining| {
                    remaining.checked_sub(1)
                })
                .is_ok()
        {
            anyhow::bail!("injected finish attempt failure");
        }
        let mut attempts = self.attempts.lock().expect("attempts");
        let current = attempts.get_mut(&attempt_number).expect("attempt");
        current.status = status;
        current.evidence_health_degraded = degraded;
        current.completed_at_unix_millis = Some(completed);
        Ok(current.clone())
    }

    fn create_retry_attempt(
        &self,
        request: BatchRetryAttemptRequest<'_>,
    ) -> anyhow::Result<BatchRetryAttemptCreation> {
        let mut attempts = self.attempts.lock().expect("attempts");
        let Some(current) = attempts.get(&request.expected_attempt_number) else {
            return Ok(BatchRetryAttemptCreation::Stale);
        };
        if !current.status.is_terminal() {
            return Ok(BatchRetryAttemptCreation::Stale);
        }
        attempts.insert(
            request.retry_attempt.attempt_number,
            request.retry_attempt.clone(),
        );
        Ok(BatchRetryAttemptCreation::Created(
            request.retry_attempt.clone(),
        ))
    }
}

struct FixedClock;
impl AppClock for FixedClock {
    fn now_unix_millis(&self) -> anyhow::Result<u128> {
        Ok(10)
    }
}

struct ExpiredClock;
impl AppClock for ExpiredClock {
    fn now_unix_millis(&self) -> anyhow::Result<u128> {
        Ok(100)
    }
}

#[derive(Default)]
struct FailingCompletionClock {
    calls: std::sync::atomic::AtomicUsize,
}

impl AppClock for FailingCompletionClock {
    fn now_unix_millis(&self) -> anyhow::Result<u128> {
        if self
            .calls
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            == 0
        {
            Ok(10)
        } else {
            anyhow::bail!("completion clock unavailable")
        }
    }
}

#[derive(Default)]
struct RecordingAuditLogWriter {
    events: Mutex<Vec<AuditLogEvent>>,
    policies: Mutex<Vec<AuditWriteFailurePolicy>>,
    fail: bool,
}

impl RecordingAuditLogWriter {
    fn failing() -> Self {
        Self {
            events: Mutex::new(Vec::new()),
            policies: Mutex::new(Vec::new()),
            fail: true,
        }
    }

    fn take_events(&self) -> Vec<AuditLogEvent> {
        std::mem::take(&mut *self.events.lock().expect("audit events"))
    }
}

impl AuditLogWriter for RecordingAuditLogWriter {
    fn record(&self, event: AuditLogEvent) -> anyhow::Result<()> {
        if self.fail {
            anyhow::bail!("audit unavailable");
        }
        self.events.lock().expect("audit events").push(event);
        Ok(())
    }

    fn record_with_policy(
        &self,
        event: AuditLogEvent,
        policy: AuditWriteFailurePolicy,
    ) -> anyhow::Result<()> {
        self.policies.lock().expect("audit policies").push(policy);
        self.record(event)
    }
}

struct StaticFacts(hmm_core::BatchPlanFacts);

impl BatchPlanFactsProvider for StaticFacts {
    fn read_batch_plan_facts(
        &self,
        _request: &hmm_core::NormalizedBatchPlanRequest,
    ) -> anyhow::Result<hmm_core::BatchPlanFacts> {
        Ok(self.0.clone())
    }
}

struct UnavailableFacts;

impl BatchPlanFactsProvider for UnavailableFacts {
    fn read_batch_plan_facts(
        &self,
        _request: &hmm_core::NormalizedBatchPlanRequest,
    ) -> anyhow::Result<hmm_core::BatchPlanFacts> {
        anyhow::bail!("injected facts outage")
    }
}

struct FakeExecutor {
    executions: Mutex<Vec<BatchInstallItemExecution>>,
}
impl BatchInstallItemExecutor for FakeExecutor {
    fn execute(&self, _request: BatchInstallItemRequest) -> BatchInstallItemExecution {
        self.executions.lock().expect("executions").remove(0)
    }
}

struct CommitCancellationExecutor {
    task_manager: Arc<TaskManager>,
    calls: AtomicUsize,
}

impl BatchInstallItemExecutor for CommitCancellationExecutor {
    fn execute(&self, request: BatchInstallItemRequest) -> BatchInstallItemExecution {
        self.calls.fetch_add(1, Ordering::Relaxed);
        let child = self
            .task_manager
            .create_task(TaskKind::Install)
            .expect("child task");
        self.task_manager
            .start_task(&child.task_id)
            .expect("child starts");
        self.task_manager
            .block_tasks_cancellation(&[&request.parent_task_id, &child.task_id])
            .expect("commit barrier");
        assert!(
            self.task_manager
                .cancel_task(&request.parent_task_id)
                .is_err(),
            "commit cancellation is deferred rather than accepted immediately"
        );
        self.task_manager
            .complete_task(&child.task_id)
            .expect("committed child completes");
        BatchInstallItemExecution::Succeeded {
            evidence_health_degraded: false,
        }
    }
}

struct StaticInstallPlanner {
    plan: InstallPlan,
}

impl crate::ImportedModInstallPlanner for StaticInstallPlanner {
    fn build_imported_mod_install_plan(
        &self,
        _request: crate::BuildImportedModInstallPlanRequest,
    ) -> Result<crate::ImportedModInstallPreflight, crate::InstallPlanningError> {
        panic!("batch install must not resolve the mutable display revision")
    }

    fn build_imported_mod_revision_install_plan(
        &self,
        _game_id: &GameId,
        mod_id: &ModId,
        revision_id: &ModRevisionId,
        _layer: &FileLayer,
    ) -> Result<crate::ImportedModInstallPreflight, crate::InstallPlanningError> {
        assert_eq!(mod_id, &ModId::new("a"));
        assert_eq!(revision_id, &ModRevisionId::new("a"));
        Ok(crate::ImportedModInstallPreflight {
            plan: self.plan.clone(),
            prerequisite_decision: ready_install_prerequisite(),
        })
    }

    fn prerequisite_decision(&self, _game_id: &GameId) -> crate::GamePrerequisiteDecision {
        ready_install_prerequisite()
    }
}

struct FailingInstallPlanner;

impl crate::ImportedModInstallPlanner for FailingInstallPlanner {
    fn build_imported_mod_install_plan(
        &self,
        request: crate::BuildImportedModInstallPlanRequest,
    ) -> Result<crate::ImportedModInstallPreflight, crate::InstallPlanningError> {
        Err(crate::InstallPlanningError::ImportedModNotFound {
            mod_id: request.mod_id,
        })
    }

    fn build_imported_mod_revision_install_plan(
        &self,
        _game_id: &GameId,
        mod_id: &ModId,
        _revision_id: &ModRevisionId,
        _layer: &FileLayer,
    ) -> Result<crate::ImportedModInstallPreflight, crate::InstallPlanningError> {
        Err(crate::InstallPlanningError::ImportedModNotFound {
            mod_id: mod_id.clone(),
        })
    }

    fn prerequisite_decision(&self, _game_id: &GameId) -> crate::GamePrerequisiteDecision {
        ready_install_prerequisite()
    }
}

struct UnreachableInstallCommitter;

impl crate::InstallPlanCommitter for UnreachableInstallCommitter {
    fn commit_install_plan(
        &self,
        _request: crate::ImportedModInstallCommitRequest,
    ) -> Result<crate::InstallCommitResult, crate::InstallCommitError> {
        panic!("cancelled planning failure must not reach commit")
    }
}

fn ready_install_prerequisite() -> crate::GamePrerequisiteDecision {
    crate::GamePrerequisiteDecision {
        game_id: GameId::mhw(),
        status: crate::GamePrerequisiteDecisionStatus::Ready,
        rules_version: Some(1),
        codes: Vec::new(),
    }
}

struct StaticInstallSource;

impl InstallSourceFileReader for StaticInstallSource {
    fn read_source_file(&self, _package_file_id: &PackageFileId) -> anyhow::Result<Vec<u8>> {
        Ok(b"new item bytes".to_vec())
    }
}

#[derive(Default)]
struct TransactionGameFiles {
    files: Mutex<HashMap<String, Vec<u8>>>,
    fail_remove: bool,
}

impl TransactionGameFiles {
    fn with_failing_remove() -> Self {
        Self {
            files: Mutex::new(HashMap::new()),
            fail_remove: true,
        }
    }

    fn file_bytes(&self, target: &str) -> Option<Vec<u8>> {
        self.files.lock().expect("game files").get(target).cloned()
    }
}

impl InstallGameFileSystem for TransactionGameFiles {
    fn read_game_file(&self, target_path: &InstallTargetPath) -> anyhow::Result<Option<Vec<u8>>> {
        Ok(self.file_bytes(target_path.as_str()))
    }

    fn write_game_file(&self, target_path: &InstallTargetPath, bytes: &[u8]) -> anyhow::Result<()> {
        self.files
            .lock()
            .expect("game files")
            .insert(target_path.as_str().to_owned(), bytes.to_vec());
        Ok(())
    }

    fn remove_game_file(&self, target_path: &InstallTargetPath) -> anyhow::Result<()> {
        if self.fail_remove {
            anyhow::bail!("injected rollback remove failure");
        }
        self.files
            .lock()
            .expect("game files")
            .remove(target_path.as_str());
        Ok(())
    }
}

struct TransactionBackupStore;

impl InstallBackupStore for TransactionBackupStore {
    fn store_backup(
        &self,
        target_path: &InstallTargetPath,
        _bytes: &[u8],
    ) -> anyhow::Result<String> {
        Ok(format!("backup-{}", short_audit_id(target_path.as_str())))
    }

    fn read_backup(&self, _backup_ref: &str) -> anyhow::Result<Option<Vec<u8>>> {
        Ok(None)
    }

    fn remove_backup(&self, _backup_ref: &str) -> anyhow::Result<()> {
        Ok(())
    }
}

struct TransactionManifestRepository {
    saved: Mutex<Option<InstallManifest>>,
    fail_save: bool,
}

impl TransactionManifestRepository {
    fn new(fail_save: bool) -> Self {
        Self {
            saved: Mutex::new(None),
            fail_save,
        }
    }

    fn saved_manifest(&self) -> Option<InstallManifest> {
        self.saved.lock().expect("manifest").clone()
    }
}

impl InstallManifestRepository for TransactionManifestRepository {
    fn load_manifest(&self, _profile_id: &ProfileId) -> anyhow::Result<Option<InstallManifest>> {
        Ok(self.saved.lock().expect("manifest").clone())
    }

    fn save_manifest(&self, manifest: &InstallManifest) -> anyhow::Result<()> {
        if self.fail_save {
            anyhow::bail!("injected manifest save failure");
        }
        *self.saved.lock().expect("manifest") = Some(manifest.clone());
        Ok(())
    }
}

#[derive(Default)]
struct TransactionRecoveryRepository {
    records: Mutex<HashMap<(String, String), InstallRecoveryRecord>>,
    history: Mutex<Vec<InstallRecoveryRecord>>,
}

impl TransactionRecoveryRepository {
    fn history(&self) -> Vec<InstallRecoveryRecord> {
        self.history.lock().expect("recovery history").clone()
    }
}

impl InstallRecoveryRecordRepository for TransactionRecoveryRepository {
    fn load_record(
        &self,
        profile_id: &ProfileId,
        mod_id: &ModId,
    ) -> anyhow::Result<Option<InstallRecoveryRecord>> {
        Ok(self
            .records
            .lock()
            .expect("recovery records")
            .get(&(profile_id.as_str().to_owned(), mod_id.as_str().to_owned()))
            .cloned())
    }

    fn list_records(&self, profile_id: &ProfileId) -> anyhow::Result<Vec<InstallRecoveryRecord>> {
        Ok(self
            .records
            .lock()
            .expect("recovery records")
            .values()
            .filter(|record| &record.profile_id == profile_id)
            .cloned()
            .collect())
    }

    fn save_record(&self, record: &InstallRecoveryRecord) -> anyhow::Result<()> {
        self.records.lock().expect("recovery records").insert(
            (
                record.profile_id.as_str().to_owned(),
                record.mod_id.as_str().to_owned(),
            ),
            record.clone(),
        );
        self.history
            .lock()
            .expect("recovery history")
            .push(record.clone());
        Ok(())
    }

    fn remove_record(&self, profile_id: &ProfileId, mod_id: &ModId) -> anyhow::Result<()> {
        self.records
            .lock()
            .expect("recovery records")
            .remove(&(profile_id.as_str().to_owned(), mod_id.as_str().to_owned()));
        Ok(())
    }
}

fn transaction_item_executor(
    task_manager: Arc<TaskManager>,
    fail_manifest_save: bool,
    fail_rollback_remove: bool,
    item_audit: Arc<dyn AuditLogWriter>,
) -> (
    Arc<InstallTaskBatchItemExecutor>,
    Arc<TransactionGameFiles>,
    Arc<TransactionRecoveryRepository>,
    Arc<TransactionManifestRepository>,
) {
    let target = InstallTargetPath::parse("nativepc/a", ["nativepc"]).expect("transaction target");
    let plan = InstallPlan::from_providers(vec![InstallFileProvider::new(
        ModId::new("a"),
        PackageFileId::new("source-a"),
        target,
        FileLayer::new("default", 1),
    )]);
    let game_files = if fail_rollback_remove {
        Arc::new(TransactionGameFiles::with_failing_remove())
    } else {
        Arc::new(TransactionGameFiles::default())
    };
    let recovery = Arc::new(TransactionRecoveryRepository::default());
    let manifests = Arc::new(TransactionManifestRepository::new(fail_manifest_save));
    let commit_service = crate::InstallCommitService::new_with_recovery_records(
        Arc::new(StaticInstallSource),
        game_files.clone(),
        Arc::new(TransactionBackupStore),
        manifests.clone(),
        recovery.clone(),
    );
    let runner = Arc::new(InstallTaskRunner::new(
        task_manager.clone(),
        Arc::new(StaticInstallPlanner { plan }),
        Arc::new(commit_service),
        item_audit,
        Arc::new(FixedClock),
    ));
    (
        Arc::new(InstallTaskBatchItemExecutor::new(runner, task_manager)),
        game_files,
        recovery,
        manifests,
    )
}

fn first_item_request(batch: &SealedBatch, parent_task_id: String) -> BatchInstallItemRequest {
    BatchInstallItemRequest {
        batch_id: batch.batch_id.clone(),
        attempt_number: 0,
        item: batch.items[0].clone(),
        plan: batch.plan.clone(),
        parent_task_id,
    }
}

fn start_parent_task(task_manager: &TaskManager) -> String {
    let parent = task_manager
        .create_task(TaskKind::Install)
        .expect("parent task");
    task_manager
        .start_task(&parent.task_id)
        .expect("parent starts");
    parent.task_id
}

fn batch() -> (SealedBatch, hmm_core::BatchAttempt, String) {
    let request = BatchPlanRequest {
        schema_version: hmm_core::BATCH_PLAN_SCHEMA_VERSION,
        operation: BatchOperation::Install,
        game_id: GameId::mhw(),
        profile_id: ProfileId::new("default"),
        execution_policy: BatchExecutionPolicy::StopOnFailure,
        items: vec![
            BatchItemInput::Install(InstallBatchItemInput {
                mod_id: ModId::new("a"),
                revision_id: ModRevisionId::new("a"),
                layer: FileLayer::new("default", 1),
                replacement_binding_snapshot: None,
            }),
            BatchItemInput::Install(InstallBatchItemInput {
                mod_id: ModId::new("b"),
                revision_id: ModRevisionId::new("b"),
                layer: FileLayer::new("default", 1),
                replacement_binding_snapshot: None,
            }),
        ],
    }
    .normalize()
    .expect("request");
    let items = request
        .items
        .iter()
        .enumerate()
        .map(|(ordinal, item)| hmm_core::BatchItemPlan {
            ordinal,
            input_snapshot: item.clone(),
            source_revision_id: Some(ModRevisionId::new(item.mod_id().as_str())),
            installed_revision_id: None,
            fact_digest: "fact".to_owned(),
            single_plan_digest: "plan".to_owned(),
            prerequisite: hmm_core::BatchPreflightDecision {
                status: BatchPreflightStatus::Ready,
                rules_version: Some(1),
                codes: Vec::new(),
            },
            target_claims: vec![BatchTargetClaim {
                target_path: InstallTargetPath::parse(
                    format!("nativepc/{}", item.mod_id().as_str()),
                    ["nativepc"],
                )
                .expect("target"),
                kind: BatchTargetWriteKind::Install,
            }],
            action_summary: BatchActionSummary {
                actions: 1,
                ..Default::default()
            },
            blocking_reasons: Vec::new(),
            warning_codes: Vec::new(),
        })
        .collect::<Vec<_>>();
    let plan = BatchPlan {
        plan_schema_version: 1,
        operation: request.operation,
        game_id: request.game_id.clone(),
        profile_id: request.profile_id.clone(),
        execution_policy: request.execution_policy,
        items,
        environment_digest: "env".to_owned(),
        prerequisite_rules_version: Some(1),
        resource_limits: BatchResourceLimits::default(),
        resource_usage: hmm_core::BatchResourceUsage {
            item_count: 2,
            target_action_count: 2,
            canonical_bytes: 1,
        },
        global_target_claims_digest: "claims".to_owned(),
        batch_digest: "digest".to_owned(),
        global_blocking_reasons: Vec::new(),
        warning_codes: Vec::new(),
    };
    let batch_id = BatchId::new("batch-a");
    let sealed = SealedBatch {
        batch_id: batch_id.clone(),
        request,
        plan,
        items: vec![
            SealedBatchItem {
                item_id: hmm_core::BatchItemId::new("item-a"),
                ordinal: 0,
                mod_id: ModId::new("a"),
            },
            SealedBatchItem {
                item_id: hmm_core::BatchItemId::new("item-b"),
                ordinal: 1,
                mod_id: ModId::new("b"),
            },
        ],
        created_at_unix_millis: 1,
    };
    let mut attempt = hmm_core::BatchAttempt {
        batch_id: batch_id.clone(),
        attempt_number: 0,
        item_ids: sealed
            .items
            .iter()
            .map(|item| item.item_id.clone())
            .collect(),
        status: BatchAttemptStatus::Sealed,
        task_id: None,
        plan_token_verifier: String::new(),
        expires_at_unix_millis: 100,
        started_at_unix_millis: None,
        completed_at_unix_millis: None,
        evidence_health_degraded: false,
    };
    let codec = crate::Sha256BatchTokenCodec::new("secret").expect("codec");
    let token = codec
        .issue(
            crate::BatchTokenKind::Plan,
            &execution_token_digest(&batch_id, 0, &attempt.item_ids, "digest", "env"),
            "env",
            1,
            100,
        )
        .expect("token")
        .token;
    attempt.plan_token_verifier = sha256_hex(token.as_bytes());
    (sealed, attempt, token)
}

fn facts_from_batch(batch: &SealedBatch) -> hmm_core::BatchPlanFacts {
    hmm_core::BatchPlanFacts {
        environment_digest: batch.plan.environment_digest.clone(),
        prerequisite_rules_version: batch.plan.prerequisite_rules_version,
        items: batch
            .plan
            .items
            .iter()
            .map(|item| hmm_core::BatchItemFacts {
                mod_id: item.input_snapshot.mod_id().clone(),
                source_revision_id: item.source_revision_id.clone(),
                installed_revision_id: item.installed_revision_id.clone(),
                fact_digest: item.fact_digest.clone(),
                single_plan_digest: item.single_plan_digest.clone(),
                target_claims: item.target_claims.clone(),
                action_summary: item.action_summary.clone(),
                prerequisite: item.prerequisite.clone(),
                blocking_reasons: item.blocking_reasons.clone(),
                warning_codes: item.warning_codes.clone(),
            })
            .collect(),
    }
}

fn replacement_snapshot(mod_id: &str) -> hmm_core::ReplacementBindingSnapshot {
    hmm_core::ReplacementBindingSnapshot::new(
        hmm_core::ReplacementBinding::new(
            hmm_core::ReplacementBindingId::parse("binding-a").expect("binding id"),
            ModId::new(mod_id),
            ProfileId::new("default"),
            hmm_core::ReplacementSourceId::parse("mhw:armor:f_equip:pl121_0000")
                .expect("source id"),
            hmm_core::ReplacementTargetId::parse("mhw:armor:fatalis-alpha").expect("target id"),
            1,
        )
        .expect("binding"),
        Some(ModRevisionId::new(mod_id)),
        "pl121_0000",
        "pl129_0000",
        "pl/f_equip",
        "pl/f_equip",
        hmm_core::ReplacementTargetKind::parse("armor").expect("target kind"),
    )
    .expect("replacement snapshot")
}

#[test]
fn stop_on_failure_preserves_success_and_skips_following_item() {
    let (batch, attempt, token) = batch();
    let repository = Arc::new(FakeRepository::default());
    let audit = Arc::new(RecordingAuditLogWriter::default());
    repository
        .seal_batch(BatchSealRequest {
            sealed_batch: &batch,
            initial_attempt: &attempt,
        })
        .expect("seal");
    let executor = Arc::new(FakeExecutor {
        executions: Mutex::new(vec![BatchInstallItemExecution::Failed {
            reason_code: "commit_failed".to_owned(),
            retryable: true,
            evidence_health_degraded: false,
        }]),
    });
    let runner = BatchInstallTaskRunner::new(
        Arc::new(TaskManager::new()),
        repository.clone(),
        executor,
        Arc::new(StaticFacts(facts_from_batch(&batch))),
        audit.clone(),
        Arc::new(FixedClock),
        Arc::new(crate::Sha256BatchTokenCodec::new("secret").expect("codec")),
    );
    let result = runner.run(&batch.batch_id, &token).expect("run");
    assert_eq!(result.status, BatchAttemptStatus::CompletedWithErrors);
    assert_eq!(result.summary.failed_count, 1);
    assert_eq!(result.summary.skipped_count, 1);
    let results = repository
        .list_item_results(&batch.batch_id, 0)
        .expect("results");
    assert_eq!(results[0].status, BatchItemStatus::Failed);
    assert_eq!(results[1].status, BatchItemStatus::Skipped);
    let event = audit.take_events().pop().expect("batch audit");
    assert_eq!(event.operation, "batch_install");
    assert_eq!(event.result, "partial_failure");
    assert_eq!(event.fields["execution_policy"], "stop_on_failure");
    assert_eq!(event.fields["attempt_number"], "0");
    assert_eq!(event.fields["failed_count"], "1");
    assert_eq!(event.fields["skipped_count"], "1");
    assert_eq!(event.fields["error_code"], "batch_items_failed");
    assert_eq!(event.fields["task_id"].len(), 12);
    assert_eq!(event.fields["batch_id"].len(), 12);
    assert!(!event.fields.values().any(|value| value == "batch-a"));
    assert!(!event.fields.contains_key("plan_token"));
    assert!(!event.fields.contains_key("batch_digest"));
    assert!(!event.fields.contains_key("path"));
}

#[test]
fn install_adapter_marks_manifest_failure_with_successful_rollback_retryable() {
    let (batch, _, _) = batch();
    let task_manager = Arc::new(TaskManager::new());
    let parent_task_id = start_parent_task(&task_manager);
    let (executor, game_files, recovery, _) = transaction_item_executor(
        task_manager,
        true,
        false,
        Arc::new(RecordingAuditLogWriter::default()),
    );

    let result = executor.execute(first_item_request(&batch, parent_task_id));

    assert_eq!(
        result,
        BatchInstallItemExecution::Failed {
            reason_code: "install_rollback_succeeded".to_owned(),
            retryable: true,
            evidence_health_degraded: false,
        }
    );
    assert_eq!(game_files.file_bytes("nativepc/a"), None);
    assert!(recovery
        .history()
        .iter()
        .all(|record| record.status != InstallRecoveryRecordStatus::RollbackRequired));
}

#[test]
fn install_adapter_preserves_cancelled_child_when_planning_also_fails() {
    let (batch, _, _) = batch();
    let task_manager = Arc::new(TaskManager::new());
    let parent_task_id = start_parent_task(&task_manager);
    task_manager
        .cancel_task(&parent_task_id)
        .expect("parent cancellation");
    let runner = Arc::new(InstallTaskRunner::new(
        task_manager.clone(),
        Arc::new(FailingInstallPlanner),
        Arc::new(UnreachableInstallCommitter),
        Arc::new(RecordingAuditLogWriter::default()),
        Arc::new(FixedClock),
    ));
    let executor = InstallTaskBatchItemExecutor::new(runner, task_manager);

    assert_eq!(
        executor.execute(first_item_request(&batch, parent_task_id)),
        BatchInstallItemExecution::Cancelled
    );
}

#[test]
fn install_adapter_marks_rollback_failure_recovery_required_and_not_retryable() {
    let (batch, _, _) = batch();
    let task_manager = Arc::new(TaskManager::new());
    let parent_task_id = start_parent_task(&task_manager);
    let (executor, game_files, recovery, _) = transaction_item_executor(
        task_manager,
        true,
        true,
        Arc::new(RecordingAuditLogWriter::default()),
    );

    let result = executor.execute(first_item_request(&batch, parent_task_id));

    assert_eq!(
        result,
        BatchInstallItemExecution::RecoveryRequired {
            reason_code: "install_rollback_failed".to_owned(),
        }
    );
    assert_eq!(
        game_files.file_bytes("nativepc/a").as_deref(),
        Some(b"new item bytes".as_slice())
    );
    assert_eq!(
        recovery.history().last().map(|record| record.status),
        Some(InstallRecoveryRecordStatus::RollbackRequired)
    );
}

#[test]
fn install_adapter_success_persists_exact_revision_manifest_and_item_audit() {
    let (batch, _, _) = batch();
    let task_manager = Arc::new(TaskManager::new());
    let parent_task_id = start_parent_task(&task_manager);
    let audit = Arc::new(RecordingAuditLogWriter::default());
    let (executor, game_files, _, manifests) =
        transaction_item_executor(task_manager, false, false, audit.clone());
    let sentinel = InstallTargetPath::parse("nativepc/sentinel", ["nativepc"]).expect("sentinel");
    game_files
        .write_game_file(&sentinel, b"untouched sentinel")
        .expect("seed sentinel");

    assert_eq!(
        executor.execute(first_item_request(&batch, parent_task_id)),
        BatchInstallItemExecution::Succeeded {
            evidence_health_degraded: false,
        }
    );
    assert_eq!(
        game_files.file_bytes("nativepc/a").as_deref(),
        Some(b"new item bytes".as_slice())
    );
    assert_eq!(
        game_files.file_bytes("nativepc/sentinel").as_deref(),
        Some(b"untouched sentinel".as_slice())
    );
    let manifest = manifests
        .saved_manifest()
        .expect("successful item saves a manifest");
    assert_eq!(
        manifest.schema_version,
        hmm_core::INSTALL_MANIFEST_SCHEMA_VERSION_V2
    );
    assert_eq!(manifest.entries.len(), 1);
    assert_eq!(
        manifest.entries[0].revision_id,
        Some(ModRevisionId::new("a"))
    );
    let events = audit.take_events();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].operation, "commit_imported_mod");
    assert_eq!(events[0].result, "success");
    assert_eq!(events[0].fields["mod_id"], "a");
    assert_eq!(events[0].fields["action_count"], "1");
}

#[test]
fn item_audit_degradation_stops_following_item_after_committed_success() {
    let (batch, attempt, token) = batch();
    let repository = Arc::new(FakeRepository::default());
    repository
        .seal_batch(BatchSealRequest {
            sealed_batch: &batch,
            initial_attempt: &attempt,
        })
        .expect("seal");
    let task_manager = Arc::new(TaskManager::new());
    let (executor, game_files, _, _) = transaction_item_executor(
        task_manager.clone(),
        false,
        false,
        Arc::new(RecordingAuditLogWriter::failing()),
    );
    let runner = BatchInstallTaskRunner::new(
        task_manager,
        repository.clone(),
        executor,
        Arc::new(StaticFacts(facts_from_batch(&batch))),
        Arc::new(RecordingAuditLogWriter::default()),
        Arc::new(FixedClock),
        Arc::new(crate::Sha256BatchTokenCodec::new("secret").expect("codec")),
    );

    let result = runner.run(&batch.batch_id, &token).expect("run");

    assert_eq!(result.status, BatchAttemptStatus::CompletedWithErrors);
    assert_eq!(result.summary.succeeded_count, 1);
    assert_eq!(result.summary.skipped_count, 1);
    assert_eq!(
        game_files.file_bytes("nativepc/a").as_deref(),
        Some(b"new item bytes".as_slice())
    );
    let attempt = repository
        .load_attempt(&batch.batch_id, 0)
        .expect("attempt")
        .expect("attempt exists");
    assert!(attempt.evidence_health_degraded);
}

#[test]
fn batch_audit_failure_marks_evidence_degraded_without_rewriting_item_results() {
    let (batch, attempt, token) = batch();
    let repository = Arc::new(FakeRepository::default());
    repository
        .seal_batch(BatchSealRequest {
            sealed_batch: &batch,
            initial_attempt: &attempt,
        })
        .expect("seal");
    let runner = BatchInstallTaskRunner::new(
        Arc::new(TaskManager::new()),
        repository.clone(),
        Arc::new(FakeExecutor {
            executions: Mutex::new(vec![
                BatchInstallItemExecution::Succeeded {
                    evidence_health_degraded: false,
                },
                BatchInstallItemExecution::Succeeded {
                    evidence_health_degraded: false,
                },
            ]),
        }),
        Arc::new(StaticFacts(facts_from_batch(&batch))),
        Arc::new(RecordingAuditLogWriter::failing()),
        Arc::new(FixedClock),
        Arc::new(crate::Sha256BatchTokenCodec::new("secret").expect("codec")),
    );

    let result = runner.run(&batch.batch_id, &token).expect("run");

    assert_eq!(result.status, BatchAttemptStatus::Completed);
    assert_eq!(result.summary.succeeded_count, 2);
    let attempt = repository
        .load_attempt(&batch.batch_id, 0)
        .expect("attempt")
        .expect("attempt exists");
    assert!(attempt.evidence_health_degraded);
    assert!(repository
        .list_item_results(&batch.batch_id, 0)
        .expect("results")
        .iter()
        .all(|item| item.status == BatchItemStatus::Succeeded));
}

#[test]
fn continue_policy_runs_later_item_after_retryable_failure() {
    let (mut batch, attempt, token) = batch();
    batch.request.execution_policy = BatchExecutionPolicy::ContinueOnItemFailure;
    batch.plan.execution_policy = BatchExecutionPolicy::ContinueOnItemFailure;
    let repository = Arc::new(FakeRepository::default());
    repository
        .seal_batch(BatchSealRequest {
            sealed_batch: &batch,
            initial_attempt: &attempt,
        })
        .expect("seal");
    let runner = BatchInstallTaskRunner::new(
        Arc::new(TaskManager::new()),
        repository.clone(),
        Arc::new(FakeExecutor {
            executions: Mutex::new(vec![
                BatchInstallItemExecution::Failed {
                    reason_code: "commit_failed".to_owned(),
                    retryable: true,
                    evidence_health_degraded: false,
                },
                BatchInstallItemExecution::Succeeded {
                    evidence_health_degraded: false,
                },
            ]),
        }),
        Arc::new(StaticFacts(facts_from_batch(&batch))),
        Arc::new(RecordingAuditLogWriter::default()),
        Arc::new(FixedClock),
        Arc::new(crate::Sha256BatchTokenCodec::new("secret").expect("codec")),
    );

    let result = runner.run(&batch.batch_id, &token).expect("run");

    assert_eq!(result.status, BatchAttemptStatus::CompletedWithErrors);
    assert_eq!(result.summary.failed_count, 1);
    assert_eq!(result.summary.succeeded_count, 1);
    assert_eq!(result.summary.skipped_count, 0);
}

#[test]
fn recovery_required_stops_continue_policy_and_cannot_be_retried() {
    let (mut batch, attempt, token) = batch();
    batch.request.execution_policy = BatchExecutionPolicy::ContinueOnItemFailure;
    batch.plan.execution_policy = BatchExecutionPolicy::ContinueOnItemFailure;
    let repository = Arc::new(FakeRepository::default());
    repository
        .seal_batch(BatchSealRequest {
            sealed_batch: &batch,
            initial_attempt: &attempt,
        })
        .expect("seal");
    let runner = BatchInstallTaskRunner::new(
        Arc::new(TaskManager::new()),
        repository.clone(),
        Arc::new(FakeExecutor {
            executions: Mutex::new(vec![BatchInstallItemExecution::RecoveryRequired {
                reason_code: "install_rollback_failed".to_owned(),
            }]),
        }),
        Arc::new(StaticFacts(facts_from_batch(&batch))),
        Arc::new(RecordingAuditLogWriter::default()),
        Arc::new(FixedClock),
        Arc::new(crate::Sha256BatchTokenCodec::new("secret").expect("codec")),
    );

    let result = runner.run(&batch.batch_id, &token).expect("run");

    assert_eq!(result.status, BatchAttemptStatus::RecoveryRequired);
    assert_eq!(result.summary.recovery_required_count, 1);
    assert_eq!(result.summary.skipped_count, 1);
    assert_eq!(
        BatchInstallRetryService::new(
            repository,
            Arc::new(FixedClock),
            Arc::new(crate::Sha256BatchTokenCodec::new("secret").expect("codec")),
        )
        .retry(&batch.batch_id, 0),
        Err(BatchInstallRetryError::RetryUnavailable)
    );
}

#[test]
fn completion_clock_failure_interrupts_attempt_and_disables_retry() {
    let (batch, attempt, token) = batch();
    let repository = Arc::new(FakeRepository::default());
    repository
        .seal_batch(BatchSealRequest {
            sealed_batch: &batch,
            initial_attempt: &attempt,
        })
        .expect("seal");
    let runner = BatchInstallTaskRunner::new(
        Arc::new(TaskManager::new()),
        repository.clone(),
        Arc::new(FakeExecutor {
            executions: Mutex::new(vec![
                BatchInstallItemExecution::Succeeded {
                    evidence_health_degraded: false,
                },
                BatchInstallItemExecution::Succeeded {
                    evidence_health_degraded: false,
                },
            ]),
        }),
        Arc::new(StaticFacts(facts_from_batch(&batch))),
        Arc::new(RecordingAuditLogWriter::default()),
        Arc::new(FailingCompletionClock::default()),
        Arc::new(crate::Sha256BatchTokenCodec::new("secret").expect("codec")),
    );

    assert_eq!(
        runner.run(&batch.batch_id, &token),
        Err(BatchInstallRunError::JournalUnavailable)
    );
    let attempt = repository
        .load_attempt(&batch.batch_id, 0)
        .expect("attempt")
        .expect("attempt exists");
    assert_eq!(attempt.status, BatchAttemptStatus::Interrupted);
    assert!(attempt.evidence_health_degraded);
    assert!(
        attempt.completed_at_unix_millis >= attempt.started_at_unix_millis,
        "clock failure fallback must preserve a monotonic journal timeline"
    );
    assert_eq!(
        BatchInstallRetryService::new(
            repository,
            Arc::new(FixedClock),
            Arc::new(crate::Sha256BatchTokenCodec::new("secret").expect("codec")),
        )
        .retry(&batch.batch_id, 0),
        Err(BatchInstallRetryError::RetryUnavailable)
    );
}

#[test]
fn terminal_journal_failure_appends_interrupted_correction_audit() {
    let (batch, attempt, token) = batch();
    let repository = Arc::new(FakeRepository::default());
    repository
        .seal_batch(BatchSealRequest {
            sealed_batch: &batch,
            initial_attempt: &attempt,
        })
        .expect("seal");
    repository
        .finish_attempt_failures_remaining
        .store(1, Ordering::Relaxed);
    let audit = Arc::new(RecordingAuditLogWriter::default());
    let runner = BatchInstallTaskRunner::new(
        Arc::new(TaskManager::new()),
        repository.clone(),
        Arc::new(FakeExecutor {
            executions: Mutex::new(vec![
                BatchInstallItemExecution::Succeeded {
                    evidence_health_degraded: false,
                },
                BatchInstallItemExecution::Succeeded {
                    evidence_health_degraded: false,
                },
            ]),
        }),
        Arc::new(StaticFacts(facts_from_batch(&batch))),
        audit.clone(),
        Arc::new(FixedClock),
        Arc::new(crate::Sha256BatchTokenCodec::new("secret").expect("codec")),
    );

    assert_eq!(
        runner.run(&batch.batch_id, &token),
        Err(BatchInstallRunError::JournalUnavailable)
    );
    let attempt = repository
        .load_attempt(&batch.batch_id, 0)
        .expect("attempt")
        .expect("attempt exists");
    assert_eq!(attempt.status, BatchAttemptStatus::Interrupted);
    assert!(attempt.evidence_health_degraded);

    let events = audit.take_events();
    assert_eq!(events.len(), 2);
    assert_eq!(events[0].result, "success");
    assert_eq!(events[1].result, "interrupted");
    assert_eq!(
        events[1].fields.get("error_code").map(String::as_str),
        Some("batch_journal_interrupted")
    );
    assert_eq!(events[1].fields["succeeded_count"], "2");
}

#[test]
fn cancelled_item_stops_future_items_without_losing_terminal_facts() {
    let (batch, attempt, token) = batch();
    let repository = Arc::new(FakeRepository::default());
    repository
        .seal_batch(BatchSealRequest {
            sealed_batch: &batch,
            initial_attempt: &attempt,
        })
        .expect("seal");
    let runner = BatchInstallTaskRunner::new(
        Arc::new(TaskManager::new()),
        repository.clone(),
        Arc::new(FakeExecutor {
            executions: Mutex::new(vec![BatchInstallItemExecution::Cancelled]),
        }),
        Arc::new(StaticFacts(facts_from_batch(&batch))),
        Arc::new(RecordingAuditLogWriter::default()),
        Arc::new(FixedClock),
        Arc::new(crate::Sha256BatchTokenCodec::new("secret").expect("codec")),
    );

    let result = runner.run(&batch.batch_id, &token).expect("run");

    assert_eq!(result.status, BatchAttemptStatus::Cancelled);
    assert_eq!(result.summary.cancelled_count, 1);
    assert_eq!(result.summary.skipped_count, 1);
    let results = repository
        .list_item_results(&batch.batch_id, 0)
        .expect("results");
    assert_eq!(results[0].status, BatchItemStatus::Cancelled);
    assert_eq!(results[1].status, BatchItemStatus::Skipped);
}

#[test]
fn commit_time_cancellation_preserves_success_and_stops_following_item() {
    let (batch, attempt, token) = batch();
    let repository = Arc::new(FakeRepository::default());
    repository
        .seal_batch(BatchSealRequest {
            sealed_batch: &batch,
            initial_attempt: &attempt,
        })
        .expect("seal");
    let task_manager = Arc::new(TaskManager::new());
    let executor = Arc::new(CommitCancellationExecutor {
        task_manager: task_manager.clone(),
        calls: AtomicUsize::new(0),
    });
    let runner = BatchInstallTaskRunner::new(
        task_manager,
        repository.clone(),
        executor.clone(),
        Arc::new(StaticFacts(facts_from_batch(&batch))),
        Arc::new(RecordingAuditLogWriter::default()),
        Arc::new(FixedClock),
        Arc::new(crate::Sha256BatchTokenCodec::new("secret").expect("codec")),
    );

    let result = runner.run(&batch.batch_id, &token).expect("run");

    assert_eq!(result.status, BatchAttemptStatus::Cancelled);
    assert_eq!(result.summary.succeeded_count, 1);
    assert_eq!(result.summary.skipped_count, 1);
    assert_eq!(executor.calls.load(Ordering::Relaxed), 1);
    let results = repository
        .list_item_results(&batch.batch_id, 0)
        .expect("results");
    assert_eq!(results[0].status, BatchItemStatus::Succeeded);
    assert_eq!(results[1].status, BatchItemStatus::Skipped);
}

#[test]
fn stale_item_facts_block_before_any_executor_call() {
    let (batch, attempt, token) = batch();
    let mut stale_facts = facts_from_batch(&batch);
    stale_facts.items[0].fact_digest = "changed".to_owned();
    let repository = Arc::new(FakeRepository::default());
    repository
        .seal_batch(BatchSealRequest {
            sealed_batch: &batch,
            initial_attempt: &attempt,
        })
        .expect("seal");
    let executor = Arc::new(FakeExecutor {
        executions: Mutex::new(Vec::new()),
    });
    let runner = BatchInstallTaskRunner::new(
        Arc::new(TaskManager::new()),
        repository.clone(),
        Arc::clone(&executor) as Arc<dyn BatchInstallItemExecutor>,
        Arc::new(StaticFacts(stale_facts)),
        Arc::new(RecordingAuditLogWriter::default()),
        Arc::new(FixedClock),
        Arc::new(crate::Sha256BatchTokenCodec::new("secret").expect("codec")),
    );

    let result = runner.run(&batch.batch_id, &token).expect("run");

    assert_eq!(result.status, BatchAttemptStatus::Blocked);
    assert_eq!(result.summary.blocked_count, 1);
    assert_eq!(result.summary.skipped_count, 1);
    assert!(
        executor.executions.lock().expect("executions").is_empty(),
        "stale facts must stop before the first single-item transaction"
    );
}

#[test]
fn stop_policy_prevalidates_later_item_before_first_write() {
    let (batch, attempt, token) = batch();
    let mut stale_facts = facts_from_batch(&batch);
    stale_facts.items[1].fact_digest = "changed".to_owned();
    let repository = Arc::new(FakeRepository::default());
    repository
        .seal_batch(BatchSealRequest {
            sealed_batch: &batch,
            initial_attempt: &attempt,
        })
        .expect("seal");
    let executor = Arc::new(FakeExecutor {
        executions: Mutex::new(vec![
            BatchInstallItemExecution::Succeeded {
                evidence_health_degraded: false,
            },
            BatchInstallItemExecution::Succeeded {
                evidence_health_degraded: false,
            },
        ]),
    });
    let runner = BatchInstallTaskRunner::new(
        Arc::new(TaskManager::new()),
        repository,
        executor.clone(),
        Arc::new(StaticFacts(stale_facts)),
        Arc::new(RecordingAuditLogWriter::default()),
        Arc::new(FixedClock),
        Arc::new(crate::Sha256BatchTokenCodec::new("secret").expect("codec")),
    );

    let result = runner.run(&batch.batch_id, &token).expect("run");

    assert_eq!(result.status, BatchAttemptStatus::Blocked);
    assert_eq!(result.summary.blocked_count, 1);
    assert_eq!(result.summary.skipped_count, 1);
    assert_eq!(executor.executions.lock().expect("executions").len(), 2);
}

#[test]
fn continue_policy_stops_when_batch_facts_are_unavailable() {
    let (mut batch, attempt, token) = batch();
    batch.request.execution_policy = BatchExecutionPolicy::ContinueOnItemFailure;
    batch.plan.execution_policy = BatchExecutionPolicy::ContinueOnItemFailure;
    let repository = Arc::new(FakeRepository::default());
    repository
        .seal_batch(BatchSealRequest {
            sealed_batch: &batch,
            initial_attempt: &attempt,
        })
        .expect("seal");
    let executor = Arc::new(FakeExecutor {
        executions: Mutex::new(vec![
            BatchInstallItemExecution::Succeeded {
                evidence_health_degraded: false,
            },
            BatchInstallItemExecution::Succeeded {
                evidence_health_degraded: false,
            },
        ]),
    });
    let runner = BatchInstallTaskRunner::new(
        Arc::new(TaskManager::new()),
        repository.clone(),
        executor.clone(),
        Arc::new(UnavailableFacts),
        Arc::new(RecordingAuditLogWriter::default()),
        Arc::new(FixedClock),
        Arc::new(crate::Sha256BatchTokenCodec::new("secret").expect("codec")),
    );

    let result = runner.run(&batch.batch_id, &token).expect("run");

    assert_eq!(result.status, BatchAttemptStatus::Blocked);
    assert_eq!(result.summary.blocked_count, 1);
    assert_eq!(result.summary.skipped_count, 1);
    assert_eq!(executor.executions.lock().expect("executions").len(), 2);
    let results = repository
        .list_item_results(&batch.batch_id, 0)
        .expect("results");
    assert_eq!(
        results[0].reason_code.as_deref(),
        Some("batch_facts_unavailable")
    );
    assert_eq!(results[1].status, BatchItemStatus::Skipped);
}

#[test]
fn retry_selects_only_prior_retryable_items() {
    let (batch, attempt, token) = batch();
    let repository = Arc::new(FakeRepository::default());
    repository
        .seal_batch(BatchSealRequest {
            sealed_batch: &batch,
            initial_attempt: &attempt,
        })
        .expect("seal");
    let runner = BatchInstallTaskRunner::new(
        Arc::new(TaskManager::new()),
        repository.clone(),
        Arc::new(FakeExecutor {
            executions: Mutex::new(vec![
                BatchInstallItemExecution::Succeeded {
                    evidence_health_degraded: false,
                },
                BatchInstallItemExecution::Failed {
                    reason_code: "commit_failed".to_owned(),
                    retryable: true,
                    evidence_health_degraded: false,
                },
            ]),
        }),
        Arc::new(StaticFacts(facts_from_batch(&batch))),
        Arc::new(RecordingAuditLogWriter::default()),
        Arc::new(FixedClock),
        Arc::new(crate::Sha256BatchTokenCodec::new("secret").expect("codec")),
    );
    runner.run(&batch.batch_id, &token).expect("first run");

    let retry = BatchInstallRetryService::new(
        repository.clone(),
        Arc::new(FixedClock),
        Arc::new(crate::Sha256BatchTokenCodec::new("secret").expect("codec")),
    )
    .retry(&batch.batch_id, 0)
    .expect("retry");

    assert_eq!(retry.attempt_number, 1);
    let retry_attempt = repository
        .load_attempt(&batch.batch_id, 1)
        .expect("retry attempt")
        .expect("retry attempt exists");
    assert_eq!(retry_attempt.item_ids, vec![batch.items[1].item_id.clone()]);
    assert!(!retry_attempt.item_ids.contains(&batch.items[0].item_id));
}

#[test]
fn item_result_journal_failure_interrupts_and_stops_later_items() {
    let (batch, attempt, token) = batch();
    let repository = Arc::new(FakeRepository::default());
    repository
        .seal_batch(BatchSealRequest {
            sealed_batch: &batch,
            initial_attempt: &attempt,
        })
        .expect("seal");
    repository
        .fail_record_item_result
        .store(true, Ordering::Relaxed);
    let executor = Arc::new(FakeExecutor {
        executions: Mutex::new(vec![
            BatchInstallItemExecution::Succeeded {
                evidence_health_degraded: false,
            },
            BatchInstallItemExecution::Succeeded {
                evidence_health_degraded: false,
            },
        ]),
    });
    let audit = Arc::new(RecordingAuditLogWriter::default());
    let task_manager = Arc::new(TaskManager::new());
    let runner = BatchInstallTaskRunner::new(
        task_manager.clone(),
        repository.clone(),
        executor.clone(),
        Arc::new(StaticFacts(facts_from_batch(&batch))),
        audit.clone(),
        Arc::new(FixedClock),
        Arc::new(crate::Sha256BatchTokenCodec::new("secret").expect("codec")),
    );

    assert_eq!(
        runner.run(&batch.batch_id, &token),
        Err(BatchInstallRunError::JournalUnavailable)
    );

    let attempt = repository
        .load_attempt(&batch.batch_id, 0)
        .expect("attempt")
        .expect("attempt exists");
    assert_eq!(attempt.status, BatchAttemptStatus::Interrupted);
    assert!(attempt.evidence_health_degraded);
    assert!(repository
        .list_item_results(&batch.batch_id, 0)
        .expect("results")
        .is_empty());
    assert_eq!(executor.executions.lock().expect("executions").len(), 1);
    assert_eq!(
        audit.policies.lock().expect("audit policies").as_slice(),
        &[AuditWriteFailurePolicy::ReportAfterCommit]
    );
    assert_eq!(
        task_manager.task_status(attempt.task_id.as_deref().expect("task id")),
        Some(TaskStatus::Failed)
    );
    assert_eq!(
        BatchInstallRetryService::new(
            repository,
            Arc::new(FixedClock),
            Arc::new(crate::Sha256BatchTokenCodec::new("secret").expect("codec")),
        )
        .retry(&batch.batch_id, 0),
        Err(BatchInstallRetryError::RetryUnavailable)
    );
}

#[test]
fn journal_failure_after_an_item_started_is_interrupted() {
    let (batch, attempt, token) = batch();
    let repository = Arc::new(FakeRepository::default());
    repository
        .seal_batch(BatchSealRequest {
            sealed_batch: &batch,
            initial_attempt: &attempt,
        })
        .expect("seal");
    repository
        .fail_record_item_result_on_call
        .store(2, Ordering::Relaxed);
    let audit = Arc::new(RecordingAuditLogWriter::default());
    let runner = BatchInstallTaskRunner::new(
        Arc::new(TaskManager::new()),
        repository.clone(),
        Arc::new(FakeExecutor {
            executions: Mutex::new(vec![
                BatchInstallItemExecution::Succeeded {
                    evidence_health_degraded: false,
                },
                BatchInstallItemExecution::Succeeded {
                    evidence_health_degraded: false,
                },
            ]),
        }),
        Arc::new(StaticFacts(facts_from_batch(&batch))),
        audit.clone(),
        Arc::new(FixedClock),
        Arc::new(crate::Sha256BatchTokenCodec::new("secret").expect("codec")),
    );

    assert_eq!(
        runner.run(&batch.batch_id, &token),
        Err(BatchInstallRunError::JournalUnavailable)
    );
    let attempt = repository
        .load_attempt(&batch.batch_id, 0)
        .expect("attempt")
        .expect("attempt exists");
    assert_eq!(attempt.status, BatchAttemptStatus::Interrupted);
    assert!(attempt.evidence_health_degraded);
    let results = repository
        .list_item_results(&batch.batch_id, 0)
        .expect("results");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].status, BatchItemStatus::Succeeded);
    let events = audit.take_events();
    assert_eq!(
        events.last().map(|event| event.result.as_str()),
        Some("interrupted")
    );
    assert_eq!(
        events
            .last()
            .and_then(|event| event.fields.get("succeeded_count"))
            .map(String::as_str),
        Some("1")
    );
}

#[test]
fn fully_unwritable_journal_preserves_running_intent_and_disables_retry() {
    let (batch, attempt, token) = batch();
    let repository = Arc::new(FakeRepository::default());
    repository
        .seal_batch(BatchSealRequest {
            sealed_batch: &batch,
            initial_attempt: &attempt,
        })
        .expect("seal");
    repository
        .fail_record_item_result
        .store(true, Ordering::Relaxed);
    repository
        .fail_finish_attempt
        .store(true, Ordering::Relaxed);
    let executor = Arc::new(FakeExecutor {
        executions: Mutex::new(vec![BatchInstallItemExecution::Succeeded {
            evidence_health_degraded: false,
        }]),
    });
    let runner = BatchInstallTaskRunner::new(
        Arc::new(TaskManager::new()),
        repository.clone(),
        executor,
        Arc::new(StaticFacts(facts_from_batch(&batch))),
        Arc::new(RecordingAuditLogWriter::default()),
        Arc::new(FixedClock),
        Arc::new(crate::Sha256BatchTokenCodec::new("secret").expect("codec")),
    );

    assert_eq!(
        runner.run(&batch.batch_id, &token),
        Err(BatchInstallRunError::JournalUnavailable)
    );
    assert_eq!(
        repository
            .load_attempt(&batch.batch_id, 0)
            .expect("attempt")
            .expect("attempt exists")
            .status,
        BatchAttemptStatus::Running
    );
    assert_eq!(
        BatchInstallRetryService::new(
            repository,
            Arc::new(FixedClock),
            Arc::new(crate::Sha256BatchTokenCodec::new("secret").expect("codec")),
        )
        .retry(&batch.batch_id, 0),
        Err(BatchInstallRetryError::RetryUnavailable)
    );
}

#[test]
fn admission_failure_terminalizes_the_temporary_task() {
    let (batch, attempt, token) = batch();
    let repository = Arc::new(FakeRepository::default());
    repository
        .seal_batch(BatchSealRequest {
            sealed_batch: &batch,
            initial_attempt: &attempt,
        })
        .expect("seal");
    repository.fail_admission.store(true, Ordering::Relaxed);
    let task_manager = Arc::new(TaskManager::new());
    let runner = BatchInstallTaskRunner::new(
        task_manager.clone(),
        repository.clone(),
        Arc::new(FakeExecutor {
            executions: Mutex::new(Vec::new()),
        }),
        Arc::new(StaticFacts(facts_from_batch(&batch))),
        Arc::new(RecordingAuditLogWriter::default()),
        Arc::new(FixedClock),
        Arc::new(crate::Sha256BatchTokenCodec::new("secret").expect("codec")),
    );

    assert_eq!(
        runner.run(&batch.batch_id, &token),
        Err(BatchInstallRunError::JournalUnavailable)
    );
    let task_id = repository
        .admission_task_ids
        .lock()
        .expect("admission task ids")[0]
        .clone();
    assert_eq!(task_manager.task_status(&task_id), Some(TaskStatus::Failed));
}

#[test]
fn active_scope_admission_fails_the_temporary_task_without_starting_the_attempt() {
    let (batch, attempt, token) = batch();
    let repository = Arc::new(FakeRepository::default());
    repository
        .seal_batch(BatchSealRequest {
            sealed_batch: &batch,
            initial_attempt: &attempt,
        })
        .expect("seal");
    repository
        .scope_active_admission
        .store(true, Ordering::Relaxed);
    let task_manager = Arc::new(TaskManager::new());
    let runner = BatchInstallTaskRunner::new(
        task_manager.clone(),
        repository.clone(),
        Arc::new(FakeExecutor {
            executions: Mutex::new(Vec::new()),
        }),
        Arc::new(StaticFacts(facts_from_batch(&batch))),
        Arc::new(RecordingAuditLogWriter::default()),
        Arc::new(FixedClock),
        Arc::new(crate::Sha256BatchTokenCodec::new("secret").expect("codec")),
    );

    assert_eq!(
        runner.run(&batch.batch_id, &token),
        Err(BatchInstallRunError::ScopeReconciliationRequired)
    );
    let task_id = repository
        .admission_task_ids
        .lock()
        .expect("admission task ids")[0]
        .clone();
    assert_eq!(task_manager.task_status(&task_id), Some(TaskStatus::Failed));
    assert_eq!(
        repository
            .load_attempt(&batch.batch_id, 0)
            .expect("load attempt")
            .expect("attempt")
            .status,
        BatchAttemptStatus::Sealed
    );
}

#[test]
fn active_scope_admission_discards_an_unadmitted_retry_attempt() {
    let (batch, initial_attempt, _) = batch();
    let repository = Arc::new(FakeRepository::default());
    repository
        .seal_batch(BatchSealRequest {
            sealed_batch: &batch,
            initial_attempt: &initial_attempt,
        })
        .expect("seal");
    let retry_item_ids = initial_attempt.item_ids.clone();
    let codec = crate::Sha256BatchTokenCodec::new("secret").expect("codec");
    let retry_token = codec
        .issue(
            crate::BatchTokenKind::Plan,
            &execution_token_digest(
                &batch.batch_id,
                1,
                &retry_item_ids,
                &batch.plan.batch_digest,
                &batch.plan.environment_digest,
            ),
            &batch.plan.environment_digest,
            1,
            100,
        )
        .expect("retry token")
        .token;
    repository.attempts.lock().expect("attempts").insert(
        1,
        hmm_core::BatchAttempt {
            batch_id: batch.batch_id.clone(),
            attempt_number: 1,
            item_ids: retry_item_ids,
            status: BatchAttemptStatus::Sealed,
            task_id: None,
            plan_token_verifier: sha256_hex(retry_token.as_bytes()),
            expires_at_unix_millis: 100,
            started_at_unix_millis: None,
            completed_at_unix_millis: None,
            evidence_health_degraded: false,
        },
    );
    repository
        .scope_active_admission
        .store(true, Ordering::Relaxed);
    let runner = BatchInstallTaskRunner::new(
        Arc::new(TaskManager::new()),
        repository.clone(),
        Arc::new(FakeExecutor {
            executions: Mutex::new(Vec::new()),
        }),
        Arc::new(StaticFacts(facts_from_batch(&batch))),
        Arc::new(RecordingAuditLogWriter::default()),
        Arc::new(FixedClock),
        Arc::new(codec),
    );

    assert_eq!(
        runner.run_attempt(&batch.batch_id, 1, &retry_token),
        Err(BatchInstallRunError::ScopeReconciliationRequired)
    );
    assert!(repository
        .load_attempt(&batch.batch_id, 1)
        .expect("load discarded retry")
        .is_none());
    assert!(repository
        .load_attempt(&batch.batch_id, 0)
        .expect("load initial attempt")
        .is_some());
}

#[test]
fn repeated_start_cancels_the_unadmitted_temporary_task() {
    let (batch, attempt, token) = batch();
    let repository = Arc::new(FakeRepository::default());
    repository
        .seal_batch(BatchSealRequest {
            sealed_batch: &batch,
            initial_attempt: &attempt,
        })
        .expect("seal");
    let task_manager = Arc::new(TaskManager::new());
    let runner = BatchInstallTaskRunner::new(
        task_manager.clone(),
        repository.clone(),
        Arc::new(FakeExecutor {
            executions: Mutex::new(vec![
                BatchInstallItemExecution::Succeeded {
                    evidence_health_degraded: false,
                },
                BatchInstallItemExecution::Succeeded {
                    evidence_health_degraded: false,
                },
            ]),
        }),
        Arc::new(StaticFacts(facts_from_batch(&batch))),
        Arc::new(RecordingAuditLogWriter::default()),
        Arc::new(FixedClock),
        Arc::new(crate::Sha256BatchTokenCodec::new("secret").expect("codec")),
    );
    let first = runner.run(&batch.batch_id, &token).expect("first run");
    let repeated = runner.run(&batch.batch_id, &token).expect("repeated run");

    assert_eq!(repeated.task_id, first.task_id);
    let task_ids = repository
        .admission_task_ids
        .lock()
        .expect("admission task ids")
        .clone();
    assert_eq!(task_ids.len(), 2);
    assert_eq!(
        task_manager.task_status(&task_ids[1]),
        Some(TaskStatus::Cancelled)
    );
}

#[test]
fn repeated_start_after_token_expiry_returns_original_task_without_reexecution() {
    let (batch, attempt, token) = batch();
    let repository = Arc::new(FakeRepository::default());
    repository
        .seal_batch(BatchSealRequest {
            sealed_batch: &batch,
            initial_attempt: &attempt,
        })
        .expect("seal");
    let task_manager = Arc::new(TaskManager::new());
    let executor = Arc::new(FakeExecutor {
        executions: Mutex::new(vec![
            BatchInstallItemExecution::Succeeded {
                evidence_health_degraded: false,
            },
            BatchInstallItemExecution::Succeeded {
                evidence_health_degraded: false,
            },
        ]),
    });
    let first_runner = BatchInstallTaskRunner::new(
        task_manager.clone(),
        repository.clone(),
        executor.clone(),
        Arc::new(StaticFacts(facts_from_batch(&batch))),
        Arc::new(RecordingAuditLogWriter::default()),
        Arc::new(FixedClock),
        Arc::new(crate::Sha256BatchTokenCodec::new("secret").expect("codec")),
    );
    let first = first_runner
        .run(&batch.batch_id, &token)
        .expect("first run");
    let repeated_runner = BatchInstallTaskRunner::new(
        task_manager,
        repository,
        executor.clone(),
        Arc::new(StaticFacts(facts_from_batch(&batch))),
        Arc::new(RecordingAuditLogWriter::default()),
        Arc::new(ExpiredClock),
        Arc::new(crate::Sha256BatchTokenCodec::new("secret").expect("codec")),
    );

    let repeated = repeated_runner
        .run(&batch.batch_id, &token)
        .expect("expired retry of an admitted attempt is idempotent");

    assert_eq!(repeated.task_id, first.task_id);
    assert!(executor.executions.lock().expect("executions").is_empty());
}

#[test]
fn token_selection_mismatch_is_rejected_before_admission() {
    let (batch, mut attempt, token) = batch();
    attempt.item_ids = vec![batch.items[1].item_id.clone()];
    let repository = Arc::new(FakeRepository::default());
    repository
        .seal_batch(BatchSealRequest {
            sealed_batch: &batch,
            initial_attempt: &attempt,
        })
        .expect("seal corrupted fixture");
    let runner = BatchInstallTaskRunner::new(
        Arc::new(TaskManager::new()),
        repository.clone(),
        Arc::new(FakeExecutor {
            executions: Mutex::new(Vec::new()),
        }),
        Arc::new(StaticFacts(facts_from_batch(&batch))),
        Arc::new(RecordingAuditLogWriter::default()),
        Arc::new(FixedClock),
        Arc::new(crate::Sha256BatchTokenCodec::new("secret").expect("codec")),
    );

    assert_eq!(
        runner.run(&batch.batch_id, &token),
        Err(BatchInstallRunError::InvalidToken)
    );
    assert!(repository
        .admission_task_ids
        .lock()
        .expect("admission task ids")
        .is_empty());
}

#[test]
fn install_runner_and_retry_reject_non_install_batch_without_admission() {
    let (mut batch, attempt, token) = batch();
    batch.request.operation = BatchOperation::Uninstall;
    batch.plan.operation = BatchOperation::Uninstall;
    let repository = Arc::new(FakeRepository::default());
    repository
        .seal_batch(BatchSealRequest {
            sealed_batch: &batch,
            initial_attempt: &attempt,
        })
        .expect("seal routed fixture");
    let runner = BatchInstallTaskRunner::new(
        Arc::new(TaskManager::new()),
        repository.clone(),
        Arc::new(FakeExecutor {
            executions: Mutex::new(Vec::new()),
        }),
        Arc::new(StaticFacts(facts_from_batch(&batch))),
        Arc::new(RecordingAuditLogWriter::default()),
        Arc::new(FixedClock),
        Arc::new(crate::Sha256BatchTokenCodec::new("secret").expect("codec")),
    );

    assert_eq!(
        runner.run(&batch.batch_id, &token),
        Err(BatchInstallRunError::OperationMismatch)
    );
    assert_eq!(
        BatchInstallRetryService::new(
            repository.clone(),
            Arc::new(FixedClock),
            Arc::new(crate::Sha256BatchTokenCodec::new("secret").expect("codec")),
        )
        .retry(&batch.batch_id, 0),
        Err(BatchInstallRetryError::RetryUnavailable)
    );
    assert!(repository
        .admission_task_ids
        .lock()
        .expect("admission task ids")
        .is_empty());
    assert_eq!(
        repository
            .load_attempt(&batch.batch_id, 0)
            .expect("attempt")
            .expect("attempt exists")
            .status,
        BatchAttemptStatus::Sealed
    );
}

#[test]
fn replacement_snapshot_is_blocked_before_plain_install_execution() {
    let (mut batch, _, _) = batch();
    let hmm_core::BatchItemInput::Install(input) = &mut batch.plan.items[0].input_snapshot else {
        panic!("install input");
    };
    input.replacement_binding_snapshot = Some(replacement_snapshot("a"));
    let task_manager = Arc::new(TaskManager::new());
    let parent_task_id = start_parent_task(&task_manager);
    let (executor, game_files, recovery, _) = transaction_item_executor(
        task_manager.clone(),
        false,
        false,
        Arc::new(RecordingAuditLogWriter::default()),
    );

    assert_eq!(
        executor.execute(first_item_request(&batch, parent_task_id)),
        BatchInstallItemExecution::Blocked {
            reason_code: "batch_retarget_install_unsupported".to_owned(),
        }
    );
    assert_eq!(game_files.file_bytes("nativepc/a"), None);
    assert!(recovery.history().is_empty());
    let next_task = task_manager
        .create_task(TaskKind::Install)
        .expect("next task");
    assert_eq!(next_task.task_id.rsplit('-').next(), Some("1"));
}
