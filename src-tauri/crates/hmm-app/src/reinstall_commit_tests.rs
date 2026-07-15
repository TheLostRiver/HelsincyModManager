use super::*;
use crate::{ReinstallCommitPhase, ReinstallCommitService};

#[test]
fn commit_happy_path_preserves_original_backup_and_replaces_only_requested_entry_set() {
    let fixture = Fixture::ready();
    fixture.manifests.update_manifest(|manifest| {
        manifest.entries.push(manifest_entry(
            "content/other.bin",
            "mod-b",
            "other",
            None,
            b"other",
        ));
    });
    fixture.game.set_fixture("content/other.bin", b"other");
    let prepared = fixture.prepare(default_request());
    let token = prepared.plan_token.clone();
    let snapshots = Arc::new(FakeSnapshots::new(fixture.source.clone()));
    let service = ReinstallCommitService::new(
        fixture.catalog.clone(),
        fixture.source.clone(),
        fixture.game.clone(),
        fixture.backups.clone(),
        fixture.manifests.clone(),
        fixture.recovery.clone(),
        snapshots.clone(),
    );

    let result = service.commit(prepared, &token).expect("commit");

    assert_eq!(fixture.source.read_count(), 8);
    assert_eq!(snapshots.source_reads_at_first_store(), Some(8));
    assert_eq!(
        snapshots.stored_targets(),
        vec![
            "content/overwritten.bin".to_owned(),
            "content/replaced.bin".to_owned(),
            "content/stale.bin".to_owned(),
        ]
    );
    assert_eq!(
        fixture.game.mutations(),
        vec![
            "write:content/added-v2.bin".to_owned(),
            "write:content/overwritten.bin".to_owned(),
            "write:content/replaced.bin".to_owned(),
            "remove:content/stale.bin".to_owned(),
        ]
    );
    assert_eq!(
        fixture.game.bytes("content/retained.bin").as_deref(),
        Some(b"same".as_slice())
    );
    assert_eq!(
        fixture.game.bytes("content/replaced.bin").as_deref(),
        Some(b"candidate-replaced".as_slice())
    );
    assert_eq!(
        fixture.game.bytes("content/overwritten.bin").as_deref(),
        Some(b"candidate-overwritten".as_slice())
    );
    assert_eq!(
        fixture.game.bytes("content/added-v2.bin").as_deref(),
        Some(b"candidate-added".as_slice())
    );
    assert_eq!(fixture.game.bytes("content/stale.bin"), None);

    assert_eq!(fixture.manifests.save_count(), 1);
    assert_eq!(
        result.manifest.schema_version,
        hmm_core::INSTALL_MANIFEST_SCHEMA_VERSION_V2
    );
    assert_eq!(result.manifest.status, InstallManifestStatus::Completed);
    assert!(result
        .manifest
        .entries
        .iter()
        .any(|entry| entry.mod_id == ModId::new("mod-b")));
    let mod_entries = result
        .manifest
        .entries
        .iter()
        .filter(|entry| entry.mod_id == ModId::new("mod-a"))
        .collect::<Vec<_>>();
    assert_eq!(mod_entries.len(), 4);
    assert!(mod_entries
        .iter()
        .all(|entry| entry.revision_id == Some(ModRevisionId::new("v2"))));
    assert!(!mod_entries
        .iter()
        .any(|entry| entry.target_path.as_str() == "content/stale.bin"));
    assert_eq!(
        mod_entries
            .iter()
            .find(|entry| entry.target_path.as_str() == "content/overwritten.bin")
            .and_then(|entry| entry.backup_ref.as_deref()),
        Some("original-overwritten")
    );
    assert!(fixture
        .backups
        .files
        .lock()
        .expect("backup files lock")
        .contains_key("original-overwritten"));
    assert_eq!(snapshots.remaining_count(), 0);
    assert_eq!(fixture.recovery.remove_count(), 1);
    let mut statuses = fixture
        .recovery
        .history()
        .into_iter()
        .map(|transaction| transaction.status)
        .collect::<Vec<_>>();
    statuses.dedup();
    assert_eq!(
        statuses,
        vec![
            ReinstallRecoveryTransactionStatus::Planned,
            ReinstallRecoveryTransactionStatus::Committing,
            ReinstallRecoveryTransactionStatus::Completed,
        ]
    );
}

#[test]
fn commit_contract_exposes_stable_phase_codes() {
    for (phase, expected) in [
        (ReinstallCommitPhase::Revalidation, "revalidation"),
        (ReinstallCommitPhase::Snapshot, "snapshot"),
        (ReinstallCommitPhase::Recovery, "recovery"),
        (ReinstallCommitPhase::Mutation, "mutation"),
        (ReinstallCommitPhase::Manifest, "manifest"),
        (ReinstallCommitPhase::Rollback, "rollback"),
        (ReinstallCommitPhase::PostCommit, "post_commit"),
        (ReinstallCommitPhase::Cleanup, "cleanup"),
    ] {
        assert_eq!(phase.code(), expected);
    }
}

