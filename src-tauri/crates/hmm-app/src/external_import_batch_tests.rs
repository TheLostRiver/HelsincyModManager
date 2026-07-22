use crate::{
    ExternalImportBatchError, ExternalImportBatchService, ImportPreviewImageProcessor,
    ModImportAnalysisService, ModImportPrepareService, TaskManager, TaskStatus,
};
use anyhow::{bail, Result};
use hmm_core::{
    Category, ExternalImportAdapterId, ExternalImportBatch, ExternalImportBatchId,
    ExternalImportBatchImportStatus, ExternalImportCandidate, ExternalImportCandidateId,
    ExternalImportCandidateStatus, ExternalImportConflictKind, ExternalImportItemResult,
    ExternalImportItemStatus, ExternalImportMetadataHint, ExternalImportReasonCode,
    ExternalImportResourceUsage, ExternalImportScanStatus, ExternalImportSelection,
    ExternalImportSelectionDecision, ExternalImportSelectionEntry, ExternalImportSelectionId,
    ExternalImportSelectionStatus, ExternalImportSource, ExternalImportSourceId, ModId,
    PreviewImageRejectionReason,
};
use hmm_ports::{
    AppClock, CategoryRepository, ExternalImportBatchRepository, ExternalImportCandidatePage,
    ExternalImportItemResultPage, ExternalImportMaterializationOutcome,
    ExternalImportMaterializeRequest, ExternalImportMaterializedPackage,
    ExternalImportMaterializer, ExternalImportSealAndStartRequest,
    ExternalImportSealAndStartResult, ExternalImportSelectionCompareAndSwapRequest,
    ExternalImportSelectionCompareAndSwapResult, ExternalImportSourceRegistration,
    ExternalImportSourceRegistry, ModImportCatalogUpsert, ModImportPackagePrepareRequest,
    ModImportPackagePreparer, ModImportResultRepository, ModImportSandboxLocator,
    ModPackageMetadata, ModPackageMetadataAnalyzer, PreparedModPackage,
    PreviewImageProcessingResult, StoredLogicalMod, StoredModImportAnalysis, StoredModRevision,
    ThumbnailRef, ThumbnailStore,
};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn start_and_retry_repository_failures_close_the_created_task() {
    let repository = Arc::new(FixtureBatchRepository::default());
    repository.set_fail_seal(true);
    let batch = fixture_batch(
        "batch-seal-failure",
        "source-current",
        ExternalImportBatchImportStatus::Pending,
    );
    let selection = fixture_selection(
        &batch,
        ExternalImportSelectionStatus::Editing,
        &["candidate-a"],
        None,
    );
    repository.seed(
        &batch,
        &selection,
        &[fixture_candidate(&batch, "candidate-a", 1)],
    );
    let (service, task_manager) = fixture_service(
        Arc::clone(&repository),
        fixture_registry(Some(fixture_registration("source-current")), None),
        Arc::new(FixtureMaterializer::default()),
        Arc::new(FixtureCatalog::succeeds()),
        Arc::new(FixtureCategoryRepository::new(0)),
        Arc::new(FixtureClock::available()),
    );

    let started_at = unix_millis();
    assert_eq!(
        service.start_import(&batch.batch_id, &selection.selection_id, selection.revision),
        Err(ExternalImportBatchError::BatchUnavailable)
    );
    let finished_at = unix_millis();
    assert_created_task_terminal(
        &task_manager,
        0,
        started_at,
        finished_at,
        TaskStatus::Failed,
    );

    let retry_repository = Arc::new(FixtureBatchRepository::default());
    retry_repository.set_fail_restart(true);
    let retry_batch = fixture_batch(
        "batch-retry-failure",
        "source-current",
        ExternalImportBatchImportStatus::Failed,
    );
    let retry_selection = fixture_selection(
        &retry_batch,
        ExternalImportSelectionStatus::Sealed,
        &["candidate-a"],
        None,
    );
    retry_repository.seed(
        &retry_batch,
        &retry_selection,
        &[fixture_candidate(&retry_batch, "candidate-a", 1)],
    );
    let (retry_service, retry_task_manager) = fixture_service(
        Arc::clone(&retry_repository),
        fixture_registry(Some(fixture_registration("source-current")), None),
        Arc::new(FixtureMaterializer::default()),
        Arc::new(FixtureCatalog::succeeds()),
        Arc::new(FixtureCategoryRepository::new(0)),
        Arc::new(FixtureClock::available()),
    );

    let started_at = unix_millis();
    assert_eq!(
        retry_service.retry_import(&retry_batch.batch_id, &retry_selection.selection_id),
        Err(ExternalImportBatchError::BatchUnavailable)
    );
    let finished_at = unix_millis();
    assert_created_task_terminal(
        &retry_task_manager,
        0,
        started_at,
        finished_at,
        TaskStatus::Failed,
    );
}

#[test]
fn clock_failure_happens_before_a_task_is_created() {
    let repository = Arc::new(FixtureBatchRepository::default());
    let batch = fixture_batch(
        "batch-clock-failure",
        "source-current",
        ExternalImportBatchImportStatus::Pending,
    );
    let selection = fixture_selection(
        &batch,
        ExternalImportSelectionStatus::Editing,
        &["candidate-a"],
        None,
    );
    repository.seed(
        &batch,
        &selection,
        &[fixture_candidate(&batch, "candidate-a", 1)],
    );
    let (service, task_manager) = fixture_service(
        repository,
        fixture_registry(Some(fixture_registration("source-current")), None),
        Arc::new(FixtureMaterializer::default()),
        Arc::new(FixtureCatalog::succeeds()),
        Arc::new(FixtureCategoryRepository::new(0)),
        Arc::new(FixtureClock::unavailable()),
    );

    assert_eq!(
        service.start_import(&batch.batch_id, &selection.selection_id, selection.revision),
        Err(ExternalImportBatchError::ClockUnavailable)
    );
    let first_task = task_manager
        .create_task(crate::TaskKind::ModImport)
        .expect("clock failure did not allocate a task");
    assert!(first_task.task_id.ends_with("-0"));
}

#[test]
fn aborting_a_cancelled_queued_import_keeps_the_batch_cancelled() {
    let repository = Arc::new(FixtureBatchRepository::default());
    let batch = fixture_batch(
        "batch-abort-cancelled",
        "source-current",
        ExternalImportBatchImportStatus::Pending,
    );
    let selection = fixture_selection(
        &batch,
        ExternalImportSelectionStatus::Editing,
        &["candidate-a"],
        None,
    );
    repository.seed(
        &batch,
        &selection,
        &[fixture_candidate(&batch, "candidate-a", 1)],
    );
    let (service, task_manager) = fixture_service(
        Arc::clone(&repository),
        fixture_registry(Some(fixture_registration("source-current")), None),
        Arc::new(FixtureMaterializer::default()),
        Arc::new(FixtureCatalog::succeeds()),
        Arc::new(FixtureCategoryRepository::new(0)),
        Arc::new(FixtureClock::available()),
    );

    let launch = service
        .start_import(&batch.batch_id, &selection.selection_id, selection.revision)
        .expect("start import");
    task_manager
        .cancel_task(&launch.task.task_id)
        .expect("cancel queued import");

    service
        .abort_queued_import(&launch)
        .expect("close undispatched import");

    assert_eq!(
        task_manager.task_status(&launch.task.task_id),
        Some(TaskStatus::Cancelled)
    );
    assert_eq!(
        repository.batch(&batch.batch_id).import_status,
        ExternalImportBatchImportStatus::Cancelled
    );
}

