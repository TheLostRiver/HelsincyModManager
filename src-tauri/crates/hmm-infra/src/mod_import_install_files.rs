use crate::controlled_fs::{
    ensure_regular_file_metadata, open_existing_directory_chain, open_existing_directory_nofollow,
    open_regular_file_nofollow,
};
use anyhow::{Context, Result};
use hmm_ports::{
    ModPackageInstallFile, ModPackageInstallFileReadRequest, ModPackageInstallFileReader,
    ModPackageInstallFileScanRequest, ModPackageInstallFileScanner,
};
use std::fs;
use std::io::Read;
use std::path::{Component, Path};

const MAX_SANDBOX_INSTALL_FILE_SCAN_DEPTH: usize = 64;

pub struct SandboxModPackageInstallFileScanner;

impl ModPackageInstallFileScanner for SandboxModPackageInstallFileScanner {
    fn scan_install_files(
        &self,
        request: ModPackageInstallFileScanRequest<'_>,
    ) -> Result<Vec<ModPackageInstallFile>> {
        let mut files = Vec::new();
        collect_sandbox_install_files(request.sandbox_root, request.sandbox_root, 0, &mut files)?;
        files.sort_by(|left, right| left.target_path.cmp(&right.target_path));
        Ok(files)
    }
}

impl ModPackageInstallFileReader for SandboxModPackageInstallFileScanner {
    fn read_install_file(&self, request: ModPackageInstallFileReadRequest<'_>) -> Result<Vec<u8>> {
        if request.max_bytes == 0 {
            anyhow::bail!("imported mod file read limit is invalid");
        }
        let relative = sandbox_install_relative_path(Path::new(request.package_file_id.as_str()))?;
        if relative != request.package_file_id.as_str() {
            anyhow::bail!("imported mod package file id is not canonical");
        }

        let mut components = relative.split('/').collect::<Vec<_>>();
        let file_name = components
            .pop()
            .context("imported mod package file id is empty")?;
        let package_root =
            open_existing_directory_nofollow(request.sandbox_root, "imported mod package root")?;
        let parent = open_existing_directory_chain(
            &package_root,
            &components,
            "imported mod package directory",
        )?;
        let mut file = open_regular_file_nofollow(
            &parent,
            std::ffi::OsStr::new(file_name),
            "imported mod package file",
        )?;
        let before = file
            .metadata()
            .context("failed to inspect opened imported mod package file")?;
        ensure_regular_file_metadata(&before, "imported mod package file")?;
        if before.len() > request.max_bytes {
            anyhow::bail!("imported mod package file exceeds read limit");
        }
        let before_modified = before
            .modified()
            .context("failed to inspect imported mod package file timestamp")?;

        let capacity = usize::try_from(before.len()).unwrap_or(0);
        let mut bytes = Vec::with_capacity(capacity);
        file.by_ref()
            .take(request.max_bytes.saturating_add(1))
            .read_to_end(&mut bytes)
            .context("failed to read imported mod package file")?;
        let bytes_read = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
        if bytes_read > request.max_bytes || bytes_read != before.len() {
            anyhow::bail!("imported mod package file exceeds read limit");
        }
        let after = file
            .metadata()
            .context("failed to re-inspect imported mod package file")?;
        ensure_regular_file_metadata(&after, "imported mod package file")?;
        if after.len() != before.len()
            || after
                .modified()
                .context("failed to re-inspect imported mod package file timestamp")?
                != before_modified
        {
            anyhow::bail!("imported mod package file changed while reading");
        }
        Ok(bytes)
    }
}

fn collect_sandbox_install_files(
    sandbox_root: &Path,
    directory: &Path,
    depth: usize,
    files: &mut Vec<ModPackageInstallFile>,
) -> Result<()> {
    if depth > MAX_SANDBOX_INSTALL_FILE_SCAN_DEPTH {
        anyhow::bail!("imported mod sandbox exceeds install file scan depth limit");
    }

    let entries = fs::read_dir(directory).context("failed to read imported mod sandbox")?;

    for entry in entries {
        let entry = entry.context("failed to read imported mod sandbox entry")?;
        let path = entry.path();
        let metadata =
            fs::symlink_metadata(&path).context("failed to inspect imported mod sandbox entry")?;

        if metadata.file_type().is_symlink() {
            anyhow::bail!("imported mod sandbox contains an unsupported link entry");
        }

        if metadata.is_dir() {
            collect_sandbox_install_files(sandbox_root, &path, depth + 1, files)?;
            continue;
        }

        if !metadata.is_file() {
            continue;
        }

        let relative_path = path
            .strip_prefix(sandbox_root)
            .context("imported mod sandbox entry escaped its root")?;
        let target_path = sandbox_install_relative_path(relative_path)?;
        files.push(ModPackageInstallFile {
            package_file_id: target_path.clone(),
            target_path,
        });
    }

    Ok(())
}