#[test]
fn commit_fakes_preserve_manifest_and_recovery_repository_keys() {
    let fixture = Fixture::ready();
    assert!(fixture
        .manifests
        .load_manifest(&ProfileId::new("other-profile"))
        .expect("wrong-profile manifest lookup")
        .is_none());

    fixture.recovery.set_active(true);
    assert!(fixture
        .recovery
        .load_transaction(&ProfileId::new("other-profile"), &ModId::new("mod-a"))
        .expect("wrong-profile recovery lookup")
        .is_none());
    assert!(fixture
        .recovery
        .load_transaction(&ProfileId::new("default"), &ModId::new("mod-b"))
        .expect("wrong-Mod recovery lookup")
        .is_none());
    let transaction = fixture
        .recovery
        .load_transaction(&ProfileId::new("default"), &ModId::new("mod-a"))
        .expect("matching recovery lookup")
        .expect("active recovery transaction");
    fixture
        .recovery
        .save_transaction(&transaction)
        .expect("store recovery fixture");
    fixture
        .recovery
        .remove_transaction(&ProfileId::new("other-profile"), &ModId::new("mod-a"))
        .expect("wrong-profile removal is a no-op");
    fixture
        .recovery
        .remove_transaction(&ProfileId::new("default"), &ModId::new("mod-b"))
        .expect("wrong-Mod removal is a no-op");
    assert!(fixture.recovery.current().is_some());
}

#[test]
fn added_unmanaged_pre_state_is_promoted_to_manifest_backup() {
    let fixture = Fixture::ready();
    fixture
        .game
        .set_fixture("content/added-v2.bin", b"unmanaged-original");
    let prepared = fixture.prepare(default_request());
    let token = prepared.plan_token.clone();
    let snapshots = Arc::new(FakeSnapshots::new(fixture.source.clone()));

    let result = commit_service(&fixture, snapshots.clone())
        .commit(prepared, &token)
        .expect("commit with promoted backup");

    let added = result
        .manifest
        .entries
        .iter()
        .find(|entry| entry.target_path.as_str() == "content/added-v2.bin")
        .expect("added manifest entry");
    assert_eq!(
        added.backup_ref.as_deref(),
        Some("snapshot:content/added-v2.bin")
    );
    assert_eq!(snapshots.remaining_count(), 1);
    let completed = fixture
        .recovery
        .history()
        .into_iter()
        .find(|transaction| transaction.status == ReinstallRecoveryTransactionStatus::Completed)
        .expect("completed transaction");
    assert!(completed.targets.iter().any(|target| {
        target.target_path.as_str() == "content/added-v2.bin"
            && matches!(
                target.snapshot,
                hmm_core::ReinstallSnapshotState::Stored {
                    cleanup_owner: hmm_core::ReinstallSnapshotCleanupOwner::Manifest,
                    ..
                }
            )
    }));
}

fn commit_service(fixture: &Fixture, snapshots: Arc<FakeSnapshots>) -> ReinstallCommitService {
    ReinstallCommitService::new(
        fixture.catalog.clone(),
        fixture.source.clone(),
        fixture.game.clone(),
        fixture.backups.clone(),
        fixture.manifests.clone(),
        fixture.recovery.clone(),
        snapshots,
    )
}

fn assert_v1_game(fixture: &Fixture) {
    assert_eq!(
        fixture.game.bytes("content/retained.bin").as_deref(),
        Some(b"same".as_slice())
    );
    assert_eq!(
        fixture.game.bytes("content/replaced.bin").as_deref(),
        Some(b"installed-replaced".as_slice())
    );
    assert_eq!(
        fixture.game.bytes("content/overwritten.bin").as_deref(),
        Some(b"installed-overwritten".as_slice())
    );
    assert_eq!(fixture.game.bytes("content/added-v2.bin"), None);
    assert_eq!(
        fixture.game.bytes("content/stale.bin").as_deref(),
        Some(b"installed-stale".as_slice())
    );
}

#[allow(dead_code)]
mod fault {
    use super::*;
    use crate::ReinstallCommitError;

    type StaleMutation = Box<dyn Fn(&Fixture)>;

    #[test]
    fn second_source_read_fails_before_snapshot_or_mutation() {
        let fixture = Fixture::ready();
        fixture.source.fail("overwritten");

        let preparation = fixture
            .service
            .prepare(default_request())
            .expect("blocked preparation");

        assert!(matches!(preparation, ReinstallPreparation::Blocked(_)));
        assert_eq!(fixture.source.read_count(), 2);
        fixture.assert_zero_mutations();
    }

