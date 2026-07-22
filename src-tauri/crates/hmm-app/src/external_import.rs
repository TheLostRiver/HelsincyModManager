use std::sync::Arc;

use hmm_core::{
    ExternalImportBatch, ExternalImportBatchId, ExternalImportBatchImportStatus,
    ExternalImportCandidate, ExternalImportResourceBudget, ExternalImportScanStatus,
    ExternalImportSource, ExternalImportSourceId,
};
use hmm_ports::{
    AppClock, CancellationToken, ExternalImportBatchRepository, ExternalImportScanRequest,
    ExternalImportScanner, ExternalImportSourceRegistry,
};
use thiserror::Error;
use uuid::Uuid;

use crate::{TaskKind, TaskManager, TaskManagerError, TaskProgressEvent, TaskStarted, TaskStatus};

pub const DEFAULT_EXTERNAL_IMPORT_PREVIEW_LIMIT: usize = 50;
pub const MAX_EXTERNAL_IMPORT_PREVIEW_LIMIT: usize = 100;

pub const EXTERNAL_IMPORT_SCAN_QUEUED_PHASE: &str = "external_import.scan.queued";
pub const EXTERNAL_IMPORT_SCAN_DISCOVERING_PHASE: &str = "external_import.scan.discovering";
pub const EXTERNAL_IMPORT_SCAN_FINGERPRINTING_PHASE: &str = "external_import.scan.fingerprinting";
pub const EXTERNAL_IMPORT_SCAN_COMPLETED_PHASE: &str = "external_import.scan.completed";
pub const EXTERNAL_IMPORT_SCAN_FAILED_PHASE: &str = "external_import.scan.failed";
pub const EXTERNAL_IMPORT_SCAN_CANCELLED_PHASE: &str = "external_import.scan.cancelled";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalImportScanLaunch {
    pub task: TaskStarted,
    pub batch_id: ExternalImportBatchId,
    source: ExternalImportSource,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalImportPreviewPage {
    pub batch: ExternalImportBatch,
    pub candidates: Vec<ExternalImportCandidate>,
    pub total_count: usize,
    pub next_offset: Option<usize>,
}

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum ExternalImportScanError {
    #[error("external import source is unavailable")]
    SourceUnavailable,
    #[error("external import task is unavailable")]
    TaskUnavailable,
    #[error("external import batch is unavailable")]
    BatchUnavailable,
    #[error("external import scan failed")]
    ScanFailed,
    #[error("external import preview request is invalid")]
    PreviewRequestInvalid,
    #[error("external import clock is unavailable")]
    ClockUnavailable,
}

impl ExternalImportScanError {
    pub fn error_code(self) -> &'static str {
        match self {
            Self::SourceUnavailable => "external_import_source_unavailable",
            Self::TaskUnavailable => "external_import_task_unavailable",
            Self::BatchUnavailable => "external_import_batch_unavailable",
            Self::ScanFailed => "external_import_scan_failed",
            Self::PreviewRequestInvalid => "external_import_preview_request_invalid",
            Self::ClockUnavailable => "external_import_clock_unavailable",
        }
    }
}

pub struct ExternalImportScanService {
    task_manager: Arc<TaskManager>,
    source_registry: Arc<dyn ExternalImportSourceRegistry>,
    scanner: Arc<dyn ExternalImportScanner>,
    batch_repository: Arc<dyn ExternalImportBatchRepository>,
    clock: Arc<dyn AppClock>,
    resource_budget: ExternalImportResourceBudget,
}

impl ExternalImportScanService {
    pub fn new(
        task_manager: Arc<TaskManager>,
        source_registry: Arc<dyn ExternalImportSourceRegistry>,
        scanner: Arc<dyn ExternalImportScanner>,
        batch_repository: Arc<dyn ExternalImportBatchRepository>,
        clock: Arc<dyn AppClock>,
    ) -> Self {
        Self {
            task_manager,
            source_registry,
            scanner,
            batch_repository,
            clock,
            resource_budget: ExternalImportResourceBudget::default(),
        }
    }

