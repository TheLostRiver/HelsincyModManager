use super::*;
use hmm_app::InstallManifestStatus;

#[test]
fn zero_profile_retention_limits_map_to_unbounded_domain_values() {
    let retention: hmm_core::ProfileBackupRetention = ProfileBackupRetentionDto {
        max_count: 0,
        max_age_days: Some(0),
        max_total_bytes: Some(0),
    }
    .into();

    assert_eq!(retention.max_count, 0);
    assert_eq!(retention.max_age_days, None);
    assert_eq!(retention.max_total_bytes, None);
}

#[cfg(test)]
mod app_settings_dto_tests {
    use super::*;

    #[test]
    fn serializes_app_settings_dto_with_camel_case_fields() {
        let dto: AppSettingsDto = AppSettings {
            thumbnail_cache_max_bytes: Some(128 * 1024 * 1024),
            thumbnail_cache_max_age_days: Some(14),
            log_storage_max_bytes: Some(64 * 1024 * 1024),
            debug_log_enabled: false,
        }
        .into();

        let value = serde_json::to_value(dto).expect("serialize settings");

        assert_eq!(value["thumbnailCacheMaxBytes"], 128 * 1024 * 1024);
        assert_eq!(value["thumbnailCacheMaxAgeDays"], 14);
    }

    #[test]
    fn maps_invalid_thumbnail_cache_setting_to_stable_error_code() {
        let error = CommandErrorDto::from_app_settings_service_error(
            AppSettingsServiceError::InvalidThumbnailCacheMaxBytes,
        );

        assert_eq!(error.code, "thumbnail_cache_max_bytes_invalid");
        assert!(!error.message.contains(':'));
        assert!(!error.message.contains('\\'));
    }

    #[test]
    fn maps_invalid_thumbnail_cache_age_setting_to_stable_error_code() {
        let error = CommandErrorDto::from_app_settings_service_error(
            AppSettingsServiceError::InvalidThumbnailCacheMaxAgeDays,
        );

        assert_eq!(error.code, "thumbnail_cache_max_age_days_invalid");
        assert!(!error.message.contains(':'));
        assert!(!error.message.contains('\\'));
    }

    #[test]
    fn serializes_log_storage_settings_with_a_narrow_camel_case_shape() {
        let dto: LogStorageSettingsDto = AppSettings {
            thumbnail_cache_max_bytes: Some(128 * 1024 * 1024),
            thumbnail_cache_max_age_days: Some(14),
            log_storage_max_bytes: Some(64 * 1024 * 1024),
            debug_log_enabled: false,
        }
        .into();

        let value = serde_json::to_value(dto).expect("serialize log storage settings");

        assert_eq!(value["maxBytes"], 64 * 1024 * 1024);
        assert_eq!(value.as_object().expect("settings object").len(), 1);
    }

    #[test]
    fn maps_invalid_log_storage_setting_to_stable_error_code() {
        let error = CommandErrorDto::from_app_settings_service_error(
            AppSettingsServiceError::InvalidLogStorageMaxBytes,
        );

        assert_eq!(error.code, "log_storage_max_bytes_invalid");
        assert!(!error.message.contains(':'));
        assert!(!error.message.contains('\\'));
    }
}

#[cfg(test)]
mod profile_dto_tests {
    use super::*;

    #[test]
    fn serializes_profile_dto_with_camel_case_fields() {
        let dto = ProfileDto {
            id: "default".to_owned(),
            name: "Default".to_owned(),
            description: Some("Base profile".to_owned()),
            is_active: true,
            created_at: 1000,
            updated_at: 2000,
        };

        let value = serde_json::to_value(dto).expect("serialize profile");

        assert_eq!(value["id"], "default");
        assert_eq!(value["name"], "Default");
        assert_eq!(value["description"], "Base profile");
        assert_eq!(value["isActive"], true);
        assert_eq!(value["createdAt"], 1000);
        assert_eq!(value["updatedAt"], 2000);
        assert!(value.get("is_active").is_none());
        assert!(value.get("created_at").is_none());
    }

