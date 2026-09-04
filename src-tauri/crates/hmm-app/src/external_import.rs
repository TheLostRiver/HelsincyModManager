use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

mod external_import_catalog;
mod external_import_history;
mod external_import_preview;
use external_import_catalog::{
    merge_external_metadata_hint, normalize_display_name, CatalogIndex, PendingCatalogImport,
};
pub use external_import_history::{
    ExternalImportHistoryEntry, ExternalImportHistoryPage, DEFAULT_EXTERNAL_IMPORT_HISTORY_LIMIT,
    MAX_EXTERNAL_IMPORT_HISTORY_LIMIT,
};
pub use external_import_preview::{ExternalImportPreviewCandidate, ExternalImportPreviewPage};

use hmm_core::{
    ExternalImportBatch, ExternalImportBatchId, ExternalImportBatchImportStatus,
    ExternalImportCandidate, ExternalImportCandidateStatus, ExternalImportConflictKind,
    ExternalImportConflictResolution, ExternalImportItemResult, ExternalImportItemStatus,
    ExternalImportReasonCode, ExternalImportResourceBudget, ExternalImportScanStatus,
    ExternalImportSelection, ExternalImportSelectionDecision, ExternalImportSelectionError,
    ExternalImportSelectionId, ExternalImportSelectionMutation,
    ExternalImportSelectionMutationResult, ExternalImportSelectionStatus, ExternalImportSource,
    ExternalImportSourceId, ModId,
};
use hmm_ports::{
    AppClock, CancellationToken, CategoryRepository, ExternalImportBatchRepository,
    ExternalImportItemResultRecord, ExternalImportMaterializationOutcome,
    ExternalImportMaterializeRequest, ExternalImportMaterializer, ExternalImportScanRequest,
    ExternalImportScanner, ExternalImportSealAndStartRequest, ExternalImportSealAndStartResult,
    ExternalImportSelectionCompareAndSwapRequest, ExternalImportSelectionCompareAndSwapResult,
    ExternalImportSourceRegistry, ModImportExternalCatalogAdmissionError,
    ModImportResultRepository, ModImportSandboxLocator,
};
use thiserror::Error;
use uuid::Uuid;

use crate::{
    ModImportPrepareService, ModStorageWriteGate, ModStorageWriteGateError, TaskKind, TaskManager,
    TaskManagerError, TaskProgressEvent, TaskStarted, TaskStatus,
};

pub const DEFAULT_EXTERNAL_IMPORT_PREVIEW_LIMIT: usize = 50;
pub const MAX_EXTERNAL_IMPORT_PREVIEW_LIMIT: usize = 100;

