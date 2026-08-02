use super::*;
use crate::{
    ReinstallCommitError, ReinstallCommitPhase, ReinstallCommitResult, ReinstallTaskAuditContext,
    ReinstallTaskPrepareError, ReinstallTaskPrepared,
};
use hmm_core::{
    BatchExecutionPolicy, BatchId, BatchItemId, BatchItemPlan, BatchPlan, BatchResourceLimits,
    BatchResourceUsage, BatchTargetClaim, BatchTargetWriteKind, FileLayer, InstallManifest,
    InstallRecoveryRecord, InstallRecoveryRecordStatus, InstallTargetPath, ModId, ModRevisionId,
    ReplacementBinding, ReplacementBindingId, ReplacementBindingSnapshot, ReplacementSourceId,
    ReplacementTargetId, ReplacementTargetKind, SealedBatchItem,
};
use hmm_ports::{AppClock, AuditLogEvent, AuditLogWriter};
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Mutex;

struct StaticFactsReader {
    rules_versions: BTreeMap<String, Option<u32>>,
    reads: AtomicUsize,
}

impl BatchReinstallItemFactsReader for StaticFactsReader {
    fn read_item_facts(
        &self,
        request: &BatchReinstallItemFactsRequest,
    ) -> anyhow::Result<BatchItemFacts> {
        self.reads.fetch_add(1, Ordering::Relaxed);
        let mod_id = request.input.mod_id.clone();
        Ok(BatchItemFacts {
            mod_id: mod_id.clone(),
            source_revision_id: Some(request.input.candidate_revision_id.clone()),
            installed_revision_id: Some(request.input.installed_revision_id.clone()),
            fact_digest: format!("fact-{}", mod_id.as_str()),
            single_plan_digest: format!("plan-{}", mod_id.as_str()),
            target_claims: vec![BatchTargetClaim {
                target_path: target(&format!("nativePC/{}.bin", mod_id.as_str())),
                kind: BatchTargetWriteKind::Install,
            }],
            action_summary: BatchActionSummary {
                actions: 1,
                replaced: 1,
                ..BatchActionSummary::default()
            },
            prerequisite: BatchPreflightDecision {
                status: BatchPreflightStatus::Ready,
                rules_version: self.rules_versions.get(mod_id.as_str()).copied().flatten(),
                codes: Vec::new(),
            },
            blocking_reasons: Vec::new(),
            warning_codes: Vec::new(),
        })
    }
}

struct ReadOnlyState {
    manifest: Mutex<Option<InstallManifest>>,
    install_recovery: Mutex<Vec<InstallRecoveryRecord>>,
}

impl InstallManifestRepository for ReadOnlyState {
    fn load_manifest(&self, _profile_id: &ProfileId) -> anyhow::Result<Option<InstallManifest>> {
        Ok(self.manifest.lock().expect("manifest").clone())
    }

    fn save_manifest(&self, _manifest: &InstallManifest) -> anyhow::Result<()> {
        panic!("batch reinstall facts must remain read-only")
    }
}

impl InstallRecoveryRecordRepository for ReadOnlyState {
    fn load_record(
        &self,
        profile_id: &ProfileId,
        mod_id: &ModId,
    ) -> anyhow::Result<Option<InstallRecoveryRecord>> {
        Ok(self
            .install_recovery
            .lock()
            .expect("install recovery")
            .iter()
            .find(|record| &record.profile_id == profile_id && &record.mod_id == mod_id)
            .cloned())
    }

    fn list_records(&self, profile_id: &ProfileId) -> anyhow::Result<Vec<InstallRecoveryRecord>> {
        Ok(self
            .install_recovery
            .lock()
            .expect("install recovery")
            .iter()
            .filter(|record| &record.profile_id == profile_id)
            .cloned()
            .collect())
    }

    fn save_record(&self, _record: &InstallRecoveryRecord) -> anyhow::Result<()> {
        panic!("batch reinstall facts must remain read-only")
    }

