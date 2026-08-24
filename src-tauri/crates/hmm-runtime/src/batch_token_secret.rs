//! Production batch token 的 per-installation secret。
//!
//! Sandbox batch token 的 key 从调用方声明的隔离根派生（可推导的 stale tag，
//! 不是认证凭据）；Production token 必须不可离线伪造，因此 key 是安装实例
//! 首次需要时生成的 256-bit 随机值，落在 app data 的 `secrets/` 下：
//!
//! - 文件内容为 64 个小写 hex 字符（32 字节随机），一行。
//! - 读取走 no-follow 语义：路径存在但不是普通文件（symlink/junction/目录）
//!   一律 fail closed，不跟随。
//! - 内容非法或长度不符时原子轮换（重新生成）——旧 token 全部失效，方向永远
//!   朝安全侧；token 本身只有 30 分钟有效期，轮换代价只是重新 preview。
//! - 写入使用同目录临时文件 + rename；与并发进程竞争时以重读为准，保证两个
//!   进程最终使用同一份 secret。
//! - secret 不进日志、不进机器输出、不进诊断导出（`secrets/` 不在任何导出
//!   或日志读取路径上）。

use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

const SECRET_DIRECTORY: &str = "secrets";
const SECRET_FILE: &str = "batch-token-secret-v1";
const SECRET_HEX_LENGTH: usize = 64;

pub(crate) fn load_or_create_batch_token_secret(app_data_dir: &Path) -> anyhow::Result<Vec<u8>> {
    let path = secret_path(app_data_dir);

    if let Some(secret) = read_valid_secret(&path)? {
        return Ok(secret);
    }

    let generated = generate_secret_hex()?;
    write_secret_atomically(&path, &generated)?;

    // 并发进程可能刚好同时轮换：rename 的最终赢家才是共识 secret，写完必须
    // 以磁盘上的最终内容为准，而不是本进程刚生成的那一份。
    read_valid_secret(&path)?
        .ok_or_else(|| anyhow::anyhow!("batch token secret unreadable after rotation"))
}

fn secret_path(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join(SECRET_DIRECTORY).join(SECRET_FILE)
}

fn read_valid_secret(path: &Path) -> anyhow::Result<Option<Vec<u8>>> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_file() => {}
        Ok(_) => anyhow::bail!("batch token secret path is not a regular file"),
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    }
    let raw = fs::read_to_string(path)?;
    let trimmed = raw.trim();
    if trimmed.len() != SECRET_HEX_LENGTH
        || !trimmed
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
    {
        // 内容损坏：按注释顶部的策略轮换，不尝试修复或沿用。
        return Ok(None);
    }
    Ok(Some(trimmed.as_bytes().to_vec()))
}

fn generate_secret_hex() -> anyhow::Result<String> {
    let mut bytes = [0_u8; SECRET_HEX_LENGTH / 2];
    getrandom::fill(&mut bytes).map_err(|error| anyhow::anyhow!("os rng unavailable: {error}"))?;
    let mut hex = String::with_capacity(SECRET_HEX_LENGTH);
    for byte in bytes {
        use std::fmt::Write as _;
        write!(&mut hex, "{byte:02x}").expect("writing to String cannot fail");
    }
    Ok(hex)
}

fn write_secret_atomically(path: &Path, secret_hex: &str) -> anyhow::Result<()> {
    let directory = path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("batch token secret path has no parent"))?;
    fs::create_dir_all(directory)?;

    let temp_path = directory.join(format!("{SECRET_FILE}.tmp-{}", std::process::id()));
    fs::write(&temp_path, format!("{secret_hex}\n"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&temp_path, fs::Permissions::from_mode(0o600));
    }

    if let Err(rename_error) = fs::rename(&temp_path, path) {
        let _ = fs::remove_file(&temp_path);
        // Windows 上 rename 不覆盖已存在文件：并发进程已经完成轮换时，
        // 竞争失败方直接采用磁盘上的赢家（调用方随后重读）。
        if fs::symlink_metadata(path).is_err() {
            return Err(rename_error.into());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secret_is_created_once_and_stable_across_loads() {
        let root = tempfile::tempdir().expect("app data root");

        let first = load_or_create_batch_token_secret(root.path()).expect("create secret");
        let second = load_or_create_batch_token_secret(root.path()).expect("reload secret");

        assert_eq!(first, second);
        assert_eq!(first.len(), SECRET_HEX_LENGTH);
        let on_disk = fs::read_to_string(secret_path(root.path())).expect("secret file");
        assert_eq!(on_disk.trim().as_bytes(), first.as_slice());
    }

    #[test]
    fn corrupted_secret_rotates_instead_of_being_reused() {
        let root = tempfile::tempdir().expect("app data root");
        let first = load_or_create_batch_token_secret(root.path()).expect("create secret");

        fs::write(secret_path(root.path()), "not-a-valid-secret").expect("corrupt secret");
        let rotated = load_or_create_batch_token_secret(root.path()).expect("rotate secret");

        assert_ne!(rotated, first);
        assert_eq!(rotated.len(), SECRET_HEX_LENGTH);
    }

    #[test]
    fn non_regular_file_secret_path_fails_closed() {
        let root = tempfile::tempdir().expect("app data root");
        fs::create_dir_all(secret_path(root.path())).expect("directory in secret slot");

        assert!(load_or_create_batch_token_secret(root.path()).is_err());
    }
}
