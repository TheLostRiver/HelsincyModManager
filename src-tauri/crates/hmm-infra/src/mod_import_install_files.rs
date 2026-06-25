use anyhow::{Context, Result};
use hmm_ports::{
    ModPackageInstallFile, ModPackageInstallFileScanRequest, ModPackageInstallFileScanner,
};
use std::fs;
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
    use hmm_ports::ModPackageInstallFileScanRequest;
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

    #[cfg(windows)]
    #[test]
    fn sandbox_install_file_scanner_rejects_windows_directory_junctions() {
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

        fs::remove_dir(&junction_path).expect("remove junction");
        assert!(
            error.to_string().contains("unsupported link") || error.to_string().contains("reparse"),
            "unexpected error: {error}"
        );
    }
}
