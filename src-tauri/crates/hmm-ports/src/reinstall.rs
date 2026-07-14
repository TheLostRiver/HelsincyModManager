use anyhow::Result;
use hmm_core::{InstallTargetPath, ModId, ProfileId, ReinstallRecoveryTransaction};

pub trait ReinstallRecoveryTransactionRepository: Send + Sync {
    fn load_transaction(
        &self,
        profile_id: &ProfileId,
        mod_id: &ModId,
    ) -> Result<Option<ReinstallRecoveryTransaction>>;
    fn list_transactions(
        &self,
        profile_id: &ProfileId,
    ) -> Result<Vec<ReinstallRecoveryTransaction>>;
    fn save_transaction(&self, transaction: &ReinstallRecoveryTransaction) -> Result<()>;
    fn remove_transaction(&self, profile_id: &ProfileId, mod_id: &ModId) -> Result<()>;
}

pub trait ReinstallSnapshotStore: Send + Sync {
    fn store_snapshot(&self, target_path: &InstallTargetPath, bytes: &[u8]) -> Result<String>;
    fn read_snapshot(&self, snapshot_ref: &str) -> Result<Option<Vec<u8>>>;
    fn remove_snapshot(&self, snapshot_ref: &str) -> Result<()>;
}
