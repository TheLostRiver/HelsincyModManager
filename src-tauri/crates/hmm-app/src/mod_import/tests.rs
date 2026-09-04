use super::*;
use crate::mod_import_diagnostics::{
    PreviewImageDiagnosticExportCategory, PreviewImageDiagnosticExportCategoryId,
    PreviewImageDiagnosticExportCategoryStatus, PreviewImageDiagnosticExportExclusionReason,
    PreviewImageFallbackDiagnostic,
};
use hmm_core::{ModId, ModMetadataOverlay, PreviewImageRejectionReason};
use hmm_ports::{
    AppSettings, AppSettingsRepository, AppSettingsRepositoryResult,
    ModImportPackagePrepareRequest, ModImportPackagePreparer, ModImportResultRepository,
    ModPackageMetadata, ModPackageMetadataAnalysis, ModPackageMetadataAnalyzer, PreparedModPackage,
    PreviewImageProcessingResult, ProcessedPreviewImage, StoredImportPreviewImage,
    StoredModImportAnalysis, ThumbnailCacheMaintenance, ThumbnailCacheMaintenanceRequest,
    ThumbnailRef, ThumbnailStore,
};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[test]
fn analyze_sandbox_includes_preview_thumbnail() {
    let service = ModImportAnalysisService::new(
        Box::new(FakePreviewImageProcessor {
            result: sample_thumbnail_result(),
        }),
        Box::new(FakeThumbnailStore::default()),
        Box::new(FakeMetadataAnalyzer::default()),
    );

    let result = service
        .analyze_sandbox(ModImportAnalysisRequest {
            task_id: "task-1".to_owned(),
            package_id: "pkg-1".to_owned(),
            sandbox_root: Path::new("sandbox").to_path_buf(),
            archive_display_name_hint: None,
        })
        .expect("analysis succeeds");

    assert_eq!(result.task_id, "task-1");
    assert_eq!(result.package_id, "pkg-1");
    assert_eq!(
        result.preview_image,
        ImportPreviewImage::Thumbnail {
            thumbnail_url: "thumbnail://pkg-1/preview-768/hash-1".to_owned(),
            width: 320,
            height: 180,
            content_hash: "hash-1".to_owned(),
            variant: "preview-768".to_owned(),
        }
    );
}

#[test]
fn analyze_sandbox_uses_package_metadata_display_name() {
    let service = ModImportAnalysisService::new(
        Box::new(FakePreviewImageProcessor {
            result: PreviewImageProcessingResult::Fallback(PreviewImageRejectionReason::Missing),
        }),
        Box::new(FakeThumbnailStore::default()),
        Box::new(FakeMetadataAnalyzer {
            display_name: Some("Better Mod Name".to_owned()),
            ..FakeMetadataAnalyzer::default()
        }),
    );

    let result = service
        .analyze_sandbox(ModImportAnalysisRequest {
            task_id: "task-1".to_owned(),
            package_id: "pkg-1".to_owned(),
            sandbox_root: Path::new("sandbox").to_path_buf(),
            archive_display_name_hint: None,
        })
        .expect("analysis succeeds");

    assert_eq!(result.display_name, "Better Mod Name");
    assert_eq!(
        stored_analysis_from_result(&ModId::new("logical-mod"), &result).display_name,
        "Better Mod Name"
    );
}

#[test]
fn analyze_sandbox_prefers_manifest_declared_name_over_archive_file_name() {
    let service = ModImportAnalysisService::new(
        Box::new(FakePreviewImageProcessor {
            result: PreviewImageProcessingResult::Fallback(PreviewImageRejectionReason::Missing),
        }),
        Box::new(FakeThumbnailStore::default()),
        Box::new(FakeMetadataAnalyzer {
            manifest_display_name: Some("Manifest Name".to_owned()),
            ..FakeMetadataAnalyzer::default()
        }),
    );

    let result = service
        .analyze_sandbox(ModImportAnalysisRequest {
            task_id: "task-1".to_owned(),
            package_id: "pkg-1".to_owned(),
            sandbox_root: Path::new("sandbox").to_path_buf(),
            archive_display_name_hint: Some("归档文件名".to_owned()),
        })
        .expect("analysis succeeds");

    // manifest 是作者在包内的结构化声明，表达作者意图，优先于文件名。
    assert_eq!(result.display_name, "Manifest Name");
}

#[test]
fn analyze_sandbox_prefers_archive_file_name_over_readme_display_name() {
    let service = ModImportAnalysisService::new(
        Box::new(FakePreviewImageProcessor {
            result: PreviewImageProcessingResult::Fallback(PreviewImageRejectionReason::Missing),
        }),
        Box::new(FakeThumbnailStore::default()),
        Box::new(FakeMetadataAnalyzer {
            // 无 manifest 时 metadata.display_name 来自 readme 首行。
            display_name: Some("安装教程：先解压到 nativePC".to_owned()),
            ..FakeMetadataAnalyzer::default()
        }),
    );

    let result = service
        .analyze_sandbox(ModImportAnalysisRequest {
            task_id: "task-1".to_owned(),
            package_id: "pkg-1".to_owned(),
            sandbox_root: Path::new("sandbox").to_path_buf(),
            archive_display_name_hint: Some("黑骑士大剑".to_owned()),
        })
        .expect("analysis succeeds");

    // 文件名是导入者导入前唯一亲自确认过的名称；readme 首行经常是教程、
    // 致谢或广告，拿它当名字会得到毫无辨识度的条目，只配在文件名缺席时兜底。
    assert_eq!(result.display_name, "黑骑士大剑");
    // 文件名同样不得回填 metadata.display_name——继承判定语义保持不变。
    assert_eq!(
        result.metadata.display_name,
        Some("安装教程：先解压到 nativePC".to_owned())
    );
}

#[test]
fn analyze_sandbox_prefers_manifest_declared_name_over_readme_display_name() {
    let service = ModImportAnalysisService::new(
        Box::new(FakePreviewImageProcessor {
            result: PreviewImageProcessingResult::Fallback(PreviewImageRejectionReason::Missing),
        }),
        Box::new(FakeThumbnailStore::default()),
        Box::new(FakeMetadataAnalyzer {
            display_name: Some("Readme Name".to_owned()),
            manifest_display_name: Some("Manifest Name".to_owned()),
            ..FakeMetadataAnalyzer::default()
        }),
    );

    let result = service
        .analyze_sandbox(ModImportAnalysisRequest {
            task_id: "task-1".to_owned(),
            package_id: "pkg-1".to_owned(),
            sandbox_root: Path::new("sandbox").to_path_buf(),
            archive_display_name_hint: None,
        })
        .expect("analysis succeeds");

    assert_eq!(result.display_name, "Manifest Name");
}

