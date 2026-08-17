use anyhow::Result;
use hmm_app::{CreateSaveBackupRequest, SaveBackupError, SaveBackupService, SaveBackupWarning};
use hmm_core::{
    GameId, Profile, ProfileBackupRetention, ProfileBackupSchedule, ProfileDirectoryMode,
    ProfileDirectorySelection, ProfileDirectoryStatus, ProfileId, ProfileSaveSettings,
    SaveBackupRetentionOutcome, SaveBackupRetentionReason, SaveBackupStatus, SaveBackupSummary,
    SaveBackupTrigger,
};
use hmm_ports::{
    AppClock, ProfileRepository, ProfileSaveDirectoryValidator, ProfileSaveSettingsRepository,
    SaveBackupDeleteReport, SaveBackupFileDeleteDisposition, SaveBackupFileDeleteResult,
    SaveBackupRepository, SaveBackupWriteRequest, SaveBackupWriteResult, SaveBackupWriter,
};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};

const DAY_MILLIS: u128 = 86_400_000;

#[test]
fn manual_backup_uses_default_backup_directory_when_custom_backup_root_is_unset() {
    let harness = Harness::new();
    harness.insert_profile("default");
    harness.insert_settings(ProfileSaveSettings {
        profile_id: "default".to_owned(),
        save_directory: custom_directory_selection("C:/Users/Test/Saves"),
        backup_directory: default_backup_directory_selection(),
        schedule: ProfileBackupSchedule::manual(),
        retention: ProfileBackupRetention {
            max_count: 3,
            max_age_days: None,
            max_total_bytes: None,
        },
        steam_account: None,
        pre_restore_backup_enabled: true,
        updated_at: 10,
    });

    let summary = harness
        .service
        .create_manual_backup(CreateSaveBackupRequest {
            game_id: GameId::mhw(),
            profile_id: ProfileId::new("default"),
            note: Some("after fatalis".to_owned()),
        })
        .expect("manual backup should use default backup directory")
        .summary;

    assert_eq!(summary.backup_id, "backup-1");
    assert_eq!(summary.trigger, SaveBackupTrigger::Manual);
    assert_eq!(summary.status, SaveBackupStatus::Completed);

    let writer_requests = harness.writer.take_requests();
    assert_eq!(writer_requests.len(), 1);
    assert_eq!(writer_requests[0].game_id.as_str(), "mhw");
    assert_eq!(writer_requests[0].profile_id.as_str(), "default");
    assert_eq!(
        writer_requests[0].source_directory.as_deref(),
        Some("C:/Users/Test/Saves")
    );
    assert_eq!(
        writer_requests[0].source_directory_selection.mode,
        ProfileDirectoryMode::Custom
    );
    assert_eq!(
        writer_requests[0].backup_directory.mode,
        ProfileDirectoryMode::Default
    );
    assert_eq!(writer_requests[0].note.as_deref(), Some("after fatalis"));
    assert_eq!(writer_requests[0].created_at_unix_millis, 42);

    let saved = harness.repository.take_saved();
    assert_eq!(saved.len(), 1);
    assert_eq!(saved[0].backup_id, "backup-1");
}

#[test]
fn auto_backup_passes_auto_trigger_to_writer_and_history() {
    let harness = Harness::new();
    harness.insert_profile("default");
    harness.insert_settings(ProfileSaveSettings {
        profile_id: "default".to_owned(),
        save_directory: custom_directory_selection("C:/Users/Test/Saves"),
        backup_directory: default_backup_directory_selection(),
        schedule: ProfileBackupSchedule {
            cadence: hmm_core::BackupCadence::Daily,
            hour: Some(3),
            minute: Some(0),
            weekdays: Vec::new(),
        },
        retention: ProfileBackupRetention::default(),
        steam_account: None,
        pre_restore_backup_enabled: true,
        updated_at: 10,
    });

    let summary = harness
        .service
        .create_backup(
            CreateSaveBackupRequest {
                game_id: GameId::mhw(),
                profile_id: ProfileId::new("default"),
                note: Some("client runtime auto check".to_owned()),
            },
            SaveBackupTrigger::Auto,
        )
        .expect("auto backup should reuse the save backup service")
        .summary;

    assert_eq!(summary.trigger, SaveBackupTrigger::Auto);

    let writer_requests = harness.writer.take_requests();
    assert_eq!(writer_requests.len(), 1);
    assert_eq!(writer_requests[0].trigger, SaveBackupTrigger::Auto);

    let saved = harness.repository.take_saved();
    assert_eq!(saved.len(), 1);
    assert_eq!(saved[0].trigger, SaveBackupTrigger::Auto);
}

#[test]
fn manual_backup_rejects_unset_save_directory_before_writer_runs() {
    let harness = Harness::new();
    harness.insert_profile("default");
    harness.insert_settings(ProfileSaveSettings {
        profile_id: "default".to_owned(),
        save_directory: unset_save_directory(),
        backup_directory: default_backup_directory_selection(),
        schedule: ProfileBackupSchedule::manual(),
        retention: ProfileBackupRetention::default(),
        steam_account: None,
        pre_restore_backup_enabled: true,
        updated_at: 10,
    });

    let error = harness
        .service
        .create_manual_backup(CreateSaveBackupRequest {
            game_id: GameId::mhw(),
            profile_id: ProfileId::new("default"),
            note: None,
        })
        .expect_err("unset source must be rejected");

    assert_eq!(error, SaveBackupError::SourceUnset);
    assert_eq!(error.code(), "save_backup_source_unset");
    assert!(harness.writer.take_requests().is_empty());
    assert!(harness.repository.take_saved().is_empty());
}