#[test]
fn unexpected_runner_failure_terminalizes_the_durable_running_batch() {
    let repository = Arc::new(FixtureBatchRepository::default());
    let batch = fixture_batch(
        "batch-runner-recovery",
        "source-current",
        ExternalImportBatchImportStatus::Pending,
    );
    let selection = fixture_selection(
        &batch,
        ExternalImportSelectionStatus::Editing,
        &["candidate-a"],
        None,
    );
    repository.seed(
        &batch,
        &selection,
        &[fixture_candidate(&batch, "candidate-a", 1)],
    );
    let (service, task_manager) = fixture_service(
        Arc::clone(&repository),
        fixture_registry(Some(fixture_registration("source-current")), None),
        Arc::new(FixtureMaterializer::default()),
        Arc::new(FixtureCatalog::succeeds()),
        Arc::new(FixtureCategoryRepository::new(0)),
        Arc::new(FixtureClock::available()),
    );

    let launch = service
        .start_import(&batch.batch_id, &selection.selection_id, selection.revision)
        .expect("start import");
    let event = service
        .recover_unhandled_import_failure(&launch, ExternalImportBatchError::CatalogUnavailable)
        .expect("recover runner failure");

    assert_eq!(event.status, TaskStatus::Failed);
    assert_eq!(
        event.error.as_deref(),
        Some("external_import_catalog_unavailable")
    );
    assert_eq!(
        task_manager.task_status(&launch.task.task_id),
        Some(TaskStatus::Failed)
    );
    assert_eq!(
        repository.batch(&batch.batch_id).import_status,
        ExternalImportBatchImportStatus::Failed
    );
}

#[test]
fn catalog_failure_reconciles_the_durable_partial_success() {
    let repository = Arc::new(FixtureBatchRepository::default());
    let batch = fixture_batch(
        "batch-catalog-partial",
        "source-current",
        ExternalImportBatchImportStatus::Pending,
    );
    let selection = fixture_selection(
        &batch,
        ExternalImportSelectionStatus::Editing,
        &["candidate-a", "candidate-b"],
        None,
    );
    repository.seed(
        &batch,
        &selection,
        &[
            fixture_candidate(&batch, "candidate-a", 1),
            fixture_candidate(&batch, "candidate-b", 2),
        ],
    );
    let catalog = Arc::new(FixtureCatalog::persists_first_then_fails());
    let sandbox_locator = Arc::new(FixtureSandboxLocator::default());
    let (service, task_manager) = fixture_service_with_sandbox_locator(
        Arc::clone(&repository),
        fixture_registry(Some(fixture_registration("source-current")), None),
        Arc::new(FixtureMaterializer::default()),
        Arc::clone(&catalog),
        Arc::new(FixtureCategoryRepository::new(0)),
        Arc::new(FixtureClock::available()),
        Arc::clone(&sandbox_locator),
    );

    let launch = service
        .start_import(&batch.batch_id, &selection.selection_id, selection.revision)
        .expect("start import");
    let events = service
        .run_import(launch.clone())
        .expect("terminal failure is an event");
    let results = repository.results(&batch.batch_id);

    assert_eq!(
        task_manager.task_status(&launch.task.task_id),
        Some(TaskStatus::Failed)
    );
    assert_eq!(
        repository.batch(&batch.batch_id).import_status,
        ExternalImportBatchImportStatus::Failed
    );
    assert_result(
        &results,
        "candidate-a",
        ExternalImportItemStatus::Imported,
        false,
        None,
    );
    assert_result(
        &results,
        "candidate-b",
        ExternalImportItemStatus::Failed,
        true,
        None,
    );
    assert_eq!(catalog.logical_mods().len(), 1);
    assert_eq!(
        sandbox_locator.cleaned_packages(),
        vec!["pkg-candidate-b".to_owned()],
        "the durable first catalog entry retains its sandbox while the failed entry is discarded"
    );
    assert_eq!(
        events.last().expect("terminal event").error.as_deref(),
        Some("external_import_catalog_unavailable")
    );
}

#[test]
fn catalog_conflict_with_another_batch_is_reconciled_as_already_imported() {
    let repository = Arc::new(FixtureBatchRepository::default());
    let batch = fixture_batch(
        "batch-catalog-concurrent",
        "source-current",
        ExternalImportBatchImportStatus::Pending,
    );
    let decision = ExternalImportSelectionDecision {
        conflict_resolution: None,
        category_id: Some("category-fixture".to_owned()),
    };
    let selection = fixture_selection(
        &batch,
        ExternalImportSelectionStatus::Editing,
        &["candidate-a"],
        Some(decision),
    );
    repository.seed(
        &batch,
        &selection,
        &[fixture_candidate(&batch, "candidate-a", 1)],
    );
    let catalog = Arc::new(FixtureCatalog::persists_foreign_duplicate_then_fails());
    let categories = Arc::new(FixtureCategoryRepository::new(0));
    let sandbox_locator = Arc::new(FixtureSandboxLocator::default());
    let (service, task_manager) = fixture_service_with_sandbox_locator(
        Arc::clone(&repository),
        fixture_registry(Some(fixture_registration("source-current")), None),
        Arc::new(FixtureMaterializer::default()),
        catalog,
        Arc::clone(&categories),
        Arc::new(FixtureClock::available()),
        Arc::clone(&sandbox_locator),
    );

    let launch = service
        .start_import(&batch.batch_id, &selection.selection_id, selection.revision)
        .expect("start import");
    service
        .run_import(launch.clone())
        .expect("catalog conflict becomes a durable terminal event");

    assert_eq!(
        task_manager.task_status(&launch.task.task_id),
        Some(TaskStatus::Failed)
    );
    let result = repository
        .results(&batch.batch_id)
        .into_iter()
        .find(|result| result.candidate_id.as_str() == "candidate-a")
        .expect("candidate result is durable");
    assert_eq!(result.status, ExternalImportItemStatus::AlreadyImported);
    assert_eq!(
        result.imported_mod_id,
        Some(ModId::new("foreign-existing-mod"))
    );
    assert!(!result.retryable);
    assert!(
        !categories.is_assigned("foreign-existing-mod", "category-fixture"),
        "an already-imported result must not mutate an existing Mod's user category overlay"
    );
    assert_eq!(
        sandbox_locator.cleaned_packages(),
        vec!["pkg-candidate-a".to_owned()],
        "a competing catalog entry must not retain this batch's sandbox"
    );
}

#[test]
fn cross_batch_duplicate_does_not_apply_a_new_category_to_the_existing_mod() {
    let repository = Arc::new(FixtureBatchRepository::default());
    let initial_batch = fixture_batch(
        "batch-initial-import",
        "source-current",
        ExternalImportBatchImportStatus::Pending,
    );
    let initial_selection = fixture_selection(
        &initial_batch,
        ExternalImportSelectionStatus::Editing,
        &["candidate-a"],
        None,
    );
    let duplicate_batch = fixture_batch(
        "batch-cross-batch-duplicate",
        "source-current",
        ExternalImportBatchImportStatus::Pending,
    );
    let duplicate_selection = fixture_selection(
        &duplicate_batch,
        ExternalImportSelectionStatus::Editing,
        &["candidate-a"],
        Some(ExternalImportSelectionDecision {
            conflict_resolution: None,
            category_id: Some("category-fixture".to_owned()),
        }),
    );
    repository.seed(
        &initial_batch,
        &initial_selection,
        &[fixture_candidate(&initial_batch, "candidate-a", 1)],
    );
    repository.seed(
        &duplicate_batch,
        &duplicate_selection,
        &[fixture_candidate(&duplicate_batch, "candidate-a", 1)],
    );

    let catalog = Arc::new(FixtureCatalog::succeeds());
    let categories = Arc::new(FixtureCategoryRepository::new(0));
    let (service, _) = fixture_service(
        Arc::clone(&repository),
        fixture_registry(Some(fixture_registration("source-current")), None),
        Arc::new(FixtureMaterializer::default()),
        Arc::clone(&catalog),
        Arc::clone(&categories),
        Arc::new(FixtureClock::available()),
    );

    let initial_launch = service
        .start_import(
            &initial_batch.batch_id,
            &initial_selection.selection_id,
            initial_selection.revision,
        )
        .expect("initial import starts");
    service
        .run_import(initial_launch)
        .expect("initial import succeeds");

    let duplicate_launch = service
        .start_import(
            &duplicate_batch.batch_id,
            &duplicate_selection.selection_id,
            duplicate_selection.revision,
        )
        .expect("duplicate import starts");
    service
        .run_import(duplicate_launch)
        .expect("duplicate import completes without catalog writes");

    let duplicate_results = repository.results(&duplicate_batch.batch_id);
    assert_result(
        &duplicate_results,
        "candidate-a",
        ExternalImportItemStatus::AlreadyImported,
        false,
        None,
    );
    assert_eq!(
        duplicate_results
            .iter()
            .find(|result| result.candidate_id.as_str() == "candidate-a")
            .expect("duplicate result exists")
            .imported_mod_id,
        Some(ModId::new("pkg-candidate-a"))
    );
    assert_eq!(catalog.logical_mods().len(), 1);
    assert!(
        !categories.is_assigned("pkg-candidate-a", "category-fixture"),
        "a cross-batch duplicate must not mutate the existing Mod's user category overlay"
    );
}