#[test]
fn analyze_sandbox_falls_back_to_archive_file_name_without_package_metadata() {
    let service = ModImportAnalysisService::new(
        Box::new(FakePreviewImageProcessor {
            result: PreviewImageProcessingResult::Fallback(PreviewImageRejectionReason::Missing),
        }),
        Box::new(FakeThumbnailStore::default()),
        Box::new(FakeMetadataAnalyzer::default()),
    );

    let result = service
        .analyze_sandbox(ModImportAnalysisRequest {
            task_id: "task-1".to_owned(),
            package_id: "mod-import-1787138082375-0".to_owned(),
            sandbox_root: Path::new("sandbox").to_path_buf(),
            archive_display_name_hint: Some("黑骑士大剑".to_owned()),
        })
        .expect("analysis succeeds");

    assert_eq!(result.display_name, "黑骑士大剑");
    assert_eq!(
        stored_analysis_from_result(&ModId::new("logical-mod"), &result).display_name,
        "黑骑士大剑"
    );
    // 文件名只影响展示名，不得回填 metadata.display_name——后者是 revision
    // 继承的判据，污染它会让 revision 导入重命名既有 logical Mod。
    assert_eq!(result.metadata.display_name, None);
}

#[test]
fn analyze_sandbox_falls_back_to_package_id_when_no_name_source_is_usable() {
    let service = ModImportAnalysisService::new(
        Box::new(FakePreviewImageProcessor {
            result: PreviewImageProcessingResult::Fallback(PreviewImageRejectionReason::Missing),
        }),
        Box::new(FakeThumbnailStore::default()),
        Box::new(FakeMetadataAnalyzer::default()),
    );

    let result = service
        .analyze_sandbox(ModImportAnalysisRequest {
            task_id: "task-1".to_owned(),
            package_id: "mod-import-1787138082375-0".to_owned(),
            sandbox_root: Path::new("sandbox").to_path_buf(),
            // 非 UTF-8 或纯空白的文件名会被净化成 None。
            archive_display_name_hint: None,
        })
        .expect("analysis succeeds");

    // 末端必须是 package_id 而非空串：投影仓储在 display_name 为空时
    // 会硬失败整个写入。
    assert_eq!(result.display_name, "mod-import-1787138082375-0");
    assert!(!result.display_name.is_empty());
}

#[test]
fn analyze_sandbox_persists_package_metadata_schema_fields() {
    let service = ModImportAnalysisService::new(
        Box::new(FakePreviewImageProcessor {
            result: PreviewImageProcessingResult::Fallback(PreviewImageRejectionReason::Missing),
        }),
        Box::new(FakeThumbnailStore::default()),
        Box::new(FakeMetadataAnalyzer {
            display_name: Some("Better Mod Name".to_owned()),
            version: Some("1.2.3".to_owned()),
            author: Some("A Hunter".to_owned()),
            category: Some("Visual".to_owned()),
            tags: vec!["armor".to_owned(), "hd".to_owned()],
            dependencies: vec!["stracker-loader".to_owned()],
            ..FakeMetadataAnalyzer::default()
        }),
    );

    let result = service
        .analyze_sandbox(ModImportAnalysisRequest {
            task_id: "task-1".to_owned(),
            package_id: "pkg-1".to_owned(),
            sandbox_root: Path::new("sandbox").to_path_buf(),
            archive_display_name_hint: None,
        })
        .expect("analysis succeeds");
    let stored = stored_analysis_from_result(&ModId::new("logical-mod"), &result);
    let library_item = library_item_from_stored(stored.clone());

    assert_eq!(result.metadata.version.as_deref(), Some("1.2.3"));
    assert_eq!(stored.mod_id, "logical-mod");
    assert_eq!(stored.package_id, "pkg-1");
    assert_eq!(stored.metadata.author.as_deref(), Some("A Hunter"));
    assert_eq!(stored.metadata.dependencies, vec!["stracker-loader"]);
    assert_eq!(
        library_item.category_labels,
        vec![
            CategoryLabel {
                name: "Visual".to_owned(),
                color: None
            },
            CategoryLabel {
                name: "armor".to_owned(),
                color: None
            },
            CategoryLabel {
                name: "hd".to_owned(),
                color: None
            },
        ]
    );
}

#[test]
fn analyze_sandbox_keeps_import_result_when_preview_falls_back() {
    let service = ModImportAnalysisService::new(
        Box::new(FakePreviewImageProcessor {
            result: PreviewImageProcessingResult::Fallback(
                PreviewImageRejectionReason::DecodeFailed,
            ),
        }),
        Box::new(FakeThumbnailStore::default()),
        Box::new(FakeMetadataAnalyzer::default()),
    );

    let result = service
        .analyze_sandbox(ModImportAnalysisRequest {
            task_id: "task-1".to_owned(),
            package_id: "pkg-1".to_owned(),
            sandbox_root: Path::new("sandbox").to_path_buf(),
            archive_display_name_hint: None,
        })
        .expect("analysis succeeds");

    assert_eq!(
        result.preview_image,
        ImportPreviewImage::Fallback {
            reason: PreviewImageRejectionReason::DecodeFailed,
        }
    );
}

#[test]
fn analyze_sandbox_passes_cancellation_token_to_preview_processor() {
    let observed = std::sync::Arc::new(Mutex::new(Vec::new()));
    let service = ModImportAnalysisService::new(
        Box::new(CancellationObservingPreviewImageProcessor {
            observed: std::sync::Arc::clone(&observed),
        }),
        Box::new(FakeThumbnailStore::default()),
        Box::new(FakeMetadataAnalyzer::default()),
    );
    let cancellation_token = TestCancellationToken { cancelled: false };

    let result = service
        .analyze_sandbox_with_cancellation(
            ModImportAnalysisRequest {
                task_id: "task-1".to_owned(),
                package_id: "pkg-1".to_owned(),
                sandbox_root: Path::new("sandbox").to_path_buf(),
                archive_display_name_hint: None,
            },
            &cancellation_token,
        )
        .expect("analysis succeeds");

    assert_eq!(
        result.preview_image,
        ImportPreviewImage::Fallback {
            reason: PreviewImageRejectionReason::Missing,
        }
    );
    assert_eq!(observed.lock().expect("observed lock").as_slice(), &[false]);
}

#[test]
fn analyze_sandbox_falls_back_when_thumbnail_url_resolution_fails() {
    let service = ModImportAnalysisService::new(
        Box::new(FakePreviewImageProcessor {
            result: sample_thumbnail_result(),
        }),
        Box::new(FakeThumbnailStore { fail_resolve: true }),
        Box::new(FakeMetadataAnalyzer::default()),
    );

    let result = service
        .analyze_sandbox(ModImportAnalysisRequest {
            task_id: "task-1".to_owned(),
            package_id: "pkg-1".to_owned(),
            sandbox_root: Path::new("sandbox").to_path_buf(),
            archive_display_name_hint: None,
        })
        .expect("analysis succeeds");

    assert_eq!(
        result.preview_image,
        ImportPreviewImage::Fallback {
            reason: PreviewImageRejectionReason::CacheWriteFailed,
        }
    );
}