#[test]
fn manual_backup_prunes_old_completed_backups_for_same_profile_only() {
    let harness = Harness::new();
    harness.insert_profile("default");
    harness.insert_profile("alt");
    harness.insert_settings(ProfileSaveSettings {
        profile_id: "default".to_owned(),
        save_directory: custom_directory_selection("C:/Users/Test/Saves"),
        backup_directory: default_backup_directory_selection(),
        schedule: ProfileBackupSchedule::manual(),
        retention: ProfileBackupRetention {
            max_count: 1,
            max_age_days: None,
            max_total_bytes: None,
        },
        steam_account: None,
        pre_restore_backup_enabled: true,
        updated_at: 10,
    });
    harness
        .repository
        .save(&sample_summary("backup-old", "default", 1))
        .expect("old summary saved");
    harness
        .repository
        .save(&sample_summary("backup-other-profile", "alt", 1))
        .expect("other profile summary saved");

    let summary = harness
        .service
        .create_manual_backup(CreateSaveBackupRequest {
            game_id: GameId::mhw(),
            profile_id: ProfileId::new("default"),
            note: None,
        })
        .expect("manual backup should prune old backup")
        .summary;

    assert_eq!(summary.backup_id, "backup-1");
    assert_eq!(harness.writer.take_deleted_ids(), vec!["backup-old"]);

    let saved = harness.repository.take_saved();
    assert_eq!(
        saved
            .iter()
            .find(|item| item.backup_id == "backup-old")
            .expect("old backup retained as history")
            .status,
        SaveBackupStatus::DeletedByRetention
    );
    assert_eq!(
        saved
            .iter()
            .find(|item| item.backup_id == "backup-other-profile")
            .expect("other profile untouched")
            .status,
        SaveBackupStatus::Completed
    );
    assert!(saved.iter().any(|item| item.backup_id == "backup-1"));
}

#[test]
fn manual_backup_returns_created_summary_when_retention_delete_fails() {
    let harness = Harness::new();
    harness.insert_profile("default");
    harness.insert_settings(ProfileSaveSettings {
        profile_id: "default".to_owned(),
        save_directory: custom_directory_selection("C:/Users/Test/Saves"),
        backup_directory: default_backup_directory_selection(),
        schedule: ProfileBackupSchedule::manual(),
        retention: ProfileBackupRetention {
            max_count: 1,
            max_age_days: None,
            max_total_bytes: None,
        },
        steam_account: None,
        pre_restore_backup_enabled: true,
        updated_at: 10,
    });
    harness
        .repository
        .save(&sample_summary("backup-old", "default", 1))
        .expect("old summary saved");
    harness.writer.fail_delete_for("backup-old");

    let result = harness
        .service
        .create_manual_backup(CreateSaveBackupRequest {
            game_id: GameId::mhw(),
            profile_id: ProfileId::new("default"),
            note: None,
        })
        .expect("created backup should still be returned when retention fails");

    assert_eq!(result.summary.backup_id, "backup-1");
    assert_eq!(result.warnings, vec![SaveBackupWarning::RetentionPartial]);
    let report = result.retention_report.expect("partial report is retained");
    assert_eq!(report.outcome, SaveBackupRetentionOutcome::Partial);
    assert_eq!(report.partial_count, 1);
    assert_eq!(report.blocked_count, 1);
    assert_eq!(harness.writer.take_deleted_ids(), vec!["backup-old"]);

    let saved = harness.repository.take_saved();
    assert!(saved.iter().any(|item| item.backup_id == "backup-1"));
    assert_eq!(
        saved
            .iter()
            .find(|item| item.backup_id == "backup-old")
            .expect("old backup remains visible as retryable partial")
            .status,
        SaveBackupStatus::RetentionPartial
    );
}

#[test]
fn manual_backup_continues_retention_after_individual_delete_failure() {
    let harness = Harness::new();
    harness.insert_profile("default");
    harness.insert_settings(ProfileSaveSettings {
        profile_id: "default".to_owned(),
        save_directory: custom_directory_selection("C:/Users/Test/Saves"),
        backup_directory: default_backup_directory_selection(),
        schedule: ProfileBackupSchedule::manual(),
        retention: ProfileBackupRetention {
            max_count: 1,
            max_age_days: None,
            max_total_bytes: None,
        },
        steam_account: None,
        pre_restore_backup_enabled: true,
        updated_at: 10,
    });
    harness
        .repository
        .save(&sample_summary("backup-failing", "default", 3))
        .expect("failing summary saved");
    harness
        .repository
        .save(&sample_summary("backup-pruned", "default", 2))
        .expect("pruned summary saved");
    harness.writer.fail_delete_for("backup-failing");

    let result = harness
        .service
        .create_manual_backup(CreateSaveBackupRequest {
            game_id: GameId::mhw(),
            profile_id: ProfileId::new("default"),
            note: None,
        })
        .expect("retention failure should not fail created backup");

    assert_eq!(result.summary.backup_id, "backup-1");
    assert_eq!(result.warnings, vec![SaveBackupWarning::RetentionPartial]);
    assert_eq!(
        harness.writer.take_deleted_ids(),
        vec!["backup-pruned", "backup-failing"]
    );

    let saved = harness.repository.take_saved();
    assert_eq!(
        saved
            .iter()
            .find(|item| item.backup_id == "backup-failing")
            .expect("failed delete remains retryable")
            .status,
        SaveBackupStatus::RetentionPartial
    );
    assert_eq!(
        saved
            .iter()
            .find(|item| item.backup_id == "backup-pruned")
            .expect("later old backup still pruned")
            .status,
        SaveBackupStatus::DeletedByRetention
    );
}

