use crate::content_root::native_pc_parents;
use anyhow::Result;
use hmm_ports::{
    CancellationToken, PackagePreviewScanner, PreviewImageCandidate, PreviewImageScanRequest,
    PreviewImageSourceRef,
};
use std::path::Path;

// `nativePC` 父目录的发现已抽到 `crate::content_root`，**与安装文件扫描共用**
// （#284）。两处必须对「内容根在哪」给出同一个答案，否则会出现
// 「预览图能显示、安装却装不上」——那正是本条 issue 的现象。
//
// 深度上限同样来自 `content_root`（现在是 4）。它比原先的 64 严格，但现实里
// 不存在超过 4 层包装的 MOD 包；统一之后，行为也更可预测。

pub struct SandboxPackagePreviewScanner;

impl PackagePreviewScanner for SandboxPackagePreviewScanner {
    fn scan_candidates(
        &self,
        request: PreviewImageScanRequest<'_>,
    ) -> Result<Vec<PreviewImageCandidate>> {
        let mut candidates = Vec::new();
        let candidate_roots =
            find_candidate_roots(request.sandbox_root, request.cancellation_token)?;

        for candidate_root in candidate_roots {
            collect_direct_candidates(
                request.package_id,
                request.sandbox_root,
                &candidate_root,
                request.policy.max_candidates_per_package,
                request.cancellation_token,
                &mut candidates,
            )?;
        }

        Ok(candidates)
    }
}

/// 找出候选图的搜索根目录。
///
/// 与安装文件扫描共用 `crate::content_root` 的解析（#284）。这里的语义比安装侧
/// **宽松**：多个 `nativePC` 时全部收图，而安装侧遇到多个必须拒绝（合集包不替
/// 用户挑一个）。两者不冲突——预览图负责「让你看见」，安装负责「不擅自决定」。
fn find_candidate_roots(
    sandbox_root: &Path,
    _cancellation_token: &dyn CancellationToken,
) -> Result<Vec<std::path::PathBuf>> {
    let mut roots = native_pc_parents(sandbox_root)?;

    if roots.is_empty() {
        roots.insert(sandbox_root.to_path_buf());
    }

    Ok(roots.into_iter().collect())
}

fn collect_direct_candidates(
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
            candidate.source_ref.logical_path.clone(),
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
    use crate::content_root::MAX_CONTENT_ROOT_SEARCH_DEPTH;
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
    fn scanner_only_collects_direct_images_next_to_native_pc() {
        let temp = tempfile::tempdir().expect("temp dir");
        let package_root = temp.path().join("外层包装");
        std::fs::create_dir_all(package_root.join("nativePC/textures"))
            .expect("create nativePC tree");
        std::fs::create_dir_all(package_root.join("screenshots")).expect("create screenshots tree");
        std::fs::write(package_root.join("中文预览图 #1!.png"), b"")
            .expect("write unicode preview");
        std::fs::write(package_root.join("123.webp"), b"").expect("write numeric preview");
        std::fs::write(package_root.join("nativePC/textures/in-game.png"), b"")
            .expect("write nativePC texture");
        std::fs::write(package_root.join("screenshots/nested.jpg"), b"")
            .expect("write nested screenshot");
        std::fs::write(temp.path().join("archive-root.jpg"), b"")
            .expect("write archive root image");

        let scanner = SandboxPackagePreviewScanner;
        let candidates = scanner
            .scan_candidates(PreviewImageScanRequest {
                package_id: "pkg-1",
                sandbox_root: temp.path(),
                policy: &PreviewImagePolicy::default(),
                cancellation_token: &NeverCancelled,
            })
            .expect("scan candidates");

        assert_eq!(
            candidates
                .iter()
                .map(|candidate| candidate.file_name.as_str())
                .collect::<Vec<_>>(),
            vec!["123.webp", "中文预览图 #1!.png"]
        );
        assert!(candidates
            .iter()
            .all(|candidate| candidate.source_ref.logical_path.starts_with("外层包装/")));
    }

    #[test]
    fn scanner_matches_native_pc_case_insensitively() {
        let temp = tempfile::tempdir().expect("temp dir");
        let package_root = temp.path().join("wrapped");
        std::fs::create_dir_all(package_root.join("NATIVEpc")).expect("create mixed-case nativePC");
        std::fs::write(package_root.join("cover.jpg"), b"").expect("write cover");

        let scanner = SandboxPackagePreviewScanner;
        let candidates = scanner
            .scan_candidates(PreviewImageScanRequest {
                package_id: "pkg-1",
                sandbox_root: temp.path(),
                policy: &PreviewImagePolicy::default(),
                cancellation_token: &NeverCancelled,
            })
            .expect("scan candidates");

        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].source_ref.logical_path, "wrapped/cover.jpg");
    }

    #[test]
    fn scanner_without_native_pc_only_collects_sandbox_root_images() {
        let temp = tempfile::tempdir().expect("temp dir");
        std::fs::create_dir_all(temp.path().join("assets")).expect("create assets");
        std::fs::write(temp.path().join("root-preview.png"), b"").expect("write root preview");
        std::fs::write(temp.path().join("assets/nested-preview.png"), b"")
            .expect("write nested preview");

        let scanner = SandboxPackagePreviewScanner;
        let candidates = scanner
            .scan_candidates(PreviewImageScanRequest {
                package_id: "pkg-1",
                sandbox_root: temp.path(),
                policy: &PreviewImagePolicy::default(),
                cancellation_token: &NeverCancelled,
            })
            .expect("scan candidates");

        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].source_ref.logical_path, "root-preview.png");
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
    fn scanner_stops_native_pc_discovery_at_the_depth_limit() {
        let temp = tempfile::tempdir().expect("temp dir");
        std::fs::write(temp.path().join("root-preview.png"), b"")
            .expect("write sandbox-root preview");

        let mut deep_path = temp.path().to_path_buf();
        for _ in 0..(MAX_CONTENT_ROOT_SEARCH_DEPTH + 2) {
            deep_path.push("d");
        }
        std::fs::create_dir_all(&deep_path).expect("create deep tree");
        std::fs::create_dir_all(deep_path.join("nativePC")).expect("create over-deep nativePC");
        std::fs::write(deep_path.join("deep-preview.png"), b"").expect("write deep preview");

        let scanner = SandboxPackagePreviewScanner;
        let candidates = scanner
            .scan_candidates(PreviewImageScanRequest {
                package_id: "pkg-1",
                sandbox_root: temp.path(),
                policy: &PreviewImagePolicy::default(),
                cancellation_token: &NeverCancelled,
            })
            .expect("depth-limited scan succeeds without recursion overflow");

        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].source_ref.logical_path, "root-preview.png");
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