    fn remove_record(&self, _profile_id: &ProfileId, _mod_id: &ModId) -> anyhow::Result<()> {
        panic!("batch reinstall facts must remain read-only")
    }
}

impl ReinstallRecoveryTransactionRepository for ReadOnlyState {
    fn load_transaction(
        &self,
        _profile_id: &ProfileId,
        _mod_id: &ModId,
    ) -> anyhow::Result<Option<hmm_core::ReinstallRecoveryTransaction>> {
        Ok(None)
    }

    fn list_transactions(
        &self,
        _profile_id: &ProfileId,
    ) -> anyhow::Result<Vec<hmm_core::ReinstallRecoveryTransaction>> {
        Ok(Vec::new())
    }

    fn save_transaction(
        &self,
        _transaction: &hmm_core::ReinstallRecoveryTransaction,
    ) -> anyhow::Result<()> {
        panic!("batch reinstall facts must remain read-only")
    }

    fn remove_transaction(&self, _profile_id: &ProfileId, _mod_id: &ModId) -> anyhow::Result<()> {
        panic!("batch reinstall facts must remain read-only")
    }
}

#[test]
fn facts_provider_is_read_only_and_rejects_mixed_prerequisite_rules_versions() {
    let state = Arc::new(ReadOnlyState {
        manifest: Mutex::new(Some(InstallManifest::completed(
            ProfileId::new("default"),
            Vec::new(),
        ))),
        install_recovery: Mutex::new(Vec::new()),
    });
    let reader = Arc::new(StaticFactsReader {
        rules_versions: BTreeMap::from([("mod-a".to_owned(), None), ("mod-b".to_owned(), Some(2))]),
        reads: AtomicUsize::new(0),
    });
    let manifests: Arc<dyn InstallManifestRepository> = state.clone();
    let install_recovery: Arc<dyn InstallRecoveryRecordRepository> = state.clone();
    let recovery: Arc<dyn ReinstallRecoveryTransactionRepository> = state;
    let provider = BatchReinstallPlanFactsProvider::new(
        reader.clone(),
        manifests,
        install_recovery,
        recovery,
        "sandbox-env",
    );

    let error = provider
        .read_batch_plan_facts(&normalized_request(vec![
            reinstall_input("mod-a", "v1", "v2", None),
            reinstall_input("mod-b", "v1", "v2", None),
        ]))
        .expect_err("mixed rules versions must fail closed");

    assert!(error.to_string().contains("prerequisite rules changed"));
    assert_eq!(reader.reads.load(Ordering::Relaxed), 2);
}

#[test]
fn facts_provider_reports_profile_manifest_as_a_global_blocker() {
    let mut unsafe_manifest = InstallManifest::completed(ProfileId::new("default"), Vec::new());
    unsafe_manifest.status = hmm_core::InstallManifestStatus::Committing;
    let state = Arc::new(ReadOnlyState {
        manifest: Mutex::new(Some(unsafe_manifest)),
        install_recovery: Mutex::new(Vec::new()),
    });
    let reader = Arc::new(StaticFactsReader {
        rules_versions: BTreeMap::from([("mod-a".to_owned(), Some(1))]),
        reads: AtomicUsize::new(0),
    });
    let manifests: Arc<dyn InstallManifestRepository> = state.clone();
    let install_recovery: Arc<dyn InstallRecoveryRecordRepository> = state.clone();
    let recovery: Arc<dyn ReinstallRecoveryTransactionRepository> = state;
    let provider =
        BatchReinstallPlanFactsProvider::new(reader, manifests, install_recovery, recovery, "env");

    let facts = provider
        .read_batch_plan_facts(&normalized_request(vec![reinstall_input(
            "mod-a", "v1", "v2", None,
        )]))
        .expect("read-only facts");

    assert_eq!(
        facts.global_blocking_reasons,
        vec![BatchReasonSummary {
            code: "batch_global_manifest_unsafe".to_owned(),
            count: 1,
        }]
    );
}

