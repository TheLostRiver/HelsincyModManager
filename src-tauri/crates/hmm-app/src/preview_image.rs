use crate::{ImportPreviewImage, ImportPreviewImageProcessor};
use anyhow::Result;
use hmm_core::{PreviewImagePolicy, PreviewImageRejectionReason};
use hmm_ports::{
    CancellationToken, ModImportResultRepository, ModImportSandboxLocator, NeverCancelled,
    PackagePreviewScanner, PreviewImageCandidate, PreviewImageProcessRequest,
    PreviewImageProcessingResult, PreviewImageProcessor, PreviewImageScanRequest,
    StoredImportPreviewImage, ThumbnailStore,
};
use std::path::Path;
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

pub const DEFAULT_PREVIEW_IMAGE_PROCESSING_CONCURRENCY: usize = 2;
const PROCESSING_LIMITER_CANCEL_POLL_INTERVAL: Duration = Duration::from_millis(10);

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
        request: PreviewImageProcessRequest<'_>,
    ) -> Result<PreviewImageProcessingResult> {
        let _permit = self.limiter.acquire(request.cancellation_token)?;
        self.inner.process_candidate(request)
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

    fn acquire(&self, cancellation_token: &dyn CancellationToken) -> Result<ProcessingPermit<'_>> {
        ensure_not_cancelled(cancellation_token)?;
        let mut active = self
            .active
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        while *active >= self.max_concurrent {
            ensure_not_cancelled(cancellation_token)?;
            active = match self
                .available
                .wait_timeout(active, PROCESSING_LIMITER_CANCEL_POLL_INTERVAL)
            {
                Ok((active, _timeout)) => active,
                Err(poisoned) => poisoned.into_inner().0,
            };
        }
        ensure_not_cancelled(cancellation_token)?;
        *active += 1;

        Ok(ProcessingPermit { limiter: self })
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

pub struct PreviewImageCandidateListService {
    policy: PreviewImagePolicy,
    result_repository: Arc<dyn ModImportResultRepository>,
    sandbox_locator: Arc<dyn ModImportSandboxLocator>,
    scanner: Box<dyn PackagePreviewScanner>,
}

pub struct PreviewImageCandidateSelectionService {
    result_repository: Arc<dyn ModImportResultRepository>,
    sandbox_locator: Arc<dyn ModImportSandboxLocator>,
    preview_image_service: PreviewImageService,
    thumbnail_store: Box<dyn ThumbnailStore>,
}

pub struct PreviewImageDetailService {
    result_repository: Arc<dyn ModImportResultRepository>,
    sandbox_locator: Arc<dyn ModImportSandboxLocator>,
    preview_image_service: PreviewImageService,
    thumbnail_store: Box<dyn ThumbnailStore>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreviewImageCandidateList {
    pub mod_id: String,
    pub candidates: Vec<PreviewImageCandidateSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreviewImageCandidateSummary {
    pub candidate_index: usize,
    pub file_name: String,
    pub compressed_size_bytes: u64,
}

impl PreviewImageCandidateListService {
    pub fn new(
        policy: PreviewImagePolicy,
        result_repository: Arc<dyn ModImportResultRepository>,
        sandbox_locator: Arc<dyn ModImportSandboxLocator>,
        scanner: Box<dyn PackagePreviewScanner>,
    ) -> Self {
        Self {
            policy,
            result_repository,
            sandbox_locator,
            scanner,
        }
    }

    pub fn list_candidates(&self, mod_id: &str) -> Result<Option<PreviewImageCandidateList>> {
        self.policy.validate()?;
        let Some(record) = self.result_repository.get_analysis(mod_id)? else {
            return Ok(None);
        };

        let sandbox_root = self
            .sandbox_locator
            .sandbox_root_for_package(&record.package_id)?;
        let candidates = self.scanner.scan_candidates(PreviewImageScanRequest {
            package_id: &record.package_id,
            sandbox_root: &sandbox_root,
            policy: &self.policy,
            cancellation_token: &NeverCancelled,
        })?;
        let candidates = candidates
            .into_iter()
            .take(self.policy.max_candidates_per_package)
            .enumerate()
            .map(candidate_summary_from_candidate)
            .collect();

        Ok(Some(PreviewImageCandidateList {
            mod_id: record.mod_id,
            candidates,
        }))
    }
}

impl PreviewImageCandidateSelectionService {
    pub fn new(
        policy: PreviewImagePolicy,
        result_repository: Arc<dyn ModImportResultRepository>,
        sandbox_locator: Arc<dyn ModImportSandboxLocator>,
        scanner: Box<dyn PackagePreviewScanner>,
        processor: Box<dyn PreviewImageProcessor>,
        thumbnail_store: Box<dyn ThumbnailStore>,
    ) -> Self {
        Self {
            result_repository,
            sandbox_locator,
            preview_image_service: PreviewImageService::new(policy, scanner, processor),
            thumbnail_store,
        }
    }

    pub fn select_candidate(
        &self,
        mod_id: &str,
        candidate_index: usize,
    ) -> Result<Option<ImportPreviewImage>> {
        let Some(mut record) = self.result_repository.get_analysis(mod_id)? else {
            return Ok(None);
        };

        let sandbox_root = self
            .sandbox_locator
            .sandbox_root_for_package(&record.package_id)?;
        let preview_image = preview_image_from_processing_result(
            self.preview_image_service
                .process_selected_package_preview(
                    "preview-image-candidate-selection",
                    &record.package_id,
                    &sandbox_root,
                    candidate_index,
                )?,
            self.thumbnail_store.as_ref(),
        );
        record.preview_image = stored_preview_from_import(&preview_image);
        self.result_repository.save_analysis(&record)?;

        Ok(Some(preview_image))
    }
}

impl PreviewImageDetailService {
    pub fn new(
        policy: PreviewImagePolicy,
        result_repository: Arc<dyn ModImportResultRepository>,
        sandbox_locator: Arc<dyn ModImportSandboxLocator>,
        scanner: Box<dyn PackagePreviewScanner>,
        processor: Box<dyn PreviewImageProcessor>,
        thumbnail_store: Box<dyn ThumbnailStore>,
    ) -> Self {
        Self {
            result_repository,
            sandbox_locator,
            preview_image_service: PreviewImageService::new(policy, scanner, processor),
            thumbnail_store,
        }
    }

    pub fn get_detail_preview_image(&self, mod_id: &str) -> Result<Option<ImportPreviewImage>> {
        let Some(record) = self.result_repository.get_analysis(mod_id)? else {
            return Ok(None);
        };

        let sandbox_root = self
            .sandbox_locator
            .sandbox_root_for_package(&record.package_id)?;
        let preview_image = preview_image_from_processing_result(
            self.preview_image_service.process_package_preview(
                "preview-image-detail",
                &record.package_id,
                &sandbox_root,
            )?,
            self.thumbnail_store.as_ref(),
        );

        Ok(Some(preview_image))
    }
}

fn preview_image_from_processing_result(
    result: PreviewImageProcessingResult,
    thumbnail_store: &dyn ThumbnailStore,
) -> ImportPreviewImage {
    match result {
        PreviewImageProcessingResult::Thumbnail(thumbnail) => {
            match thumbnail_store.resolve_url(&thumbnail.thumbnail_ref) {
                Ok(thumbnail_url) => ImportPreviewImage::Thumbnail {
                    thumbnail_url,
                    width: thumbnail.width,
                    height: thumbnail.height,
                    content_hash: thumbnail.content_hash,
                    variant: thumbnail.thumbnail_ref.variant,
                },
                Err(_) => ImportPreviewImage::Fallback {
                    reason: PreviewImageRejectionReason::CacheWriteFailed,
                },
            }
        }
        PreviewImageProcessingResult::Fallback(reason) => ImportPreviewImage::Fallback { reason },
    }
}

fn candidate_summary_from_candidate(
    (candidate_index, candidate): (usize, PreviewImageCandidate),
) -> PreviewImageCandidateSummary {
    PreviewImageCandidateSummary {
        candidate_index,
        file_name: candidate.file_name,
        compressed_size_bytes: candidate.compressed_size,
    }
}

fn stored_preview_from_import(preview_image: &ImportPreviewImage) -> StoredImportPreviewImage {
    match preview_image {
        ImportPreviewImage::Thumbnail {
            thumbnail_url,
            width,
            height,
            content_hash,
            variant,
        } => StoredImportPreviewImage::Thumbnail {
            thumbnail_url: thumbnail_url.clone(),
            width: *width,
            height: *height,
            content_hash: content_hash.clone(),
            variant: variant.clone(),
        },
        ImportPreviewImage::Fallback { reason } => {
            StoredImportPreviewImage::Fallback { reason: *reason }
        }
    }
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
        task_id: &str,
        package_id: &str,
        sandbox_root: &Path,
    ) -> Result<PreviewImageProcessingResult> {
        self.process_package_preview_with_cancellation(
            task_id,
            package_id,
            sandbox_root,
            &NeverCancelled,
        )
    }

    pub fn process_package_preview_with_cancellation(
        &self,
        _task_id: &str,
        package_id: &str,
        sandbox_root: &Path,
        cancellation_token: &dyn CancellationToken,
    ) -> Result<PreviewImageProcessingResult> {
        self.policy.validate()?;
        ensure_not_cancelled(cancellation_token)?;

        let candidates = self.scanner.scan_candidates(PreviewImageScanRequest {
            package_id,
            sandbox_root,
            policy: &self.policy,
            cancellation_token,
        })?;
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
            ensure_not_cancelled(cancellation_token)?;
            match self
                .processor
                .process_candidate(PreviewImageProcessRequest {
                    sandbox_root,
                    candidate: &candidate,
                    policy: &self.policy,
                    cancellation_token,
                })? {
                PreviewImageProcessingResult::Thumbnail(thumbnail) => {
                    return Ok(PreviewImageProcessingResult::Thumbnail(thumbnail));
                }
                PreviewImageProcessingResult::Fallback(reason) => {
                    last_reason = reason;
                }
            }
            ensure_not_cancelled(cancellation_token)?;
        }

        Ok(PreviewImageProcessingResult::Fallback(last_reason))
    }

    pub fn process_selected_package_preview(
        &self,
        task_id: &str,
        package_id: &str,
        sandbox_root: &Path,
        selected_candidate_index: usize,
    ) -> Result<PreviewImageProcessingResult> {
        self.process_selected_package_preview_with_cancellation(
            task_id,
            package_id,
            sandbox_root,
            selected_candidate_index,
            &NeverCancelled,
        )
    }

    pub fn process_selected_package_preview_with_cancellation(
        &self,
        _task_id: &str,
        package_id: &str,
        sandbox_root: &Path,
        selected_candidate_index: usize,
        cancellation_token: &dyn CancellationToken,
    ) -> Result<PreviewImageProcessingResult> {
        self.policy.validate()?;
        ensure_not_cancelled(cancellation_token)?;

        let candidates = self.bounded_candidates(package_id, sandbox_root, cancellation_token)?;
        let Some(candidate) = candidates.into_iter().nth(selected_candidate_index) else {
            return Ok(PreviewImageProcessingResult::Fallback(
                PreviewImageRejectionReason::Missing,
            ));
        };

        ensure_not_cancelled(cancellation_token)?;
        self.processor
            .process_candidate(PreviewImageProcessRequest {
                sandbox_root,
                candidate: &candidate,
                policy: &self.policy,
                cancellation_token,
            })
    }

    fn bounded_candidates(
        &self,
        package_id: &str,
        sandbox_root: &Path,
        cancellation_token: &dyn CancellationToken,
    ) -> Result<Vec<hmm_ports::PreviewImageCandidate>> {
        let candidates = self.scanner.scan_candidates(PreviewImageScanRequest {
            package_id,
            sandbox_root,
            policy: &self.policy,
            cancellation_token,
        })?;

        Ok(candidates
            .into_iter()
            .take(self.policy.max_candidates_per_package)
            .collect())
    }
}

fn ensure_not_cancelled(cancellation_token: &dyn CancellationToken) -> Result<()> {
    if cancellation_token.is_cancelled() {
        anyhow::bail!("preview image processing cancelled");
    }

    Ok(())
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

    fn process_package_preview_with_cancellation(
        &self,
        task_id: &str,
        package_id: &str,
        sandbox_root: &Path,
        cancellation_token: &dyn CancellationToken,
    ) -> Result<PreviewImageProcessingResult> {
        PreviewImageService::process_package_preview_with_cancellation(
            self,
            task_id,
            package_id,
            sandbox_root,
            cancellation_token,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;
    use hmm_core::{PreviewImagePolicy, PreviewImageRejectionReason};
    use hmm_ports::{
        CancellationToken, ModImportResultRepository, ModImportSandboxLocator,
        PackagePreviewScanner, PreviewImageCandidate, PreviewImageProcessRequest,
        PreviewImageProcessingResult, PreviewImageProcessor, PreviewImageScanRequest,
        PreviewImageSourceRef, ProcessedPreviewImage, StoredImportPreviewImage,
        StoredModImportAnalysis, StoredModPackageMetadata, ThumbnailRef,
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
    fn processes_selected_candidate_by_bounded_scan_index() {
        let first = preview_candidate("pkg-1", "first.png");
        let second = preview_candidate("pkg-1", "second.png");
        let processed_paths = Arc::new(Mutex::new(Vec::new()));
        let service = PreviewImageService::new(
            PreviewImagePolicy::default(),
            Box::new(FakeScanner::new(vec![first, second])),
            Box::new(RecordingProcessor::new(
                Arc::clone(&processed_paths),
                vec![PreviewImageProcessingResult::Thumbnail(
                    ProcessedPreviewImage {
                        thumbnail_ref: ThumbnailRef {
                            package_id: "pkg-1".to_owned(),
                            content_hash: "hash".to_owned(),
                            variant: "preview-768".to_owned(),
                        },
                        width: 4,
                        height: 4,
                        content_hash: "hash".to_owned(),
                    },
                )],
            )),
        );

        let result = service
            .process_selected_package_preview("task-1", "pkg-1", Path::new("sandbox"), 1)
            .expect("selected preview result");

        assert!(matches!(result, PreviewImageProcessingResult::Thumbnail(_)));
        assert_eq!(
            *processed_paths.lock().expect("processed paths lock"),
            vec!["second.png".to_owned()]
        );
    }

    #[test]
    fn selected_candidate_index_out_of_range_returns_missing_without_processing() {
        let candidate = preview_candidate("pkg-1", "first.png");
        let processed_paths = Arc::new(Mutex::new(Vec::new()));
        let service = PreviewImageService::new(
            PreviewImagePolicy::default(),
            Box::new(FakeScanner::new(vec![candidate])),
            Box::new(RecordingProcessor::new(
                Arc::clone(&processed_paths),
                vec![PreviewImageProcessingResult::Thumbnail(
                    ProcessedPreviewImage {
                        thumbnail_ref: ThumbnailRef {
                            package_id: "pkg-1".to_owned(),
                            content_hash: "hash".to_owned(),
                            variant: "preview-768".to_owned(),
                        },
                        width: 4,
                        height: 4,
                        content_hash: "hash".to_owned(),
                    },
                )],
            )),
        );

        let result = service
            .process_selected_package_preview("task-1", "pkg-1", Path::new("sandbox"), 4)
            .expect("selected preview result");

        assert_eq!(
            result,
            PreviewImageProcessingResult::Fallback(PreviewImageRejectionReason::Missing)
        );
        assert!(processed_paths
            .lock()
            .expect("processed paths lock")
            .is_empty());
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
                        .process_candidate(PreviewImageProcessRequest {
                            sandbox_root: Path::new("sandbox"),
                            candidate: &candidate,
                            policy: &policy,
                            cancellation_token: &hmm_ports::NeverCancelled,
                        })
                        .expect("preview processing succeeds");
                })
            })
            .collect();

        for handle in handles {
            handle.join().expect("thread joins");
        }

        assert_eq!(stats.lock().expect("stats lock").max_active, 2);
    }

    #[test]
    fn limited_processor_returns_when_cancelled_while_waiting_for_permit() {
        let (entered_tx, entered_rx) = std::sync::mpsc::channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        let inner_calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let processor = Arc::new(LimitedPreviewImageProcessor::new(
            Box::new(PermitHoldingProcessor {
                entered_tx: Mutex::new(Some(entered_tx)),
                release_rx: Mutex::new(release_rx),
                inner_calls: Arc::clone(&inner_calls),
            }),
            1,
        ));
        let candidate = preview_candidate("pkg-1", "good.png");
        let policy = PreviewImagePolicy::default();

        let first_processor = Arc::clone(&processor);
        let first_candidate = candidate.clone();
        let first_policy = policy.clone();
        let first_handle = std::thread::spawn(move || {
            first_processor.process_candidate(PreviewImageProcessRequest {
                sandbox_root: Path::new("sandbox"),
                candidate: &first_candidate,
                policy: &first_policy,
                cancellation_token: &hmm_ports::NeverCancelled,
            })
        });
        entered_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("first processor acquired permit");

        let cancellation_token = Arc::new(ToggleCancellationToken::default());
        let second_processor = Arc::clone(&processor);
        let second_candidate = candidate.clone();
        let second_policy = policy.clone();
        let second_token = Arc::clone(&cancellation_token);
        let (result_tx, result_rx) = std::sync::mpsc::channel();
        let second_handle = std::thread::spawn(move || {
            let result = second_processor.process_candidate(PreviewImageProcessRequest {
                sandbox_root: Path::new("sandbox"),
                candidate: &second_candidate,
                policy: &second_policy,
                cancellation_token: second_token.as_ref(),
            });
            result_tx.send(result.map(|_| ())).expect("send result");
        });

        std::thread::sleep(Duration::from_millis(25));
        cancellation_token.cancel_for_test();
        let result = match result_rx.recv_timeout(Duration::from_millis(250)) {
            Ok(result) => result,
            Err(error) => {
                release_tx.send(()).expect("release first processor");
                release_tx.send(()).expect("release second processor");
                first_handle.join().expect("first thread joins").ok();
                second_handle.join().expect("second thread joins");
                panic!("cancelled waiter did not return before timeout: {error}");
            }
        };

        release_tx.send(()).expect("release first processor");
        first_handle
            .join()
            .expect("first thread joins")
            .expect("first processor succeeds");
        second_handle.join().expect("second thread joins");

        let error = result.expect_err("waiting processor is cancelled");
        assert!(error.to_string().contains("cancelled"));
        assert_eq!(
            inner_calls.load(std::sync::atomic::Ordering::SeqCst),
            1,
            "cancelled waiter must not enter the inner processor"
        );
    }

    #[test]
    fn passes_cancellation_token_to_scanner_and_processor() {
        let observed = Arc::new(Mutex::new(Vec::new()));
        let candidate = preview_candidate("pkg-1", "preview.png");
        let service = PreviewImageService::new(
            PreviewImagePolicy::default(),
            Box::new(CancellationObservingScanner {
                observed: Arc::clone(&observed),
                candidates: vec![candidate],
            }),
            Box::new(CancellationObservingProcessor {
                observed: Arc::clone(&observed),
            }),
        );
        let cancellation_token = ToggleCancellationToken::default();

        let result = service
            .process_package_preview_with_cancellation(
                "task-1",
                "pkg-1",
                Path::new("sandbox"),
                &cancellation_token,
            )
            .expect("preview result");

        assert!(matches!(result, PreviewImageProcessingResult::Thumbnail(_)));
        assert_eq!(
            observed.lock().expect("observed lock").as_slice(),
            &[false, false]
        );
    }

    #[test]
    fn stops_before_processing_next_candidate_when_cancelled() {
        let first = preview_candidate("pkg-1", "first.png");
        let second = preview_candidate("pkg-1", "second.png");
        let processed_paths = Arc::new(Mutex::new(Vec::new()));
        let cancellation_token = Arc::new(ToggleCancellationToken::default());
        let service = PreviewImageService::new(
            PreviewImagePolicy::default(),
            Box::new(FakeScanner::new(vec![first, second])),
            Box::new(CancellingProcessor {
                processed_paths: Arc::clone(&processed_paths),
                cancellation_token: Arc::clone(&cancellation_token),
            }),
        );

        let error = service
            .process_package_preview_with_cancellation(
                "task-1",
                "pkg-1",
                Path::new("sandbox"),
                cancellation_token.as_ref(),
            )
            .expect_err("cancellation stops preview processing");

        assert!(error.to_string().contains("cancelled"));
        assert_eq!(
            *processed_paths.lock().expect("processed paths lock"),
            vec!["first.png".to_owned()]
        );
    }

    #[test]
    fn candidate_list_uses_import_record_and_returns_bounded_display_fields() {
        let repository = Arc::new(FakeModImportResultRepository {
            record: Mutex::new(Some(stored_analysis("mod-1", "pkg-1"))),
        });
        let located_packages = Arc::new(Mutex::new(Vec::new()));
        let service = PreviewImageCandidateListService::new(
            PreviewImagePolicy {
                max_candidates_per_package: 2,
                ..PreviewImagePolicy::default()
            },
            repository,
            Arc::new(FakeSandboxLocator {
                located_packages: Arc::clone(&located_packages),
            }),
            Box::new(FakeScanner::new(vec![
                preview_candidate("pkg-1", "nested/preview.png"),
                preview_candidate("pkg-1", "cover.webp"),
                preview_candidate("pkg-1", "extra.jpg"),
            ])),
        );

        let result = service
            .list_candidates("mod-1")
            .expect("candidate list succeeds")
            .expect("mod exists");

        assert_eq!(result.mod_id, "mod-1");
        assert_eq!(
            *located_packages.lock().expect("located packages lock"),
            vec!["pkg-1".to_owned()]
        );
        assert_eq!(
            result.candidates,
            vec![
                PreviewImageCandidateSummary {
                    candidate_index: 0,
                    file_name: "preview.png".to_owned(),
                    compressed_size_bytes: 0,
                },
                PreviewImageCandidateSummary {
                    candidate_index: 1,
                    file_name: "cover.webp".to_owned(),
                    compressed_size_bytes: 0,
                },
            ]
        );
    }

    #[test]
    fn candidate_list_returns_none_for_unknown_mod_without_scanning() {
        let service = PreviewImageCandidateListService::new(
            PreviewImagePolicy::default(),
            Arc::new(FakeModImportResultRepository {
                record: Mutex::new(None),
            }),
            Arc::new(FakeSandboxLocator {
                located_packages: Arc::new(Mutex::new(Vec::new())),
            }),
            Box::new(PanicScanner),
        );

        let result = service
            .list_candidates("missing-mod")
            .expect("candidate list query succeeds");

        assert!(result.is_none());
    }

    #[test]
    fn selected_candidate_updates_import_record_with_resolved_thumbnail() {
        let repository = Arc::new(FakeModImportResultRepository {
            record: Mutex::new(Some(stored_analysis("mod-1", "pkg-1"))),
        });
        let processed_paths = Arc::new(Mutex::new(Vec::new()));
        let service = PreviewImageCandidateSelectionService::new(
            PreviewImagePolicy::default(),
            repository.clone(),
            Arc::new(FakeSandboxLocator {
                located_packages: Arc::new(Mutex::new(Vec::new())),
            }),
            Box::new(FakeScanner::new(vec![
                preview_candidate("pkg-1", "first.png"),
                preview_candidate("pkg-1", "second.png"),
            ])),
            Box::new(RecordingProcessor::new(
                Arc::clone(&processed_paths),
                vec![PreviewImageProcessingResult::Thumbnail(
                    ProcessedPreviewImage {
                        thumbnail_ref: ThumbnailRef {
                            package_id: "pkg-1".to_owned(),
                            content_hash: "hash-2".to_owned(),
                            variant: "preview-768".to_owned(),
                        },
                        width: 640,
                        height: 360,
                        content_hash: "hash-2".to_owned(),
                    },
                )],
            )),
            Box::new(FakeThumbnailStore),
        );

        let result = service
            .select_candidate("mod-1", 1)
            .expect("selection succeeds")
            .expect("mod exists");

        assert_eq!(
            *processed_paths.lock().expect("processed paths lock"),
            vec!["second.png".to_owned()]
        );
        assert_eq!(
            result,
            crate::ImportPreviewImage::Thumbnail {
                thumbnail_url: "thumbnail://pkg-1/preview-768/hash-2".to_owned(),
                width: 640,
                height: 360,
                content_hash: "hash-2".to_owned(),
                variant: "preview-768".to_owned(),
            }
        );
        assert_eq!(
            repository
                .get_analysis("mod-1")
                .expect("record read succeeds")
                .expect("record exists")
                .preview_image,
            StoredImportPreviewImage::Thumbnail {
                thumbnail_url: "thumbnail://pkg-1/preview-768/hash-2".to_owned(),
                width: 640,
                height: 360,
                content_hash: "hash-2".to_owned(),
                variant: "preview-768".to_owned(),
            }
        );
    }

    #[test]
    fn selected_candidate_returns_none_for_unknown_mod_without_scanning() {
        let service = PreviewImageCandidateSelectionService::new(
            PreviewImagePolicy::default(),
            Arc::new(FakeModImportResultRepository {
                record: Mutex::new(None),
            }),
            Arc::new(FakeSandboxLocator {
                located_packages: Arc::new(Mutex::new(Vec::new())),
            }),
            Box::new(PanicScanner),
            Box::new(FakeProcessor::new(vec![])),
            Box::new(FakeThumbnailStore),
        );

        let result = service
            .select_candidate("missing-mod", 0)
            .expect("selection query succeeds");

        assert!(result.is_none());
    }

    #[test]
    fn detail_preview_uses_larger_variant_without_updating_import_record() {
        let original = stored_analysis("mod-1", "pkg-1");
        let save_calls = Arc::new(Mutex::new(0));
        let repository = Arc::new(ReadOnlyTrackingRepository {
            record: original.clone(),
            save_calls: Arc::clone(&save_calls),
        });
        let observed_edges = Arc::new(Mutex::new(Vec::new()));
        let service = PreviewImageDetailService::new(
            PreviewImagePolicy {
                output_max_edge_px: 1024,
                ..PreviewImagePolicy::default()
            },
            repository.clone(),
            Arc::new(FakeSandboxLocator {
                located_packages: Arc::new(Mutex::new(Vec::new())),
            }),
            Box::new(FakeScanner::new(vec![preview_candidate(
                "pkg-1",
                "preview.png",
            )])),
            Box::new(PolicyRecordingProcessor {
                observed_edges: Arc::clone(&observed_edges),
            }),
            Box::new(FakeThumbnailStore),
        );

        let result = service
            .get_detail_preview_image("mod-1")
            .expect("detail preview succeeds")
            .expect("mod exists");

        assert_eq!(*observed_edges.lock().expect("edges lock"), vec![1024]);
        assert_eq!(
            result,
            crate::ImportPreviewImage::Thumbnail {
                thumbnail_url: "thumbnail://pkg-1/preview-1024/hash-1024".to_owned(),
                width: 1024,
                height: 576,
                content_hash: "hash-1024".to_owned(),
                variant: "preview-1024".to_owned(),
            }
        );
        assert_eq!(*save_calls.lock().expect("save calls lock"), 0);
        assert_eq!(
            repository
                .get_analysis("mod-1")
                .expect("record read succeeds")
                .expect("record exists")
                .preview_image,
            original.preview_image
        );
    }

    fn preview_candidate(package_id: &str, logical_path: &str) -> PreviewImageCandidate {
        PreviewImageCandidate {
            source_ref: PreviewImageSourceRef {
                package_id: package_id.to_owned(),
                logical_path: logical_path.to_owned(),
            },
            file_name: Path::new(logical_path)
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or(logical_path)
                .to_owned(),
            compressed_size: 0,
            priority: 0,
        }
    }

    struct FakeModImportResultRepository {
        record: Mutex<Option<StoredModImportAnalysis>>,
    }

    impl ModImportResultRepository for FakeModImportResultRepository {
        fn save_analysis(&self, analysis: &StoredModImportAnalysis) -> Result<()> {
            *self.record.lock().expect("record lock") = Some(analysis.clone());
            Ok(())
        }

        fn list_analysis(&self) -> Result<Vec<StoredModImportAnalysis>> {
            Ok(self
                .record
                .lock()
                .expect("record lock")
                .clone()
                .into_iter()
                .collect())
        }

        fn get_analysis(&self, mod_id: &str) -> Result<Option<StoredModImportAnalysis>> {
            Ok(self
                .record
                .lock()
                .expect("record lock")
                .clone()
                .filter(|record| record.mod_id == mod_id))
        }
    }

    struct FakeThumbnailStore;

    impl hmm_ports::ThumbnailStore for FakeThumbnailStore {
        fn put_thumbnail(
            &self,
            _package_id: &str,
            _content_hash: &str,
            _variant: &str,
            _extension: &str,
            _bytes: &[u8],
        ) -> Result<ThumbnailRef> {
            unreachable!("selection service should not write thumbnails")
        }

        fn resolve_url(&self, thumbnail_ref: &ThumbnailRef) -> Result<String> {
            Ok(format!(
                "thumbnail://{}/{}/{}",
                thumbnail_ref.package_id, thumbnail_ref.variant, thumbnail_ref.content_hash
            ))
        }
    }

    struct FakeSandboxLocator {
        located_packages: Arc<Mutex<Vec<String>>>,
    }

    impl ModImportSandboxLocator for FakeSandboxLocator {
        fn sandbox_root_for_package(&self, package_id: &str) -> Result<std::path::PathBuf> {
            self.located_packages
                .lock()
                .expect("located packages lock")
                .push(package_id.to_owned());
            Ok(std::path::PathBuf::from("sandbox"))
        }
    }

    struct PanicScanner;

    impl PackagePreviewScanner for PanicScanner {
        fn scan_candidates(
            &self,
            _request: PreviewImageScanRequest<'_>,
        ) -> Result<Vec<PreviewImageCandidate>> {
            panic!("unknown mod should not scan candidates")
        }
    }

    fn stored_analysis(mod_id: &str, package_id: &str) -> StoredModImportAnalysis {
        StoredModImportAnalysis {
            mod_id: mod_id.to_owned(),
            task_id: "task-1".to_owned(),
            package_id: package_id.to_owned(),
            display_name: mod_id.to_owned(),
            metadata: StoredModPackageMetadata::default(),
            preview_image: StoredImportPreviewImage::Fallback {
                reason: PreviewImageRejectionReason::Missing,
            },
        }
    }

    struct ReadOnlyTrackingRepository {
        record: StoredModImportAnalysis,
        save_calls: Arc<Mutex<usize>>,
    }

    impl ModImportResultRepository for ReadOnlyTrackingRepository {
        fn save_analysis(&self, _analysis: &StoredModImportAnalysis) -> Result<()> {
            *self.save_calls.lock().expect("save calls lock") += 1;
            Ok(())
        }

        fn list_analysis(&self) -> Result<Vec<StoredModImportAnalysis>> {
            Ok(vec![self.record.clone()])
        }

        fn get_analysis(&self, mod_id: &str) -> Result<Option<StoredModImportAnalysis>> {
            Ok((self.record.mod_id == mod_id).then(|| self.record.clone()))
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
            _request: PreviewImageScanRequest<'_>,
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
            _request: PreviewImageProcessRequest<'_>,
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
            request: PreviewImageProcessRequest<'_>,
        ) -> Result<PreviewImageProcessingResult> {
            self.processed_paths
                .lock()
                .expect("processed paths lock")
                .push(request.candidate.source_ref.logical_path.clone());

            Ok(self
                .results
                .lock()
                .expect("processor lock")
                .pop()
                .expect("processor result"))
        }
    }

    struct PolicyRecordingProcessor {
        observed_edges: Arc<Mutex<Vec<u32>>>,
    }

    impl PreviewImageProcessor for PolicyRecordingProcessor {
        fn process_candidate(
            &self,
            request: PreviewImageProcessRequest<'_>,
        ) -> Result<PreviewImageProcessingResult> {
            let edge = request.policy.output_max_edge_px;
            self.observed_edges
                .lock()
                .expect("observed edges lock")
                .push(edge);

            Ok(PreviewImageProcessingResult::Thumbnail(
                ProcessedPreviewImage {
                    thumbnail_ref: ThumbnailRef {
                        package_id: request.candidate.source_ref.package_id.clone(),
                        content_hash: format!("hash-{edge}"),
                        variant: format!("preview-{edge}"),
                    },
                    width: edge,
                    height: edge * 9 / 16,
                    content_hash: format!("hash-{edge}"),
                },
            ))
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
            _request: PreviewImageProcessRequest<'_>,
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

    struct PermitHoldingProcessor {
        entered_tx: Mutex<Option<std::sync::mpsc::Sender<()>>>,
        release_rx: Mutex<std::sync::mpsc::Receiver<()>>,
        inner_calls: Arc<std::sync::atomic::AtomicUsize>,
    }

    impl PreviewImageProcessor for PermitHoldingProcessor {
        fn process_candidate(
            &self,
            _request: PreviewImageProcessRequest<'_>,
        ) -> Result<PreviewImageProcessingResult> {
            self.inner_calls
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            if let Some(entered_tx) = self.entered_tx.lock().expect("entered lock").take() {
                entered_tx.send(()).expect("send entered signal");
            }
            self.release_rx
                .lock()
                .expect("release lock")
                .recv()
                .expect("wait for release");
            Ok(PreviewImageProcessingResult::Fallback(
                PreviewImageRejectionReason::Missing,
            ))
        }
    }

    struct CancellationObservingScanner {
        observed: Arc<Mutex<Vec<bool>>>,
        candidates: Vec<PreviewImageCandidate>,
    }

    impl PackagePreviewScanner for CancellationObservingScanner {
        fn scan_candidates(
            &self,
            request: PreviewImageScanRequest<'_>,
        ) -> Result<Vec<PreviewImageCandidate>> {
            self.observed
                .lock()
                .expect("observed lock")
                .push(request.cancellation_token.is_cancelled());
            Ok(self.candidates.clone())
        }
    }

    struct CancellationObservingProcessor {
        observed: Arc<Mutex<Vec<bool>>>,
    }

    impl PreviewImageProcessor for CancellationObservingProcessor {
        fn process_candidate(
            &self,
            request: PreviewImageProcessRequest<'_>,
        ) -> Result<PreviewImageProcessingResult> {
            self.observed
                .lock()
                .expect("observed lock")
                .push(request.cancellation_token.is_cancelled());
            Ok(PreviewImageProcessingResult::Thumbnail(
                ProcessedPreviewImage {
                    thumbnail_ref: ThumbnailRef {
                        package_id: request.candidate.source_ref.package_id.clone(),
                        content_hash: "hash".to_owned(),
                        variant: "preview-768".to_owned(),
                    },
                    width: 4,
                    height: 4,
                    content_hash: "hash".to_owned(),
                },
            ))
        }
    }

    struct CancellingProcessor {
        processed_paths: Arc<Mutex<Vec<String>>>,
        cancellation_token: Arc<ToggleCancellationToken>,
    }

    impl PreviewImageProcessor for CancellingProcessor {
        fn process_candidate(
            &self,
            request: PreviewImageProcessRequest<'_>,
        ) -> Result<PreviewImageProcessingResult> {
            self.processed_paths
                .lock()
                .expect("processed paths lock")
                .push(request.candidate.source_ref.logical_path.clone());
            self.cancellation_token.cancel_for_test();
            Ok(PreviewImageProcessingResult::Fallback(
                PreviewImageRejectionReason::DecodeFailed,
            ))
        }
    }

    #[derive(Default)]
    struct ToggleCancellationToken {
        cancelled: Mutex<bool>,
    }

    impl ToggleCancellationToken {
        fn cancel_for_test(&self) {
            *self.cancelled.lock().expect("cancelled lock") = true;
        }
    }

    impl CancellationToken for ToggleCancellationToken {
        fn is_cancelled(&self) -> bool {
            *self.cancelled.lock().expect("cancelled lock")
        }
    }
}
