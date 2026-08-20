use hmm_ports::{GameDirectoryProbe, GameDirectoryProbeFactory};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

pub struct RealGameDirectoryProbe {
    root_dir: PathBuf,
}

impl RealGameDirectoryProbe {
    pub fn new(root_dir: PathBuf) -> Self {
        Self { root_dir }
    }

    fn join_relative(&self, relative_path: &str) -> PathBuf {
        self.root_dir.join(relative_path)
    }
}

impl GameDirectoryProbe for RealGameDirectoryProbe {
    fn root_dir(&self) -> &Path {
        &self.root_dir
    }

    fn root_exists(&self) -> bool {
        self.root_dir.is_dir()
    }

    fn exists(&self, relative_path: &str) -> bool {
        self.join_relative(relative_path).exists()
    }

    fn is_file(&self, relative_path: &str) -> bool {
        self.join_relative(relative_path).is_file()
    }

    fn is_dir(&self, relative_path: &str) -> bool {
        self.join_relative(relative_path).is_dir()
    }

    fn read_text_file(&self, relative_path: &str) -> anyhow::Result<String> {
        Ok(std::fs::read_to_string(self.join_relative(relative_path))?)
    }

    fn sha256_hex(&self, relative_path: &str) -> anyhow::Result<String> {
        let bytes = std::fs::read(self.join_relative(relative_path))?;
        let digest = Sha256::digest(bytes);
        Ok(digest.iter().map(|byte| format!("{byte:02x}")).collect())
    }

    /// Windows 上只看只读属性判断不出可写性——真正决定权限的是 ACL、
    /// 目录所有权和安全软件的锁。唯一可靠的办法是实际试写一次。
    ///
    /// 这个探针刻意做到不可能破坏玩家数据：
    /// - 文件名带纳秒时间戳，且用 `create_new`，已存在就失败而不是覆盖；
    /// - 只写在游戏根目录下，不进 `nativePC`；
    /// - 无论成功失败都立即删除。
    fn root_writable(&self) -> bool {
        if !self.root_exists() {
            return false;
        }

        let probe_path = self.root_dir.join(format!(
            ".hmm-write-probe-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|elapsed| elapsed.as_nanos())
                .unwrap_or_default()
        ));

        let created = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&probe_path);
        let writable = created.is_ok();
        drop(created);
        // 试写成功就必须清理干净，不给玩家目录留垃圾。
        let _ = std::fs::remove_file(&probe_path);

        writable
    }
}

pub struct RealGameDirectoryProbeFactory;

impl GameDirectoryProbeFactory for RealGameDirectoryProbeFactory {
    fn create(&self, directory: PathBuf) -> Box<dyn GameDirectoryProbe> {
        Box::new(RealGameDirectoryProbe::new(directory))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn probe_checks_files_relative_to_root() {
        let root = std::env::temp_dir().join(format!(
            "hmm-probe-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        ));
        fs::create_dir_all(root.join("nativePC")).expect("create dir");
        fs::write(root.join("MonsterHunterWorld.exe"), b"fake exe").expect("write file");

        let probe = RealGameDirectoryProbe::new(root);

        assert!(probe.root_exists());
        assert!(probe.is_file("MonsterHunterWorld.exe"));
        assert!(probe.is_dir("nativePC"));
    }

    #[test]
    fn writable_probe_reports_true_and_leaves_no_residue() {
        let root = std::env::temp_dir().join(format!(
            "hmm-probe-writable-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        ));
        fs::create_dir_all(&root).expect("create dir");
        fs::write(root.join("keep.txt"), "keep").expect("write sentinel");

        let probe = RealGameDirectoryProbe::new(root.clone());
        assert!(probe.root_writable());

        // 探针必须清理干净：只留下原本就有的文件。
        let remaining = fs::read_dir(&root)
            .expect("read dir")
            .map(|entry| {
                entry
                    .expect("entry")
                    .file_name()
                    .to_string_lossy()
                    .to_string()
            })
            .collect::<Vec<_>>();
        assert_eq!(remaining, vec!["keep.txt".to_owned()]);
    }

    #[test]
    fn writable_probe_reports_false_for_a_missing_root() {
        // 目录不存在时必须 fail closed，不能因为"试写失败原因不明"而放行。
        let missing = std::env::temp_dir().join(format!(
            "hmm-probe-missing-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        ));

        assert!(!RealGameDirectoryProbe::new(missing).root_writable());
    }

    #[test]
    fn probe_reads_text_and_computes_sha256_for_relative_files() {
        let root = std::env::temp_dir().join(format!(
            "hmm-probe-hash-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        ));
        fs::create_dir_all(&root).expect("create dir");
        fs::write(root.join("notes.txt"), "abc").expect("write file");

        let probe = RealGameDirectoryProbe::new(root);

        assert_eq!(probe.read_text_file("notes.txt").expect("read text"), "abc");
        assert_eq!(
            probe.sha256_hex("notes.txt").expect("hash file"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}