#[test]
fn facts_provider_blocks_only_unsettled_install_recovery_records() {
    let profile_id = ProfileId::new("default");
    let statuses = [
        InstallRecoveryRecordStatus::Planned,
        InstallRecoveryRecordStatus::Committing,
        InstallRecoveryRecordStatus::Completed,
        InstallRecoveryRecordStatus::RollbackRequired,
        InstallRecoveryRecordStatus::RolledBack,
        InstallRecoveryRecordStatus::RepairRequired,
    ];
    let expected_blocking_count = statuses
        .iter()
        .filter(|status| install_recovery_status_blocks_batch_reinstall(**status))
        .count();
    let records = statuses
        .into_iter()
        .enumerate()
        .map(|(index, status)| InstallRecoveryRecord {
            profile_id: profile_id.clone(),
            mod_id: ModId::new(format!("recovery-{index}")),
            status,
            entries: Vec::new(),
        })
        .collect();
    let state = Arc::new(ReadOnlyState {
        manifest: Mutex::new(Some(InstallManifest::completed(profile_id, Vec::new()))),
        install_recovery: Mutex::new(records),
    });
    let reader = Arc::new(StaticFactsReader {
        rules_versions: BTreeMap::from([("mod-a".to_owned(), Some(1))]),
        reads: AtomicUsize::new(0),
    });
    let manifests: Arc<dyn InstallManifestRepository> = state.clone();
    let install_recovery: Arc<dyn InstallRecoveryRecordRepository> = state.clone();
    let recovery: Arc<dyn ReinstallRecoveryTransactionRepository> = state;
    let provider =
        BatchReinstallPlanFactsProvider::new(reader, manifests, install_recovery, recovery, "env");

    let facts = provider
        .read_batch_plan_facts(&normalized_request(vec![reinstall_input(
            "mod-a", "v1", "v2", None,
        )]))
        .expect("read-only facts");

    assert_eq!(
        facts.global_blocking_reasons,
        vec![BatchReasonSummary {
            code: "batch_global_recovery_active".to_owned(),
            count: expected_blocking_count,
        }]
    );
}

#[test]
fn preparation_projection_reuses_blocked_reinstall_facts_contract() {
    let request = BatchReinstallItemFactsRequest {
        game_id: GameId::mhw(),
        profile_id: ProfileId::new("default"),
        input: reinstall_input("mod-a", "v1", "v2", None),
    };
    let preview = ReinstallPlanPreview {
        status: crate::ReinstallPreviewStatus::Blocked,
        prerequisite_decision: crate::GamePrerequisiteDecision {
            game_id: GameId::mhw(),
            status: crate::GamePrerequisiteDecisionStatus::Ready,
            rules_version: Some(1),
            codes: Vec::new(),
        },
        installed_revision: Some(crate::ReinstallRevisionSummary {
            revision_id: ModRevisionId::new("v1"),
        }),
        candidate_revision: Some(crate::ReinstallRevisionSummary {
            revision_id: ModRevisionId::new("v2"),
        }),
        counts: crate::ReinstallTargetCounts::default(),
        blocking_reasons: vec![crate::ReinstallBlockingReasonSummary {
            reason: crate::ReinstallBlockingReason::TargetChanged,
            count: 1,
        }],
        plan_token: None,
    };

    let facts = ReinstallPreviewBatchItemFactsReader::facts_from_preparation(
        &request,
        ReinstallPreparation::Blocked(preview),
    )
    .expect("blocked facts");

    assert_eq!(facts.mod_id, ModId::new("mod-a"));
    assert_eq!(facts.installed_revision_id, Some(ModRevisionId::new("v1")));
    assert_eq!(facts.source_revision_id, Some(ModRevisionId::new("v2")));
    assert_eq!(facts.blocking_reasons, vec!["installed_target_changed"]);
    assert!(facts.target_claims.is_empty());
    assert!(!facts.single_plan_digest.is_empty());
}