    #[test]
    fn snapshot_store_failure_cleans_created_snapshots_without_mutation() {
        let fixture = Fixture::ready();
        let prepared = fixture.prepare(default_request());
        let token = prepared.plan_token.clone();
        let snapshots = Arc::new(FakeSnapshots::new(fixture.source.clone()));
        snapshots.fail_store(2);

        let error = commit_service(&fixture, snapshots.clone())
            .commit(prepared, &token)
            .expect_err("snapshot failure");

        assert_eq!(
            error,
            ReinstallCommitError::Failed {
                phase: ReinstallCommitPhase::Snapshot
            }
        );
        assert_eq!(snapshots.remaining_count(), 0);
        assert!(fixture.game.mutations().is_empty());
        assert_eq!(fixture.manifests.save_count(), 0);
    }

    #[test]
    fn planned_and_committing_save_failures_stop_before_mutation() {
        for failed_save in [1, 2] {
            let fixture = Fixture::ready();
            let prepared = fixture.prepare(default_request());
            let token = prepared.plan_token.clone();
            fixture.recovery.fail_save(failed_save);
            let snapshots = Arc::new(FakeSnapshots::new(fixture.source.clone()));

            let error = commit_service(&fixture, snapshots.clone())
                .commit(prepared, &token)
                .expect_err("recovery save failure");

            assert_eq!(
                error,
                ReinstallCommitError::Failed {
                    phase: ReinstallCommitPhase::Recovery
                }
            );
            assert!(fixture.game.mutations().is_empty());
            assert_eq!(fixture.manifests.save_count(), 0);
            assert_eq!(snapshots.remaining_count(), 0);
        }
    }

    #[test]
    fn pre_mutation_abort_removes_unowned_snapshots_when_recovery_stays_unavailable() {
        let fixture = Fixture::ready();
        let prepared = fixture.prepare(default_request());
        let token = prepared.plan_token.clone();
        fixture.recovery.fail_save(1);
        fixture.recovery.fail_save(2);
        let snapshots = Arc::new(FakeSnapshots::new(fixture.source.clone()));

        let error = commit_service(&fixture, snapshots.clone())
            .commit(prepared, &token)
            .expect_err("planned and abort saves fail");

        assert_eq!(
            error,
            ReinstallCommitError::Failed {
                phase: ReinstallCommitPhase::Recovery
            }
        );
        assert!(fixture.game.mutations().is_empty());
        assert_eq!(snapshots.remaining_count(), 0);
        assert!(fixture.recovery.current().is_none());
    }

    #[test]
    fn pre_mutation_abort_resumes_the_durable_planned_transaction() {
        let fixture = Fixture::ready();
        let prepared = fixture.prepare(default_request());
        let token = prepared.plan_token.clone();
        fixture.recovery.fail_save(2);
        fixture.recovery.fail_save(3);
        let snapshots = Arc::new(FakeSnapshots::new(fixture.source.clone()));

        let error = commit_service(&fixture, snapshots.clone())
            .commit(prepared, &token)
            .expect_err("committing and abort saves fail");

        assert_eq!(
            error,
            ReinstallCommitError::Failed {
                phase: ReinstallCommitPhase::Recovery
            }
        );
        assert!(fixture.game.mutations().is_empty());
        assert_eq!(snapshots.remaining_count(), 0);
        assert!(fixture.recovery.current().is_none());
        assert!(fixture.recovery.history().iter().any(|transaction| {
            transaction.targets.iter().any(|target| {
                matches!(
                    target.snapshot,
                    hmm_core::ReinstallSnapshotState::CleanupPending { .. }
                )
            })
        }));
    }

    #[test]
    fn pre_mutation_abort_reloads_an_ambiguously_persisted_transaction() {
        let fixture = Fixture::ready();
        let prepared = fixture.prepare(default_request());
        let token = prepared.plan_token.clone();
        fixture.recovery.fail_save(1);
        fixture.recovery.persist_then_fail_save(2);
        let snapshots = Arc::new(FakeSnapshots::new(fixture.source.clone()));

        let error = commit_service(&fixture, snapshots.clone())
            .commit(prepared, &token)
            .expect_err("abort save persists before returning an error");

        assert_eq!(
            error,
            ReinstallCommitError::Failed {
                phase: ReinstallCommitPhase::Recovery
            }
        );
        assert!(fixture.game.mutations().is_empty());
        assert_eq!(snapshots.remaining_count(), 0);
        assert!(fixture.recovery.current().is_none());
        assert!(fixture.recovery.history().iter().any(|transaction| {
            transaction.targets.iter().any(|target| {
                matches!(
                    target.snapshot,
                    hmm_core::ReinstallSnapshotState::CleanupPending { .. }
                )
            })
        }));
    }

