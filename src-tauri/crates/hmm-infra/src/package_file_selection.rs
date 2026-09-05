//! 玩家在某个包里勾掉的文件的持久化（`#354` 切片 D3）。
//!
//! 形状与 [`crate::JsonModPackageContentRootRepository`] 一致：一条记录一个 JSON 文件，
//! 文件名取 `package_id` 的摘要，原子写，读写都走 `ensure_contained_existing_path` 防越界。
//!
//! 存的是**排除集合**而不是包含集合，理由见端口
//! [`hmm_ports::ModPackageFileSelectionRepository`] 的文档：默认全选必须是空集合（计划才能
//! 逐字不变），且包重新解压出新文件时要照常安装而不是静默漏装。

use crate::install_commit::{
    atomic_write_file, ensure_contained_existing_path, ensure_existing_directory,
};
use anyhow::{Context, Result};
use hmm_ports::ModPackageFileSelectionRepository;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

pub struct JsonModPackageFileSelectionRepository {
    root: PathBuf,
}

#[derive(Serialize, Deserialize)]
struct StoredFileSelection {
    schema_version: u32,
    package_id: String,
    excluded_files: Vec<String>,
}

impl JsonModPackageFileSelectionRepository {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    fn selection_path(&self, package_id: &str) -> PathBuf {
        let mut hasher = Sha256::new();
        hasher.update(b"package:");
        hasher.update(package_id.as_bytes());
        let digest_hex: String = hasher
            .finalize()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();

        self.root.join(format!("file-selection-{digest_hex}.json"))
    }

    fn deserialize_selection(&self, path: &Path, package_id: &str) -> Result<Vec<String>> {
        let serialized = fs::read_to_string(path).context("failed to read file selection")?;
        let stored: StoredFileSelection =
            serde_json::from_str(&serialized).context("failed to deserialize file selection")?;
        // 摘要碰撞或人为改名都会让记录对不上号。fail closed——「按别的包的勾选去装」与
        // 「没勾过」后果完全不同，不能靠运气区分。
        if stored.package_id != package_id {
            anyhow::bail!("file selection does not match the requested package");
        }
        Ok(stored.excluded_files)
    }
}

impl ModPackageFileSelectionRepository for JsonModPackageFileSelectionRepository {
    fn load_excluded_files(&self, package_id: &str) -> Result<Vec<String>> {
        match fs::symlink_metadata(&self.root) {
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(error).context("failed to inspect file selection root"),
        }

        ensure_existing_directory(&self.root, "file selection root")?;
        ensure_contained_existing_path(&self.root, &self.root)?;
        let selection_path = self.selection_path(package_id);
        let metadata = match fs::symlink_metadata(&selection_path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(error).context("failed to inspect file selection"),
        };
        if metadata.is_symlink() || !metadata.is_file() {
            anyhow::bail!("file selection is not a regular file");
        }
        ensure_contained_existing_path(&self.root, &selection_path)?;
        self.deserialize_selection(&selection_path, package_id)
    }

    fn save_excluded_files(&self, package_id: &str, excluded: &[String]) -> Result<()> {
        if !self.root.exists() {
            fs::create_dir_all(&self.root).context("failed to create file selection root")?;
        }
        ensure_existing_directory(&self.root, "file selection root")?;
        ensure_contained_existing_path(&self.root, &self.root)?;
        // 排序 + 去重：同一份勾选不该因为提交顺序不同而写出两种字节，否则记录的 diff 会
        // 噪声化，也让「有没有变过」难以判断。
        let mut excluded_files = excluded.to_vec();
        excluded_files.sort_unstable();
        excluded_files.dedup();
        let stored = StoredFileSelection {
            schema_version: 1,
            package_id: package_id.to_owned(),
            excluded_files,
        };
        let serialized =
            serde_json::to_vec_pretty(&stored).context("failed to serialize file selection")?;
        atomic_write_file(&self.selection_path(package_id), &serialized)
    }

    fn clear_excluded_files(&self, package_id: &str) -> Result<()> {
        match fs::symlink_metadata(&self.root) {
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(error).context("failed to inspect file selection root"),
        }

        let selection_path = self.selection_path(package_id);
        match fs::symlink_metadata(&selection_path) {
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(error).context("failed to inspect file selection"),
        }
        ensure_contained_existing_path(&self.root, &selection_path)?;
        fs::remove_file(&selection_path).context("failed to remove file selection")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_saved_selection_round_trips_sorted_and_deduplicated() {
        let temp = tempfile::tempdir().expect("temp dir");
        let repository = JsonModPackageFileSelectionRepository::new(temp.path().join("selections"));

        assert!(repository
            .load_excluded_files("package-a")
            .expect("load")
            .is_empty());

        repository
            .save_excluded_files(
                "package-a",
                &["b.exe".to_owned(), "a.dll".to_owned(), "b.exe".to_owned()],
            )
            .expect("save");

        assert_eq!(
            repository.load_excluded_files("package-a").expect("load"),
            vec!["a.dll".to_owned(), "b.exe".to_owned()],
            "同一份勾选不该因为提交顺序或重复项写出不同字节"
        );
        assert!(
            repository
                .load_excluded_files("package-b")
                .expect("load")
                .is_empty(),
            "记录按包键，别的包不受影响"
        );

        repository.clear_excluded_files("package-a").expect("clear");
        assert!(repository
            .load_excluded_files("package-a")
            .expect("load")
            .is_empty());
    }

    /// 显式存一个空集合与「从没勾过」在读取侧等价——两者都表示「全都装」。
    /// 这正是选排除集合而不是包含集合的好处：不需要哨兵值区分。
    #[test]
    fn an_empty_exclusion_set_reads_back_as_install_everything() {
        let temp = tempfile::tempdir().expect("temp dir");
        let repository = JsonModPackageFileSelectionRepository::new(temp.path().join("selections"));

        repository
            .save_excluded_files("package-a", &[])
            .expect("save");

        assert!(repository
            .load_excluded_files("package-a")
            .expect("load")
            .is_empty());
    }

    #[test]
    fn a_record_that_names_another_package_fails_closed() {
        let temp = tempfile::tempdir().expect("temp dir");
        let repository = JsonModPackageFileSelectionRepository::new(temp.path().join("selections"));
        repository
            .save_excluded_files("package-a", &["a.exe".to_owned()])
            .expect("save");

        let path = repository.selection_path("package-a");
        let tampered = serde_json::to_vec_pretty(&StoredFileSelection {
            schema_version: 1,
            package_id: "package-b".to_owned(),
            excluded_files: vec!["a.exe".to_owned()],
        })
        .expect("serialize");
        fs::write(&path, tampered).expect("write tampered record");

        assert!(repository.load_excluded_files("package-a").is_err());
    }
}
