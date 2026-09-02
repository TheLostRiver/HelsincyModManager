use anyhow::Result;
use hmm_core::{
    InstallManifest, InstallRecoveryRecord, InstallTargetPath, ModId, PackageFileId, ProfileId,
};
use std::time::SystemTime;

pub trait InstallSourceFileReader: Send + Sync {
    fn read_source_file(&self, package_file_id: &PackageFileId) -> Result<Vec<u8>>;
}

pub trait InstallGameFileSystem: Send + Sync {
    fn read_game_file(&self, target_path: &InstallTargetPath) -> Result<Option<Vec<u8>>>;
    fn write_game_file(&self, target_path: &InstallTargetPath, bytes: &[u8]) -> Result<()>;
    fn remove_game_file(&self, target_path: &InstallTargetPath) -> Result<()>;
}

/// 游戏目录里某个目标文件的廉价指纹，用于 TOCTOU 复核。
///
/// 与 `InstallGameFileSystem` 分开的原因：读写文件是**写入事务**的能力，而
/// 只 stat 是**只读观测**的能力。合进同一个 trait 会逼每个假实现都为不需要的
/// 方法填 `unreachable!`，也会让「这层只需要观测」的调用方背上写方法。
///
/// 刻意只包含廉价可得且跨平台稳定的字段。mtime 用 `SystemTime` 而不是 u64：
/// 让实现决定如何取，调用方只比较相等性（见 `GameFileFingerprint::matches`）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameFileFingerprint {
    pub size_bytes: u64,
    pub modified: Option<SystemTime>,
}

impl GameFileFingerprint {
    /// 两次观测是否指向同一个文件状态。
    ///
    /// 拿不到 mtime 时（`None`）退化为只比 size——**弱判据但不假称强判据**：
    /// 调用方据此丢弃结果（fail-closed），不会因此误报「没变」。
    pub fn matches(&self, other: &Self) -> bool {
        match (self.modified, other.modified) {
            (Some(left), Some(right)) => self.size_bytes == other.size_bytes && left == right,
            _ => self.size_bytes == other.size_bytes,
        }
    }
}

/// 只读观测游戏目录里的文件，不读内容。
///
/// 存在的理由：外部 MOD 状态扫描要在**游戏写锁内**做前后两次复核，而写锁内不得做
/// 长时间 hash（项目 5 处文档明令）。读全文再算摘要在几百 MB 的文件上是不可接受的，
/// 所以复核只能用 stat。
pub trait InstallGameFileInspector: Send + Sync {
    /// 文件不存在返回 `Ok(None)`；存在但 stat 不到（权限、穿越、非普通文件）返回 `Err`。
    fn stat_game_file(
        &self,
        target_path: &InstallTargetPath,
    ) -> Result<Option<GameFileFingerprint>>;
}

pub trait InstallBackupStore: Send + Sync {
    fn store_backup(&self, target_path: &InstallTargetPath, bytes: &[u8]) -> Result<String>;
    fn read_backup(&self, backup_ref: &str) -> Result<Option<Vec<u8>>>;
    fn remove_backup(&self, backup_ref: &str) -> Result<()>;
}

pub trait InstallManifestRepository: Send + Sync {
    fn load_manifest(&self, profile_id: &ProfileId) -> Result<Option<InstallManifest>>;
    fn save_manifest(&self, manifest: &InstallManifest) -> Result<()>;
}

pub trait InstallRecoveryRecordRepository: Send + Sync {
    fn load_record(
        &self,
        profile_id: &ProfileId,
        mod_id: &ModId,
    ) -> Result<Option<InstallRecoveryRecord>>;
    fn list_records(&self, profile_id: &ProfileId) -> Result<Vec<InstallRecoveryRecord>>;
    fn save_record(&self, record: &InstallRecoveryRecord) -> Result<()>;
    fn remove_record(&self, profile_id: &ProfileId, mod_id: &ModId) -> Result<()>;
}