    #[test]
    fn pre_mutation_abort_keeps_transaction_until_snapshot_cleanup_finishes() {
        let fixture = Fixture::ready();
        let prepared = fixture.prepare(default_request());
        let token = prepared.plan_token.clone();
        fixture.recovery.fail_save(2);
        let snapshots = Arc::new(FakeSnapshots::new(fixture.source.clone()));
        snapshots.fail_remove(1);

        let error = commit_service(&fixture, snapshots.clone())
            .commit(prepared, &token)
            .expect_err("committing save failure");

        assert_eq!(
            error,
            ReinstallCommitError::Failed {
                phase: ReinstallCommitPhase::Recovery
            }
        );
        assert!(fixture.game.mutations().is_empty());
        let transaction = fixture
            .recovery
            .current()
            .expect("cleanup-owned transaction");
        assert_eq!(
            transaction.status,
            ReinstallRecoveryTransactionStatus::Committing
        );
        assert!(transaction.targets.iter().any(|target| matches!(
            target.snapshot,
            hmm_core::ReinstallSnapshotState::CleanupPending { .. }
        )));
        assert_eq!(snapshots.remaining_count(), 3);
    }

    #[test]
    fn every_mutation_failure_rolls_back_to_v1() {
        for failed_mutation in 1..=4 {
            let fixture = Fixture::ready();
            let old_manifest = fixture
                .manifests
                .manifest
                .lock()
                .expect("manifest lock")
                .clone();
            let prepared = fixture.prepare(default_request());
            let token = prepared.plan_token.clone();
            fixture.game.fail_mutation(failed_mutation);
            let snapshots = Arc::new(FakeSnapshots::new(fixture.source.clone()));

            let error = commit_service(&fixture, snapshots.clone())
                .commit(prepared, &token)
                .expect_err("mutation failure");

            assert_eq!(
                error,
                ReinstallCommitError::RolledBack {
                    failed_phase: ReinstallCommitPhase::Mutation,
                    cleanup_pending: false,
                }
            );
            assert_v1_game(&fixture);
            assert_eq!(
                *fixture.manifests.manifest.lock().expect("manifest lock"),
                old_manifest
            );
            assert_eq!(fixture.manifests.save_count(), 0);
            assert_eq!(snapshots.remaining_count(), 0);
        }
    }

    #[test]
    fn manifest_error_old_or_candidate_visible_restores_v1() {
        for candidate_visible in [false, true] {
            let fixture = Fixture::ready();
            let old_manifest = fixture
                .manifests
                .manifest
                .lock()
                .expect("manifest lock")
                .clone();
            let prepared = fixture.prepare(default_request());
            let token = prepared.plan_token.clone();
            fixture.manifests.fail_save(1, candidate_visible);
            let snapshots = Arc::new(FakeSnapshots::new(fixture.source.clone()));

            let error = commit_service(&fixture, snapshots.clone())
                .commit(prepared, &token)
                .expect_err("manifest failure");

            assert_eq!(
                error,
                ReinstallCommitError::RolledBack {
                    failed_phase: ReinstallCommitPhase::Manifest,
                    cleanup_pending: false,
                }
            );
            assert_v1_game(&fixture);
            assert_eq!(
                *fixture.manifests.manifest.lock().expect("manifest lock"),
                old_manifest
            );
            assert_eq!(
                fixture.manifests.save_count(),
                if candidate_visible { 2 } else { 1 }
            );
            assert_eq!(snapshots.remaining_count(), 0);
        }
    }

    #[test]
    fn ambiguous_manifest_or_old_manifest_restore_failure_requires_repair() {
        let unknown = Fixture::ready();
        let prepared = unknown.prepare(default_request());
        let token = prepared.plan_token.clone();
        unknown.manifests.fail_save(1, false);
        unknown.manifests.fail_load(3);
        let snapshots = Arc::new(FakeSnapshots::new(unknown.source.clone()));
        assert_eq!(
            commit_service(&unknown, snapshots).commit(prepared, &token),
            Err(ReinstallCommitError::RepairRequired {
                failed_phase: ReinstallCommitPhase::Manifest
            })
        );
        assert_eq!(
            unknown
                .recovery
                .current()
                .expect("repair transaction")
                .status,
            ReinstallRecoveryTransactionStatus::RepairRequired
        );

        let restore_failed = Fixture::ready();
        let prepared = restore_failed.prepare(default_request());
        let token = prepared.plan_token.clone();
        restore_failed.manifests.fail_save(1, true);
        restore_failed.manifests.fail_save(2, false);
        let snapshots = Arc::new(FakeSnapshots::new(restore_failed.source.clone()));
        assert_eq!(
            commit_service(&restore_failed, snapshots).commit(prepared, &token),
            Err(ReinstallCommitError::RepairRequired {
                failed_phase: ReinstallCommitPhase::Manifest
            })
        );
        assert_eq!(
            restore_failed
                .recovery
                .current()
                .expect("repair transaction")
                .status,
            ReinstallRecoveryTransactionStatus::RepairRequired
        );
    }

