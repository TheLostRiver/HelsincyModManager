use super::external_import::*;

fn candidate(
    batch_id: &str,
    candidate_id: impl Into<String>,
    status: ExternalImportCandidateStatus,
) -> ExternalImportCandidate {
    ExternalImportCandidate {
        batch_id: ExternalImportBatchId::new(batch_id),
        candidate_id: ExternalImportCandidateId::new(candidate_id),
        source_item_key_hash: "item-key-hash".to_owned(),
        content_fingerprint: "content-fingerprint".to_owned(),
        metadata_hint: ExternalImportMetadataHint::default(),
        resource_usage: ExternalImportResourceUsage {
            file_count: 1,
            source_bytes: 10,
            materialization_bytes: 10,
        },
        preview_status: status,
        conflict_kind: ExternalImportConflictKind::None,
    }
}

fn selection() -> ExternalImportSelection {
    ExternalImportSelection::new(
        ExternalImportSelectionId::new("selection-a"),
        ExternalImportBatchId::new("batch-a"),
        1_000,
    )
}

fn select(candidate_id: impl Into<String>) -> ExternalImportSelectionMutation {
    ExternalImportSelectionMutation {
        candidate_id: ExternalImportCandidateId::new(candidate_id),
        selected: true,
        decision: None,
    }
}

#[test]
fn selection_mutation_accepts_one_199_and_200_items_but_rejects_zero_and_201() {
    let candidates = (0..201)
        .map(|index| {
            candidate(
                "batch-a",
                format!("candidate-{index}"),
                ExternalImportCandidateStatus::Ready,
            )
        })
        .collect::<Vec<_>>();
    let budget = ExternalImportResourceBudget::default();

    let mut empty = selection();
    assert_eq!(
        empty
            .apply_mutation(0, &[], &candidates, &budget, 1)
            .expect_err("empty mutation is rejected"),
        ExternalImportSelectionError::MutationEmpty
    );

    for count in [1, 199, 200] {
        let mut current = selection();
        let mutations = (0..count)
            .map(|index| select(format!("candidate-{index}")))
            .collect::<Vec<_>>();
        let result = current
            .apply_mutation(0, &mutations, &candidates, &budget, 1)
            .expect("bounded mutation succeeds");
        assert_eq!(result.selected_count, count);
        assert_eq!(result.revision, 1);
    }

    let mut too_many = selection();
    let mutations = (0..201)
        .map(|index| select(format!("candidate-{index}")))
        .collect::<Vec<_>>();
    assert_eq!(
        too_many
            .apply_mutation(0, &mutations, &candidates, &budget, 1)
            .expect_err("201-item mutation is rejected"),
        ExternalImportSelectionError::MutationLimitExceeded
    );
    assert_eq!(too_many.selected_count(), 0);
}

#[test]
fn server_side_select_all_adds_only_ready_candidates_and_keeps_explicit_decisions() {
    let candidates = vec![
        candidate("batch-a", "ready", ExternalImportCandidateStatus::Ready),
        candidate(
            "batch-a",
            "blocked",
            ExternalImportCandidateStatus::StructureInvalid,
        ),
        candidate(
            "batch-a",
            "metadata",
            ExternalImportCandidateStatus::MetadataInvalid,
        ),
    ];
    let budget = ExternalImportResourceBudget::default();
    let mut current = selection();
    current
        .apply_mutation(
            0,
            &[ExternalImportSelectionMutation {
                candidate_id: ExternalImportCandidateId::new("metadata"),
                selected: true,
                decision: Some(ExternalImportSelectionDecision {
                    conflict_resolution: Some(
                        ExternalImportConflictResolution::IgnoreInvalidMetadata,
                    ),
                    category_id: Some("category-a".to_owned()),
                }),
            }],
            &candidates,
            &budget,
            1,
        )
        .expect("explicit metadata decision is valid");

    let result = current
        .select_all_ready(current.revision, &candidates, &budget, 2)
        .expect("server-side select all succeeds");

    assert_eq!(result.selected_count, 2);
    assert_eq!(
        current
            .entries
            .iter()
            .map(|entry| entry.candidate_id.as_str())
            .collect::<Vec<_>>(),
        ["metadata", "ready"]
    );
}

