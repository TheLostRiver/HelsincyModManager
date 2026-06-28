use super::*;
use hmm_core::{
    FileLayer, GameDirectoryValidation, GameId, InstallRecoveryRecord, InstallRecoveryRecordStatus,
    InstallManifestStatus, InstallTargetPathError, ModId, PackageFileId, ProfileId,
};
use hmm_ports::{
    GameAdapter, GameDirectoryProbe, InstallBackupStore, InstallGameFileSystem,
    InstallManifestRepository, InstallRecoveryRecordRepository, InstallSourceFileReader,
    ModImportResultRepository, ModImportSandboxLocator, ModPackageInstallFile,
    ModPackageInstallFileScanRequest, ModPackageInstallFileScanner, StoredImportPreviewImage,
    StoredModImportAnalysis, StoredModPackageMetadata,
};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

fn install_file(
    mod_id: &str,
    package_file_id: &str,
    target_path: &str,
    priority: i32,
) -> InstallPlanFile {
    InstallPlanFile {
        mod_id: ModId::new(mod_id),
        package_file_id: PackageFileId::new(package_file_id),
        target_path: target_path.to_owned(),
        layer: FileLayer::new("test", priority),
    }
}

#[test]
fn build_plan_parses_allowed_target_paths_into_core_plan() {
    let service = InstallPlanningService::new();
    let request = BuildInstallPlanRequest {
        allowed_target_roots: vec!["content".to_owned()],
        files: vec![install_file(
            "mod-a",
            "file-a",
            "content/models/player.mod3",
            0,
        )],
    };

    let plan = service
        .build_plan(request)
        .expect("valid request should build an install plan");

    assert!(!plan.has_blocking_conflicts());
    assert_eq!(plan.actions.len(), 1);
    assert_eq!(
        plan.actions[0].target_path.as_str(),
        "content/models/player.mod3"
    );
    assert_eq!(plan.actions[0].provider.mod_id.as_str(), "mod-a");
}

#[test]
fn build_plan_reports_package_file_for_invalid_target_path() {
    let service = InstallPlanningService::new();
    let request = BuildInstallPlanRequest {
        allowed_target_roots: vec!["content".to_owned()],
        files: vec![install_file("mod-a", "file-a", "../outside.bin", 0)],
    };

    let error = service
        .build_plan(request)
        .expect_err("invalid target path should fail planning");

    assert_eq!(
        error,
        InstallPlanningError::InvalidTargetPath {
            package_file_id: PackageFileId::new("file-a"),
            source: InstallTargetPathError::ParentTraversal,
        }
    );
}

#[test]
fn build_plan_preserves_core_conflicts() {
    let service = InstallPlanningService::new();
    let request = BuildInstallPlanRequest {
        allowed_target_roots: vec!["content".to_owned()],
        files: vec![
            install_file("mod-a", "file-a", "content/models/player.mod3", 0),
            install_file("mod-b", "file-b", "content/models/player.mod3", 0),
        ],
    };

    let plan = service
        .build_plan(request)
        .expect("valid paths should build a plan even when conflicts exist");

    assert!(plan.has_blocking_conflicts());
    assert_eq!(plan.conflicts.len(), 1);
    assert_eq!(plan.conflicts[0].providers.len(), 2);
}

#[test]
fn build_plan_from_imported_mod_uses_sandbox_files_and_adapter_roots() {
    let repository = Arc::new(FakeModImportResultRepository::new(vec![stored_analysis(
        "mod-a",
        "package-a",
    )]));
    let locator = Arc::new(FakeSandboxLocator {
        root: PathBuf::from("controlled-sandbox/package-a"),
    });
    let scanner = Arc::new(FakeInstallFileScanner {
        files: vec![ModPackageInstallFile {
            package_file_id: "nativePC/models/player.mod3".to_owned(),
            target_path: "nativePC/models/player.mod3".to_owned(),
        }],
        seen_requests: Mutex::new(Vec::new()),
    });
    let service = InstallPlanningService::with_imported_mod_sources(
        repository,
        locator,
        scanner.clone(),
        vec![Arc::new(FakeGameAdapter {
            game_id: GameId::mhw(),
            allowed_roots: vec!["nativePC".to_owned()],
        })],
    );

    let plan = service
        .build_plan_from_imported_mod(BuildImportedModInstallPlanRequest {
            game_id: GameId::mhw(),
            mod_id: ModId::new("mod-a"),
            layer: FileLayer::new("base", 0),
        })
        .expect("imported mod should build a plan");

    assert_eq!(plan.actions.len(), 1);
    assert_eq!(
        plan.actions[0].target_path.as_str(),
        "nativePC/models/player.mod3"
    );
    assert_eq!(plan.actions[0].provider.mod_id.as_str(), "mod-a");
    assert_eq!(
        plan.actions[0].provider.package_file_id.as_str(),
        "nativePC/models/player.mod3"
    );
    assert_eq!(
        scanner.seen_requests.lock().expect("requests").as_slice(),
        &[(
            "package-a".to_owned(),
            PathBuf::from("controlled-sandbox/package-a")
        )]
    );
}

#[test]
fn build_plan_from_imported_mod_ignores_files_outside_adapter_roots() {
    let repository = Arc::new(FakeModImportResultRepository::new(vec![stored_analysis(
        "mod-a",
        "package-a",
    )]));
    let locator = Arc::new(FakeSandboxLocator {
        root: PathBuf::from("controlled-sandbox/package-a"),
    });
    let scanner = Arc::new(FakeInstallFileScanner {
        files: vec![
            ModPackageInstallFile {
                package_file_id: "readme.txt".to_owned(),
                target_path: "readme.txt".to_owned(),
            },
            ModPackageInstallFile {
                package_file_id: "nativePC/models/player.mod3".to_owned(),
                target_path: "nativePC/models/player.mod3".to_owned(),
            },
        ],
        seen_requests: Mutex::new(Vec::new()),
    });
    let service = InstallPlanningService::with_imported_mod_sources(
        repository,
        locator,
        scanner,
        vec![Arc::new(FakeGameAdapter {
            game_id: GameId::mhw(),
            allowed_roots: vec!["nativePC".to_owned()],
        })],
    );

    let plan = service
        .build_plan_from_imported_mod(BuildImportedModInstallPlanRequest {
            game_id: GameId::mhw(),
            mod_id: ModId::new("mod-a"),
            layer: FileLayer::new("base", 0),
        })
        .expect("non-install files should be ignored");

    assert_eq!(plan.actions.len(), 1);
    assert_eq!(
        plan.actions[0].target_path.as_str(),
        "nativePC/models/player.mod3"
    );
}

#[test]
fn commit_plan_writes_new_files_and_persists_manifest() {
    let target = InstallTargetPath::parse("nativePC/models/player.mod3", ["nativePC"])
        .expect("valid target");
    let plan = InstallPlan::from_providers(vec![InstallFileProvider::new(
        ModId::new("mod-a"),
        PackageFileId::new("nativePC/models/player.mod3"),
        target,
        FileLayer::new("base", 0),
    )]);
    let source_files = Arc::new(RecordingInstallSourceFileReader::new([(
        "nativePC/models/player.mod3",
        b"new model".as_slice(),
    )]));
    let game_files = Arc::new(RecordingInstallGameFileSystem::default());
    let backups = Arc::new(RecordingInstallBackupStore::default());
    let manifests = Arc::new(RecordingInstallManifestRepository::default());
    let service = InstallCommitService::new(
        source_files,
        game_files.clone(),
        backups.clone(),
        manifests.clone(),
    );

    let result = service
        .commit_plan(CommitInstallPlanRequest {
            profile_id: ProfileId::new("default"),
            plan,
        })
        .expect("commit should succeed");

    assert_eq!(
        game_files
            .file_bytes("nativePC/models/player.mod3")
            .as_deref(),
        Some(b"new model".as_slice())
    );
    assert_eq!(backups.records().len(), 0);
    let manifest = manifests.take_manifest().expect("manifest should be saved");
    assert_eq!(manifest.profile_id.as_str(), "default");
    assert_eq!(manifest.status, InstallManifestStatus::Completed);
    assert_eq!(manifest.backend.as_deref(), Some("install_plan"));
    assert!(manifest.completed_at.is_some());
    assert!(manifest.plan_hash.is_none());
    assert_eq!(manifest.entries.len(), 1);
    assert_eq!(
        manifest.entries[0].target_path.as_str(),
        "nativePC/models/player.mod3"
    );
    assert_eq!(manifest.entries[0].backup_ref, None);
    let installed_file = manifest.entries[0]
        .installed_file
        .as_ref()
        .expect("manifest entry should record installed file summary");
    assert_eq!(installed_file.size_bytes, 9);
    assert_eq!(
        installed_file.sha256,
        "d556e02a85803b1d71c94a462432da55b16b443f7579c8bfdc4a44a4c7d6a17a"
    );
    assert_eq!(result.manifest, manifest);
}

