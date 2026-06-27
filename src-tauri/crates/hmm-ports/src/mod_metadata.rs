use anyhow::Result;
use hmm_core::ModMetadataOverlay;

pub trait ModMetadataRepository: Send + Sync {
    fn get(&self, mod_id: &str) -> Result<Option<ModMetadataOverlay>>;
    fn save(&self, overlay: &ModMetadataOverlay) -> Result<()>;
    fn delete(&self, mod_id: &str) -> Result<()>;
    fn list_all(&self) -> Result<Vec<ModMetadataOverlay>>;
}