    #[test]
    fn serializes_profile_save_settings_without_raw_storage_paths() {
        let dto: ProfileSaveSettingsDto = hmm_core::ProfileSaveSettings {
            profile_id: "default".to_owned(),
            save_directory: hmm_core::ProfileDirectorySelection {
                mode: hmm_core::ProfileDirectoryMode::Custom,
                status: hmm_core::ProfileDirectoryStatus::Valid,
                directory: Some("C:/Users/Test/Saves".to_owned()),
                path_label: Some("Saves".to_owned()),
                messages: Vec::new(),
            },
            backup_directory: hmm_core::ProfileDirectorySelection {
                mode: hmm_core::ProfileDirectoryMode::Default,
                status: hmm_core::ProfileDirectoryStatus::Defaulted,
                directory: None,
                path_label: Some("HelsincyModManager/Backups".to_owned()),
                messages: vec!["使用默认备份目录".to_owned()],
            },
            schedule: hmm_core::ProfileBackupSchedule {
                cadence: hmm_core::BackupCadence::Weekly,
                hour: Some(3),
                minute: Some(0),
                weekdays: vec![0],
            },
            retention: hmm_core::ProfileBackupRetention {
                max_count: 20,
                max_age_days: Some(30),
                max_total_bytes: None,
            },
            steam_account: None,
            pre_restore_backup_enabled: true,
            updated_at: 42,
        }
        .into();

        let value = serde_json::to_value(dto).expect("serialize profile save settings");

        assert_eq!(value["profileId"], "default");
        assert_eq!(value["saveDirectory"]["pathLabel"], "Saves");
        assert_eq!(value["backupDirectory"]["mode"], "default");
        assert_eq!(value["schedule"]["cadence"], "weekly");
        assert_eq!(value["schedule"]["weekdays"][0], 0);
        assert_eq!(value["retention"]["maxCount"], 20);
        assert_eq!(value["preRestoreBackupEnabled"], true);
        assert!(value.get("manifestPath").is_none());
        assert!(value.get("backupRoot").is_none());
        assert!(!value.to_string().contains("C:/Users/"));
    }
}

#[cfg(test)]
mod preview_image_tests {
    use super::*;

    #[test]
    fn serializes_thumbnail_dto_with_camel_case_fields() {
        let dto = PreviewImageDto::Thumbnail {
            thumbnail_url: "thumbnail://pkg/preview/hash".to_owned(),
            width: 512,
            height: 768,
            content_hash: "abc123".to_owned(),
        };

        let value = serde_json::to_value(dto).expect("serialize dto");

        assert_eq!(value["kind"], "thumbnail");
        assert_eq!(value["thumbnailUrl"], "thumbnail://pkg/preview/hash");
        assert_eq!(value["contentHash"], "abc123");
    }

    #[test]
    fn serializes_fallback_reason_as_snake_case() {
        let dto = PreviewImageDto::Fallback {
            reason: PreviewImageRejectionReason::PixelLimitExceeded.into(),
        };

        let value = serde_json::to_value(dto).expect("serialize dto");

        assert_eq!(value["kind"], "fallback");
        assert_eq!(value["reason"], "pixel_limit_exceeded");
    }

    #[test]
    fn maps_all_domain_fallback_reasons_to_dto() {
        let cases = [
            (PreviewImageRejectionReason::Missing, "missing"),
            (PreviewImageRejectionReason::TooLarge, "too_large"),
            (
                PreviewImageRejectionReason::TooManyCandidates,
                "too_many_candidates",
            ),
            (
                PreviewImageRejectionReason::UnsupportedFormat,
                "unsupported_format",
            ),
            (PreviewImageRejectionReason::DecodeFailed, "decode_failed"),
            (
                PreviewImageRejectionReason::PixelLimitExceeded,
                "pixel_limit_exceeded",
            ),
            (
                PreviewImageRejectionReason::CacheWriteFailed,
                "cache_write_failed",
            ),
        ];

        for (reason, expected) in cases {
            let dto_reason: PreviewImageFallbackReasonDto = reason.into();
            let value = serde_json::to_value(dto_reason).expect("serialize reason");
            assert_eq!(value, expected);
        }
    }