#[test]
fn commit_plan_persists_recovery_record_lifecycle_when_commit_succeeds() {
    let target = InstallTargetPath::parse("nativePC/models/player.mod3", ["nativePC"])
        .expect("valid target");
    let plan = InstallPlan::from_providers(vec![InstallFileProvider::new(
        ModId::new("mod-a"),
        PackageFileId::new("nativePC/models/player.mod3"),
        target,
        FileLayer::new("base", 0),
    )]);
    let source_files = Arc::new(RecordingInstallSourceFileReader::new([(
        "nativePC/models/player.mod3",
        b"new model".as_slice(),
    )]));
    let game_files = Arc::new(RecordingInstallGameFileSystem::default());
    let backups = Arc::new(RecordingInstallBackupStore::default());
    let manifests = Arc::new(RecordingInstallManifestRepository::default());
    let recovery_records = Arc::new(RecordingInstallRecoveryRecordRepository::default());
    let service = InstallCommitService::new_with_recovery_records(
        source_files,
        game_files,
        backups,
        manifests,
        recovery_records.clone(),
    );

    service
        .commit_plan(CommitInstallPlanRequest {
            profile_id: ProfileId::new("default"),
            plan,
        })
        .expect("commit should succeed");

    let saved_records = recovery_records.saved_records();
    let statuses = saved_records
        .iter()
        .map(|record| record.status)
        .collect::<Vec<_>>();
    assert_eq!(
        statuses,
        vec![
            InstallRecoveryRecordStatus::Planned,
            InstallRecoveryRecordStatus::Committing,
            InstallRecoveryRecordStatus::Completed,
        ]
    );
    let completed = saved_records.last().expect("completed record");
    assert_eq!(completed.profile_id.as_str(), "default");
    assert_eq!(completed.mod_id.as_str(), "mod-a");
    assert_eq!(completed.entries.len(), 1);
    assert_eq!(
        completed.entries[0].target_path.as_str(),
        "nativePC/models/player.mod3"
    );
    assert_eq!(completed.entries[0].backup_ref, None);
    let installed_file = completed.entries[0]
        .installed_file
        .as_ref()
        .expect("completed record should include installed file summary");
    assert_eq!(installed_file.size_bytes, 9);
    assert_eq!(
        installed_file.sha256,
        "d556e02a85803b1d71c94a462432da55b16b443f7579c8bfdc4a44a4c7d6a17a"
    );
    assert!(recovery_records.removed_records().is_empty());
}

#[test]
fn uninstall_mod_removes_manifest_owned_new_file_when_summary_matches() {
    let target = InstallTargetPath::parse("nativePC/models/player.mod3", ["nativePC"])
        .expect("valid target");
    let existing_manifest = InstallManifest::completed(
        ProfileId::new("default"),
        vec![InstallManifestEntry {
            target_path: target,
            mod_id: ModId::new("mod-a"),
            package_file_id: PackageFileId::new("nativePC/models/player.mod3"),
            layer: FileLayer::new("base", 0),
            backup_ref: None,
            installed_file: Some(installed_file_summary(b"new model")),
        }],
    );
    let game_files = Arc::new(RecordingInstallGameFileSystem::with_files([(
        "nativePC/models/player.mod3",
        b"new model".as_slice(),
    )]));
    let backups = Arc::new(RecordingInstallBackupStore::default());
    let manifests = Arc::new(
        RecordingInstallManifestRepository::default().with_existing_manifest(existing_manifest),
    );
    let service = UninstallModService::new(game_files.clone(), backups.clone(), manifests.clone());

    let result = service
        .uninstall_mod(UninstallModRequest {
            profile_id: ProfileId::new("default"),
            mod_id: ModId::new("mod-a"),
        })
        .expect("matching manifest-owned new file should uninstall");

    assert_eq!(game_files.file_bytes("nativePC/models/player.mod3"), None);
    assert_eq!(backups.removed_refs(), Vec::<String>::new());
    let manifest = manifests.take_manifest().expect("manifest should be saved");
    assert_eq!(manifest.entries, Vec::<InstallManifestEntry>::new());
    assert_eq!(result.manifest, manifest);
    assert_eq!(result.removed_file_count, 1);
    assert_eq!(result.restored_file_count, 0);
}

#[test]
fn uninstall_mod_restores_manifest_owned_overwrite_from_backup_when_summary_matches() {
    let target = InstallTargetPath::parse("nativePC/models/player.mod3", ["nativePC"])
        .expect("valid target");
    let existing_manifest = InstallManifest::completed(
        ProfileId::new("default"),
        vec![InstallManifestEntry {
            target_path: target,
            mod_id: ModId::new("mod-a"),
            package_file_id: PackageFileId::new("nativePC/models/player.mod3"),
            layer: FileLayer::new("base", 0),
            backup_ref: Some("backup-original-player".to_owned()),
            installed_file: Some(installed_file_summary(b"modded model")),
        }],
    );
    let game_files = Arc::new(RecordingInstallGameFileSystem::with_files([(
        "nativePC/models/player.mod3",
        b"modded model".as_slice(),
    )]));
    let backups = Arc::new(RecordingInstallBackupStore::with_backups([(
        "backup-original-player",
        b"original model".as_slice(),
    )]));
    let manifests = Arc::new(
        RecordingInstallManifestRepository::default().with_existing_manifest(existing_manifest),
    );
    let service = UninstallModService::new(game_files.clone(), backups.clone(), manifests.clone());

    let result = service
        .uninstall_mod(UninstallModRequest {
            profile_id: ProfileId::new("default"),
            mod_id: ModId::new("mod-a"),
        })
        .expect("matching manifest-owned overwrite should restore backup");

    assert_eq!(
        game_files
            .file_bytes("nativePC/models/player.mod3")
            .as_deref(),
        Some(b"original model".as_slice())
    );
    assert_eq!(
        backups.removed_refs(),
        vec!["backup-original-player".to_owned()]
    );
    let manifest = manifests.take_manifest().expect("manifest should be saved");
    assert_eq!(manifest.entries, Vec::<InstallManifestEntry>::new());
    assert_eq!(result.manifest, manifest);
    assert_eq!(result.removed_file_count, 0);
    assert_eq!(result.restored_file_count, 1);
}

#[test]
fn uninstall_mod_preserves_manifest_origin_metadata_for_remaining_entries() {
    let remove_target = InstallTargetPath::parse("nativePC/models/remove.mod3", ["nativePC"])
        .expect("valid target");
    let keep_target = InstallTargetPath::parse("nativePC/models/keep.mod3", ["nativePC"])
        .expect("valid target");
    let existing_manifest = InstallManifest::completed_with_metadata(
        ProfileId::new("default"),
        vec![
            InstallManifestEntry {
                target_path: remove_target,
                mod_id: ModId::new("mod-a"),
                package_file_id: PackageFileId::new("nativePC/models/remove.mod3"),
                layer: FileLayer::new("base", 0),
                backup_ref: None,
                installed_file: Some(installed_file_summary(b"remove model")),
            },
            InstallManifestEntry {
                target_path: keep_target,
                mod_id: ModId::new("mod-b"),
                package_file_id: PackageFileId::new("nativePC/models/keep.mod3"),
                layer: FileLayer::new("base", 0),
                backup_ref: Some("backup-keep".to_owned()),
                installed_file: Some(installed_file_summary(b"keep model")),
            },
        ],
        Some("install_plan".to_owned()),
        Some("2026-06-29T00:00:00Z".to_owned()),
        Some("2026-06-29T00:00:01Z".to_owned()),
        Some("sha256:stale-plan".to_owned()),
    );
    let game_files = Arc::new(RecordingInstallGameFileSystem::with_files([(
        "nativePC/models/remove.mod3",
        b"remove model".as_slice(),
    )]));
    let backups = Arc::new(RecordingInstallBackupStore::default());
    let manifests = Arc::new(
        RecordingInstallManifestRepository::default().with_existing_manifest(existing_manifest),
    );
    let service = UninstallModService::new(game_files, backups, manifests.clone());

    service
        .uninstall_mod(UninstallModRequest {
            profile_id: ProfileId::new("default"),
            mod_id: ModId::new("mod-a"),
        })
        .expect("uninstall should preserve manifest metadata for remaining entries");

    let manifest = manifests.take_manifest().expect("manifest should be saved");
    assert_eq!(manifest.backend.as_deref(), Some("install_plan"));
    assert_eq!(
        manifest.created_at.as_deref(),
        Some("2026-06-29T00:00:00Z")
    );
    assert_eq!(manifest.status, InstallManifestStatus::Completed);
    assert!(manifest.completed_at.is_some());
    assert_ne!(
        manifest.completed_at.as_deref(),
        Some("2026-06-29T00:00:01Z")
    );
    assert_eq!(manifest.plan_hash, None);
    assert_eq!(manifest.entries.len(), 1);
    assert_eq!(manifest.entries[0].mod_id.as_str(), "mod-b");
}

