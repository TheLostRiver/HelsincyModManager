use super::*;

struct NarrowFacts {
    facts: hmm_core::BatchPlanFacts,
    full_reads: AtomicUsize,
    item_reads: AtomicUsize,
    block_second: bool,
}

impl BatchPlanFactsProvider for NarrowFacts {
    fn read_batch_plan_facts(
        &self,
        _: &hmm_core::NormalizedBatchPlanRequest,
    ) -> anyhow::Result<hmm_core::BatchPlanFacts> {
        self.full_reads.fetch_add(1, Ordering::Relaxed);
        Ok(self.facts.clone())
    }

    fn read_batch_item_facts(
        &self,
        request: &hmm_core::NormalizedBatchPlanRequest,
        mod_id: &ModId,
    ) -> anyhow::Result<hmm_core::BatchPlanFacts> {
        assert_eq!(
            request.items.len(),
            self.facts.items.len(),
            "global checks must retain the full selection"
        );
        let call = self.item_reads.fetch_add(1, Ordering::Relaxed);
        let mut facts = self.facts.clone();
        facts.items.retain(|item| &item.mod_id == mod_id);
        if self.block_second && call == 1 {
            facts
                .global_blocking_reasons
                .push(hmm_core::BatchReasonSummary {
                    code: "batch_global_recovery_active".to_owned(),
                    count: 1,
                });
        }
        Ok(facts)
    }
}

fn counted_batch(count: usize) -> (SealedBatch, hmm_core::BatchAttempt, String) {
    let (mut batch, mut attempt, _) = batch();
    let template = batch.plan.items[0].clone();
    batch.request.items = (0..count)
        .map(|i| {
            BatchItemInput::Install(InstallBatchItemInput {
                mod_id: ModId::new(format!("mod-{i:02}")),
                revision_id: ModRevisionId::new(format!("rev-{i:02}")),
                layer: FileLayer::new("default", 1),
                replacement_binding_snapshot: None,
            })
        })
        .collect();
    batch.plan.items = batch
        .request
        .items
        .iter()
        .enumerate()
        .map(|(ordinal, input)| {
            let mut item = template.clone();
            item.ordinal = ordinal;
            item.input_snapshot = input.clone();
            let BatchItemInput::Install(install) = input else {
                unreachable!()
            };
            item.source_revision_id = Some(install.revision_id.clone());
            item.target_claims[0].target_path = InstallTargetPath::parse(
                format!("nativePC/{}.bin", input.mod_id().as_str()),
                ["nativePC"],
            )
            .unwrap();
            item
        })
        .collect();
    batch.plan = hmm_core::build_batch_plan(
        batch.request.clone(),
        facts_from_batch(&batch),
        BatchResourceLimits::default(),
    )
    .unwrap();
    batch.items = batch
        .request
        .items
        .iter()
        .enumerate()
        .map(|(ordinal, input)| SealedBatchItem {
            item_id: hmm_core::BatchItemId::new(format!("item-{ordinal}")),
            ordinal,
            mod_id: input.mod_id().clone(),
        })
        .collect();
    attempt.item_ids = batch
        .items
        .iter()
        .map(|item| item.item_id.clone())
        .collect();
    let token = crate::Sha256BatchTokenCodec::new("secret")
        .unwrap()
        .issue(
            crate::BatchTokenKind::Plan,
            &execution_token_digest(
                &batch.batch_id,
                0,
                &attempt.item_ids,
                &batch.plan.batch_digest,
                &batch.plan.environment_digest,
            ),
            &batch.plan.environment_digest,
            1,
            100,
        )
        .unwrap()
        .token;
    attempt.plan_token_verifier = sha256_hex(token.as_bytes());
    (batch, attempt, token)
}

#[test]
fn runner_reads_whole_batch_once_then_only_current_items() {
    for count in [5, 10, 20] {
        let (batch, attempt, token) = counted_batch(count);
        let facts = Arc::new(NarrowFacts {
            facts: facts_from_batch(&batch),
            full_reads: AtomicUsize::new(0),
            item_reads: AtomicUsize::new(0),
            block_second: false,
        });
        let repository = Arc::new(FakeRepository::default());
        repository
            .seal_batch(BatchSealRequest {
                sealed_batch: &batch,
                initial_attempt: &attempt,
            })
            .unwrap();
        let executor = Arc::new(FakeExecutor {
            executions: Mutex::new(
                (0..count)
                    .map(|_| BatchInstallItemExecution::Succeeded {
                        evidence_health_degraded: false,
                    })
                    .collect(),
            ),
        });
        let runner = BatchInstallTaskRunner::new(
            Arc::new(TaskManager::new()),
            repository,
            executor.clone(),
            facts.clone(),
            Arc::new(RecordingAuditLogWriter::default()),
            Arc::new(FixedClock),
            Arc::new(crate::Sha256BatchTokenCodec::new("secret").unwrap()),
        );
        let result = runner.run(&batch.batch_id, &token).unwrap();
        assert_eq!(result.status, BatchAttemptStatus::Completed);
        assert_eq!(facts.full_reads.load(Ordering::Relaxed), 1);
        assert_eq!(facts.item_reads.load(Ordering::Relaxed), count);
        assert!(executor.executions.lock().unwrap().is_empty());
    }
}

#[test]
fn narrow_reads_still_stop_continue_policy_for_a_new_global_recovery() {
    let (mut batch, attempt, token) = counted_batch(5);
    batch.request.execution_policy = BatchExecutionPolicy::ContinueOnItemFailure;
    batch.plan.execution_policy = BatchExecutionPolicy::ContinueOnItemFailure;
    let facts = Arc::new(NarrowFacts {
        facts: facts_from_batch(&batch),
        full_reads: AtomicUsize::new(0),
        item_reads: AtomicUsize::new(0),
        block_second: true,
    });
    let repository = Arc::new(FakeRepository::default());
    repository
        .seal_batch(BatchSealRequest {
            sealed_batch: &batch,
            initial_attempt: &attempt,
        })
        .unwrap();
    let executor = Arc::new(FakeExecutor {
        executions: Mutex::new(vec![BatchInstallItemExecution::Succeeded {
            evidence_health_degraded: false,
        }]),
    });
    let runner = BatchInstallTaskRunner::new(
        Arc::new(TaskManager::new()),
        repository,
        executor,
        facts.clone(),
        Arc::new(RecordingAuditLogWriter::default()),
        Arc::new(FixedClock),
        Arc::new(crate::Sha256BatchTokenCodec::new("secret").unwrap()),
    );
    let result = runner.run(&batch.batch_id, &token).unwrap();
    assert_eq!(result.summary.succeeded_count, 1);
    assert_eq!(result.summary.blocked_count, 1);
    assert_eq!(result.summary.skipped_count, 3);
    assert_eq!(facts.item_reads.load(Ordering::Relaxed), 2);
}
