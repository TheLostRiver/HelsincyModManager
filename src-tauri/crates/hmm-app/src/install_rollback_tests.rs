use super::*;

#[test]
fn commit_plan_prunes_restored_entries_when_another_rollback_entry_fails() {
    let first_target =
        InstallTargetPath::parse("nativePC/models/first.mod3", ["nativePC"]).expect("valid");
    let second_target =
        InstallTargetPath::parse("nativePC/models/second.mod3", ["nativePC"]).expect("valid");
    let plan = InstallPlan::from_providers(vec![
        InstallFileProvider::new(
            ModId::new("mod-a"),
            PackageFileId::new("nativePC/models/first.mod3"),
            first_target,
            FileLayer::new("base", 0),
        ),
        InstallFileProvider::new(
            ModId::new("mod-a"),
            PackageFileId::new("nativePC/models/second.mod3"),
            second_target,
            FileLayer::new("base", 0),
        ),
    ]);
    let source_files = Arc::new(RecordingInstallSourceFileReader::new([
        ("nativePC/models/first.mod3", b"new first".as_slice()),
        ("nativePC/models/second.mod3", b"new second".as_slice()),
    ]));
    let game_files = Arc::new(RecordingInstallGameFileSystem::with_write_failures(
        [
            ("nativePC/models/first.mod3", b"old first".as_slice()),
            ("nativePC/models/second.mod3", b"old second".as_slice()),
        ],
        [
            RecordingWriteFailure::NoFailure,
            RecordingWriteFailure::AfterMutation,
            RecordingWriteFailure::BeforeMutation,
        ],
    ));
    let backups = Arc::new(RecordingInstallBackupStore::default());
    let manifests = Arc::new(RecordingInstallManifestRepository::default());
    let recovery_records = Arc::new(RecordingInstallRecoveryRecordRepository::default());
    let service = InstallCommitService::new_with_recovery_records(
        source_files,
        game_files.clone(),
        backups.clone(),
        manifests.clone(),
        recovery_records.clone(),
    );

    let error = service
        .commit_plan(CommitInstallPlanRequest {
            game_id: GameId::mhw(),
            profile_id: ProfileId::new("default"),
            plan,
        })
        .expect_err("mixed rollback outcomes should retain only unresolved recovery state");

    assert_eq!(
        error,
        InstallCommitError::RollbackFailed {
            failed_phase: InstallCommitPhase::Write
        }
    );
    assert_eq!(
        game_files
            .file_bytes("nativePC/models/first.mod3")
            .as_deref(),
        Some(b"old first".as_slice())
    );
    assert_eq!(
        game_files
            .file_bytes("nativePC/models/second.mod3")
            .as_deref(),
        Some(b"new second".as_slice())
    );
    assert_eq!(
        backups.removed_refs(),
        vec!["backup-nativePC-models-first.mod3".to_owned()]
    );
    assert!(manifests.take_manifest().is_none());

    let saved_records = recovery_records.saved_records();
    let rollback_required = saved_records.last().expect("rollback-required record");
    assert_eq!(
        rollback_required.status,
        InstallRecoveryRecordStatus::RollbackRequired
    );
    assert_eq!(rollback_required.entries.len(), 1);
    assert_eq!(
        rollback_required.entries[0].target_path.as_str(),
        "nativePC/models/second.mod3"
    );
    assert_eq!(
        rollback_required.entries[0].backup_ref.as_deref(),
        Some("backup-nativePC-models-second.mod3-1")
    );
    assert!(recovery_records.removed_records().is_empty());
}