#[test]
fn uninstall_mod_rolls_back_removed_file_when_manifest_save_fails() {
    let target = InstallTargetPath::parse("nativePC/models/player.mod3", ["nativePC"])
        .expect("valid target");
    let existing_manifest = InstallManifest::completed(
        ProfileId::new("default"),
        vec![InstallManifestEntry {
            target_path: target,
            mod_id: ModId::new("mod-a"),
            package_file_id: PackageFileId::new("nativePC/models/player.mod3"),
            layer: FileLayer::new("base", 0),
            backup_ref: None,
            installed_file: Some(installed_file_summary(b"new model")),
        }],
    );
    let game_files = Arc::new(RecordingInstallGameFileSystem::with_files([(
        "nativePC/models/player.mod3",
        b"new model".as_slice(),
    )]));
    let backups = Arc::new(RecordingInstallBackupStore::default());
    let manifests = Arc::new(
        RecordingInstallManifestRepository::failing().with_existing_manifest(existing_manifest),
    );
    let service = UninstallModService::new(game_files.clone(), backups.clone(), manifests.clone());

    let error = service
        .uninstall_mod(UninstallModRequest {
            profile_id: ProfileId::new("default"),
            mod_id: ModId::new("mod-a"),
        })
        .expect_err("manifest save failure should abort uninstall");

    assert_eq!(error, UninstallModError::ManifestSaveFailed);
    assert_eq!(
        game_files
            .file_bytes("nativePC/models/player.mod3")
            .as_deref(),
        Some(b"new model".as_slice())
    );
    assert!(manifests.take_manifest().is_none());
}

#[test]
fn uninstall_mod_revalidates_target_before_removing_new_file() {
    let existing_manifest = InstallManifest::completed(
        ProfileId::new("default"),
        vec![InstallManifestEntry {
            target_path: InstallTargetPath::parse("nativePC/models/player.mod3", ["nativePC"])
                .expect("valid target"),
            mod_id: ModId::new("mod-a"),
            package_file_id: PackageFileId::new("nativePC/models/player.mod3"),
            layer: FileLayer::new("base", 0),
            backup_ref: None,
            installed_file: Some(installed_file_summary(b"new model")),
        }],
    );
    let game_files = Arc::new(
        RecordingInstallGameFileSystem::with_files([(
            "nativePC/models/player.mod3",
            b"new model".as_slice(),
        )])
        .with_read_mutation("nativePC/models/player.mod3", b"external edit"),
    );
    let backups = Arc::new(RecordingInstallBackupStore::default());
    let manifests = Arc::new(
        RecordingInstallManifestRepository::default().with_existing_manifest(existing_manifest),
    );
    let service = UninstallModService::new(game_files.clone(), backups, manifests.clone());

    let error = service
        .uninstall_mod(UninstallModRequest {
            profile_id: ProfileId::new("default"),
            mod_id: ModId::new("mod-a"),
        })
        .expect_err("changed target should be blocked at mutation time");

    assert_eq!(error, UninstallModError::TargetStateMismatch);
    assert_eq!(
        game_files
            .file_bytes("nativePC/models/player.mod3")
            .as_deref(),
        Some(b"external edit".as_slice())
    );
    assert!(manifests.take_manifest().is_none());
}

#[test]
fn uninstall_mod_reports_rollback_failure_when_manifest_save_rollback_fails() {
    let existing_manifest = InstallManifest::completed(
        ProfileId::new("default"),
        vec![InstallManifestEntry {
            target_path: InstallTargetPath::parse("nativePC/models/player.mod3", ["nativePC"])
                .expect("valid target"),
            mod_id: ModId::new("mod-a"),
            package_file_id: PackageFileId::new("nativePC/models/player.mod3"),
            layer: FileLayer::new("base", 0),
            backup_ref: None,
            installed_file: Some(installed_file_summary(b"new model")),
        }],
    );
    let game_files = Arc::new(RecordingInstallGameFileSystem::with_failing_writes([(
        "nativePC/models/player.mod3",
        b"new model".as_slice(),
    )]));
    let backups = Arc::new(RecordingInstallBackupStore::default());
    let manifests = Arc::new(
        RecordingInstallManifestRepository::failing().with_existing_manifest(existing_manifest),
    );
    let service = UninstallModService::new(game_files.clone(), backups, manifests.clone());

    let error = service
        .uninstall_mod(UninstallModRequest {
            profile_id: ProfileId::new("default"),
            mod_id: ModId::new("mod-a"),
        })
        .expect_err("rollback failure should be reported");

    assert_eq!(
        error,
        UninstallModError::RollbackFailed {
            failed_phase: UninstallModPhase::ManifestSave
        }
    );
    assert_eq!(game_files.file_bytes("nativePC/models/player.mod3"), None);
    assert!(manifests.take_manifest().is_none());
}

#[test]
fn uninstall_mod_blocks_legacy_manifest_entry_without_installed_file_summary() {
    let existing_manifest = InstallManifest::completed(
        ProfileId::new("default"),
        vec![InstallManifestEntry {
            target_path: InstallTargetPath::parse("nativePC/models/player.mod3", ["nativePC"])
                .expect("valid target"),
            mod_id: ModId::new("mod-a"),
            package_file_id: PackageFileId::new("nativePC/models/player.mod3"),
            layer: FileLayer::new("base", 0),
            backup_ref: None,
            installed_file: None,
        }],
    );
    let game_files = Arc::new(RecordingInstallGameFileSystem::with_files([(
        "nativePC/models/player.mod3",
        b"new model".as_slice(),
    )]));
    let backups = Arc::new(RecordingInstallBackupStore::default());
    let manifests = Arc::new(
        RecordingInstallManifestRepository::default().with_existing_manifest(existing_manifest),
    );
    let service = UninstallModService::new(game_files.clone(), backups, manifests.clone());

    let error = service
        .uninstall_mod(UninstallModRequest {
            profile_id: ProfileId::new("default"),
            mod_id: ModId::new("mod-a"),
        })
        .expect_err("legacy entries without installed summary must be blocked");

    assert_eq!(error, UninstallModError::MissingInstalledFileSummary);
    assert_eq!(
        game_files
            .file_bytes("nativePC/models/player.mod3")
            .as_deref(),
        Some(b"new model".as_slice())
    );
    assert!(manifests.take_manifest().is_none());
}

#[test]
fn uninstall_mod_blocks_when_target_summary_differs_from_manifest() {
    let existing_manifest = InstallManifest::completed(
        ProfileId::new("default"),
        vec![InstallManifestEntry {
            target_path: InstallTargetPath::parse("nativePC/models/player.mod3", ["nativePC"])
                .expect("valid target"),
            mod_id: ModId::new("mod-a"),
            package_file_id: PackageFileId::new("nativePC/models/player.mod3"),
            layer: FileLayer::new("base", 0),
            backup_ref: None,
            installed_file: Some(installed_file_summary(b"new model")),
        }],
    );
    let game_files = Arc::new(RecordingInstallGameFileSystem::with_files([(
        "nativePC/models/player.mod3",
        b"external edit".as_slice(),
    )]));
    let backups = Arc::new(RecordingInstallBackupStore::default());
    let manifests = Arc::new(
        RecordingInstallManifestRepository::default().with_existing_manifest(existing_manifest),
    );
    let service = UninstallModService::new(game_files.clone(), backups, manifests.clone());

    let error = service
        .uninstall_mod(UninstallModRequest {
            profile_id: ProfileId::new("default"),
            mod_id: ModId::new("mod-a"),
        })
        .expect_err("changed target should be blocked");

    assert_eq!(error, UninstallModError::TargetStateMismatch);
    assert_eq!(
        game_files
            .file_bytes("nativePC/models/player.mod3")
            .as_deref(),
        Some(b"external edit".as_slice())
    );
    assert!(manifests.take_manifest().is_none());
}

