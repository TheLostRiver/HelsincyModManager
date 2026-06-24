use anyhow::Result;
use hmm_ports::{
    CancellationToken, PackagePreviewScanner, PreviewImageCandidate, PreviewImageScanRequest,
    PreviewImageSourceRef,
};
use std::path::Path;

pub struct SandboxPackagePreviewScanner;

impl PackagePreviewScanner for SandboxPackagePreviewScanner {
    fn scan_candidates(
        &self,
        request: PreviewImageScanRequest<'_>,
    ) -> Result<Vec<PreviewImageCandidate>> {
        let mut candidates = Vec::new();
        collect_candidates(
            request.package_id,
            request.sandbox_root,
            request.sandbox_root,
            request.policy.max_candidates_per_package,
            request.cancellation_token,
            &mut candidates,
        )?;
        Ok(candidates)
    }
}

fn collect_candidates(
    package_id: &str,
    sandbox_root: &Path,
    current_dir: &Path,
    max_candidates: usize,
    cancellation_token: &dyn CancellationToken,
    out: &mut Vec<PreviewImageCandidate>,
) -> Result<()> {
    ensure_not_cancelled(cancellation_token)?;
    for entry in std::fs::read_dir(current_dir)? {
        ensure_not_cancelled(cancellation_token)?;
        let entry = entry?;
        let file_type = entry.file_type()?;

        if file_type.is_symlink() {
            continue;
        }

        let path = entry.path();
        if file_type.is_dir() {
            collect_candidates(
                package_id,
                sandbox_root,
                &path,
                max_candidates,
                cancellation_token,
                out,
            )?;
            continue;
        }

        if !file_type.is_file() || !has_image_extension(&path) {
            continue;
        }

        let relative = path.strip_prefix(sandbox_root)?;
        let logical_path = relative.to_string_lossy().replace('\\', "/");
        let file_name = entry.file_name().to_string_lossy().to_string();
        let compressed_size = entry.metadata()?.len();

        insert_candidate(
            out,
            max_candidates,
            PreviewImageCandidate {
                source_ref: PreviewImageSourceRef {
                    package_id: package_id.to_owned(),
                    logical_path,
                },
                file_name: file_name.clone(),
                compressed_size,
                priority: candidate_priority(&file_name),
            },
        );
    }

    Ok(())
}

fn ensure_not_cancelled(cancellation_token: &dyn CancellationToken) -> Result<()> {
    if cancellation_token.is_cancelled() {
        anyhow::bail!("preview image scan cancelled");
    }

    Ok(())
}

fn insert_candidate(
    candidates: &mut Vec<PreviewImageCandidate>,
    max_candidates: usize,
    candidate: PreviewImageCandidate,
) {
    if max_candidates == 0 {
        return;
    }

    candidates.push(candidate);
    candidates.sort_by_key(|candidate| {
        (
            candidate.priority,
            candidate.source_ref.logical_path.to_ascii_lowercase(),
        )
    });
    candidates.truncate(max_candidates);
}

fn has_image_extension(path: &Path) -> bool {
    let Some(extension) = path.extension().and_then(|value| value.to_str()) else {
        return false;
    };

    matches!(
        extension.to_ascii_lowercase().as_str(),
        "png" | "jpg" | "jpeg" | "webp"
    )
}

fn candidate_priority(file_name: &str) -> u16 {
    let stem = Path::new(file_name)
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    match stem.as_str() {
        "preview" => 0,
        "cover" => 1,
        "poster" => 2,
        "thumbnail" => 3,
        "image" => 4,
        _ => 10,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hmm_core::PreviewImagePolicy;
    use hmm_ports::{
        CancellationToken, NeverCancelled, PackagePreviewScanner, PreviewImageScanRequest,
    };

    #[test]
    fn scanner_prefers_preview_names_and_limits_count() {
        let temp = tempfile::tempdir().expect("temp dir");
        std::fs::write(temp.path().join("zzz.jpg"), b"").expect("write zzz");
        std::fs::write(temp.path().join("preview.png"), b"").expect("write preview");
        std::fs::write(temp.path().join("cover.webp"), b"").expect("write cover");

        let policy = PreviewImagePolicy {
            max_candidates_per_package: 2,
            ..PreviewImagePolicy::default()
        };

        let scanner = SandboxPackagePreviewScanner;
        let candidates = scanner
            .scan_candidates(PreviewImageScanRequest {
                package_id: "pkg-1",
                sandbox_root: temp.path(),
                policy: &policy,
                cancellation_token: &NeverCancelled,
            })
            .expect("scan candidates");

        assert_eq!(candidates.len(), 2);
        assert_eq!(candidates[0].file_name, "preview.png");
        assert_eq!(candidates[1].file_name, "cover.webp");
        assert_eq!(candidates[0].source_ref.logical_path, "preview.png");
    }

    #[test]
    fn scanner_ignores_non_image_extensions() {
        let temp = tempfile::tempdir().expect("temp dir");
        std::fs::write(temp.path().join("readme.txt"), b"").expect("write readme");

        let scanner = SandboxPackagePreviewScanner;
        let candidates = scanner
            .scan_candidates(PreviewImageScanRequest {
                package_id: "pkg-1",
                sandbox_root: temp.path(),
                policy: &PreviewImagePolicy::default(),
                cancellation_token: &NeverCancelled,
            })
            .expect("scan candidates");

        assert!(candidates.is_empty());
    }

    #[test]
    fn scanner_bounds_candidate_count_during_traversal() {
        let temp = tempfile::tempdir().expect("temp dir");
        for index in 0..100 {
            std::fs::write(temp.path().join(format!("candidate-{index:03}.png")), b"")
                .expect("write candidate");
        }

        let policy = PreviewImagePolicy {
            max_candidates_per_package: 3,
            ..PreviewImagePolicy::default()
        };

        let scanner = SandboxPackagePreviewScanner;
        let candidates = scanner
            .scan_candidates(PreviewImageScanRequest {
                package_id: "pkg-1",
                sandbox_root: temp.path(),
                policy: &policy,
                cancellation_token: &NeverCancelled,
            })
            .expect("scan candidates");

        assert_eq!(candidates.len(), 3);
        assert_eq!(candidates[0].file_name, "candidate-000.png");
        assert_eq!(candidates[1].file_name, "candidate-001.png");
        assert_eq!(candidates[2].file_name, "candidate-002.png");
    }

    #[test]
    fn scanner_stops_when_cancelled() {
        let temp = tempfile::tempdir().expect("temp dir");
        std::fs::write(temp.path().join("preview.png"), b"").expect("write preview");

        let scanner = SandboxPackagePreviewScanner;
        let error = scanner
            .scan_candidates(PreviewImageScanRequest {
                package_id: "pkg-1",
                sandbox_root: temp.path(),
                policy: &PreviewImagePolicy::default(),
                cancellation_token: &AlwaysCancelled,
            })
            .expect_err("cancelled scan fails");

        assert!(error.to_string().contains("cancelled"));
    }

    struct AlwaysCancelled;

    impl CancellationToken for AlwaysCancelled {
        fn is_cancelled(&self) -> bool {
            true
        }
    }
}
