use anyhow::{bail, Context, Result};
use hmm_ports::SystemDirectoryOpener;
use std::fs;
use std::path::Path;
use std::process::{Command, Stdio};

/// 用系统文件管理器打开目录。
///
/// 打开前必须自己复核目标仍是普通目录:路径来自持久化配置,而配置写入到用户点击之间
/// 目标可能已被替换成 symlink、junction 或普通文件。对文件调用系统打开等于用默认程序
/// 执行它,所以这里只接受目录,不接受任何其他文件类型。
pub struct SystemShellDirectoryOpener;

impl SystemShellDirectoryOpener {
    pub fn new() -> Self {
        Self
    }
}

impl Default for SystemShellDirectoryOpener {
    fn default() -> Self {
        Self::new()
    }
}

impl SystemDirectoryOpener for SystemShellDirectoryOpener {
    fn open_directory(&self, path: &Path) -> Result<()> {
        let metadata = fs::symlink_metadata(path).context("failed to inspect directory")?;
        if is_symlink_or_reparse_point(&metadata) {
            bail!("refusing to open a linked or reparse-point directory");
        }
        if !metadata.is_dir() {
            bail!("refusing to open a path that is not a regular directory");
        }
        spawn_file_manager(path)
    }
}

#[cfg(windows)]
fn spawn_file_manager(path: &Path) -> Result<()> {
    use std::path::PathBuf;

    // explorer 解析不了混合分隔符:持久化下来的路径可能形如
    // `D:\Steam\userdata\405331074\582010/remote`(Rust 的 fs API 不在乎,
    // 所以校验会通过),但 explorer 拿到后无法定位,会静默回退到打开默认位置——
    // 表现就是「点了存档目录却打开了文档」。传给 shell 前必须统一成反斜杠。
    let normalized = PathBuf::from(path.to_string_lossy().replace('/', "\\"));
    // 路径按单个参数传入,不拼接命令行字符串,避免任何注入面。
    // explorer 在成功时也可能返回非零退出码,因此只 spawn 不等待退出码。
    Command::new("explorer.exe")
        .arg(&normalized)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .context("failed to launch the system file manager")?;
    Ok(())
}

#[cfg(target_os = "macos")]
fn spawn_file_manager(path: &Path) -> Result<()> {
    Command::new("open")
        .arg(path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .context("failed to launch the system file manager")?;
    Ok(())
}

#[cfg(all(unix, not(target_os = "macos")))]
fn spawn_file_manager(path: &Path) -> Result<()> {
    Command::new("xdg-open")
        .arg(path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .context("failed to launch the system file manager")?;
    Ok(())
}

#[cfg(windows)]
fn is_symlink_or_reparse_point(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;

    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;
    metadata.file_type().is_symlink()
        || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn is_symlink_or_reparse_point(metadata: &fs::Metadata) -> bool {
    metadata.file_type().is_symlink()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opening_a_regular_file_is_refused_before_any_launch() {
        let fixture = tempfile::tempdir().expect("fixture directory");
        let file = fixture.path().join("not-a-directory.txt");
        fs::write(&file, b"fixture").expect("write fixture file");

        let error = SystemShellDirectoryOpener::new()
            .open_directory(&file)
            .expect_err("a regular file must be refused");
        assert!(error.to_string().contains("not a regular directory"));
    }

    #[test]
    fn opening_a_missing_path_is_refused() {
        let fixture = tempfile::tempdir().expect("fixture directory");

        assert!(SystemShellDirectoryOpener::new()
            .open_directory(&fixture.path().join("missing"))
            .is_err());
    }

    #[cfg(unix)]
    #[test]
    fn opening_a_symlinked_directory_is_refused_without_following_it() {
        let fixture = tempfile::tempdir().expect("fixture directory");
        let target = fixture.path().join("target");
        fs::create_dir(&target).expect("create target directory");
        let link = fixture.path().join("link");
        std::os::unix::fs::symlink(&target, &link).expect("create symlink");

        let error = SystemShellDirectoryOpener::new()
            .open_directory(&link)
            .expect_err("a symlinked directory must be refused");
        assert!(error.to_string().contains("linked or reparse-point"));
    }
}
