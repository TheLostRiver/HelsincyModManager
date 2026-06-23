use crate::ImportPreviewImageProcessor;
use anyhow::Result;
use hmm_core::{PreviewImagePolicy, PreviewImageRejectionReason};
use hmm_ports::{
    PackagePreviewScanner, PreviewImageProcessingResult, PreviewImageProcessor, ThumbnailStore,
};
use std::path::Path;

pub struct PreviewImageService {
    policy: PreviewImagePolicy,
    scanner: Box<dyn PackagePreviewScanner>,
    processor: Box<dyn PreviewImageProcessor>,
    thumbnail_store: Box<dyn ThumbnailStore>,
}

impl PreviewImageService {
    pub fn new(
        policy: PreviewImagePolicy,
        scanner: Box<dyn PackagePreviewScanner>,
        processor: Box<dyn PreviewImageProcessor>,
        thumbnail_store: Box<dyn ThumbnailStore>,
    ) -> Self {
        Self {
            policy,
            scanner,
            processor,
            thumbnail_store,
        }
    }

    pub fn process_package_preview(
        &self,
        _task_id: &str,
        package_id: &str,
        sandbox_root: &Path,
    ) -> Result<PreviewImageProcessingResult> {
        self.policy.validate()?;

        let candidates = self
            .scanner
            .scan_candidates(package_id, sandbox_root, &self.policy)?;
        if candidates.is_empty() {
            return Ok(PreviewImageProcessingResult::Fallback(
                PreviewImageRejectionReason::Missing,
            ));
        }

        let mut last_reason = PreviewImageRejectionReason::Missing;
        for candidate in candidates {
            match self
                .processor
                .process_candidate(sandbox_root, &candidate, &self.policy)?
            {
                PreviewImageProcessingResult::Thumbnail(thumbnail) => {
                    if self
                        .thumbnail_store
                        .resolve_url(&thumbnail.thumbnail_ref)
                        .is_err()
                    {
                        last_reason = PreviewImageRejectionReason::CacheWriteFailed;
                        continue;
                    }
                    return Ok(PreviewImageProcessingResult::Thumbnail(thumbnail));
                }
                PreviewImageProcessingResult::Fallback(reason) => {
                    last_reason = reason;
                }
            }
        }

        Ok(PreviewImageProcessingResult::Fallback(last_reason))
    }
}