    #[test]
    fn partial_rollback_keeps_only_unrestored_target_and_snapshot() {
        let fixture = Fixture::ready();
        let prepared = fixture.prepare(default_request());
        let token = prepared.plan_token.clone();
        fixture.game.fail_mutation(2);
        fixture.game.fail_mutation_before(3);
        let snapshots = Arc::new(FakeSnapshots::new(fixture.source.clone()));

        let error = commit_service(&fixture, snapshots.clone())
            .commit(prepared, &token)
            .expect_err("partial rollback");

        assert_eq!(
            error,
            ReinstallCommitError::RollbackRequired {
                failed_phase: ReinstallCommitPhase::Mutation
            }
        );
        let transaction = fixture.recovery.current().expect("rollback transaction");
        assert_eq!(
            transaction.status,
            ReinstallRecoveryTransactionStatus::RollbackRequired
        );
        assert_eq!(transaction.targets.len(), 1);
        assert_eq!(
            transaction.targets[0].target_path.as_str(),
            "content/overwritten.bin"
        );
        assert_eq!(snapshots.remaining_count(), 1);
    }

    #[test]
    fn first_middle_and_last_restore_failures_keep_only_the_unrestored_target() {
        for (restore_attempt, expected_target, expected_snapshots) in [
            (5, "content/stale.bin", 1),
            (6, "content/replaced.bin", 1),
            (8, "content/added-v2.bin", 0),
        ] {
            let fixture = Fixture::ready();
            let prepared = fixture.prepare(default_request());
            let token = prepared.plan_token.clone();
            fixture.game.fail_mutation(4);
            fixture.game.fail_mutation_before(restore_attempt);
            let snapshots = Arc::new(FakeSnapshots::new(fixture.source.clone()));

            let error = commit_service(&fixture, snapshots.clone())
                .commit(prepared, &token)
                .expect_err("restore failure");

            assert_eq!(
                error,
                ReinstallCommitError::RollbackRequired {
                    failed_phase: ReinstallCommitPhase::Mutation
                }
            );
            let transaction = fixture.recovery.current().expect("rollback transaction");
            assert_eq!(
                transaction.status,
                ReinstallRecoveryTransactionStatus::RollbackRequired
            );
            assert_eq!(transaction.targets.len(), 1);
            assert_eq!(transaction.targets[0].target_path.as_str(), expected_target);
            assert_eq!(snapshots.remaining_count(), expected_snapshots);
        }
    }

    #[test]
    fn unreadable_or_missing_snapshot_keeps_unrestored_target_and_snapshot_owned() {
        for missing in [false, true] {
            let fixture = Fixture::ready();
            let prepared = fixture.prepare(default_request());
            let token = prepared.plan_token.clone();
            fixture.game.fail_mutation(2);
            let snapshots = Arc::new(FakeSnapshots::new(fixture.source.clone()));
            let snapshot_ref = "snapshot:content/overwritten.bin";
            if missing {
                snapshots.miss_read(snapshot_ref);
            } else {
                snapshots.fail_read(snapshot_ref);
            }

            let error = commit_service(&fixture, snapshots.clone())
                .commit(prepared, &token)
                .expect_err("snapshot read failure");

            assert_eq!(
                error,
                ReinstallCommitError::RollbackRequired {
                    failed_phase: ReinstallCommitPhase::Mutation
                }
            );
            let transaction = fixture.recovery.current().expect("rollback transaction");
            assert_eq!(transaction.targets.len(), 1);
            assert_eq!(
                transaction.targets[0].target_path.as_str(),
                "content/overwritten.bin"
            );
            assert!(matches!(
                &transaction.targets[0].snapshot,
                hmm_core::ReinstallSnapshotState::Stored {
                    snapshot_ref: stored_ref,
                    cleanup_owner: hmm_core::ReinstallSnapshotCleanupOwner::Transaction,
                    ..
                } if stored_ref == snapshot_ref
            ));
            assert_eq!(snapshots.remaining_count(), 1);
        }
    }

    #[test]
    fn rollback_recovery_update_failure_keeps_snapshots_and_marks_repair() {
        let fixture = Fixture::ready();
        let prepared = fixture.prepare(default_request());
        let token = prepared.plan_token.clone();
        fixture.game.fail_mutation(2);
        fixture.recovery.fail_save(4);
        let snapshots = Arc::new(FakeSnapshots::new(fixture.source.clone()));

        let error = commit_service(&fixture, snapshots.clone())
            .commit(prepared, &token)
            .expect_err("rollback recovery update failure");

        assert_eq!(
            error,
            ReinstallCommitError::RepairRequired {
                failed_phase: ReinstallCommitPhase::Mutation
            }
        );
        assert_v1_game(&fixture);
        assert_eq!(
            fixture
                .recovery
                .current()
                .expect("repair transaction")
                .status,
            ReinstallRecoveryTransactionStatus::RepairRequired
        );
        assert_eq!(snapshots.remaining_count(), 3);
    }