    #[test]
    fn maps_import_preview_thumbnail_to_dto() {
        let dto: PreviewImageDto = ImportPreviewImage::Thumbnail {
            thumbnail_url: "thumbnail://pkg-1/preview-768/hash".to_owned(),
            width: 320,
            height: 180,
            content_hash: "hash".to_owned(),
            variant: "preview-768".to_owned(),
        }
        .into();

        let value = serde_json::to_value(dto).expect("serialize dto");

        assert_eq!(value["kind"], "thumbnail");
        assert_eq!(value["thumbnailUrl"], "thumbnail://pkg-1/preview-768/hash");
        assert_eq!(value["width"], 320);
        assert_eq!(value["height"], 180);
        assert_eq!(value["contentHash"], "hash");
    }

    #[test]
    fn maps_import_preview_fallback_to_dto() {
        let dto: PreviewImageDto = ImportPreviewImage::Fallback {
            reason: PreviewImageRejectionReason::DecodeFailed,
        }
        .into();

        let value = serde_json::to_value(dto).expect("serialize dto");

        assert_eq!(value["kind"], "fallback");
        assert_eq!(value["reason"], "decode_failed");
    }

    #[test]
    fn serializes_preview_image_diagnostics_without_thumbnail_urls() {
        let dto: PreviewImageDiagnosticsDto = hmm_app::PreviewImageDiagnosticsSummary {
            total_imported_mods: 4,
            thumbnail_count: 1,
            fallback_count: 3,
            fallback_reasons: vec![hmm_app::PreviewImageFallbackDiagnostic {
                reason: PreviewImageRejectionReason::DecodeFailed,
                count: 2,
            }],
            export_categories: vec![
                hmm_app::PreviewImageDiagnosticExportCategory {
                    category: hmm_app::PreviewImageDiagnosticExportCategoryId::PreviewImageSummary,
                    status: hmm_app::PreviewImageDiagnosticExportCategoryStatus::Included,
                    reason: None,
                },
                hmm_app::PreviewImageDiagnosticExportCategory {
                    category: hmm_app::PreviewImageDiagnosticExportCategoryId::ThumbnailFiles,
                    status: hmm_app::PreviewImageDiagnosticExportCategoryStatus::Excluded,
                    reason: Some(
                        hmm_app::PreviewImageDiagnosticExportExclusionReason::DerivedImageContent,
                    ),
                },
            ],
        }
        .into();

        let value = serde_json::to_value(dto).expect("serialize diagnostics");

        assert_eq!(value["totalImportedMods"], 4);
        assert_eq!(value["thumbnailCount"], 1);
        assert_eq!(value["fallbackCount"], 3);
        assert_eq!(value["fallbackReasons"][0]["reason"], "decode_failed");
        assert_eq!(value["fallbackReasons"][0]["count"], 2);
        assert_eq!(
            value["exportCategories"][0]["category"],
            "preview_image_summary"
        );
        assert_eq!(value["exportCategories"][0]["status"], "included");
        assert!(value["exportCategories"][0].get("reason").is_none());
        assert_eq!(value["exportCategories"][1]["category"], "thumbnail_files");
        assert_eq!(value["exportCategories"][1]["status"], "excluded");
        assert_eq!(
            value["exportCategories"][1]["reason"],
            "derived_image_content"
        );
        assert!(value.get("thumbnailUrl").is_none());
        assert!(value.get("contentHash").is_none());
        assert!(value.get("path").is_none());
    }