pub const EXTERNAL_IMPORT_SCAN_QUEUED_PHASE: &str = "external_import.scan.queued";
pub const EXTERNAL_IMPORT_SCAN_DISCOVERING_PHASE: &str = "external_import.scan.discovering";
pub const EXTERNAL_IMPORT_SCAN_FINGERPRINTING_PHASE: &str = "external_import.scan.fingerprinting";
pub const EXTERNAL_IMPORT_SCAN_COMPLETED_PHASE: &str = "external_import.scan.completed";
pub const EXTERNAL_IMPORT_SCAN_FAILED_PHASE: &str = "external_import.scan.failed";
pub const EXTERNAL_IMPORT_SCAN_CANCELLED_PHASE: &str = "external_import.scan.cancelled";
pub const EXTERNAL_IMPORT_BATCH_QUEUED_PHASE: &str = "external_import.import.queued";
pub const EXTERNAL_IMPORT_BATCH_MATERIALIZING_PHASE: &str = "external_import.import.materializing";
pub const EXTERNAL_IMPORT_BATCH_PREPARING_PHASE: &str = "external_import.import.preparing";
pub const EXTERNAL_IMPORT_BATCH_PERSISTING_PHASE: &str = "external_import.import.persisting";
pub const EXTERNAL_IMPORT_BATCH_COMPLETED_PHASE: &str = "external_import.import.completed";
pub const EXTERNAL_IMPORT_BATCH_FAILED_PHASE: &str = "external_import.import.failed";
pub const EXTERNAL_IMPORT_BATCH_CANCELLED_PHASE: &str = "external_import.import.cancelled";
pub const DEFAULT_EXTERNAL_IMPORT_SELECTION_TTL_MILLIS: u64 = 30 * 60 * 1000;
pub const DEFAULT_EXTERNAL_IMPORT_RESULT_LIMIT: usize = 50;
pub const MAX_EXTERNAL_IMPORT_RESULT_LIMIT: usize = 100;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalImportScanLaunch {
    pub task: TaskStarted,
    pub batch_id: ExternalImportBatchId,
    source: ExternalImportSource,
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
            source_id: Some(registration.source.source_id.clone()),
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalImportBatchLaunch {
    pub task: TaskStarted,
    pub batch_id: ExternalImportBatchId,
    selection_id: ExternalImportSelectionId,
    source_id: ExternalImportSourceId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalImportResultPage {
    pub batch: ExternalImportBatch,
    pub results: Vec<ExternalImportItemResultRecord>,
    pub total_count: usize,
    pub next_offset: Option<usize>,
}

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum ExternalImportBatchError {
    #[error("external import source is unavailable")]
    SourceUnavailable,
    #[error("external import task is unavailable")]
    TaskUnavailable,
    #[error("external import batch is unavailable")]
    BatchUnavailable,
    #[error("external import selection is unavailable")]
    SelectionUnavailable,
    #[error("external import selection is invalid")]
    Selection(ExternalImportSelectionError),
    #[error("external import batch is not startable")]
    BatchNotStartable,
    #[error("external import catalog is unavailable")]
    CatalogUnavailable,
    #[error("external import category is unavailable")]
    CategoryUnavailable,
    #[error("external import preview page is invalid")]
    PreviewPageInvalid,
    #[error("external import result page is invalid")]
    ResultPageInvalid,
    #[error("external import history page is invalid")]
    HistoryPageInvalid,
    #[error("external import clock is unavailable")]
    ClockUnavailable,
    /// #275: the storage root is migrating or already switched; materialised packages would be
    /// written to a sandbox root that is being copied away or is stale after restart.
    #[error("{0}")]
    StorageWriteFrozen(ModStorageWriteGateError),
}

impl ExternalImportBatchError {
    pub fn error_code(self) -> &'static str {
        match self {
            Self::StorageWriteFrozen(error) => error.code(),
            Self::SourceUnavailable => "external_import_source_unavailable",
            Self::TaskUnavailable => "external_import_task_unavailable",
            Self::BatchUnavailable => "external_import_batch_unavailable",
            Self::SelectionUnavailable => "external_import_selection_unavailable",
            Self::Selection(error) => error.reason_code().as_str(),
            Self::BatchNotStartable => "external_import_batch_not_startable",
            Self::CatalogUnavailable => "external_import_catalog_unavailable",
            Self::CategoryUnavailable => "external_import_category_unavailable",
            Self::PreviewPageInvalid => "external_import_preview_request_invalid",
            Self::ResultPageInvalid => "external_import_result_request_invalid",
            Self::HistoryPageInvalid => "external_import_history_request_invalid",
            Self::ClockUnavailable => "external_import_clock_unavailable",
        }
    }
}

pub struct ExternalImportBatchService {
    task_manager: Arc<TaskManager>,
    source_registry: Arc<dyn ExternalImportSourceRegistry>,
    materializer: Arc<dyn ExternalImportMaterializer>,
    batch_repository: Arc<dyn ExternalImportBatchRepository>,
    catalog: Arc<dyn ModImportResultRepository>,
    category_repository: Arc<dyn CategoryRepository>,
    sandbox_locator: Arc<dyn ModImportSandboxLocator>,
    prepare_service: Arc<ModImportPrepareService>,
    clock: Arc<dyn AppClock>,
    resource_budget: ExternalImportResourceBudget,
    write_gate: Arc<ModStorageWriteGate>,
}

impl ExternalImportBatchService {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        task_manager: Arc<TaskManager>,
        source_registry: Arc<dyn ExternalImportSourceRegistry>,
        materializer: Arc<dyn ExternalImportMaterializer>,
        batch_repository: Arc<dyn ExternalImportBatchRepository>,
        catalog: Arc<dyn ModImportResultRepository>,
        category_repository: Arc<dyn CategoryRepository>,
        sandbox_locator: Arc<dyn ModImportSandboxLocator>,
        prepare_service: Arc<ModImportPrepareService>,
        clock: Arc<dyn AppClock>,
    ) -> Self {
        Self {
            task_manager,
            source_registry,
            materializer,
            batch_repository,
            catalog,
            category_repository,
            sandbox_locator,
            prepare_service,
            clock,
            resource_budget: ExternalImportResourceBudget::default(),
            write_gate: Arc::new(ModStorageWriteGate::new()),
        }
    }

    pub fn with_resource_budget(mut self, resource_budget: ExternalImportResourceBudget) -> Self {
        self.resource_budget = resource_budget;
        self
    }

    /// Shares the storage write gate with the migration task and the other sandbox writers.
    pub fn with_write_gate(mut self, write_gate: Arc<ModStorageWriteGate>) -> Self {
        self.write_gate = write_gate;
        self
    }

    /// Startup-only recovery for batches owned by a previous process. It never resumes source
    /// access or creates a task; a user must explicitly reselect the source before retrying.
    pub fn recover_interrupted_batches(&self) -> Result<usize, ExternalImportBatchError> {
        self.batch_repository
            .recover_interrupted_batches()
            .map_err(|_| ExternalImportBatchError::BatchUnavailable)
    }

    pub fn create_selection(
        &self,
        batch_id: &ExternalImportBatchId,
    ) -> Result<ExternalImportSelection, ExternalImportBatchError> {
        let batch = self.get_batch(batch_id)?;
        if batch.scan_status != ExternalImportScanStatus::Completed
            || batch.import_status != ExternalImportBatchImportStatus::Pending
        {
            return Err(ExternalImportBatchError::BatchNotStartable);
        }
        self.refresh_preview_conflicts(&batch)?;
        let now = self.now_unix_millis()?;
        let expires_at_unix_millis = now
            .checked_add(DEFAULT_EXTERNAL_IMPORT_SELECTION_TTL_MILLIS)
            .ok_or(ExternalImportBatchError::ClockUnavailable)?;
        let selection = ExternalImportSelection::new(
            ExternalImportSelectionId::new(format!("external-import-selection-{}", Uuid::new_v4())),
            batch.batch_id,
            expires_at_unix_millis,
        );
        self.batch_repository
            .create_selection(&selection)
            .map_err(|_| ExternalImportBatchError::BatchUnavailable)?;
        Ok(selection)
    }

    pub fn update_selection(
        &self,
        selection_id: &ExternalImportSelectionId,
        expected_revision: u64,
        mutations: &[ExternalImportSelectionMutation],
    ) -> Result<ExternalImportSelectionMutationResult, ExternalImportBatchError> {
        self.validate_mutation_categories(mutations)?;
        let mut selection = self.get_selection(selection_id)?;
        let candidates = self
            .batch_repository
            .list_candidates(&selection.batch_id)
            .map_err(|_| ExternalImportBatchError::BatchUnavailable)?;
        let result = selection
            .apply_mutation(
                expected_revision,
                mutations,
                &candidates,
                &self.resource_budget,
                self.now_unix_millis()?,
            )
            .map_err(ExternalImportBatchError::Selection)?;
        self.persist_selection_cas(&selection, expected_revision)?;
        Ok(result)
    }

    pub fn select_all_ready(
        &self,
        selection_id: &ExternalImportSelectionId,
        expected_revision: u64,
    ) -> Result<ExternalImportSelectionMutationResult, ExternalImportBatchError> {
        let mut selection = self.get_selection(selection_id)?;
        self.validate_selection_categories(&selection)?;
        let candidates = self
            .batch_repository
            .list_candidates(&selection.batch_id)
            .map_err(|_| ExternalImportBatchError::BatchUnavailable)?;
        let result = selection
            .select_all_ready(
                expected_revision,
                &candidates,
                &self.resource_budget,
                self.now_unix_millis()?,
            )
            .map_err(ExternalImportBatchError::Selection)?;
        self.persist_selection_cas(&selection, expected_revision)?;
        Ok(result)
    }

    pub fn start_import(
        &self,
        batch_id: &ExternalImportBatchId,
        selection_id: &ExternalImportSelectionId,
        expected_revision: u64,
    ) -> Result<ExternalImportBatchLaunch, ExternalImportBatchError> {
        let mut batch = self.get_batch(batch_id)?;
        let selection = self.get_selection(selection_id)?;
        if selection.batch_id != batch.batch_id {
            return Err(ExternalImportBatchError::BatchNotStartable);
        }
        self.validate_selection_categories(&selection)?;
        let source_id = self.source_id_for_batch(&mut batch)?;
        let now_unix_millis = self.now_unix_millis()?;
        let task = self.register_import_task()?;
        let result = match self.batch_repository.seal_selection_and_start(
            ExternalImportSealAndStartRequest {
                selection_id,
                expected_revision,
                now_unix_millis,
                resource_budget: &self.resource_budget,
            },
        ) {
            Ok(result) => result,
            Err(_) => {
                self.fail_queued_task(&task.task_id)?;
                return Err(ExternalImportBatchError::BatchUnavailable);
            }
        };
        match result {
            ExternalImportSealAndStartResult::Started { .. } => Ok(ExternalImportBatchLaunch {
                task: TaskStarted {
                    task_id: task.task_id,
                    kind: task.kind,
                    status: task.status,
                },
                batch_id: batch.batch_id,
                selection_id: selection.selection_id,
                source_id,
            }),
            ExternalImportSealAndStartResult::RevisionConflict { .. } => {
                self.fail_queued_task(&task.task_id)?;
                Err(ExternalImportBatchError::Selection(
                    ExternalImportSelectionError::RevisionConflict,
                ))
            }
            ExternalImportSealAndStartResult::SelectionRejected { error } => {
                self.fail_queued_task(&task.task_id)?;
                Err(ExternalImportBatchError::Selection(error))
            }
            ExternalImportSealAndStartResult::BatchNotStartable => {
                self.fail_queued_task(&task.task_id)?;
                Err(ExternalImportBatchError::BatchNotStartable)
            }
        }
    }

    pub fn retry_import(
        &self,
        batch_id: &ExternalImportBatchId,
        selection_id: &ExternalImportSelectionId,
    ) -> Result<ExternalImportBatchLaunch, ExternalImportBatchError> {
        let mut batch = self.get_batch(batch_id)?;
        let selection = self.get_selection(selection_id)?;
        if selection.batch_id != batch.batch_id
            || selection.status != ExternalImportSelectionStatus::Sealed
        {
            return Err(ExternalImportBatchError::BatchNotStartable);
        }
        self.validate_selection_categories(&selection)?;
        let source_id = self.source_id_for_batch(&mut batch)?;
        let task = self.register_import_task()?;
        let restarted = match self.batch_repository.restart_batch(&batch.batch_id) {
            Ok(restarted) => restarted,
            Err(_) => {
                self.fail_queued_task(&task.task_id)?;
                return Err(ExternalImportBatchError::BatchUnavailable);
            }
        };
        if restarted.is_none() {
            self.fail_queued_task(&task.task_id)?;
            return Err(ExternalImportBatchError::BatchNotStartable);
        }
        Ok(ExternalImportBatchLaunch {
            task: TaskStarted {
                task_id: task.task_id,
                kind: task.kind,
                status: task.status,
            },
            batch_id: batch.batch_id,
            selection_id: selection.selection_id,
            source_id,
        })
    }

    /// Registers the batch task under the storage write gate, so a migration admitted right
    /// afterwards sees it and refuses to start instead of racing the materialisation.
    fn register_import_task(&self) -> Result<crate::TaskSnapshot, ExternalImportBatchError> {
        self.write_gate
            .admit(|| self.task_manager.create_task(TaskKind::ModImport))
            .map_err(ExternalImportBatchError::StorageWriteFrozen)?
            .map_err(|_| ExternalImportBatchError::TaskUnavailable)
    }

    pub fn abort_queued_import(
        &self,
        launch: &ExternalImportBatchLaunch,
    ) -> Result<(), ExternalImportBatchError> {
        let terminal_import_status = match self.task_manager.task_status(&launch.task.task_id) {
            Some(TaskStatus::Queued | TaskStatus::Running) => {
                self.fail_queued_task(&launch.task.task_id)?;
                ExternalImportBatchImportStatus::Failed
            }
            Some(TaskStatus::Failed) => ExternalImportBatchImportStatus::Failed,
            Some(TaskStatus::Cancelled) => ExternalImportBatchImportStatus::Cancelled,
            _ => return Err(ExternalImportBatchError::TaskUnavailable),
        };
        let mut batch = self.get_batch(&launch.batch_id)?;
        if batch.import_status == terminal_import_status {
            return Ok(());
        }
        if batch.import_status != ExternalImportBatchImportStatus::Running {
            return Err(ExternalImportBatchError::BatchUnavailable);
        }
        batch.import_status = terminal_import_status;
        self.batch_repository
            .update_batch(&batch)
            .map_err(|_| ExternalImportBatchError::BatchUnavailable)?;
        Ok(())
    }

    /// Best-effort durable recovery for an unexpected runner failure. The Tauri shell calls this
    /// instead of independently changing task state so the batch journal cannot remain running
    /// when its task has already reached a terminal state.
    pub fn recover_unhandled_import_failure(
        &self,
        launch: &ExternalImportBatchLaunch,
        error: ExternalImportBatchError,
    ) -> Result<TaskProgressEvent, ExternalImportBatchError> {
        let mut batch = self.get_batch(&launch.batch_id)?;
        let mut events = self.finish_import_failed(launch, &mut batch, error)?;
        events
            .pop()
            .ok_or(ExternalImportBatchError::TaskUnavailable)
    }

    /// Runs only read/prepare work and app-private catalog persistence. It deliberately does not
    /// acquire game or profile write locks.
    pub fn run_import(
        &self,
        launch: ExternalImportBatchLaunch,
    ) -> Result<Vec<TaskProgressEvent>, ExternalImportBatchError> {
        let mut batch = self.get_batch(&launch.batch_id)?;
        let selection = self.get_selection(&launch.selection_id)?;
        if batch.import_status != ExternalImportBatchImportStatus::Running
            || selection.status != ExternalImportSelectionStatus::Sealed
            || selection.batch_id != batch.batch_id
            || batch.source_id.as_ref() != Some(&launch.source_id)
        {
            return self.finish_import_failed(
                &launch,
                &mut batch,
                ExternalImportBatchError::BatchNotStartable,
            );
        }

        if self.is_cancelled(&launch.task.task_id) {
            return self.finish_import_cancelled(&launch, &mut batch, &selection);
        }
        match self.task_manager.start_task(&launch.task.task_id) {
            Ok(_) => {}
            Err(_) if self.is_cancelled(&launch.task.task_id) => {
                return self.finish_import_cancelled(&launch, &mut batch, &selection)
            }
            Err(_) => {
                return self.finish_import_failed(
                    &launch,
                    &mut batch,
                    ExternalImportBatchError::TaskUnavailable,
                )
            }
        }

        let run_result = self.run_import_started(&launch, &mut batch, &selection);
        match run_result {
            Ok(events) => Ok(events),
            Err(error) => self.finish_import_failed(&launch, &mut batch, error),
        }
    }

    fn run_import_started(
        &self,
        launch: &ExternalImportBatchLaunch,
        batch: &mut ExternalImportBatch,
        selection: &ExternalImportSelection,
    ) -> Result<Vec<TaskProgressEvent>, ExternalImportBatchError> {
        let candidates = self
            .batch_repository
            .list_candidates(&batch.batch_id)
            .map_err(|_| ExternalImportBatchError::BatchUnavailable)?;
        let selected_entries = selection
            .entries
            .iter()
            .map(|entry| (entry.candidate_id.clone(), entry.decision.clone()))
            .collect::<BTreeMap<_, _>>();
        let selected_candidates = candidates
            .iter()
            .filter(|candidate| selected_entries.contains_key(&candidate.candidate_id))
            .cloned()
            .collect::<Vec<_>>();
        if selected_candidates.len() != selection.entries.len() {
            return Err(ExternalImportBatchError::BatchUnavailable);
        }

        let mut events = vec![batch_event(
            launch,
            TaskStatus::Running,
            EXTERNAL_IMPORT_BATCH_MATERIALIZING_PHASE,
            0,
            selected_candidates.len(),
        )];
        events.push(batch_event(
            launch,
            TaskStatus::Running,
            EXTERNAL_IMPORT_BATCH_PREPARING_PHASE,
            0,
            selected_candidates.len(),
        ));
        let cancellation_token = TaskManagerCancellationToken {
            task_manager: Arc::clone(&self.task_manager),
            task_id: launch.task.task_id.clone(),
        };
        let mut catalog_index = CatalogIndex::load(self.catalog.as_ref())
            .map_err(|_| ExternalImportBatchError::CatalogUnavailable)?;
        let existing_results = self
            .batch_repository
            .list_item_results(&batch.batch_id)
            .map_err(|_| ExternalImportBatchError::BatchUnavailable)?
            .into_iter()
            .map(|result| (result.candidate_id.clone(), result))
            .collect::<BTreeMap<_, _>>();
        let imported_at_unix_millis = self.now_unix_millis()?;
        let mut pending_catalog: Vec<PendingCatalogImport> = Vec::new();
        let mut pending_results = Vec::new();

        for (index, candidate) in selected_candidates.iter().enumerate() {
            if cancellation_token.is_cancelled() {
                self.cleanup_unpersisted_pending_catalog(&pending_catalog);
                self.flush_item_results(&batch.batch_id, &mut pending_results)?;
                return self.finish_import_cancelled(launch, batch, selection);
            }
            if existing_results
                .get(&candidate.candidate_id)
                .is_some_and(|result| !result.retryable)
            {
                continue;
            }
            let decision = selected_entries
                .get(&candidate.candidate_id)
                .cloned()
                .flatten();

            if let Some(existing_import) = catalog_index
                .by_content_fingerprint
                .get(&candidate.content_fingerprint)
            {
                pending_results.push(self.catalog_result(
                    candidate,
                    &existing_import.mod_id,
                    decision.as_ref(),
                    ExternalImportItemStatus::AlreadyImported,
                    existing_import.matches_candidate(batch, candidate),
                ));
            } else {
                match self
                    .materializer
                    .materialize(ExternalImportMaterializeRequest {
                        source_id: &launch.source_id,
                        batch_id: &batch.batch_id,
                        candidate,
                        expected_content_fingerprint: &candidate.content_fingerprint,
                        task_id: &launch.task.task_id,
                        resource_budget: &self.resource_budget,
                        cancellation_token: &cancellation_token,
                    }) {
                    Ok(ExternalImportMaterializationOutcome::SourceChanged) => {
                        pending_results.push(item_result(
                            candidate,
                            ExternalImportItemStatus::Blocked,
                            Some(ExternalImportReasonCode::SourceChanged),
                            None,
                            false,
                        ));
                    }
                    Ok(ExternalImportMaterializationOutcome::Materialized(package)) => {
                        if package.candidate_id != candidate.candidate_id
                            || package.content_fingerprint != candidate.content_fingerprint
                            || package.resource_usage != candidate.resource_usage
                        {
                            self.cleanup_unpersisted_sandbox(&package.package_id);
                            pending_results.push(item_result(
                                candidate,
                                ExternalImportItemStatus::Blocked,
                                Some(ExternalImportReasonCode::SourceChanged),
                                None,
                                false,
                            ));
                        } else {
                            let package_id = package.package_id.clone();
                            let analysis = self
                                .prepare_service
                                .analyze_prepared_package_with_cancellation(
                                    launch.task.task_id.clone(),
                                    package.package_id,
                                    self.sandbox_locator.as_ref(),
                                    &cancellation_token,
                                );
                            match analysis {
                                Ok(mut analysis) => {
                                    merge_external_metadata_hint(&mut analysis, candidate);
                                    let normalized_name =
                                        normalize_display_name(&analysis.display_name);
                                    let permits_keep_both = decision
                                        .as_ref()
                                        .and_then(|decision| decision.conflict_resolution)
                                        == Some(ExternalImportConflictResolution::KeepBoth);
                                    let pending_name_collision =
                                        pending_catalog.iter().any(|entry| {
                                            normalize_display_name(&entry.analysis.display_name)
                                                == normalized_name
                                        });
                                    if (catalog_index.display_names.contains(&normalized_name)
                                        || pending_name_collision)
                                        && !permits_keep_both
                                    {
                                        self.cleanup_unpersisted_sandbox(&package_id);
                                        pending_results.push(item_result(
                                            candidate,
                                            ExternalImportItemStatus::Blocked,
                                            Some(ExternalImportReasonCode::NameCollision),
                                            None,
                                            false,
                                        ));
                                    } else {
                                        let pending = PendingCatalogImport::new(
                                            batch,
                                            candidate,
                                            decision,
                                            analysis,
                                            imported_at_unix_millis,
                                        );
                                        match pending {
                                            Ok(pending) => pending_catalog.push(pending),
                                            Err(error) => {
                                                self.cleanup_unpersisted_sandbox(&package_id);
                                                self.cleanup_unpersisted_pending_catalog(
                                                    &pending_catalog,
                                                );
                                                return Err(error);
                                            }
                                        }
                                    }
                                }
                                Err(_) if cancellation_token.is_cancelled() => {
                                    self.cleanup_unpersisted_sandbox(&package_id);
                                    self.cleanup_unpersisted_pending_catalog(&pending_catalog);
                                    self.flush_item_results(&batch.batch_id, &mut pending_results)?;
                                    return self.finish_import_cancelled(launch, batch, selection);
                                }
                                Err(_) => {
                                    self.cleanup_unpersisted_sandbox(&package_id);
                                    pending_results.push(item_result(
                                        candidate,
                                        ExternalImportItemStatus::Failed,
                                        None,
                                        None,
                                        true,
                                    ));
                                }
                            }
                        }
                    }
                    Err(_) if cancellation_token.is_cancelled() => {
                        self.cleanup_unpersisted_pending_catalog(&pending_catalog);
                        self.flush_item_results(&batch.batch_id, &mut pending_results)?;
                        return self.finish_import_cancelled(launch, batch, selection);
                    }
                    Err(_) => pending_results.push(item_result(
                        candidate,
                        ExternalImportItemStatus::Failed,
                        None,
                        None,
                        true,
                    )),
                }
            }

            if pending_catalog.len() == hmm_ports::MOD_IMPORT_UPSERT_CHUNK_SIZE {
                events.push(batch_event(
                    launch,
                    TaskStatus::Running,
                    EXTERNAL_IMPORT_BATCH_PERSISTING_PHASE,
                    index + 1,
                    selected_candidates.len(),
                ));
                self.flush_catalog_upserts(
                    &batch.batch_id,
                    &mut pending_catalog,
                    &mut pending_results,
                    &mut catalog_index,
                )?;
            }
            if pending_results.len() >= hmm_ports::MOD_IMPORT_UPSERT_CHUNK_SIZE {
                if let Err(error) = self.flush_item_results(&batch.batch_id, &mut pending_results) {
                    self.cleanup_unpersisted_pending_catalog(&pending_catalog);
                    return Err(error);
                }
            }
        }

        if cancellation_token.is_cancelled() {
            self.cleanup_unpersisted_pending_catalog(&pending_catalog);
            self.flush_item_results(&batch.batch_id, &mut pending_results)?;
            return self.finish_import_cancelled(launch, batch, selection);
        }
        if !pending_catalog.is_empty() {
            events.push(batch_event(
                launch,
                TaskStatus::Running,
                EXTERNAL_IMPORT_BATCH_PERSISTING_PHASE,
                selected_candidates.len(),
                selected_candidates.len(),
            ));
            self.flush_catalog_upserts(
                &batch.batch_id,
                &mut pending_catalog,
                &mut pending_results,
                &mut catalog_index,
            )?;
        }
        self.flush_item_results(&batch.batch_id, &mut pending_results)?;

        // The durable terminal transition is deliberately short. Cancellation before this barrier
        // still leaves a coherent cancelled batch; after it, completed-with-errors is authoritative.
        if self
            .task_manager
            .block_task_cancellation(&launch.task.task_id)
            .is_err()
        {
            if self.is_cancelled(&launch.task.task_id) {
                return self.finish_import_cancelled(launch, batch, selection);
            }
            return Err(ExternalImportBatchError::TaskUnavailable);
        }
        let results = self
            .batch_repository
            .list_item_results(&batch.batch_id)
            .map_err(|_| ExternalImportBatchError::BatchUnavailable)?;
        batch.import_status = if results.iter().any(|result| {
            matches!(
                result.status,
                ExternalImportItemStatus::Blocked
                    | ExternalImportItemStatus::Failed
                    | ExternalImportItemStatus::Cancelled
            )
        }) {
            ExternalImportBatchImportStatus::CompletedWithErrors
        } else {
            ExternalImportBatchImportStatus::Completed
        };
        self.batch_repository
            .update_batch(batch)
            .map_err(|_| ExternalImportBatchError::BatchUnavailable)?;
        self.task_manager
            .complete_task(&launch.task.task_id)
            .map_err(|_| ExternalImportBatchError::TaskUnavailable)?;
        events.push(batch_event(
            launch,
            TaskStatus::Completed,
            EXTERNAL_IMPORT_BATCH_COMPLETED_PHASE,
            selected_candidates.len(),
            selected_candidates.len(),
        ));
        Ok(events)
    }

    fn flush_catalog_upserts(
        &self,
        batch_id: &ExternalImportBatchId,
        pending_catalog: &mut Vec<PendingCatalogImport>,
        pending_results: &mut Vec<ExternalImportItemResult>,
        catalog_index: &mut CatalogIndex,
    ) -> Result<(), ExternalImportBatchError> {
        let entries = std::mem::take(pending_catalog);
        let upserts = entries
            .iter()
            .map(PendingCatalogImport::external_catalog_upsert)
            .collect::<Vec<_>>();
        if let Err(error) = self.catalog.upsert_external_import_many(&upserts) {
            let reconciled = CatalogIndex::load(self.catalog.as_ref()).ok();
            let admission = error
                .downcast_ref::<ModImportExternalCatalogAdmissionError>()
                .cloned();
            if let (Some(reconciled), Some(admission)) = (reconciled.as_ref(), admission) {
                *catalog_index = reconciled.clone();
                let mut retry = Vec::new();
                for entry in entries {
                    if let Some(existing_import) = reconciled
                        .by_content_fingerprint
                        .get(&entry.content_fingerprint)
                    {
                        let own_logical_mod =
                            existing_import.mod_id == entry.upsert.logical_mod.mod_id;
                        let status = if own_logical_mod {
                            ExternalImportItemStatus::Imported
                        } else {
                            ExternalImportItemStatus::AlreadyImported
                        };
                        if !own_logical_mod {
                            self.cleanup_unpersisted_sandbox(&entry.analysis.package_id);
                        }
                        pending_results.push(self.catalog_result(
                            &entry.candidate,
                            &existing_import.mod_id,
                            entry.decision.as_ref(),
                            status,
                            own_logical_mod,
                        ));
                        continue;
                    }

                    let permits_keep_both = entry
                        .decision
                        .as_ref()
                        .and_then(|decision| decision.conflict_resolution)
                        == Some(ExternalImportConflictResolution::KeepBoth);
                    let rejected_for_name = matches!(
                        &admission,
                        ModImportExternalCatalogAdmissionError::DisplayNameCollision { display_name }
                            if !permits_keep_both
                                && normalize_display_name(&entry.analysis.display_name)
                                    == normalize_display_name(display_name)
                    );
                    let rejected_for_content = matches!(
                        &admission,
                        ModImportExternalCatalogAdmissionError::ContentAlreadyImported {
                            content_fingerprint,
                            ..
                        } if entry.content_fingerprint == *content_fingerprint
                    );
                    if rejected_for_name {
                        self.cleanup_unpersisted_sandbox(&entry.analysis.package_id);
                        pending_results.push(item_result(
                            &entry.candidate,
                            ExternalImportItemStatus::Blocked,
                            Some(ExternalImportReasonCode::NameCollision),
                            None,
                            false,
                        ));
                    } else if rejected_for_content {
                        let ModImportExternalCatalogAdmissionError::ContentAlreadyImported {
                            existing_mod_id,
                            ..
                        } = &admission
                        else {
                            unreachable!("content admission branch requires a content rejection");
                        };
                        self.cleanup_unpersisted_sandbox(&entry.analysis.package_id);
                        pending_results.push(self.catalog_result(
                            &entry.candidate,
                            existing_mod_id,
                            entry.decision.as_ref(),
                            ExternalImportItemStatus::AlreadyImported,
                            false,
                        ));
                    } else {
                        retry.push(entry);
                    }
                }
                if !retry.is_empty() {
                    *pending_catalog = retry;
                    self.flush_catalog_upserts(
                        batch_id,
                        pending_catalog,
                        pending_results,
                        catalog_index,
                    )?;
                }
                return Ok(());
            }
            for entry in entries {
                if let Some(existing_import) = reconciled
                    .as_ref()
                    .and_then(|index| index.by_content_fingerprint.get(&entry.content_fingerprint))
                {
                    let own_logical_mod = existing_import.mod_id == entry.upsert.logical_mod.mod_id;
                    let status = if own_logical_mod {
                        ExternalImportItemStatus::Imported
                    } else {
                        ExternalImportItemStatus::AlreadyImported
                    };
                    if !own_logical_mod {
                        self.cleanup_unpersisted_sandbox(&entry.analysis.package_id);
                    }
                    pending_results.push(self.catalog_result(
                        &entry.candidate,
                        &existing_import.mod_id,
                        entry.decision.as_ref(),
                        status,
                        own_logical_mod,
                    ));
                } else {
                    self.cleanup_unpersisted_sandbox(&entry.analysis.package_id);
                    pending_results.push(item_result(
                        &entry.candidate,
                        ExternalImportItemStatus::Failed,
                        None,
                        None,
                        true,
                    ));
                }
            }
            self.flush_item_results(batch_id, pending_results)?;
            return Err(ExternalImportBatchError::CatalogUnavailable);
        }

        for entry in entries {
            catalog_index.record(&entry.upsert.logical_mod, &entry.analysis.display_name);
            let mod_id = entry.upsert.logical_mod.mod_id.clone();
            pending_results.push(self.catalog_result(
                &entry.candidate,
                &mod_id,
                entry.decision.as_ref(),
                ExternalImportItemStatus::Imported,
                true,
            ));
        }
        Ok(())
    }

    fn cleanup_unpersisted_sandbox(&self, package_id: &str) {
        let _ = self.sandbox_locator.cleanup_sandbox_for_package(package_id);
    }

    fn cleanup_unpersisted_pending_catalog(&self, pending_catalog: &[PendingCatalogImport]) {
        for entry in pending_catalog {
            self.cleanup_unpersisted_sandbox(&entry.analysis.package_id);
        }
    }

    fn catalog_result(
        &self,
        candidate: &ExternalImportCandidate,
        mod_id: &ModId,
        decision: Option<&ExternalImportSelectionDecision>,
        success_status: ExternalImportItemStatus,
        apply_selection_category: bool,
    ) -> ExternalImportItemResult {
        // A cross-batch content duplicate references an existing user-owned Mod. Only a retry
        // proven to belong to the same batch candidate may complete its pending category work.
        if !apply_selection_category {
            return item_result(candidate, success_status, None, Some(mod_id.clone()), false);
        }
        match self.apply_category(mod_id, decision) {
            Ok(()) => item_result(candidate, success_status, None, Some(mod_id.clone()), false),
            Err(_) => item_result(
                candidate,
                ExternalImportItemStatus::Failed,
                None,
                None,
                true,
            ),
        }
    }

    fn apply_category(
        &self,
        mod_id: &ModId,
        decision: Option<&ExternalImportSelectionDecision>,
    ) -> Result<(), ExternalImportBatchError> {
        let Some(category_id) = decision.and_then(|decision| decision.category_id.as_deref())
        else {
            return Ok(());
        };
        if self
            .category_repository
            .get(category_id)
            .map_err(|_| ExternalImportBatchError::CategoryUnavailable)?
            .is_none()
        {
            return Err(ExternalImportBatchError::CategoryUnavailable);
        }
        let mut category_ids = self
            .category_repository
            .get_mod_categories(mod_id.as_str())
            .map_err(|_| ExternalImportBatchError::CategoryUnavailable)?
            .into_iter()
            .map(|category| category.id)
            .collect::<Vec<_>>();
        if !category_ids.iter().any(|existing| existing == category_id) {
            category_ids.push(category_id.to_owned());
            self.category_repository
                .set_mod_categories(mod_id.as_str(), &category_ids)
                .map_err(|_| ExternalImportBatchError::CategoryUnavailable)?;
        }
        Ok(())
    }

    fn flush_item_results(
        &self,
        batch_id: &ExternalImportBatchId,
        pending_results: &mut Vec<ExternalImportItemResult>,
    ) -> Result<(), ExternalImportBatchError> {
        if pending_results.is_empty() {
            return Ok(());
        }
        let results = std::mem::take(pending_results);
        self.batch_repository
            .append_item_results(batch_id, &results)
            .map_err(|_| ExternalImportBatchError::BatchUnavailable)
    }

    fn finish_import_cancelled(
        &self,
        launch: &ExternalImportBatchLaunch,
        batch: &mut ExternalImportBatch,
        selection: &ExternalImportSelection,
    ) -> Result<Vec<TaskProgressEvent>, ExternalImportBatchError> {
        let existing_results = self
            .batch_repository
            .list_item_results(&batch.batch_id)
            .map_err(|_| ExternalImportBatchError::BatchUnavailable)?
            .into_iter()
            .map(|result| (result.candidate_id.clone(), result))
            .collect::<BTreeMap<_, _>>();
        let cancellation_results = selection
            .entries
            .iter()
            .filter(|entry| !existing_results.contains_key(&entry.candidate_id))
            .map(|entry| ExternalImportItemResult {
                candidate_id: entry.candidate_id.clone(),
                status: ExternalImportItemStatus::Cancelled,
                reason_code: None,
                imported_mod_id: None,
                retryable: true,
            })
            .collect::<Vec<_>>();
        self.batch_repository
            .append_item_results(&batch.batch_id, &cancellation_results)
            .map_err(|_| ExternalImportBatchError::BatchUnavailable)?;
        batch.import_status = ExternalImportBatchImportStatus::Cancelled;
        self.batch_repository
            .update_batch(batch)
            .map_err(|_| ExternalImportBatchError::BatchUnavailable)?;
        if matches!(
            self.task_manager.task_status(&launch.task.task_id),
            Some(TaskStatus::Queued | TaskStatus::Running)
        ) {
            self.task_manager
                .cancel_task(&launch.task.task_id)
                .map_err(|_| ExternalImportBatchError::TaskUnavailable)?;
        }
        Ok(vec![batch_event(
            launch,
            TaskStatus::Cancelled,
            EXTERNAL_IMPORT_BATCH_CANCELLED_PHASE,
            0,
            selection.entries.len(),
        )])
    }

    fn finish_import_failed(
        &self,
        launch: &ExternalImportBatchLaunch,
        batch: &mut ExternalImportBatch,
        error: ExternalImportBatchError,
    ) -> Result<Vec<TaskProgressEvent>, ExternalImportBatchError> {
        if self.is_cancelled(&launch.task.task_id) {
            let selection = self.get_selection(&launch.selection_id)?;
            return self.finish_import_cancelled(launch, batch, &selection);
        }
        batch.import_status = ExternalImportBatchImportStatus::Failed;
        self.batch_repository
            .update_batch(batch)
            .map_err(|_| ExternalImportBatchError::BatchUnavailable)?;
        if matches!(
            self.task_manager.task_status(&launch.task.task_id),
            Some(TaskStatus::Queued | TaskStatus::Running)
        ) {
            self.task_manager
                .fail_task(&launch.task.task_id)
                .map_err(|_| ExternalImportBatchError::TaskUnavailable)?;
        }
        let mut event = batch_event(
            launch,
            TaskStatus::Failed,
            EXTERNAL_IMPORT_BATCH_FAILED_PHASE,
            0,
            0,
        );
        event.error = Some(error.error_code().to_owned());
        Ok(vec![event])
    }

    fn is_cancelled(&self, task_id: &str) -> bool {
        self.task_manager.task_status(task_id) == Some(TaskStatus::Cancelled)
    }

    fn fail_queued_task(&self, task_id: &str) -> Result<(), ExternalImportBatchError> {
        self.task_manager
            .fail_task(task_id)
            .map(|_| ())
            .map_err(|_| ExternalImportBatchError::TaskUnavailable)
    }

    pub fn get_results(
        &self,
        batch_id: &ExternalImportBatchId,
        offset: usize,
        limit: usize,
    ) -> Result<ExternalImportResultPage, ExternalImportBatchError> {
        if !(1..=MAX_EXTERNAL_IMPORT_RESULT_LIMIT).contains(&limit) {
            return Err(ExternalImportBatchError::ResultPageInvalid);
        }
        let batch = self.get_batch(batch_id)?;
        let page = self
            .batch_repository
            .list_item_result_details_page(batch_id, offset, limit)
            .map_err(|_| ExternalImportBatchError::BatchUnavailable)?;
        Ok(ExternalImportResultPage {
            batch,
            results: page.records,
            total_count: page.total_count,
            next_offset: page.next_offset,
        })
    }

    fn get_batch(
        &self,
        batch_id: &ExternalImportBatchId,
    ) -> Result<ExternalImportBatch, ExternalImportBatchError> {
        self.batch_repository
            .get_batch(batch_id)
            .map_err(|_| ExternalImportBatchError::BatchUnavailable)?
            .ok_or(ExternalImportBatchError::BatchUnavailable)
    }

    fn get_selection(
        &self,
        selection_id: &ExternalImportSelectionId,
    ) -> Result<ExternalImportSelection, ExternalImportBatchError> {
        self.batch_repository
            .get_selection(selection_id)
            .map_err(|_| ExternalImportBatchError::BatchUnavailable)?
            .ok_or(ExternalImportBatchError::SelectionUnavailable)
    }

    fn persist_selection_cas(
        &self,
        selection: &ExternalImportSelection,
        expected_revision: u64,
    ) -> Result<(), ExternalImportBatchError> {
        match self
            .batch_repository
            .compare_and_swap_selection(ExternalImportSelectionCompareAndSwapRequest {
                selection,
                expected_revision,
            })
            .map_err(|_| ExternalImportBatchError::BatchUnavailable)?
        {
            ExternalImportSelectionCompareAndSwapResult::Applied(_) => Ok(()),
            ExternalImportSelectionCompareAndSwapResult::RevisionConflict { .. } => Err(
                ExternalImportBatchError::Selection(ExternalImportSelectionError::RevisionConflict),
            ),
        }
    }

    fn source_id_for_batch(
        &self,
        batch: &mut ExternalImportBatch,
    ) -> Result<ExternalImportSourceId, ExternalImportBatchError> {
        if let Some(source_id) = batch.source_id.as_ref() {
            if let Some(registration) = self
                .source_registry
                .resolve_source(source_id)
                .map_err(|_| ExternalImportBatchError::SourceUnavailable)?
            {
                if registration.source.adapter_id == batch.adapter_id
                    && registration.source_fingerprint == batch.source_fingerprint
                {
                    return Ok(source_id.clone());
                }
            }
        }

        let registration = self
            .source_registry
            .resolve_matching_source(&batch.source_fingerprint)
            .map_err(|_| ExternalImportBatchError::SourceUnavailable)?
            .filter(|registration| registration.source.adapter_id == batch.adapter_id)
            .ok_or(ExternalImportBatchError::SourceUnavailable)?;
        batch.source_id = Some(registration.source.source_id.clone());
        self.batch_repository
            .update_batch(batch)
            .map_err(|_| ExternalImportBatchError::BatchUnavailable)?;
        Ok(registration.source.source_id)
    }

    fn validate_mutation_categories(
        &self,
        mutations: &[ExternalImportSelectionMutation],
    ) -> Result<(), ExternalImportBatchError> {
        let category_ids = mutations
            .iter()
            .filter_map(|mutation| {
                mutation
                    .decision
                    .as_ref()
                    .and_then(|decision| decision.category_id.as_deref())
            })
            .collect::<BTreeSet<_>>();
        for category_id in category_ids {
            if self
                .category_repository
                .get(category_id)
                .map_err(|_| ExternalImportBatchError::CategoryUnavailable)?
                .is_none()
            {
                return Err(ExternalImportBatchError::CategoryUnavailable);
            }
        }
        Ok(())
    }

    fn validate_selection_categories(
        &self,
        selection: &ExternalImportSelection,
    ) -> Result<(), ExternalImportBatchError> {
        let mutations = selection
            .entries
            .iter()
            .map(|entry| ExternalImportSelectionMutation {
                candidate_id: entry.candidate_id.clone(),
                selected: true,
                decision: entry.decision.clone(),
            })
            .collect::<Vec<_>>();
        self.validate_mutation_categories(&mutations)
    }

    fn refresh_preview_conflicts(
        &self,
        batch: &ExternalImportBatch,
    ) -> Result<(), ExternalImportBatchError> {
        let catalog = CatalogIndex::load(self.catalog.as_ref())
            .map_err(|_| ExternalImportBatchError::CatalogUnavailable)?;
        let mut candidates = self
            .batch_repository
            .list_candidates(&batch.batch_id)
            .map_err(|_| ExternalImportBatchError::BatchUnavailable)?;
        let mut changed = false;
        for candidate in &mut candidates {
            if candidate.preview_status != ExternalImportCandidateStatus::Ready {
                continue;
            }
            if catalog
                .by_content_fingerprint
                .contains_key(&candidate.content_fingerprint)
            {
                candidate.preview_status = ExternalImportCandidateStatus::AlreadyImported;
                candidate.conflict_kind = ExternalImportConflictKind::ContentDuplicate;
                changed = true;
            } else if candidate
                .metadata_hint
                .display_name
                .as_deref()
                .map(normalize_display_name)
                .is_some_and(|name| catalog.display_names.contains(&name))
            {
                candidate.preview_status = ExternalImportCandidateStatus::NameCollision;
                candidate.conflict_kind = ExternalImportConflictKind::NameCollision;
                changed = true;
            }
        }
        if changed {
            self.batch_repository
                .replace_candidates(&batch.batch_id, &candidates)
                .map_err(|_| ExternalImportBatchError::BatchUnavailable)?;
        }
        Ok(())
    }

    fn now_unix_millis(&self) -> Result<u64, ExternalImportBatchError> {
        self.clock
            .now_unix_millis()
            .ok()
            .and_then(|value| u64::try_from(value).ok())
            .ok_or(ExternalImportBatchError::ClockUnavailable)
    }
}