    pub fn with_resource_budget(mut self, resource_budget: ExternalImportResourceBudget) -> Self {
        self.resource_budget = resource_budget;
        self
    }

    pub fn start_scan(
        &self,
        source_id: ExternalImportSourceId,
    ) -> Result<ExternalImportScanLaunch, ExternalImportScanError> {
        let registration = self
            .source_registry
            .resolve_source(&source_id)
            .map_err(|_| ExternalImportScanError::SourceUnavailable)?
            .ok_or(ExternalImportScanError::SourceUnavailable)?;
        let created_at_unix_millis = self.now_unix_millis()?;
        let task = self
            .task_manager
            .create_task(TaskKind::ModImport)
            .map_err(map_task_manager_error)?;
        let batch = ExternalImportBatch {
            batch_id: ExternalImportBatchId::new(format!(
                "external-import-batch-{}",
                Uuid::new_v4()
            )),
            adapter_id: registration.source.adapter_id.clone(),
            source_fingerprint: registration.source_fingerprint,
            scan_status: ExternalImportScanStatus::Pending,
            import_status: ExternalImportBatchImportStatus::Pending,
            created_at_unix_millis,
        };

        if self.batch_repository.create_batch(&batch).is_err() {
            let _ = self.task_manager.fail_task(&task.task_id);
            return Err(ExternalImportScanError::BatchUnavailable);
        }

        Ok(ExternalImportScanLaunch {
            task: TaskStarted {
                task_id: task.task_id,
                kind: task.kind,
                status: task.status,
            },
            batch_id: batch.batch_id,
            source: registration.source,
        })
    }

    /// Closes a launch that could not be handed to the task runner.
    ///
    /// This is used when the queued event cannot be emitted. The command has not returned a
    /// task/batch identity in that case, so leaving its pending durable record would make it
    /// unreachable to the user.
    pub fn abort_queued_scan(
        &self,
        launch: &ExternalImportScanLaunch,
    ) -> Result<(), ExternalImportScanError> {
        let terminal_scan_status = match self.task_manager.task_status(&launch.task.task_id) {
            Some(TaskStatus::Queued | TaskStatus::Running) => {
                self.task_manager
                    .fail_task(&launch.task.task_id)
                    .map_err(map_task_manager_error)?;
                ExternalImportScanStatus::Failed
            }
            Some(TaskStatus::Failed) => ExternalImportScanStatus::Failed,
            Some(TaskStatus::Cancelled) => ExternalImportScanStatus::Cancelled,
            _ => return Err(ExternalImportScanError::TaskUnavailable),
        };
        let mut batch = self
            .batch_repository
            .get_batch(&launch.batch_id)
            .map_err(|_| ExternalImportScanError::BatchUnavailable)?
            .ok_or(ExternalImportScanError::BatchUnavailable)?;
        if batch.scan_status == terminal_scan_status {
            return Ok(());
        }
        if batch.scan_status != ExternalImportScanStatus::Pending {
            return Err(ExternalImportScanError::BatchUnavailable);
        }

        batch.scan_status = terminal_scan_status;
        self.batch_repository
            .update_batch(&batch)
            .map_err(|_| ExternalImportScanError::BatchUnavailable)?;
        Ok(())
    }