#[derive(Clone)]
struct FakePrepared {
    digest: String,
    token: String,
    candidate_revision_id: ModRevisionId,
}

impl ReinstallTaskPrepared for FakePrepared {
    fn audit_context(&self) -> ReinstallTaskAuditContext {
        ReinstallTaskAuditContext {
            previous_revision_id: Some(ModRevisionId::new("v1")),
            candidate_revision_id: self.candidate_revision_id.clone(),
            counts: crate::ReinstallTargetCounts {
                replaced: 1,
                ..crate::ReinstallTargetCounts::default()
            },
        }
    }

    fn plan_token(&self) -> &str {
        &self.token
    }

    fn batch_plan_digest(&self) -> String {
        self.digest.clone()
    }
}

struct FakeExecutor {
    prepared: FakePrepared,
    commit_result: Mutex<Option<Result<ReinstallCommitResult, ReinstallCommitError>>>,
    normal_prepare_count: AtomicUsize,
    retarget_prepare_count: AtomicUsize,
    commit_count: AtomicUsize,
    retarget_target: Mutex<Option<ReplacementTargetId>>,
}

impl FakeExecutor {
    fn new(
        digest: &str,
        commit_result: Result<ReinstallCommitResult, ReinstallCommitError>,
    ) -> Self {
        Self {
            prepared: FakePrepared {
                digest: digest.to_owned(),
                token: "current-full-plan-token".to_owned(),
                candidate_revision_id: ModRevisionId::new("v2"),
            },
            commit_result: Mutex::new(Some(commit_result)),
            normal_prepare_count: AtomicUsize::new(0),
            retarget_prepare_count: AtomicUsize::new(0),
            commit_count: AtomicUsize::new(0),
            retarget_target: Mutex::new(None),
        }
    }
}

impl ReinstallTaskExecutor for FakeExecutor {
    type Prepared = FakePrepared;

    fn prepare(
        &self,
        request: ReinstallPreviewRequest,
    ) -> Result<Self::Prepared, ReinstallTaskPrepareError> {
        self.normal_prepare_count.fetch_add(1, Ordering::Relaxed);
        let mut prepared = self.prepared.clone();
        prepared.candidate_revision_id = request.candidate_revision_id;
        Ok(prepared)
    }

    fn revalidate(&self, _prepared: &Self::Prepared) -> Result<(), ReinstallCommitError> {
        Ok(())
    }

    fn commit(
        &self,
        _prepared: Self::Prepared,
        expected_plan_token: &str,
    ) -> Result<ReinstallCommitResult, ReinstallCommitError> {
        assert_eq!(expected_plan_token, "current-full-plan-token");
        self.commit_count.fetch_add(1, Ordering::Relaxed);
        self.commit_result
            .lock()
            .expect("commit result")
            .take()
            .expect("commit called once")
    }
}

impl RetargetReinstallTaskExecutor for FakeExecutor {
    fn prepare_retarget_reinstall(
        &self,
        request: crate::RetargetReinstallRequest,
    ) -> Result<Self::Prepared, ReinstallTaskPrepareError> {
        self.retarget_prepare_count.fetch_add(1, Ordering::Relaxed);
        *self.retarget_target.lock().expect("retarget target") = Some(request.target_id);
        let mut prepared = self.prepared.clone();
        prepared.candidate_revision_id = ModRevisionId::new("v1");
        Ok(prepared)
    }
}

struct FixedClock;

impl AppClock for FixedClock {
    fn now_unix_millis(&self) -> anyhow::Result<u128> {
        Ok(1)
    }
}

struct TestAudit {
    fail: AtomicBool,
}

