use crate::install::ModId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModMetadataOverlay {
    pub mod_id: ModId,
    pub display_name: Option<String>,
    pub author: Option<String>,
    pub version: Option<String>,
    pub description: Option<String>,
    pub nexus_mod_id: Option<u64>,
    pub updated_at: u128,
}