#[test]
fn category_failure_is_retryable_after_catalog_persistence() {
    let repository = Arc::new(FixtureBatchRepository::default());
    let batch = fixture_batch(
        "batch-category-retry",
        "source-current",
        ExternalImportBatchImportStatus::Pending,
    );
    let decision = ExternalImportSelectionDecision {
        conflict_resolution: None,
        category_id: Some("category-fixture".to_owned()),
    };
    let selection = fixture_selection(
        &batch,
        ExternalImportSelectionStatus::Editing,
        &["candidate-a"],
        Some(decision),
    );
    repository.seed(
        &batch,
        &selection,
        &[fixture_candidate(&batch, "candidate-a", 1)],
    );
    let categories = Arc::new(FixtureCategoryRepository::new(1));
    let (service, task_manager) = fixture_service(
        Arc::clone(&repository),
        fixture_registry(Some(fixture_registration("source-current")), None),
        Arc::new(FixtureMaterializer::default()),
        Arc::new(FixtureCatalog::succeeds()),
        Arc::clone(&categories),
        Arc::new(FixtureClock::available()),
    );

    let launch = service
        .start_import(&batch.batch_id, &selection.selection_id, selection.revision)
        .expect("start import");
    service
        .run_import(launch)
        .expect("category failure remains recoverable");
    assert_result(
        &repository.results(&batch.batch_id),
        "candidate-a",
        ExternalImportItemStatus::Failed,
        true,
        None,
    );
    assert_eq!(
        repository.batch(&batch.batch_id).import_status,
        ExternalImportBatchImportStatus::CompletedWithErrors
    );

    let retry = service
        .retry_import(&batch.batch_id, &selection.selection_id)
        .expect("retry terminal batch");
    service.run_import(retry.clone()).expect("retry succeeds");

    assert_eq!(
        task_manager.task_status(&retry.task.task_id),
        Some(TaskStatus::Completed)
    );
    assert_eq!(
        repository.batch(&batch.batch_id).import_status,
        ExternalImportBatchImportStatus::Completed
    );
    assert_result(
        &repository.results(&batch.batch_id),
        "candidate-a",
        ExternalImportItemStatus::AlreadyImported,
        false,
        None,
    );
    assert!(categories.is_assigned("pkg-candidate-a", "category-fixture"));
}

#[test]
fn cancellation_does_not_replace_an_existing_retryable_failure() {
    let repository = Arc::new(FixtureBatchRepository::default());
    let batch = fixture_batch(
        "batch-cancel-preserves-failure",
        "source-current",
        ExternalImportBatchImportStatus::Pending,
    );
    let selection = fixture_selection(
        &batch,
        ExternalImportSelectionStatus::Editing,
        &["candidate-a"],
        None,
    );
    let candidate = fixture_candidate(&batch, "candidate-a", 1);
    repository.seed(&batch, &selection, std::slice::from_ref(&candidate));
    let (service, task_manager) = fixture_service(
        Arc::clone(&repository),
        fixture_registry(Some(fixture_registration("source-current")), None),
        Arc::new(FixtureMaterializer::default()),
        Arc::new(FixtureCatalog::succeeds()),
        Arc::new(FixtureCategoryRepository::new(0)),
        Arc::new(FixtureClock::available()),
    );

    let launch = service
        .start_import(&batch.batch_id, &selection.selection_id, selection.revision)
        .expect("start import");
    repository
        .append_item_results(
            &batch.batch_id,
            &[ExternalImportItemResult {
                candidate_id: candidate.candidate_id,
                status: ExternalImportItemStatus::Failed,
                reason_code: None,
                imported_mod_id: None,
                retryable: true,
            }],
        )
        .expect("seed existing failure");
    task_manager
        .cancel_task(&launch.task.task_id)
        .expect("cancel queued task");

    service
        .run_import(launch.clone())
        .expect("cancel is terminal");

    assert_eq!(
        task_manager.task_status(&launch.task.task_id),
        Some(TaskStatus::Cancelled)
    );
    assert_eq!(
        repository.batch(&batch.batch_id).import_status,
        ExternalImportBatchImportStatus::Cancelled
    );
    assert_result(
        &repository.results(&batch.batch_id),
        "candidate-a",
        ExternalImportItemStatus::Failed,
        true,
        None,
    );
}

#[test]
fn analysis_failure_cleans_the_unpersisted_materialized_sandbox() {
    let repository = Arc::new(FixtureBatchRepository::default());
    let batch = fixture_batch(
        "batch-analysis-cleanup",
        "source-current",
        ExternalImportBatchImportStatus::Pending,
    );
    let selection = fixture_selection(
        &batch,
        ExternalImportSelectionStatus::Editing,
        &["candidate-a"],
        None,
    );
    repository.seed(
        &batch,
        &selection,
        &[fixture_candidate(&batch, "candidate-a", 1)],
    );
    let sandbox_locator = Arc::new(FixtureSandboxLocator::failing_for("pkg-candidate-a"));
    let (service, _) = fixture_service_with_sandbox_locator(
        Arc::clone(&repository),
        fixture_registry(Some(fixture_registration("source-current")), None),
        Arc::new(FixtureMaterializer::default()),
        Arc::new(FixtureCatalog::succeeds()),
        Arc::new(FixtureCategoryRepository::new(0)),
        Arc::new(FixtureClock::available()),
        Arc::clone(&sandbox_locator),
    );

    let launch = service
        .start_import(&batch.batch_id, &selection.selection_id, selection.revision)
        .expect("start import");
    service
        .run_import(launch)
        .expect("per-item analysis failure is terminalized as a result");

    assert_result(
        &repository.results(&batch.batch_id),
        "candidate-a",
        ExternalImportItemStatus::Failed,
        true,
        None,
    );
    assert_eq!(
        sandbox_locator.cleaned_packages(),
        vec!["pkg-candidate-a".to_owned()]
    );
}