#[test]
fn uninstall_mod_blocks_when_manifest_backup_is_missing() {
    let existing_manifest = InstallManifest::completed(
        ProfileId::new("default"),
        vec![InstallManifestEntry {
            target_path: InstallTargetPath::parse("nativePC/models/player.mod3", ["nativePC"])
                .expect("valid target"),
            mod_id: ModId::new("mod-a"),
            package_file_id: PackageFileId::new("nativePC/models/player.mod3"),
            layer: FileLayer::new("base", 0),
            backup_ref: Some("missing-backup".to_owned()),
            installed_file: Some(installed_file_summary(b"new model")),
        }],
    );
    let game_files = Arc::new(RecordingInstallGameFileSystem::with_files([(
        "nativePC/models/player.mod3",
        b"new model".as_slice(),
    )]));
    let backups = Arc::new(RecordingInstallBackupStore::default());
    let manifests = Arc::new(
        RecordingInstallManifestRepository::default().with_existing_manifest(existing_manifest),
    );
    let service = UninstallModService::new(game_files.clone(), backups, manifests.clone());

    let error = service
        .uninstall_mod(UninstallModRequest {
            profile_id: ProfileId::new("default"),
            mod_id: ModId::new("mod-a"),
        })
        .expect_err("missing backup must block restore");

    assert_eq!(error, UninstallModError::BackupUnavailable);
    assert_eq!(
        game_files
            .file_bytes("nativePC/models/player.mod3")
            .as_deref(),
        Some(b"new model".as_slice())
    );
    assert!(manifests.take_manifest().is_none());
}

#[test]
fn commit_plan_merges_existing_manifest_by_target_path() {
    let target = InstallTargetPath::parse("nativePC/models/player.mod3", ["nativePC"])
        .expect("valid target");
    let plan = InstallPlan::from_providers(vec![InstallFileProvider::new(
        ModId::new("mod-new"),
        PackageFileId::new("nativePC/models/player.mod3"),
        target,
        FileLayer::new("base", 0),
    )]);
    let source_files = Arc::new(RecordingInstallSourceFileReader::new([(
        "nativePC/models/player.mod3",
        b"new model".as_slice(),
    )]));
    let game_files = Arc::new(RecordingInstallGameFileSystem::default());
    let backups = Arc::new(RecordingInstallBackupStore::default());
    let existing_manifest = InstallManifest::completed(
        ProfileId::new("default"),
        vec![
            InstallManifestEntry {
                target_path: InstallTargetPath::parse("nativePC/models/keep.mod3", ["nativePC"])
                    .expect("valid target"),
                mod_id: ModId::new("mod-new"),
                package_file_id: PackageFileId::new("nativePC/models/keep.mod3"),
                layer: FileLayer::new("base", 0),
                backup_ref: None,
                installed_file: None,
            },
            InstallManifestEntry {
                target_path: InstallTargetPath::parse("nativePC/models/player.mod3", ["nativePC"])
                    .expect("valid target"),
                mod_id: ModId::new("mod-old"),
                package_file_id: PackageFileId::new("nativePC/models/player-old.mod3"),
                layer: FileLayer::new("base", 0),
                backup_ref: Some("backup-old-player".to_owned()),
                installed_file: None,
            },
        ],
    );
    let manifests = Arc::new(
        RecordingInstallManifestRepository::default().with_existing_manifest(existing_manifest),
    );
    let service = InstallCommitService::new(source_files, game_files, backups, manifests.clone());

    service
        .commit_plan(CommitInstallPlanRequest {
            profile_id: ProfileId::new("default"),
            plan,
        })
        .expect("commit should succeed");

    let manifest = manifests.take_manifest().expect("manifest should be saved");
    assert_eq!(manifest.profile_id.as_str(), "default");
    assert_eq!(
        manifest
            .entries
            .iter()
            .map(|entry| (
                entry.target_path.as_str(),
                entry.mod_id.as_str(),
                entry.package_file_id.as_str()
            ))
            .collect::<Vec<_>>(),
        vec![
            (
                "nativePC/models/keep.mod3",
                "mod-new",
                "nativePC/models/keep.mod3"
            ),
            (
                "nativePC/models/player.mod3",
                "mod-new",
                "nativePC/models/player.mod3"
            ),
        ]
    );
}

#[test]
fn commit_plan_preserves_existing_backup_ref_when_replacing_manifest_entry() {
    let target = InstallTargetPath::parse("nativePC/models/player.mod3", ["nativePC"])
        .expect("valid target");
    let plan = InstallPlan::from_providers(vec![InstallFileProvider::new(
        ModId::new("mod-new"),
        PackageFileId::new("nativePC/models/player-new.mod3"),
        target,
        FileLayer::new("base", 0),
    )]);
    let source_files = Arc::new(RecordingInstallSourceFileReader::new([(
        "nativePC/models/player-new.mod3",
        b"new model".as_slice(),
    )]));
    let game_files = Arc::new(RecordingInstallGameFileSystem::with_files([(
        "nativePC/models/player.mod3",
        b"old managed model".as_slice(),
    )]));
    let backups = Arc::new(RecordingInstallBackupStore::default());
    let existing_manifest = InstallManifest::completed(
        ProfileId::new("default"),
        vec![InstallManifestEntry {
            target_path: InstallTargetPath::parse("nativePC/models/player.mod3", ["nativePC"])
                .expect("valid target"),
            mod_id: ModId::new("mod-old"),
            package_file_id: PackageFileId::new("nativePC/models/player-old.mod3"),
            layer: FileLayer::new("base", 0),
            backup_ref: Some("backup-original-player".to_owned()),
            installed_file: None,
        }],
    );
    let manifests = Arc::new(
        RecordingInstallManifestRepository::default().with_existing_manifest(existing_manifest),
    );
    let service =
        InstallCommitService::new(source_files, game_files, backups.clone(), manifests.clone());

    service
        .commit_plan(CommitInstallPlanRequest {
            profile_id: ProfileId::new("default"),
            plan,
        })
        .expect("commit should succeed");

    let manifest = manifests.take_manifest().expect("manifest should be saved");
    assert_eq!(manifest.entries.len(), 1);
    assert_eq!(
        manifest.entries[0].package_file_id.as_str(),
        "nativePC/models/player-new.mod3"
    );
    assert_eq!(
        manifest.entries[0].backup_ref.as_deref(),
        Some("backup-original-player")
    );
    assert_eq!(
        backups.records(),
        vec![(
            "nativePC/models/player.mod3".to_owned(),
            b"old managed model".to_vec()
        )]
    );
    assert_eq!(
        backups.removed_refs(),
        vec!["backup-nativePC-models-player.mod3".to_owned()]
    );
}

#[test]
fn commit_plan_keeps_absent_backup_ref_when_replacing_managed_new_file() {
    let target = InstallTargetPath::parse("nativePC/models/new-file.mod3", ["nativePC"])
        .expect("valid target");
    let plan = InstallPlan::from_providers(vec![InstallFileProvider::new(
        ModId::new("mod-new"),
        PackageFileId::new("nativePC/models/new-file-v2.mod3"),
        target,
        FileLayer::new("base", 0),
    )]);
    let source_files = Arc::new(RecordingInstallSourceFileReader::new([(
        "nativePC/models/new-file-v2.mod3",
        b"new model v2".as_slice(),
    )]));
    let game_files = Arc::new(RecordingInstallGameFileSystem::with_files([(
        "nativePC/models/new-file.mod3",
        b"old managed new file".as_slice(),
    )]));
    let backups = Arc::new(RecordingInstallBackupStore::default());
    let existing_manifest = InstallManifest::completed(
        ProfileId::new("default"),
        vec![InstallManifestEntry {
            target_path: InstallTargetPath::parse("nativePC/models/new-file.mod3", ["nativePC"])
                .expect("valid target"),
            mod_id: ModId::new("mod-old"),
            package_file_id: PackageFileId::new("nativePC/models/new-file-v1.mod3"),
            layer: FileLayer::new("base", 0),
            backup_ref: None,
            installed_file: None,
        }],
    );
    let manifests = Arc::new(
        RecordingInstallManifestRepository::default().with_existing_manifest(existing_manifest),
    );
    let service =
        InstallCommitService::new(source_files, game_files, backups.clone(), manifests.clone());

    service
        .commit_plan(CommitInstallPlanRequest {
            profile_id: ProfileId::new("default"),
            plan,
        })
        .expect("commit should succeed");

    let manifest = manifests.take_manifest().expect("manifest should be saved");
    assert_eq!(manifest.entries.len(), 1);
    assert_eq!(
        manifest.entries[0].package_file_id.as_str(),
        "nativePC/models/new-file-v2.mod3"
    );
    assert_eq!(manifest.entries[0].backup_ref, None);
    assert_eq!(
        backups.removed_refs(),
        vec!["backup-nativePC-models-new-file.mod3".to_owned()]
    );
}

