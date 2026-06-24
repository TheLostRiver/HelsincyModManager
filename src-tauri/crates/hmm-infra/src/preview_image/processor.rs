use crate::preview_image::magic_bytes::detect_image_format;
use anyhow::Result;
use hmm_core::{PreviewImageOutputFormat, PreviewImagePolicy, PreviewImageRejectionReason};
use hmm_ports::{
    CancellationToken, PreviewImageProcessRequest, PreviewImageProcessingResult,
    PreviewImageProcessor, ProcessedPreviewImage, ThumbnailStore,
};
use image::{
    codecs::{jpeg::JpegEncoder, webp::WebPEncoder},
    ExtendedColorType, GenericImageView, ImageEncoder,
};
use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::Path;

pub struct ImageCratePreviewImageProcessor {
    thumbnail_store: Box<dyn ThumbnailStore>,
}

impl ImageCratePreviewImageProcessor {
    pub fn new(thumbnail_store: Box<dyn ThumbnailStore>) -> Self {
        Self { thumbnail_store }
    }
}

impl PreviewImageProcessor for ImageCratePreviewImageProcessor {
    fn process_candidate(
        &self,
        request: PreviewImageProcessRequest<'_>,
    ) -> Result<PreviewImageProcessingResult> {
        let sandbox_root = request.sandbox_root;
        let candidate = request.candidate;
        let policy = request.policy;
        let cancellation_token = request.cancellation_token;
        ensure_not_cancelled(cancellation_token)?;

        let Some(candidate_path) =
            resolve_logical_path(sandbox_root, &candidate.source_ref.logical_path)
        else {
            return Ok(PreviewImageProcessingResult::Fallback(
                PreviewImageRejectionReason::UnsupportedFormat,
            ));
        };
        if !candidate_stays_inside_sandbox(sandbox_root, &candidate_path) {
            return Ok(PreviewImageProcessingResult::Fallback(
                PreviewImageRejectionReason::UnsupportedFormat,
            ));
        }
        ensure_not_cancelled(cancellation_token)?;

        let metadata = match std::fs::metadata(&candidate_path) {
            Ok(metadata) => metadata,
            Err(_) => {
                return Ok(PreviewImageProcessingResult::Fallback(
                    PreviewImageRejectionReason::DecodeFailed,
                ))
            }
        };

        if metadata.len() > policy.max_input_bytes {
            return Ok(PreviewImageProcessingResult::Fallback(
                PreviewImageRejectionReason::TooLarge,
            ));
        }
        ensure_not_cancelled(cancellation_token)?;

        let mut file = match std::fs::File::open(&candidate_path) {
            Ok(file) => file,
            Err(_) => {
                return Ok(PreviewImageProcessingResult::Fallback(
                    PreviewImageRejectionReason::DecodeFailed,
                ))
            }
        };
        let mut header = [0_u8; 16];
        let read = match file.read(&mut header) {
            Ok(read) => read,
            Err(_) => {
                return Ok(PreviewImageProcessingResult::Fallback(
                    PreviewImageRejectionReason::DecodeFailed,
                ))
            }
        };
        if detect_image_format(&header[..read]).is_none() {
            return Ok(PreviewImageProcessingResult::Fallback(
                PreviewImageRejectionReason::UnsupportedFormat,
            ));
        }
        ensure_not_cancelled(cancellation_token)?;

        let reader = match image::ImageReader::open(&candidate_path)
            .and_then(|reader| reader.with_guessed_format())
        {
            Ok(reader) => reader,
            Err(_) => {
                return Ok(PreviewImageProcessingResult::Fallback(
                    PreviewImageRejectionReason::DecodeFailed,
                ))
            }
        };
        let dimensions = match reader.into_dimensions() {
            Ok(dimensions) => dimensions,
            Err(_) => {
                return Ok(PreviewImageProcessingResult::Fallback(
                    PreviewImageRejectionReason::DecodeFailed,
                ))
            }
        };

        let decoded_pixels = u64::from(dimensions.0).saturating_mul(u64::from(dimensions.1));
        if decoded_pixels > policy.max_decoded_pixels {
            return Ok(PreviewImageProcessingResult::Fallback(
                PreviewImageRejectionReason::PixelLimitExceeded,
            ));
        }
        ensure_not_cancelled(cancellation_token)?;

        let image = match image::open(&candidate_path) {
            Ok(image) => image,
            Err(_) => {
                return Ok(PreviewImageProcessingResult::Fallback(
                    PreviewImageRejectionReason::DecodeFailed,
                ))
            }
        };
        ensure_not_cancelled(cancellation_token)?;

        let resized = image.thumbnail(policy.output_max_edge_px, policy.output_max_edge_px);
        let mut output = Vec::new();
        let encoded = encode_thumbnail(&resized, policy, &mut output);
        let extension = match encoded {
            Ok(extension) => extension,
            Err(_) => {
                return Ok(PreviewImageProcessingResult::Fallback(
                    PreviewImageRejectionReason::DecodeFailed,
                ))
            }
        };
        ensure_not_cancelled(cancellation_token)?;

        let content_hash = hex_sha256(&output);
        let variant = thumbnail_variant_for_policy(policy);
        let thumbnail_ref = match self.thumbnail_store.put_thumbnail(
            &candidate.source_ref.package_id,
            &content_hash,
            &variant,
            extension,
            &output,
        ) {
            Ok(thumbnail_ref) => thumbnail_ref,
            Err(_) => {
                return Ok(PreviewImageProcessingResult::Fallback(
                    PreviewImageRejectionReason::CacheWriteFailed,
                ))
            }
        };
        ensure_not_cancelled(cancellation_token)?;

        Ok(PreviewImageProcessingResult::Thumbnail(
            ProcessedPreviewImage {
                thumbnail_ref,
                width: resized.dimensions().0,
                height: resized.dimensions().1,
                content_hash,
            },
        ))
    }
}

