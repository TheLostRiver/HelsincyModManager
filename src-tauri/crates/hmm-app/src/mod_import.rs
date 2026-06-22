use hmm_core::PreviewImageRejectionReason;
use hmm_ports::{PreviewImageProcessingResult, ThumbnailStore};
use std::path::{Path, PathBuf};

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
        PreviewImageProcessingResult, ProcessedPreviewImage, ThumbnailRef, ThumbnailStore,
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
}