#[test]
fn commit_plan_aborts_before_writes_when_manifest_load_fails() {
    let target = InstallTargetPath::parse("nativePC/models/player.mod3", ["nativePC"])
        .expect("valid target");
    let plan = InstallPlan::from_providers(vec![InstallFileProvider::new(
        ModId::new("mod-a"),
        PackageFileId::new("nativePC/models/player.mod3"),
        target,
        FileLayer::new("base", 0),
    )]);
    let source_files = Arc::new(RecordingInstallSourceFileReader::new([]));
    let game_files = Arc::new(RecordingInstallGameFileSystem::default());
    let backups = Arc::new(RecordingInstallBackupStore::default());
    let manifests = Arc::new(RecordingInstallManifestRepository::failing_load());
    let service = InstallCommitService::new(source_files, game_files.clone(), backups, manifests);

    let error = service
        .commit_plan(CommitInstallPlanRequest {
            profile_id: ProfileId::new("default"),
            plan,
        })
        .expect_err("manifest load failure should abort before file operations");

    assert_eq!(
        error,
        InstallCommitError::Failed {
            phase: InstallCommitPhase::ManifestRead
        }
    );
    assert_eq!(game_files.file_bytes("nativePC/models/player.mod3"), None);
}

#[test]
fn commit_plan_backs_up_overwritten_files_before_writing_manifest() {
    let target = InstallTargetPath::parse("nativePC/models/player.mod3", ["nativePC"])
        .expect("valid target");
    let plan = InstallPlan::from_providers(vec![InstallFileProvider::new(
        ModId::new("mod-a"),
        PackageFileId::new("nativePC/models/player.mod3"),
        target,
        FileLayer::new("base", 0),
    )]);
    let source_files = Arc::new(RecordingInstallSourceFileReader::new([(
        "nativePC/models/player.mod3",
        b"new model".as_slice(),
    )]));
    let game_files = Arc::new(RecordingInstallGameFileSystem::with_files([(
        "nativePC/models/player.mod3",
        b"old model".as_slice(),
    )]));
    let backups = Arc::new(RecordingInstallBackupStore::default());
    let manifests = Arc::new(RecordingInstallManifestRepository::default());
    let service = InstallCommitService::new(
        source_files,
        game_files.clone(),
        backups.clone(),
        manifests.clone(),
    );

    service
        .commit_plan(CommitInstallPlanRequest {
            profile_id: ProfileId::new("default"),
            plan,
        })
        .expect("commit should succeed");

    assert_eq!(
        game_files
            .file_bytes("nativePC/models/player.mod3")
            .as_deref(),
        Some(b"new model".as_slice())
    );
    assert_eq!(
        backups.records(),
        vec![(
            "nativePC/models/player.mod3".to_owned(),
            b"old model".to_vec()
        )]
    );
    let manifest = manifests.take_manifest().expect("manifest should be saved");
    assert_eq!(
        manifest.entries[0].backup_ref.as_deref(),
        Some("backup-nativePC-models-player.mod3")
    );
}

#[test]
fn commit_plan_applies_layered_same_target_actions_in_priority_order() {
    let target = InstallTargetPath::parse("nativePC/models/player.mod3", ["nativePC"])
        .expect("valid target");
    let plan = InstallPlan::from_providers(vec![
        InstallFileProvider::new(
            ModId::new("mod-low"),
            PackageFileId::new("nativePC/models/player-low.mod3"),
            target.clone(),
            FileLayer::new("low", 0),
        ),
        InstallFileProvider::new(
            ModId::new("mod-high"),
            PackageFileId::new("nativePC/models/player-high.mod3"),
            target,
            FileLayer::new("high", 10),
        ),
    ]);
    let source_files = Arc::new(RecordingInstallSourceFileReader::new([
        ("nativePC/models/player-low.mod3", b"low layer".as_slice()),
        ("nativePC/models/player-high.mod3", b"high layer".as_slice()),
    ]));
    let game_files = Arc::new(RecordingInstallGameFileSystem::with_files([(
        "nativePC/models/player.mod3",
        b"original".as_slice(),
    )]));
    let backups = Arc::new(RecordingInstallBackupStore::default());
    let manifests = Arc::new(RecordingInstallManifestRepository::default());
    let service = InstallCommitService::new(
        source_files,
        game_files.clone(),
        backups.clone(),
        manifests.clone(),
    );

    service
        .commit_plan(CommitInstallPlanRequest {
            profile_id: ProfileId::new("default"),
            plan,
        })
        .expect("commit should succeed");

    assert_eq!(
        game_files
            .file_bytes("nativePC/models/player.mod3")
            .as_deref(),
        Some(b"high layer".as_slice())
    );
    assert_eq!(
        backups.records(),
        vec![
            (
                "nativePC/models/player.mod3".to_owned(),
                b"original".to_vec()
            ),
            (
                "nativePC/models/player.mod3".to_owned(),
                b"low layer".to_vec()
            ),
        ]
    );
    let manifest = manifests.take_manifest().expect("manifest should be saved");
    assert_eq!(manifest.entries.len(), 2);
    assert_eq!(
        manifest.entries[0].package_file_id.as_str(),
        "nativePC/models/player-low.mod3"
    );
    assert_eq!(
        manifest.entries[1].package_file_id.as_str(),
        "nativePC/models/player-high.mod3"
    );
}

#[test]
fn commit_plan_rolls_back_written_files_when_manifest_save_fails() {
    let new_target =
        InstallTargetPath::parse("nativePC/models/new.mod3", ["nativePC"]).expect("valid");
    let existing_target =
        InstallTargetPath::parse("nativePC/models/player.mod3", ["nativePC"]).expect("valid");
    let plan = InstallPlan::from_providers(vec![
        InstallFileProvider::new(
            ModId::new("mod-a"),
            PackageFileId::new("nativePC/models/new.mod3"),
            new_target,
            FileLayer::new("base", 0),
        ),
        InstallFileProvider::new(
            ModId::new("mod-a"),
            PackageFileId::new("nativePC/models/player.mod3"),
            existing_target,
            FileLayer::new("base", 0),
        ),
    ]);
    let source_files = Arc::new(RecordingInstallSourceFileReader::new([
        ("nativePC/models/new.mod3", b"new file".as_slice()),
        ("nativePC/models/player.mod3", b"new model".as_slice()),
    ]));
    let game_files = Arc::new(RecordingInstallGameFileSystem::with_files([(
        "nativePC/models/player.mod3",
        b"old model".as_slice(),
    )]));
    let backups = Arc::new(RecordingInstallBackupStore::default());
    let manifests = Arc::new(RecordingInstallManifestRepository::failing());
    let service =
        InstallCommitService::new(source_files, game_files.clone(), backups.clone(), manifests);

    let error = service
        .commit_plan(CommitInstallPlanRequest {
            profile_id: ProfileId::new("default"),
            plan,
        })
        .expect_err("manifest failure should abort commit");

    assert_eq!(
        error,
        InstallCommitError::RollbackSucceeded {
            failed_phase: InstallCommitPhase::Manifest
        }
    );
    assert_eq!(game_files.file_bytes("nativePC/models/new.mod3"), None);
    assert_eq!(
        game_files
            .file_bytes("nativePC/models/player.mod3")
            .as_deref(),
        Some(b"old model".as_slice())
    );
    assert_eq!(
        backups.removed_refs(),
        vec!["backup-nativePC-models-player.mod3".to_owned()]
    );
}