#[test]
fn cancellation_after_materialization_cleans_the_unpersisted_sandbox() {
    let repository = Arc::new(FixtureBatchRepository::default());
    let batch = fixture_batch(
        "batch-cancel-cleanup",
        "source-current",
        ExternalImportBatchImportStatus::Pending,
    );
    let selection = fixture_selection(
        &batch,
        ExternalImportSelectionStatus::Editing,
        &["candidate-a"],
        None,
    );
    repository.seed(
        &batch,
        &selection,
        &[fixture_candidate(&batch, "candidate-a", 1)],
    );
    let materializer = Arc::new(FixtureMaterializer::default());
    let sandbox_locator = Arc::new(FixtureSandboxLocator::default());
    let (service, task_manager) = fixture_service_with_sandbox_locator(
        Arc::clone(&repository),
        fixture_registry(Some(fixture_registration("source-current")), None),
        Arc::clone(&materializer),
        Arc::new(FixtureCatalog::succeeds()),
        Arc::new(FixtureCategoryRepository::new(0)),
        Arc::new(FixtureClock::available()),
        Arc::clone(&sandbox_locator),
    );
    materializer.cancel_after_materialization(Arc::clone(&task_manager));

    let launch = service
        .start_import(&batch.batch_id, &selection.selection_id, selection.revision)
        .expect("start import");
    service
        .run_import(launch.clone())
        .expect("cancellation is terminalized");

    assert_eq!(
        task_manager.task_status(&launch.task.task_id),
        Some(TaskStatus::Cancelled)
    );
    assert_eq!(
        sandbox_locator.cleaned_packages(),
        vec!["pkg-candidate-a".to_owned()]
    );
}

#[test]
fn matching_reselected_source_still_rejects_a_changed_candidate() {
    let repository = Arc::new(FixtureBatchRepository::default());
    let batch = fixture_batch(
        "batch-reselect-source-changed",
        "source-expired",
        ExternalImportBatchImportStatus::Pending,
    );
    let selection = fixture_selection(
        &batch,
        ExternalImportSelectionStatus::Editing,
        &["candidate-a"],
        None,
    );
    repository.seed(
        &batch,
        &selection,
        &[fixture_candidate(&batch, "candidate-a", 1)],
    );
    let materializer = Arc::new(FixtureMaterializer::source_changed_for("candidate-a"));
    let (service, task_manager) = fixture_service(
        Arc::clone(&repository),
        fixture_registry(None, Some(fixture_registration("source-reselected"))),
        materializer,
        Arc::new(FixtureCatalog::succeeds()),
        Arc::new(FixtureCategoryRepository::new(0)),
        Arc::new(FixtureClock::available()),
    );

    let launch = service
        .start_import(&batch.batch_id, &selection.selection_id, selection.revision)
        .expect("matching source allows a retryable import launch");
    service
        .run_import(launch.clone())
        .expect("source change is a result, not a task failure");

    assert_eq!(
        task_manager.task_status(&launch.task.task_id),
        Some(TaskStatus::Completed)
    );
    assert_eq!(
        repository.batch(&batch.batch_id).source_id,
        Some(ExternalImportSourceId::new("source-reselected"))
    );
    assert_eq!(
        repository.batch(&batch.batch_id).import_status,
        ExternalImportBatchImportStatus::CompletedWithErrors
    );
    assert_result(
        &repository.results(&batch.batch_id),
        "candidate-a",
        ExternalImportItemStatus::Blocked,
        false,
        Some(ExternalImportReasonCode::SourceChanged),
    );
}

#[test]
fn materializer_output_must_match_the_selected_candidate_before_analysis() {
    let repository = Arc::new(FixtureBatchRepository::default());
    let batch = fixture_batch(
        "batch-materializer-mismatch",
        "source-current",
        ExternalImportBatchImportStatus::Pending,
    );
    let selection = fixture_selection(
        &batch,
        ExternalImportSelectionStatus::Editing,
        &["candidate-a"],
        None,
    );
    repository.seed(
        &batch,
        &selection,
        &[fixture_candidate(&batch, "candidate-a", 1)],
    );
    let catalog = Arc::new(FixtureCatalog::succeeds());
    let (service, task_manager) = fixture_service(
        Arc::clone(&repository),
        fixture_registry(Some(fixture_registration("source-current")), None),
        Arc::new(FixtureMaterializer::mismatch_for("candidate-a")),
        Arc::clone(&catalog),
        Arc::new(FixtureCategoryRepository::new(0)),
        Arc::new(FixtureClock::available()),
    );

    let launch = service
        .start_import(&batch.batch_id, &selection.selection_id, selection.revision)
        .expect("start import");
    service
        .run_import(launch.clone())
        .expect("mismatched materialization is a result, not a task failure");

    assert_eq!(
        task_manager.task_status(&launch.task.task_id),
        Some(TaskStatus::Completed)
    );
    assert_eq!(
        repository.batch(&batch.batch_id).import_status,
        ExternalImportBatchImportStatus::CompletedWithErrors
    );
    assert_result(
        &repository.results(&batch.batch_id),
        "candidate-a",
        ExternalImportItemStatus::Blocked,
        false,
        Some(ExternalImportReasonCode::SourceChanged),
    );
    assert!(catalog.logical_mods().is_empty());
}

#[test]
fn analysis_name_collision_within_one_catalog_chunk_requires_keep_both() {
    let repository = Arc::new(FixtureBatchRepository::default());
    let batch = fixture_batch(
        "batch-analysis-name-collision",
        "source-current",
        ExternalImportBatchImportStatus::Pending,
    );
    let selection = fixture_selection(
        &batch,
        ExternalImportSelectionStatus::Editing,
        &["candidate-a", "candidate-b"],
        None,
    );
    repository.seed(
        &batch,
        &selection,
        &[
            fixture_candidate(&batch, "candidate-a", 1),
            fixture_candidate(&batch, "candidate-b", 2),
        ],
    );
    let catalog = Arc::new(FixtureCatalog::succeeds());
    let sandbox_locator = Arc::new(FixtureSandboxLocator::default());
    let (service, _) = fixture_service_with_analysis_display_name_and_sandbox_locator(
        Arc::clone(&repository),
        fixture_registry(Some(fixture_registration("source-current")), None),
        Arc::new(FixtureMaterializer::default()),
        Arc::clone(&catalog),
        Arc::new(FixtureCategoryRepository::new(0)),
        Arc::new(FixtureClock::available()),
        Some("Shared analyzed name".to_owned()),
        Arc::clone(&sandbox_locator),
    );

    let launch = service
        .start_import(&batch.batch_id, &selection.selection_id, selection.revision)
        .expect("start import");
    service
        .run_import(launch)
        .expect("batch reaches a terminal state");

    assert_eq!(
        repository.batch(&batch.batch_id).import_status,
        ExternalImportBatchImportStatus::CompletedWithErrors
    );
    assert_eq!(catalog.logical_mods().len(), 1);
    assert_result(
        &repository.results(&batch.batch_id),
        "candidate-a",
        ExternalImportItemStatus::Imported,
        false,
        None,
    );
    assert_result(
        &repository.results(&batch.batch_id),
        "candidate-b",
        ExternalImportItemStatus::Blocked,
        false,
        Some(ExternalImportReasonCode::NameCollision),
    );
    assert_eq!(
        sandbox_locator.cleaned_packages(),
        vec!["pkg-candidate-b".to_owned()]
    );
}

#[test]
fn result_page_limits_are_rejected_before_repository_access() {
    let repository = Arc::new(FixtureBatchRepository::default());
    let (service, _) = fixture_service(
        repository,
        fixture_registry(Some(fixture_registration("source-current")), None),
        Arc::new(FixtureMaterializer::default()),
        Arc::new(FixtureCatalog::succeeds()),
        Arc::new(FixtureCategoryRepository::new(0)),
        Arc::new(FixtureClock::available()),
    );
    let unknown_batch = ExternalImportBatchId::new("batch-page-limits");

    assert_eq!(
        service.get_results(&unknown_batch, 0, 0),
        Err(ExternalImportBatchError::ResultPageInvalid)
    );
    assert_eq!(
        service.get_results(&unknown_batch, 0, 101),
        Err(ExternalImportBatchError::ResultPageInvalid)
    );
}

