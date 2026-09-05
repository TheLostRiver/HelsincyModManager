use crate::install_commit::{
    atomic_write_file, contained_path, ensure_contained_existing_path, ensure_existing_directory,
    ensure_nearest_existing_ancestor_contained, ensure_safe_write_target,
    FileSystemInstallSourceFileReader,
};
use anyhow::{Context, Result};
use hmm_core::{InstallPlan, InstallTargetPath, PackageFileId};
use hmm_ports::{
    ContentTransformDispatchError, ContentTransformRequest, ContentTransformerRegistry,
    InstallSourceFileReader, RetargetStagingError, RetargetStagingFile,
    RetargetStagingMaterializer,
};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub struct FileSystemRetargetStagingMaterializer {
    staging_root: PathBuf,
    source_files: Arc<dyn InstallSourceFileReader>,
    transformers: Arc<ContentTransformerRegistry>,
}

impl FileSystemRetargetStagingMaterializer {
    pub fn new(staging_root: PathBuf, source_files: Arc<dyn InstallSourceFileReader>) -> Self {
        Self {
            staging_root,
            source_files,
            transformers: Arc::new(ContentTransformerRegistry::empty()),
        }
    }

    pub fn new_with_registry(
        staging_root: PathBuf,
        source_files: Arc<dyn InstallSourceFileReader>,
        transformers: Arc<ContentTransformerRegistry>,
    ) -> Self {
        Self {
            staging_root,
            source_files,
            transformers,
        }
    }

    fn pending_root(&self) -> Result<PathBuf, RetargetStagingError> {
        let parent = self
            .staging_root
            .parent()
            .ok_or(RetargetStagingError::DestinationUnavailable)?;
        let file_name = self
            .staging_root
            .file_name()
            .ok_or(RetargetStagingError::DestinationUnavailable)?;
        let mut pending_name = OsString::from(".");
        pending_name.push(file_name);
        pending_name.push(".partial");
        Ok(parent.join(pending_name))
    }

    fn materialize_pending(
        &self,
        pending_root: &Path,
        files: &[RetargetStagingFile],
    ) -> Result<(), RetargetStagingError> {
        for file in files {
            let source_bytes = self
                .source_files
                .read_source_file(file.package_file_id())
                .map_err(|_| RetargetStagingError::SourceUnavailable)?;
            let bytes = self.materialize_content(file, source_bytes)?;
            let target = contained_path(pending_root, file.target_path().as_str())
                .map_err(|_| RetargetStagingError::UnsafeTarget)?;
            let parent = target.parent().ok_or(RetargetStagingError::UnsafeTarget)?;
            ensure_nearest_existing_ancestor_contained(pending_root, parent)
                .map_err(|_| RetargetStagingError::UnsafeTarget)?;
            fs::create_dir_all(parent).map_err(|_| RetargetStagingError::WriteFailed)?;
            ensure_contained_existing_path(pending_root, parent)
                .map_err(|_| RetargetStagingError::UnsafeTarget)?;
            ensure_safe_write_target(pending_root, &target)
                .map_err(|_| RetargetStagingError::UnsafeTarget)?;
            atomic_write_file(&target, &bytes).map_err(|_| RetargetStagingError::WriteFailed)?;
        }
        Ok(())
    }

    fn materialize_content(
        &self,
        file: &RetargetStagingFile,
        source_bytes: Vec<u8>,
    ) -> Result<Vec<u8>, RetargetStagingError> {
        let Some(invocation) = file.content_transform() else {
            return Ok(source_bytes);
        };
        if sha256_hex(&source_bytes) != invocation.source_content_sha256() {
            return Err(RetargetStagingError::SourceDigestMismatch);
        }

        let mut dependencies = BTreeMap::new();
        for (package_file_id, expected_sha256) in invocation.dependencies() {
            let bytes = self
                .source_files
                .read_source_file(package_file_id)
                .map_err(|_| RetargetStagingError::SourceUnavailable)?;
            if sha256_hex(&bytes) != *expected_sha256 {
                return Err(RetargetStagingError::SourceDigestMismatch);
            }
            dependencies.insert(package_file_id.clone(), bytes);
        }

        let output = self
            .transformers
            .transform(ContentTransformRequest::new(
                invocation,
                file.package_file_id(),
                &source_bytes,
                &dependencies,
            ))
            .map_err(|error| match error {
                ContentTransformDispatchError::TransformerUnavailable => {
                    RetargetStagingError::TransformerUnavailable
                }
                ContentTransformDispatchError::TransformFailed(error) => {
                    RetargetStagingError::TransformFailed {
                        code: error.code().to_owned(),
                    }
                }
            })?;
        if output.canonical_mapping_sha256() != invocation.canonical_mapping_sha256()
            || sha256_hex(output.bytes()) != invocation.output_content_sha256()
        {
            return Err(RetargetStagingError::TransformOutputInvalid);
        }
        Ok(output.into_bytes())
    }