#[test]
fn selection_enforces_9999_10000_and_10001_total_limits_atomically() {
    let candidates = (0..10_001)
        .map(|index| {
            candidate(
                "batch-a",
                format!("candidate-{index}"),
                ExternalImportCandidateStatus::Ready,
            )
        })
        .collect::<Vec<_>>();
    let budget = ExternalImportResourceBudget::default();
    let mut selection = selection();

    for chunk in (0..9_999).collect::<Vec<_>>().chunks(200) {
        let mutations = chunk
            .iter()
            .map(|index| select(format!("candidate-{index}")))
            .collect::<Vec<_>>();
        let revision = selection.revision;
        selection
            .apply_mutation(revision, &mutations, &candidates, &budget, 1)
            .expect("selection below total limit succeeds");
    }
    assert_eq!(selection.selected_count(), 9_999);

    let revision = selection.revision;
    selection
        .apply_mutation(
            revision,
            &[select("candidate-9999")],
            &candidates,
            &budget,
            1,
        )
        .expect("10,000th selection succeeds");
    assert_eq!(selection.selected_count(), 10_000);

    let revision_before_rejection = selection.revision;
    let usage_before_rejection = selection.selected_resource_usage;
    assert_eq!(
        selection
            .apply_mutation(
                revision_before_rejection,
                &[select("candidate-10000")],
                &candidates,
                &budget,
                1,
            )
            .expect_err("10,001st selection is rejected"),
        ExternalImportSelectionError::TotalLimitExceeded
    );
    assert_eq!(selection.selected_count(), 10_000);
    assert_eq!(selection.revision, revision_before_rejection);
    assert_eq!(selection.selected_resource_usage, usage_before_rejection);
}

#[test]
fn selection_rejects_duplicate_unknown_cross_batch_and_blocked_candidates() {
    let candidates = vec![
        candidate("batch-a", "ready", ExternalImportCandidateStatus::Ready),
        candidate(
            "batch-b",
            "other-batch",
            ExternalImportCandidateStatus::Ready,
        ),
        candidate(
            "batch-a",
            "blocked",
            ExternalImportCandidateStatus::StructureInvalid,
        ),
    ];
    let budget = ExternalImportResourceBudget::default();
    let cases = [
        vec![select("ready"), select("ready")],
        vec![select("unknown")],
        vec![select("other-batch")],
        vec![select("blocked")],
    ];

    for mutations in cases {
        let mut current = selection();
        assert_eq!(
            current
                .apply_mutation(0, &mutations, &candidates, &budget, 1)
                .expect_err("invalid candidate mutation is rejected"),
            ExternalImportSelectionError::CandidateInvalid
        );
        assert_eq!(current.selected_count(), 0);
    }

    let mut current = selection();
    current
        .apply_mutation(0, &[select("ready")], &candidates, &budget, 1)
        .expect("ready candidate is selected");
    let revision = current.revision;
    current
        .apply_mutation(
            revision,
            &[ExternalImportSelectionMutation {
                candidate_id: ExternalImportCandidateId::new("ready"),
                selected: false,
                decision: None,
            }],
            &candidates,
            &budget,
            1,
        )
        .expect("deselection removes the same-batch entry");
    assert_eq!(current.selected_count(), 0);

    let mut malformed = selection();
    malformed.entries.push(ExternalImportSelectionEntry {
        candidate_id: ExternalImportCandidateId::new("other-batch"),
        decision: None,
        updated_at_unix_millis: 1,
    });
    assert_eq!(
        malformed
            .seal(0, &candidates, &budget, 1)
            .expect_err("sealing revalidates cross-batch persisted entries"),
        ExternalImportSelectionError::CandidateInvalid
    );
}