#[test]
fn prepare_import_runs_preparer_and_preview_analysis_with_task_events() {
    let service = ModImportPrepareService::new(
        Box::new(FakePackagePreparer::new(
            "task-1",
            Path::new("C:/mods/sample.zip"),
            "pkg-1",
            Path::new("sandbox"),
        )),
        ModImportAnalysisService::new(
            Box::new(FakePreviewImageProcessor {
                result: sample_thumbnail_result(),
            }),
            Box::new(FakeThumbnailStore::default()),
            Box::new(FakeMetadataAnalyzer::default()),
        ),
    );

    let result = service
        .prepare_import(ModImportPrepareRequest {
            task_id: "task-1".to_owned(),
            archive_path: Path::new("C:/mods/sample.zip").to_path_buf(),
            archive_display_name_hint: None,
        })
        .expect("prepare succeeds");

    assert_eq!(result.analysis.task_id, "task-1");
    assert_eq!(result.analysis.package_id, "pkg-1");
    assert_eq!(
        result.analysis.preview_image,
        ImportPreviewImage::Thumbnail {
            thumbnail_url: "thumbnail://pkg-1/preview-768/hash-1".to_owned(),
            width: 320,
            height: 180,
            content_hash: "hash-1".to_owned(),
            variant: "preview-768".to_owned(),
        }
    );
    assert_eq!(
        event_phases(&result.events),
        vec![
            "mod_import.unpack.started",
            "mod_import.unpack.completed",
            "mod_import.preview_image.processing",
        ]
    );
    assert!(result.events.iter().all(|event| event.task_id == "task-1"
        && event.kind == crate::TaskKind::ModImport
        && event.status == crate::TaskStatus::Running));
}

#[test]
fn prepare_import_emits_preview_fallback_event_when_preview_falls_back() {
    let service = ModImportPrepareService::new(
        Box::new(FakePackagePreparer::new(
            "task-1",
            Path::new("C:/mods/sample.zip"),
            "pkg-1",
            Path::new("sandbox"),
        )),
        ModImportAnalysisService::new(
            Box::new(FakePreviewImageProcessor {
                result: PreviewImageProcessingResult::Fallback(
                    PreviewImageRejectionReason::DecodeFailed,
                ),
            }),
            Box::new(FakeThumbnailStore::default()),
            Box::new(FakeMetadataAnalyzer::default()),
        ),
    );

    let result = service
        .prepare_import(ModImportPrepareRequest {
            task_id: "task-1".to_owned(),
            archive_path: Path::new("C:/mods/sample.zip").to_path_buf(),
            archive_display_name_hint: None,
        })
        .expect("prepare succeeds");

    assert_eq!(
        result.analysis.preview_image,
        ImportPreviewImage::Fallback {
            reason: PreviewImageRejectionReason::DecodeFailed,
        }
    );
    assert_eq!(
        event_phases(&result.events),
        vec![
            "mod_import.unpack.started",
            "mod_import.unpack.completed",
            "mod_import.preview_image.processing",
            "mod_import.preview_image.fallback",
        ]
    );
    let fallback_event = result
        .events
        .iter()
        .find(|event| event.phase == "mod_import.preview_image.fallback")
        .expect("fallback event is emitted");
    assert_eq!(fallback_event.error.as_deref(), Some("decode_failed"));
    assert_eq!(fallback_event.message, None);
    assert_eq!(fallback_event.result_ref, None);
}

#[test]
fn preview_image_rejection_reason_key_returns_contract_values() {
    assert_eq!(
        [
            preview_image_rejection_reason_key(PreviewImageRejectionReason::Missing),
            preview_image_rejection_reason_key(PreviewImageRejectionReason::TooLarge),
            preview_image_rejection_reason_key(PreviewImageRejectionReason::TooManyCandidates),
            preview_image_rejection_reason_key(PreviewImageRejectionReason::UnsupportedFormat),
            preview_image_rejection_reason_key(PreviewImageRejectionReason::DecodeFailed),
            preview_image_rejection_reason_key(PreviewImageRejectionReason::PixelLimitExceeded),
            preview_image_rejection_reason_key(PreviewImageRejectionReason::CacheWriteFailed),
        ],
        [
            "missing",
            "too_large",
            "too_many_candidates",
            "unsupported_format",
            "decode_failed",
            "pixel_limit_exceeded",
            "cache_write_failed",
        ]
    );
}

#[test]
fn task_runner_executes_prepare_and_marks_task_completed() {
    let task_manager = std::sync::Arc::new(crate::TaskManager::new());
    let task = task_manager
        .create_task(crate::TaskKind::ModImport)
        .expect("task can be created");
    let result_repository = std::sync::Arc::new(FakeModImportResultRepository::default());
    let runner = ModImportTaskRunner::new(
        std::sync::Arc::clone(&task_manager),
        std::sync::Arc::new(ModImportPrepareService::new(
            Box::new(FakePackagePreparer::new(
                &task.task_id,
                Path::new("C:/mods/sample.zip"),
                "pkg-1",
                Path::new("sandbox"),
            )),
            ModImportAnalysisService::new(
                Box::new(FakePreviewImageProcessor {
                    result: PreviewImageProcessingResult::Fallback(
                        PreviewImageRejectionReason::Missing,
                    ),
                }),
                Box::new(FakeThumbnailStore::default()),
                Box::new(FakeMetadataAnalyzer {
                    display_name: Some("Better Mod Name".to_owned()),
                    ..FakeMetadataAnalyzer::default()
                }),
            ),
        )),
        result_repository,
    );

    let events = runner
        .run_prepare_task(&task.task_id, Path::new("C:/mods/sample.zip").to_path_buf())
        .expect("runner succeeds");

    assert_eq!(
        event_phases(&events),
        vec![
            "mod_import.unpack.started",
            "mod_import.unpack.completed",
            "mod_import.preview_image.processing",
            "mod_import.preview_image.fallback",
            "mod_import.prepare.completed",
        ]
    );
    assert!(events.iter().all(|event| event.task_id == task.task_id));
    assert_eq!(
        task_manager.task_status(&task.task_id),
        Some(crate::TaskStatus::Completed)
    );
}

#[test]
fn task_runner_persists_prepare_analysis_for_library_queries() {
    let task_manager = std::sync::Arc::new(crate::TaskManager::new());
    let task = task_manager
        .create_task(crate::TaskKind::ModImport)
        .expect("task can be created");
    let result_repository = std::sync::Arc::new(FakeModImportResultRepository::default());
    let runner = ModImportTaskRunner::new(
        std::sync::Arc::clone(&task_manager),
        std::sync::Arc::new(ModImportPrepareService::new(
            Box::new(FakePackagePreparer::new(
                &task.task_id,
                Path::new("C:/mods/sample.zip"),
                "pkg-1",
                Path::new("sandbox"),
            )),
            ModImportAnalysisService::new(
                Box::new(FakePreviewImageProcessor {
                    result: sample_thumbnail_result(),
                }),
                Box::new(FakeThumbnailStore::default()),
                // manifest 声明名优于文件名，"sample.zip" 不应劫持展示名；
                // 本测试关注的是分析结果的持久化，不是命名优先级。
                Box::new(FakeMetadataAnalyzer {
                    manifest_display_name: Some("Better Mod Name".to_owned()),
                    ..FakeMetadataAnalyzer::default()
                }),
            ),
        )),
        result_repository.clone(),
    );

    runner
        .run_prepare_task(&task.task_id, Path::new("C:/mods/sample.zip").to_path_buf())
        .expect("runner succeeds");

    let stored = result_repository
        .get_analysis("pkg-1")
        .expect("repository read succeeds")
        .expect("analysis was saved");

    assert_eq!(stored.mod_id, "pkg-1");
    assert_eq!(stored.package_id, "pkg-1");
    assert_eq!(stored.display_name, "Better Mod Name");
    assert_eq!(stored.task_id, task.task_id);
    assert_eq!(
        stored.preview_image,
        StoredImportPreviewImage::Thumbnail {
            thumbnail_url: "thumbnail://pkg-1/preview-768/hash-1".to_owned(),
            width: 320,
            height: 180,
            content_hash: "hash-1".to_owned(),
            variant: "preview-768".to_owned(),
        }
    );
}