fn thumbnail_variant_for_policy(policy: &PreviewImagePolicy) -> String {
    format!("preview-{}", policy.output_max_edge_px)
}

fn ensure_not_cancelled(cancellation_token: &dyn CancellationToken) -> Result<()> {
    if cancellation_token.is_cancelled() {
        anyhow::bail!("preview image processing cancelled");
    }

    Ok(())
}

fn hex_sha256(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn candidate_stays_inside_sandbox(sandbox_root: &Path, candidate_path: &Path) -> bool {
    let Ok(root) = sandbox_root.canonicalize() else {
        return false;
    };
    let Ok(candidate) = candidate_path.canonicalize() else {
        return false;
    };

    candidate.starts_with(root)
}

fn encode_thumbnail(
    image: &image::DynamicImage,
    policy: &PreviewImagePolicy,
    output: &mut Vec<u8>,
) -> image::ImageResult<&'static str> {
    match policy.preferred_output_format {
        PreviewImageOutputFormat::Jpeg => {
            let rgb = image.to_rgb8();
            JpegEncoder::new_with_quality(output, policy.output_quality).write_image(
                rgb.as_raw(),
                rgb.width(),
                rgb.height(),
                ExtendedColorType::Rgb8,
            )?;
            Ok("jpg")
        }
        PreviewImageOutputFormat::WebP => {
            let rgba = image.to_rgba8();
            WebPEncoder::new_lossless(output).write_image(
                rgba.as_raw(),
                rgba.width(),
                rgba.height(),
                ExtendedColorType::Rgba8,
            )?;
            Ok("webp")
        }
    }
}

