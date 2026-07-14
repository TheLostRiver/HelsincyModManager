use serde::{de::Error as _, Deserialize, Deserializer, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

macro_rules! string_id {
    ($name:ident) => {
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        pub struct $name(String);

        impl $name {
            pub fn new(value: impl Into<String>) -> Self {
                Self(value.into())
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }
    };
}

string_id!(ModId);
string_id!(ModRevisionId);
string_id!(ProfileId);
string_id!(PackageFileId);

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum InstallTargetPathError {
    #[error("install target path cannot be empty")]
    Empty,
    #[error("install target path cannot be absolute")]
    Absolute,
    #[error("install target path cannot include parent traversal")]
    ParentTraversal,
    #[error("install target path cannot use a Windows drive prefix")]
    WindowsDrivePrefix,
    #[error("install target path contains an empty or current-directory segment")]
    InvalidSegment,
    #[error("install target root is not allowed: {root}")]
    TargetRootNotAllowed { root: String },
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct InstallTargetPath(String);

impl InstallTargetPath {
    pub fn parse<I, R>(
        value: impl Into<String>,
        allowed_roots: I,
    ) -> Result<Self, InstallTargetPathError>
    where
        I: IntoIterator<Item = R>,
        R: AsRef<str>,
    {
        let normalized = normalize_target_path(&value.into())?;
        let root = normalized
            .split('/')
            .next()
            .expect("normalized target paths always have a root")
            .to_owned();

        let is_allowed = allowed_roots
            .into_iter()
            .filter_map(|allowed_root| normalize_allowed_root(allowed_root.as_ref()))
            .any(|allowed_root| {
                normalized == allowed_root
                    || normalized
                        .strip_prefix(&allowed_root)
                        .is_some_and(|remainder| remainder.starts_with('/'))
            });

        if !is_allowed {
            return Err(InstallTargetPathError::TargetRootNotAllowed { root });
        }

        Ok(Self(normalized))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileLayer {
    pub name: String,
    pub priority: i32,
}

impl FileLayer {
    pub fn new(name: impl Into<String>, priority: i32) -> Self {
        Self {
            name: name.into(),
            priority,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstallFileProvider {
    pub mod_id: ModId,
    pub package_file_id: PackageFileId,
    pub target_path: InstallTargetPath,
    pub layer: FileLayer,
}

impl InstallFileProvider {
    pub fn new(
        mod_id: ModId,
        package_file_id: PackageFileId,
        target_path: InstallTargetPath,
        layer: FileLayer,
    ) -> Self {
        Self {
            mod_id,
            package_file_id,
            target_path,
            layer,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstallAction {
    pub target_path: InstallTargetPath,
    pub provider: InstallFileProvider,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstallConflict {
    pub target_path: InstallTargetPath,
    pub providers: Vec<InstallFileProvider>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstallPlan {
    pub actions: Vec<InstallAction>,
    pub conflicts: Vec<InstallConflict>,
}

impl InstallPlan {
    pub fn from_providers(providers: impl IntoIterator<Item = InstallFileProvider>) -> Self {
        let mut providers_by_target: BTreeMap<InstallTargetPath, Vec<InstallFileProvider>> =
            BTreeMap::new();

        for provider in providers {
            providers_by_target
                .entry(provider.target_path.clone())
                .or_default()
                .push(provider);
        }

        let mut actions = Vec::new();
        let mut conflicts = Vec::new();

        for (target_path, mut target_providers) in providers_by_target {
            target_providers.sort_by(|left, right| {
                left.layer
                    .priority
                    .cmp(&right.layer.priority)
                    .then_with(|| left.mod_id.cmp(&right.mod_id))
                    .then_with(|| left.package_file_id.cmp(&right.package_file_id))
            });

            if has_duplicate_priorities(&target_providers) {
                conflicts.push(InstallConflict {
                    target_path,
                    providers: target_providers,
                });
                continue;
            }

            actions.extend(target_providers.into_iter().map(|provider| InstallAction {
                target_path: provider.target_path.clone(),
                provider,
            }));
        }

        Self { actions, conflicts }
    }

    pub fn has_blocking_conflicts(&self) -> bool {
        !self.conflicts.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstallManifestEntry {
    pub target_path: InstallTargetPath,
    pub mod_id: ModId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revision_id: Option<ModRevisionId>,
    pub package_file_id: PackageFileId,
    pub layer: FileLayer,
    pub backup_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub installed_file: Option<InstalledFileSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstalledFileSummary {
    pub size_bytes: u64,
    pub sha256: String,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstallManifestStatus {
    Planned,
    Committing,
    #[default]
    Completed,
    RollbackRequired,
    RolledBack,
    RepairRequired,
}

/// profile 级 manifest status 的只读消费规则。
///
/// manifest 是 profile 聚合文档，status 描述整个 manifest 的状态机位置；
/// 只读消费方（状态摘要查询、恢复扫描）必须先经过该规则，
/// 保证失败状态不会被误报为已完成。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallManifestStatusConsumption {
    /// entries 是已固化事实，可继续按 entry 消费（`completed` / `rolled_back`）。
    /// `rolled_back` 只代表某次受控回滚已完成，剩余 entries 仍然可信。
    TrustEntries,
    /// 提交进行中（`planned` / `committing`），entries 不可当作已固化事实。
    InFlight,
    /// manifest 处于 `rollback_required` 失败态。
    RollbackRequired,
    /// manifest 处于 `repair_required` 失败态。
    RepairRequired,
}

impl InstallManifestStatus {
    pub fn consumption(self) -> InstallManifestStatusConsumption {
        match self {
            Self::Completed | Self::RolledBack => InstallManifestStatusConsumption::TrustEntries,
            Self::Planned | Self::Committing => InstallManifestStatusConsumption::InFlight,
            Self::RollbackRequired => InstallManifestStatusConsumption::RollbackRequired,
            Self::RepairRequired => InstallManifestStatusConsumption::RepairRequired,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstallRecoveryRecordStatus {
    Planned,
    Committing,
    Completed,
    RollbackRequired,
    RolledBack,
    RepairRequired,
}

impl InstallRecoveryRecordStatus {
    pub fn can_transition_to(self, next: Self) -> bool {
        use InstallRecoveryRecordStatus::{
            Committing, Completed, Planned, RepairRequired, RollbackRequired, RolledBack,
        };

        self == next
            || matches!(
                (self, next),
                (Planned, Committing)
                    | (Planned, RolledBack)
                    | (Committing, Completed)
                    | (Committing, RollbackRequired)
                    | (Committing, RepairRequired)
                    | (RollbackRequired, RolledBack)
                    | (RollbackRequired, RepairRequired)
                    | (RepairRequired, Planned)
                    | (Completed, RollbackRequired)
                    | (Completed, RepairRequired)
            )
    }
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum InstallRecoveryRecordTransitionError {
    #[error("invalid install recovery record transition from {from:?} to {to:?}")]
    InvalidTransition {
        from: InstallRecoveryRecordStatus,
        to: InstallRecoveryRecordStatus,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstallRecoveryRecordEntry {
    pub target_path: InstallTargetPath,
    pub package_file_id: PackageFileId,
    pub backup_ref: Option<String>,
    pub installed_file: Option<InstalledFileSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstallRecoveryRecord {
    pub profile_id: ProfileId,
    pub mod_id: ModId,
    pub status: InstallRecoveryRecordStatus,
    pub entries: Vec<InstallRecoveryRecordEntry>,
}

impl InstallRecoveryRecord {
    pub fn transition_to(
        &mut self,
        next: InstallRecoveryRecordStatus,
    ) -> Result<(), InstallRecoveryRecordTransitionError> {
        if self.status.can_transition_to(next) {
            self.status = next;
            Ok(())
        } else {
            Err(InstallRecoveryRecordTransitionError::InvalidTransition {
                from: self.status,
                to: next,
            })
        }
    }
}

pub const INSTALL_MANIFEST_SCHEMA_VERSION_V1: u32 = 1;
pub const INSTALL_MANIFEST_SCHEMA_VERSION_V2: u32 = 2;
pub const INSTALL_MANIFEST_SCHEMA_VERSION: u32 = INSTALL_MANIFEST_SCHEMA_VERSION_V1;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum InstallManifestValidationError {
    #[error("unsupported install manifest schema version: {schema_version}")]
    UnsupportedSchemaVersion { schema_version: u32 },
    #[error("schema v1 install manifest cannot contain revisioned entries")]
    RevisionedEntriesRequireSchemaV2,
    #[error("install manifest entries for Mod {mod_id} mix legacy and revisioned facts")]
    MixedRevisionSet { mod_id: String },
    #[error("install manifest entries for Mod {mod_id} contain multiple revisions")]
    MultipleRevisionSet { mod_id: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct InstallManifest {
    pub profile_id: ProfileId,
    pub manifest_id: String,
    pub schema_version: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schema_migration: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backend: Option<String>,
    #[serde(default)]
    pub status: InstallManifestStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plan_hash: Option<String>,
    pub entries: Vec<InstallManifestEntry>,
}

#[derive(Deserialize)]
struct InstallManifestWire {
    profile_id: ProfileId,
    #[serde(default)]
    manifest_id: Option<String>,
    #[serde(default)]
    schema_version: Option<u32>,
    #[serde(default)]
    schema_migration: Option<String>,
    #[serde(default)]
    backend: Option<String>,
    #[serde(default)]
    status: InstallManifestStatus,
    #[serde(default)]
    created_at: Option<String>,
    #[serde(default)]
    completed_at: Option<String>,
    #[serde(default)]
    plan_hash: Option<String>,
    entries: Vec<InstallManifestEntry>,
}

impl<'de> Deserialize<'de> for InstallManifest {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = InstallManifestWire::deserialize(deserializer)?;
        let manifest_id = wire
            .manifest_id
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| manifest_id_for_profile(&wire.profile_id));

        let manifest = Self {
            profile_id: wire.profile_id,
            manifest_id,
            schema_version: wire
                .schema_version
                .unwrap_or(INSTALL_MANIFEST_SCHEMA_VERSION),
            schema_migration: wire.schema_migration,
            backend: wire.backend,
            status: wire.status,
            created_at: wire.created_at,
            completed_at: wire.completed_at,
            plan_hash: wire.plan_hash,
            entries: wire.entries,
        };
        manifest.validate().map_err(D::Error::custom)?;
        Ok(manifest)
    }
}

impl InstallManifest {
    pub fn validate(&self) -> Result<(), InstallManifestValidationError> {
        if !matches!(
            self.schema_version,
            INSTALL_MANIFEST_SCHEMA_VERSION_V1 | INSTALL_MANIFEST_SCHEMA_VERSION_V2
        ) {
            return Err(InstallManifestValidationError::UnsupportedSchemaVersion {
                schema_version: self.schema_version,
            });
        }

        if self.schema_version == INSTALL_MANIFEST_SCHEMA_VERSION_V1
            && self.entries.iter().any(|entry| entry.revision_id.is_some())
        {
            return Err(InstallManifestValidationError::RevisionedEntriesRequireSchemaV2);
        }

        let mut revisions_by_mod = BTreeMap::<ModId, Option<ModRevisionId>>::new();
        for entry in &self.entries {
            match revisions_by_mod.get(&entry.mod_id) {
                None => {
                    revisions_by_mod.insert(entry.mod_id.clone(), entry.revision_id.clone());
                }
                Some(current) if current == &entry.revision_id => {}
                Some(current) if current.is_none() || entry.revision_id.is_none() => {
                    return Err(InstallManifestValidationError::MixedRevisionSet {
                        mod_id: entry.mod_id.as_str().to_owned(),
                    });
                }
                Some(_) => {
                    return Err(InstallManifestValidationError::MultipleRevisionSet {
                        mod_id: entry.mod_id.as_str().to_owned(),
                    });
                }
            }
        }

        Ok(())
    }

    pub fn completed(profile_id: ProfileId, entries: Vec<InstallManifestEntry>) -> Self {
        let manifest_id = manifest_id_for_profile(&profile_id);
        Self {
            profile_id,
            manifest_id,
            schema_version: INSTALL_MANIFEST_SCHEMA_VERSION,
            schema_migration: None,
            backend: None,
            status: InstallManifestStatus::Completed,
            created_at: None,
            completed_at: None,
            plan_hash: None,
            entries,
        }
    }

    pub fn completed_with_metadata(
        profile_id: ProfileId,
        entries: Vec<InstallManifestEntry>,
        backend: Option<String>,
        created_at: Option<String>,
        completed_at: Option<String>,
        plan_hash: Option<String>,
    ) -> Self {
        let manifest_id = manifest_id_for_profile(&profile_id);
        Self {
            profile_id,
            manifest_id,
            schema_version: INSTALL_MANIFEST_SCHEMA_VERSION,
            schema_migration: None,
            backend,
            status: InstallManifestStatus::Completed,
            created_at,
            completed_at,
            plan_hash,
            entries,
        }
    }
}

fn manifest_id_for_profile(profile_id: &ProfileId) -> String {
    format!("profile:{}", profile_id.as_str())
}

fn has_duplicate_priorities(providers: &[InstallFileProvider]) -> bool {
    let mut priorities = BTreeSet::new();
    providers
        .iter()
        .any(|provider| !priorities.insert(provider.layer.priority))
}

fn normalize_allowed_root(value: &str) -> Option<String> {
    normalize_relative_path(value).ok()
}

fn normalize_target_path(value: &str) -> Result<String, InstallTargetPathError> {
    normalize_relative_path(value)
}

fn normalize_relative_path(value: &str) -> Result<String, InstallTargetPathError> {
    let trimmed = value.trim();

    if trimmed.is_empty() {
        return Err(InstallTargetPathError::Empty);
    }

    if has_windows_drive_prefix(trimmed) {
        return Err(InstallTargetPathError::WindowsDrivePrefix);
    }

    if trimmed.starts_with('/') || trimmed.starts_with('\\') {
        return Err(InstallTargetPathError::Absolute);
    }

    let normalized_separator = trimmed.replace('\\', "/");
    let mut segments = Vec::new();

    for segment in normalized_separator.split('/') {
        if segment == ".." {
            return Err(InstallTargetPathError::ParentTraversal);
        }

        if segment.is_empty() || segment == "." {
            return Err(InstallTargetPathError::InvalidSegment);
        }

        segments.push(segment);
    }

    Ok(segments.join("/"))
}

fn has_windows_drive_prefix(value: &str) -> bool {
    let bytes = value.as_bytes();

    bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':'
}

#[cfg(test)]
mod tests {
    use super::*;

    fn allowed_roots() -> [&'static str; 2] {
        ["content", "plugins"]
    }

    #[test]
    fn target_path_accepts_relative_path_under_allowed_root() {
        let path = InstallTargetPath::parse("content/models/player.mod3", allowed_roots())
            .expect("path under an allowed root should be valid");

        assert_eq!(path.as_str(), "content/models/player.mod3");
    }

    #[test]
    fn target_path_rejects_empty_path() {
        let result = InstallTargetPath::parse(" ", allowed_roots());

        assert_eq!(result, Err(InstallTargetPathError::Empty));
    }

    #[test]
    fn target_path_rejects_absolute_path() {
        let result = InstallTargetPath::parse("/content/models/player.mod3", allowed_roots());

        assert_eq!(result, Err(InstallTargetPathError::Absolute));
    }

    #[test]
    fn target_path_rejects_parent_traversal() {
        let result = InstallTargetPath::parse("content/../outside.bin", allowed_roots());

        assert_eq!(result, Err(InstallTargetPathError::ParentTraversal));
    }

    #[test]
    fn target_path_rejects_windows_drive_prefix() {
        let result = InstallTargetPath::parse("C:/game/content/player.mod3", allowed_roots());

        assert_eq!(result, Err(InstallTargetPathError::WindowsDrivePrefix));
    }

    #[test]
    fn target_path_rejects_unallowed_root() {
        let result = InstallTargetPath::parse("other/player.mod3", allowed_roots());

        assert_eq!(
            result,
            Err(InstallTargetPathError::TargetRootNotAllowed {
                root: "other".to_owned()
            })
        );
    }

    #[test]
    fn plan_reports_blocking_conflict_for_same_target_without_priority_resolution() {
        let target = InstallTargetPath::parse("content/models/player.mod3", allowed_roots())
            .expect("target should be valid");
        let providers = vec![
            InstallFileProvider::new(
                ModId::new("mod-a"),
                PackageFileId::new("a-1"),
                target.clone(),
                FileLayer::new("base", 0),
            ),
            InstallFileProvider::new(
                ModId::new("mod-b"),
                PackageFileId::new("b-1"),
                target.clone(),
                FileLayer::new("base", 0),
            ),
        ];

        let plan = InstallPlan::from_providers(providers);

        assert!(plan.has_blocking_conflicts());
        assert!(plan.actions.is_empty());
        assert_eq!(plan.conflicts.len(), 1);
        assert_eq!(plan.conflicts[0].target_path, target);
        assert_eq!(plan.conflicts[0].providers.len(), 2);
    }

    #[test]
    fn plan_orders_same_target_actions_by_explicit_layer_priority() {
        let target = InstallTargetPath::parse("content/models/player.mod3", allowed_roots())
            .expect("target should be valid");
        let providers = vec![
            InstallFileProvider::new(
                ModId::new("overlay"),
                PackageFileId::new("overlay-1"),
                target.clone(),
                FileLayer::new("overlay", 20),
            ),
            InstallFileProvider::new(
                ModId::new("base"),
                PackageFileId::new("base-1"),
                target.clone(),
                FileLayer::new("base", 10),
            ),
        ];

        let plan = InstallPlan::from_providers(providers);

        assert!(!plan.has_blocking_conflicts());
        assert_eq!(plan.actions.len(), 2);
        assert_eq!(plan.actions[0].provider.mod_id.as_str(), "base");
        assert_eq!(plan.actions[0].provider.layer.name, "base");
        assert_eq!(plan.actions[1].provider.mod_id.as_str(), "overlay");
        assert_eq!(plan.actions[1].provider.layer.name, "overlay");
    }

    #[test]
    fn manifest_entry_accepts_legacy_missing_installed_file_summary() {
        let manifest: InstallManifest = serde_json::from_str(
            r#"{
                "profile_id": "default",
                "entries": [
                    {
                        "target_path": "content/models/player.mod3",
                        "mod_id": "mod-a",
                        "package_file_id": "content/models/player.mod3",
                        "layer": { "name": "base", "priority": 0 },
                        "backup_ref": null
                    }
                ]
            }"#,
        )
        .expect("legacy manifest should remain readable");

        assert_eq!(manifest.entries[0].installed_file, None);
        assert_eq!(manifest.entries[0].revision_id, None);
    }

    #[test]
    fn manifest_entry_revision_round_trips_with_stable_field_name() {
        let mut manifest = InstallManifest::completed(
            ProfileId::new("default"),
            vec![InstallManifestEntry {
                target_path: InstallTargetPath::parse(
                    "content/models/player.mod3",
                    allowed_roots(),
                )
                .expect("target should be valid"),
                mod_id: ModId::new("mod-a"),
                revision_id: Some(ModRevisionId::new("revision-v2")),
                package_file_id: PackageFileId::new("content/models/player.mod3"),
                layer: FileLayer::new("base", 0),
                backup_ref: None,
                installed_file: None,
            }],
        );
        manifest.schema_version = INSTALL_MANIFEST_SCHEMA_VERSION_V2;

        let serialized = serde_json::to_value(&manifest).expect("serialize revisioned manifest");
        let entry = &serialized["entries"][0];
        assert_eq!(entry["revision_id"], serde_json::json!("revision-v2"));
        assert!(entry.get("revisionId").is_none());

        let round_trip: InstallManifest =
            serde_json::from_value(serialized).expect("deserialize revisioned manifest");
        assert_eq!(
            round_trip.entries[0].revision_id,
            Some(ModRevisionId::new("revision-v2"))
        );
    }

    #[test]
    fn manifest_deserialization_rejects_mixed_or_multiple_revision_sets() {
        for entries in [
            serde_json::json!([
                manifest_entry_json("legacy.bin", None),
                manifest_entry_json("revisioned.bin", Some("v1")),
            ]),
            serde_json::json!([
                manifest_entry_json("v1.bin", Some("v1")),
                manifest_entry_json("v2.bin", Some("v2")),
            ]),
        ] {
            let error = serde_json::from_value::<InstallManifest>(serde_json::json!({
                "profile_id": "default",
                "schema_version": INSTALL_MANIFEST_SCHEMA_VERSION_V2,
                "entries": entries,
            }))
            .expect_err("ambiguous revision facts must fail closed");

            assert!(error.to_string().contains("revision"));
        }
    }

    #[test]
    fn legacy_manifest_defaults_to_completed_rich_status() {
        let manifest: InstallManifest = serde_json::from_str(
            r#"{
                "profile_id": "default",
                "entries": []
            }"#,
        )
        .expect("legacy manifest should remain readable");

        assert_eq!(manifest.status, InstallManifestStatus::Completed);
        assert_eq!(manifest.manifest_id, "profile:default");
        assert_eq!(manifest.schema_version, 1);
        assert_eq!(manifest.schema_migration, None);
        assert_eq!(manifest.backend, None);
        assert_eq!(manifest.created_at, None);
        assert_eq!(manifest.completed_at, None);
        assert_eq!(manifest.plan_hash, None);
    }

    #[test]
    fn manifest_status_serializes_as_stable_snake_case() {
        let manifest = InstallManifest {
            profile_id: ProfileId::new("default"),
            manifest_id: "profile:default".to_owned(),
            schema_version: 1,
            schema_migration: None,
            backend: Some("install_plan".to_owned()),
            status: InstallManifestStatus::RolledBack,
            created_at: Some("2026-06-29T00:00:00Z".to_owned()),
            completed_at: Some("2026-06-29T00:00:01Z".to_owned()),
            plan_hash: Some("sha256:test-plan".to_owned()),
            entries: Vec::new(),
        };

        let serialized = serde_json::to_string(&manifest).expect("serialize manifest");

        assert!(serialized.contains("\"status\":\"rolled_back\""));
        assert!(serialized.contains("\"backend\":\"install_plan\""));
        assert!(serialized.contains("\"plan_hash\":\"sha256:test-plan\""));
        assert!(!serialized.contains("RolledBack"));
    }

    #[test]
    fn manifest_metadata_serializes_with_stable_schema_fields() {
        let manifest = InstallManifest::completed(ProfileId::new("default"), Vec::new());

        let serialized = serde_json::to_string(&manifest).expect("serialize manifest");

        assert!(serialized.contains("\"manifest_id\":\"profile:default\""));
        assert!(serialized.contains("\"schema_version\":1"));
        assert!(!serialized.contains("schema_migration"));
    }

    #[test]
    fn manifest_status_consumption_classifies_read_side_rules() {
        use InstallManifestStatusConsumption::{
            InFlight, RepairRequired, RollbackRequired, TrustEntries,
        };

        assert_eq!(InstallManifestStatus::Completed.consumption(), TrustEntries);
        assert_eq!(
            InstallManifestStatus::RolledBack.consumption(),
            TrustEntries
        );
        assert_eq!(InstallManifestStatus::Planned.consumption(), InFlight);
        assert_eq!(InstallManifestStatus::Committing.consumption(), InFlight);
        assert_eq!(
            InstallManifestStatus::RollbackRequired.consumption(),
            RollbackRequired
        );
        assert_eq!(
            InstallManifestStatus::RepairRequired.consumption(),
            RepairRequired
        );
    }

    #[test]
    fn recovery_record_requires_committing_evidence_before_rollback_required() {
        let mut record = sample_recovery_record(InstallRecoveryRecordStatus::Planned);

        let error = record
            .transition_to(InstallRecoveryRecordStatus::RollbackRequired)
            .expect_err("planned records must not become rollback_required directly");

        assert_eq!(
            error,
            InstallRecoveryRecordTransitionError::InvalidTransition {
                from: InstallRecoveryRecordStatus::Planned,
                to: InstallRecoveryRecordStatus::RollbackRequired,
            }
        );
        assert_eq!(record.status, InstallRecoveryRecordStatus::Planned);
    }

    #[test]
    fn recovery_record_allows_controlled_rollback_lifecycle() {
        let mut record = sample_recovery_record(InstallRecoveryRecordStatus::Planned);

        record
            .transition_to(InstallRecoveryRecordStatus::Committing)
            .expect("planned records can enter committing");
        record
            .transition_to(InstallRecoveryRecordStatus::RollbackRequired)
            .expect("committing records can require rollback");
        record
            .transition_to(InstallRecoveryRecordStatus::RolledBack)
            .expect("rollback_required records can be marked rolled_back");

        assert_eq!(record.status, InstallRecoveryRecordStatus::RolledBack);
    }

    #[test]
    fn recovery_record_status_serializes_as_stable_snake_case() {
        let record = sample_recovery_record(InstallRecoveryRecordStatus::RollbackRequired);

        let serialized = serde_json::to_string(&record).expect("serialize recovery record");

        assert!(serialized.contains("\"status\":\"rollback_required\""));
        assert!(!serialized.contains("RollbackRequired"));
    }

    fn sample_recovery_record(status: InstallRecoveryRecordStatus) -> InstallRecoveryRecord {
        InstallRecoveryRecord {
            profile_id: ProfileId::new("default"),
            mod_id: ModId::new("mod-a"),
            status,
            entries: vec![InstallRecoveryRecordEntry {
                target_path: InstallTargetPath::parse("nativePC/models/player.mod3", ["nativePC"])
                    .expect("target"),
                package_file_id: PackageFileId::new("nativePC/models/player.mod3"),
                backup_ref: Some("backup-player".to_owned()),
                installed_file: Some(InstalledFileSummary {
                    size_bytes: 11,
                    sha256: "hash-player".to_owned(),
                }),
            }],
        }
    }

    fn manifest_entry_json(path: &str, revision_id: Option<&str>) -> serde_json::Value {
        let mut entry = serde_json::json!({
            "target_path": format!("content/{path}"),
            "mod_id": "mod-a",
            "package_file_id": path,
            "layer": { "name": "base", "priority": 0 },
            "backup_ref": null,
        });
        if let Some(revision_id) = revision_id {
            entry["revision_id"] = serde_json::json!(revision_id);
        }
        entry
    }
}