#[test]
fn task_runner_prunes_thumbnail_cache_using_all_persisted_thumbnail_refs() {
    let task_manager = std::sync::Arc::new(crate::TaskManager::new());
    let task = task_manager
        .create_task(crate::TaskKind::ModImport)
        .expect("task can be created");
    let result_repository = std::sync::Arc::new(FakeModImportResultRepository::default());
    result_repository
        .save_analysis(&StoredModImportAnalysis {
            mod_id: "pkg-old".to_owned(),
            task_id: "task-old".to_owned(),
            package_id: "pkg-old".to_owned(),
            display_name: "Old Mod".to_owned(),
            metadata: StoredModPackageMetadata::default(),
            preview_image: StoredImportPreviewImage::Thumbnail {
                thumbnail_url: "thumbnail://pkg-old/preview-1024/hash-old".to_owned(),
                width: 320,
                height: 180,
                content_hash: "hash-old".to_owned(),
                variant: "preview-1024".to_owned(),
            },
        })
        .expect("seed old analysis");
    let thumbnail_cache_maintenance = std::sync::Arc::new(FakeThumbnailCacheMaintenance::default());
    let runner = ModImportTaskRunner::new(
        std::sync::Arc::clone(&task_manager),
        std::sync::Arc::new(ModImportPrepareService::new(
            Box::new(FakePackagePreparer::new(
                &task.task_id,
                Path::new("C:/mods/sample.zip"),
                "pkg-1",
                Path::new("sandbox"),
            )),
            ModImportAnalysisService::new(
                Box::new(FakePreviewImageProcessor {
                    result: sample_thumbnail_result(),
                }),
                Box::new(FakeThumbnailStore::default()),
                Box::new(FakeMetadataAnalyzer::default()),
            ),
        )),
        result_repository,
    )
    .with_thumbnail_cache_maintenance(thumbnail_cache_maintenance.clone());

    runner
        .run_prepare_task(&task.task_id, Path::new("C:/mods/sample.zip").to_path_buf())
        .expect("runner succeeds");

    let calls = thumbnail_cache_maintenance
        .calls
        .lock()
        .expect("calls lock");
    assert_eq!(calls.len(), 1);
    assert_eq!(
        calls[0].retained,
        vec![
            ThumbnailRef {
                package_id: "pkg-old".to_owned(),
                variant: "preview-1024".to_owned(),
                content_hash: "hash-old".to_owned(),
            },
            ThumbnailRef {
                package_id: "pkg-1".to_owned(),
                variant: "preview-768".to_owned(),
                content_hash: "hash-1".to_owned(),
            },
        ]
    );
    assert_eq!(calls[0].max_bytes, Some(DEFAULT_THUMBNAIL_CACHE_MAX_BYTES));
}

#[test]
fn task_runner_uses_configured_thumbnail_cache_size_limit_for_maintenance() {
    let task_manager = std::sync::Arc::new(crate::TaskManager::new());
    let task = task_manager
        .create_task(crate::TaskKind::ModImport)
        .expect("task can be created");
    let result_repository = std::sync::Arc::new(FakeModImportResultRepository::default());
    let thumbnail_cache_maintenance = std::sync::Arc::new(FakeThumbnailCacheMaintenance::default());
    let settings_repository = fake_app_settings_repository(64 * 1024 * 1024, 14);
    let runner = ModImportTaskRunner::new(
        std::sync::Arc::clone(&task_manager),
        std::sync::Arc::new(ModImportPrepareService::new(
            Box::new(FakePackagePreparer::new(
                &task.task_id,
                Path::new("C:/mods/sample.zip"),
                "pkg-1",
                Path::new("sandbox"),
            )),
            ModImportAnalysisService::new(
                Box::new(FakePreviewImageProcessor {
                    result: PreviewImageProcessingResult::Fallback(
                        PreviewImageRejectionReason::Missing,
                    ),
                }),
                Box::new(FakeThumbnailStore::default()),
                Box::new(FakeMetadataAnalyzer::default()),
            ),
        )),
        result_repository,
    )
    .with_thumbnail_cache_maintenance(thumbnail_cache_maintenance.clone())
    .with_app_settings_repository(settings_repository.clone());

    runner
        .run_prepare_task(&task.task_id, Path::new("C:/mods/sample.zip").to_path_buf())
        .expect("runner succeeds");

    let calls = thumbnail_cache_maintenance
        .calls
        .lock()
        .expect("calls lock");
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].max_bytes, Some(64 * 1024 * 1024));
    assert_eq!(calls[0].max_age, Some(days(14)));
    assert_eq!(settings_repository.load_count(), 1);
}

#[test]
fn scheduled_thumbnail_cache_maintenance_runs_one_cycle_after_interval() {
    let task_manager = std::sync::Arc::new(crate::TaskManager::new());
    let result_repository = std::sync::Arc::new(FakeModImportResultRepository::default());
    result_repository
        .save_analysis(&StoredModImportAnalysis {
            mod_id: "pkg-1".to_owned(),
            task_id: "task-1".to_owned(),
            package_id: "pkg-1".to_owned(),
            display_name: "Mod".to_owned(),
            metadata: StoredModPackageMetadata::default(),
            preview_image: StoredImportPreviewImage::Thumbnail {
                thumbnail_url: "thumbnail://pkg-1/preview-768/hash-1".to_owned(),
                width: 320,
                height: 180,
                content_hash: "hash-1".to_owned(),
                variant: "preview-768".to_owned(),
            },
        })
        .expect("seed analysis");
    let thumbnail_cache_maintenance = std::sync::Arc::new(FakeThumbnailCacheMaintenance::default());
    let settings_repository = fake_app_settings_repository(32 * 1024 * 1024, 7);
    let runner = std::sync::Arc::new(
        ModImportTaskRunner::new(
            task_manager,
            std::sync::Arc::new(ModImportPrepareService::new(
                Box::new(FakePackagePreparer::new(
                    "unused-task",
                    Path::new("C:/mods/unused.zip"),
                    "pkg-unused",
                    Path::new("sandbox"),
                )),
                ModImportAnalysisService::new(
                    Box::new(FakePreviewImageProcessor {
                        result: PreviewImageProcessingResult::Fallback(
                            PreviewImageRejectionReason::Missing,
                        ),
                    }),
                    Box::new(FakeThumbnailStore::default()),
                    Box::new(FakeMetadataAnalyzer::default()),
                ),
            )),
            result_repository,
        )
        .with_thumbnail_cache_maintenance(thumbnail_cache_maintenance.clone())
        .with_app_settings_repository(settings_repository),
    );
    let scheduler = ThumbnailCacheMaintenanceScheduler::new(runner, Duration::from_secs(3600));
    let slept_intervals = Mutex::new(Vec::new());

    scheduler.run_one_cycle_with_sleep(|duration| {
        slept_intervals
            .lock()
            .expect("slept intervals lock")
            .push(duration);
    });

    assert_eq!(
        *slept_intervals.lock().expect("slept intervals lock"),
        vec![Duration::from_secs(3600)]
    );
    let calls = thumbnail_cache_maintenance
        .calls
        .lock()
        .expect("calls lock");
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].max_bytes, Some(32 * 1024 * 1024));
    assert_eq!(calls[0].max_age, Some(days(7)));
    assert_eq!(
        calls[0].retained,
        vec![ThumbnailRef {
            package_id: "pkg-1".to_owned(),
            variant: "preview-768".to_owned(),
            content_hash: "hash-1".to_owned(),
        }]
    );
}