#[test]
fn commit_plan_marks_recovery_record_rollback_required_when_rollback_fails() {
    let target = InstallTargetPath::parse("nativePC/models/new.mod3", ["nativePC"]).expect("valid");
    let plan = InstallPlan::from_providers(vec![InstallFileProvider::new(
        ModId::new("mod-a"),
        PackageFileId::new("nativePC/models/new.mod3"),
        target,
        FileLayer::new("base", 0),
    )]);
    let source_files = Arc::new(RecordingInstallSourceFileReader::new([(
        "nativePC/models/new.mod3",
        b"new file".as_slice(),
    )]));
    let game_files = Arc::new(RecordingInstallGameFileSystem::default().with_failing_removes());
    let backups = Arc::new(RecordingInstallBackupStore::default());
    let manifests = Arc::new(RecordingInstallManifestRepository::failing());
    let recovery_records = Arc::new(RecordingInstallRecoveryRecordRepository::default());
    let service = InstallCommitService::new_with_recovery_records(
        source_files,
        game_files.clone(),
        backups,
        manifests,
        recovery_records.clone(),
    );

    let error = service
        .commit_plan(CommitInstallPlanRequest {
            profile_id: ProfileId::new("default"),
            plan,
        })
        .expect_err("rollback failure should leave a recovery record");

    assert_eq!(
        error,
        InstallCommitError::RollbackFailed {
            failed_phase: InstallCommitPhase::Manifest
        }
    );
    assert_eq!(
        game_files.file_bytes("nativePC/models/new.mod3").as_deref(),
        Some(b"new file".as_slice())
    );

    let saved_records = recovery_records.saved_records();
    let statuses = saved_records
        .iter()
        .map(|record| record.status)
        .collect::<Vec<_>>();
    assert_eq!(
        statuses,
        vec![
            InstallRecoveryRecordStatus::Planned,
            InstallRecoveryRecordStatus::Committing,
            InstallRecoveryRecordStatus::RollbackRequired,
        ]
    );
    let rollback_required = saved_records.last().expect("rollback-required record");
    assert_eq!(rollback_required.profile_id.as_str(), "default");
    assert_eq!(rollback_required.mod_id.as_str(), "mod-a");
    assert_eq!(rollback_required.entries.len(), 1);
    assert_eq!(
        rollback_required.entries[0].target_path.as_str(),
        "nativePC/models/new.mod3"
    );
    assert_eq!(rollback_required.entries[0].backup_ref, None);
    assert!(rollback_required.entries[0].installed_file.is_some());
    assert!(recovery_records.removed_records().is_empty());
}

#[test]
fn commit_plan_rollback_record_uses_pending_backup_when_replacing_managed_target() {
    let new_file_target =
        InstallTargetPath::parse("nativePC/models/new.mod3", ["nativePC"]).expect("valid target");
    let replaced_target = InstallTargetPath::parse("nativePC/models/player.mod3", ["nativePC"])
        .expect("valid target");
    let plan = InstallPlan::from_providers(vec![
        InstallFileProvider::new(
            ModId::new("mod-new"),
            PackageFileId::new("nativePC/models/new.mod3"),
            new_file_target,
            FileLayer::new("base", 0),
        ),
        InstallFileProvider::new(
            ModId::new("mod-new"),
            PackageFileId::new("nativePC/models/player-v2.mod3"),
            replaced_target,
            FileLayer::new("base", 0),
        ),
    ]);
    let source_files = Arc::new(RecordingInstallSourceFileReader::new([
        ("nativePC/models/new.mod3", b"new file".as_slice()),
        (
            "nativePC/models/player-v2.mod3",
            b"new managed model".as_slice(),
        ),
    ]));
    let game_files = Arc::new(
        RecordingInstallGameFileSystem::with_files([(
            "nativePC/models/player.mod3",
            b"old managed model".as_slice(),
        )])
        .with_failing_removes(),
    );
    let backups = Arc::new(RecordingInstallBackupStore::with_backups([(
        "backup-original-player",
        b"original game model".as_slice(),
    )]));
    let existing_manifest = InstallManifest::completed(
        ProfileId::new("default"),
        vec![InstallManifestEntry {
            target_path: InstallTargetPath::parse("nativePC/models/player.mod3", ["nativePC"])
                .expect("valid target"),
            mod_id: ModId::new("mod-old"),
            package_file_id: PackageFileId::new("nativePC/models/player-v1.mod3"),
            layer: FileLayer::new("base", 0),
            backup_ref: Some("backup-original-player".to_owned()),
            installed_file: Some(installed_file_summary(b"old managed model")),
        }],
    );
    let manifests = Arc::new(
        RecordingInstallManifestRepository::failing().with_existing_manifest(existing_manifest),
    );
    let recovery_records = Arc::new(RecordingInstallRecoveryRecordRepository::default());
    let service = InstallCommitService::new_with_recovery_records(
        source_files,
        game_files,
        backups,
        manifests,
        recovery_records.clone(),
    );

    let error = service
        .commit_plan(CommitInstallPlanRequest {
            profile_id: ProfileId::new("default"),
            plan,
        })
        .expect_err("manifest failure and rollback failure should leave a recovery record");

    assert_eq!(
        error,
        InstallCommitError::RollbackFailed {
            failed_phase: InstallCommitPhase::Manifest
        }
    );
    let saved_records = recovery_records.saved_records();
    let rollback_required = saved_records.last().expect("rollback-required record");
    assert_eq!(
        rollback_required.status,
        InstallRecoveryRecordStatus::RollbackRequired
    );
    let replaced_entry = rollback_required
        .entries
        .iter()
        .find(|entry| entry.target_path.as_str() == "nativePC/models/player.mod3")
        .expect("replaced target should be tracked");

    assert_eq!(
        replaced_entry.backup_ref.as_deref(),
        Some("backup-nativePC-models-player.mod3-1"),
        "recovery rollback must restore the immediate pre-commit bytes, not the long-term manifest backup"
    );
}

#[test]
fn commit_plan_persists_committing_record_after_later_pending_backup_update() {
    let new_file_target =
        InstallTargetPath::parse("nativePC/models/new.mod3", ["nativePC"]).expect("valid target");
    let replaced_target = InstallTargetPath::parse("nativePC/models/player.mod3", ["nativePC"])
        .expect("valid target");
    let plan = InstallPlan::from_providers(vec![
        InstallFileProvider::new(
            ModId::new("mod-new"),
            PackageFileId::new("nativePC/models/new.mod3"),
            new_file_target,
            FileLayer::new("base", 0),
        ),
        InstallFileProvider::new(
            ModId::new("mod-new"),
            PackageFileId::new("nativePC/models/player-v2.mod3"),
            replaced_target,
            FileLayer::new("base", 0),
        ),
    ]);
    let source_files = Arc::new(RecordingInstallSourceFileReader::new([
        ("nativePC/models/new.mod3", b"new file".as_slice()),
        (
            "nativePC/models/player-v2.mod3",
            b"new managed model".as_slice(),
        ),
    ]));
    let game_files = Arc::new(RecordingInstallGameFileSystem::with_files([(
        "nativePC/models/player.mod3",
        b"old managed model".as_slice(),
    )]));
    let backups = Arc::new(RecordingInstallBackupStore::with_backups([(
        "backup-original-player",
        b"original game model".as_slice(),
    )]));
    let existing_manifest = InstallManifest::completed(
        ProfileId::new("default"),
        vec![InstallManifestEntry {
            target_path: InstallTargetPath::parse("nativePC/models/player.mod3", ["nativePC"])
                .expect("valid target"),
            mod_id: ModId::new("mod-old"),
            package_file_id: PackageFileId::new("nativePC/models/player-v1.mod3"),
            layer: FileLayer::new("base", 0),
            backup_ref: Some("backup-original-player".to_owned()),
            installed_file: Some(installed_file_summary(b"old managed model")),
        }],
    );
    let manifests = Arc::new(
        RecordingInstallManifestRepository::default().with_existing_manifest(existing_manifest),
    );
    let recovery_records = Arc::new(RecordingInstallRecoveryRecordRepository::default());
    let service = InstallCommitService::new_with_recovery_records(
        source_files,
        game_files,
        backups,
        manifests,
        recovery_records.clone(),
    );

    service
        .commit_plan(CommitInstallPlanRequest {
            profile_id: ProfileId::new("default"),
            plan,
        })
        .expect("commit succeeds");

    let saved_records = recovery_records.saved_records();
    let committing = saved_records
        .iter()
        .rev()
        .find(|record| record.status == InstallRecoveryRecordStatus::Committing)
        .expect("committing record should be persisted after every rollback entry update");
    let replaced_entry = committing
        .entries
        .iter()
        .find(|entry| entry.target_path.as_str() == "nativePC/models/player.mod3")
        .expect("replaced target should be tracked");

    assert_eq!(
        replaced_entry.backup_ref.as_deref(),
        Some("backup-nativePC-models-player.mod3-1"),
        "crash recovery must see the pending backup for later actions before manifest save"
    );

    let completed = saved_records
        .iter()
        .rev()
        .find(|record| record.status == InstallRecoveryRecordStatus::Completed)
        .expect("completed record should be persisted after manifest save");
    let completed_entry = completed
        .entries
        .iter()
        .find(|entry| entry.target_path.as_str() == "nativePC/models/player.mod3")
        .expect("replaced target should be tracked");

    assert_eq!(
        completed_entry.backup_ref.as_deref(),
        Some("backup-original-player"),
        "completed record should resync to the manifest's long-term backup"
    );
}