fn sandbox_install_relative_path(path: &Path) -> Result<String> {
    let mut segments = Vec::new();

    for component in path.components() {
        match component {
            Component::Normal(value) => {
                let segment = value
                    .to_str()
                    .context("imported mod sandbox path is not valid UTF-8")?;
                if segment.is_empty() {
                    anyhow::bail!("imported mod sandbox path contains an empty segment");
                }
                segments.push(segment.to_owned());
            }
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                anyhow::bail!("imported mod sandbox path is not relative");
            }
        }
    }

    if segments.is_empty() {
        anyhow::bail!("imported mod sandbox path is empty");
    }

    Ok(segments.join("/"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use hmm_core::PackageFileId;
    use hmm_ports::{ModPackageInstallFileReadRequest, ModPackageInstallFileScanRequest};
    use std::fs;

    #[test]
    fn sandbox_install_file_scanner_lists_regular_files_as_relative_targets() {
        let temp = tempfile::tempdir().expect("temp dir");
        let sandbox_root = temp.path().join("package-a");
        fs::create_dir_all(sandbox_root.join("nativePC/models")).expect("create fixture dirs");
        fs::write(sandbox_root.join("nativePC/models/player.mod3"), b"model")
            .expect("write fixture");

        let files = SandboxModPackageInstallFileScanner
            .scan_install_files(ModPackageInstallFileScanRequest {
                package_id: "package-a",
                sandbox_root: &sandbox_root,
            })
            .expect("scan sandbox files");

        assert_eq!(
            files,
            vec![ModPackageInstallFile {
                package_file_id: "nativePC/models/player.mod3".to_owned(),
                target_path: "nativePC/models/player.mod3".to_owned(),
            }]
        );
    }

    #[test]
    fn sandbox_install_file_scanner_rejects_excessive_directory_depth() {
        let temp = tempfile::tempdir().expect("temp dir");
        let sandbox_root = temp.path().join("package-a");
        let mut deep_dir = sandbox_root.clone();

        for index in 0..70 {
            deep_dir = deep_dir.join(format!("level-{index}"));
        }

        fs::create_dir_all(&deep_dir).expect("create deep fixture dirs");

        let error = SandboxModPackageInstallFileScanner
            .scan_install_files(ModPackageInstallFileScanRequest {
                package_id: "package-a",
                sandbox_root: &sandbox_root,
            })
            .expect_err("excessive depth should be rejected");

        assert!(error.to_string().contains("depth"));
    }

    #[test]
    fn sandbox_install_file_reader_enforces_canonical_id_and_size_limit() {
        let temp = tempfile::tempdir().expect("temp dir");
        let sandbox_root = temp.path().join("package-a");
        fs::create_dir_all(sandbox_root.join("nativePC/wp/one/one001/mod"))
            .expect("create fixture dirs");
        fs::write(
            sandbox_root.join("nativePC/wp/one/one001/mod/one001.mod3"),
            b"artificial",
        )
        .expect("write fixture");

        let reader = SandboxModPackageInstallFileScanner;
        let package_file_id = PackageFileId::new("nativePC/wp/one/one001/mod/one001.mod3");
        let bytes = reader
            .read_install_file(ModPackageInstallFileReadRequest {
                package_id: "package-a",
                sandbox_root: &sandbox_root,
                package_file_id: &package_file_id,
                max_bytes: 32,
            })
            .expect("read contained fixture");
        assert_eq!(bytes, b"artificial");

        let error = reader
            .read_install_file(ModPackageInstallFileReadRequest {
                package_id: "package-a",
                sandbox_root: &sandbox_root,
                package_file_id: &package_file_id,
                max_bytes: 4,
            })
            .expect_err("size limit must reject");
        assert!(error.to_string().contains("read limit"));
    }

    #[cfg(windows)]
    #[test]
    fn sandbox_install_file_access_rejects_windows_directory_junctions() {
        use std::process::Command;

        let temp = tempfile::tempdir().expect("temp dir");
        let sandbox_root = temp.path().join("package-a");
        let outside_root = temp.path().join("outside");
        let junction_path = sandbox_root.join("nativePC").join("junction");

        fs::create_dir_all(junction_path.parent().expect("junction parent"))
            .expect("create sandbox dirs");
        fs::create_dir_all(&outside_root).expect("create outside dir");
        fs::write(outside_root.join("escape.mod3"), b"outside").expect("write outside file");

        let output = Command::new("cmd")
            .args([
                "/C",
                "mklink",
                "/J",
                junction_path.to_str().expect("junction path"),
                outside_root.to_str().expect("outside path"),
            ])
            .output()
            .expect("create junction");
        assert!(
            output.status.success(),
            "mklink failed: {}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );

        let error = SandboxModPackageInstallFileScanner
            .scan_install_files(ModPackageInstallFileScanRequest {
                package_id: "package-a",
                sandbox_root: &sandbox_root,
            })
            .expect_err("junction should be rejected");

        let package_file_id = PackageFileId::new("nativePC/junction/escape.mod3");
        let read_error = SandboxModPackageInstallFileScanner
            .read_install_file(ModPackageInstallFileReadRequest {
                package_id: "package-a",
                sandbox_root: &sandbox_root,
                package_file_id: &package_file_id,
                max_bytes: 32,
            })
            .expect_err("reader must not follow a junction");

        fs::remove_dir(&junction_path).expect("remove junction");
        assert!(
            error.to_string().contains("unsupported link") || error.to_string().contains("reparse"),
            "unexpected error: {error}"
        );
        assert!(
            read_error.to_string().contains("directory")
                || read_error.to_string().contains("reparse")
                || read_error.to_string().contains("link"),
            "unexpected reader error: {read_error}"
        );
    }
}