#[test]
fn manual_thumbnail_cache_maintenance_uses_retained_refs_and_settings() {
    let task_manager = std::sync::Arc::new(crate::TaskManager::new());
    let result_repository = std::sync::Arc::new(FakeModImportResultRepository::default());
    result_repository
        .save_analysis(&StoredModImportAnalysis {
            mod_id: "pkg-1".to_owned(),
            task_id: "task-1".to_owned(),
            package_id: "pkg-1".to_owned(),
            display_name: "Mod".to_owned(),
            metadata: StoredModPackageMetadata::default(),
            preview_image: StoredImportPreviewImage::Thumbnail {
                thumbnail_url: "thumbnail://pkg-1/preview-768/hash-1".to_owned(),
                width: 320,
                height: 180,
                content_hash: "hash-1".to_owned(),
                variant: "preview-768".to_owned(),
            },
        })
        .expect("seed analysis");
    let thumbnail_cache_maintenance = std::sync::Arc::new(FakeThumbnailCacheMaintenance::default());
    let settings_repository = fake_app_settings_repository(96 * 1024 * 1024, 3);
    let runner = ModImportTaskRunner::new(
        task_manager,
        std::sync::Arc::new(ModImportPrepareService::new(
            Box::new(FakePackagePreparer::new(
                "unused-task",
                Path::new("C:/mods/unused.zip"),
                "pkg-unused",
                Path::new("sandbox"),
            )),
            ModImportAnalysisService::new(
                Box::new(FakePreviewImageProcessor {
                    result: PreviewImageProcessingResult::Fallback(
                        PreviewImageRejectionReason::Missing,
                    ),
                }),
                Box::new(FakeThumbnailStore::default()),
                Box::new(FakeMetadataAnalyzer::default()),
            ),
        )),
        result_repository,
    )
    .with_thumbnail_cache_maintenance(thumbnail_cache_maintenance.clone())
    .with_app_settings_repository(settings_repository);

    runner.maintain_thumbnail_cache_now();

    let calls = thumbnail_cache_maintenance
        .calls
        .lock()
        .expect("calls lock");
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].max_bytes, Some(96 * 1024 * 1024));
    assert_eq!(calls[0].max_age, Some(days(3)));
    assert_eq!(
        calls[0].retained,
        vec![ThumbnailRef {
            package_id: "pkg-1".to_owned(),
            variant: "preview-768".to_owned(),
            content_hash: "hash-1".to_owned(),
        }]
    );
}

#[test]
fn task_runner_completes_when_thumbnail_cache_maintenance_fails() {
    let task_manager = std::sync::Arc::new(crate::TaskManager::new());
    let task = task_manager
        .create_task(crate::TaskKind::ModImport)
        .expect("task can be created");
    let result_repository = std::sync::Arc::new(FakeModImportResultRepository::default());
    let runner = ModImportTaskRunner::new(
        std::sync::Arc::clone(&task_manager),
        std::sync::Arc::new(ModImportPrepareService::new(
            Box::new(FakePackagePreparer::new(
                &task.task_id,
                Path::new("C:/mods/sample.zip"),
                "pkg-1",
                Path::new("sandbox"),
            )),
            ModImportAnalysisService::new(
                Box::new(FakePreviewImageProcessor {
                    result: sample_thumbnail_result(),
                }),
                Box::new(FakeThumbnailStore::default()),
                Box::new(FakeMetadataAnalyzer::default()),
            ),
        )),
        result_repository.clone(),
    )
    .with_thumbnail_cache_maintenance(std::sync::Arc::new(FakeThumbnailCacheMaintenance {
        fail: true,
        ..FakeThumbnailCacheMaintenance::default()
    }));

    let events = runner
        .run_prepare_task(&task.task_id, Path::new("C:/mods/sample.zip").to_path_buf())
        .expect("maintenance failure does not fail import");

    assert_eq!(
        task_manager.task_status(&task.task_id),
        Some(crate::TaskStatus::Completed)
    );
    assert_eq!(
        event_phases(&events).last(),
        Some(&"mod_import.prepare.completed")
    );
    assert!(
        result_repository
            .get_analysis("pkg-1")
            .expect("repository read succeeds")
            .is_some(),
        "analysis remains persisted when cache maintenance fails"
    );
}

#[test]
fn library_service_summarizes_preview_image_diagnostics_without_content() {
    let result_repository = std::sync::Arc::new(FakeModImportResultRepository::default());
    for record in [
        StoredModImportAnalysis {
            mod_id: "pkg-thumbnail".to_owned(),
            task_id: "task-1".to_owned(),
            package_id: "pkg-thumbnail".to_owned(),
            display_name: "Thumbnail Mod".to_owned(),
            metadata: StoredModPackageMetadata::default(),
            preview_image: StoredImportPreviewImage::Thumbnail {
                thumbnail_url: "thumbnail://pkg-thumbnail/preview-768/hash-1".to_owned(),
                width: 320,
                height: 180,
                content_hash: "hash-1".to_owned(),
                variant: "preview-768".to_owned(),
            },
        },
        StoredModImportAnalysis {
            mod_id: "pkg-missing".to_owned(),
            task_id: "task-2".to_owned(),
            package_id: "pkg-missing".to_owned(),
            display_name: "Missing Preview".to_owned(),
            metadata: StoredModPackageMetadata::default(),
            preview_image: StoredImportPreviewImage::Fallback {
                reason: PreviewImageRejectionReason::Missing,
            },
        },
        StoredModImportAnalysis {
            mod_id: "pkg-decode".to_owned(),
            task_id: "task-3".to_owned(),
            package_id: "pkg-decode".to_owned(),
            display_name: "Decode Failed".to_owned(),
            metadata: StoredModPackageMetadata::default(),
            preview_image: StoredImportPreviewImage::Fallback {
                reason: PreviewImageRejectionReason::DecodeFailed,
            },
        },
        StoredModImportAnalysis {
            mod_id: "pkg-decode-2".to_owned(),
            task_id: "task-4".to_owned(),
            package_id: "pkg-decode-2".to_owned(),
            display_name: "Decode Failed Again".to_owned(),
            metadata: StoredModPackageMetadata::default(),
            preview_image: StoredImportPreviewImage::Fallback {
                reason: PreviewImageRejectionReason::DecodeFailed,
            },
        },
    ] {
        result_repository
            .save_analysis(&record)
            .expect("save analysis");
    }
    let service = ModLibraryService::new(
        result_repository,
        empty_metadata_repo(),
        empty_category_repo(),
    );

    let summary = service
        .get_preview_image_diagnostics()
        .expect("diagnostics query succeeds");

    assert_eq!(summary.total_imported_mods, 4);
    assert_eq!(summary.thumbnail_count, 1);
    assert_eq!(summary.fallback_count, 3);
    assert_eq!(
        summary.fallback_reasons,
        vec![
            PreviewImageFallbackDiagnostic {
                reason: PreviewImageRejectionReason::Missing,
                count: 1,
            },
            PreviewImageFallbackDiagnostic {
                reason: PreviewImageRejectionReason::DecodeFailed,
                count: 2,
            },
        ]
    );
    assert_eq!(
        summary.export_categories,
        vec![
            PreviewImageDiagnosticExportCategory {
                category: PreviewImageDiagnosticExportCategoryId::PreviewImageSummary,
                status: PreviewImageDiagnosticExportCategoryStatus::Included,
                reason: None,
            },
            PreviewImageDiagnosticExportCategory {
                category: PreviewImageDiagnosticExportCategoryId::ThumbnailFiles,
                status: PreviewImageDiagnosticExportCategoryStatus::Excluded,
                reason: Some(PreviewImageDiagnosticExportExclusionReason::DerivedImageContent),
            },
            PreviewImageDiagnosticExportCategory {
                category: PreviewImageDiagnosticExportCategoryId::ThumbnailUrls,
                status: PreviewImageDiagnosticExportCategoryStatus::Excluded,
                reason: Some(PreviewImageDiagnosticExportExclusionReason::OpaqueResourceReference),
            },
            PreviewImageDiagnosticExportCategory {
                category: PreviewImageDiagnosticExportCategoryId::RawPackageContent,
                status: PreviewImageDiagnosticExportCategoryStatus::Excluded,
                reason: Some(PreviewImageDiagnosticExportExclusionReason::ThirdPartyModContent),
            },
        ]
    );
}