impl AuditLogWriter for TestAudit {
    fn record(&self, _event: AuditLogEvent) -> anyhow::Result<()> {
        if self.fail.load(Ordering::Relaxed) {
            anyhow::bail!("injected audit failure");
        }
        Ok(())
    }
}

#[test]
fn item_executor_routes_cross_revision_and_same_revision_retarget() {
    let normal = executor_fixture("sealed-digest", Ok(commit_success()), false);
    let result = normal.executor.execute(item_request(
        &normal.task_manager,
        reinstall_input("mod-a", "v1", "v2", None),
        "sealed-digest",
    ));
    assert_eq!(
        result,
        BatchInstallItemExecution::Succeeded {
            evidence_health_degraded: false,
        }
    );
    assert_eq!(normal.inner.normal_prepare_count.load(Ordering::Relaxed), 1);
    assert_eq!(
        normal.inner.retarget_prepare_count.load(Ordering::Relaxed),
        0
    );

    let binding = replacement_snapshot("mhw:armor:fatalis-alpha", "v1");
    let retarget = executor_fixture("sealed-digest", Ok(commit_success()), false);
    let result = retarget.executor.execute(item_request(
        &retarget.task_manager,
        reinstall_input("mod-a", "v1", "v1", Some(binding)),
        "sealed-digest",
    ));
    assert!(matches!(
        result,
        BatchInstallItemExecution::Succeeded { .. }
    ));
    assert_eq!(
        retarget.inner.normal_prepare_count.load(Ordering::Relaxed),
        0
    );
    assert_eq!(
        retarget
            .inner
            .retarget_prepare_count
            .load(Ordering::Relaxed),
        1
    );
    assert_eq!(
        retarget
            .inner
            .retarget_target
            .lock()
            .expect("retarget target")
            .as_ref()
            .map(ReplacementTargetId::as_str),
        Some("mhw:armor:fatalis-alpha")
    );
}

#[test]
fn same_revision_without_binding_is_blocked_before_any_task_runs() {
    let fixture = executor_fixture("sealed-digest", Ok(commit_success()), false);

    let result = fixture.executor.execute(item_request(
        &fixture.task_manager,
        reinstall_input("mod-a", "v1", "v1", None),
        "sealed-digest",
    ));

    assert_eq!(
        result,
        BatchInstallItemExecution::Blocked {
            reason_code: "batch_retarget_binding_required".to_owned(),
        }
    );
    assert_eq!(fixture.inner.commit_count.load(Ordering::Relaxed), 0);
    assert_eq!(
        fixture.inner.retarget_prepare_count.load(Ordering::Relaxed),
        0
    );
}

#[test]
fn stale_scoped_digest_blocks_before_commit() {
    let fixture = executor_fixture("current-digest", Ok(commit_success()), false);

    let result = fixture.executor.execute(item_request(
        &fixture.task_manager,
        reinstall_input("mod-a", "v1", "v2", None),
        "sealed-digest",
    ));

    assert_eq!(
        result,
        BatchInstallItemExecution::Blocked {
            reason_code: "reinstall_plan_stale".to_owned(),
        }
    );
    assert_eq!(fixture.inner.commit_count.load(Ordering::Relaxed), 0);
}