    #[test]
    fn serializes_preview_image_diagnostics_export_without_paths_or_thumbnail_urls() {
        let dto: PreviewImageDiagnosticsExportDto = hmm_app::PreviewImageDiagnosticsExport {
            export_id: "preview-image-diagnostics-42.zip".to_owned(),
            file_name: "preview-image-diagnostics-42.zip".to_owned(),
            size_bytes: 1234,
            diagnostics: hmm_app::PreviewImageDiagnosticsSummary {
                total_imported_mods: 2,
                thumbnail_count: 1,
                fallback_count: 1,
                fallback_reasons: vec![hmm_app::PreviewImageFallbackDiagnostic {
                    reason: PreviewImageRejectionReason::DecodeFailed,
                    count: 1,
                }],
                export_categories: vec![hmm_app::PreviewImageDiagnosticExportCategory {
                    category: hmm_app::PreviewImageDiagnosticExportCategoryId::PreviewImageSummary,
                    status: hmm_app::PreviewImageDiagnosticExportCategoryStatus::Included,
                    reason: None,
                }],
            },
        }
        .into();

        let value = serde_json::to_value(dto).expect("serialize export");

        assert_eq!(value["exportId"], "preview-image-diagnostics-42.zip");
        assert_eq!(value["fileName"], "preview-image-diagnostics-42.zip");
        assert_eq!(value["sizeBytes"], 1234);
        assert_eq!(value["diagnostics"]["totalImportedMods"], 2);
        assert_eq!(value["diagnostics"]["thumbnailCount"], 1);
        assert!(!value.to_string().contains("thumbnailUrl"));
        assert!(!value.to_string().contains("contentHash"));
        assert!(!value.to_string().contains("thumbnail://"));
        assert!(!value.to_string().contains("C:/"));
        assert!(!value.to_string().contains("sandbox"));
    }

    #[test]
    fn serializes_audit_log_diagnostics_export_without_paths_or_raw_events() {
        let dto: AuditLogDiagnosticsExportDto = hmm_app::AuditLogDiagnosticsExport {
            export_id: "audit-log-diagnostics-42.zip".to_owned(),
            file_name: "audit-log-diagnostics-42.zip".to_owned(),
            size_bytes: 1234,
            audit_event_count: 2,
        }
        .into();

        let value = serde_json::to_value(dto).expect("serialize audit diagnostics export");

        assert_eq!(value["exportId"], "audit-log-diagnostics-42.zip");
        assert_eq!(value["fileName"], "audit-log-diagnostics-42.zip");
        assert_eq!(value["sizeBytes"], 1234);
        assert_eq!(value["auditEventCount"], 2);
        assert!(value.get("events").is_none());
        assert!(!value.to_string().contains("thumbnail://"));
        assert!(!value.to_string().contains("contentHash"));
        assert!(!value.to_string().contains("raw_path"));
        assert!(!value.to_string().contains("C:/"));
        assert!(!value.to_string().contains("sandbox"));
    }

    #[test]
    fn serializes_support_diagnostics_export_without_paths_or_raw_logs() {
        let dto: SupportDiagnosticsExportDto = hmm_app::SupportDiagnosticsExport {
            export_id: "support-diagnostics-42.zip".to_owned(),
            file_name: "support-diagnostics-42.zip".to_owned(),
            size_bytes: 4096,
            app_log_line_count: 2,
            debug_log_line_count: 3,
            task_log_line_count: 3,
            audit_event_count: 4,
            evidence_health: hmm_ports::DiagnosticsEvidenceHealthSnapshot {
                debug_log_status: "debug_log_write_failed".to_owned(),
                task_log_status: "task_log_write_failed".to_owned(),
                audit_log_status: "audit_write_failed_after_commit".to_owned(),
                log_storage_status: "log_storage_budget_unsatisfied".to_owned(),
                debug_log_event_rejected_count: 8,
                debug_log_write_failure_count: 9,
                debug_log_retention_failure_count: 10,
                task_log_write_failure_count: 1,
                task_log_retention_failure_count: 3,
                audit_write_failure_count: 2,
                audit_write_failure_after_commit_count: 1,
                audit_log_retention_failure_count: 4,
                log_storage_failure_count: 5,
                log_storage_unsatisfied_count: 6,
                log_storage_settings_failure_count: 7,
            },
        }
        .into();

        let value = serde_json::to_value(dto).expect("serialize support diagnostics export");

        assert_eq!(value["exportId"], "support-diagnostics-42.zip");
        assert_eq!(value["fileName"], "support-diagnostics-42.zip");
        assert_eq!(value["sizeBytes"], 4096);
        assert_eq!(value["appLogLineCount"], 2);
        assert_eq!(value["debugLogLineCount"], 3);
        assert_eq!(value["taskLogLineCount"], 3);
        assert_eq!(value["auditEventCount"], 4);
        assert_eq!(value["taskLogStatus"], "task_log_write_failed");
        assert_eq!(value["debugLogStatus"], "debug_log_write_failed");
        assert_eq!(value["auditLogStatus"], "audit_write_failed_after_commit");
        assert_eq!(value["taskLogWriteFailureCount"], 1);
        assert_eq!(value["debugLogEventRejectedCount"], 8);
        assert_eq!(value["debugLogWriteFailureCount"], 9);
        assert_eq!(value["debugLogRetentionFailureCount"], 10);
        assert_eq!(value["taskLogRetentionFailureCount"], 3);
        assert_eq!(value["auditWriteFailureCount"], 2);
        assert_eq!(value["auditWriteFailureAfterCommitCount"], 1);
        assert_eq!(value["auditLogRetentionFailureCount"], 4);
        assert_eq!(value["logStorageStatus"], "log_storage_budget_unsatisfied");
        assert_eq!(value["logStorageFailureCount"], 5);
        assert_eq!(value["logStorageUnsatisfiedCount"], 6);
        assert_eq!(value["logStorageSettingsFailureCount"], 7);
        assert!(value.get("appLogLines").is_none());
        assert!(value.get("taskLogLines").is_none());
        assert!(value.get("events").is_none());
        assert!(value.get("path").is_none());
        assert!(!value.to_string().contains("thumbnail://"));
        assert!(!value.to_string().contains("contentHash"));
        assert!(!value.to_string().contains("raw_path"));
        assert!(!value.to_string().contains("C:/"));
        assert!(!value.to_string().contains("sandbox"));
    }