fn resolve_logical_path(sandbox_root: &Path, logical_path: &str) -> Option<std::path::PathBuf> {
    if logical_path.contains('\\') {
        return None;
    }

    let mut path = sandbox_root.to_path_buf();
    for segment in logical_path
        .split('/')
        .filter(|segment| !segment.is_empty())
    {
        if segment == "." || segment == ".." || segment.contains(':') {
            return None;
        }
        path.push(segment);
    }

    Some(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use hmm_core::{PreviewImagePolicy, PreviewImageRejectionReason};
    use hmm_ports::{
        CancellationToken, NeverCancelled, PreviewImageCandidate, PreviewImageProcessRequest,
        PreviewImageProcessingResult, PreviewImageSourceRef, ThumbnailRef, ThumbnailStore,
    };
    use image::{DynamicImage, ImageBuffer, ImageFormat, Rgb, Rgba};
    use std::io::Cursor;
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    };

    #[test]
    fn rejects_candidate_over_input_size_limit() {
        let temp = tempfile::tempdir().expect("temp dir");
        std::fs::write(temp.path().join("preview.png"), vec![0_u8; 128]).expect("write fake image");

        let candidate = preview_candidate("pkg-1", "preview.png", 128);
        let policy = PreviewImagePolicy {
            max_input_bytes: 64,
            ..PreviewImagePolicy::default()
        };

        let processor =
            ImageCratePreviewImageProcessor::new(Box::new(InMemoryThumbnailStore::default()));
        let result = process_candidate(&processor, temp.path(), &candidate, &policy)
            .expect("processing result");

        assert_eq!(
            result,
            PreviewImageProcessingResult::Fallback(PreviewImageRejectionReason::TooLarge)
        );
    }

    #[test]
    fn rejects_magic_bytes_mismatch() {
        let temp = tempfile::tempdir().expect("temp dir");
        std::fs::write(temp.path().join("preview.png"), b"not an image").expect("write fake image");

        let candidate = preview_candidate("pkg-1", "preview.png", 12);
        let processor =
            ImageCratePreviewImageProcessor::new(Box::new(InMemoryThumbnailStore::default()));
        let result = process_candidate(
            &processor,
            temp.path(),
            &candidate,
            &PreviewImagePolicy::default(),
        )
        .expect("processing result");

        assert_eq!(
            result,
            PreviewImageProcessingResult::Fallback(PreviewImageRejectionReason::UnsupportedFormat)
        );
    }

    #[test]
    fn creates_thumbnail_for_valid_png() {
        let temp = tempfile::tempdir().expect("temp dir");
        write_png(temp.path().join("preview.png").as_path(), 8, 4);

        let candidate = preview_candidate("pkg-1", "preview.png", 0);
        let policy = PreviewImagePolicy {
            output_max_edge_px: 4,
            ..PreviewImagePolicy::default()
        };

        let store = InMemoryThumbnailStore::default();
        let processor = ImageCratePreviewImageProcessor::new(Box::new(store));
        let result = process_candidate(&processor, temp.path(), &candidate, &policy)
            .expect("processing result");

        let PreviewImageProcessingResult::Thumbnail(thumbnail) = result else {
            panic!("expected thumbnail result");
        };

        assert_eq!(thumbnail.width, 4);
        assert_eq!(thumbnail.height, 2);
        assert_eq!(thumbnail.thumbnail_ref.package_id, "pkg-1");
        assert!(!thumbnail.content_hash.is_empty());
    }

    #[test]
    fn passes_policy_derived_variant_to_thumbnail_store() {
        let temp = tempfile::tempdir().expect("temp dir");
        write_png(temp.path().join("preview.png").as_path(), 16, 8);

        let candidate = preview_candidate("pkg-1", "preview.png", 0);
        let policy = PreviewImagePolicy {
            output_max_edge_px: 1024,
            ..PreviewImagePolicy::default()
        };

        let store = InMemoryThumbnailStore::default();
        let recorded_variant = store.recorded_variant.clone();
        let processor = ImageCratePreviewImageProcessor::new(Box::new(store));
        let result = process_candidate(&processor, temp.path(), &candidate, &policy)
            .expect("processing result");

        let PreviewImageProcessingResult::Thumbnail(thumbnail) = result else {
            panic!("expected thumbnail result");
        };

        assert_eq!(thumbnail.thumbnail_ref.variant, "preview-1024");
        assert_eq!(
            recorded_variant
                .lock()
                .expect("thumbnail store variant lock")
                .as_deref(),
            Some("preview-1024")
        );
    }

    #[test]
    fn creates_thumbnail_for_valid_jpeg() {
        let temp = tempfile::tempdir().expect("temp dir");
        write_jpeg(temp.path().join("preview.jpg").as_path(), 10, 5);

        let candidate = preview_candidate("pkg-1", "preview.jpg", 0);
        let policy = PreviewImagePolicy {
            output_max_edge_px: 5,
            ..PreviewImagePolicy::default()
        };

        let processor =
            ImageCratePreviewImageProcessor::new(Box::new(InMemoryThumbnailStore::default()));
        let result = process_candidate(&processor, temp.path(), &candidate, &policy)
            .expect("processing result");

        let PreviewImageProcessingResult::Thumbnail(thumbnail) = result else {
            panic!("expected thumbnail result");
        };

        assert_eq!(thumbnail.width, 5);
        assert_eq!(thumbnail.height, 3);
        assert!(!thumbnail.content_hash.is_empty());
    }

    #[test]
    fn creates_thumbnail_for_valid_webp() {
        let temp = tempfile::tempdir().expect("temp dir");
        write_webp(temp.path().join("preview.webp").as_path(), 6, 3);

        let candidate = preview_candidate("pkg-1", "preview.webp", 0);
        let policy = PreviewImagePolicy {
            preferred_output_format: PreviewImageOutputFormat::WebP,
            output_max_edge_px: 3,
            ..PreviewImagePolicy::default()
        };
        let store = InMemoryThumbnailStore::default();
        let recorded_extension = store.recorded_extension.clone();
        let processor = ImageCratePreviewImageProcessor::new(Box::new(store));
        let result = process_candidate(&processor, temp.path(), &candidate, &policy)
            .expect("processing result");

        let PreviewImageProcessingResult::Thumbnail(thumbnail) = result else {
            panic!("expected thumbnail result");
        };

        assert_eq!(thumbnail.width, 3);
        assert_eq!(thumbnail.height, 2);
        assert_eq!(
            recorded_extension
                .lock()
                .expect("thumbnail store extension lock")
                .as_deref(),
            Some("webp")
        );
    }

    #[test]
    fn returns_decode_failed_for_corrupted_image_after_magic_bytes_match() {
        let temp = tempfile::tempdir().expect("temp dir");
        std::fs::write(
            temp.path().join("preview.png"),
            [
                0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a, b'b', b'a', b'd',
            ],
        )
        .expect("write corrupted png");

        let candidate = preview_candidate("pkg-1", "preview.png", 0);
        let processor =
            ImageCratePreviewImageProcessor::new(Box::new(InMemoryThumbnailStore::default()));
        let result = process_candidate(
            &processor,
            temp.path(),
            &candidate,
            &PreviewImagePolicy::default(),
        )
        .expect("processing result");

        assert_eq!(
            result,
            PreviewImageProcessingResult::Fallback(PreviewImageRejectionReason::DecodeFailed)
        );
    }

    #[test]
    fn rejects_image_over_decoded_pixel_limit_before_decode() {
        let temp = tempfile::tempdir().expect("temp dir");
        write_png(temp.path().join("preview.png").as_path(), 4, 4);

        let candidate = preview_candidate("pkg-1", "preview.png", 0);
        let policy = PreviewImagePolicy {
            max_decoded_pixels: 15,
            ..PreviewImagePolicy::default()
        };
        let processor =
            ImageCratePreviewImageProcessor::new(Box::new(InMemoryThumbnailStore::default()));
        let result = process_candidate(&processor, temp.path(), &candidate, &policy)
            .expect("processing result");

        assert_eq!(
            result,
            PreviewImageProcessingResult::Fallback(PreviewImageRejectionReason::PixelLimitExceeded)
        );
    }

    #[test]
    fn resolves_nested_logical_paths_inside_sandbox() {
        let temp = tempfile::tempdir().expect("temp dir");
        std::fs::create_dir_all(temp.path().join("nested")).expect("create nested dir");
        write_png(temp.path().join("nested/preview.png").as_path(), 4, 4);

        let candidate = preview_candidate("pkg-1", "nested/preview.png", 0);
        let processor =
            ImageCratePreviewImageProcessor::new(Box::new(InMemoryThumbnailStore::default()));
        let result = process_candidate(
            &processor,
            temp.path(),
            &candidate,
            &PreviewImagePolicy::default(),
        )
        .expect("processing result");

        assert!(matches!(result, PreviewImageProcessingResult::Thumbnail(_)));
    }

    #[test]
    fn rejects_logical_paths_that_escape_sandbox() {
        let temp = tempfile::tempdir().expect("temp dir");
        let outside = temp.path().join("outside.png");
        write_png(&outside, 4, 4);

        let sandbox = temp.path().join("sandbox");
        std::fs::create_dir_all(&sandbox).expect("create sandbox");
        let candidate = preview_candidate("pkg-1", "../outside.png", 0);
        let processor =
            ImageCratePreviewImageProcessor::new(Box::new(InMemoryThumbnailStore::default()));
        let result = process_candidate(
            &processor,
            &sandbox,
            &candidate,
            &PreviewImagePolicy::default(),
        )
        .expect("processing result");

        assert_eq!(
            result,
            PreviewImageProcessingResult::Fallback(PreviewImageRejectionReason::UnsupportedFormat)
        );
    }

    #[test]
    fn returns_fallback_when_candidate_disappears_before_open() {
        let temp = tempfile::tempdir().expect("temp dir");

        let candidate = preview_candidate("pkg-1", "missing.png", 0);
        let processor =
            ImageCratePreviewImageProcessor::new(Box::new(InMemoryThumbnailStore::default()));
        let result = process_candidate(
            &processor,
            temp.path(),
            &candidate,
            &PreviewImagePolicy::default(),
        )
        .expect("preview result should degrade to fallback");

        assert_eq!(
            result,
            PreviewImageProcessingResult::Fallback(PreviewImageRejectionReason::UnsupportedFormat)
        );
    }

    #[test]
    fn returns_cache_fallback_when_thumbnail_store_fails() {
        let temp = tempfile::tempdir().expect("temp dir");
        write_png(temp.path().join("preview.png").as_path(), 4, 4);

        let candidate = preview_candidate("pkg-1", "preview.png", 0);
        let processor = ImageCratePreviewImageProcessor::new(Box::new(FailingThumbnailStore));
        let result = process_candidate(
            &processor,
            temp.path(),
            &candidate,
            &PreviewImagePolicy::default(),
        )
        .expect("preview result should degrade to fallback");

        assert_eq!(
            result,
            PreviewImageProcessingResult::Fallback(PreviewImageRejectionReason::CacheWriteFailed)
        );
    }

    #[test]
    fn uses_jpeg_policy_extension_for_thumbnail_ref() {
        let temp = tempfile::tempdir().expect("temp dir");
        write_png(temp.path().join("preview.png").as_path(), 4, 4);

        let candidate = preview_candidate("pkg-1", "preview.png", 0);
        let policy = PreviewImagePolicy {
            output_quality: 72,
            preferred_output_format: PreviewImageOutputFormat::Jpeg,
            ..PreviewImagePolicy::default()
        };

        let store = InMemoryThumbnailStore::default();
        let recorded_extension = store.recorded_extension.clone();
        let processor = ImageCratePreviewImageProcessor::new(Box::new(store));
        let result = process_candidate(&processor, temp.path(), &candidate, &policy)
            .expect("processing result");

        assert!(matches!(result, PreviewImageProcessingResult::Thumbnail(_)));
        assert_eq!(
            recorded_extension
                .lock()
                .expect("thumbnail store extension lock")
                .as_deref(),
            Some("jpg")
        );
    }

    #[test]
    fn cancellation_before_thumbnail_write_avoids_cache_write() {
        let temp = tempfile::tempdir().expect("temp dir");
        write_png(temp.path().join("preview.png").as_path(), 4, 4);

        let candidate = preview_candidate("pkg-1", "preview.png", 0);
        let store_calls = Arc::new(AtomicUsize::new(0));
        let processor = ImageCratePreviewImageProcessor::new(Box::new(CountingThumbnailStore {
            calls: Arc::clone(&store_calls),
        }));
        let cancellation_token = CountingCancellationToken::new(6);

        let error = process_candidate_with_token(
            &processor,
            temp.path(),
            &candidate,
            &PreviewImagePolicy::default(),
            &cancellation_token,
        )
        .expect_err("cancelled processing fails before cache write");

        assert!(error.to_string().contains("cancelled"));
        assert_eq!(store_calls.load(Ordering::SeqCst), 0);
    }

    fn preview_candidate(
        package_id: &str,
        logical_path: &str,
        compressed_size: u64,
    ) -> PreviewImageCandidate {
        PreviewImageCandidate {
            source_ref: PreviewImageSourceRef {
                package_id: package_id.to_owned(),
                logical_path: logical_path.to_owned(),
            },
            file_name: logical_path
                .rsplit('/')
                .next()
                .unwrap_or(logical_path)
                .to_owned(),
            compressed_size,
            priority: 0,
        }
    }

    fn process_candidate(
        processor: &ImageCratePreviewImageProcessor,
        sandbox_root: &std::path::Path,
        candidate: &PreviewImageCandidate,
        policy: &PreviewImagePolicy,
    ) -> anyhow::Result<PreviewImageProcessingResult> {
        process_candidate_with_token(processor, sandbox_root, candidate, policy, &NeverCancelled)
    }

    fn process_candidate_with_token(
        processor: &ImageCratePreviewImageProcessor,
        sandbox_root: &std::path::Path,
        candidate: &PreviewImageCandidate,
        policy: &PreviewImagePolicy,
        cancellation_token: &dyn CancellationToken,
    ) -> anyhow::Result<PreviewImageProcessingResult> {
        processor.process_candidate(PreviewImageProcessRequest {
            sandbox_root,
            candidate,
            policy,
            cancellation_token,
        })
    }

    fn write_png(path: &std::path::Path, width: u32, height: u32) {
        let image = ImageBuffer::from_pixel(width, height, Rgba([255_u8, 0, 0, 255]));
        let mut bytes = Vec::new();
        image
            .write_to(&mut Cursor::new(&mut bytes), ImageFormat::Png)
            .expect("encode png");
        std::fs::write(path, bytes).expect("write png");
    }

    fn write_jpeg(path: &std::path::Path, width: u32, height: u32) {
        let image = ImageBuffer::from_pixel(width, height, Rgb([255_u8, 0, 0]));
        let mut bytes = Vec::new();
        DynamicImage::ImageRgb8(image)
            .write_to(&mut Cursor::new(&mut bytes), ImageFormat::Jpeg)
            .expect("encode jpeg");
        std::fs::write(path, bytes).expect("write jpeg");
    }

    fn write_webp(path: &std::path::Path, width: u32, height: u32) {
        let image = ImageBuffer::from_pixel(width, height, Rgba([255_u8, 0, 0, 255]));
        let mut bytes = Vec::new();
        DynamicImage::ImageRgba8(image)
            .write_to(&mut Cursor::new(&mut bytes), ImageFormat::WebP)
            .expect("encode webp");
        std::fs::write(path, bytes).expect("write webp");
    }

    #[derive(Default)]
    struct InMemoryThumbnailStore {
        last_bytes: Mutex<Option<Vec<u8>>>,
        recorded_extension: std::sync::Arc<Mutex<Option<String>>>,
        recorded_variant: std::sync::Arc<Mutex<Option<String>>>,
    }

    impl ThumbnailStore for InMemoryThumbnailStore {
        fn put_thumbnail(
            &self,
            package_id: &str,
            content_hash: &str,
            variant: &str,
            extension: &str,
            bytes: &[u8],
        ) -> anyhow::Result<ThumbnailRef> {
            *self.last_bytes.lock().expect("thumbnail store lock") = Some(bytes.to_vec());
            *self
                .recorded_extension
                .lock()
                .expect("thumbnail store extension lock") = Some(extension.to_owned());
            *self
                .recorded_variant
                .lock()
                .expect("thumbnail store variant lock") = Some(variant.to_owned());
            Ok(ThumbnailRef {
                package_id: package_id.to_owned(),
                content_hash: content_hash.to_owned(),
                variant: variant.to_owned(),
            })
        }

        fn resolve_url(&self, thumbnail_ref: &ThumbnailRef) -> anyhow::Result<String> {
            Ok(format!(
                "thumbnail://{}/{}/{}",
                thumbnail_ref.package_id, thumbnail_ref.variant, thumbnail_ref.content_hash
            ))
        }
    }

    struct FailingThumbnailStore;

    impl ThumbnailStore for FailingThumbnailStore {
        fn put_thumbnail(
            &self,
            _package_id: &str,
            _content_hash: &str,
            _variant: &str,
            _extension: &str,
            _bytes: &[u8],
        ) -> anyhow::Result<ThumbnailRef> {
            anyhow::bail!("thumbnail store unavailable")
        }

        fn resolve_url(&self, _thumbnail_ref: &ThumbnailRef) -> anyhow::Result<String> {
            unreachable!("processor should not resolve thumbnail URLs")
        }
    }

    struct CountingThumbnailStore {
        calls: Arc<AtomicUsize>,
    }

    impl ThumbnailStore for CountingThumbnailStore {
        fn put_thumbnail(
            &self,
            package_id: &str,
            content_hash: &str,
            variant: &str,
            _extension: &str,
            _bytes: &[u8],
        ) -> anyhow::Result<ThumbnailRef> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(ThumbnailRef {
                package_id: package_id.to_owned(),
                content_hash: content_hash.to_owned(),
                variant: variant.to_owned(),
            })
        }

        fn resolve_url(&self, _thumbnail_ref: &ThumbnailRef) -> anyhow::Result<String> {
            unreachable!("processor should not resolve thumbnail URLs")
        }
    }

    struct CountingCancellationToken {
        allowed_checks: usize,
        checks: AtomicUsize,
    }

    impl CountingCancellationToken {
        fn new(allowed_checks: usize) -> Self {
            Self {
                allowed_checks,
                checks: AtomicUsize::new(0),
            }
        }
    }

    impl CancellationToken for CountingCancellationToken {
        fn is_cancelled(&self) -> bool {
            let check = self.checks.fetch_add(1, Ordering::SeqCst) + 1;
            check > self.allowed_checks
        }
    }
}
