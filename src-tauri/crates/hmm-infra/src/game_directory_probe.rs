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
