use super::*;

#[test]
fn uninstall_mod_removes_manifest_owned_new_file_when_summary_matches() {
    let target = InstallTargetPath::parse("nativePC/models/player.mod3", ["nativePC"])
        .expect("valid target");
    let existing_manifest = InstallManifest::completed(
        ProfileId::new("default"),
        vec![InstallManifestEntry {
            target_path: target,
            mod_id: ModId::new("mod-a"),
            revision_id: None,
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
            game_id: GameId::mhw(),
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
fn revision_bound_uninstall_rejects_manifest_drift_before_writing() {
    let target = InstallTargetPath::parse("nativePC/models/player.mod3", ["nativePC"])
        .expect("valid target");
    let mut existing_manifest = InstallManifest::completed(
        ProfileId::new("default"),
        vec![InstallManifestEntry {
            target_path: target,
            mod_id: ModId::new("mod-a"),
            revision_id: Some(ModRevisionId::new("revision-b")),
            package_file_id: PackageFileId::new("nativePC/models/player.mod3"),
            layer: FileLayer::new("base", 0),
            backup_ref: None,
            installed_file: Some(installed_file_summary(b"new model")),
        }],
    );
    existing_manifest.schema_version = INSTALL_MANIFEST_SCHEMA_VERSION_V2;
    let game_files = Arc::new(RecordingInstallGameFileSystem::with_files([(
        "nativePC/models/player.mod3",
        b"new model".as_slice(),
    )]));
    let manifests = Arc::new(
        RecordingInstallManifestRepository::default().with_existing_manifest(existing_manifest),
    );
    let service = UninstallModService::new(
        game_files.clone(),
        Arc::new(RecordingInstallBackupStore::default()),
        manifests.clone(),
    );

    let error = service
        .uninstall_mod_for_revision(
            UninstallModRequest {
                game_id: GameId::mhw(),
                profile_id: ProfileId::new("default"),
                mod_id: ModId::new("mod-a"),
            },
            ModRevisionId::new("revision-a"),
        )
        .expect_err("revision drift must fail closed");

    assert_eq!(error, UninstallModError::InstalledRevisionMismatch);
    assert_eq!(
        game_files
            .file_bytes("nativePC/models/player.mod3")
            .as_deref(),
        Some(b"new model".as_slice())
    );
    assert!(manifests.take_manifest().is_none());
}

#[test]
fn manifest_bound_uninstall_rejects_same_revision_binding_drift_before_writing() {
    let target = InstallTargetPath::parse("nativePC/models/player.mod3", ["nativePC"])
        .expect("valid target");
    let mut expected_manifest = InstallManifest::completed(
        ProfileId::new("default"),
        vec![InstallManifestEntry {
            target_path: target,
            mod_id: ModId::new("mod-a"),
            revision_id: Some(ModRevisionId::new("revision-a")),
            package_file_id: PackageFileId::new("nativePC/models/player.mod3"),
            layer: FileLayer::new("base", 0),
            backup_ref: None,
            installed_file: Some(installed_file_summary(b"new model")),
        }],
    );
    expected_manifest.schema_version = INSTALL_MANIFEST_SCHEMA_VERSION_V2;
    expected_manifest.replacement_bindings = vec![replacement_snapshot(
        "mod-a",
        "default",
        "binding-a",
        "mhw:armor:fatalis-alpha",
        Some("revision-a"),
    )];
    let expected_digest =
        uninstall_manifest_snapshot_digest(&expected_manifest, &ModId::new("mod-a"));
    let mut drifted_manifest = expected_manifest;
    drifted_manifest.replacement_bindings = vec![replacement_snapshot(
        "mod-a",
        "default",
        "binding-b",
        "mhw:armor:fatalis-beta",
        Some("revision-a"),
    )];
    let game_files = Arc::new(RecordingInstallGameFileSystem::with_files([(
        "nativePC/models/player.mod3",
        b"new model".as_slice(),
    )]));
    let manifests = Arc::new(
        RecordingInstallManifestRepository::default().with_existing_manifest(drifted_manifest),
    );
    let service = UninstallModService::new(
        game_files.clone(),
        Arc::new(RecordingInstallBackupStore::default()),
        manifests.clone(),
    );

    let error = service
        .uninstall_mod_for_revision_and_manifest(
            UninstallModRequest {
                game_id: GameId::mhw(),
                profile_id: ProfileId::new("default"),
                mod_id: ModId::new("mod-a"),
            },
            ModRevisionId::new("revision-a"),
            &expected_digest,
        )
        .expect_err("same-revision binding drift must fail closed");

    assert_eq!(error, UninstallModError::ManifestStateMismatch);
    assert_eq!(
        game_files
            .file_bytes("nativePC/models/player.mod3")
            .as_deref(),
        Some(b"new model".as_slice())
    );
    assert!(manifests.take_manifest().is_none());
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
            revision_id: None,
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
            game_id: GameId::mhw(),
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
    let keep_target =
        InstallTargetPath::parse("nativePC/models/keep.mod3", ["nativePC"]).expect("valid target");
    let mut existing_manifest = InstallManifest::completed_with_metadata(
        ProfileId::new("default"),
        vec![
            InstallManifestEntry {
                target_path: remove_target,
                mod_id: ModId::new("mod-a"),
                revision_id: None,
                package_file_id: PackageFileId::new("nativePC/models/remove.mod3"),
                layer: FileLayer::new("base", 0),
                backup_ref: None,
                installed_file: Some(installed_file_summary(b"remove model")),
            },
            InstallManifestEntry {
                target_path: keep_target,
                mod_id: ModId::new("mod-b"),
                revision_id: None,
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
    existing_manifest.manifest_id = "profile:custom-default".to_owned();
    existing_manifest.schema_version = 7;
    existing_manifest.schema_migration = Some("v1-to-v7".to_owned());
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
            game_id: GameId::mhw(),
            profile_id: ProfileId::new("default"),
            mod_id: ModId::new("mod-a"),
        })
        .expect("uninstall should preserve manifest metadata for remaining entries");

    let manifest = manifests.take_manifest().expect("manifest should be saved");
    assert_eq!(manifest.manifest_id, "profile:custom-default");
    assert_eq!(manifest.schema_version, 7);
    assert_eq!(manifest.schema_migration.as_deref(), Some("v1-to-v7"));
    assert_eq!(manifest.backend.as_deref(), Some("install_plan"));
    assert_eq!(manifest.created_at.as_deref(), Some("2026-06-29T00:00:00Z"));
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
fn uninstall_mod_preserves_non_completed_manifest_status_for_remaining_entries() {
    let remove_target = InstallTargetPath::parse("nativePC/models/remove.mod3", ["nativePC"])
        .expect("valid target");
    let keep_target =
        InstallTargetPath::parse("nativePC/models/keep.mod3", ["nativePC"]).expect("valid target");
    let mut existing_manifest = InstallManifest::completed_with_metadata(
        ProfileId::new("default"),
        vec![
            InstallManifestEntry {
                target_path: remove_target,
                mod_id: ModId::new("mod-a"),
                revision_id: None,
                package_file_id: PackageFileId::new("nativePC/models/remove.mod3"),
                layer: FileLayer::new("base", 0),
                backup_ref: None,
                installed_file: Some(installed_file_summary(b"remove model")),
            },
            InstallManifestEntry {
                target_path: keep_target,
                mod_id: ModId::new("mod-b"),
                revision_id: None,
                package_file_id: PackageFileId::new("nativePC/models/keep.mod3"),
                layer: FileLayer::new("base", 0),
                backup_ref: None,
                installed_file: Some(installed_file_summary(b"keep model")),
            },
        ],
        Some("install_plan".to_owned()),
        Some("2026-06-29T00:00:00Z".to_owned()),
        Some("2026-06-29T00:00:01Z".to_owned()),
        None,
    );
    existing_manifest.status = InstallManifestStatus::RepairRequired;
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
            game_id: GameId::mhw(),
            profile_id: ProfileId::new("default"),
            mod_id: ModId::new("mod-a"),
        })
        .expect("uninstall should preserve sticky manifest status");

    let manifest = manifests.take_manifest().expect("manifest should be saved");
    assert_eq!(manifest.status, InstallManifestStatus::RepairRequired);
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
            revision_id: None,
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
            game_id: GameId::mhw(),
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
            revision_id: None,
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
            game_id: GameId::mhw(),
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
            revision_id: None,
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
            game_id: GameId::mhw(),
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
            revision_id: None,
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
            game_id: GameId::mhw(),
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
            revision_id: None,
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
            game_id: GameId::mhw(),
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
            revision_id: None,
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
            game_id: GameId::mhw(),
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
fn uninstall_mod_removes_only_the_requested_mod_replacement_snapshot() {
    let mut existing_manifest = InstallManifest::completed(
        ProfileId::new("default"),
        vec![
            InstallManifestEntry {
                target_path: InstallTargetPath::parse("nativePC/mod-a.bin", ["nativePC"])
                    .expect("Mod A target"),
                mod_id: ModId::new("mod-a"),
                revision_id: None,
                package_file_id: PackageFileId::new("mod-a.bin"),
                layer: FileLayer::new("base", 0),
                backup_ref: None,
                installed_file: Some(installed_file_summary(b"mod-a")),
            },
            InstallManifestEntry {
                target_path: InstallTargetPath::parse("nativePC/mod-b.bin", ["nativePC"])
                    .expect("Mod B target"),
                mod_id: ModId::new("mod-b"),
                revision_id: None,
                package_file_id: PackageFileId::new("mod-b.bin"),
                layer: FileLayer::new("base", 0),
                backup_ref: None,
                installed_file: Some(installed_file_summary(b"mod-b")),
            },
        ],
    );
    let mod_b_snapshot = replacement_snapshot(
        "mod-b",
        "default",
        "binding-b",
        "mhw:armor:alatreon-alpha",
        None,
    );
    existing_manifest.replacement_bindings = vec![
        replacement_snapshot(
            "mod-a",
            "default",
            "binding-a",
            "mhw:armor:fatalis-alpha",
            None,
        ),
        mod_b_snapshot.clone(),
    ];
    let game_files = Arc::new(RecordingInstallGameFileSystem::with_files([
        ("nativePC/mod-a.bin", b"mod-a".as_slice()),
        ("nativePC/mod-b.bin", b"mod-b".as_slice()),
    ]));
    let manifests = Arc::new(
        RecordingInstallManifestRepository::default().with_existing_manifest(existing_manifest),
    );
    let service = UninstallModService::new(
        game_files,
        Arc::new(RecordingInstallBackupStore::default()),
        manifests.clone(),
    );

    service
        .uninstall_mod(UninstallModRequest {
            game_id: GameId::mhw(),
            profile_id: ProfileId::new("default"),
            mod_id: ModId::new("mod-a"),
        })
        .expect("uninstall Mod A");

    let saved = manifests.take_manifest().expect("saved manifest");
    assert_eq!(saved.replacement_bindings, vec![mod_b_snapshot]);
    assert!(saved
        .entries
        .iter()
        .all(|entry| entry.mod_id == ModId::new("mod-b")));
}
