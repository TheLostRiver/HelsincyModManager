use anyhow::Result;
use hmm_core::{ModId, ModRevisionId};
use serde::{Deserialize, Serialize};

/// Facts captured when a revision enters the library. Missing legacy values stay unknown.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModRevisionStatistics {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub imported_at_unix_millis: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_size_bytes: Option<u64>,
}

impl ModRevisionStatistics {
    pub fn is_empty(&self) -> bool {
        self.imported_at_unix_millis.is_none() && self.content_size_bytes.is_none()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModRevisionSizeUpdate {
    pub mod_id: ModId,
    pub revision_id: ModRevisionId,
    pub package_id: String,
    pub content_size_bytes: u64,
}

/// Reads only regular-file lengths below a controlled package root, without following links.
pub trait ModPackageSizeReader: Send + Sync {
    fn read_content_size(&self, package_id: &str) -> Result<u64>;
}