fn fixture_service(
    repository: Arc<FixtureBatchRepository>,
    source_registry: Arc<FixtureSourceRegistry>,
    materializer: Arc<FixtureMaterializer>,
    catalog: Arc<FixtureCatalog>,
    categories: Arc<FixtureCategoryRepository>,
    clock: Arc<FixtureClock>,
) -> (ExternalImportBatchService, Arc<TaskManager>) {
    fixture_service_with_analysis_display_name(
        repository,
        source_registry,
        materializer,
        catalog,
        categories,
        clock,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
fn fixture_service_with_analysis_display_name(
    repository: Arc<FixtureBatchRepository>,
    source_registry: Arc<FixtureSourceRegistry>,
    materializer: Arc<FixtureMaterializer>,
    catalog: Arc<FixtureCatalog>,
    categories: Arc<FixtureCategoryRepository>,
    clock: Arc<FixtureClock>,
    display_name: Option<String>,
) -> (ExternalImportBatchService, Arc<TaskManager>) {
    fixture_service_with_analysis_display_name_and_sandbox_locator(
        repository,
        source_registry,
        materializer,
        catalog,
        categories,
        clock,
        display_name,
        Arc::new(FixtureSandboxLocator::default()),
    )
}

#[allow(clippy::too_many_arguments)]
fn fixture_service_with_sandbox_locator(
    repository: Arc<FixtureBatchRepository>,
    source_registry: Arc<FixtureSourceRegistry>,
    materializer: Arc<FixtureMaterializer>,
    catalog: Arc<FixtureCatalog>,
    categories: Arc<FixtureCategoryRepository>,
    clock: Arc<FixtureClock>,
    sandbox_locator: Arc<FixtureSandboxLocator>,
) -> (ExternalImportBatchService, Arc<TaskManager>) {
    fixture_service_with_analysis_display_name_and_sandbox_locator(
        repository,
        source_registry,
        materializer,
        catalog,
        categories,
        clock,
        None,
        sandbox_locator,
    )
}

#[allow(clippy::too_many_arguments)]
fn fixture_service_with_analysis_display_name_and_sandbox_locator(
    repository: Arc<FixtureBatchRepository>,
    source_registry: Arc<FixtureSourceRegistry>,
    materializer: Arc<FixtureMaterializer>,
    catalog: Arc<FixtureCatalog>,
    categories: Arc<FixtureCategoryRepository>,
    clock: Arc<FixtureClock>,
    display_name: Option<String>,
    sandbox_locator: Arc<FixtureSandboxLocator>,
) -> (ExternalImportBatchService, Arc<TaskManager>) {
    let task_manager = Arc::new(TaskManager::new());
    let prepare_service = Arc::new(ModImportPrepareService::new(
        Box::new(NoopPackagePreparer),
        ModImportAnalysisService::new(
            Box::new(FallbackPreviewProcessor),
            Box::new(NoopThumbnailStore),
            Box::new(FixtureMetadataAnalyzer { display_name }),
        ),
    ));
    let service = ExternalImportBatchService::new(
        Arc::clone(&task_manager),
        source_registry,
        materializer,
        repository,
        catalog,
        categories,
        sandbox_locator,
        prepare_service,
        clock,
    );
    (service, task_manager)
}

fn fixture_registry(
    direct: Option<ExternalImportSourceRegistration>,
    matching: Option<ExternalImportSourceRegistration>,
) -> Arc<FixtureSourceRegistry> {
    Arc::new(FixtureSourceRegistry { direct, matching })
}

fn fixture_registration(source_id: &str) -> ExternalImportSourceRegistration {
    ExternalImportSourceRegistration {
        source: ExternalImportSource {
            source_id: ExternalImportSourceId::new(source_id),
            adapter_id: ExternalImportAdapterId::new("hunting_box_directory_v1"),
            display_label: "Fixture source".to_owned(),
            expires_at_unix_millis: 10_000,
        },
        source_fingerprint: "fixture-source-fingerprint".to_owned(),
    }
}

fn fixture_batch(
    batch_id: &str,
    source_id: &str,
    import_status: ExternalImportBatchImportStatus,
) -> ExternalImportBatch {
    ExternalImportBatch {
        batch_id: ExternalImportBatchId::new(batch_id),
        source_id: Some(ExternalImportSourceId::new(source_id)),
        adapter_id: ExternalImportAdapterId::new("hunting_box_directory_v1"),
        source_fingerprint: "fixture-source-fingerprint".to_owned(),
        scan_status: ExternalImportScanStatus::Completed,
        import_status,
        created_at_unix_millis: 1,
    }
}

fn fixture_selection(
    batch: &ExternalImportBatch,
    status: ExternalImportSelectionStatus,
    candidate_ids: &[&str],
    decision: Option<ExternalImportSelectionDecision>,
) -> ExternalImportSelection {
    ExternalImportSelection {
        selection_id: ExternalImportSelectionId::new(format!(
            "selection-{}",
            batch.batch_id.as_str()
        )),
        batch_id: batch.batch_id.clone(),
        revision: 0,
        status,
        entries: candidate_ids
            .iter()
            .map(|candidate_id| ExternalImportSelectionEntry {
                candidate_id: ExternalImportCandidateId::new(*candidate_id),
                decision: decision.clone(),
                updated_at_unix_millis: 1,
            })
            .collect(),
        selected_resource_usage: ExternalImportResourceUsage {
            file_count: candidate_ids.len() as u64,
            source_bytes: candidate_ids.len() as u64,
            materialization_bytes: candidate_ids.len() as u64,
        },
        expires_at_unix_millis: 10_000,
    }
}

fn fixture_candidate(
    batch: &ExternalImportBatch,
    candidate_id: &str,
    fingerprint_number: u64,
) -> ExternalImportCandidate {
    ExternalImportCandidate {
        batch_id: batch.batch_id.clone(),
        candidate_id: ExternalImportCandidateId::new(candidate_id),
        source_item_key_hash: format!("source-key-{candidate_id}"),
        content_fingerprint: format!("sha256:{fingerprint_number:064x}"),
        metadata_hint: ExternalImportMetadataHint {
            display_name: Some(format!("Fixture {candidate_id}")),
            author: Some("Fixture author".to_owned()),
            version: Some("1.0".to_owned()),
            source_mod_type: None,
        },
        resource_usage: ExternalImportResourceUsage {
            file_count: 1,
            source_bytes: 1,
            materialization_bytes: 1,
        },
        preview_status: ExternalImportCandidateStatus::Ready,
        conflict_kind: ExternalImportConflictKind::None,
    }
}

fn assert_result(
    results: &[ExternalImportItemResult],
    candidate_id: &str,
    status: ExternalImportItemStatus,
    retryable: bool,
    reason_code: Option<ExternalImportReasonCode>,
) {
    let result = results
        .iter()
        .find(|result| result.candidate_id.as_str() == candidate_id)
        .expect("candidate result exists");
    assert_eq!(result.status, status);
    assert_eq!(result.retryable, retryable);
    assert_eq!(result.reason_code, reason_code);
}

fn unix_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time after epoch")
        .as_millis()
}

fn assert_created_task_terminal(
    task_manager: &TaskManager,
    sequence: u64,
    started_at: u128,
    finished_at: u128,
    expected: TaskStatus,
) {
    let statuses = (started_at.saturating_sub(1)..=finished_at.saturating_add(1))
        .filter_map(|millis| task_manager.task_status(&format!("mod-import-{millis}-{sequence}")))
        .collect::<Vec<_>>();
    assert_eq!(statuses, vec![expected]);
}

#[derive(Default)]
struct FixtureBatchRepository {
    state: Mutex<FixtureBatchRepositoryState>,
    fail_seal: Mutex<bool>,
    fail_restart: Mutex<bool>,
}

#[derive(Default)]
struct FixtureBatchRepositoryState {
    batches: BTreeMap<String, ExternalImportBatch>,
    selections: BTreeMap<String, ExternalImportSelection>,
    candidates: BTreeMap<String, Vec<ExternalImportCandidate>>,
    results: BTreeMap<String, Vec<ExternalImportItemResult>>,
}

impl FixtureBatchRepository {
    fn seed(
        &self,
        batch: &ExternalImportBatch,
        selection: &ExternalImportSelection,
        candidates: &[ExternalImportCandidate],
    ) {
        let mut state = self.state.lock().expect("fixture repository lock");
        state
            .batches
            .insert(batch.batch_id.as_str().to_owned(), batch.clone());
        state.selections.insert(
            selection.selection_id.as_str().to_owned(),
            selection.clone(),
        );
        state
            .candidates
            .insert(batch.batch_id.as_str().to_owned(), candidates.to_vec());
    }

    fn set_fail_seal(&self, value: bool) {
        *self.fail_seal.lock().expect("seal mode lock") = value;
    }

    fn set_fail_restart(&self, value: bool) {
        *self.fail_restart.lock().expect("restart mode lock") = value;
    }

    fn batch(&self, batch_id: &ExternalImportBatchId) -> ExternalImportBatch {
        self.state
            .lock()
            .expect("fixture repository lock")
            .batches
            .get(batch_id.as_str())
            .cloned()
            .expect("batch exists")
    }

    fn results(&self, batch_id: &ExternalImportBatchId) -> Vec<ExternalImportItemResult> {
        self.state
            .lock()
            .expect("fixture repository lock")
            .results
            .get(batch_id.as_str())
            .cloned()
            .unwrap_or_default()
    }
}

impl ExternalImportBatchRepository for FixtureBatchRepository {
    fn create_batch(&self, batch: &ExternalImportBatch) -> Result<()> {
        self.state
            .lock()
            .expect("fixture repository lock")
            .batches
            .insert(batch.batch_id.as_str().to_owned(), batch.clone());
        Ok(())
    }

    fn get_batch(&self, batch_id: &ExternalImportBatchId) -> Result<Option<ExternalImportBatch>> {
        Ok(self
            .state
            .lock()
            .expect("fixture repository lock")
            .batches
            .get(batch_id.as_str())
            .cloned())
    }

    fn update_batch(&self, batch: &ExternalImportBatch) -> Result<()> {
        let mut state = self.state.lock().expect("fixture repository lock");
        if !state.batches.contains_key(batch.batch_id.as_str()) {
            bail!("fixture batch is unavailable");
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
        self.update_batch(batch)?;
        self.replace_candidates(&batch.batch_id, candidates)
    }

    fn replace_candidates(
        &self,
        batch_id: &ExternalImportBatchId,
        candidates: &[ExternalImportCandidate],
    ) -> Result<()> {
        self.state
            .lock()
            .expect("fixture repository lock")
            .candidates
            .insert(batch_id.as_str().to_owned(), candidates.to_vec());
        Ok(())
    }

    fn list_candidates(
        &self,
        batch_id: &ExternalImportBatchId,
    ) -> Result<Vec<ExternalImportCandidate>> {
        Ok(self
            .state
            .lock()
            .expect("fixture repository lock")
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
        let all = self.list_candidates(batch_id)?;
        let total_count = all.len();
        let candidates = all.into_iter().skip(offset).take(limit).collect::<Vec<_>>();
        let next_offset = offset
            .checked_add(candidates.len())
            .filter(|next_offset| *next_offset < total_count);
        Ok(ExternalImportCandidatePage {
            candidates,
            total_count,
            next_offset,
        })
    }

    fn create_selection(&self, selection: &ExternalImportSelection) -> Result<()> {
        self.state
            .lock()
            .expect("fixture repository lock")
            .selections
            .insert(
                selection.selection_id.as_str().to_owned(),
                selection.clone(),
            );
        Ok(())
    }

    fn get_selection(
        &self,
        selection_id: &ExternalImportSelectionId,
    ) -> Result<Option<ExternalImportSelection>> {
        Ok(self
            .state
            .lock()
            .expect("fixture repository lock")
            .selections
            .get(selection_id.as_str())
            .cloned())
    }

    fn compare_and_swap_selection(
        &self,
        request: ExternalImportSelectionCompareAndSwapRequest<'_>,
    ) -> Result<ExternalImportSelectionCompareAndSwapResult> {
        let mut state = self.state.lock().expect("fixture repository lock");
        let current = state
            .selections
            .get(request.selection.selection_id.as_str())
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("fixture selection is unavailable"))?;
        if current.revision != request.expected_revision {
            return Ok(
                ExternalImportSelectionCompareAndSwapResult::RevisionConflict {
                    current_revision: current.revision,
                },
            );
        }
        state.selections.insert(
            request.selection.selection_id.as_str().to_owned(),
            request.selection.clone(),
        );
        Ok(ExternalImportSelectionCompareAndSwapResult::Applied(
            request.selection.clone(),
        ))
    }

    fn seal_selection_and_start(
        &self,
        request: ExternalImportSealAndStartRequest<'_>,
    ) -> Result<ExternalImportSealAndStartResult> {
        if *self.fail_seal.lock().expect("seal mode lock") {
            bail!("fixture sealed start failed");
        }
        let mut state = self.state.lock().expect("fixture repository lock");
        let mut selection = state
            .selections
            .get(request.selection_id.as_str())
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("fixture selection is unavailable"))?;
        if selection.revision != request.expected_revision {
            return Ok(ExternalImportSealAndStartResult::RevisionConflict {
                current_revision: selection.revision,
            });
        }
        if selection.entries.is_empty() {
            return Ok(ExternalImportSealAndStartResult::SelectionRejected {
                error: hmm_core::ExternalImportSelectionError::Empty,
            });
        }
        let mut batch = state
            .batches
            .get(selection.batch_id.as_str())
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("fixture batch is unavailable"))?;
        if batch.scan_status != ExternalImportScanStatus::Completed
            || batch.import_status != ExternalImportBatchImportStatus::Pending
        {
            return Ok(ExternalImportSealAndStartResult::BatchNotStartable);
        }
        selection.status = ExternalImportSelectionStatus::Sealed;
        selection.revision = selection
            .revision
            .checked_add(1)
            .ok_or_else(|| anyhow::anyhow!("fixture selection revision overflow"))?;
        batch.import_status = ExternalImportBatchImportStatus::Running;
        state.selections.insert(
            selection.selection_id.as_str().to_owned(),
            selection.clone(),
        );
        state
            .batches
            .insert(batch.batch_id.as_str().to_owned(), batch.clone());
        Ok(ExternalImportSealAndStartResult::Started {
            batch,
            selection: Box::new(selection),
        })
    }

    fn restart_batch(
        &self,
        batch_id: &ExternalImportBatchId,
    ) -> Result<Option<ExternalImportBatch>> {
        if *self.fail_restart.lock().expect("restart mode lock") {
            bail!("fixture restart failed");
        }
        let mut state = self.state.lock().expect("fixture repository lock");
        let mut batch = match state.batches.get(batch_id.as_str()).cloned() {
            Some(batch) => batch,
            None => return Ok(None),
        };
        if !matches!(
            batch.import_status,
            ExternalImportBatchImportStatus::CompletedWithErrors
                | ExternalImportBatchImportStatus::Failed
                | ExternalImportBatchImportStatus::Cancelled
        ) {
            return Ok(None);
        }
        batch.import_status = ExternalImportBatchImportStatus::Running;
        state
            .batches
            .insert(batch.batch_id.as_str().to_owned(), batch.clone());
        Ok(Some(batch))
    }

    fn append_item_results(
        &self,
        batch_id: &ExternalImportBatchId,
        results: &[ExternalImportItemResult],
    ) -> Result<()> {
        let mut state = self.state.lock().expect("fixture repository lock");
        let stored = state
            .results
            .entry(batch_id.as_str().to_owned())
            .or_default();
        for result in results {
            if let Some(existing) = stored
                .iter_mut()
                .find(|existing| existing.candidate_id == result.candidate_id)
            {
                *existing = result.clone();
            } else {
                stored.push(result.clone());
            }
        }
        Ok(())
    }

    fn list_item_results(
        &self,
        batch_id: &ExternalImportBatchId,
    ) -> Result<Vec<ExternalImportItemResult>> {
        Ok(self.results(batch_id))
    }

    fn list_item_results_page(
        &self,
        batch_id: &ExternalImportBatchId,
        offset: usize,
        limit: usize,
    ) -> Result<ExternalImportItemResultPage> {
        let all = self.results(batch_id);
        let total_count = all.len();
        let results = all.into_iter().skip(offset).take(limit).collect::<Vec<_>>();
        let next_offset = offset
            .checked_add(results.len())
            .filter(|next_offset| *next_offset < total_count);
        Ok(ExternalImportItemResultPage {
            results,
            total_count,
            next_offset,
        })
    }
}