    #[test]
    fn rollback_transaction_remove_failure_keeps_durable_rolled_back_record() {
        let fixture = Fixture::ready();
        let prepared = fixture.prepare(default_request());
        let token = prepared.plan_token.clone();
        fixture.game.fail_mutation(2);
        fixture.recovery.fail_remove(1);
        let snapshots = Arc::new(FakeSnapshots::new(fixture.source.clone()));

        let error = commit_service(&fixture, snapshots.clone())
            .commit(prepared, &token)
            .expect_err("mutation failure");

        assert_eq!(
            error,
            ReinstallCommitError::RolledBack {
                failed_phase: ReinstallCommitPhase::Mutation,
                cleanup_pending: true,
            }
        );
        assert_v1_game(&fixture);
        let transaction = fixture.recovery.current().expect("rolled back transaction");
        assert_eq!(
            transaction.status,
            ReinstallRecoveryTransactionStatus::RolledBack
        );
        assert!(transaction.targets.is_empty());
        assert_eq!(snapshots.remaining_count(), 0);
    }

    #[test]
    fn completed_bookkeeping_failure_keeps_v2_and_committing_transaction() {
        let fixture = Fixture::ready();
        let prepared = fixture.prepare(default_request());
        let token = prepared.plan_token.clone();
        fixture.recovery.fail_save(3);
        let snapshots = Arc::new(FakeSnapshots::new(fixture.source.clone()));

        let error = commit_service(&fixture, snapshots.clone())
            .commit(prepared, &token)
            .expect_err("post commit failure");

        assert_eq!(error, ReinstallCommitError::PostCommit);
        assert_eq!(error.code(), "install_reinstall_failed:post_commit");
        assert_eq!(fixture.manifests.save_count(), 1);
        assert_eq!(
            fixture
                .recovery
                .current()
                .expect("committing transaction")
                .status,
            ReinstallRecoveryTransactionStatus::Committing
        );
        assert_eq!(snapshots.remaining_count(), 3);
        assert_eq!(
            fixture.game.bytes("content/added-v2.bin").as_deref(),
            Some(b"candidate-added".as_slice())
        );
    }

    #[test]
    fn cleanup_failure_keeps_v2_and_completed_transaction() {
        let fixture = Fixture::ready();
        let prepared = fixture.prepare(default_request());
        let token = prepared.plan_token.clone();
        let snapshots = Arc::new(FakeSnapshots::new(fixture.source.clone()));
        snapshots.fail_remove(2);
        let service = commit_service(&fixture, snapshots.clone());

        let error = service
            .commit(prepared, &token)
            .expect_err("cleanup failure");

        assert_eq!(error, ReinstallCommitError::CleanupPending);
        assert_eq!(fixture.manifests.save_count(), 1);
        assert_eq!(
            fixture
                .recovery
                .current()
                .expect("completed transaction")
                .status,
            ReinstallRecoveryTransactionStatus::Completed
        );
        let transaction = fixture.recovery.current().expect("completed transaction");
        assert_eq!(
            transaction
                .targets
                .iter()
                .filter(|target| matches!(
                    target.snapshot,
                    hmm_core::ReinstallSnapshotState::Cleaned { .. }
                ))
                .count(),
            1
        );
        assert_eq!(snapshots.remaining_count(), 2);

        service
            .cleanup_committed(&transaction)
            .expect("resume cleanup");
        assert_eq!(snapshots.remove_count(), 4);
        assert_eq!(snapshots.remaining_count(), 0);
        assert!(fixture.recovery.current().is_none());
    }

    #[test]
    fn completed_transaction_remove_failure_keeps_checkpointed_cleanup_record() {
        let fixture = Fixture::ready();
        let prepared = fixture.prepare(default_request());
        let token = prepared.plan_token.clone();
        fixture.recovery.fail_remove(1);
        let snapshots = Arc::new(FakeSnapshots::new(fixture.source.clone()));

        let error = commit_service(&fixture, snapshots.clone())
            .commit(prepared, &token)
            .expect_err("transaction removal failure");

        assert_eq!(error, ReinstallCommitError::CleanupPending);
        let transaction = fixture
            .recovery
            .current()
            .expect("completed cleanup record");
        assert_eq!(
            transaction.status,
            ReinstallRecoveryTransactionStatus::Completed
        );
        assert!(transaction.targets.iter().all(|target| !matches!(
            target.snapshot,
            hmm_core::ReinstallSnapshotState::Stored {
                cleanup_owner: hmm_core::ReinstallSnapshotCleanupOwner::Transaction,
                ..
            } | hmm_core::ReinstallSnapshotState::CleanupPending { .. }
        )));
        assert_eq!(snapshots.remaining_count(), 0);
    }