#[test]
fn task_runner_marks_task_failed_without_exposing_paths() {
    let task_manager = std::sync::Arc::new(crate::TaskManager::new());
    let task = task_manager
        .create_task(crate::TaskKind::ModImport)
        .expect("task can be created");
    let archive_path = Path::new("C:/Users/Alice/Mods/bad.zip").to_path_buf();
    let runner = ModImportTaskRunner::new(
        std::sync::Arc::clone(&task_manager),
        std::sync::Arc::new(ModImportPrepareService::new(
            Box::new(FailingPackagePreparer),
            ModImportAnalysisService::new(
                Box::new(FakePreviewImageProcessor {
                    result: PreviewImageProcessingResult::Fallback(
                        PreviewImageRejectionReason::Missing,
                    ),
                }),
                Box::new(FakeThumbnailStore::default()),
                Box::new(FakeMetadataAnalyzer::default()),
            ),
        )),
        std::sync::Arc::new(FakeModImportResultRepository::default()),
    );

    let error = runner
        .run_prepare_task(&task.task_id, archive_path)
        .expect_err("runner fails");

    assert_eq!(
        task_manager.task_status(&task.task_id),
        Some(crate::TaskStatus::Failed)
    );
    assert_eq!(
        event_phases(&error.events),
        vec!["mod_import.unpack.failed"]
    );
    let failure = error.events.last().expect("failure event exists");
    assert_eq!(failure.status, crate::TaskStatus::Failed);
    assert_eq!(failure.error.as_deref(), Some("mod_import_prepare_failed"));
    assert!(!failure.error.as_deref().unwrap().contains("Alice"));
    assert!(!failure.error.as_deref().unwrap().contains("bad.zip"));
}

#[test]
fn task_runner_does_not_emit_failed_event_for_already_cancelled_task() {
    let task_manager = std::sync::Arc::new(crate::TaskManager::new());
    let task = task_manager
        .create_task(crate::TaskKind::ModImport)
        .expect("task can be created");
    task_manager
        .cancel_task(&task.task_id)
        .expect("task can be cancelled");
    let runner = ModImportTaskRunner::new(
        std::sync::Arc::clone(&task_manager),
        std::sync::Arc::new(ModImportPrepareService::new(
            Box::new(FakePackagePreparer::new(
                &task.task_id,
                Path::new("C:/mods/sample.zip"),
                "pkg-1",
                Path::new("sandbox"),
            )),
            ModImportAnalysisService::new(
                Box::new(FakePreviewImageProcessor {
                    result: PreviewImageProcessingResult::Fallback(
                        PreviewImageRejectionReason::Missing,
                    ),
                }),
                Box::new(FakeThumbnailStore::default()),
                Box::new(FakeMetadataAnalyzer::default()),
            ),
        )),
        std::sync::Arc::new(FakeModImportResultRepository::default()),
    );

    let error = runner
        .run_prepare_task(&task.task_id, Path::new("C:/mods/sample.zip").to_path_buf())
        .expect_err("cancelled task does not run");

    assert!(error.events.is_empty());
    assert_eq!(
        task_manager.task_status(&task.task_id),
        Some(crate::TaskStatus::Cancelled)
    );
}

#[test]
fn task_runner_does_not_complete_or_persist_when_cancelled_during_prepare() {
    let task_manager = std::sync::Arc::new(crate::TaskManager::new());
    let task = task_manager
        .create_task(crate::TaskKind::ModImport)
        .expect("task can be created");
    let result_repository = std::sync::Arc::new(FakeModImportResultRepository::default());
    let runner = ModImportTaskRunner::new(
        std::sync::Arc::clone(&task_manager),
        std::sync::Arc::new(ModImportPrepareService::new(
            Box::new(CancellingPackagePreparer {
                task_manager: std::sync::Arc::clone(&task_manager),
                task_id: task.task_id.clone(),
            }),
            ModImportAnalysisService::new(
                Box::new(FakePreviewImageProcessor {
                    result: sample_thumbnail_result(),
                }),
                Box::new(FakeThumbnailStore::default()),
                Box::new(FakeMetadataAnalyzer::default()),
            ),
        )),
        result_repository.clone(),
    );

    let error = runner
        .run_prepare_task(&task.task_id, Path::new("C:/mods/sample.zip").to_path_buf())
        .expect_err("cancelled running task stops after prepare checkpoint");

    assert!(error.events.is_empty());
    assert_eq!(
        task_manager.task_status(&task.task_id),
        Some(crate::TaskStatus::Cancelled)
    );
    assert!(
        result_repository
            .get_analysis("pkg-1")
            .expect("repository read succeeds")
            .is_none(),
        "cancelled prepare result must not be persisted"
    );
}

#[test]
fn task_runner_maintains_thumbnail_cache_when_cancelled_after_preview_processing() {
    let task_manager = std::sync::Arc::new(crate::TaskManager::new());
    let task = task_manager
        .create_task(crate::TaskKind::ModImport)
        .expect("task can be created");
    let result_repository = std::sync::Arc::new(FakeModImportResultRepository::default());
    let thumbnail_cache_maintenance = std::sync::Arc::new(FakeThumbnailCacheMaintenance::default());
    let runner = ModImportTaskRunner::new(
        std::sync::Arc::clone(&task_manager),
        std::sync::Arc::new(ModImportPrepareService::new(
            Box::new(FakePackagePreparer::new(
                &task.task_id,
                Path::new("C:/mods/sample.zip"),
                "pkg-1",
                Path::new("sandbox"),
            )),
            ModImportAnalysisService::new(
                Box::new(CancellingPreviewImageProcessor {
                    task_manager: std::sync::Arc::clone(&task_manager),
                    task_id: task.task_id.clone(),
                }),
                Box::new(FakeThumbnailStore::default()),
                Box::new(FakeMetadataAnalyzer::default()),
            ),
        )),
        result_repository.clone(),
    )
    .with_thumbnail_cache_maintenance(thumbnail_cache_maintenance.clone());

    let error = runner
        .run_prepare_task(&task.task_id, Path::new("C:/mods/sample.zip").to_path_buf())
        .expect_err("cancelled running task stops after preview checkpoint");

    assert!(error.events.is_empty());
    assert_eq!(
        task_manager.task_status(&task.task_id),
        Some(crate::TaskStatus::Cancelled)
    );
    assert!(result_repository
        .list_analysis()
        .expect("repository read succeeds")
        .is_empty());

    let calls = thumbnail_cache_maintenance
        .calls
        .lock()
        .expect("calls lock");
    assert_eq!(calls.len(), 1);
    assert!(calls[0].retained.is_empty());
    assert_eq!(calls[0].max_bytes, Some(DEFAULT_THUMBNAIL_CACHE_MAX_BYTES));
}