    /// Runs a scan outside of database and game/profile locks.
    ///
    /// The caller owns thread scheduling and forwards the resulting aggregate-only events.
    pub fn run_scan(
        &self,
        launch: ExternalImportScanLaunch,
    ) -> Result<Vec<TaskProgressEvent>, ExternalImportScanError> {
        let mut batch = self
            .batch_repository
            .get_batch(&launch.batch_id)
            .map_err(|_| ExternalImportScanError::BatchUnavailable)?
            .ok_or(ExternalImportScanError::BatchUnavailable)?;

        if self.is_cancelled(&launch.task.task_id) {
            return self.finish_cancelled(&launch, &mut batch);
        }

        match self.task_manager.start_task(&launch.task.task_id) {
            Ok(_) => {}
            Err(_) if self.is_cancelled(&launch.task.task_id) => {
                return self.finish_cancelled(&launch, &mut batch)
            }
            Err(error) => return Err(map_task_manager_error(error)),
        }
        batch.scan_status = ExternalImportScanStatus::Running;
        self.batch_repository
            .update_batch(&batch)
            .map_err(|_| ExternalImportScanError::BatchUnavailable)?;

        let mut events = vec![scan_event(
            &launch,
            TaskStatus::Running,
            EXTERNAL_IMPORT_SCAN_DISCOVERING_PHASE,
        )];
        events.push(scan_event(
            &launch,
            TaskStatus::Running,
            EXTERNAL_IMPORT_SCAN_FINGERPRINTING_PHASE,
        ));

        let cancellation_token = TaskManagerCancellationToken {
            task_manager: Arc::clone(&self.task_manager),
            task_id: launch.task.task_id.clone(),
        };
        let scan_result = self.scanner.scan(ExternalImportScanRequest {
            source: &launch.source,
            batch: &batch,
            resource_budget: &self.resource_budget,
            cancellation_token: &cancellation_token,
        });

        match scan_result {
            Ok(scan_result) => {
                if cancellation_token.is_cancelled() {
                    events.extend(self.finish_cancelled(&launch, &mut batch)?);
                    return Ok(events);
                }

                // Candidate traversal and hashing are cancellable. Once this short durable
                // transition starts, cancellation must not produce a batch/task split state.
                if let Err(error) = self
                    .task_manager
                    .block_task_cancellation(&launch.task.task_id)
                {
                    if self.is_cancelled(&launch.task.task_id) {
                        events.extend(self.finish_cancelled(&launch, &mut batch)?);
                        return Ok(events);
                    }
                    return Err(map_task_manager_error(error));
                }

                batch.scan_status = ExternalImportScanStatus::Completed;
                if self
                    .batch_repository
                    .save_scan_result(&batch, &scan_result.candidates)
                    .is_err()
                {
                    events.extend(self.finish_failed(&launch, &mut batch)?);
                    return Ok(events);
                }

                self.task_manager
                    .complete_task(&launch.task.task_id)
                    .map_err(map_task_manager_error)?;
                events.push(scan_event(
                    &launch,
                    TaskStatus::Completed,
                    EXTERNAL_IMPORT_SCAN_COMPLETED_PHASE,
                ));
                Ok(events)
            }
            Err(_) => {
                let terminal_events = if cancellation_token.is_cancelled() {
                    self.finish_cancelled(&launch, &mut batch)?
                } else {
                    self.finish_failed(&launch, &mut batch)?
                };
                events.extend(terminal_events);
                Ok(events)
            }
        }
    }

    pub fn get_preview(
        &self,
        batch_id: &ExternalImportBatchId,
        offset: usize,
        limit: usize,
    ) -> Result<ExternalImportPreviewPage, ExternalImportScanError> {
        if !(1..=MAX_EXTERNAL_IMPORT_PREVIEW_LIMIT).contains(&limit) {
            return Err(ExternalImportScanError::PreviewRequestInvalid);
        }

        let batch = self
            .batch_repository
            .get_batch(batch_id)
            .map_err(|_| ExternalImportScanError::BatchUnavailable)?
            .ok_or(ExternalImportScanError::BatchUnavailable)?;
        let page = self
            .batch_repository
            .list_candidates_page(batch_id, offset, limit)
            .map_err(|_| ExternalImportScanError::BatchUnavailable)?;

        Ok(ExternalImportPreviewPage {
            batch,
            candidates: page.candidates,
            total_count: page.total_count,
            next_offset: page.next_offset,
        })
    }

