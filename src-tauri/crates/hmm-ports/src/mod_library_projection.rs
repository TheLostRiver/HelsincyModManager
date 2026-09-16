use anyhow::Result;
use hmm_core::{ModId, ModRevisionId, ProfileId};
use unicode_normalization::UnicodeNormalization;

use crate::StoredImportPreviewImage;

/// v3 adds natural name keys, first-import timestamps and display-revision sizes.
pub const MOD_LIBRARY_PROJECTION_SCHEMA_VERSION: u32 = 3;
pub const MOD_LIBRARY_QUERY_KEY_VERSION: &str = "mod-library-query-key-v1";

pub fn normalize_mod_library_query_key(value: &str) -> String {
    value
        .nfkc()
        .flat_map(char::to_lowercase)
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Locale-independent natural ordering. Numeric runs use their significant length and digits;
/// other text retains normalized Unicode order. This key is also used by the browser mock.
pub fn mod_library_name_sort_key(value: &str) -> Vec<u8> {
    let normalized = normalize_mod_library_query_key(value);
    let bytes = normalized.as_bytes();
    let mut key = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index].is_ascii_digit() {
            let start = index;
            while index < bytes.len() && bytes[index].is_ascii_digit() {
                index += 1;
            }
            let digits = normalized[start..index].trim_start_matches('0');
            let digits = if digits.is_empty() { "0" } else { digits };
            key.push(b'0');
            key.extend_from_slice(&(digits.len() as u64).to_be_bytes());
            key.extend_from_slice(digits.as_bytes());
            key.push(0);
        } else {
            key.push(bytes[index]);
            index += 1;
        }
    }
    key
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ModLibrarySort {
    NameAsc,
    NameDesc,
    ImportedAtAsc,
    #[default]
    ImportedAtDesc,
    SizeAsc,
    SizeDesc,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModLibraryProjectionLabel {
    pub category_id: Option<String>,
    pub name: String,
    pub color: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModLibraryProjectionRecord {
    pub mod_id: ModId,
    pub display_revision_id: ModRevisionId,
    pub package_id: String,
    pub display_name: String,
    pub author: Option<String>,
    pub version_label: Option<String>,
    pub size_label: String,
    pub imported_at_unix_millis: Option<u64>,
    pub content_size_bytes: Option<u64>,
    pub preview_image: StoredImportPreviewImage,
    pub labels: Vec<ModLibraryProjectionLabel>,
    /// 外部导入来源的 adapter id（如狩技盒子）；普通 zip 导入为 `None`。
    ///
    /// 列表页「外部来源」标记的唯一事实来源（#286 切片 3b-1）。只存 adapter id，
    /// 展示名由前端字典按 id 取词——与详情层 `ModOriginSummary` 的口径一致。
    pub external_import_adapter_id: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModLibraryProjectionStatus {
    Installed,
    CommittedCleanupPending,
    CleanupPending,
    RollbackRequired,
    RepairRequired,
}

impl ModLibraryProjectionStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Installed => "installed",
            Self::CommittedCleanupPending => "committed_cleanup_pending",
            Self::CleanupPending => "cleanup_pending",
            Self::RollbackRequired => "rollback_required",
            Self::RepairRequired => "repair_required",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModLibraryProjectionQueryStatus {
    NotInstalled,
    Installed,
    CommittedCleanupPending,
    CleanupPending,
    RollbackRequired,
    RepairRequired,
    Unknown,
}

impl ModLibraryProjectionQueryStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NotInstalled => "not_installed",
            Self::Installed => "installed",
            Self::CommittedCleanupPending => "committed_cleanup_pending",
            Self::CleanupPending => "cleanup_pending",
            Self::RollbackRequired => "rollback_required",
            Self::RepairRequired => "repair_required",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModLibraryProjectionQueryFilter {
    All,
    Status(ModLibraryProjectionQueryStatus),
    Category(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModLibraryProjectionProfileQuery {
    pub profile_id: ProfileId,
    pub source_fingerprint: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModLibraryProjectionQueryRequest {
    pub source_fingerprint: String,
    pub profile: Option<ModLibraryProjectionProfileQuery>,
    pub normalized_search: String,
    pub filter: ModLibraryProjectionQueryFilter,
    pub sort: ModLibrarySort,
    pub page: u64,
    pub page_size: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModLibraryProjectionPageItem {
    pub record: ModLibraryProjectionRecord,
    pub status: Option<ModLibraryProjectionStatusRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModLibraryProjectionQueryPage {
    pub items: Vec<ModLibraryProjectionPageItem>,
    pub page: u64,
    pub page_size: u32,
    pub library_total: usize,
    pub matching_total: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModLibraryProjectionQueryError {
    Unavailable,
    ProfileUnavailable,
    CategoryNotFound,
}

pub trait ModLibraryProjectionQueryRepository: Send + Sync {
    fn query(
        &self,
        request: &ModLibraryProjectionQueryRequest,
    ) -> std::result::Result<ModLibraryProjectionQueryPage, ModLibraryProjectionQueryError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModLibraryProjectionStatusRecord {
    pub mod_id: ModId,
    pub status: ModLibraryProjectionStatus,
    pub managed_file_count: u64,
    pub backup_count: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModLibraryProfileProjection {
    pub profile_id: ProfileId,
    pub source_fingerprint: String,
    pub statuses: Vec<ModLibraryProjectionStatusRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModLibraryProjectionSnapshot {
    pub source_fingerprint: String,
    pub records: Vec<ModLibraryProjectionRecord>,
    pub profiles: Vec<ModLibraryProfileProjection>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModLibraryProjectionReadiness {
    Dirty,
    Complete,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModLibraryProjectionState {
    pub schema_version: u32,
    pub key_version: String,
    pub generation: u64,
    pub source_fingerprint: Option<String>,
    pub readiness: ModLibraryProjectionReadiness,
}

impl ModLibraryProjectionState {
    pub fn is_complete_for(&self, source_fingerprint: &str) -> bool {
        self.schema_version == MOD_LIBRARY_PROJECTION_SCHEMA_VERSION
            && self.key_version == MOD_LIBRARY_QUERY_KEY_VERSION
            && self.readiness == ModLibraryProjectionReadiness::Complete
            && self.source_fingerprint.as_deref() == Some(source_fingerprint)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModLibraryProfileProjectionState {
    pub profile_id: ProfileId,
    pub generation: u64,
    pub source_fingerprint: Option<String>,
    pub readiness: ModLibraryProjectionReadiness,
}

impl ModLibraryProfileProjectionState {
    pub fn is_complete_for(&self, source_fingerprint: &str) -> bool {
        self.readiness == ModLibraryProjectionReadiness::Complete
            && self.source_fingerprint.as_deref() == Some(source_fingerprint)
    }
}

pub trait ModLibraryProjectionRepository: Send + Sync {
    fn state(&self) -> Result<ModLibraryProjectionState>;

    fn mark_dirty(&self, observed_source_fingerprint: Option<&str>) -> Result<()>;

    fn rebuild(&self, snapshot: &ModLibraryProjectionSnapshot)
        -> Result<ModLibraryProjectionState>;

    fn profile_state(
        &self,
        profile_id: &ProfileId,
    ) -> Result<Option<ModLibraryProfileProjectionState>>;

    fn mark_profile_dirty(
        &self,
        profile_id: &ProfileId,
        observed_source_fingerprint: Option<&str>,
    ) -> Result<()>;

    fn replace_profile(
        &self,
        projection: &ModLibraryProfileProjection,
    ) -> Result<ModLibraryProfileProjectionState>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn natural_name_keys_are_stable_across_numbers_and_compatibility_characters() {
        assert_eq!(
            mod_library_name_sort_key("Mod 02"),
            vec![109, 111, 100, 32, 48, 0, 0, 0, 0, 0, 0, 0, 1, 50, 0]
        );
        assert_eq!(
            mod_library_name_sort_key("ＭＯＤ　2"),
            mod_library_name_sort_key("Mod 02")
        );
        assert!(mod_library_name_sort_key("Mod 2") < mod_library_name_sort_key("Mod 10"));
        assert!(
            mod_library_name_sort_key("Mod 99999999999999999999999")
                < mod_library_name_sort_key("Mod 100000000000000000000000")
        );
    }

    #[test]
    fn query_key_v1_normalizes_compatibility_equivalents_and_whitespace() {
        assert_eq!(
            normalize_mod_library_query_key("  Ａrmor\u{3000}CAFÉ  "),
            "armor café"
        );
        assert_eq!(
            normalize_mod_library_query_key("Cafe\u{301}"),
            normalize_mod_library_query_key("Café")
        );
        assert_eq!(normalize_mod_library_query_key("İ"), "i\u{307}");
    }
}