#[test]
fn selection_requires_matching_decisions_and_honors_cas_expiry_and_sealing() {
    let candidates = vec![
        candidate("batch-a", "ready", ExternalImportCandidateStatus::Ready),
        candidate(
            "batch-a",
            "collision",
            ExternalImportCandidateStatus::NameCollision,
        ),
        candidate(
            "batch-a",
            "metadata",
            ExternalImportCandidateStatus::MetadataInvalid,
        ),
    ];
    let budget = ExternalImportResourceBudget::default();
    let mut current = selection();

    assert_eq!(
        current
            .apply_mutation(0, &[select("collision")], &candidates, &budget, 1)
            .expect_err("collision needs an explicit decision"),
        ExternalImportSelectionError::CandidateInvalid
    );
    current
        .apply_mutation(
            0,
            &[ExternalImportSelectionMutation {
                candidate_id: ExternalImportCandidateId::new("collision"),
                selected: true,
                decision: Some(ExternalImportSelectionDecision {
                    conflict_resolution: Some(ExternalImportConflictResolution::KeepBoth),
                    category_id: Some("category-a".to_owned()),
                }),
            }],
            &candidates,
            &budget,
            1,
        )
        .expect("collision decision succeeds");
    assert_eq!(
        current
            .apply_mutation(0, &[select("ready")], &candidates, &budget, 1)
            .expect_err("stale revision is rejected"),
        ExternalImportSelectionError::RevisionConflict
    );

    let revision = current.revision;
    current
        .seal(revision, &candidates, &budget, 1)
        .expect("selection is sealed");
    assert_eq!(
        current
            .apply_mutation(
                current.revision,
                &[select("ready")],
                &candidates,
                &budget,
                1
            )
            .expect_err("sealed selection is closed"),
        ExternalImportSelectionError::Closed
    );

    let mut expired = selection();
    assert_eq!(
        expired
            .apply_mutation(0, &[select("ready")], &candidates, &budget, 1_000)
            .expect_err("selection expires at its deadline"),
        ExternalImportSelectionError::Expired
    );
}

#[test]
fn selection_rejects_resource_budget_overruns_without_mutating_snapshot() {
    let mut heavy = candidate("batch-a", "heavy", ExternalImportCandidateStatus::Ready);
    heavy.resource_usage = ExternalImportResourceUsage {
        file_count: 11,
        source_bytes: 101,
        materialization_bytes: 101,
    };
    let budget = ExternalImportResourceBudget {
        max_total_candidates: EXTERNAL_IMPORT_SELECTION_MAX_ITEMS as u64,
        max_total_files: 10,
        max_total_source_bytes: 100,
        max_total_materialization_bytes: 100,
        materialization: ExternalImportMaterializationBudget::default(),
    };
    let mut current = selection();
    assert_eq!(
        current
            .apply_mutation(0, &[select("heavy")], &[heavy], &budget, 1)
            .expect_err("resource overrun is rejected"),
        ExternalImportSelectionError::ResourceLimitExceeded
    );
    assert_eq!(current.selected_count(), 0);
    assert_eq!(current.revision, 0);
}

#[test]
fn provenance_serialization_contains_only_opaque_import_facts() {
    let provenance = ExternalImportProvenance {
        adapter_id: ExternalImportAdapterId::new("hunting_box_directory_v1"),
        batch_id: ExternalImportBatchId::new("batch-a"),
        source_item_key_hash: "item-key-hash".to_owned(),
        content_fingerprint: "content-fingerprint".to_owned(),
        imported_at_unix_millis: 42,
    };

    provenance.validate().expect("opaque provenance is valid");
    assert_eq!(
        serde_json::to_value(&provenance).expect("serialize provenance"),
        serde_json::json!({
            "adapter_id": "hunting_box_directory_v1",
            "batch_id": "batch-a",
            "source_item_key_hash": "item-key-hash",
            "content_fingerprint": "content-fingerprint",
            "imported_at_unix_millis": 42
        })
    );
}

#[test]
fn provenance_accepts_the_scanner_sha256_fingerprint_shape() {
    let provenance = ExternalImportProvenance {
        adapter_id: ExternalImportAdapterId::new("hunting_box_directory_v1"),
        batch_id: ExternalImportBatchId::new("batch-a"),
        source_item_key_hash: "item-key-hash".to_owned(),
        content_fingerprint: format!("sha256:{}", "a".repeat(64)),
        imported_at_unix_millis: 42,
    };

    provenance
        .validate()
        .expect("scanner content fingerprints are valid provenance facts");
}

#[test]
fn provenance_rejects_path_like_values_in_opaque_fields() {
    for path_like_value in [
        "C:\\synthetic\\source",
        "/synthetic/source",
        "file:///synthetic/source",
    ] {
        let provenance = ExternalImportProvenance {
            adapter_id: ExternalImportAdapterId::new("hunting_box_directory_v1"),
            batch_id: ExternalImportBatchId::new("batch-a"),
            source_item_key_hash: path_like_value.to_owned(),
            content_fingerprint: "content-fingerprint".to_owned(),
            imported_at_unix_millis: 42,
        };

        assert_eq!(
            provenance.validate(),
            Err(ExternalImportProvenanceError::InvalidOpaqueValue)
        );
    }
}