#[test]
fn structured_commit_outcomes_drive_retry_recovery_and_committed_success() {
    let rolled_back = executor_fixture(
        "sealed",
        Err(ReinstallCommitError::RolledBack {
            failed_phase: ReinstallCommitPhase::Manifest,
            cleanup_pending: false,
        }),
        false,
    );
    assert_eq!(
        rolled_back.executor.execute(item_request(
            &rolled_back.task_manager,
            reinstall_input("mod-a", "v1", "v2", None),
            "sealed",
        )),
        BatchInstallItemExecution::Failed {
            reason_code: "reinstall_rollback_succeeded".to_owned(),
            retryable: true,
            evidence_health_degraded: false,
        }
    );

    let rolled_back_with_cleanup = executor_fixture(
        "sealed",
        Err(ReinstallCommitError::RolledBack {
            failed_phase: ReinstallCommitPhase::Manifest,
            cleanup_pending: true,
        }),
        false,
    );
    assert_eq!(
        rolled_back_with_cleanup.executor.execute(item_request(
            &rolled_back_with_cleanup.task_manager,
            reinstall_input("mod-a", "v1", "v2", None),
            "sealed",
        )),
        BatchInstallItemExecution::Failed {
            reason_code: "reinstall_rollback_succeeded".to_owned(),
            retryable: true,
            evidence_health_degraded: true,
        }
    );

    let recovery = executor_fixture(
        "sealed",
        Err(ReinstallCommitError::RollbackRequired {
            failed_phase: ReinstallCommitPhase::Manifest,
        }),
        false,
    );
    assert_eq!(
        recovery.executor.execute(item_request(
            &recovery.task_manager,
            reinstall_input("mod-a", "v1", "v2", None),
            "sealed",
        )),
        BatchInstallItemExecution::RecoveryRequired {
            reason_code: "reinstall_recovery_required".to_owned(),
        }
    );

    let committed = executor_fixture("sealed", Err(ReinstallCommitError::PostCommit), false);
    assert_eq!(
        committed.executor.execute(item_request(
            &committed.task_manager,
            reinstall_input("mod-a", "v1", "v2", None),
            "sealed",
        )),
        BatchInstallItemExecution::Succeeded {
            evidence_health_degraded: true,
        }
    );
}

#[test]
fn audit_failure_after_commit_is_a_degraded_success() {
    let fixture = executor_fixture("sealed", Ok(commit_success()), true);

    let result = fixture.executor.execute(item_request(
        &fixture.task_manager,
        reinstall_input("mod-a", "v1", "v2", None),
        "sealed",
    ));

    assert_eq!(
        result,
        BatchInstallItemExecution::Succeeded {
            evidence_health_degraded: true,
        }
    );
}

struct ExecutorFixture {
    executor: ReinstallTaskBatchItemExecutor<FakeExecutor>,
    inner: Arc<FakeExecutor>,
    task_manager: Arc<TaskManager>,
}

fn executor_fixture(
    digest: &str,
    commit_result: Result<ReinstallCommitResult, ReinstallCommitError>,
    fail_audit: bool,
) -> ExecutorFixture {
    let task_manager = Arc::new(TaskManager::new());
    let inner = Arc::new(FakeExecutor::new(digest, commit_result));
    let audit = Arc::new(TestAudit {
        fail: AtomicBool::new(fail_audit),
    });
    let runner = Arc::new(ReinstallTaskRunner::new(
        Arc::clone(&task_manager),
        Arc::clone(&inner),
        audit,
        Arc::new(FixedClock),
    ));
    ExecutorFixture {
        executor: ReinstallTaskBatchItemExecutor::new(runner, Arc::clone(&task_manager)),
        inner,
        task_manager,
    }
}

fn item_request(
    task_manager: &TaskManager,
    input: ReinstallBatchItemInput,
    single_plan_digest: &str,
) -> BatchInstallItemRequest {
    let parent = task_manager
        .create_task(TaskKind::Install)
        .expect("parent task");
    task_manager
        .start_task(&parent.task_id)
        .expect("parent starts");
    let plan = plan(input.clone(), single_plan_digest);
    BatchInstallItemRequest {
        batch_id: BatchId::new("batch-reinstall"),
        attempt_number: 0,
        item: SealedBatchItem {
            item_id: BatchItemId::new("item-reinstall"),
            ordinal: 0,
            mod_id: input.mod_id,
        },
        plan,
        parent_task_id: parent.task_id,
    }
}