#[test]
fn manual_backup_retention_uses_each_backup_original_directory() {
    let harness = Harness::new();
    harness.insert_profile("default");
    harness.insert_settings(ProfileSaveSettings {
        profile_id: "default".to_owned(),
        save_directory: custom_directory_selection("C:/Users/Test/Saves"),
        backup_directory: custom_directory_selection("D:/CurrentBackups"),
        schedule: ProfileBackupSchedule::manual(),
        retention: ProfileBackupRetention {
            max_count: 1,
            max_age_days: None,
            max_total_bytes: None,
        },
        steam_account: None,
        pre_restore_backup_enabled: true,
        updated_at: 10,
    });
    let mut old_summary = sample_summary("backup-old", "default", 1);
    old_summary.backup_directory = custom_directory_selection("E:/OriginalBackups");
    harness
        .repository
        .save(&old_summary)
        .expect("old summary saved");

    harness
        .service
        .create_manual_backup(CreateSaveBackupRequest {
            game_id: GameId::mhw(),
            profile_id: ProfileId::new("default"),
            note: None,
        })
        .expect("manual backup should prune using original directory");

    let deleted = harness.writer.take_deleted();
    assert_eq!(deleted.len(), 1);
    assert_eq!(deleted[0].0, "backup-old");
    assert_eq!(deleted[0].1.as_deref(), Some("E:/OriginalBackups"));
}

#[test]
fn ordinary_retention_does_not_delete_pre_restore_backups() {
    let harness = Harness::new();
    harness.insert_profile("default");
    harness.insert_settings(ProfileSaveSettings {
        profile_id: "default".to_owned(),
        save_directory: custom_directory_selection("C:/Users/Test/Saves"),
        backup_directory: default_backup_directory_selection(),
        schedule: ProfileBackupSchedule::manual(),
        retention: ProfileBackupRetention {
            max_count: 1,
            max_age_days: None,
            max_total_bytes: None,
        },
        steam_account: None,
        pre_restore_backup_enabled: true,
        updated_at: 10,
    });
    let mut pre_restore = sample_summary("backup-pre-restore", "default", 1);
    pre_restore.trigger = SaveBackupTrigger::PreRestore;
    harness
        .repository
        .save(&pre_restore)
        .expect("save pre-restore summary");

    harness
        .service
        .create_manual_backup(CreateSaveBackupRequest {
            game_id: GameId::mhw(),
            profile_id: ProfileId::new("default"),
            note: None,
        })
        .expect("manual backup succeeds");

    assert!(!harness
        .writer
        .take_deleted_ids()
        .contains(&"backup-pre-restore".to_owned()));
}

#[test]
fn pre_restore_backup_creation_does_not_prune_the_restore_candidate() {
    let harness = Harness::new();
    harness.insert_profile("default");
    harness.insert_settings(ProfileSaveSettings {
        profile_id: "default".to_owned(),
        save_directory: custom_directory_selection("C:/Users/Test/Saves"),
        backup_directory: default_backup_directory_selection(),
        schedule: ProfileBackupSchedule::manual(),
        retention: ProfileBackupRetention {
            max_count: 1,
            max_age_days: None,
            max_total_bytes: None,
        },
        steam_account: None,
        pre_restore_backup_enabled: true,
        updated_at: 10,
    });
    harness
        .repository
        .save(&sample_summary("backup-restore-source", "default", 1))
        .expect("save restore source");
    harness
        .repository
        .save(&sample_summary("backup-latest", "default", 2))
        .expect("save latest ordinary backup");

    let result = harness
        .service
        .create_backup(
            CreateSaveBackupRequest {
                game_id: GameId::mhw(),
                profile_id: ProfileId::new("default"),
                note: Some("before restore".to_owned()),
            },
            SaveBackupTrigger::PreRestore,
        )
        .expect("pre-restore protection point succeeds without ordinary retention");

    assert_eq!(result.summary.trigger, SaveBackupTrigger::PreRestore);
    assert!(result.warnings.is_empty());
    assert!(result.retention_report.is_none());
    assert!(harness.writer.take_deleted_ids().is_empty());
    let saved = harness.repository.take_saved();
    assert_eq!(
        saved
            .iter()
            .find(|summary| summary.backup_id == "backup-restore-source")
            .expect("restore source remains available")
            .status,
        SaveBackupStatus::Completed
    );
}

#[test]
fn retention_with_all_limits_unbounded_keeps_every_completed_backup() {
    let harness = Harness::new();
    harness.insert_profile("default");
    harness.insert_settings(ProfileSaveSettings {
        profile_id: "default".to_owned(),
        save_directory: custom_directory_selection("C:/Users/Test/Saves"),
        backup_directory: default_backup_directory_selection(),
        schedule: ProfileBackupSchedule::manual(),
        retention: ProfileBackupRetention::default(),
        steam_account: None,
        pre_restore_backup_enabled: true,
        updated_at: 10,
    });
    for (backup_id, created_at) in [
        ("backup-latest", 3),
        ("backup-middle", 2),
        ("backup-oldest", 1),
    ] {
        harness
            .repository
            .save(&sample_summary(backup_id, "default", created_at))
            .expect("save completed backup");
    }

    let report = harness
        .service
        .run_retention(&GameId::mhw(), &ProfileId::new("default"))
        .expect("unbounded retention should be a no-op");

    assert_eq!(report.outcome, SaveBackupRetentionOutcome::WithinPolicy);
    assert_eq!(report.scanned_count, 3);
    assert_eq!(report.candidate_count, 0);
    assert_eq!(report.deleted_count, 0);
    assert_eq!(report.archive_bytes_before, 384);
    assert_eq!(report.archive_bytes_after, 384);
    assert_eq!(report.released_bytes, 0);
    assert!(report.budget_satisfied);
    assert!(harness.writer.take_deleted_ids().is_empty());
    assert!(harness.repository.take_retention_reasons().is_empty());
    assert!(harness
        .repository
        .take_saved()
        .iter()
        .all(|summary| summary.status == SaveBackupStatus::Completed));
}