    #[test]
    fn serializes_preview_image_candidate_list_without_paths_or_urls() {
        let dto: PreviewImageCandidateListDto = hmm_app::PreviewImageCandidateList {
            mod_id: "mod-1".to_owned(),
            candidates: vec![hmm_app::PreviewImageCandidateSummary {
                candidate_index: 0,
                file_name: "preview.png".to_owned(),
                compressed_size_bytes: 1234,
            }],
        }
        .into();

        let value = serde_json::to_value(dto).expect("serialize candidate list");

        assert_eq!(value["modId"], "mod-1");
        assert_eq!(value["candidates"][0]["candidateIndex"], 0);
        assert_eq!(value["candidates"][0]["fileName"], "preview.png");
        assert_eq!(value["candidates"][0]["compressedSizeBytes"], 1234);
        assert!(value["candidates"][0].get("logicalPath").is_none());
        assert!(value["candidates"][0].get("thumbnailUrl").is_none());
        assert!(value["candidates"][0].get("path").is_none());
    }

    #[test]
    fn serializes_mod_library_item_with_preview_image() {
        let dto: ModLibraryItemDto = hmm_app::ModLibraryItem {
            id: "pkg-1".to_owned(),
            name: "pkg-1".to_owned(),
            author: Some("A Hunter".to_owned()),
            version_label: Some("v1.2.3".to_owned()),
            size_label: "导入完成".to_owned(),
            status: hmm_app::ModLibraryStatus::Disabled,
            category_labels: Vec::new(),
            preview_image: ImportPreviewImage::Thumbnail {
                thumbnail_url: "thumbnail://pkg-1/preview/hash".to_owned(),
                width: 320,
                height: 180,
                content_hash: "hash".to_owned(),
                variant: "preview".to_owned(),
            },
        }
        .into();

        let value = serde_json::to_value(dto).expect("serialize dto");

        assert_eq!(value["id"], "pkg-1");
        assert_eq!(value["name"], "pkg-1");
        assert_eq!(value["author"], "A Hunter");
        assert_eq!(value["versionLabel"], "v1.2.3");
        assert_eq!(value["sizeLabel"], "导入完成");
        assert_eq!(value["status"], "disabled");
        assert!(value.get("installSummary").is_none());
        assert_eq!(value["previewImage"]["kind"], "thumbnail");
        assert_eq!(
            value["previewImage"]["thumbnailUrl"],
            "thumbnail://pkg-1/preview/hash"
        );
    }

