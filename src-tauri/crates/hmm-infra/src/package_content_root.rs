//! 玩家为某个包选定的内容根的持久化（`#354` 切片 D2）。
//!
//! 形状与 [`crate::JsonReplacementSelectionRepository`] 一致：每条记录一个 JSON 文件，
//! 文件名取 `package_id` 的摘要（`package_id` 可能含路径不安全的字符，不能直接当文件名），
//! 原子写，读写都走 `ensure_contained_existing_path` 防越界。
//!
//! **为什么必须持久化**：提交安装时 `start_install_task` 会从沙箱**重建**计划，重装同理。
//! 选择若只活在预览请求里，重建那一刻就没了，装出来的位置与玩家看到的预览不一致。

use crate::install_commit::{
    atomic_write_file, ensure_contained_existing_path, ensure_existing_directory,
};
use anyhow::{Context, Result};
use hmm_ports::ModPackageContentRootRepository;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

pub struct JsonModPackageContentRootRepository {
    root: PathBuf,
}

#[derive(Serialize, Deserialize)]
struct StoredContentRootChoice {
    schema_version: u32,
    package_id: String,
    /// 沙箱根相对的正斜杠路径；空串表示沙箱根本身。
    content_root: String,
}

impl JsonModPackageContentRootRepository {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    fn choice_path(&self, package_id: &str) -> PathBuf {
        let mut hasher = Sha256::new();
        hasher.update(b"package:");
        hasher.update(package_id.as_bytes());
        let digest_hex: String = hasher
            .finalize()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();

        self.root.join(format!("content-root-{digest_hex}.json"))
    }

    fn deserialize_choice(&self, path: &Path, package_id: &str) -> Result<String> {
        let serialized = fs::read_to_string(path).context("failed to read content root choice")?;
        let stored: StoredContentRootChoice = serde_json::from_str(&serialized)
            .context("failed to deserialize content root choice")?;
        // 摘要碰撞或人为改名都会让记录对不上号。这里 fail closed——「装到别的包选的位置」
        // 与「没有记录」在后果上完全不同，不能靠运气区分。
        if stored.package_id != package_id {
            anyhow::bail!("content root choice does not match the requested package");
        }
        Ok(stored.content_root)
    }
}

impl ModPackageContentRootRepository for JsonModPackageContentRootRepository {
    fn load_content_root(&self, package_id: &str) -> Result<Option<String>> {
        match fs::symlink_metadata(&self.root) {
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error).context("failed to inspect content root choice root"),
        }

        ensure_existing_directory(&self.root, "content root choice root")?;
        ensure_contained_existing_path(&self.root, &self.root)?;
        let choice_path = self.choice_path(package_id);
        let metadata = match fs::symlink_metadata(&choice_path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error).context("failed to inspect content root choice"),
        };
        if metadata.is_symlink() || !metadata.is_file() {
            anyhow::bail!("content root choice is not a regular file");
        }
        ensure_contained_existing_path(&self.root, &choice_path)?;
        Ok(Some(self.deserialize_choice(&choice_path, package_id)?))
    }

    fn save_content_root(&self, package_id: &str, content_root: &str) -> Result<()> {
        if !self.root.exists() {
            fs::create_dir_all(&self.root).context("failed to create content root choice root")?;
        }
        ensure_existing_directory(&self.root, "content root choice root")?;
        ensure_contained_existing_path(&self.root, &self.root)?;
        let stored = StoredContentRootChoice {
            schema_version: 1,
            package_id: package_id.to_owned(),
            content_root: content_root.to_owned(),
        };
        let serialized = serde_json::to_vec_pretty(&stored)
            .context("failed to serialize content root choice")?;
        atomic_write_file(&self.choice_path(package_id), &serialized)
    }

    fn clear_content_root(&self, package_id: &str) -> Result<()> {
        match fs::symlink_metadata(&self.root) {
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(error).context("failed to inspect content root choice root"),
        }

        let choice_path = self.choice_path(package_id);
        match fs::symlink_metadata(&choice_path) {
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(error).context("failed to inspect content root choice"),
        }
        ensure_contained_existing_path(&self.root, &choice_path)?;
        fs::remove_file(&choice_path).context("failed to remove content root choice")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_saved_choice_round_trips_and_can_be_cleared() {
        let temp = tempfile::tempdir().expect("temp dir");
        let repository = JsonModPackageContentRootRepository::new(temp.path().join("choices"));

        assert_eq!(
            repository.load_content_root("package-a").expect("load"),
            None
        );

        repository
            .save_content_root("package-a", "大剑")
            .expect("save");
        assert_eq!(
            repository.load_content_root("package-a").expect("load"),
            Some("大剑".to_owned())
        );

        // 别的包不受影响——记录是按包键的。
        assert_eq!(
            repository.load_content_root("package-b").expect("load"),
            None
        );

        repository.clear_content_root("package-a").expect("clear");
        assert_eq!(
            repository.load_content_root("package-a").expect("load"),
            None
        );
    }

    /// 空串是合法值：内容根就是沙箱根本身。它必须与「没有记录」区分得开，
    /// 否则玩家把根显式选成沙箱根之后，下一次又会被问一遍。
    #[test]
    fn an_empty_choice_is_a_recorded_choice_not_a_missing_one() {
        let temp = tempfile::tempdir().expect("temp dir");
        let repository = JsonModPackageContentRootRepository::new(temp.path().join("choices"));

        repository.save_content_root("package-a", "").expect("save");

        assert_eq!(
            repository.load_content_root("package-a").expect("load"),
            Some(String::new())
        );
    }

    /// 记录里的 `package_id` 对不上就 fail closed，不返回「没有记录」——
    /// 「装到别的包选的位置」与「没选过」后果完全不同，不能靠运气区分。
    #[test]
    fn a_record_that_names_another_package_fails_closed() {
        let temp = tempfile::tempdir().expect("temp dir");
        let root = temp.path().join("choices");
        let repository = JsonModPackageContentRootRepository::new(root.clone());
        repository
            .save_content_root("package-a", "大剑")
            .expect("save");

        // 直接改写记录里的 package_id，模拟摘要碰撞或人为改名。
        let path = repository.choice_path("package-a");
        let tampered = serde_json::to_vec_pretty(&StoredContentRootChoice {
            schema_version: 1,
            package_id: "package-b".to_owned(),
            content_root: "大剑".to_owned(),
        })
        .expect("serialize");
        fs::write(&path, tampered).expect("write tampered record");

        assert!(repository.load_content_root("package-a").is_err());
    }
}