    fn finish_cancelled(
        &self,
        launch: &ExternalImportScanLaunch,
        batch: &mut ExternalImportBatch,
    ) -> Result<Vec<TaskProgressEvent>, ExternalImportScanError> {
        batch.scan_status = ExternalImportScanStatus::Cancelled;
        self.batch_repository
            .update_batch(batch)
            .map_err(|_| ExternalImportScanError::BatchUnavailable)?;

        if matches!(
            self.task_manager.task_status(&launch.task.task_id),
            Some(TaskStatus::Queued | TaskStatus::Running)
        ) {
            self.task_manager
                .cancel_task(&launch.task.task_id)
                .map_err(map_task_manager_error)?;
        }

        Ok(vec![scan_event(
            launch,
            TaskStatus::Cancelled,
            EXTERNAL_IMPORT_SCAN_CANCELLED_PHASE,
        )])
    }

    fn finish_failed(
        &self,
        launch: &ExternalImportScanLaunch,
        batch: &mut ExternalImportBatch,
    ) -> Result<Vec<TaskProgressEvent>, ExternalImportScanError> {
        batch.scan_status = ExternalImportScanStatus::Failed;
        let update_result = self.batch_repository.update_batch(batch);

        if self.is_cancelled(&launch.task.task_id) {
            return self.finish_cancelled(launch, batch);
        }

        self.task_manager
            .fail_task(&launch.task.task_id)
            .map_err(map_task_manager_error)?;
        if update_result.is_err() {
            return Err(ExternalImportScanError::BatchUnavailable);
        }
        let mut event = scan_event(
            launch,
            TaskStatus::Failed,
            EXTERNAL_IMPORT_SCAN_FAILED_PHASE,
        );
        event.error = Some(ExternalImportScanError::ScanFailed.error_code().to_owned());
        Ok(vec![event])
    }

    fn now_unix_millis(&self) -> Result<u64, ExternalImportScanError> {
        self.clock
            .now_unix_millis()
            .ok()
            .and_then(|value| u64::try_from(value).ok())
            .ok_or(ExternalImportScanError::ClockUnavailable)
    }

    fn is_cancelled(&self, task_id: &str) -> bool {
        self.task_manager.task_status(task_id) == Some(TaskStatus::Cancelled)
    }
}

fn map_task_manager_error(_error: TaskManagerError) -> ExternalImportScanError {
    ExternalImportScanError::TaskUnavailable
}

fn scan_event(
    launch: &ExternalImportScanLaunch,
    status: TaskStatus,
    phase: &'static str,
) -> TaskProgressEvent {
    let mut event =
        TaskProgressEvent::new(launch.task.task_id.clone(), launch.task.kind, status, phase);
    event.result_ref = Some(launch.batch_id.as_str().to_owned());
    event
}

struct TaskManagerCancellationToken {
    task_manager: Arc<TaskManager>,
    task_id: String,
}