    #[test]
    fn stale_original_backup_is_restored_and_cleanup_failure_keeps_completed_state() {
        let fixture = Fixture::ready();
        fixture.manifests.update_manifest(|manifest| {
            manifest
                .entries
                .iter_mut()
                .find(|entry| entry.target_path.as_str() == "content/stale.bin")
                .expect("stale entry")
                .backup_ref = Some("original-stale".to_owned());
        });
        fixture
            .backups
            .set_fixture("original-stale", b"baseline-stale");
        let prepared = fixture.prepare(default_request());
        let token = prepared.plan_token.clone();
        fixture.backups.fail_removes();
        let snapshots = Arc::new(FakeSnapshots::new(fixture.source.clone()));

        let error = commit_service(&fixture, snapshots)
            .commit(prepared, &token)
            .expect_err("stale backup cleanup failure");

        assert_eq!(error, ReinstallCommitError::CleanupPending);
        assert_eq!(
            fixture.game.bytes("content/stale.bin").as_deref(),
            Some(b"baseline-stale".as_slice())
        );
        assert_eq!(
            fixture
                .recovery
                .current()
                .expect("completed transaction")
                .status,
            ReinstallRecoveryTransactionStatus::Completed
        );
        assert!(fixture
            .backups
            .files
            .lock()
            .expect("backup files lock")
            .contains_key("original-stale"));
        assert!(fixture
            .recovery
            .current()
            .expect("completed transaction")
            .targets
            .iter()
            .all(|target| !matches!(
                target.snapshot,
                hmm_core::ReinstallSnapshotState::Stored {
                    cleanup_owner: hmm_core::ReinstallSnapshotCleanupOwner::Transaction,
                    ..
                } | hmm_core::ReinstallSnapshotState::CleanupPending { .. }
            )));
    }

    #[test]
    fn stale_backup_cleanup_retries_after_checkpoint_save_failure() {
        let fixture = Fixture::ready();
        fixture.manifests.update_manifest(|manifest| {
            manifest
                .entries
                .iter_mut()
                .find(|entry| entry.target_path.as_str() == "content/stale.bin")
                .expect("stale entry")
                .backup_ref = Some("original-stale".to_owned());
        });
        fixture
            .backups
            .set_fixture("original-stale", b"baseline-stale");
        let prepared = fixture.prepare(default_request());
        let token = prepared.plan_token.clone();
        fixture.backups.fail_removes();
        let snapshots = Arc::new(FakeSnapshots::new(fixture.source.clone()));
        let service = commit_service(&fixture, snapshots);

        service
            .commit(prepared, &token)
            .expect_err("first stale backup cleanup fails");
        fixture.backups.allow_removes();
        fixture
            .recovery
            .fail_save(fixture.recovery.save_count() + 1);
        let transaction = fixture
            .recovery
            .current()
            .expect("completed cleanup transaction");

        service
            .cleanup_committed(&transaction)
            .expect_err("backup ref checkpoint fails after deletion");

        assert!(!fixture
            .backups
            .files
            .lock()
            .expect("backup files lock")
            .contains_key("original-stale"));
        let durable = fixture
            .recovery
            .current()
            .expect("retryable completed transaction");
        assert!(durable.targets.iter().any(|target| {
            target.target_path.as_str() == "content/stale.bin"
                && target.original_backup_ref.as_deref() == Some("original-stale")
        }));

        service
            .cleanup_committed(&durable)
            .expect("missing backup deletion is idempotent");
        assert!(fixture.recovery.current().is_none());
    }

    #[test]
    fn lock_time_revalidation_fails_closed_for_every_prepared_fact() {
        let cases: Vec<StaleMutation> = vec![
            Box::new(|fixture| {
                fixture
                    .manifests
                    .update_manifest(|manifest| manifest.plan_hash = Some("changed".to_owned()))
            }),
            Box::new(|fixture| fixture.source.set("added-v2", b"changed-source")),
            Box::new(|fixture| {
                fixture
                    .game
                    .set_fixture("content/retained.bin", b"changed-target")
            }),
            Box::new(|fixture| {
                fixture
                    .backups
                    .set_fixture("original-overwritten", b"changed-backup")
            }),
            Box::new(|fixture| {
                fixture
                    .catalog
                    .set_revision(candidate_revision("v2", "mod-b"))
            }),
            Box::new(|fixture| fixture.recovery.set_active(true)),
            Box::new(|fixture| fixture.recovery.set_active_mod("mod-b")),
        ];
        for mutate in cases {
            let fixture = Fixture::ready();
            let prepared = fixture.prepare(default_request());
            let token = prepared.plan_token.clone();
            mutate(&fixture);
            let snapshots = Arc::new(FakeSnapshots::new(fixture.source.clone()));

            let error = commit_service(&fixture, snapshots.clone())
                .commit(prepared, &token)
                .expect_err("stale prepared facts");

            assert_eq!(error, ReinstallCommitError::PreviewStale);
            assert!(fixture.game.mutations().is_empty());
            assert_eq!(snapshots.remaining_count(), 0);
            assert_eq!(fixture.manifests.save_count(), 0);
        }

        let fixture = Fixture::ready();
        let prepared = fixture.prepare(default_request());
        let snapshots = Arc::new(FakeSnapshots::new(fixture.source.clone()));
        assert_eq!(
            commit_service(&fixture, snapshots).commit(prepared, "wrong-token"),
            Err(ReinstallCommitError::PreviewStale)
        );
    }
}