impl ImportPreviewImageProcessor for PreviewImageService {
    fn process_package_preview(
        &self,
        task_id: &str,
        package_id: &str,
        sandbox_root: &Path,
    ) -> Result<PreviewImageProcessingResult> {
        PreviewImageService::process_package_preview(self, task_id, package_id, sandbox_root)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;
    use hmm_core::{PreviewImagePolicy, PreviewImageRejectionReason};
    use hmm_ports::{
        PackagePreviewScanner, PreviewImageCandidate, PreviewImageProcessingResult,
        PreviewImageProcessor, PreviewImageSourceRef, ProcessedPreviewImage, ThumbnailRef,
        ThumbnailStore,
    };
    use std::path::Path;
    use std::sync::Mutex;

    #[test]
    fn returns_missing_fallback_when_no_candidates_exist() {
        let service = PreviewImageService::new(
            PreviewImagePolicy::default(),
            Box::new(FakeScanner::new(vec![])),
            Box::new(FakeProcessor::new(vec![])),
            Box::new(FakeThumbnailStore::default()),
        );

        let result = service
            .process_package_preview("task-1", "pkg-1", Path::new("sandbox"))
            .expect("preview result");

        assert_eq!(
            result,
            PreviewImageProcessingResult::Fallback(PreviewImageRejectionReason::Missing)
        );
    }

    #[test]
    fn tries_next_candidate_after_fallback() {
        let first = preview_candidate("pkg-1", "bad.png");
        let second = preview_candidate("pkg-1", "good.png");
        let service = PreviewImageService::new(
            PreviewImagePolicy::default(),
            Box::new(FakeScanner::new(vec![first, second])),
            Box::new(FakeProcessor::new(vec![
                PreviewImageProcessingResult::Fallback(PreviewImageRejectionReason::DecodeFailed),
                PreviewImageProcessingResult::Thumbnail(ProcessedPreviewImage {
                    thumbnail_ref: ThumbnailRef {
                        package_id: "pkg-1".to_owned(),
                        content_hash: "hash".to_owned(),
                        variant: "preview-768".to_owned(),
                    },
                    width: 4,
                    height: 4,
                    content_hash: "hash".to_owned(),
                }),
            ])),
            Box::new(FakeThumbnailStore::default()),
        );

        let result = service
            .process_package_preview("task-1", "pkg-1", Path::new("sandbox"))
            .expect("preview result");

        assert!(matches!(result, PreviewImageProcessingResult::Thumbnail(_)));
    }

    #[test]
    fn returns_cache_fallback_when_thumbnail_url_resolution_fails() {
        let candidate = preview_candidate("pkg-1", "good.png");
        let service = PreviewImageService::new(
            PreviewImagePolicy::default(),
            Box::new(FakeScanner::new(vec![candidate])),
            Box::new(FakeProcessor::new(vec![
                PreviewImageProcessingResult::Thumbnail(ProcessedPreviewImage {
                    thumbnail_ref: ThumbnailRef {
                        package_id: "pkg-1".to_owned(),
                        content_hash: "hash".to_owned(),
                        variant: "preview-768".to_owned(),
                    },
                    width: 4,
                    height: 4,
                    content_hash: "hash".to_owned(),
                }),
            ])),
            Box::new(FakeThumbnailStore { fail_resolve: true }),
        );

        let result = service
            .process_package_preview("task-1", "pkg-1", Path::new("sandbox"))
            .expect("preview result should degrade to fallback");

        assert_eq!(
            result,
            PreviewImageProcessingResult::Fallback(PreviewImageRejectionReason::CacheWriteFailed)
        );
    }

    fn preview_candidate(package_id: &str, logical_path: &str) -> PreviewImageCandidate {
        PreviewImageCandidate {
            source_ref: PreviewImageSourceRef {
                package_id: package_id.to_owned(),
                logical_path: logical_path.to_owned(),
            },
            file_name: logical_path.to_owned(),
            compressed_size: 0,
            priority: 0,
        }
    }

    struct FakeScanner {
        candidates: Vec<PreviewImageCandidate>,
    }

    impl FakeScanner {
        fn new(candidates: Vec<PreviewImageCandidate>) -> Self {
            Self { candidates }
        }
    }

    impl PackagePreviewScanner for FakeScanner {
        fn scan_candidates(
            &self,
            _package_id: &str,
            _sandbox_root: &Path,
            _policy: &PreviewImagePolicy,
        ) -> Result<Vec<PreviewImageCandidate>> {
            Ok(self.candidates.clone())
        }
    }

    struct FakeProcessor {
        results: Mutex<Vec<PreviewImageProcessingResult>>,
    }

    impl FakeProcessor {
        fn new(results: Vec<PreviewImageProcessingResult>) -> Self {
            Self {
                results: Mutex::new(results.into_iter().rev().collect()),
            }
        }
    }

    impl PreviewImageProcessor for FakeProcessor {
        fn process_candidate(
            &self,
            _sandbox_root: &Path,
            _candidate: &PreviewImageCandidate,
            _policy: &PreviewImagePolicy,
        ) -> Result<PreviewImageProcessingResult> {
            Ok(self
                .results
                .lock()
                .expect("processor lock")
                .pop()
                .expect("processor result"))
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
        ) -> Result<ThumbnailRef> {
            unreachable!("app service should not call store directly")
        }

        fn resolve_url(&self, _thumbnail_ref: &ThumbnailRef) -> Result<String> {
            if self.fail_resolve {
                anyhow::bail!("resolve failed");
            }

            Ok("thumbnail://pkg-1/preview-768/hash".to_owned())
        }
    }
}