impl CancellationToken for TaskManagerCancellationToken {
    fn is_cancelled(&self) -> bool {
        self.task_manager.task_status(&self.task_id) == Some(TaskStatus::Cancelled)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::{bail, Result};
    use hmm_core::{
        ExternalImportAdapterId, ExternalImportCandidateId, ExternalImportCandidateStatus,
        ExternalImportConflictKind, ExternalImportMetadataHint, ExternalImportResourceUsage,
    };
    use hmm_ports::{
        ExternalImportBatchRepository, ExternalImportCandidatePage, ExternalImportScanResult,
        ExternalImportSelectionCompareAndSwapRequest, ExternalImportSelectionCompareAndSwapResult,
        ExternalImportSourceRegistration,
    };
    use std::collections::BTreeMap;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::Mutex;

    #[test]
    fn successful_scan_persists_a_page_and_emits_aggregate_task_events() {
        let (service, task_manager, repository, scanner, source) = make_service(false);
        let launch = service
            .start_scan(source.source_id.clone())
            .expect("start scan");
        let task_id = launch.task.task_id.clone();
        let batch_id = launch.batch_id.clone();

        let events = service.run_scan(launch).expect("scan succeeds");
        let preview = service
            .get_preview(&batch_id, 0, 50)
            .expect("preview is readable");

        assert_eq!(scanner.calls.load(Ordering::SeqCst), 1);
        assert_eq!(
            task_manager.task_status(&task_id),
            Some(TaskStatus::Completed)
        );
        assert_eq!(
            preview.batch.scan_status,
            ExternalImportScanStatus::Completed
        );
        assert_eq!(preview.total_count, 1);
        assert_eq!(preview.candidates.len(), 1);
        assert_eq!(
            events
                .iter()
                .map(|event| event.phase.as_str())
                .collect::<Vec<_>>(),
            [
                EXTERNAL_IMPORT_SCAN_DISCOVERING_PHASE,
                EXTERNAL_IMPORT_SCAN_FINGERPRINTING_PHASE,
                EXTERNAL_IMPORT_SCAN_COMPLETED_PHASE,
            ]
        );
        assert!(events.iter().all(|event| {
            event.task_id == task_id
                && event.result_ref.as_deref() == Some(batch_id.as_str())
                && event.message.is_none()
                && event.error.is_none()
        }));
        assert_eq!(
            repository
                .get_batch(&batch_id)
                .expect("read batch")
                .expect("batch exists")
                .scan_status,
            ExternalImportScanStatus::Completed
        );
    }

    #[test]
    fn scanner_failure_marks_the_task_and_batch_failed() {
        let (service, task_manager, repository, scanner, source) = make_service(true);
        let launch = service
            .start_scan(source.source_id.clone())
            .expect("start scan");
        let task_id = launch.task.task_id.clone();
        let batch_id = launch.batch_id.clone();

        let events = service
            .run_scan(launch)
            .expect("failure is represented as events");

        assert_eq!(scanner.calls.load(Ordering::SeqCst), 1);
        assert_eq!(task_manager.task_status(&task_id), Some(TaskStatus::Failed));
        assert_eq!(
            repository
                .get_batch(&batch_id)
                .expect("read batch")
                .expect("batch exists")
                .scan_status,
            ExternalImportScanStatus::Failed
        );
        assert_eq!(events.len(), 3);
        let failure = events.last().expect("terminal failure event");
        assert_eq!(failure.phase, EXTERNAL_IMPORT_SCAN_FAILED_PHASE);
        assert_eq!(
            failure.error.as_deref(),
            Some("external_import_scan_failed")
        );
        assert_eq!(failure.result_ref.as_deref(), Some(batch_id.as_str()));
    }

    #[test]
    fn cancelling_before_the_runner_starts_leaves_the_source_unscanned() {
        let (service, task_manager, repository, scanner, source) = make_service(false);
        let launch = service
            .start_scan(source.source_id.clone())
            .expect("start scan");
        let task_id = launch.task.task_id.clone();
        let batch_id = launch.batch_id.clone();
        task_manager
            .cancel_task(&task_id)
            .expect("cancel queued task");

        let events = service.run_scan(launch).expect("cancel is handled");

        assert_eq!(scanner.calls.load(Ordering::SeqCst), 0);
        assert_eq!(
            task_manager.task_status(&task_id),
            Some(TaskStatus::Cancelled)
        );
        assert_eq!(
            repository
                .get_batch(&batch_id)
                .expect("read batch")
                .expect("batch exists")
                .scan_status,
            ExternalImportScanStatus::Cancelled
        );
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].phase, EXTERNAL_IMPORT_SCAN_CANCELLED_PHASE);
        assert_eq!(events[0].result_ref.as_deref(), Some(batch_id.as_str()));
    }

    #[test]
    fn aborting_a_queued_launch_marks_its_batch_and_task_failed_without_scanning() {
        let (service, task_manager, repository, scanner, source) = make_service(false);
        let launch = service
            .start_scan(source.source_id.clone())
            .expect("start scan");
        let task_id = launch.task.task_id.clone();
        let batch_id = launch.batch_id.clone();

        service
            .abort_queued_scan(&launch)
            .expect("queued launch is closed");

        assert_eq!(scanner.calls.load(Ordering::SeqCst), 0);
        assert_eq!(task_manager.task_status(&task_id), Some(TaskStatus::Failed));
        assert_eq!(
            repository
                .get_batch(&batch_id)
                .expect("read batch")
                .expect("batch exists")
                .scan_status,
            ExternalImportScanStatus::Failed
        );
    }

    #[test]
    fn aborting_a_launch_that_was_cancelled_before_queued_event_delivery_keeps_both_states_cancelled(
    ) {
        let (service, task_manager, repository, scanner, source) = make_service(false);
        let launch = service
            .start_scan(source.source_id.clone())
            .expect("start scan");
        let task_id = launch.task.task_id.clone();
        let batch_id = launch.batch_id.clone();
        task_manager
            .cancel_task(&task_id)
            .expect("cancel queued task");

        service
            .abort_queued_scan(&launch)
            .expect("cancelled launch is closed");

        assert_eq!(scanner.calls.load(Ordering::SeqCst), 0);
        assert_eq!(
            task_manager.task_status(&task_id),
            Some(TaskStatus::Cancelled)
        );
        assert_eq!(
            repository
                .get_batch(&batch_id)
                .expect("read batch")
                .expect("batch exists")
                .scan_status,
            ExternalImportScanStatus::Cancelled
        );
    }

    #[test]
    fn scan_result_commit_rejects_late_cancellation_and_keeps_terminal_state_consistent() {
        let (service, task_manager, repository, _scanner, source) = make_service(false);
        let launch = service
            .start_scan(source.source_id.clone())
            .expect("start scan");
        let task_id = launch.task.task_id.clone();
        let batch_id = launch.batch_id.clone();
        repository.configure_cancellation_attempt(Arc::clone(&task_manager), task_id.clone());

        let events = service.run_scan(launch).expect("scan succeeds");

        assert!(repository
            .cancellation_attempt_was_rejected()
            .expect("cancellation attempt was recorded"));
        assert_eq!(
            task_manager.task_status(&task_id),
            Some(TaskStatus::Completed)
        );
        assert_eq!(
            repository
                .get_batch(&batch_id)
                .expect("read batch")
                .expect("batch exists")
                .scan_status,
            ExternalImportScanStatus::Completed
        );
        assert_eq!(
            events.last().expect("terminal event").phase,
            EXTERNAL_IMPORT_SCAN_COMPLETED_PHASE
        );
    }

    fn make_service(
        scanner_fails: bool,
    ) -> (
        ExternalImportScanService,
        Arc<TaskManager>,
        Arc<FakeBatchRepository>,
        Arc<FakeScanner>,
        ExternalImportSource,
    ) {
        let source = ExternalImportSource {
            source_id: ExternalImportSourceId::new("external-import-source-fixture"),
            adapter_id: ExternalImportAdapterId::new("hunting_box_directory_v1"),
            display_label: "Hunting Box directory".to_owned(),
            expires_at_unix_millis: 10_000,
        };
        let task_manager = Arc::new(TaskManager::new());
        let repository = Arc::new(FakeBatchRepository::new());
        let scanner = Arc::new(FakeScanner::new(scanner_fails));
        let service = ExternalImportScanService::new(
            Arc::clone(&task_manager),
            Arc::new(FakeSourceRegistry {
                source: source.clone(),
            }),
            scanner.clone(),
            repository.clone(),
            Arc::new(FixedClock),
        );

        (service, task_manager, repository, scanner, source)
    }

    struct FixedClock;

    impl AppClock for FixedClock {
        fn now_unix_millis(&self) -> Result<u128> {
            Ok(1)
        }
    }

    struct FakeSourceRegistry {
        source: ExternalImportSource,
    }

    impl ExternalImportSourceRegistry for FakeSourceRegistry {
        fn resolve_source(
            &self,
            source_id: &ExternalImportSourceId,
        ) -> Result<Option<ExternalImportSourceRegistration>> {
            Ok(
                (source_id == &self.source.source_id).then(|| ExternalImportSourceRegistration {
                    source: self.source.clone(),
                    source_fingerprint: "private-source-fingerprint".to_owned(),
                }),
            )
        }
    }

    struct FakeScanner {
        calls: AtomicUsize,
        fails: bool,
    }

    impl FakeScanner {
        fn new(fails: bool) -> Self {
            Self {
                calls: AtomicUsize::new(0),
                fails,
            }
        }
    }

    impl ExternalImportScanner for FakeScanner {
        fn scan(&self, request: ExternalImportScanRequest<'_>) -> Result<ExternalImportScanResult> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            if self.fails {
                bail!("fixture scanner failed");
            }
            if request.cancellation_token.is_cancelled() {
                bail!("fixture scanner cancelled");
            }
            let candidate = ExternalImportCandidate {
                batch_id: request.batch.batch_id.clone(),
                candidate_id: ExternalImportCandidateId::new("external-import-candidate-fixture"),
                source_item_key_hash: "private-source-item-key".to_owned(),
                content_fingerprint: "sha256:private-content".to_owned(),
                metadata_hint: ExternalImportMetadataHint {
                    display_name: Some("Fixture Mod".to_owned()),
                    author: None,
                    version: None,
                    source_mod_type: None,
                },
                resource_usage: ExternalImportResourceUsage {
                    file_count: 1,
                    source_bytes: 3,
                    materialization_bytes: 3,
                },
                preview_status: ExternalImportCandidateStatus::Ready,
                conflict_kind: ExternalImportConflictKind::None,
            };
            Ok(ExternalImportScanResult {
                candidates: vec![candidate],
                observed_resource_usage: ExternalImportResourceUsage {
                    file_count: 1,
                    source_bytes: 3,
                    materialization_bytes: 3,
                },
            })
        }
    }

    #[derive(Default)]
    struct FakeRepositoryState {
        batches: BTreeMap<String, ExternalImportBatch>,
        candidates: BTreeMap<String, Vec<ExternalImportCandidate>>,
    }

    struct FakeBatchRepository {
        state: Mutex<FakeRepositoryState>,
        cancellation_task_manager: Mutex<Option<Arc<TaskManager>>>,
        cancellation_task_id: Mutex<Option<String>>,
        cancellation_attempt_rejected: AtomicBool,
        cancellation_attempted: AtomicBool,
    }

    impl FakeBatchRepository {
        fn new() -> Self {
            Self {
                state: Mutex::new(FakeRepositoryState::default()),
                cancellation_task_manager: Mutex::new(None),
                cancellation_task_id: Mutex::new(None),
                cancellation_attempt_rejected: AtomicBool::new(false),
                cancellation_attempted: AtomicBool::new(false),
            }
        }

        fn configure_cancellation_attempt(&self, task_manager: Arc<TaskManager>, task_id: String) {
            *self
                .cancellation_task_manager
                .lock()
                .expect("cancellation task manager lock") = Some(task_manager);
            *self
                .cancellation_task_id
                .lock()
                .expect("cancellation task id lock") = Some(task_id);
        }

        fn cancellation_attempt_was_rejected(&self) -> Option<bool> {
            self.cancellation_attempted
                .load(Ordering::SeqCst)
                .then(|| self.cancellation_attempt_rejected.load(Ordering::SeqCst))
        }

        fn save_candidates(
            &self,
            batch: &ExternalImportBatch,
            candidates: &[ExternalImportCandidate],
        ) {
            let mut state = self.state.lock().expect("repository state lock");
            state
                .batches
                .insert(batch.batch_id.as_str().to_owned(), batch.clone());
            state
                .candidates
                .insert(batch.batch_id.as_str().to_owned(), candidates.to_vec());
        }
    }

    impl ExternalImportBatchRepository for FakeBatchRepository {
        fn create_batch(&self, batch: &ExternalImportBatch) -> Result<()> {
            self.state
                .lock()
                .expect("repository state lock")
                .batches
                .insert(batch.batch_id.as_str().to_owned(), batch.clone());
            Ok(())
        }

        fn get_batch(
            &self,
            batch_id: &ExternalImportBatchId,
        ) -> Result<Option<ExternalImportBatch>> {
            Ok(self
                .state
                .lock()
                .expect("repository state lock")
                .batches
                .get(batch_id.as_str())
                .cloned())
        }

        fn update_batch(&self, batch: &ExternalImportBatch) -> Result<()> {
            let mut state = self.state.lock().expect("repository state lock");
            if !state.batches.contains_key(batch.batch_id.as_str()) {
                bail!("batch is unavailable");
            }
            state
                .batches
                .insert(batch.batch_id.as_str().to_owned(), batch.clone());
            Ok(())
        }

        fn save_scan_result(
            &self,
            batch: &ExternalImportBatch,
            candidates: &[ExternalImportCandidate],
        ) -> Result<()> {
            let task_manager = self
                .cancellation_task_manager
                .lock()
                .expect("cancellation task manager lock")
                .clone();
            let task_id = self
                .cancellation_task_id
                .lock()
                .expect("cancellation task id lock")
                .clone();
            if let (Some(task_manager), Some(task_id)) = (task_manager, task_id) {
                self.cancellation_attempted.store(true, Ordering::SeqCst);
                self.cancellation_attempt_rejected.store(
                    task_manager.cancel_task(&task_id).is_err(),
                    Ordering::SeqCst,
                );
            }
            self.save_candidates(batch, candidates);
            Ok(())
        }

        fn replace_candidates(
            &self,
            batch_id: &ExternalImportBatchId,
            candidates: &[ExternalImportCandidate],
        ) -> Result<()> {
            let batch = self
                .get_batch(batch_id)?
                .ok_or_else(|| anyhow::anyhow!("batch is unavailable"))?;
            self.save_candidates(&batch, candidates);
            Ok(())
        }

        fn list_candidates(
            &self,
            batch_id: &ExternalImportBatchId,
        ) -> Result<Vec<ExternalImportCandidate>> {
            Ok(self
                .state
                .lock()
                .expect("repository state lock")
                .candidates
                .get(batch_id.as_str())
                .cloned()
                .unwrap_or_default())
        }

        fn list_candidates_page(
            &self,
            batch_id: &ExternalImportBatchId,
            offset: usize,
            limit: usize,
        ) -> Result<ExternalImportCandidatePage> {
            let candidates = self.list_candidates(batch_id)?;
            let total_count = candidates.len();
            let candidates = candidates
                .into_iter()
                .skip(offset)
                .take(limit)
                .collect::<Vec<_>>();
            let next_offset = offset
                .checked_add(candidates.len())
                .filter(|next_offset| *next_offset < total_count);
            Ok(ExternalImportCandidatePage {
                candidates,
                total_count,
                next_offset,
            })
        }

        fn create_selection(&self, _selection: &hmm_core::ExternalImportSelection) -> Result<()> {
            bail!("selection is not used by scan tests")
        }

        fn get_selection(
            &self,
            _selection_id: &hmm_core::ExternalImportSelectionId,
        ) -> Result<Option<hmm_core::ExternalImportSelection>> {
            bail!("selection is not used by scan tests")
        }

        fn compare_and_swap_selection(
            &self,
            _request: ExternalImportSelectionCompareAndSwapRequest<'_>,
        ) -> Result<ExternalImportSelectionCompareAndSwapResult> {
            bail!("selection is not used by scan tests")
        }

        fn append_item_results(
            &self,
            _batch_id: &ExternalImportBatchId,
            _results: &[hmm_core::ExternalImportItemResult],
        ) -> Result<()> {
            bail!("item results are not used by scan tests")
        }

        fn list_item_results(
            &self,
            _batch_id: &ExternalImportBatchId,
        ) -> Result<Vec<hmm_core::ExternalImportItemResult>> {
            bail!("item results are not used by scan tests")
        }
    }
}
