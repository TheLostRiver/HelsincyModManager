use crate::preview_image::magic_bytes::detect_image_format;
use anyhow::{Context, Result};
use hmm_core::{PreviewImagePolicy, PreviewImageRejectionReason};
use hmm_ports::{
    PreviewImageCandidate, PreviewImageProcessingResult, PreviewImageProcessor,
    ProcessedPreviewImage, ThumbnailStore,
};
use image::{GenericImageView, ImageFormat};
use sha2::{Digest, Sha256};
use std::io::{Cursor, Read};
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
        sandbox_root: &Path,
        candidate: &PreviewImageCandidate,
        policy: &PreviewImagePolicy,
    ) -> Result<PreviewImageProcessingResult> {
        let Some(candidate_path) = resolve_logical_path(sandbox_root, &candidate.source_ref.logical_path) else {
            return Ok(PreviewImageProcessingResult::Fallback(
                PreviewImageRejectionReason::UnsupportedFormat,
            ));
        };
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

        let mut file = std::fs::File::open(&candidate_path)?;
        let mut header = [0_u8; 16];
        let read = file.read(&mut header)?;
        if detect_image_format(&header[..read]).is_none() {
            return Ok(PreviewImageProcessingResult::Fallback(
                PreviewImageRejectionReason::UnsupportedFormat,
            ));
        }

        let reader = image::ImageReader::open(&candidate_path)?.with_guessed_format()?;
        let dimensions = match reader.into_dimensions() {
            Ok(dimensions) => dimensions,
            Err(_) => {
                return Ok(PreviewImageProcessingResult::Fallback(
                    PreviewImageRejectionReason::DecodeFailed,
                ))
            }
        };

        let decoded_pixels = u64::from(dimensions.0)
            .checked_mul(u64::from(dimensions.1))
            .unwrap_or(u64::MAX);
        if decoded_pixels > policy.max_decoded_pixels {
            return Ok(PreviewImageProcessingResult::Fallback(
                PreviewImageRejectionReason::PixelLimitExceeded,
            ));
        }

        let image = match image::open(&candidate_path) {
            Ok(image) => image,
            Err(_) => {
                return Ok(PreviewImageProcessingResult::Fallback(
                    PreviewImageRejectionReason::DecodeFailed,
                ))
            }
        };

        let resized = image.thumbnail(policy.output_max_edge_px, policy.output_max_edge_px);
        let mut output = Vec::new();
        resized
            .write_to(&mut Cursor::new(&mut output), ImageFormat::Jpeg)
            .context("encode preview thumbnail")?;

        let content_hash = hex_sha256(&output);
        let thumbnail_ref = self.thumbnail_store.put_thumbnail(
            &candidate.source_ref.package_id,
            &content_hash,
            "jpg",
            &output,
        )?;

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

fn hex_sha256(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn resolve_logical_path(sandbox_root: &Path, logical_path: &str) -> Option<std::path::PathBuf> {
    if logical_path.contains('\\') {
        return None;
    }

    let mut path = sandbox_root.to_path_buf();
    for segment in logical_path.split('/').filter(|segment| !segment.is_empty()) {
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
        PreviewImageCandidate, PreviewImageProcessingResult, PreviewImageSourceRef, ThumbnailRef,
        ThumbnailStore,
    };
    use image::{ImageBuffer, ImageFormat, Rgba};
    use std::io::Cursor;
    use std::sync::Mutex;

    #[test]
    fn rejects_candidate_over_input_size_limit() {
        let temp = tempfile::tempdir().expect("temp dir");
        std::fs::write(temp.path().join("preview.png"), vec![0_u8; 128])
            .expect("write fake image");

        let candidate = preview_candidate("pkg-1", "preview.png", 128);
        let mut policy = PreviewImagePolicy::default();
        policy.max_input_bytes = 64;

        let processor = ImageCratePreviewImageProcessor::new(Box::new(InMemoryThumbnailStore::default()));
        let result = processor
            .process_candidate(temp.path(), &candidate, &policy)
            .expect("processing result");

        assert_eq!(
            result,
            PreviewImageProcessingResult::Fallback(PreviewImageRejectionReason::TooLarge)
        );
    }

    #[test]
    fn rejects_magic_bytes_mismatch() {
        let temp = tempfile::tempdir().expect("temp dir");
        std::fs::write(temp.path().join("preview.png"), b"not an image")
            .expect("write fake image");

        let candidate = preview_candidate("pkg-1", "preview.png", 12);
        let processor = ImageCratePreviewImageProcessor::new(Box::new(InMemoryThumbnailStore::default()));
        let result = processor
            .process_candidate(temp.path(), &candidate, &PreviewImagePolicy::default())
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
        let mut policy = PreviewImagePolicy::default();
        policy.output_max_edge_px = 4;

        let store = InMemoryThumbnailStore::default();
        let processor = ImageCratePreviewImageProcessor::new(Box::new(store));
        let result = processor
            .process_candidate(temp.path(), &candidate, &policy)
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
    fn resolves_nested_logical_paths_inside_sandbox() {
        let temp = tempfile::tempdir().expect("temp dir");
        std::fs::create_dir_all(temp.path().join("nested")).expect("create nested dir");
        write_png(temp.path().join("nested/preview.png").as_path(), 4, 4);

        let candidate = preview_candidate("pkg-1", "nested/preview.png", 0);
        let processor =
            ImageCratePreviewImageProcessor::new(Box::new(InMemoryThumbnailStore::default()));
        let result = processor
            .process_candidate(temp.path(), &candidate, &PreviewImagePolicy::default())
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
        let result = processor
            .process_candidate(&sandbox, &candidate, &PreviewImagePolicy::default())
            .expect("processing result");

        assert_eq!(
            result,
            PreviewImageProcessingResult::Fallback(PreviewImageRejectionReason::UnsupportedFormat)
        );
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
            file_name: logical_path.rsplit('/').next().unwrap_or(logical_path).to_owned(),
            compressed_size,
            priority: 0,
        }
    }

    fn write_png(path: &std::path::Path, width: u32, height: u32) {
        let image = ImageBuffer::from_pixel(width, height, Rgba([255_u8, 0, 0, 255]));
        let mut bytes = Vec::new();
        image
            .write_to(&mut Cursor::new(&mut bytes), ImageFormat::Png)
            .expect("encode png");
        std::fs::write(path, bytes).expect("write png");
    }

    #[derive(Default)]
    struct InMemoryThumbnailStore {
        last_bytes: Mutex<Option<Vec<u8>>>,
    }

    impl ThumbnailStore for InMemoryThumbnailStore {
        fn put_thumbnail(
            &self,
            package_id: &str,
            content_hash: &str,
            _extension: &str,
            bytes: &[u8],
        ) -> anyhow::Result<ThumbnailRef> {
            *self.last_bytes.lock().expect("thumbnail store lock") = Some(bytes.to_vec());
            Ok(ThumbnailRef {
                package_id: package_id.to_owned(),
                content_hash: content_hash.to_owned(),
                variant: "preview-768".to_owned(),
            })
        }

        fn resolve_url(&self, thumbnail_ref: &ThumbnailRef) -> anyhow::Result<String> {
            Ok(format!(
                "thumbnail://{}/{}/{}",
                thumbnail_ref.package_id, thumbnail_ref.variant, thumbnail_ref.content_hash
            ))
        }
    }
}
