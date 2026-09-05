use crate::content_root::{resolve_content_root, ContentRootResolution};
use crate::controlled_fs::{
    ensure_regular_file_metadata, open_existing_directory_chain, open_existing_directory_nofollow,
    open_regular_file_nofollow,
};
use anyhow::{Context, Result};
use hmm_ports::{
    ModPackageContentEntry, ModPackageContentRoot, ModPackageContentScanRequest,
    ModPackageContentScanner, ModPackageContents, ModPackageInstallFile,
    ModPackageInstallFileReadRequest, ModPackageInstallFileReader, ModPackageInstallFileScanError,
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
    ) -> Result<Vec<ModPackageInstallFile>, ModPackageInstallFileScanError> {
        // #284：第三方压缩包普遍在 `nativePC` 外套一层甚至多层包装目录，而安装
        // 路径过滤要求目标路径以 `nativePC` 打头——若仍以沙箱根为基准算路径，
        // 整包都会被过滤掉，装成一个空计划（#285）。
        //
        // 因此先解析出真正的**内容根**，再据此计算目标路径。这里与预览图扫描
        // 共用 `resolve_content_root`，保证两边对「内容根在哪」判断一致，
        // 不会出现「图能显示、却装不上」。
        let resolution = resolve_content_root(request.sandbox_root)
            .map_err(|_| ModPackageInstallFileScanError::Unavailable)?;
        let Some(content_root) = resolution.install_root() else {
            // 多个 nativePC 不是「坏包」，而是需要玩家自己决定。返回枚举而不是
            // 笼统错误，好让上层把它呈现成可操作的提示（#284 review 的 R1）。
            return Err(ModPackageInstallFileScanError::AmbiguousContentRoot);
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

impl ModPackageContentScanner for SandboxModPackageInstallFileScanner {
    fn scan_package_contents(
        &self,
        request: ModPackageContentScanRequest<'_>,
    ) -> Result<ModPackageContents, ModPackageInstallFileScanError> {
        // 与 `scan_install_files` 共用同一份内容根解析——两处对「内容根在哪」必须同结论，
        // 否则会重演 #284 的「图能显示、却装不上」。差别只在**处置**：那边遇到多个
        // nativePC 直接失败，这里如实报告候选，因为玩家要挑就得先看得见。
        let resolution = resolve_content_root(request.sandbox_root)
            .map_err(|_| ModPackageInstallFileScanError::Unavailable)?;
        let content_root = content_root_from_resolution(request.sandbox_root, &resolution)?;

        // 从**沙箱根**而不是内容根开始遍历：包装目录之外的 readme、预览图同样属于包内容，
        // 玩家要能看见它们才谈得上「知道这个包里有什么」。是否可安装由上层按内容根分档。
        let mut entries = Vec::new();
        collect_sandbox_content_entries(
            request.sandbox_root,
            request.sandbox_root,
            0,
            &mut entries,
        )?;
        entries.sort_by(|left, right| left.package_file_id.cmp(&right.package_file_id));
        Ok(ModPackageContents {
            entries,
            content_root,
        })
    }
}

fn content_root_from_resolution(
    sandbox_root: &Path,
    resolution: &ContentRootResolution,
) -> Result<ModPackageContentRoot, ModPackageInstallFileScanError> {
    let relative = |directory: &Path| -> Result<String, ModPackageInstallFileScanError> {
        let stripped = directory
            .strip_prefix(sandbox_root)
            .map_err(|_| ModPackageInstallFileScanError::Unavailable)?;
        sandbox_relative_directory_path(stripped)
            .map_err(|_| ModPackageInstallFileScanError::Unavailable)
    };

    Ok(match resolution {
        // `Fallback` 的根恒等于沙箱根，没有第二种取值，因此不带载荷。
        ContentRootResolution::Fallback(_) => ModPackageContentRoot::Fallback,
        ContentRootResolution::Single(directory) => {
            ModPackageContentRoot::Single(relative(directory)?)
        }
        ContentRootResolution::Ambiguous(directories) => ModPackageContentRoot::Ambiguous(
            directories
                .iter()
                .map(|directory| relative(directory))
                .collect::<Result<Vec<_>, _>>()?,
        ),
    })
}

/// 遍历沙箱内全部文件。
///
/// 防御与 [`collect_sandbox_install_files`] **逐条相同**（符号链接拒绝、深度上限、只收
/// 常规文件），因为它们面对的是同一个不可信沙箱。这里唯一放宽的是**起点**：从沙箱根而不是
/// 内容根开始。放宽起点不放宽校验——路径仍然逐段过 `sandbox_relative_segments`。
fn collect_sandbox_content_entries(
    sandbox_root: &Path,
    directory: &Path,
    depth: usize,
    entries: &mut Vec<ModPackageContentEntry>,
) -> Result<(), ModPackageInstallFileScanError> {
    if depth > MAX_SANDBOX_INSTALL_FILE_SCAN_DEPTH {
        return Err(ModPackageInstallFileScanError::DepthLimitExceeded);
    }

    let read_dir =
        fs::read_dir(directory).map_err(|_| ModPackageInstallFileScanError::Unavailable)?;

    for entry in read_dir {
        let entry = entry.map_err(|_| ModPackageInstallFileScanError::Unavailable)?;
        let path = entry.path();
        let metadata =
            fs::symlink_metadata(&path).map_err(|_| ModPackageInstallFileScanError::Unavailable)?;

        if metadata.file_type().is_symlink() {
            return Err(ModPackageInstallFileScanError::UnsupportedEntry);
        }

        if metadata.is_dir() {
            collect_sandbox_content_entries(sandbox_root, &path, depth + 1, entries)?;
            continue;
        }

        if !metadata.is_file() {
            continue;
        }

        let package_relative = path
            .strip_prefix(sandbox_root)
            .map_err(|_| ModPackageInstallFileScanError::Unavailable)?;
        let package_file_id = sandbox_install_relative_path(package_relative)
            .map_err(|_| ModPackageInstallFileScanError::Unavailable)?;

        entries.push(ModPackageContentEntry {
            package_file_id,
            size_bytes: metadata.len(),
        });
    }

    Ok(())
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
) -> Result<(), ModPackageInstallFileScanError> {
    if depth > MAX_SANDBOX_INSTALL_FILE_SCAN_DEPTH {
        return Err(ModPackageInstallFileScanError::DepthLimitExceeded);
    }

    let entries =
        fs::read_dir(directory).map_err(|_| ModPackageInstallFileScanError::Unavailable)?;

    for entry in entries {
        let entry = entry.map_err(|_| ModPackageInstallFileScanError::Unavailable)?;
        let path = entry.path();
        let metadata =
            fs::symlink_metadata(&path).map_err(|_| ModPackageInstallFileScanError::Unavailable)?;

        if metadata.file_type().is_symlink() {
            return Err(ModPackageInstallFileScanError::UnsupportedEntry);
        }

        if metadata.is_dir() {
            collect_sandbox_install_files(sandbox_root, content_root, &path, depth + 1, files)?;
            continue;
        }

        if !metadata.is_file() {
            continue;
        }

        // 下面两处 strip_prefix 与路径规范化在正常流程下不会失败（path 就是从
        // 对应根遍历出来的），保留判定只是防御性：真失败时统一归为 Unavailable，
        // 不把内部路径细节往外带。
        let package_relative = path
            .strip_prefix(sandbox_root)
            .map_err(|_| ModPackageInstallFileScanError::Unavailable)?;
        let package_file_id = sandbox_install_relative_path(package_relative)
            .map_err(|_| ModPackageInstallFileScanError::Unavailable)?;

        let content_relative = path
            .strip_prefix(content_root)
            .map_err(|_| ModPackageInstallFileScanError::Unavailable)?;
        let target_path = sandbox_install_relative_path(content_relative)
            .map_err(|_| ModPackageInstallFileScanError::Unavailable)?;

        files.push(ModPackageInstallFile {
            package_file_id,
            target_path,
        });
    }

    Ok(())
}

fn sandbox_relative_segments(path: &Path) -> Result<Vec<String>> {
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

    Ok(segments)
}

fn sandbox_install_relative_path(path: &Path) -> Result<String> {
    let segments = sandbox_relative_segments(path)?;

    if segments.is_empty() {
        anyhow::bail!("imported mod sandbox path is empty");
    }

    Ok(segments.join("/"))
}

/// 目录版：与 [`sandbox_install_relative_path`] 共用同一套逐段校验，唯一的差别是**允许空串**
/// ——内容根可以就是沙箱根本身，那时它的相对路径正是空。文件路径为空则始终是错误。
fn sandbox_relative_directory_path(path: &Path) -> Result<String> {
    Ok(sandbox_relative_segments(path)?.join("/"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use hmm_core::PackageFileId;
    use hmm_ports::{ModPackageInstallFileReadRequest, ModPackageInstallFileScanRequest};
    use std::fs;

    fn scan_contents(sandbox_root: &Path) -> ModPackageContents {
        SandboxModPackageInstallFileScanner
            .scan_package_contents(ModPackageContentScanRequest {
                package_id: "package-a",
                sandbox_root,
            })
            .expect("scan package contents")
    }

    fn content_paths(contents: &ModPackageContents) -> Vec<&str> {
        contents
            .entries
            .iter()
            .map(|entry| entry.package_file_id.as_str())
            .collect()
    }

    /*
     * #354 D1 的核心用例，也是 D2（手动指定内容根）能不能做的前提。
     *
     * `scan_install_files` 在多个 nativePC 时返回 `AmbiguousContentRoot`，一个文件都拿不到，
     * 于是玩家看到的只有「请拆分后分别导入」。内容根有歧义是**要玩家决定的状态**，不是失败
     * ——而玩家能决定的前提是先看得见整包。断言到具体清单而不是「没报错」：只断言 `is_ok()`
     * 会漏掉「返回了但只列了其中一个 nativePC 之下的文件」这类更隐蔽的错误。
     */
    #[test]
    fn package_contents_are_listed_even_when_the_content_root_is_ambiguous() {
        let temp = tempfile::tempdir().expect("temp dir");
        let sandbox_root = temp.path().join("package-a");
        fs::create_dir_all(sandbox_root.join("大剑/nativePC/wp")).expect("create fixture");
        fs::create_dir_all(sandbox_root.join("太刀/nativePC/wp")).expect("create fixture");
        fs::write(sandbox_root.join("大剑/nativePC/wp/two003.mod3"), b"a").expect("write fixture");
        fs::write(sandbox_root.join("太刀/nativePC/wp/swo035.mod3"), b"bb").expect("write fixture");
        fs::write(sandbox_root.join("readme.txt"), b"ccc").expect("write fixture");

        let contents = scan_contents(&sandbox_root);

        assert_eq!(
            contents.content_root,
            ModPackageContentRoot::Ambiguous(vec!["大剑".to_owned(), "太刀".to_owned()]),
            "两个 nativePC 的父目录都要如实列成候选，不能替玩家挑一个"
        );
        assert_eq!(
            content_paths(&contents),
            vec![
                "readme.txt",
                "大剑/nativePC/wp/two003.mod3",
                "太刀/nativePC/wp/swo035.mod3",
            ],
            "歧义状态下必须列出整包，包括两个候选各自之下的文件"
        );
    }

    /*
     * 起点从沙箱根而不是内容根开始，是本方法与 `scan_install_files` 的**唯一**差别。
     * 钉住它：包装目录之外的 readme 属于包内容，玩家要能看见；它可不可安装是上层的分档，
     * 不是扫描侧提前替玩家删掉的理由。
     */
    #[test]
    fn package_contents_include_files_outside_the_content_root() {
        let temp = tempfile::tempdir().expect("temp dir");
        let sandbox_root = temp.path().join("package-a");
        fs::create_dir_all(sandbox_root.join("黑骑士大剑/nativePC/wp")).expect("create fixture");
        fs::write(
            sandbox_root.join("黑骑士大剑/nativePC/wp/two003.mod3"),
            b"a",
        )
        .expect("write fixture");
        fs::write(sandbox_root.join("黑骑士大剑/预览.png"), b"bb").expect("write fixture");
        fs::write(sandbox_root.join("readme.txt"), b"ccc").expect("write fixture");

        let contents = scan_contents(&sandbox_root);

        assert_eq!(
            contents.content_root,
            ModPackageContentRoot::Single("黑骑士大剑".to_owned())
        );
        assert_eq!(
            content_paths(&contents),
            vec![
                "readme.txt",
                "黑骑士大剑/nativePC/wp/two003.mod3",
                "黑骑士大剑/预览.png",
            ]
        );
        assert_eq!(
            contents
                .entries
                .iter()
                .map(|entry| entry.size_bytes)
                .collect::<Vec<_>>(),
            vec![3, 1, 2]
        );
    }

    /// 沙箱根直接就是内容根时，相对路径是空串——这是 `sandbox_relative_directory_path`
    /// 与文件版唯一的行为差异，单独钉住。
    #[test]
    fn a_content_root_at_the_sandbox_root_is_reported_as_an_empty_relative_path() {
        let temp = tempfile::tempdir().expect("temp dir");
        let sandbox_root = temp.path().join("package-a");
        fs::create_dir_all(sandbox_root.join("nativePC/wp")).expect("create fixture");
        fs::write(sandbox_root.join("nativePC/wp/two003.mod3"), b"a").expect("write fixture");

        assert_eq!(
            scan_contents(&sandbox_root).content_root,
            ModPackageContentRoot::Single(String::new())
        );
    }

    /// 包里没有 `nativePC`：回退为沙箱根，而不是失败。有没有可安装的东西由上层判定
    /// （与 `resolve_content_root` 的 `Fallback` 语义一致，这里不抢先下结论）。
    #[test]
    fn a_package_without_any_native_pc_falls_back_instead_of_failing() {
        let temp = tempfile::tempdir().expect("temp dir");
        let sandbox_root = temp.path().join("package-a");
        fs::create_dir_all(&sandbox_root).expect("create fixture");
        fs::write(sandbox_root.join("readme.txt"), b"a").expect("write fixture");

        let contents = scan_contents(&sandbox_root);

        assert_eq!(contents.content_root, ModPackageContentRoot::Fallback);
        assert_eq!(content_paths(&contents), vec!["readme.txt"]);
    }

    /*
     * 放宽起点**不放宽校验**。重解析点在 `scan_install_files` 上是失败关闭的，从沙箱根起步
     * 同样必须失败关闭——沙箱不可信这一点与遍历起点无关。这里 junction 指向沙箱之外，
     * 不拦就等于把沙箱外的文件列进包内容。
     *
     * 用 `mklink /J`（目录 junction）而不是符号链接：符号链接在未开启开发者模式的
     * Windows 上创建会失败，写成「创建失败就 return」的测试会**恒绿且不承重**——本条最初
     * 就是那么写的，反向验证（把闸门改成 `if false`）照样通过，属于假绿。junction 不需要
     * 特权，且这里**断言**创建成功，造不出来就转红而不是跳过。
     */
    #[cfg(windows)]
    #[test]
    fn package_content_scanning_rejects_reparse_points_like_the_install_file_scan_does() {
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
            .scan_package_contents(ModPackageContentScanRequest {
                package_id: "package-a",
                sandbox_root: &sandbox_root,
            })
            .expect_err("指向沙箱外的重解析点必须失败关闭");

        assert_eq!(error, ModPackageInstallFileScanError::UnsupportedEntry);
    }

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

    // 嵌套的 `nativePC`（`sandbox/nativePC/nativePC/...`）：取**最浅**的那个，
    // 内容根 = sandbox，且不深入其内部——里面不可能有「更该被当作内容根」的层级。
    #[test]
    fn sandbox_install_file_scanner_prefers_the_shallowest_native_pc() {
        let temp = tempfile::tempdir().expect("temp dir");
        let sandbox_root = temp.path().join("package-a");
        fs::create_dir_all(sandbox_root.join("nativePC/nativePC/models")).expect("create fixture");
        fs::write(
            sandbox_root.join("nativePC/nativePC/models/player.mod3"),
            b"model",
        )
        .expect("write fixture");

        let files = SandboxModPackageInstallFileScanner
            .scan_install_files(ModPackageInstallFileScanRequest {
                package_id: "package-a",
                sandbox_root: &sandbox_root,
            })
            .expect("scan sandbox files");

        // 内容根 = sandbox，因此 target_path 从最外层 nativePC 算起。
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].target_path, "nativePC/nativePC/models/player.mod3");
    }

    // 「根级 nativePC + 包装目录里还有一个」= 两个 nativePC，必须拒绝。
    #[test]
    fn sandbox_install_file_scanner_refuses_a_root_native_pc_alongside_a_wrapped_one() {
        let temp = tempfile::tempdir().expect("temp dir");
        let sandbox_root = temp.path().join("package-a");
        fs::create_dir_all(sandbox_root.join("nativePC/models")).expect("create fixture");
        fs::create_dir_all(sandbox_root.join("wrapper/nativePC/models")).expect("create fixture");
        fs::write(sandbox_root.join("nativePC/models/a.mod3"), b"a").expect("write a");
        fs::write(sandbox_root.join("wrapper/nativePC/models/b.mod3"), b"b").expect("write b");

        let error = SandboxModPackageInstallFileScanner
            .scan_install_files(ModPackageInstallFileScanRequest {
                package_id: "package-a",
                sandbox_root: &sandbox_root,
            })
            .expect_err("two nativePC directories must not be guessed");

        assert_eq!(error, ModPackageInstallFileScanError::AmbiguousContentRoot);
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

        // 用枚举而非字符串匹配——这正是 #284 R1 想要的：原因要能被上层**区分**，
        // 而不是混在笼统的「扫描失败」里。
        assert_eq!(error, ModPackageInstallFileScanError::AmbiguousContentRoot);
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

        assert_eq!(error, ModPackageInstallFileScanError::DepthLimitExceeded);
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
        assert_eq!(
            error,
            ModPackageInstallFileScanError::UnsupportedEntry,
            "junction must be classified as an unsupported entry"
        );
        // 读取链路仍走 anyhow（它不需要可判定的分类），这里保持字符串匹配。
        assert!(
            read_error.to_string().contains("directory")
                || read_error.to_string().contains("reparse")
                || read_error.to_string().contains("link"),
            "unexpected reader error: {read_error}"
        );
    }
}