#[test]
fn retention_combines_count_age_and_space_candidates_oldest_first() {
    let harness = Harness::with_now(10 * DAY_MILLIS);
    harness.insert_profile("default");
    harness.insert_settings(ProfileSaveSettings {
        profile_id: "default".to_owned(),
        save_directory: custom_directory_selection("C:/Users/Test/Saves"),
        backup_directory: default_backup_directory_selection(),
        schedule: ProfileBackupSchedule::manual(),
        retention: ProfileBackupRetention {
            max_count: 3,
            max_age_days: Some(5),
            max_total_bytes: Some(150),
        },
        steam_account: None,
        pre_restore_backup_enabled: true,
        updated_at: 10,
    });
    for (backup_id, created_at) in [
        ("backup-latest", 9 * DAY_MILLIS),
        ("backup-space", 8 * DAY_MILLIS),
        ("backup-age", DAY_MILLIS),
        ("backup-oldest", 0),
    ] {
        let mut summary = sample_summary(backup_id, "default", created_at);
        summary.archive_size_bytes = 100;
        harness.repository.save(&summary).expect("save summary");
    }

    let report = harness
        .service
        .run_retention(&GameId::mhw(), &ProfileId::new("default"))
        .expect("combined retention succeeds");

    assert_eq!(report.outcome, SaveBackupRetentionOutcome::Completed);
    assert_eq!(report.candidate_count, 3);
    assert_eq!(report.deleted_count, 3);
    assert_eq!(report.partial_count, 0);
    assert_eq!(report.blocked_count, 0);
    assert_eq!(report.archive_bytes_before, 400);
    assert_eq!(report.archive_bytes_after, 100);
    assert_eq!(report.released_bytes, 300);
    assert!(report.budget_satisfied);
    assert_eq!(
        harness.writer.take_deleted_ids(),
        vec!["backup-oldest", "backup-age", "backup-space"]
    );

    let reasons = harness
        .repository
        .take_retention_reasons()
        .into_iter()
        .collect::<BTreeMap<_, _>>();
    assert_eq!(
        reasons["backup-oldest"],
        vec![
            SaveBackupRetentionReason::Count,
            SaveBackupRetentionReason::Age
        ]
    );
    assert_eq!(reasons["backup-age"], vec![SaveBackupRetentionReason::Age]);
    assert_eq!(
        reasons["backup-space"],
        vec![SaveBackupRetentionReason::Space]
    );

    let saved = harness.repository.take_saved();
    assert_eq!(
        saved
            .iter()
            .find(|summary| summary.backup_id == "backup-latest")
            .expect("latest summary")
            .status,
        SaveBackupStatus::Completed
    );
}

#[test]
fn space_retention_uses_remaining_bytes_when_selecting_additional_candidates() {
    let harness = Harness::new();
    harness.insert_profile("default");
    harness.insert_settings(ProfileSaveSettings {
        profile_id: "default".to_owned(),
        save_directory: custom_directory_selection("C:/Users/Test/Saves"),
        backup_directory: default_backup_directory_selection(),
        schedule: ProfileBackupSchedule::manual(),
        retention: ProfileBackupRetention {
            max_count: 10,
            max_age_days: None,
            max_total_bytes: Some(150),
        },
        steam_account: None,
        pre_restore_backup_enabled: true,
        updated_at: 10,
    });

    let mut latest = sample_summary("backup-latest", "default", 3);
    latest.archive_size_bytes = 100;
    harness.repository.save(&latest).expect("save latest");

    let mut additional = sample_summary("backup-additional", "default", 2);
    additional.archive_size_bytes = 100;
    harness
        .repository
        .save(&additional)
        .expect("save additional candidate");

    let mut partially_released = sample_summary("backup-partially-released", "default", 1);
    partially_released.archive_size_bytes = 100;
    partially_released.retention_released_bytes = 80;
    harness
        .repository
        .save(&partially_released)
        .expect("save partially released history");
    harness
        .writer
        .report_delete_for("backup-partially-released", successful_delete_report(20));

    let report = harness
        .service
        .run_retention(&GameId::mhw(), &ProfileId::new("default"))
        .expect("space retention should select enough candidates for the remaining bytes");

    assert_eq!(report.outcome, SaveBackupRetentionOutcome::Completed);
    assert_eq!(report.candidate_count, 2);
    assert_eq!(report.deleted_count, 2);
    assert_eq!(report.archive_bytes_before, 220);
    assert_eq!(report.archive_bytes_after, 100);
    assert_eq!(report.released_bytes, 120);
    assert!(report.budget_satisfied);
    assert_eq!(
        harness.writer.take_deleted_ids(),
        vec!["backup-partially-released", "backup-additional"]
    );
}