fn item_result(
    candidate: &ExternalImportCandidate,
    status: ExternalImportItemStatus,
    reason_code: Option<ExternalImportReasonCode>,
    imported_mod_id: Option<ModId>,
    retryable: bool,
) -> ExternalImportItemResult {
    ExternalImportItemResult {
        candidate_id: candidate.candidate_id.clone(),
        status,
        reason_code,
        imported_mod_id,
        retryable,
    }
}

fn batch_event(
    launch: &ExternalImportBatchLaunch,
    status: TaskStatus,
    phase: &'static str,
    current: usize,
    total: usize,
) -> TaskProgressEvent {
    let mut event =
        TaskProgressEvent::new(launch.task.task_id.clone(), launch.task.kind, status, phase);
    event.current = u64::try_from(current).ok();
    event.total = u64::try_from(total).ok();
    event.result_ref = Some(launch.batch_id.as_str().to_owned());
    event
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
        let persisted_batch = repository
            .get_batch(&batch_id)
            .expect("read batch")
            .expect("batch exists");
        let persisted_candidates = repository
            .list_candidates_page(&batch_id, 0, 50)
            .expect("read candidates");

        assert_eq!(scanner.calls.load(Ordering::SeqCst), 1);
        assert_eq!(
            task_manager.task_status(&task_id),
            Some(TaskStatus::Completed)
        );
        assert_eq!(
            persisted_batch.scan_status,
            ExternalImportScanStatus::Completed
        );
        assert_eq!(persisted_candidates.total_count, 1);
        assert_eq!(persisted_candidates.candidates.len(), 1);
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
