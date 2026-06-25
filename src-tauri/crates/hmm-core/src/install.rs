use serde::{Deserialize, Serialize};
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
}