#[test]
fn task_runner_passes_running_cancellation_token_to_preparer() {
    let task_manager = std::sync::Arc::new(crate::TaskManager::new());
    let task = task_manager
        .create_task(crate::TaskKind::ModImport)
        .expect("task can be created");
    let result_repository = std::sync::Arc::new(FakeModImportResultRepository::default());
    let observed = std::sync::Arc::new(Mutex::new(Vec::new()));
    let runner = ModImportTaskRunner::new(
        std::sync::Arc::clone(&task_manager),
        std::sync::Arc::new(ModImportPrepareService::new(
            Box::new(CancellationObservingPackagePreparer {
                task_manager: std::sync::Arc::clone(&task_manager),
                task_id: task.task_id.clone(),
                observed: std::sync::Arc::clone(&observed),
            }),
            ModImportAnalysisService::new(
                Box::new(FakePreviewImageProcessor {
                    result: PreviewImageProcessingResult::Fallback(
                        PreviewImageRejectionReason::Missing,
                    ),
                }),
                Box::new(FakeThumbnailStore::default()),
                Box::new(FakeMetadataAnalyzer::default()),
            ),
        )),
        result_repository.clone(),
    );

    let error = runner
        .run_prepare_task(&task.task_id, Path::new("C:/mods/sample.zip").to_path_buf())
        .expect_err("cancelled running task stops after prepare checkpoint");

    assert!(error.events.is_empty());
    assert_eq!(
        observed.lock().expect("observed lock").as_slice(),
        &[false, true]
    );
    assert_eq!(
        task_manager.task_status(&task.task_id),
        Some(crate::TaskStatus::Cancelled)
    );
    assert!(result_repository
        .list_analysis()
        .expect("repository read succeeds")
        .is_empty());
}

struct FakePreviewImageProcessor {
    result: PreviewImageProcessingResult,
}

impl ImportPreviewImageProcessor for FakePreviewImageProcessor {
    fn process_package_preview(
        &self,
        _task_id: &str,
        _package_id: &str,
        _sandbox_root: &Path,
    ) -> anyhow::Result<PreviewImageProcessingResult> {
        Ok(self.result.clone())
    }
}

struct CancellationObservingPreviewImageProcessor {
    observed: std::sync::Arc<Mutex<Vec<bool>>>,
}

impl ImportPreviewImageProcessor for CancellationObservingPreviewImageProcessor {
    fn process_package_preview(
        &self,
        _task_id: &str,
        _package_id: &str,
        _sandbox_root: &Path,
    ) -> anyhow::Result<PreviewImageProcessingResult> {
        anyhow::bail!("preview processor should receive cancellation-aware call")
    }

    fn process_package_preview_with_cancellation(
        &self,
        _task_id: &str,
        _package_id: &str,
        _sandbox_root: &Path,
        cancellation_token: &dyn CancellationToken,
    ) -> anyhow::Result<PreviewImageProcessingResult> {
        self.observed
            .lock()
            .expect("observed lock")
            .push(cancellation_token.is_cancelled());
        Ok(PreviewImageProcessingResult::Fallback(
            PreviewImageRejectionReason::Missing,
        ))
    }
}

struct CancellingPreviewImageProcessor {
    task_manager: std::sync::Arc<crate::TaskManager>,
    task_id: String,
}

impl ImportPreviewImageProcessor for CancellingPreviewImageProcessor {
    fn process_package_preview(
        &self,
        _task_id: &str,
        _package_id: &str,
        _sandbox_root: &Path,
    ) -> anyhow::Result<PreviewImageProcessingResult> {
        anyhow::bail!("preview processor should receive cancellation-aware call")
    }

    fn process_package_preview_with_cancellation(
        &self,
        _task_id: &str,
        _package_id: &str,
        _sandbox_root: &Path,
        cancellation_token: &dyn CancellationToken,
    ) -> anyhow::Result<PreviewImageProcessingResult> {
        assert!(!cancellation_token.is_cancelled());
        self.task_manager
            .cancel_task(&self.task_id)
            .expect("running task can be cancelled");

        Ok(sample_thumbnail_result())
    }
}

struct TestCancellationToken {
    cancelled: bool,
}

impl CancellationToken for TestCancellationToken {
    fn is_cancelled(&self) -> bool {
        self.cancelled
    }
}

#[derive(Default)]
struct FakeMetadataAnalyzer {
    display_name: Option<String>,
    manifest_display_name: Option<String>,
    version: Option<String>,
    author: Option<String>,
    category: Option<String>,
    tags: Vec<String>,
    dependencies: Vec<String>,
}

impl ModPackageMetadataAnalyzer for FakeMetadataAnalyzer {
    fn analyze_metadata(
        &self,
        _package_id: &str,
        _sandbox_root: &Path,
    ) -> anyhow::Result<ModPackageMetadataAnalysis> {
        Ok(ModPackageMetadataAnalysis {
            metadata: ModPackageMetadata {
                display_name: self.display_name.clone(),
                version: self.version.clone(),
                author: self.author.clone(),
                category: self.category.clone(),
                tags: self.tags.clone(),
                dependencies: self.dependencies.clone(),
            },
            manifest_display_name: self.manifest_display_name.clone(),
        })
    }
}

struct FakePackagePreparer {
    expected_task_id: String,
    expected_archive_path: std::path::PathBuf,
    package_id: String,
    sandbox_root: std::path::PathBuf,
}

impl FakePackagePreparer {
    fn new(
        expected_task_id: &str,
        expected_archive_path: &Path,
        package_id: &str,
        sandbox_root: &Path,
    ) -> Self {
        Self {
            expected_task_id: expected_task_id.to_owned(),
            expected_archive_path: expected_archive_path.to_path_buf(),
            package_id: package_id.to_owned(),
            sandbox_root: sandbox_root.to_path_buf(),
        }
    }
}

impl ModImportPackagePreparer for FakePackagePreparer {
    fn prepare_package(
        &self,
        request: ModImportPackagePrepareRequest<'_>,
    ) -> anyhow::Result<PreparedModPackage> {
        assert_eq!(request.task_id, self.expected_task_id);
        assert_eq!(request.archive_path, self.expected_archive_path);

        Ok(PreparedModPackage {
            package_id: self.package_id.clone(),
            sandbox_root: self.sandbox_root.clone(),
        })
    }
}

struct FailingPackagePreparer;