#[test]
fn retention_reports_blocked_when_latest_and_pre_restore_exceed_space_budget() {
    let harness = Harness::new();
    harness.insert_profile("default");
    harness.insert_settings(ProfileSaveSettings {
        profile_id: "default".to_owned(),
        save_directory: custom_directory_selection("C:/Users/Test/Saves"),
        backup_directory: default_backup_directory_selection(),
        schedule: ProfileBackupSchedule::manual(),
        retention: ProfileBackupRetention {
            max_count: 10,
            max_age_days: None,
            max_total_bytes: Some(100),
        },
        steam_account: None,
        pre_restore_backup_enabled: true,
        updated_at: 10,
    });
    let mut latest = sample_summary("backup-latest", "default", 2);
    latest.archive_size_bytes = 100;
    harness.repository.save(&latest).expect("save latest");
    let mut protected = sample_summary("backup-pre-restore", "default", 1);
    protected.trigger = SaveBackupTrigger::PreRestore;
    protected.archive_size_bytes = 300;
    harness.repository.save(&protected).expect("save protected");

    let report = harness
        .service
        .run_retention(&GameId::mhw(), &ProfileId::new("default"))
        .expect("blocked retention is a structured result");

    assert_eq!(report.outcome, SaveBackupRetentionOutcome::Blocked);
    assert_eq!(report.protected_count, 1);
    assert_eq!(report.candidate_count, 0);
    assert_eq!(report.blocked_count, 1);
    assert_eq!(report.archive_bytes_after, 400);
    assert!(!report.budget_satisfied);
    assert!(harness.writer.take_deleted_ids().is_empty());
}

#[test]
fn retention_does_not_delete_files_when_intent_persistence_fails() {
    let harness = Harness::new();
    harness.insert_profile("default");
    harness.insert_settings(ProfileSaveSettings {
        profile_id: "default".to_owned(),
        save_directory: custom_directory_selection("C:/Users/Test/Saves"),
        backup_directory: default_backup_directory_selection(),
        schedule: ProfileBackupSchedule::manual(),
        retention: ProfileBackupRetention {
            max_count: 1,
            max_age_days: None,
            max_total_bytes: None,
        },
        steam_account: None,
        pre_restore_backup_enabled: true,
        updated_at: 10,
    });
    harness
        .repository
        .save(&sample_summary("backup-old", "default", 1))
        .expect("save old backup");
    harness.repository.fail_begin_for("backup-old");

    let result = harness
        .service
        .create_manual_backup(CreateSaveBackupRequest {
            game_id: GameId::mhw(),
            profile_id: ProfileId::new("default"),
            note: None,
        })
        .expect("durable new backup survives retention intent failure");

    assert_eq!(result.warnings, vec![SaveBackupWarning::RetentionFailed]);
    assert_eq!(result.retention_report, None);
    assert!(harness.writer.take_deleted_ids().is_empty());
    let saved = harness.repository.take_saved();
    assert_eq!(
        saved
            .iter()
            .find(|summary| summary.backup_id == "backup-old")
            .expect("old backup remains")
            .status,
        SaveBackupStatus::Completed
    );
}

#[test]
fn retention_retry_converges_when_final_status_write_failed_after_file_deletion() {
    let harness = Harness::new();
    harness.insert_profile("default");
    harness.insert_settings(ProfileSaveSettings {
        profile_id: "default".to_owned(),
        save_directory: custom_directory_selection("C:/Users/Test/Saves"),
        backup_directory: default_backup_directory_selection(),
        schedule: ProfileBackupSchedule::manual(),
        retention: ProfileBackupRetention {
            max_count: 1,
            max_age_days: None,
            max_total_bytes: None,
        },
        steam_account: None,
        pre_restore_backup_enabled: true,
        updated_at: 10,
    });
    harness
        .repository
        .save(&sample_summary("backup-latest", "default", 2))
        .expect("save latest");
    harness
        .repository
        .save(&sample_summary("backup-old", "default", 1))
        .expect("save old");
    harness.repository.fail_finish_once_for("backup-old");

    let error = harness
        .service
        .run_retention(&GameId::mhw(), &ProfileId::new("default"))
        .expect_err("final status failure must remain a retryable retention failure");
    assert_eq!(error, SaveBackupError::RetentionFailed);
    assert_eq!(harness.writer.take_deleted_ids(), vec!["backup-old"]);
    assert_eq!(
        harness
            .repository
            .saved_snapshot()
            .into_iter()
            .find(|summary| summary.backup_id == "backup-old")
            .expect("old backup remains in history")
            .status,
        SaveBackupStatus::RetentionPending
    );

    harness.writer.report_delete_for(
        "backup-old",
        SaveBackupDeleteReport {
            archive: SaveBackupFileDeleteResult {
                disposition: SaveBackupFileDeleteDisposition::AlreadyMissing,
                released_bytes: 0,
                error_code: None,
            },
            manifest: SaveBackupFileDeleteResult {
                disposition: SaveBackupFileDeleteDisposition::AlreadyMissing,
                released_bytes: 0,
                error_code: None,
            },
        },
    );
    let retry = harness
        .service
        .run_retention(&GameId::mhw(), &ProfileId::new("default"))
        .expect("missing files should converge the pending record on retry");

    assert_eq!(retry.outcome, SaveBackupRetentionOutcome::Completed);
    assert_eq!(retry.candidate_count, 1);
    assert_eq!(retry.deleted_count, 1);
    assert_eq!(retry.released_bytes, 0);
    assert_eq!(retry.archive_bytes_after, 128);
    let old = harness
        .repository
        .saved_snapshot()
        .into_iter()
        .find(|summary| summary.backup_id == "backup-old")
        .expect("old backup history remains");
    assert_eq!(old.status, SaveBackupStatus::DeletedByRetention);
    assert_eq!(old.retention_released_bytes, 0);
}

