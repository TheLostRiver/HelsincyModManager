use crate::ImportPreviewImageProcessor;
use anyhow::Result;
use hmm_core::{PreviewImagePolicy, PreviewImageRejectionReason};
use hmm_ports::{PackagePreviewScanner, PreviewImageProcessingResult, PreviewImageProcessor};
use std::path::Path;
use std::sync::{Condvar, Mutex};

pub const DEFAULT_PREVIEW_IMAGE_PROCESSING_CONCURRENCY: usize = 2;

pub struct LimitedPreviewImageProcessor {
    inner: Box<dyn PreviewImageProcessor>,
    limiter: ProcessingLimiter,
}

impl LimitedPreviewImageProcessor {
    pub fn new(inner: Box<dyn PreviewImageProcessor>, max_concurrent: usize) -> Self {
        Self {
            inner,
            limiter: ProcessingLimiter::new(max_concurrent.max(1)),
        }
    }
}

impl PreviewImageProcessor for LimitedPreviewImageProcessor {
    fn process_candidate(
        &self,
        sandbox_root: &Path,
        candidate: &hmm_ports::PreviewImageCandidate,
        policy: &PreviewImagePolicy,
    ) -> Result<PreviewImageProcessingResult> {
        let _permit = self.limiter.acquire();
        self.inner
            .process_candidate(sandbox_root, candidate, policy)
    }
}

struct ProcessingLimiter {
    max_concurrent: usize,
    active: Mutex<usize>,
    available: Condvar,
}

impl ProcessingLimiter {
    fn new(max_concurrent: usize) -> Self {
        Self {
            max_concurrent,
            active: Mutex::new(0),
            available: Condvar::new(),
        }
    }

    fn acquire(&self) -> ProcessingPermit<'_> {
        let mut active = self
            .active
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        while *active >= self.max_concurrent {
            active = self
                .available
                .wait(active)
                .unwrap_or_else(|poisoned| poisoned.into_inner());
        }
        *active += 1;

        ProcessingPermit { limiter: self }
    }
}

struct ProcessingPermit<'a> {
    limiter: &'a ProcessingLimiter,
}

impl Drop for ProcessingPermit<'_> {
    fn drop(&mut self) {
        let mut active = self
            .limiter
            .active
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        *active = active.saturating_sub(1);
        self.limiter.available.notify_one();
    }
}

pub struct PreviewImageService {
    policy: PreviewImagePolicy,
    scanner: Box<dyn PackagePreviewScanner>,
    processor: Box<dyn PreviewImageProcessor>,
}