    #[test]
    fn serializes_mod_detail_with_preview_image() {
        let dto: ModDetailDto = hmm_app::ModDetail {
            id: "pkg-1".to_owned(),
            name: "pkg-1".to_owned(),
            package_id: "pkg-1".to_owned(),
            metadata: hmm_app::ModPackageMetadataSummary {
                version: Some("1.2.3".to_owned()),
                author: Some("A Hunter".to_owned()),
                category: Some("Visual".to_owned()),
                tags: vec!["armor".to_owned(), "hd".to_owned()],
                dependencies: vec!["stracker-loader".to_owned()],
            },
            description: None,
            nexus_mod_id: None,
            preview_image: ImportPreviewImage::Fallback {
                reason: PreviewImageRejectionReason::Missing,
            },
        }
        .into();

        let value = serde_json::to_value(dto).expect("serialize dto");

        assert_eq!(value["id"], "pkg-1");
        assert_eq!(value["packageId"], "pkg-1");
        assert_eq!(value["metadata"]["version"], "1.2.3");
        assert_eq!(value["metadata"]["author"], "A Hunter");
        assert_eq!(value["metadata"]["category"], "Visual");
        assert_eq!(value["metadata"]["tags"][0], "armor");
        assert_eq!(value["metadata"]["dependencies"][0], "stracker-loader");
        assert_eq!(value["previewImage"]["kind"], "fallback");
        assert_eq!(value["previewImage"]["reason"], "missing");
    }

    #[test]
    fn serializes_mod_dependency_graph_without_install_status_or_paths() {
        let dto: ModDependencyGraphDto = hmm_app::ModDependencyGraph {
            nodes: vec![hmm_app::ModDependencyGraphNode {
                mod_id: "armor-pack".to_owned(),
                name: "Armor Pack".to_owned(),
            }],
            edges: vec![hmm_app::ModDependencyGraphEdge {
                source_mod_id: "armor-pack".to_owned(),
                dependency: "stracker-loader".to_owned(),
                matched_imported_mod_id: Some("stracker-loader".to_owned()),
            }],
        }
        .into();

        let value = serde_json::to_value(dto).expect("serialize dto");

        assert_eq!(value["nodes"][0]["modId"], "armor-pack");
        assert_eq!(value["nodes"][0]["name"], "Armor Pack");
        assert_eq!(value["edges"][0]["sourceModId"], "armor-pack");
        assert_eq!(value["edges"][0]["dependency"], "stracker-loader");
        assert_eq!(value["edges"][0]["matchedImportedModId"], "stracker-loader");
        assert!(value["edges"][0].get("installed").is_none());
        assert!(value["edges"][0].get("path").is_none());
    }
}

#[cfg(test)]
mod task_dto_tests {
    use super::*;

    #[test]
    fn serializes_task_started_dto_with_camel_case_fields() {
        let dto = TaskStartedDto {
            task_id: "mod-import-123".to_owned(),
            kind: TaskKindDto::ModImport,
            status: TaskStatusDto::Queued,
        };

        let value = serde_json::to_value(dto).expect("serialize dto");

        assert_eq!(value["taskId"], "mod-import-123");
        assert_eq!(value["kind"], "mod_import");
        assert_eq!(value["status"], "queued");
    }

    #[test]
    fn serializes_install_task_kind_as_stable_snake_case() {
        let dto: TaskStartedDto = TaskStarted {
            task_id: "install-123".to_owned(),
            kind: TaskKind::Install,
            status: TaskStatus::Queued,
        }
        .into();

        let value = serde_json::to_value(dto).expect("serialize dto");

        assert_eq!(value["taskId"], "install-123");
        assert_eq!(value["kind"], "install");
        assert_eq!(value["status"], "queued");
    }

    #[test]
    fn serializes_save_backup_task_kind_as_stable_snake_case() {
        let dto: TaskStartedDto = TaskStarted {
            task_id: "save-backup-123".to_owned(),
            kind: TaskKind::SaveBackup,
            status: TaskStatus::Queued,
        }
        .into();

        let value = serde_json::to_value(dto).expect("serialize dto");

        assert_eq!(value["taskId"], "save-backup-123");
        assert_eq!(value["kind"], "save_backup");
        assert_eq!(value["status"], "queued");
    }