#[test]
fn partial_archive_delete_retries_without_double_counting_released_bytes() {
    let harness = Harness::new();
    harness.insert_profile("default");
    harness.insert_settings(ProfileSaveSettings {
        profile_id: "default".to_owned(),
        save_directory: custom_directory_selection("C:/Users/Test/Saves"),
        backup_directory: default_backup_directory_selection(),
        schedule: ProfileBackupSchedule::manual(),
        retention: ProfileBackupRetention {
            max_count: 1,
            max_age_days: None,
            max_total_bytes: None,
        },
        steam_account: None,
        pre_restore_backup_enabled: true,
        updated_at: 10,
    });
    harness
        .repository
        .save(&sample_summary("backup-latest", "default", 2))
        .expect("save latest");
    harness
        .repository
        .save(&sample_summary("backup-old", "default", 1))
        .expect("save old");
    harness.writer.report_delete_for(
        "backup-old",
        SaveBackupDeleteReport {
            archive: SaveBackupFileDeleteResult {
                disposition: SaveBackupFileDeleteDisposition::Deleted,
                released_bytes: 128,
                error_code: None,
            },
            manifest: SaveBackupFileDeleteResult {
                disposition: SaveBackupFileDeleteDisposition::Blocked,
                released_bytes: 0,
                error_code: Some("save_backup_retention_delete_failed".to_owned()),
            },
        },
    );

    let first = harness
        .service
        .run_retention(&GameId::mhw(), &ProfileId::new("default"))
        .expect("partial delete is reported");
    assert_eq!(first.outcome, SaveBackupRetentionOutcome::Partial);
    assert_eq!(first.archive_bytes_before, 256);
    assert_eq!(first.archive_bytes_after, 128);
    assert_eq!(first.released_bytes, 128);

    harness.writer.report_delete_for(
        "backup-old",
        SaveBackupDeleteReport {
            archive: SaveBackupFileDeleteResult {
                disposition: SaveBackupFileDeleteDisposition::AlreadyMissing,
                released_bytes: 0,
                error_code: None,
            },
            manifest: SaveBackupFileDeleteResult {
                disposition: SaveBackupFileDeleteDisposition::Deleted,
                released_bytes: 0,
                error_code: None,
            },
        },
    );
    let retry = harness
        .service
        .run_retention(&GameId::mhw(), &ProfileId::new("default"))
        .expect("partial delete converges on retry");
    assert_eq!(retry.outcome, SaveBackupRetentionOutcome::Completed);
    assert_eq!(retry.archive_bytes_before, 128);
    assert_eq!(retry.archive_bytes_after, 128);
    assert_eq!(retry.released_bytes, 0);

    let saved = harness.repository.take_saved();
    let old = saved
        .iter()
        .find(|summary| summary.backup_id == "backup-old")
        .expect("old backup history remains");
    assert_eq!(old.status, SaveBackupStatus::DeletedByRetention);
    assert_eq!(old.retention_released_bytes, 128);
}

struct Harness {
    service: SaveBackupService,
    profile_repository: Arc<FakeProfileRepository>,
    settings_repository: Arc<FakeProfileSaveSettingsRepository>,
    repository: Arc<FakeSaveBackupRepository>,
    writer: Arc<FakeSaveBackupWriter>,
}

impl Harness {
    fn new() -> Self {
        Self::with_now(42)
    }

    fn with_now(now_unix_millis: u128) -> Self {
        let profile_repository = Arc::new(FakeProfileRepository::default());
        let settings_repository = Arc::new(FakeProfileSaveSettingsRepository::default());
        let validator = Arc::new(FakeProfileSaveDirectoryValidator);
        let repository = Arc::new(FakeSaveBackupRepository::default());
        let writer = Arc::new(FakeSaveBackupWriter::default());
        let service = SaveBackupService::new(
            profile_repository.clone(),
            settings_repository.clone(),
            validator,
            repository.clone(),
            writer.clone(),
            Arc::new(FixedClock(now_unix_millis)),
        );

        Self {
            service,
            profile_repository,
            settings_repository,
            repository,
            writer,
        }
    }

    fn insert_profile(&self, profile_id: &str) {
        self.profile_repository
            .save(&Profile {
                id: profile_id.to_owned(),
                name: "Profile".to_owned(),
                description: None,
                is_active: profile_id == "default",
                created_at: 1,
                updated_at: 1,
            })
            .expect("profile saved");
    }

    fn insert_settings(&self, settings: ProfileSaveSettings) {
        self.settings_repository
            .save_settings(&settings)
            .expect("settings saved");
    }
}

#[derive(Default)]
struct FakeProfileRepository {
    profiles: Mutex<Vec<Profile>>,
}

impl ProfileRepository for FakeProfileRepository {
    fn get(&self, profile_id: &str) -> Result<Option<Profile>> {
        Ok(self
            .profiles
            .lock()
            .unwrap()
            .iter()
            .find(|profile| profile.id == profile_id)
            .cloned())
    }

    fn save(&self, profile: &Profile) -> Result<()> {
        let mut profiles = self.profiles.lock().unwrap();
        profiles.retain(|existing| existing.id != profile.id);
        profiles.push(profile.clone());
        Ok(())
    }

    fn delete(&self, profile_id: &str) -> Result<()> {
        self.profiles
            .lock()
            .unwrap()
            .retain(|profile| profile.id != profile_id);
        Ok(())
    }

    fn list_all(&self) -> Result<Vec<Profile>> {
        Ok(self.profiles.lock().unwrap().clone())
    }

    fn get_active(&self) -> Result<Option<Profile>> {
        Ok(self
            .profiles
            .lock()
            .unwrap()
            .iter()
            .find(|profile| profile.is_active)
            .cloned())
    }

    fn set_active(&self, profile_id: &str, updated_at: u128) -> Result<()> {
        let mut profiles = self.profiles.lock().unwrap();
        for profile in profiles.iter_mut() {
            profile.is_active = profile.id == profile_id;
            if profile.is_active {
                profile.updated_at = updated_at;
            }
        }
        Ok(())
    }
}