    fn cleanup_pending(
        pending_root: &Path,
        original: RetargetStagingError,
    ) -> RetargetStagingError {
        match fs::remove_dir_all(pending_root) {
            Ok(()) => original,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => original,
            Err(_) => RetargetStagingError::CleanupFailed,
        }
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

impl RetargetStagingMaterializer for FileSystemRetargetStagingMaterializer {
    fn materialize(&self, files: &[RetargetStagingFile]) -> Result<(), RetargetStagingError> {
        validate_batch(files)?;
        let pending_root = self.pending_root()?;
        let parent = self
            .staging_root
            .parent()
            .ok_or(RetargetStagingError::DestinationUnavailable)?;
        fs::create_dir_all(parent).map_err(|_| RetargetStagingError::DestinationUnavailable)?;
        ensure_existing_directory(parent, "retarget staging parent")
            .map_err(|_| RetargetStagingError::DestinationUnavailable)?;

        if self.staging_root.exists() || pending_root.exists() {
            return Err(RetargetStagingError::DestinationUnavailable);
        }
        fs::create_dir(&pending_root).map_err(|_| RetargetStagingError::DestinationUnavailable)?;
        ensure_existing_directory(&pending_root, "retarget staging pending root").map_err(
            |error| {
                Self::cleanup_pending(
                    &pending_root,
                    if error.to_string().is_empty() {
                        RetargetStagingError::DestinationUnavailable
                    } else {
                        RetargetStagingError::UnsafeTarget
                    },
                )
            },
        )?;

        if let Err(error) = self.materialize_pending(&pending_root, files) {
            return Err(Self::cleanup_pending(&pending_root, error));
        }
        if self.staging_root.exists() {
            return Err(Self::cleanup_pending(
                &pending_root,
                RetargetStagingError::DestinationUnavailable,
            ));
        }
        if fs::rename(&pending_root, &self.staging_root).is_err() {
            return Err(Self::cleanup_pending(
                &pending_root,
                RetargetStagingError::PublishFailed,
            ));
        }
        Ok(())
    }
}

fn validate_batch(files: &[RetargetStagingFile]) -> Result<(), RetargetStagingError> {
    if files.is_empty() {
        return Err(RetargetStagingError::EmptyBatch);
    }
    let mut package_file_ids = BTreeSet::new();
    let mut case_folded_targets = BTreeSet::new();
    for file in files {
        if file.package_file_id().as_str().trim().is_empty()
            || !package_file_ids.insert(file.package_file_id().clone())
        {
            return Err(RetargetStagingError::DuplicatePackageFile);
        }
        if !case_folded_targets.insert(file.target_path().as_str().to_lowercase()) {
            return Err(RetargetStagingError::CaseInsensitiveTargetCollision);
        }
    }
    Ok(())
}

struct StagedSourceFile {
    reader: Arc<FileSystemInstallSourceFileReader>,
    target_path: InstallTargetPath,
}

/// 按 `package_file_id` 路由的安装源读取器。
///
/// `#349` 切片③b：一个计划可以同时包含多个绑定的重定向产出与「保持原位」的原包文件，
/// 所以「源」不再是一个根。受重定向的文件各自落在自己绑定的 staging 根下（内部按
/// `target_path` 布局）；其余文件交给 `passthrough`（沙箱原包，按 `package_file_id` 读）。
///
/// 路由由**组装计划的一方**给出，不从 `InstallPlan` 反推——见 `RetargetSourceRouting`。
pub struct RetargetStagingInstallSourceFileReader {
    staged: BTreeMap<PackageFileId, StagedSourceFile>,
    passthrough: Option<Arc<dyn InstallSourceFileReader>>,
}

impl RetargetStagingInstallSourceFileReader {
    /// 单根：计划里的每个动作都来自同一个 staging 根，没有直通文件。
    pub fn from_install_plan(staging_root: PathBuf, plan: &InstallPlan) -> Result<Self> {
        let roots = plan
            .actions
            .iter()
            .map(|action| {
                (
                    action.provider.package_file_id.clone(),
                    staging_root.clone(),
                )
            })
            .collect::<BTreeMap<_, _>>();
        Self::routed(roots, plan, None)
    }

    /// 多根 + 直通：`staging_roots` 给出每个**受重定向**文件落在哪个根下，不在其中的
    /// `package_file_id` 交给 `passthrough`。
    ///
    /// 至少要有一个受重定向文件——整个计划都直通时调用方该直接用沙箱读取器，别绕这一层。
    pub fn routed(
        staging_roots: BTreeMap<PackageFileId, PathBuf>,
        plan: &InstallPlan,
        passthrough: Option<Arc<dyn InstallSourceFileReader>>,
    ) -> Result<Self> {
        if staging_roots.is_empty() {
            anyhow::bail!("retarget staging plan mapping is empty");
        }
        let mut readers_by_root =
            BTreeMap::<PathBuf, Arc<FileSystemInstallSourceFileReader>>::new();
        for staging_root in staging_roots.values() {
            if readers_by_root.contains_key(staging_root) {
                continue;
            }
            ensure_existing_directory(staging_root, "retarget staging root")?;
            ensure_contained_existing_path(staging_root, staging_root)?;
            readers_by_root.insert(
                staging_root.clone(),
                Arc::new(FileSystemInstallSourceFileReader::new(staging_root.clone())),
            );
        }

        let mut staged = BTreeMap::new();
        let mut routed_count = 0usize;
        for action in &plan.actions {
            let Some(staging_root) = staging_roots.get(&action.provider.package_file_id) else {
                continue;
            };
            routed_count += 1;
            // staging 内部按 `target_path` 布局，所以计划不能在这一步再改写目标路径
            // （层叠重映射）——否则按 `target_path` 去 staging 里取文件会取不到。
            // 同一个受重定向文件出现两次也是歧义：两条动作的目标路径不同，取哪个都可能错。
            if action.target_path != action.provider.target_path
                || staged
                    .insert(
                        action.provider.package_file_id.clone(),
                        StagedSourceFile {
                            reader: Arc::clone(&readers_by_root[staging_root]),
                            target_path: action.target_path.clone(),
                        },
                    )
                    .is_some()
            {
                anyhow::bail!("retarget staging plan mapping is ambiguous");
            }
        }
        // 路由里出现了计划中没有的文件，说明组装方与计划已经不同步，宁可拒绝也不要
        // 半信半疑地提交。
        if routed_count != staging_roots.len() {
            anyhow::bail!("retarget staging routing references files outside the plan");
        }
        Ok(Self {
            staged,
            passthrough,
        })
    }
}

impl InstallSourceFileReader for RetargetStagingInstallSourceFileReader {
    fn read_source_file(&self, package_file_id: &PackageFileId) -> Result<Vec<u8>> {
        if let Some(staged) = self.staged.get(package_file_id) {
            return staged
                .reader
                .read_source_file(&PackageFileId::new(staged.target_path.as_str()));
        }
        self.passthrough
            .as_ref()
            .context("retarget staging package file is not mapped")?
            .read_source_file(package_file_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hmm_core::InstallTargetPath;

    struct LinkingSourceReader {
        pending_root: PathBuf,
        outside_root: PathBuf,
    }

    impl InstallSourceFileReader for LinkingSourceReader {
        fn read_source_file(&self, _package_file_id: &PackageFileId) -> Result<Vec<u8>> {
            fs::create_dir_all(&self.outside_root)?;
            create_directory_link(&self.outside_root, &self.pending_root.join("nativePC"))?;
            Ok(b"must-not-escape".to_vec())
        }
    }

    #[cfg(unix)]
    fn create_directory_link(source: &Path, target: &Path) -> std::io::Result<()> {
        std::os::unix::fs::symlink(source, target)
    }

    #[cfg(windows)]
    fn create_directory_link(source: &Path, target: &Path) -> std::io::Result<()> {
        std::os::windows::fs::symlink_dir(source, target)
    }

    #[test]
    fn staging_rejects_intermediate_directory_link_escape() {
        let temp = tempfile::tempdir().expect("temp root");
        let staging_root = temp.path().join("staging");
        let pending_root = temp.path().join(".staging.partial");
        let outside_root = temp.path().join("outside");
        let materializer = FileSystemRetargetStagingMaterializer::new(
            staging_root.clone(),
            Arc::new(LinkingSourceReader {
                pending_root: pending_root.clone(),
                outside_root: outside_root.clone(),
            }),
        );
        let target =
            InstallTargetPath::parse("nativePC/escape.bin", ["nativePC"]).expect("target path");

        let result = materializer.materialize(&[RetargetStagingFile::new(
            PackageFileId::new("source.bin"),
            target,
        )]);

        match result {
            Err(RetargetStagingError::SourceUnavailable) if cfg!(windows) => {
                // Windows may deny symlink creation when Developer Mode is disabled.
            }
            other => assert_eq!(other, Err(RetargetStagingError::UnsafeTarget)),
        }
        assert!(!staging_root.exists());
        assert!(!pending_root.exists());
        assert!(!outside_root.join("escape.bin").exists());
    }
}