struct FixtureSourceRegistry {
    direct: Option<ExternalImportSourceRegistration>,
    matching: Option<ExternalImportSourceRegistration>,
}

impl ExternalImportSourceRegistry for FixtureSourceRegistry {
    fn resolve_source(
        &self,
        source_id: &ExternalImportSourceId,
    ) -> Result<Option<ExternalImportSourceRegistration>> {
        Ok(self
            .direct
            .as_ref()
            .filter(|registration| registration.source.source_id == *source_id)
            .cloned())
    }

    fn resolve_matching_source(
        &self,
        source_fingerprint: &str,
    ) -> Result<Option<ExternalImportSourceRegistration>> {
        Ok(self
            .matching
            .as_ref()
            .filter(|registration| registration.source_fingerprint == source_fingerprint)
            .cloned())
    }
}

#[derive(Default)]
struct FixtureMaterializer {
    source_changed: BTreeSet<String>,
    mismatched: BTreeSet<String>,
    cancellation_task_manager: Mutex<Option<Arc<TaskManager>>>,
}

impl FixtureMaterializer {
    fn source_changed_for(candidate_id: &str) -> Self {
        Self {
            source_changed: [candidate_id.to_owned()].into_iter().collect(),
            mismatched: BTreeSet::new(),
            cancellation_task_manager: Mutex::new(None),
        }
    }