#[derive(Default)]
struct FakeProfileSaveSettingsRepository {
    settings: Mutex<Vec<ProfileSaveSettings>>,
}

impl ProfileSaveSettingsRepository for FakeProfileSaveSettingsRepository {
    fn get_settings(&self, profile_id: &str) -> Result<Option<ProfileSaveSettings>> {
        Ok(self
            .settings
            .lock()
            .unwrap()
            .iter()
            .find(|settings| settings.profile_id == profile_id)
            .cloned())
    }

    fn save_settings(&self, settings: &ProfileSaveSettings) -> Result<()> {
        let mut all_settings = self.settings.lock().unwrap();
        all_settings.retain(|existing| existing.profile_id != settings.profile_id);
        all_settings.push(settings.clone());
        Ok(())
    }
}

struct FakeProfileSaveDirectoryValidator;

impl ProfileSaveDirectoryValidator for FakeProfileSaveDirectoryValidator {
    fn validate_save_directory(
        &self,
        _game_id: &str,
        directory: &str,
    ) -> Result<ProfileDirectorySelection> {
        Ok(custom_directory_selection(directory))
    }

    fn validate_backup_directory(
        &self,
        _game_id: &str,
        directory: &str,
    ) -> Result<ProfileDirectorySelection> {
        Ok(custom_directory_selection(directory))
    }

    fn default_backup_directory(&self, _game_id: &str) -> Result<ProfileDirectorySelection> {
        Ok(default_backup_directory_selection())
    }
}

#[derive(Default)]
struct FakeSaveBackupRepository {
    saved: Mutex<Vec<SaveBackupSummary>>,
    begin_failures: Mutex<BTreeSet<String>>,
    finish_failures: Mutex<BTreeSet<String>>,
    retention_reasons: Mutex<Vec<(String, Vec<SaveBackupRetentionReason>)>>,
}

impl FakeSaveBackupRepository {
    fn take_saved(&self) -> Vec<SaveBackupSummary> {
        std::mem::take(&mut *self.saved.lock().unwrap())
    }

    fn saved_snapshot(&self) -> Vec<SaveBackupSummary> {
        self.saved.lock().unwrap().clone()
    }

    fn fail_begin_for(&self, backup_id: &str) {
        self.begin_failures
            .lock()
            .unwrap()
            .insert(backup_id.to_owned());
    }

    fn fail_finish_once_for(&self, backup_id: &str) {
        self.finish_failures
            .lock()
            .unwrap()
            .insert(backup_id.to_owned());
    }

    fn take_retention_reasons(&self) -> Vec<(String, Vec<SaveBackupRetentionReason>)> {
        std::mem::take(&mut *self.retention_reasons.lock().unwrap())
    }
}

impl SaveBackupRepository for FakeSaveBackupRepository {
    fn save(&self, summary: &SaveBackupSummary) -> Result<()> {
        self.saved.lock().unwrap().push(summary.clone());
        Ok(())
    }

    fn list_for_profile(
        &self,
        game_id: &GameId,
        profile_id: &ProfileId,
        _limit: Option<usize>,
    ) -> Result<Vec<SaveBackupSummary>> {
        Ok(self
            .saved
            .lock()
            .unwrap()
            .iter()
            .filter(|summary| &summary.game_id == game_id && &summary.profile_id == profile_id)
            .cloned()
            .collect())
    }

    fn mark_status(&self, backup_id: &str, status: SaveBackupStatus) -> Result<()> {
        for summary in self.saved.lock().unwrap().iter_mut() {
            if summary.backup_id == backup_id {
                summary.status = status;
            }
        }
        Ok(())
    }

    fn begin_retention(
        &self,
        _game_id: &GameId,
        _profile_id: &ProfileId,
        backup_id: &str,
        reasons: &[SaveBackupRetentionReason],
        _attempted_at: u128,
    ) -> Result<bool> {
        self.retention_reasons
            .lock()
            .unwrap()
            .push((backup_id.to_owned(), reasons.to_vec()));
        if self.begin_failures.lock().unwrap().contains(backup_id) {
            anyhow::bail!("begin retention failed");
        }
        self.mark_status(backup_id, SaveBackupStatus::RetentionPending)?;
        Ok(true)
    }

    fn finish_retention(
        &self,
        _game_id: &GameId,
        _profile_id: &ProfileId,
        backup_id: &str,
        status: SaveBackupStatus,
        _error_code: Option<&str>,
        released_bytes: u64,
    ) -> Result<()> {
        if self.finish_failures.lock().unwrap().remove(backup_id) {
            anyhow::bail!("finish retention failed");
        }
        let mut summaries = self.saved.lock().unwrap();
        let summary = summaries
            .iter_mut()
            .find(|summary| summary.backup_id == backup_id)
            .expect("retention summary exists");
        assert_eq!(summary.status, SaveBackupStatus::RetentionPending);
        summary.status = status;
        summary.retention_released_bytes = summary
            .retention_released_bytes
            .saturating_add(released_bytes);
        Ok(())
    }
}

#[derive(Default)]
struct FakeSaveBackupWriter {
    requests: Mutex<Vec<SaveBackupWriteRequest>>,
    deleted: Mutex<Vec<(String, Option<String>)>>,
    delete_failures: Mutex<BTreeSet<String>>,
    delete_reports: Mutex<BTreeMap<String, SaveBackupDeleteReport>>,
}

impl FakeSaveBackupWriter {
    fn take_requests(&self) -> Vec<SaveBackupWriteRequest> {
        std::mem::take(&mut *self.requests.lock().unwrap())
    }