    #[test]
    fn serializes_task_progress_event_dto_with_camel_case_fields() {
        let dto: TaskProgressEventDto = TaskProgressEvent::new(
            "mod-import-123",
            TaskKind::ModImport,
            TaskStatus::Queued,
            "mod_import.queued",
        )
        .into();

        let value = serde_json::to_value(dto).expect("serialize dto");

        assert_eq!(value["taskId"], "mod-import-123");
        assert_eq!(value["kind"], "mod_import");
        assert_eq!(value["status"], "queued");
        assert_eq!(value["phase"], "mod_import.queued");
        assert!(value["current"].is_null());
        assert!(value["total"].is_null());
        assert!(value["message"].is_null());
        assert!(value["error"].is_null());
        assert!(value["resultRef"].is_null());
    }

    #[test]
    fn maps_mod_import_task_error_to_command_error_code() {
        let dto =
            CommandErrorDto::from_mod_import_task_error(ModImportTaskError::ArchivePathNotAbsolute);

        assert_eq!(dto.code, "archive_path_not_absolute");
    }
}

#[cfg(test)]
mod install_preflight_dto_tests {
    use super::*;

    #[test]
    fn serializes_imported_mod_preflight_decision_without_diagnostic_details() {
        let dto: ImportedModInstallPreflightDto = hmm_app::ImportedModInstallPreflight {
            plan: hmm_core::InstallPlan::from_providers(Vec::new()),
            prerequisite_decision: hmm_app::GamePrerequisiteDecision {
                game_id: hmm_core::GameId::mhw(),
                status: hmm_app::GamePrerequisiteDecisionStatus::Blocked,
                rules_version: Some(7),
                codes: vec![
                    hmm_app::GamePrerequisiteDecisionCode::MissingRequiredFile,
                    hmm_app::GamePrerequisiteDecisionCode::ConfigInvalidJson,
                ],
            },
        }
        .into();

        let value = serde_json::to_value(dto).expect("serialize imported Mod preflight");
        let serialized = value.to_string();

        assert_eq!(value["hasBlockingConflicts"], false);
        assert_eq!(value["prerequisiteDecision"]["status"], "blocked");
        assert_eq!(value["prerequisiteDecision"]["rulesVersion"], 7);
        assert_eq!(
            value["prerequisiteDecision"]["codes"],
            serde_json::json!(["missing_required_file", "config_invalid_json"])
        );
        assert!(value.get("prerequisite_decision").is_none());
        assert!(!serialized.contains("issuePath"));
        assert!(!serialized.contains("message"));
        assert!(!serialized.contains("loader-config.json"));
        assert!(!serialized.contains("C:\\Users\\"));
    }
}

#[cfg(test)]
mod install_recovery_dto_tests {
    use super::*;
    use hmm_core::{ModId, ProfileId};

    #[test]
    fn serializes_rollback_required_manifest_status_as_stable_snake_case() {
        let dto: InstallManifestStatusSummaryDto = InstallManifestStatusSummary {
            profile_id: ProfileId::new("default"),
            mod_id: ModId::new("mod-a"),
            status: InstallManifestStatus::RollbackRequired,
            managed_file_count: 1,
            backup_count: 0,
            installed_revision_id: None,
        }
        .into();

        let value = serde_json::to_value(dto).expect("serialize manifest status summary");

        assert_eq!(value["status"], "rollback_required");
        assert!(value.get("targetPath").is_none());
        assert!(value.get("backupRef").is_none());
    }

    #[test]
    fn serializes_install_recovery_summary_without_paths_or_backup_refs() {
        let dto: InstallRecoverySummaryDto = hmm_app::InstallRecoverySummary {
            profile_id: ProfileId::new("default"),
            mod_id: ModId::new("mod-a"),
            status: hmm_app::InstallRecoveryStatus::RepairRequired,
            managed_file_count: 1,
            backup_count: 1,
            issue_count: 1,
            issues: vec![hmm_app::InstallRecoveryIssueSummary {
                issue: hmm_app::InstallRecoveryIssue::BackupMissing,
                count: 1,
            }],
        }
        .into();

        let value = serde_json::to_value(dto).expect("serialize recovery summary");

        assert_eq!(value["profileId"], "default");
        assert_eq!(value["modId"], "mod-a");
        assert_eq!(value["status"], "repair_required");
        assert_eq!(value["managedFileCount"], 1);
        assert_eq!(value["backupCount"], 1);
        assert_eq!(value["issueCount"], 1);
        assert_eq!(value["issues"][0]["issue"], "backup_missing");
        assert_eq!(value["issues"][0]["count"], 1);
        assert!(value.get("targetPath").is_none());
        assert!(value.get("backupRef").is_none());
        assert!(!value.to_string().contains("nativePC"));
        assert!(!value.to_string().contains("backup-original"));
    }