    fn mismatch_for(candidate_id: &str) -> Self {
        Self {
            source_changed: BTreeSet::new(),
            mismatched: [candidate_id.to_owned()].into_iter().collect(),
            cancellation_task_manager: Mutex::new(None),
        }
    }

    fn cancel_after_materialization(&self, task_manager: Arc<TaskManager>) {
        *self
            .cancellation_task_manager
            .lock()
            .expect("fixture materializer cancellation lock") = Some(task_manager);
    }
}

impl ExternalImportMaterializer for FixtureMaterializer {
    fn materialize(
        &self,
        request: ExternalImportMaterializeRequest<'_>,
    ) -> Result<ExternalImportMaterializationOutcome> {
        if self
            .source_changed
            .contains(request.candidate.candidate_id.as_str())
        {
            return Ok(ExternalImportMaterializationOutcome::SourceChanged);
        }
        if self
            .mismatched
            .contains(request.candidate.candidate_id.as_str())
        {
            return Ok(ExternalImportMaterializationOutcome::Materialized(
                ExternalImportMaterializedPackage {
                    candidate_id: ExternalImportCandidateId::new("candidate-mismatch"),
                    package_id: format!("pkg-{}", request.candidate.candidate_id.as_str()),
                    content_fingerprint: request.expected_content_fingerprint.to_owned(),
                    resource_usage: request.candidate.resource_usage,
                },
            ));
        }
        let package = ExternalImportMaterializedPackage {
            candidate_id: request.candidate.candidate_id.clone(),
            package_id: format!("pkg-{}", request.candidate.candidate_id.as_str()),
            content_fingerprint: request.expected_content_fingerprint.to_owned(),
            resource_usage: request.candidate.resource_usage,
        };
        if let Some(task_manager) = self
            .cancellation_task_manager
            .lock()
            .expect("fixture materializer cancellation lock")
            .as_ref()
        {
            let _ = task_manager.cancel_task(request.task_id);
        }
        Ok(ExternalImportMaterializationOutcome::Materialized(package))
    }
}

enum FixtureCatalogMode {
    Succeeds,
    PersistsFirstThenFails,
    PersistsForeignDuplicateThenFails,
}

struct FixtureCatalog {
    mode: FixtureCatalogMode,
    entries: Mutex<Vec<ModImportCatalogUpsert>>,
}

impl FixtureCatalog {
    fn succeeds() -> Self {
        Self {
            mode: FixtureCatalogMode::Succeeds,
            entries: Mutex::new(Vec::new()),
        }
    }

    fn persists_first_then_fails() -> Self {
        Self {
            mode: FixtureCatalogMode::PersistsFirstThenFails,
            entries: Mutex::new(Vec::new()),
        }
    }

    fn persists_foreign_duplicate_then_fails() -> Self {
        Self {
            mode: FixtureCatalogMode::PersistsForeignDuplicateThenFails,
            entries: Mutex::new(Vec::new()),
        }
    }

    fn store(&self, upsert: &ModImportCatalogUpsert) {
        let mut entries = self.entries.lock().expect("catalog lock");
        entries.retain(|existing| existing.logical_mod.mod_id != upsert.logical_mod.mod_id);
        entries.push(upsert.clone());
    }

    fn logical_mods(&self) -> Vec<StoredLogicalMod> {
        self.entries
            .lock()
            .expect("catalog lock")
            .iter()
            .map(|entry| entry.logical_mod.clone())
            .collect()
    }
}

impl ModImportResultRepository for FixtureCatalog {
    fn upsert_many(&self, upserts: &[ModImportCatalogUpsert]) -> Result<()> {
        match self.mode {
            FixtureCatalogMode::Succeeds => {
                for upsert in upserts {
                    self.store(upsert);
                }
                Ok(())
            }
            FixtureCatalogMode::PersistsFirstThenFails => {
                if let Some(upsert) = upserts.first() {
                    self.store(upsert);
                }
                bail!("fixture JSON catalog failed after its first durable upsert")
            }
            FixtureCatalogMode::PersistsForeignDuplicateThenFails => {
                if let Some(upsert) = upserts.first() {
                    let mut foreign = upsert.clone();
                    let foreign_mod_id = ModId::new("foreign-existing-mod");
                    let foreign_revision_id =
                        hmm_core::ModRevisionId::new("foreign-existing-revision");
                    foreign.logical_mod.mod_id = foreign_mod_id.clone();
                    foreign.logical_mod.origin_revision_id = foreign_revision_id.clone();
                    foreign.logical_mod.display_revision_id = foreign_revision_id.clone();
                    foreign.revision.mod_id = foreign_mod_id;
                    foreign.revision.revision_id = foreign_revision_id;
                    foreign.revision.package_id = "foreign-existing-package".to_owned();
                    self.store(&foreign);
                }
                bail!("fixture JSON catalog rejected an externally persisted content duplicate")
            }
        }
    }

    fn list_mods(&self) -> Result<Vec<StoredLogicalMod>> {
        Ok(self.logical_mods())
    }