    fn take_deleted(&self) -> Vec<(String, Option<String>)> {
        std::mem::take(&mut *self.deleted.lock().unwrap())
    }

    fn take_deleted_ids(&self) -> Vec<String> {
        self.take_deleted()
            .into_iter()
            .map(|(backup_id, _)| backup_id)
            .collect()
    }

    fn fail_delete_for(&self, backup_id: &str) {
        self.delete_failures
            .lock()
            .unwrap()
            .insert(backup_id.to_owned());
    }

    fn report_delete_for(&self, backup_id: &str, report: SaveBackupDeleteReport) {
        self.delete_reports
            .lock()
            .unwrap()
            .insert(backup_id.to_owned(), report);
    }
}

impl SaveBackupWriter for FakeSaveBackupWriter {
    fn write_backup(&self, request: SaveBackupWriteRequest) -> Result<SaveBackupWriteResult> {
        self.requests.lock().unwrap().push(request.clone());
        Ok(SaveBackupWriteResult {
            summary: SaveBackupSummary {
                backup_id: "backup-1".to_owned(),
                game_id: request.game_id,
                profile_id: request.profile_id,
                trigger: request.trigger,
                status: SaveBackupStatus::Completed,
                archive_file_name: "20260704-221530_mhw_profile-default_manual.zip".to_owned(),
                manifest_file_name: "20260704-221530_mhw_profile-default_manual.manifest.json"
                    .to_owned(),
                archive_size_bytes: 128,
                retention_released_bytes: 0,
                archive_sha256: "sha256:test".to_owned(),
                file_count: 1,
                created_at: request.created_at_unix_millis,
                source_path_label: Some("Saves".to_owned()),
                source_path_hash: "sha256:source".to_owned(),
                backup_directory: request.backup_directory,
                notes: request.note,
            },
        })
    }

    fn delete_backup_files(
        &self,
        backup_directory: &ProfileDirectorySelection,
        summary: &SaveBackupSummary,
    ) -> Result<()> {
        self.deleted.lock().unwrap().push((
            summary.backup_id.clone(),
            backup_directory.directory.clone(),
        ));
        if self
            .delete_failures
            .lock()
            .unwrap()
            .contains(&summary.backup_id)
        {
            anyhow::bail!("delete failed");
        }
        Ok(())
    }

    fn delete_backup_files_report(
        &self,
        backup_directory: &ProfileDirectorySelection,
        summary: &SaveBackupSummary,
    ) -> Result<SaveBackupDeleteReport> {
        self.deleted.lock().unwrap().push((
            summary.backup_id.clone(),
            backup_directory.directory.clone(),
        ));
        if self
            .delete_failures
            .lock()
            .unwrap()
            .contains(&summary.backup_id)
        {
            anyhow::bail!("delete failed");
        }
        if let Some(report) = self
            .delete_reports
            .lock()
            .unwrap()
            .remove(&summary.backup_id)
        {
            return Ok(report);
        }
        Ok(successful_delete_report(summary.archive_size_bytes))
    }
}

struct FixedClock(u128);

impl AppClock for FixedClock {
    fn now_unix_millis(&self) -> Result<u128> {
        Ok(self.0)
    }
}

fn successful_delete_report(archive_size_bytes: u64) -> SaveBackupDeleteReport {
    SaveBackupDeleteReport {
        archive: SaveBackupFileDeleteResult {
            disposition: SaveBackupFileDeleteDisposition::Deleted,
            released_bytes: archive_size_bytes,
            error_code: None,
        },
        manifest: SaveBackupFileDeleteResult {
            disposition: SaveBackupFileDeleteDisposition::Deleted,
            released_bytes: 0,
            error_code: None,
        },
    }
}

fn custom_directory_selection(directory: &str) -> ProfileDirectorySelection {
    ProfileDirectorySelection {
        mode: ProfileDirectoryMode::Custom,
        status: ProfileDirectoryStatus::Valid,
        directory: Some(directory.to_owned()),
        path_label: Some("Saves".to_owned()),
        messages: Vec::new(),
    }
}

fn default_backup_directory_selection() -> ProfileDirectorySelection {
    ProfileDirectorySelection {
        mode: ProfileDirectoryMode::Default,
        status: ProfileDirectoryStatus::Defaulted,
        directory: None,
        path_label: Some("HelsincyModManager/backups/saves/mhw/profile-default".to_owned()),
        messages: vec!["使用默认备份目录".to_owned()],
    }
}

fn sample_summary(backup_id: &str, profile_id: &str, created_at: u128) -> SaveBackupSummary {
    SaveBackupSummary {
        backup_id: backup_id.to_owned(),
        game_id: GameId::mhw(),
        profile_id: ProfileId::new(profile_id),
        trigger: SaveBackupTrigger::Manual,
        status: SaveBackupStatus::Completed,
        archive_file_name: format!("{backup_id}.zip"),
        manifest_file_name: format!("{backup_id}.manifest.json"),
        archive_size_bytes: 128,
        retention_released_bytes: 0,
        archive_sha256: "sha256:test".to_owned(),
        file_count: 1,
        created_at,
        source_path_label: Some("Saves".to_owned()),
        source_path_hash: "sha256:source".to_owned(),
        backup_directory: default_backup_directory_selection(),
        notes: None,
    }
}

fn unset_save_directory() -> ProfileDirectorySelection {
    ProfileDirectorySelection {
        mode: ProfileDirectoryMode::Unset,
        status: ProfileDirectoryStatus::Unset,
        directory: None,
        path_label: None,
        messages: vec!["尚未选择游戏存档目录".to_owned()],
    }
}
