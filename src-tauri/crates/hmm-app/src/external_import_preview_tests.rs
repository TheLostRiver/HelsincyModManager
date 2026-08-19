use super::*;
use hmm_core::ExternalImportConflictResolution;

#[test]
fn preview_without_selection_returns_an_unselected_page() {
    let repository = Arc::new(FixtureBatchRepository::default());
    let batch = fixture_batch(
        "batch-preview-without-selection",
        "source-current",
        ExternalImportBatchImportStatus::Pending,
    );
    let unused_selection =
        fixture_selection(&batch, ExternalImportSelectionStatus::Editing, &[], None);
    repository.seed(
        &batch,
        &unused_selection,
        &[
            fixture_candidate(&batch, "candidate-a", 1),
            fixture_candidate(&batch, "candidate-b", 2),
        ],
    );
    let (service, _) = fixture_service(
        repository,
        fixture_registry(Some(fixture_registration("source-current")), None),
        Arc::new(FixtureMaterializer::default()),
        Arc::new(FixtureCatalog::succeeds()),
        Arc::new(FixtureCategoryRepository::new(0)),
        Arc::new(FixtureClock::available()),
    );

    let page = service
        .get_preview(&batch.batch_id, None, 0, 1)
        .expect("unbound preview is readable");

    assert!(page.selection.is_none());
    assert_eq!(page.total_count, 2);
    assert_eq!(page.next_offset, Some(1));
    assert_eq!(page.candidates.len(), 1);
    assert_eq!(
        page.candidates[0].candidate.candidate_id.as_str(),
        "candidate-a"
    );
    assert!(!page.candidates[0].selected);
    assert!(page.candidates[0].selection_decision.is_none());
}

#[test]
fn preview_with_selection_projects_only_the_current_page_decisions() {
    let repository = Arc::new(FixtureBatchRepository::default());
    let batch = fixture_batch(
        "batch-preview-with-selection",
        "source-current",
        ExternalImportBatchImportStatus::Pending,
    );
    let decision = ExternalImportSelectionDecision {
        conflict_resolution: Some(ExternalImportConflictResolution::KeepBoth),
        category_id: Some("category-fixture".to_owned()),
    };
    let selection = fixture_selection(
        &batch,
        ExternalImportSelectionStatus::Editing,
        &["candidate-b"],
        Some(decision.clone()),
    );
    repository.seed(
        &batch,
        &selection,
        &[
            fixture_candidate(&batch, "candidate-a", 1),
            fixture_candidate(&batch, "candidate-b", 2),
            fixture_candidate(&batch, "candidate-c", 3),
        ],
    );
    let (service, _) = fixture_service(
        repository,
        fixture_registry(Some(fixture_registration("source-current")), None),
        Arc::new(FixtureMaterializer::default()),
        Arc::new(FixtureCatalog::succeeds()),
        Arc::new(FixtureCategoryRepository::new(0)),
        Arc::new(FixtureClock::available()),
    );

    let page = service
        .get_preview(&batch.batch_id, Some(&selection.selection_id), 0, 3)
        .expect("selection-aware preview is readable");

    assert_eq!(page.selection, Some(selection));
    assert!(!page.candidates[0].selected);
    assert!(page.candidates[0].selection_decision.is_none());
    assert!(page.candidates[1].selected);
    assert_eq!(
        page.candidates[1].selection_decision.as_ref(),
        Some(&decision)
    );
    assert!(!page.candidates[2].selected);
    assert!(page.candidates[2].selection_decision.is_none());
}

#[test]
fn preview_rejects_missing_or_cross_batch_selection() {
    let repository = Arc::new(FixtureBatchRepository::default());
    let batch = fixture_batch(
        "batch-preview-selection-owner",
        "source-current",
        ExternalImportBatchImportStatus::Pending,
    );
    let other_batch = fixture_batch(
        "batch-preview-selection-other",
        "source-current",
        ExternalImportBatchImportStatus::Pending,
    );
    let other_selection = fixture_selection(
        &other_batch,
        ExternalImportSelectionStatus::Editing,
        &[],
        None,
    );
    repository.seed(
        &batch,
        &other_selection,
        &[fixture_candidate(&batch, "candidate-a", 1)],
    );
    let (service, _) = fixture_service(
        repository,
        fixture_registry(Some(fixture_registration("source-current")), None),
        Arc::new(FixtureMaterializer::default()),
        Arc::new(FixtureCatalog::succeeds()),
        Arc::new(FixtureCategoryRepository::new(0)),
        Arc::new(FixtureClock::available()),
    );

    assert_eq!(
        service.get_preview(
            &batch.batch_id,
            Some(&ExternalImportSelectionId::new("selection-missing")),
            0,
            50,
        ),
        Err(ExternalImportBatchError::SelectionUnavailable)
    );
    assert_eq!(
        service.get_preview(&batch.batch_id, Some(&other_selection.selection_id), 0, 50,),
        Err(ExternalImportBatchError::SelectionUnavailable)
    );
}

#[test]
fn preview_derives_expired_selection_without_writing_repository_state() {
    let repository = Arc::new(FixtureBatchRepository::default());
    let batch = fixture_batch(
        "batch-preview-expired-selection",
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
    let (service, _) = fixture_service(
        Arc::clone(&repository),
        fixture_registry(Some(fixture_registration("source-current")), None),
        Arc::new(FixtureMaterializer::default()),
        Arc::new(FixtureCatalog::succeeds()),
        Arc::new(FixtureCategoryRepository::new(0)),
        Arc::new(FixtureClock {
            value: Some(selection.expires_at_unix_millis as u128),
        }),
    );

    let page = service
        .get_preview(&batch.batch_id, Some(&selection.selection_id), 0, 50)
        .expect("expired selection is projected");

    assert_eq!(
        page.selection.expect("selection summary").status,
        ExternalImportSelectionStatus::Expired
    );
    assert_eq!(
        repository
            .get_selection(&selection.selection_id)
            .expect("repository read")
            .expect("stored selection")
            .status,
        ExternalImportSelectionStatus::Editing,
        "a read-only preview must not persist the derived expiry"
    );
}