fn plan(input: ReinstallBatchItemInput, single_plan_digest: &str) -> BatchPlan {
    BatchPlan {
        plan_schema_version: hmm_core::BATCH_PLAN_SCHEMA_VERSION,
        operation: BatchOperation::Reinstall,
        game_id: GameId::mhw(),
        profile_id: ProfileId::new("default"),
        execution_policy: BatchExecutionPolicy::StopOnFailure,
        items: vec![BatchItemPlan {
            ordinal: 0,
            input_snapshot: BatchItemInput::Reinstall(input.clone()),
            source_revision_id: Some(input.candidate_revision_id),
            installed_revision_id: Some(input.installed_revision_id),
            fact_digest: "fact".to_owned(),
            single_plan_digest: single_plan_digest.to_owned(),
            prerequisite: BatchPreflightDecision {
                status: BatchPreflightStatus::Ready,
                rules_version: Some(1),
                codes: Vec::new(),
            },
            target_claims: vec![BatchTargetClaim {
                target_path: target("nativePC/a.bin"),
                kind: BatchTargetWriteKind::Install,
            }],
            action_summary: BatchActionSummary {
                actions: 1,
                replaced: 1,
                ..BatchActionSummary::default()
            },
            blocking_reasons: Vec::new(),
            warning_codes: Vec::new(),
        }],
        environment_digest: "env".to_owned(),
        prerequisite_rules_version: Some(1),
        resource_limits: BatchResourceLimits::default(),
        resource_usage: BatchResourceUsage {
            item_count: 1,
            target_action_count: 1,
            canonical_bytes: 1,
        },
        global_target_claims_digest: "claims".to_owned(),
        batch_digest: "digest".to_owned(),
        global_blocking_reasons: Vec::new(),
        warning_codes: Vec::new(),
    }
}

fn normalized_request(inputs: Vec<ReinstallBatchItemInput>) -> NormalizedBatchPlanRequest {
    hmm_core::BatchPlanRequest {
        schema_version: hmm_core::BATCH_PLAN_SCHEMA_VERSION,
        operation: BatchOperation::Reinstall,
        game_id: GameId::mhw(),
        profile_id: ProfileId::new("default"),
        execution_policy: BatchExecutionPolicy::StopOnFailure,
        items: inputs.into_iter().map(BatchItemInput::Reinstall).collect(),
    }
    .normalize()
    .expect("normalized reinstall request")
}

fn reinstall_input(
    mod_id: &str,
    installed_revision_id: &str,
    candidate_revision_id: &str,
    replacement_binding_snapshot: Option<ReplacementBindingSnapshot>,
) -> ReinstallBatchItemInput {
    ReinstallBatchItemInput {
        mod_id: ModId::new(mod_id),
        installed_revision_id: ModRevisionId::new(installed_revision_id),
        candidate_revision_id: ModRevisionId::new(candidate_revision_id),
        layer: FileLayer::new("base", 0),
        replacement_binding_snapshot,
    }
}

fn replacement_snapshot(target_id: &str, revision_id: &str) -> ReplacementBindingSnapshot {
    ReplacementBindingSnapshot::new(
        ReplacementBinding::new(
            ReplacementBindingId::parse("binding-a").expect("binding id"),
            ModId::new("mod-a"),
            ProfileId::new("default"),
            ReplacementSourceId::parse("mhw:armor:f_equip:pl121_0000").expect("source id"),
            ReplacementTargetId::parse(target_id).expect("target id"),
            1,
        )
        .expect("binding"),
        Some(ModRevisionId::new(revision_id)),
        "pl121_0000",
        "pl129_0000",
        "f_equip",
        "f_equip",
        ReplacementTargetKind::parse("armor").expect("replacement kind"),
    )
    .expect("binding snapshot")
}

fn commit_success() -> ReinstallCommitResult {
    ReinstallCommitResult {
        manifest: InstallManifest::completed(ProfileId::new("default"), Vec::new()),
    }
}

fn target(path: &str) -> InstallTargetPath {
    InstallTargetPath::parse(path, ["nativePC"]).expect("target")
}