#[test]
fn commit_plan_removes_recovery_record_when_rollback_succeeds() {
    let target = InstallTargetPath::parse("nativePC/models/new.mod3", ["nativePC"]).expect("valid");
    let plan = InstallPlan::from_providers(vec![InstallFileProvider::new(
        ModId::new("mod-a"),
        PackageFileId::new("nativePC/models/new.mod3"),
        target,
        FileLayer::new("base", 0),
    )]);
    let source_files = Arc::new(RecordingInstallSourceFileReader::new([(
        "nativePC/models/new.mod3",
        b"new file".as_slice(),
    )]));
    let game_files = Arc::new(RecordingInstallGameFileSystem::default());
    let backups = Arc::new(RecordingInstallBackupStore::default());
    let manifests = Arc::new(RecordingInstallManifestRepository::failing());
    let recovery_records = Arc::new(RecordingInstallRecoveryRecordRepository::default());
    let service = InstallCommitService::new_with_recovery_records(
        source_files,
        game_files.clone(),
        backups,
        manifests,
        recovery_records.clone(),
    );

    let error = service
        .commit_plan(CommitInstallPlanRequest {
            profile_id: ProfileId::new("default"),
            plan,
        })
        .expect_err("manifest failure should rollback and clear recovery record");

    assert_eq!(
        error,
        InstallCommitError::RollbackSucceeded {
            failed_phase: InstallCommitPhase::Manifest
        }
    );
    assert_eq!(game_files.file_bytes("nativePC/models/new.mod3"), None);

    let saved_records = recovery_records.saved_records();
    let statuses = saved_records
        .iter()
        .map(|record| record.status)
        .collect::<Vec<_>>();
    assert_eq!(
        statuses,
        vec![
            InstallRecoveryRecordStatus::Planned,
            InstallRecoveryRecordStatus::Committing,
        ]
    );
    assert_eq!(
        recovery_records.removed_records(),
        vec![("default".to_owned(), "mod-a".to_owned())]
    );
}

#[test]
fn commit_plan_cleans_pending_backup_when_write_fails() {
    let target = InstallTargetPath::parse("nativePC/models/player.mod3", ["nativePC"])
        .expect("valid target");
    let plan = InstallPlan::from_providers(vec![InstallFileProvider::new(
        ModId::new("mod-a"),
        PackageFileId::new("nativePC/models/player.mod3"),
        target,
        FileLayer::new("base", 0),
    )]);
    let source_files = Arc::new(RecordingInstallSourceFileReader::new([(
        "nativePC/models/player.mod3",
        b"new model".as_slice(),
    )]));
    let game_files = Arc::new(RecordingInstallGameFileSystem::with_failing_writes([(
        "nativePC/models/player.mod3",
        b"old model".as_slice(),
    )]));
    let backups = Arc::new(RecordingInstallBackupStore::default());
    let manifests = Arc::new(RecordingInstallManifestRepository::default());
    let service = InstallCommitService::new(
        source_files,
        game_files.clone(),
        backups.clone(),
        manifests.clone(),
    );

    let error = service
        .commit_plan(CommitInstallPlanRequest {
            profile_id: ProfileId::new("default"),
            plan,
        })
        .expect_err("write failure should abort commit");

    assert_eq!(
        error,
        InstallCommitError::RollbackSucceeded {
            failed_phase: InstallCommitPhase::Write
        }
    );
    assert_eq!(
        game_files
            .file_bytes("nativePC/models/player.mod3")
            .as_deref(),
        Some(b"old model".as_slice())
    );
    assert_eq!(
        backups.removed_refs(),
        vec!["backup-nativePC-models-player.mod3".to_owned()]
    );
    assert!(manifests.take_manifest().is_none());
}

#[test]
fn commit_plan_restores_all_files_even_when_backup_cleanup_fails() {
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
    let game_files = Arc::new(RecordingInstallGameFileSystem::with_files([
        ("nativePC/models/first.mod3", b"old first".as_slice()),
        ("nativePC/models/second.mod3", b"old second".as_slice()),
    ]));
    let backups = Arc::new(RecordingInstallBackupStore::failing_removals());
    let manifests = Arc::new(RecordingInstallManifestRepository::failing());
    let service =
        InstallCommitService::new(source_files, game_files.clone(), backups.clone(), manifests);

    let error = service
        .commit_plan(CommitInstallPlanRequest {
            profile_id: ProfileId::new("default"),
            plan,
        })
        .expect_err("manifest failure should trigger rollback");

    assert_eq!(
        error,
        InstallCommitError::RollbackSucceeded {
            failed_phase: InstallCommitPhase::Manifest
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
        Some(b"old second".as_slice())
    );
    assert_eq!(
        backups.removed_refs(),
        vec![
            "backup-nativePC-models-second.mod3-1".to_owned(),
            "backup-nativePC-models-first.mod3".to_owned(),
        ]
    );
}

fn stored_analysis(mod_id: &str, package_id: &str) -> StoredModImportAnalysis {
    StoredModImportAnalysis {
        mod_id: mod_id.to_owned(),
        task_id: "task-a".to_owned(),
        package_id: package_id.to_owned(),
        display_name: "Test Mod".to_owned(),
        metadata: StoredModPackageMetadata::default(),
        preview_image: StoredImportPreviewImage::Fallback {
            reason: hmm_core::PreviewImageRejectionReason::Missing,
        },
    }
}

struct FakeModImportResultRepository {
    records: Vec<StoredModImportAnalysis>,
}

impl FakeModImportResultRepository {
    fn new(records: Vec<StoredModImportAnalysis>) -> Self {
        Self { records }
    }
}

impl ModImportResultRepository for FakeModImportResultRepository {
    fn save_analysis(&self, _analysis: &StoredModImportAnalysis) -> anyhow::Result<()> {
        unreachable!("install planning must not save import analysis")
    }

    fn list_analysis(&self) -> anyhow::Result<Vec<StoredModImportAnalysis>> {
        unreachable!("install planning should look up the requested mod directly")
    }

    fn get_analysis(&self, mod_id: &str) -> anyhow::Result<Option<StoredModImportAnalysis>> {
        Ok(self
            .records
            .iter()
            .find(|record| record.mod_id == mod_id)
            .cloned())
    }
}

struct FakeSandboxLocator {
    root: PathBuf,
}

impl ModImportSandboxLocator for FakeSandboxLocator {
    fn sandbox_root_for_package(&self, _package_id: &str) -> anyhow::Result<PathBuf> {
        Ok(self.root.clone())
    }
}

struct FakeInstallFileScanner {
    files: Vec<ModPackageInstallFile>,
    seen_requests: Mutex<Vec<(String, PathBuf)>>,
}

impl ModPackageInstallFileScanner for FakeInstallFileScanner {
    fn scan_install_files(
        &self,
        request: ModPackageInstallFileScanRequest<'_>,
    ) -> anyhow::Result<Vec<ModPackageInstallFile>> {
        self.seen_requests.lock().expect("requests").push((
            request.package_id.to_owned(),
            request.sandbox_root.to_path_buf(),
        ));
        Ok(self.files.clone())
    }
}

struct FakeGameAdapter {
    game_id: GameId,
    allowed_roots: Vec<String>,
}

impl GameAdapter for FakeGameAdapter {
    fn game_id(&self) -> GameId {
        self.game_id.clone()
    }

    fn display_name(&self) -> &'static str {
        "Fake Game"
    }

    fn validate_directory(&self, _probe: &dyn GameDirectoryProbe) -> GameDirectoryValidation {
        unreachable!("install planning must not probe game directories")
    }

    fn allowed_install_roots(&self) -> Vec<String> {
        self.allowed_roots.clone()
    }
}

struct RecordingInstallSourceFileReader {
    files: BTreeMap<String, Vec<u8>>,
}

impl RecordingInstallSourceFileReader {
    fn new<const N: usize>(files: [(&str, &[u8]); N]) -> Self {
        Self {
            files: files
                .into_iter()
                .map(|(path, bytes)| (path.to_owned(), bytes.to_vec()))
                .collect(),
        }
    }
}

impl InstallSourceFileReader for RecordingInstallSourceFileReader {
    fn read_source_file(&self, package_file_id: &PackageFileId) -> anyhow::Result<Vec<u8>> {
        self.files
            .get(package_file_id.as_str())
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("missing source file"))
    }
}

