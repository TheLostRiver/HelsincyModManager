use crate::content_root::resolve_content_root;
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
        // #284：第三方压缩包普遍在 `nativePC` 外套一层甚至多层包装目录，而安装
        // 路径过滤要求目标路径以 `nativePC` 打头——若仍以沙箱根为基准算路径，
        // 整包都会被过滤掉，装成一个空计划（#285）。
        //
        // 因此先解析出真正的**内容根**，再据此计算目标路径。这里与预览图扫描
        // 共用 `resolve_content_root`，保证两边对「内容根在哪」判断一致，
        // 不会出现「图能显示、却装不上」。
        let resolution = resolve_content_root(request.sandbox_root)?;
        let Some(content_root) = resolution.install_root() else {
            anyhow::bail!(
                "imported mod package contains more than one nativePC directory; \
                 refusing to guess which one to install"
            );
        };

        let mut files = Vec::new();
        collect_sandbox_install_files(
            request.sandbox_root,
            content_root,
            content_root,
            0,
            &mut files,
        )?;
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

/// 收集可安装文件。
///
/// 两个路径字段的基准**不同**，这不是笔误：
///
/// - `package_file_id` 相对**沙箱根**——读取链路（`install_commit`、
///   `staging`、`replacement`）都以沙箱根为基准解析它，改了就读不到文件。
/// - `target_path` 相对**内容根**（`nativePC` 的父目录）——安装路径过滤要求
///   它以 `nativePC` 打头。
///
/// 内容根为沙箱根时两者相同（无包装目录的常规包），这正是改动前的行为。
fn collect_sandbox_install_files(
    sandbox_root: &Path,
    content_root: &Path,
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
            collect_sandbox_install_files(sandbox_root, content_root, &path, depth + 1, files)?;
            continue;
        }

        if !metadata.is_file() {
            continue;
        }

        let package_relative = path
            .strip_prefix(sandbox_root)
            .context("imported mod sandbox entry escaped its root")?;
        let package_file_id = sandbox_install_relative_path(package_relative)?;

        let content_relative = path
            .strip_prefix(content_root)
            .context("imported mod sandbox entry escaped its content root")?;
        let target_path = sandbox_install_relative_path(content_relative)?;

        files.push(ModPackageInstallFile {
            package_file_id,
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

    // #284：zip 里套一层包装目录是最常见的第三方形态（`黑骑士大剑/nativePC/...`）。
    // 修复前 target_path 会带上包装层，被安装路径过滤整个丢弃，装成空计划。
    #[test]
    fn sandbox_install_file_scanner_strips_a_single_wrapper_directory() {
        let temp = tempfile::tempdir().expect("temp dir");
        let sandbox_root = temp.path().join("package-a");
        fs::create_dir_all(sandbox_root.join("黑骑士大剑/nativePC/models"))
            .expect("create fixture");
        fs::write(
            sandbox_root.join("黑骑士大剑/nativePC/models/player.mod3"),
            b"model",
        )
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
                // package_file_id 相对**沙箱根**：读取链路靠它定位文件。
                package_file_id: "黑骑士大剑/nativePC/models/player.mod3".to_owned(),
                // target_path 相对**内容根**：以 nativePC 打头，能通过安装过滤。
                target_path: "nativePC/models/player.mod3".to_owned(),
            }]
        );
    }

    #[test]
    fn sandbox_install_file_scanner_strips_two_wrapper_directories() {
        let temp = tempfile::tempdir().expect("temp dir");
        let sandbox_root = temp.path().join("package-a");
        fs::create_dir_all(sandbox_root.join("outer/inner/nativePC/models")).expect("create");
        fs::write(
            sandbox_root.join("outer/inner/nativePC/models/player.mod3"),
            b"model",
        )
        .expect("write fixture");

        let files = SandboxModPackageInstallFileScanner
            .scan_install_files(ModPackageInstallFileScanRequest {
                package_id: "package-a",
                sandbox_root: &sandbox_root,
            })
            .expect("scan sandbox files");

        assert_eq!(files.len(), 1);
        assert_eq!(files[0].target_path, "nativePC/models/player.mod3");
        assert_eq!(
            files[0].package_file_id,
            "outer/inner/nativePC/models/player.mod3"
        );
    }

    // 内容根（nativePC 的父目录）**之外**的文件不属于这个 MOD，不该被扫描进来。
    //
    // 注意区分：内容根之内的同级说明文件（如 wrapper/说明.txt）**会**被扫到，
    // 但它的 target_path 不以 nativePC 打头，随后由安装路径过滤丢弃——
    // 那是下一道工序的职责，扫描阶段不越权删。
    #[test]
    fn sandbox_install_file_scanner_ignores_files_outside_the_content_root() {
        let temp = tempfile::tempdir().expect("temp dir");
        let sandbox_root = temp.path().join("package-a");
        fs::create_dir_all(sandbox_root.join("wrapper/nativePC/models")).expect("create fixture");
        fs::write(
            sandbox_root.join("wrapper/nativePC/models/player.mod3"),
            b"model",
        )
        .expect("write fixture");
        fs::write(sandbox_root.join("外层说明.txt"), b"outside").expect("write outside");

        let files = SandboxModPackageInstallFileScanner
            .scan_install_files(ModPackageInstallFileScanRequest {
                package_id: "package-a",
                sandbox_root: &sandbox_root,
            })
            .expect("scan sandbox files");

        assert_eq!(files.len(), 1, "内容根之外的文件不应进入扫描结果");
        assert_eq!(files[0].target_path, "nativePC/models/player.mod3");
        assert_eq!(
            files[0].package_file_id,
            "wrapper/nativePC/models/player.mod3"
        );
    }

    // 合集包：不替用户挑一个——静默合并会写入玩家没预期的文件。
    #[test]
    fn sandbox_install_file_scanner_refuses_several_native_pc_directories() {
        let temp = tempfile::tempdir().expect("temp dir");
        let sandbox_root = temp.path().join("package-a");
        fs::create_dir_all(sandbox_root.join("mod-a/nativePC/models")).expect("create fixture");
        fs::create_dir_all(sandbox_root.join("mod-b/nativePC/models")).expect("create fixture");
        fs::write(sandbox_root.join("mod-a/nativePC/models/a.mod3"), b"a").expect("write a");
        fs::write(sandbox_root.join("mod-b/nativePC/models/b.mod3"), b"b").expect("write b");

        let error = SandboxModPackageInstallFileScanner
            .scan_install_files(ModPackageInstallFileScanRequest {
                package_id: "package-a",
                sandbox_root: &sandbox_root,
            })
            .expect_err("several nativePC directories must not be guessed");

        assert!(
            error.to_string().contains("more than one nativePC"),
            "unexpected error: {error}"
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