impl ModImportPackagePreparer for FailingPackagePreparer {
    fn prepare_package(
        &self,
        _request: ModImportPackagePrepareRequest<'_>,
    ) -> anyhow::Result<PreparedModPackage> {
        anyhow::bail!("failed to prepare C:/Users/Alice/Mods/bad.zip")
    }
}

struct CancellingPackagePreparer {
    task_manager: std::sync::Arc<crate::TaskManager>,
    task_id: String,
}

impl ModImportPackagePreparer for CancellingPackagePreparer {
    fn prepare_package(
        &self,
        request: ModImportPackagePrepareRequest<'_>,
    ) -> anyhow::Result<PreparedModPackage> {
        assert_eq!(request.task_id, self.task_id);
        self.task_manager
            .cancel_task(&self.task_id)
            .expect("running task can be cancelled");

        Ok(PreparedModPackage {
            package_id: "pkg-1".to_owned(),
            sandbox_root: Path::new("sandbox").to_path_buf(),
        })
    }
}

struct CancellationObservingPackagePreparer {
    task_manager: std::sync::Arc<crate::TaskManager>,
    task_id: String,
    observed: std::sync::Arc<Mutex<Vec<bool>>>,
}

impl ModImportPackagePreparer for CancellationObservingPackagePreparer {
    fn prepare_package(
        &self,
        request: ModImportPackagePrepareRequest<'_>,
    ) -> anyhow::Result<PreparedModPackage> {
        assert_eq!(request.task_id, self.task_id);
        self.observed
            .lock()
            .expect("observed lock")
            .push(request.cancellation_token.is_cancelled());
        self.task_manager
            .cancel_task(&self.task_id)
            .expect("running task can be cancelled");
        self.observed
            .lock()
            .expect("observed lock")
            .push(request.cancellation_token.is_cancelled());

        Ok(PreparedModPackage {
            package_id: "pkg-1".to_owned(),
            sandbox_root: Path::new("sandbox").to_path_buf(),
        })
    }
}

#[derive(Default)]
struct FakeThumbnailStore {
    fail_resolve: bool,
}

impl ThumbnailStore for FakeThumbnailStore {
    fn put_thumbnail(
        &self,
        _package_id: &str,
        _content_hash: &str,
        _variant: &str,
        _extension: &str,
        _bytes: &[u8],
    ) -> anyhow::Result<ThumbnailRef> {
        unreachable!("import analysis should not write thumbnails")
    }

    fn resolve_url(&self, thumbnail_ref: &ThumbnailRef) -> anyhow::Result<String> {
        if self.fail_resolve {
            anyhow::bail!("thumbnail url unavailable");
        }

        Ok(format!(
            "thumbnail://{}/{}/{}",
            thumbnail_ref.package_id, thumbnail_ref.variant, thumbnail_ref.content_hash
        ))
    }
}

#[derive(Default)]
struct FakeModImportResultRepository {
    records: Mutex<Vec<StoredModImportAnalysis>>,
}

impl ModImportResultRepository for FakeModImportResultRepository {
    fn save_analysis(&self, analysis: &StoredModImportAnalysis) -> anyhow::Result<()> {
        let mut records = self.records.lock().expect("records lock");
        records.retain(|record| record.mod_id != analysis.mod_id);
        records.push(analysis.clone());
        Ok(())
    }

    fn list_analysis(&self) -> anyhow::Result<Vec<StoredModImportAnalysis>> {
        Ok(self.records.lock().expect("records lock").clone())
    }

    fn get_analysis(&self, mod_id: &str) -> anyhow::Result<Option<StoredModImportAnalysis>> {
        Ok(self
            .records
            .lock()
            .expect("records lock")
            .iter()
            .find(|record| record.mod_id == mod_id)
            .cloned())
    }
}

struct EmptyMetadataRepo;
impl ModMetadataRepository for EmptyMetadataRepo {
    fn get(&self, _: &str) -> anyhow::Result<Option<ModMetadataOverlay>> {
        Ok(None)
    }
    fn save(&self, _: &ModMetadataOverlay) -> anyhow::Result<()> {
        Ok(())
    }
    fn delete(&self, _: &str) -> anyhow::Result<()> {
        Ok(())
    }
    fn list_all(&self) -> anyhow::Result<Vec<ModMetadataOverlay>> {
        Ok(vec![])
    }
}

fn empty_metadata_repo() -> Arc<EmptyMetadataRepo> {
    Arc::new(EmptyMetadataRepo)
}
use crate::category::test_support::empty_category_repo;

#[derive(Default)]
struct FakeThumbnailCacheMaintenance {
    calls: Mutex<Vec<FakeThumbnailCacheMaintenanceCall>>,
    fail: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FakeThumbnailCacheMaintenanceCall {
    retained: Vec<ThumbnailRef>,
    max_bytes: Option<u64>,
    max_age: Option<Duration>,
}

impl ThumbnailCacheMaintenance for FakeThumbnailCacheMaintenance {
    fn maintain_thumbnail_cache(
        &self,
        request: ThumbnailCacheMaintenanceRequest<'_>,
    ) -> anyhow::Result<()> {
        self.calls
            .lock()
            .expect("calls lock")
            .push(FakeThumbnailCacheMaintenanceCall {
                retained: request.retained.to_vec(),
                max_bytes: request.max_bytes,
                max_age: request.max_age,
            });

        if self.fail {
            anyhow::bail!("cache maintenance unavailable");
        }

        Ok(())
    }
}

#[derive(Default)]
struct FakeAppSettingsRepository {
    settings: AppSettings,
    load_count: Mutex<usize>,
}

impl AppSettingsRepository for FakeAppSettingsRepository {
    fn load_settings(&self) -> AppSettingsRepositoryResult<AppSettings> {
        *self.load_count.lock().expect("load count lock") += 1;
        Ok(self.settings.clone())
    }

    fn save_settings(&self, _settings: &AppSettings) -> AppSettingsRepositoryResult<()> {
        Ok(())
    }
}

impl FakeAppSettingsRepository {
    fn load_count(&self) -> usize {
        *self.load_count.lock().expect("load count lock")
    }
}

fn fake_app_settings_repository(
    max_bytes: u64,
    max_age_days: u32,
) -> Arc<FakeAppSettingsRepository> {
    Arc::new(FakeAppSettingsRepository {
        settings: AppSettings {
            thumbnail_cache_max_bytes: Some(max_bytes),
            thumbnail_cache_max_age_days: Some(max_age_days),
            log_storage_max_bytes: None,
            debug_log_enabled: false,
            mod_storage_dir: None,
        },
        load_count: Mutex::new(0),
    })
}

fn days(days: u64) -> Duration {
    Duration::from_secs(days * 24 * 60 * 60)
}

fn event_phases(events: &[crate::TaskProgressEvent]) -> Vec<&str> {
    events.iter().map(|event| event.phase.as_str()).collect()
}

fn sample_thumbnail_result() -> PreviewImageProcessingResult {
    PreviewImageProcessingResult::Thumbnail(ProcessedPreviewImage {
        thumbnail_ref: ThumbnailRef {
            package_id: "pkg-1".to_owned(),
            variant: "preview-768".to_owned(),
            content_hash: "hash-1".to_owned(),
        },
        width: 320,
        height: 180,
        content_hash: "hash-1".to_owned(),
    })
}