    fn get_revision(
        &self,
        revision_id: &hmm_core::ModRevisionId,
    ) -> Result<Option<StoredModRevision>> {
        Ok(self
            .entries
            .lock()
            .expect("catalog lock")
            .iter()
            .find(|entry| entry.revision.revision_id == *revision_id)
            .map(|entry| entry.revision.clone()))
    }

    fn list_revisions(&self, mod_id: &ModId) -> Result<Vec<StoredModRevision>> {
        Ok(self
            .entries
            .lock()
            .expect("catalog lock")
            .iter()
            .filter(|entry| entry.logical_mod.mod_id == *mod_id)
            .map(|entry| entry.revision.clone())
            .collect())
    }

    fn save_analysis(&self, _analysis: &StoredModImportAnalysis) -> Result<()> {
        Ok(())
    }

    fn list_analysis(&self) -> Result<Vec<StoredModImportAnalysis>> {
        Ok(self
            .entries
            .lock()
            .expect("catalog lock")
            .iter()
            .map(|entry| entry.revision.as_analysis())
            .collect())
    }

    fn get_analysis(&self, mod_id: &str) -> Result<Option<StoredModImportAnalysis>> {
        Ok(self
            .entries
            .lock()
            .expect("catalog lock")
            .iter()
            .find(|entry| entry.logical_mod.mod_id.as_str() == mod_id)
            .map(|entry| entry.revision.as_analysis()))
    }
}

struct FixtureCategoryRepository {
    category: Category,
    assignments: Mutex<BTreeMap<String, Vec<String>>>,
    fail_set_calls_remaining: Mutex<usize>,
}

impl FixtureCategoryRepository {
    fn new(fail_set_calls: usize) -> Self {
        Self {
            category: Category {
                id: "category-fixture".to_owned(),
                name: "Fixture category".to_owned(),
                color: None,
                sort_order: 0,
                created_at: 0,
            },
            assignments: Mutex::new(BTreeMap::new()),
            fail_set_calls_remaining: Mutex::new(fail_set_calls),
        }
    }

    fn is_assigned(&self, mod_id: &str, category_id: &str) -> bool {
        self.assignments
            .lock()
            .expect("category assignments lock")
            .get(mod_id)
            .is_some_and(|categories| categories.iter().any(|category| category == category_id))
    }
}

impl CategoryRepository for FixtureCategoryRepository {
    fn get(&self, category_id: &str) -> Result<Option<Category>> {
        Ok((category_id == self.category.id).then(|| self.category.clone()))
    }

    fn save(&self, _category: &Category) -> Result<()> {
        Ok(())
    }

    fn delete(&self, _category_id: &str) -> Result<()> {
        Ok(())
    }

    fn list_all(&self) -> Result<Vec<Category>> {
        Ok(vec![self.category.clone()])
    }

    fn count_mods(&self, category_id: &str) -> Result<u32> {
        Ok(self
            .assignments
            .lock()
            .expect("category assignments lock")
            .values()
            .filter(|categories| categories.iter().any(|category| category == category_id))
            .count() as u32)
    }

    fn get_mod_categories(&self, mod_id: &str) -> Result<Vec<Category>> {
        Ok(self
            .assignments
            .lock()
            .expect("category assignments lock")
            .get(mod_id)
            .into_iter()
            .flatten()
            .filter(|category_id| *category_id == &self.category.id)
            .map(|_| self.category.clone())
            .collect())
    }

    fn set_mod_categories(&self, mod_id: &str, category_ids: &[String]) -> Result<()> {
        let mut remaining = self
            .fail_set_calls_remaining
            .lock()
            .expect("category failure lock");
        if *remaining > 0 {
            *remaining -= 1;
            bail!("fixture category repository write failed");
        }
        self.assignments
            .lock()
            .expect("category assignments lock")
            .insert(mod_id.to_owned(), category_ids.to_vec());
        Ok(())
    }

    fn list_mod_category_pairs(&self) -> Result<Vec<(String, Category)>> {
        Ok(self
            .assignments
            .lock()
            .expect("category assignments lock")
            .iter()
            .flat_map(|(mod_id, category_ids)| {
                category_ids
                    .iter()
                    .filter(|category_id| *category_id == &self.category.id)
                    .map(|_| (mod_id.clone(), self.category.clone()))
                    .collect::<Vec<_>>()
            })
            .collect())
    }
}

struct FixtureClock {
    value: Option<u128>,
}

impl FixtureClock {
    fn available() -> Self {
        Self { value: Some(1) }
    }

    fn unavailable() -> Self {
        Self { value: None }
    }
}

impl AppClock for FixtureClock {
    fn now_unix_millis(&self) -> Result<u128> {
        self.value
            .ok_or_else(|| anyhow::anyhow!("fixture clock unavailable"))
    }
}

#[derive(Default)]
struct FixtureSandboxLocator {
    failing_packages: BTreeSet<String>,
    cleaned_packages: Mutex<Vec<String>>,
}

impl FixtureSandboxLocator {
    fn failing_for(package_id: &str) -> Self {
        Self {
            failing_packages: [package_id.to_owned()].into_iter().collect(),
            cleaned_packages: Mutex::new(Vec::new()),
        }
    }

    fn cleaned_packages(&self) -> Vec<String> {
        self.cleaned_packages
            .lock()
            .expect("fixture sandbox cleanup lock")
            .clone()
    }
}

impl ModImportSandboxLocator for FixtureSandboxLocator {
    fn sandbox_root_for_package(&self, package_id: &str) -> Result<PathBuf> {
        if self.failing_packages.contains(package_id) {
            bail!("fixture sandbox lookup failed");
        }
        Ok(PathBuf::from("fixture-sandbox"))
    }

    fn cleanup_sandbox_for_package(&self, package_id: &str) -> Result<()> {
        self.cleaned_packages
            .lock()
            .expect("fixture sandbox cleanup lock")
            .push(package_id.to_owned());
        Ok(())
    }
}

struct NoopPackagePreparer;

impl ModImportPackagePreparer for NoopPackagePreparer {
    fn prepare_package(
        &self,
        _request: ModImportPackagePrepareRequest<'_>,
    ) -> Result<PreparedModPackage> {
        Ok(PreparedModPackage {
            package_id: "unused-package".to_owned(),
            sandbox_root: PathBuf::from("fixture-sandbox"),
        })
    }
}

struct FallbackPreviewProcessor;

impl ImportPreviewImageProcessor for FallbackPreviewProcessor {
    fn process_package_preview(
        &self,
        _task_id: &str,
        _package_id: &str,
        _sandbox_root: &Path,
    ) -> Result<PreviewImageProcessingResult> {
        Ok(PreviewImageProcessingResult::Fallback(
            PreviewImageRejectionReason::Missing,
        ))
    }
}

struct NoopThumbnailStore;

impl ThumbnailStore for NoopThumbnailStore {
    fn put_thumbnail(
        &self,
        _package_id: &str,
        _content_hash: &str,
        _variant: &str,
        _extension: &str,
        _bytes: &[u8],
    ) -> Result<ThumbnailRef> {
        bail!("fallback preview never writes thumbnails")
    }

    fn resolve_url(&self, _thumbnail_ref: &ThumbnailRef) -> Result<String> {
        bail!("fallback preview never resolves thumbnails")
    }
}

struct FixtureMetadataAnalyzer {
    display_name: Option<String>,
}

impl ModPackageMetadataAnalyzer for FixtureMetadataAnalyzer {
    fn analyze_metadata(
        &self,
        package_id: &str,
        _sandbox_root: &Path,
    ) -> Result<ModPackageMetadata> {
        Ok(ModPackageMetadata {
            display_name: self
                .display_name
                .clone()
                .or_else(|| Some(format!("Fixture {package_id}"))),
            ..ModPackageMetadata::default()
        })
    }
}