struct FakeSnapshots {
    source: Arc<FakeCandidateSource>,
    files: Mutex<BTreeMap<String, Vec<u8>>>,
    stored_targets: Mutex<Vec<String>>,
    reads_at_first_store: Mutex<Option<usize>>,
    store_attempts: Mutex<usize>,
    remove_attempts: Mutex<usize>,
    fail_stores: Mutex<BTreeSet<usize>>,
    fail_removes: Mutex<BTreeSet<usize>>,
    fail_reads: Mutex<BTreeSet<String>>,
    missing_reads: Mutex<BTreeSet<String>>,
}

impl FakeSnapshots {
    fn new(source: Arc<FakeCandidateSource>) -> Self {
        Self {
            source,
            files: Mutex::new(BTreeMap::new()),
            stored_targets: Mutex::new(Vec::new()),
            reads_at_first_store: Mutex::new(None),
            store_attempts: Mutex::new(0),
            remove_attempts: Mutex::new(0),
            fail_stores: Mutex::new(BTreeSet::new()),
            fail_removes: Mutex::new(BTreeSet::new()),
            fail_reads: Mutex::new(BTreeSet::new()),
            missing_reads: Mutex::new(BTreeSet::new()),
        }
    }

    fn source_reads_at_first_store(&self) -> Option<usize> {
        *self
            .reads_at_first_store
            .lock()
            .expect("snapshot read count lock")
    }

    fn stored_targets(&self) -> Vec<String> {
        self.stored_targets
            .lock()
            .expect("stored targets lock")
            .clone()
    }

    fn remaining_count(&self) -> usize {
        self.files.lock().expect("snapshot files lock").len()
    }

    fn remove_count(&self) -> usize {
        *self
            .remove_attempts
            .lock()
            .expect("snapshot remove attempts lock")
    }

    fn fail_store(&self, attempt: usize) {
        self.fail_stores
            .lock()
            .expect("snapshot store failures lock")
            .insert(attempt);
    }

    fn fail_remove(&self, attempt: usize) {
        self.fail_removes
            .lock()
            .expect("snapshot remove failures lock")
            .insert(attempt);
    }

    fn fail_read(&self, snapshot_ref: &str) {
        self.fail_reads
            .lock()
            .expect("snapshot read failures lock")
            .insert(snapshot_ref.to_owned());
    }

    fn miss_read(&self, snapshot_ref: &str) {
        self.missing_reads
            .lock()
            .expect("snapshot missing reads lock")
            .insert(snapshot_ref.to_owned());
    }
}

impl ReinstallSnapshotStore for FakeSnapshots {
    fn store_snapshot(&self, target_path: &InstallTargetPath, bytes: &[u8]) -> Result<String> {
        let mut attempts = self
            .store_attempts
            .lock()
            .expect("snapshot store attempts lock");
        *attempts += 1;
        if self
            .fail_stores
            .lock()
            .expect("snapshot store failures lock")
            .remove(&*attempts)
        {
            anyhow::bail!("injected snapshot store failure");
        }
        let mut first = self
            .reads_at_first_store
            .lock()
            .expect("snapshot read count lock");
        first.get_or_insert_with(|| self.source.read_count());
        let reference = format!("snapshot:{}", target_path.as_str());
        self.files
            .lock()
            .expect("snapshot files lock")
            .insert(reference.clone(), bytes.to_vec());
        self.stored_targets
            .lock()
            .expect("stored targets lock")
            .push(target_path.as_str().to_owned());
        Ok(reference)
    }

    fn read_snapshot(&self, snapshot_ref: &str) -> Result<Option<Vec<u8>>> {
        if self
            .fail_reads
            .lock()
            .expect("snapshot read failures lock")
            .remove(snapshot_ref)
        {
            anyhow::bail!("injected snapshot read failure");
        }
        if self
            .missing_reads
            .lock()
            .expect("snapshot missing reads lock")
            .remove(snapshot_ref)
        {
            return Ok(None);
        }
        Ok(self
            .files
            .lock()
            .expect("snapshot files lock")
            .get(snapshot_ref)
            .cloned())
    }

    fn remove_snapshot(&self, snapshot_ref: &str) -> Result<()> {
        let mut attempts = self
            .remove_attempts
            .lock()
            .expect("snapshot remove attempts lock");
        *attempts += 1;
        if self
            .fail_removes
            .lock()
            .expect("snapshot remove failures lock")
            .remove(&*attempts)
        {
            anyhow::bail!("injected snapshot remove failure");
        }
        self.files
            .lock()
            .expect("snapshot files lock")
            .remove(snapshot_ref);
        Ok(())
    }
}