impl PreviewImageService {
    pub fn new(
        policy: PreviewImagePolicy,
        scanner: Box<dyn PackagePreviewScanner>,
        processor: Box<dyn PreviewImageProcessor>,
    ) -> Self {
        Self {
            policy,
            scanner,
            processor,
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
        for candidate in candidates
            .into_iter()
            .take(self.policy.max_candidates_per_package)
        {
            match self
                .processor
                .process_candidate(sandbox_root, &candidate, &self.policy)?
            {
                PreviewImageProcessingResult::Thumbnail(thumbnail) => {
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
    };
    use std::path::Path;
    use std::sync::{Arc, Barrier, Mutex};
    use std::time::Duration;

    #[test]
    fn returns_missing_fallback_when_no_candidates_exist() {
        let service = PreviewImageService::new(
            PreviewImagePolicy::default(),
            Box::new(FakeScanner::new(vec![])),
            Box::new(FakeProcessor::new(vec![])),
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
        );

        let result = service
            .process_package_preview("task-1", "pkg-1", Path::new("sandbox"))
            .expect("preview result");

        assert!(matches!(result, PreviewImageProcessingResult::Thumbnail(_)));
    }

    #[test]
    fn returns_first_processed_thumbnail_without_resolving_url() {
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
        );

        let result = service
            .process_package_preview("task-1", "pkg-1", Path::new("sandbox"))
            .expect("preview result");

        assert!(matches!(result, PreviewImageProcessingResult::Thumbnail(_)));
    }

    #[test]
    fn processes_only_policy_candidate_limit_when_scanner_returns_more() {
        let first = preview_candidate("pkg-1", "first.png");
        let second = preview_candidate("pkg-1", "second.png");
        let third = preview_candidate("pkg-1", "third.png");
        let processed_paths = Arc::new(Mutex::new(Vec::new()));
        let service = PreviewImageService::new(
            PreviewImagePolicy {
                max_candidates_per_package: 2,
                ..PreviewImagePolicy::default()
            },
            Box::new(FakeScanner::new(vec![first, second, third])),
            Box::new(RecordingProcessor::new(
                Arc::clone(&processed_paths),
                vec![
                    PreviewImageProcessingResult::Fallback(
                        PreviewImageRejectionReason::DecodeFailed,
                    ),
                    PreviewImageProcessingResult::Fallback(
                        PreviewImageRejectionReason::UnsupportedFormat,
                    ),
                    PreviewImageProcessingResult::Fallback(
                        PreviewImageRejectionReason::CacheWriteFailed,
                    ),
                ],
            )),
        );

        let result = service
            .process_package_preview("task-1", "pkg-1", Path::new("sandbox"))
            .expect("preview result");

        assert_eq!(
            result,
            PreviewImageProcessingResult::Fallback(PreviewImageRejectionReason::UnsupportedFormat)
        );
        assert_eq!(
            *processed_paths.lock().expect("processed paths lock"),
            vec!["first.png".to_owned(), "second.png".to_owned()]
        );
    }

    #[test]
    fn limited_processor_caps_concurrent_candidate_processing() {
        let stats = Arc::new(Mutex::new(ConcurrencyStats::default()));
        let processor = Arc::new(LimitedPreviewImageProcessor::new(
            Box::new(BlockingProcessor {
                stats: Arc::clone(&stats),
            }),
            2,
        ));
        let barrier = Arc::new(Barrier::new(4));
        let candidate = preview_candidate("pkg-1", "good.png");
        let policy = PreviewImagePolicy::default();

        let handles: Vec<_> = (0..4)
            .map(|_| {
                let processor = Arc::clone(&processor);
                let barrier = Arc::clone(&barrier);
                let candidate = candidate.clone();
                let policy = policy.clone();
                std::thread::spawn(move || {
                    barrier.wait();
                    processor
                        .process_candidate(Path::new("sandbox"), &candidate, &policy)
                        .expect("preview processing succeeds");
                })
            })
            .collect();

        for handle in handles {
            handle.join().expect("thread joins");
        }

        assert_eq!(stats.lock().expect("stats lock").max_active, 2);
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

    struct RecordingProcessor {
        processed_paths: Arc<Mutex<Vec<String>>>,
        results: Mutex<Vec<PreviewImageProcessingResult>>,
    }

    impl RecordingProcessor {
        fn new(
            processed_paths: Arc<Mutex<Vec<String>>>,
            results: Vec<PreviewImageProcessingResult>,
        ) -> Self {
            Self {
                processed_paths,
                results: Mutex::new(results.into_iter().rev().collect()),
            }
        }
    }

    impl PreviewImageProcessor for RecordingProcessor {
        fn process_candidate(
            &self,
            _sandbox_root: &Path,
            candidate: &PreviewImageCandidate,
            _policy: &PreviewImagePolicy,
        ) -> Result<PreviewImageProcessingResult> {
            self.processed_paths
                .lock()
                .expect("processed paths lock")
                .push(candidate.source_ref.logical_path.clone());

            Ok(self
                .results
                .lock()
                .expect("processor lock")
                .pop()
                .expect("processor result"))
        }
    }

    #[derive(Default)]
    struct ConcurrencyStats {
        active: usize,
        max_active: usize,
    }

    struct BlockingProcessor {
        stats: Arc<Mutex<ConcurrencyStats>>,
    }

    impl PreviewImageProcessor for BlockingProcessor {
        fn process_candidate(
            &self,
            _sandbox_root: &Path,
            _candidate: &PreviewImageCandidate,
            _policy: &PreviewImagePolicy,
        ) -> Result<PreviewImageProcessingResult> {
            {
                let mut stats = self.stats.lock().expect("stats lock");
                stats.active += 1;
                stats.max_active = stats.max_active.max(stats.active);
            }

            std::thread::sleep(Duration::from_millis(50));

            {
                let mut stats = self.stats.lock().expect("stats lock");
                stats.active -= 1;
            }

            Ok(PreviewImageProcessingResult::Fallback(
                PreviewImageRejectionReason::Missing,
            ))
        }
    }
}