    #[test]
    fn serializes_rollback_required_recovery_status_as_stable_snake_case() {
        let dto: InstallRecoverySummaryDto = hmm_app::InstallRecoverySummary {
            profile_id: ProfileId::new("default"),
            mod_id: ModId::new("mod-a"),
            status: hmm_app::InstallRecoveryStatus::RollbackRequired,
            managed_file_count: 1,
            backup_count: 0,
            issue_count: 0,
            issues: Vec::new(),
        }
        .into();

        let value = serde_json::to_value(dto).expect("serialize recovery summary");

        assert_eq!(value["status"], "rollback_required");
        assert!(value.get("targetPath").is_none());
        assert!(value.get("backupRef").is_none());
    }

    #[test]
    fn pending_reinstall_states_keep_distinct_public_codes() {
        assert_eq!(
            serde_json::to_value(InstallRecoveryStatusDto::from(
                hmm_app::InstallRecoveryStatus::CommittedCleanupPending
            ))
            .expect("serialize committed cleanup recovery status"),
            "committed_cleanup_pending"
        );
        assert_eq!(
            serde_json::to_value(InstallRecoveryStatusDto::from(
                hmm_app::InstallRecoveryStatus::CleanupPending
            ))
            .expect("serialize cleanup recovery status"),
            "cleanup_pending"
        );
        assert_eq!(
            serde_json::to_value(InstallManifestStatusDto::from(
                InstallManifestStatus::CommittedCleanupPending
            ))
            .expect("serialize committed cleanup manifest status"),
            "committed_cleanup_pending"
        );
        assert_eq!(
            serde_json::to_value(InstallManifestStatusDto::from(
                InstallManifestStatus::CleanupPending
            ))
            .expect("serialize cleanup manifest status"),
            "cleanup_pending"
        );
    }

    #[test]
    fn serializes_recovery_action_preview_without_paths_or_backup_refs() {
        let dto: InstallRecoveryActionPreviewDto = hmm_app::InstallRecoveryActionPreview {
            profile_id: ProfileId::new("default"),
            mod_id: ModId::new("mod-a"),
            action_kind: hmm_app::InstallRecoveryActionKind::RollbackInstall,
            availability: hmm_app::InstallRecoveryActionAvailability::Blocked,
            remove_file_count: 1,
            restore_file_count: 1,
            backup_count: 1,
            blocking_issue_count: 2,
            blocking_reasons: vec![
                hmm_app::InstallRecoveryActionBlockReasonSummary {
                    reason: hmm_app::InstallRecoveryActionBlockReason::TargetChanged,
                    count: 1,
                },
                hmm_app::InstallRecoveryActionBlockReasonSummary {
                    reason: hmm_app::InstallRecoveryActionBlockReason::BackupMissing,
                    count: 1,
                },
            ],
        }
        .into();

        let value = serde_json::to_value(dto).expect("serialize recovery action preview");

        assert_eq!(value["profileId"], "default");
        assert_eq!(value["modId"], "mod-a");
        assert_eq!(value["actionKind"], "rollback_install");
        assert_eq!(value["availability"], "blocked");
        assert_eq!(value["removeFileCount"], 1);
        assert_eq!(value["restoreFileCount"], 1);
        assert_eq!(value["backupCount"], 1);
        assert_eq!(value["blockingIssueCount"], 2);
        assert_eq!(value["blockingReasons"][0]["reason"], "target_changed");
        assert_eq!(value["blockingReasons"][1]["reason"], "backup_missing");
        assert!(value.get("targetPath").is_none());
        assert!(value.get("backupRef").is_none());
        assert!(value.get("manifestPath").is_none());
        assert!(!value.to_string().contains("nativePC"));
        assert!(!value.to_string().contains("backup-original"));
    }
}
