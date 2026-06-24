use hmm_core::PreviewImageRejectionReason;
use hmm_ports::{ModImportPackagePreparer, PreviewImageProcessingResult, ThumbnailStore};
use std::path::{Path, PathBuf};

const MOD_IMPORT_UNPACK_STARTED_PHASE: &str = "mod_import.unpack.started";
const MOD_IMPORT_UNPACK_COMPLETED_PHASE: &str = "mod_import.unpack.completed";
const MOD_IMPORT_PREVIEW_IMAGE_PROCESSING_PHASE: &str = "mod_import.preview_image.processing";
const MOD_IMPORT_PREVIEW_IMAGE_FALLBACK_PHASE: &str = "mod_import.preview_image.fallback";

pub trait ImportPreviewImageProcessor: Send + Sync {
    fn process_package_preview(
        &self,
        task_id: &str,
        package_id: &str,
        sandbox_root: &Path,
    ) -> anyhow::Result<PreviewImageProcessingResult>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModImportAnalysisRequest {
    pub task_id: String,
    pub package_id: String,
    pub sandbox_root: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModImportAnalysisResult {
    pub task_id: String,
    pub package_id: String,
    pub preview_image: ImportPreviewImage,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModImportPrepareRequest {
    pub task_id: String,
    pub archive_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModImportPrepareResult {
    pub analysis: ModImportAnalysisResult,
    pub events: Vec<crate::TaskProgressEvent>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportPreviewImage {
    Thumbnail {
        thumbnail_url: String,
        width: u32,
        height: u32,
        content_hash: String,
    },
    Fallback {
        reason: PreviewImageRejectionReason,
    },
}

pub struct ModImportPrepareService {
    package_preparer: Box<dyn ModImportPackagePreparer>,
    analysis_service: ModImportAnalysisService,
}

impl ModImportPrepareService {
    pub fn new(
        package_preparer: Box<dyn ModImportPackagePreparer>,
        analysis_service: ModImportAnalysisService,
    ) -> Self {
        Self {
            package_preparer,
            analysis_service,
        }
    }

    pub fn prepare_import(
        &self,
        request: ModImportPrepareRequest,
    ) -> anyhow::Result<ModImportPrepareResult> {
        let mut events = Vec::new();
        events.push(running_event(
            &request.task_id,
            MOD_IMPORT_UNPACK_STARTED_PHASE,
        ));

        let prepared_package = self
            .package_preparer
            .prepare_package(&request.task_id, &request.archive_path)?;
        events.push(running_event(
            &request.task_id,
            MOD_IMPORT_UNPACK_COMPLETED_PHASE,
        ));
        events.push(running_event(
            &request.task_id,
            MOD_IMPORT_PREVIEW_IMAGE_PROCESSING_PHASE,
        ));

        let analysis = self
            .analysis_service
            .analyze_sandbox(ModImportAnalysisRequest {
                task_id: request.task_id.clone(),
                package_id: prepared_package.package_id,
                sandbox_root: prepared_package.sandbox_root,
            })?;

        if matches!(analysis.preview_image, ImportPreviewImage::Fallback { .. }) {
            events.push(running_event(
                &request.task_id,
                MOD_IMPORT_PREVIEW_IMAGE_FALLBACK_PHASE,
            ));
        }

        Ok(ModImportPrepareResult { analysis, events })
    }
}

fn running_event(task_id: &str, phase: &str) -> crate::TaskProgressEvent {
    crate::TaskProgressEvent::new(
        task_id.to_owned(),
        crate::TaskKind::ModImport,
        crate::TaskStatus::Running,
        phase,
    )
}

pub struct ModImportAnalysisService {
    preview_image_processor: Box<dyn ImportPreviewImageProcessor>,
    thumbnail_store: Box<dyn ThumbnailStore>,
}

impl ModImportAnalysisService {
    pub fn new(
        preview_image_processor: Box<dyn ImportPreviewImageProcessor>,
        thumbnail_store: Box<dyn ThumbnailStore>,
    ) -> Self {
        Self {
            preview_image_processor,
            thumbnail_store,
        }
    }

    pub fn analyze_sandbox(
        &self,
        request: ModImportAnalysisRequest,
    ) -> anyhow::Result<ModImportAnalysisResult> {
        let preview_image = match self.preview_image_processor.process_package_preview(
            &request.task_id,
            &request.package_id,
            &request.sandbox_root,
        )? {
            PreviewImageProcessingResult::Thumbnail(thumbnail) => {
                match self.thumbnail_store.resolve_url(&thumbnail.thumbnail_ref) {
                    Ok(thumbnail_url) => ImportPreviewImage::Thumbnail {
                        thumbnail_url,
                        width: thumbnail.width,
                        height: thumbnail.height,
                        content_hash: thumbnail.content_hash,
                    },
                    Err(_) => ImportPreviewImage::Fallback {
                        reason: PreviewImageRejectionReason::CacheWriteFailed,
                    },
                }
            }
            PreviewImageProcessingResult::Fallback(reason) => {
                ImportPreviewImage::Fallback { reason }
            }
        };

        Ok(ModImportAnalysisResult {
            task_id: request.task_id,
            package_id: request.package_id,
            preview_image,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hmm_core::PreviewImageRejectionReason;
    use hmm_ports::{
        ModImportPackagePreparer, PreparedModPackage, PreviewImageProcessingResult,
        ProcessedPreviewImage, ThumbnailRef, ThumbnailStore,
    };
    use std::path::Path;

    #[test]
    fn analyze_sandbox_includes_preview_thumbnail() {
        let service = ModImportAnalysisService::new(
            Box::new(FakePreviewImageProcessor {
                result: PreviewImageProcessingResult::Thumbnail(ProcessedPreviewImage {
                    thumbnail_ref: ThumbnailRef {
                        package_id: "pkg-1".to_owned(),
                        variant: "preview-768".to_owned(),
                        content_hash: "hash-1".to_owned(),
                    },
                    width: 320,
                    height: 180,
                    content_hash: "hash-1".to_owned(),
                }),
            }),
            Box::new(FakeThumbnailStore::default()),
        );

        let result = service
            .analyze_sandbox(ModImportAnalysisRequest {
                task_id: "task-1".to_owned(),
                package_id: "pkg-1".to_owned(),
                sandbox_root: Path::new("sandbox").to_path_buf(),
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
            }
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
        );

        let result = service
            .analyze_sandbox(ModImportAnalysisRequest {
                task_id: "task-1".to_owned(),
                package_id: "pkg-1".to_owned(),
                sandbox_root: Path::new("sandbox").to_path_buf(),
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
    fn analyze_sandbox_falls_back_when_thumbnail_url_resolution_fails() {
        let service = ModImportAnalysisService::new(
            Box::new(FakePreviewImageProcessor {
                result: PreviewImageProcessingResult::Thumbnail(ProcessedPreviewImage {
                    thumbnail_ref: ThumbnailRef {
                        package_id: "pkg-1".to_owned(),
                        variant: "preview-768".to_owned(),
                        content_hash: "hash-1".to_owned(),
                    },
                    width: 320,
                    height: 180,
                    content_hash: "hash-1".to_owned(),
                }),
            }),
            Box::new(FakeThumbnailStore { fail_resolve: true }),
        );

        let result = service
            .analyze_sandbox(ModImportAnalysisRequest {
                task_id: "task-1".to_owned(),
                package_id: "pkg-1".to_owned(),
                sandbox_root: Path::new("sandbox").to_path_buf(),
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
                    result: PreviewImageProcessingResult::Thumbnail(ProcessedPreviewImage {
                        thumbnail_ref: ThumbnailRef {
                            package_id: "pkg-1".to_owned(),
                            variant: "preview-768".to_owned(),
                            content_hash: "hash-1".to_owned(),
                        },
                        width: 320,
                        height: 180,
                        content_hash: "hash-1".to_owned(),
                    }),
                }),
                Box::new(FakeThumbnailStore::default()),
            ),
        );

        let result = service
            .prepare_import(ModImportPrepareRequest {
                task_id: "task-1".to_owned(),
                archive_path: Path::new("C:/mods/sample.zip").to_path_buf(),
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
            ),
        );

        let result = service
            .prepare_import(ModImportPrepareRequest {
                task_id: "task-1".to_owned(),
                archive_path: Path::new("C:/mods/sample.zip").to_path_buf(),
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
            task_id: &str,
            archive_path: &Path,
        ) -> anyhow::Result<PreparedModPackage> {
            assert_eq!(task_id, self.expected_task_id);
            assert_eq!(archive_path, self.expected_archive_path);

            Ok(PreparedModPackage {
                package_id: self.package_id.clone(),
                sandbox_root: self.sandbox_root.clone(),
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

    fn event_phases(events: &[crate::TaskProgressEvent]) -> Vec<&str> {
        events.iter().map(|event| event.phase.as_str()).collect()
    }
}
