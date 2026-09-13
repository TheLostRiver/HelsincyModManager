//! 游戏无关的插件选择事实。文件类型与目录规则由游戏适配器解释。
use crate::{
    GameId, InstallTargetPath, InstalledFileSummary, ModId, ModRevisionId, PackageFileId, ProfileId,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use thiserror::Error;

pub const PLUGIN_SELECTION_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginSelectionScope {
    pub game_id: GameId,
    pub profile_id: ProfileId,
    pub mod_id: ModId,
    pub revision_id: ModRevisionId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginFileChoiceKind {
    Include,
    Exclude,
    /// 既有可信文件可保持；这不是新装不受当前策略支持的文件的授权。
    RetainInstalled,
}

impl PluginFileChoiceKind {
    pub fn is_included(self) -> bool {
        self != Self::Exclude
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginFileChoice {
    pub package_file_id: PackageFileId,
    pub target_path: InstallTargetPath,
    pub source_file: InstalledFileSummary,
    pub choice: PluginFileChoiceKind,
    #[serde(default, skip_serializing_if = "is_false")]
    pub excluded_by_package_selection: bool,
}

fn is_false(value: &bool) -> bool {
    !value
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "PluginSelectionWire")]
pub struct PluginSelectionSnapshot {
    schema_version: u32,
    scope: PluginSelectionScope,
    policy_id: String,
    policy_version: u32,
    files: Vec<PluginFileChoice>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PluginSelectionWire {
    schema_version: u32,
    scope: PluginSelectionScope,
    policy_id: String,
    policy_version: u32,
    files: Vec<PluginFileChoice>,
}

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum PluginSelectionError {
    #[error("plugin selection schema is unsupported")]
    UnsupportedSchema,
    #[error("plugin selection scope is invalid")]
    InvalidScope,
    #[error("plugin policy identity is invalid")]
    InvalidPolicy,
    #[error("plugin selection file facts are invalid")]
    InvalidFile,
    #[error("plugin selection repeats a file or equivalent target")]
    DuplicateFile,
    #[error("plugin selection does not match the operation scope")]
    ScopeMismatch,
    #[error("plugin selection does not match the planned files")]
    PlanMismatch,
}

impl PluginSelectionSnapshot {
    pub fn new(
        scope: PluginSelectionScope,
        policy_id: impl Into<String>,
        policy_version: u32,
        mut files: Vec<PluginFileChoice>,
    ) -> Result<Self, PluginSelectionError> {
        for id in [
            scope.game_id.as_str(),
            scope.profile_id.as_str(),
            scope.mod_id.as_str(),
            scope.revision_id.as_str(),
        ] {
            if id.is_empty() || id.trim() != id || id.chars().any(char::is_control) {
                return Err(PluginSelectionError::InvalidScope);
            }
        }
        let policy_id = policy_id.into();
        if policy_version == 0
            || policy_id.is_empty()
            || policy_id.len() > 128
            || !policy_id
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"._-".contains(&byte))
        {
            return Err(PluginSelectionError::InvalidPolicy);
        }
        if files.is_empty() {
            return Err(PluginSelectionError::InvalidFile);
        }
        let mut ids = BTreeSet::new();
        let mut paths = BTreeSet::new();
        for file in &files {
            let raw = file.target_path.as_str();
            let root = raw.split('/').next().unwrap_or_default();
            if file.package_file_id.as_str().is_empty()
                || file.package_file_id.as_str().chars().any(char::is_control)
                || InstallTargetPath::parse(raw, [root]).as_ref() != Ok(&file.target_path)
                || file.source_file.sha256.len() != 64
                || !file
                    .source_file
                    .sha256
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
                || (file.excluded_by_package_selection && file.choice.is_included())
            {
                return Err(PluginSelectionError::InvalidFile);
            }
            if !ids.insert(file.package_file_id.clone())
                || !paths.insert(file.target_path.windows_key())
            {
                return Err(PluginSelectionError::DuplicateFile);
            }
        }
        files.sort_by(|left, right| {
            left.package_file_id
                .as_str()
                .cmp(right.package_file_id.as_str())
        });
        Ok(Self {
            schema_version: PLUGIN_SELECTION_SCHEMA_VERSION,
            scope,
            policy_id,
            policy_version,
            files,
        })
    }

    pub fn scope(&self) -> &PluginSelectionScope {
        &self.scope
    }
    pub fn policy_id(&self) -> &str {
        &self.policy_id
    }
    pub fn policy_version(&self) -> u32 {
        self.policy_version
    }
    pub fn files(&self) -> &[PluginFileChoice] {
        &self.files
    }

    /// 不含选择值的盘点身份；改勾选不改变它，换 scope、版本、文件或规则必须改变它。
    pub fn inventory_id(&self) -> String {
        let mut hash = Sha256::new();
        for value in [
            "hmm-plugin-inventory-v1",
            self.scope.game_id.as_str(),
            self.scope.profile_id.as_str(),
            self.scope.mod_id.as_str(),
            self.scope.revision_id.as_str(),
            self.policy_id.as_str(),
        ] {
            hash_field(&mut hash, value);
        }
        hash.update(self.policy_version.to_le_bytes());
        for file in &self.files {
            hash_field(&mut hash, file.package_file_id.as_str());
            hash_field(&mut hash, file.target_path.as_str());
            hash_field(&mut hash, &file.source_file.sha256);
            hash.update(file.source_file.size_bytes.to_le_bytes());
            hash.update([u8::from(file.excluded_by_package_selection)]);
        }
        format!("plugin-inventory-v1:{:x}", hash.finalize())
    }

    pub fn validate_scope(&self, scope: &PluginSelectionScope) -> Result<(), PluginSelectionError> {
        if self.scope == *scope {
            Ok(())
        } else {
            Err(PluginSelectionError::ScopeMismatch)
        }
    }
}

impl TryFrom<PluginSelectionWire> for PluginSelectionSnapshot {
    type Error = PluginSelectionError;
    fn try_from(wire: PluginSelectionWire) -> Result<Self, Self::Error> {
        if wire.schema_version != PLUGIN_SELECTION_SCHEMA_VERSION {
            return Err(PluginSelectionError::UnsupportedSchema);
        }
        Self::new(wire.scope, wire.policy_id, wire.policy_version, wire.files)
    }
}

fn hash_field(hash: &mut Sha256, value: &str) {
    hash.update((value.len() as u64).to_le_bytes());
    hash.update(value.as_bytes());
}

/// 没有装备绑定时，当前配置重新应用只允许处理明确声明的插件文件。
pub fn is_same_revision_plugin_reapply(
    manifest: &crate::InstallManifest,
    mod_id: &ModId,
    revision_id: &ModRevisionId,
    candidate_bindings: &[crate::ReplacementBindingSnapshot],
    selections: &[PluginSelectionSnapshot],
) -> bool {
    let [selection] = selections else {
        return false;
    };
    let entries = manifest
        .entries
        .iter()
        .filter(|entry| entry.mod_id == *mod_id)
        .collect::<Vec<_>>();
    candidate_bindings.is_empty()
        && !manifest
            .replacement_bindings
            .iter()
            .any(|binding| binding.mod_id() == mod_id)
        && !entries.is_empty()
        && entries
            .iter()
            .all(|entry| entry.revision_id.as_ref() == Some(revision_id))
        && selection.scope().mod_id == *mod_id
        && selection.scope().profile_id == manifest.profile_id
        && selection.scope().revision_id == *revision_id
}

pub fn plugin_reapply_targets_are_allowed<'a>(
    selections: &[PluginSelectionSnapshot],
    targets: impl IntoIterator<Item = (&'a InstallTargetPath, crate::ReinstallTargetClass)>,
) -> bool {
    let paths = selections
        .iter()
        .flat_map(|selection| {
            selection
                .files()
                .iter()
                .map(|file| file.target_path.windows_key())
        })
        .collect::<BTreeSet<_>>();
    targets.into_iter().all(|(path, class)| {
        class == crate::ReinstallTargetClass::Retained || paths.contains(&path.windows_key())
    })
}

/// Empty candidates are permitted only when removing every owned file of a plugin-only Mod.
/// Ordinary empty reinstalls and undeclared non-plugin deletions remain rejected.
pub fn is_complete_plugin_removal(
    manifest: &crate::InstallManifest,
    mod_id: &ModId,
    revision: &ModRevisionId,
    bindings: &[crate::ReplacementBindingSnapshot],
    selections: &[PluginSelectionSnapshot],
) -> bool {
    is_same_revision_plugin_reapply(manifest, mod_id, revision, bindings, selections)
        && selections
            .iter()
            .flat_map(|selection| selection.files())
            .all(|file| file.choice == PluginFileChoiceKind::Exclude)
        && manifest
            .entries
            .iter()
            .filter(|entry| entry.mod_id == *mod_id)
            .all(|entry| {
                !entry.adopted
                    && selections[0].files().iter().any(|file| {
                        file.package_file_id == entry.package_file_id
                            && file.target_path.windows_key() == entry.target_path.windows_key()
                            && entry.installed_file.as_ref() == Some(&file.source_file)
                    })
            })
}

impl crate::InstallPlan {
    pub fn validate_plugin_selections(
        &self,
        game_id: &GameId,
        profile_id: &ProfileId,
    ) -> Result<(), PluginSelectionError> {
        let mut mods = BTreeSet::new();
        for selection in &self.plugin_selections {
            let scope = selection.scope();
            if scope.game_id != *game_id
                || scope.profile_id != *profile_id
                || !mods.insert(&scope.mod_id)
            {
                return Err(PluginSelectionError::ScopeMismatch);
            }
            let providers = self
                .actions
                .iter()
                .map(|action| &action.provider)
                .chain(
                    self.conflicts
                        .iter()
                        .flat_map(|conflict| &conflict.providers),
                )
                .filter(|provider| provider.mod_id == scope.mod_id)
                .collect::<Vec<_>>();
            for file in selection.files() {
                let matching = providers
                    .iter()
                    .filter(|provider| provider.package_file_id == file.package_file_id)
                    .collect::<Vec<_>>();
                if file.choice.is_included() {
                    if matching.is_empty()
                        || matching.iter().any(|provider| {
                            !provider
                                .target_path
                                .as_str()
                                .eq_ignore_ascii_case(file.target_path.as_str())
                        })
                    {
                        return Err(PluginSelectionError::PlanMismatch);
                    }
                    if self.actions.iter().any(|action| {
                        action.provider.mod_id == scope.mod_id
                            && action.provider.package_file_id == file.package_file_id
                            && !action
                                .target_path
                                .as_str()
                                .eq_ignore_ascii_case(file.target_path.as_str())
                    }) {
                        return Err(PluginSelectionError::PlanMismatch);
                    }
                } else if !matching.is_empty() {
                    return Err(PluginSelectionError::PlanMismatch);
                }
            }
        }
        Ok(())
    }
}

pub(crate) fn validate_manifest_plugins(
    manifest: &crate::InstallManifest,
) -> Result<(), PluginSelectionError> {
    let mut mods = BTreeSet::new();
    for selection in &manifest.plugin_selections {
        let scope = selection.scope();
        if scope.profile_id != manifest.profile_id || !mods.insert(&scope.mod_id) {
            return Err(PluginSelectionError::ScopeMismatch);
        }
        let owned = manifest
            .entries
            .iter()
            .filter(|entry| entry.mod_id == scope.mod_id)
            .collect::<Vec<_>>();
        if owned.is_empty()
            || owned
                .iter()
                .any(|entry| entry.revision_id.as_ref() != Some(&scope.revision_id))
        {
            return Err(PluginSelectionError::ScopeMismatch);
        }
        for file in selection.files() {
            let matching = owned
                .iter()
                .filter(|entry| entry.package_file_id == file.package_file_id)
                .collect::<Vec<_>>();
            if file.choice.is_included() {
                let [entry] = matching.as_slice() else {
                    return Err(PluginSelectionError::PlanMismatch);
                };
                if entry.adopted
                    || !entry
                        .target_path
                        .as_str()
                        .eq_ignore_ascii_case(file.target_path.as_str())
                    || entry.installed_file.as_ref() != Some(&file.source_file)
                {
                    return Err(PluginSelectionError::PlanMismatch);
                }
            } else if !matching.is_empty() {
                return Err(PluginSelectionError::PlanMismatch);
            }
        }
    }
    Ok(())
}

pub(crate) fn validate_recovery_plugins(
    transaction: &crate::ReinstallRecoveryTransaction,
) -> Result<(), PluginSelectionError> {
    if transaction.candidate_plugin_selections.len() > 1 {
        return Err(PluginSelectionError::ScopeMismatch);
    }
    for selection in &transaction.candidate_plugin_selections {
        let scope = selection.scope();
        if scope.profile_id != transaction.profile_id
            || scope.mod_id != transaction.mod_id
            || scope.revision_id != transaction.candidate_revision_id
        {
            return Err(PluginSelectionError::ScopeMismatch);
        }
        if !matches!(
            transaction.status,
            crate::ReinstallRecoveryTransactionStatus::Planned
                | crate::ReinstallRecoveryTransactionStatus::Committing
        ) {
            continue;
        }
        for file in selection.files() {
            let target = transaction.targets.iter().find(|target| {
                target
                    .target_path
                    .as_str()
                    .eq_ignore_ascii_case(file.target_path.as_str())
            });
            if file.choice.is_included() {
                let Some(target) = target else {
                    return Err(PluginSelectionError::PlanMismatch);
                };
                if target.candidate_state.as_ref() != Some(&file.source_file)
                    || (file.choice == PluginFileChoiceKind::RetainInstalled
                        && target.class != crate::ReinstallTargetClass::Retained)
                {
                    return Err(PluginSelectionError::PlanMismatch);
                }
            } else if target.is_some_and(|target| target.candidate_state.is_some()) {
                return Err(PluginSelectionError::PlanMismatch);
            }
        }
    }
    Ok(())
}