#[derive(Default)]
struct RecordingInstallGameFileSystem {
    files: Mutex<BTreeMap<String, Vec<u8>>>,
    read_mutation: Mutex<Option<(String, Vec<u8>)>>,
    fail_writes: bool,
    fail_removes: bool,
}

impl RecordingInstallGameFileSystem {
    fn with_files<const N: usize>(files: [(&str, &[u8]); N]) -> Self {
        Self {
            files: Mutex::new(
                files
                    .into_iter()
                    .map(|(path, bytes)| (path.to_owned(), bytes.to_vec()))
                    .collect(),
            ),
            read_mutation: Mutex::new(None),
            fail_writes: false,
            fail_removes: false,
        }
    }

    fn with_failing_writes<const N: usize>(files: [(&str, &[u8]); N]) -> Self {
        Self {
            files: Mutex::new(
                files
                    .into_iter()
                    .map(|(path, bytes)| (path.to_owned(), bytes.to_vec()))
                    .collect(),
            ),
            read_mutation: Mutex::new(None),
            fail_writes: true,
            fail_removes: false,
        }
    }

    fn with_failing_removes(mut self) -> Self {
        self.fail_removes = true;
        self
    }

    fn with_read_mutation(self, target_path: &str, bytes: &[u8]) -> Self {
        *self.read_mutation.lock().expect("read mutation") =
            Some((target_path.to_owned(), bytes.to_vec()));
        self
    }

    fn file_bytes(&self, target_path: &str) -> Option<Vec<u8>> {
        self.files.lock().expect("files").get(target_path).cloned()
    }
}

impl InstallGameFileSystem for RecordingInstallGameFileSystem {
    fn read_game_file(&self, target_path: &InstallTargetPath) -> anyhow::Result<Option<Vec<u8>>> {
        let bytes = self
            .files
            .lock()
            .expect("files")
            .get(target_path.as_str())
            .cloned();
        if let Some((path, replacement)) = self.read_mutation.lock().expect("read mutation").take()
        {
            self.files.lock().expect("files").insert(path, replacement);
        }
        Ok(bytes)
    }

    fn write_game_file(&self, target_path: &InstallTargetPath, bytes: &[u8]) -> anyhow::Result<()> {
        if self.fail_writes {
            anyhow::bail!("write failed");
        }
        self.files
            .lock()
            .expect("files")
            .insert(target_path.as_str().to_owned(), bytes.to_vec());
        Ok(())
    }

    fn remove_game_file(&self, target_path: &InstallTargetPath) -> anyhow::Result<()> {
        if self.fail_removes {
            anyhow::bail!("remove failed");
        }
        self.files
            .lock()
            .expect("files")
            .remove(target_path.as_str());
        Ok(())
    }
}

#[derive(Default)]
struct RecordingInstallBackupStore {
    records: Mutex<Vec<(String, String, Vec<u8>)>>,
    removed_refs: Mutex<Vec<String>>,
    fail_removals: bool,
}

impl RecordingInstallBackupStore {
    fn with_backups<const N: usize>(backups: [(&str, &[u8]); N]) -> Self {
        Self {
            records: Mutex::new(
                backups
                    .into_iter()
                    .map(|(backup_ref, bytes)| {
                        (
                            backup_ref.to_owned(),
                            "<preexisting>".to_owned(),
                            bytes.to_vec(),
                        )
                    })
                    .collect(),
            ),
            removed_refs: Mutex::new(Vec::new()),
            fail_removals: false,
        }
    }

    fn failing_removals() -> Self {
        Self {
            records: Mutex::new(Vec::new()),
            removed_refs: Mutex::new(Vec::new()),
            fail_removals: true,
        }
    }

    fn records(&self) -> Vec<(String, Vec<u8>)> {
        self.records
            .lock()
            .expect("records")
            .iter()
            .map(|(_, target_path, bytes)| (target_path.clone(), bytes.clone()))
            .collect()
    }

    fn removed_refs(&self) -> Vec<String> {
        self.removed_refs.lock().expect("removed refs").clone()
    }
}

impl InstallBackupStore for RecordingInstallBackupStore {
    fn store_backup(
        &self,
        target_path: &InstallTargetPath,
        bytes: &[u8],
    ) -> anyhow::Result<String> {
        let mut records = self.records.lock().expect("records");
        let base_ref = format!("backup-{}", target_path.as_str().replace('/', "-"));
        let backup_ref = if records.is_empty() {
            base_ref
        } else {
            format!("{base_ref}-{}", records.len())
        };
        records.push((
            backup_ref.clone(),
            target_path.as_str().to_owned(),
            bytes.to_vec(),
        ));
        Ok(backup_ref)
    }

    fn read_backup(&self, backup_ref: &str) -> anyhow::Result<Option<Vec<u8>>> {
        Ok(self
            .records
            .lock()
            .expect("records")
            .iter()
            .find(|(record_ref, _, _)| record_ref == backup_ref)
            .map(|(_, _, bytes)| bytes.clone()))
    }

    fn remove_backup(&self, backup_ref: &str) -> anyhow::Result<()> {
        self.removed_refs
            .lock()
            .expect("removed refs")
            .push(backup_ref.to_owned());
        if self.fail_removals {
            anyhow::bail!("backup cleanup failed");
        }
        Ok(())
    }
}

#[derive(Default)]
struct RecordingInstallManifestRepository {
    existing_manifest: Option<InstallManifest>,
    saved_manifest: Mutex<Option<InstallManifest>>,
    fail_load: bool,
    fail_save: bool,
}

impl RecordingInstallManifestRepository {
    fn failing_load() -> Self {
        Self {
            existing_manifest: None,
            saved_manifest: Mutex::new(None),
            fail_load: true,
            fail_save: false,
        }
    }

    fn failing() -> Self {
        Self {
            existing_manifest: None,
            saved_manifest: Mutex::new(None),
            fail_load: false,
            fail_save: true,
        }
    }

    fn with_existing_manifest(mut self, manifest: InstallManifest) -> Self {
        self.existing_manifest = Some(manifest);
        self
    }

    fn take_manifest(&self) -> Option<InstallManifest> {
        self.saved_manifest.lock().expect("manifest").take()
    }
}

impl InstallManifestRepository for RecordingInstallManifestRepository {
    fn load_manifest(&self, _profile_id: &ProfileId) -> anyhow::Result<Option<InstallManifest>> {
        if self.fail_load {
            anyhow::bail!("manifest load failed");
        }
        Ok(self.existing_manifest.clone())
    }

    fn save_manifest(&self, manifest: &InstallManifest) -> anyhow::Result<()> {
        if self.fail_save {
            anyhow::bail!("manifest save failed");
        }
        *self.saved_manifest.lock().expect("manifest") = Some(manifest.clone());
        Ok(())
    }
}

#[derive(Default)]
struct RecordingInstallRecoveryRecordRepository {
    saved_records: Mutex<Vec<InstallRecoveryRecord>>,
    removed_records: Mutex<Vec<(String, String)>>,
}

impl RecordingInstallRecoveryRecordRepository {
    fn saved_records(&self) -> Vec<InstallRecoveryRecord> {
        self.saved_records.lock().expect("saved records").clone()
    }

    fn removed_records(&self) -> Vec<(String, String)> {
        self.removed_records
            .lock()
            .expect("removed records")
            .clone()
    }
}

impl InstallRecoveryRecordRepository for RecordingInstallRecoveryRecordRepository {
    fn load_record(
        &self,
        _profile_id: &ProfileId,
        _mod_id: &ModId,
    ) -> anyhow::Result<Option<InstallRecoveryRecord>> {
        Ok(None)
    }

    fn list_records(&self, profile_id: &ProfileId) -> anyhow::Result<Vec<InstallRecoveryRecord>> {
        Ok(self
            .saved_records
            .lock()
            .expect("saved records")
            .iter()
            .filter(|record| record.profile_id == *profile_id)
            .cloned()
            .collect())
    }

    fn save_record(&self, record: &InstallRecoveryRecord) -> anyhow::Result<()> {
        self.saved_records
            .lock()
            .expect("saved records")
            .push(record.clone());
        Ok(())
    }

    fn remove_record(&self, profile_id: &ProfileId, mod_id: &ModId) -> anyhow::Result<()> {
        self.removed_records
            .lock()
            .expect("removed records")
            .push((profile_id.as_str().to_owned(), mod_id.as_str().to_owned()));
        Ok(())
    }
}
