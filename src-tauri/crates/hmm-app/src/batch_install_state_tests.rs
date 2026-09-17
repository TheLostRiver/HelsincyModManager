use super::*;
use hmm_ports::{GameRunningDetector, GameRunningStatus};

struct ObservedExecution {
    trace: Arc<Mutex<Vec<String>>>,
}

impl BatchInstallItemExecutor for ObservedExecution {
    fn execute(&self, request: BatchInstallItemRequest) -> BatchInstallItemExecution {
        self.trace
            .lock()
            .unwrap()
            .push(format!("execute:{}", request.item.mod_id.as_str()));
        BatchInstallItemExecution::Succeeded {
            evidence_health_degraded: false,
        }
    }
}

struct StateObserver {
    trace: Arc<Mutex<Vec<String>>>,
    repository: Arc<FakeRepository>,
}

impl crate::ModInstallationStateObserver for StateObserver {
    fn state_changed(
        &self,
        task_id: &str,
        game_id: &GameId,
        profile_id: &ProfileId,
        mod_id: &ModId,
    ) {
        assert!(!task_id.is_empty());
        assert_eq!(game_id, &GameId::mhw());
        assert_eq!(profile_id, &ProfileId::new("default"));
        // Result recording was attempted before observation, including failure paths.
        assert!(
            self.repository
                .record_item_result_calls
                .load(Ordering::Relaxed)
                > 0
        );
        self.trace
            .lock()
            .unwrap()
            .push(format!("observe:{}", mod_id.as_str()));
    }
}

#[test]
fn state_observation_follows_each_transaction_even_if_its_result_journal_fails() {
    for fail_journal in [false, true] {
        let (batch, attempt, token) = batch();
        let repository = Arc::new(FakeRepository::default());
        repository
            .seal_batch(BatchSealRequest {
                sealed_batch: &batch,
                initial_attempt: &attempt,
            })
            .unwrap();
        repository
            .fail_record_item_result
            .store(fail_journal, Ordering::Relaxed);
        let trace = Arc::new(Mutex::new(Vec::new()));
        let runner = BatchInstallTaskRunner::new(
            Arc::new(TaskManager::new()),
            repository.clone(),
            Arc::new(ObservedExecution {
                trace: trace.clone(),
            }),
            Arc::new(StaticFacts(facts_from_batch(&batch))),
            Arc::new(RecordingAuditLogWriter::default()),
            Arc::new(FixedClock),
            Arc::new(crate::Sha256BatchTokenCodec::new("secret").unwrap()),
        )
        .with_state_observer(Arc::new(StateObserver {
            trace: trace.clone(),
            repository,
        }));
        let result = runner.run(&batch.batch_id, &token);
        if fail_journal {
            assert_eq!(result, Err(BatchInstallRunError::JournalUnavailable));
            assert_eq!(*trace.lock().unwrap(), ["execute:a", "observe:a"]);
        } else {
            assert_eq!(result.unwrap().status, BatchAttemptStatus::Completed);
            assert_eq!(
                *trace.lock().unwrap(),
                ["execute:a", "observe:a", "execute:b", "observe:b"]
            );
        }
    }
}

struct PerItemPlanner;
impl crate::ImportedModInstallPlanner for PerItemPlanner {
    fn build_imported_mod_install_plan(
        &self,
        _: crate::BuildImportedModInstallPlanRequest,
    ) -> Result<crate::ImportedModInstallPreflight, crate::InstallPlanningError> {
        panic!("batch must use sealed revisions")
    }
    fn build_imported_mod_revision_install_plan(
        &self,
        _: &GameId,
        mod_id: &ModId,
        _: &ModRevisionId,
        layer: &FileLayer,
    ) -> Result<crate::ImportedModInstallPreflight, crate::InstallPlanningError> {
        Ok(crate::ImportedModInstallPreflight {
            plan: InstallPlan::from_providers(vec![InstallFileProvider::new(
                mod_id.clone(),
                PackageFileId::new(format!("source-{}", mod_id.as_str())),
                InstallTargetPath::parse(format!("nativepc/{}", mod_id.as_str()), ["nativepc"])
                    .unwrap(),
                layer.clone(),
            )]),
            prerequisite_decision: ready_install_prerequisite(),
        })
    }
    fn prerequisite_decision(&self, _: &GameId) -> crate::GamePrerequisiteDecision {
        ready_install_prerequisite()
    }
}

struct ChangingGameState {
    queries: AtomicUsize,
    next: GameRunningStatus,
}
impl GameRunningDetector for ChangingGameState {
    fn game_running_status(&self, _: &GameId) -> GameRunningStatus {
        if self.queries.fetch_add(1, Ordering::Relaxed) == 0 {
            GameRunningStatus::NotRunning
        } else {
            self.next
        }
    }
}

#[test]
fn batch_checks_game_again_before_the_second_items_write() {
    for next in [GameRunningStatus::Running, GameRunningStatus::Unknown] {
        let (batch, attempt, token) = batch();
        let repository = Arc::new(FakeRepository::default());
        repository
            .seal_batch(BatchSealRequest {
                sealed_batch: &batch,
                initial_attempt: &attempt,
            })
            .unwrap();
        let detector = Arc::new(ChangingGameState {
            queries: AtomicUsize::new(0),
            next,
        });
        let game_files = Arc::new(TransactionGameFiles::default());
        let recovery = Arc::new(TransactionRecoveryRepository::default());
        let manifest = Arc::new(TransactionManifestRepository::new(false));
        let commit = crate::InstallCommitService::new_with_recovery_records(
            Arc::new(StaticInstallSource),
            game_files.clone(),
            Arc::new(TransactionBackupStore),
            manifest.clone(),
            recovery.clone(),
        )
        .with_game_running_detector(detector.clone());
        let tasks = Arc::new(TaskManager::new());
        let item_runner = Arc::new(InstallTaskRunner::with_write_coordination(
            tasks.clone(),
            Arc::new(PerItemPlanner),
            Arc::new(commit),
            Arc::new(RecordingAuditLogWriter::default()),
            Arc::new(FixedClock),
            Arc::new(crate::GameProfileWriteLockRegistry::default()),
            Arc::new(crate::install_task::AllowInstallWriteAdmission),
            crate::replacement_selection_test_support::noop_selection_repository(),
        ));
        let runner = BatchInstallTaskRunner::new(
            tasks.clone(),
            repository.clone(),
            Arc::new(InstallTaskBatchItemExecutor::new(item_runner, tasks)),
            Arc::new(StaticFacts(facts_from_batch(&batch))),
            Arc::new(RecordingAuditLogWriter::default()),
            Arc::new(FixedClock),
            Arc::new(crate::Sha256BatchTokenCodec::new("secret").unwrap()),
        );
        assert_eq!(
            runner.run(&batch.batch_id, &token).unwrap().status,
            BatchAttemptStatus::CompletedWithErrors
        );
        assert_eq!(detector.queries.load(Ordering::Relaxed), 2);
        assert!(game_files.file_bytes("nativepc/a").is_some());
        assert!(game_files.file_bytes("nativepc/b").is_none());
        assert!(manifest
            .load_manifest(&ProfileId::new("default"))
            .unwrap()
            .unwrap()
            .entries
            .iter()
            .all(|entry| entry.mod_id.as_str() == "a"));
        assert!(recovery
            .history()
            .iter()
            .all(|record| record.mod_id.as_str() == "a"));
        let results = repository.list_item_results(&batch.batch_id, 0).unwrap();
        assert_eq!(results[0].status, BatchItemStatus::Succeeded);
        assert_eq!(results[1].status, BatchItemStatus::Blocked);
    }
}
