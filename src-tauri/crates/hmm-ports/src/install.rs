use anyhow::Result;
use hmm_core::{
    InstallManifest, InstallRecoveryRecord, InstallTargetPath, ModId, PackageFileId, ProfileId,
};

pub trait InstallSourceFileReader: Send + Sync {
    fn read_source_file(&self, package_file_id: &PackageFileId) -> Result<Vec<u8>>;
}

pub trait InstallGameFileSystem: Send + Sync {
    fn read_game_file(&self, target_path: &InstallTargetPath) -> Result<Option<Vec<u8>>>;
    fn write_game_file(&self, target_path: &InstallTargetPath, bytes: &[u8]) -> Result<()>;
    fn remove_game_file(&self, target_path: &InstallTargetPath) -> Result<()>;
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
