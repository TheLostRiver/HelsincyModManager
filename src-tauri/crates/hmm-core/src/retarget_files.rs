use crate::{
    InstallTargetPath, PackageFileId, ReplacementSourceId, RetargetAction, RetargetError,
    RetargetPolicyExcludedFile,
};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RetargetFileDisposition {
    Relocated,
    KeptInPlace,
    PackageCompanion,
    InstalledAttachmentRetained,
    PluginCandidate,
    PolicyExcluded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RetargetFileReason {
    TargetMapping,
    OriginalTarget,
    TextureReference,
    UnmappedResource,
    PackageResource,
    InstalledAttachment,
    PluginNotIncluded,
    ExecutablePolicy,
}

/// 后端计划的文件处置事实。只有受控预览 DTO 可以投影相对路径，不作为调用方输入。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RetargetFileEffect {
    pub package_file_id: PackageFileId,
    pub source_id: Option<ReplacementSourceId>,
    pub source_path: InstallTargetPath,
    pub target_path: Option<InstallTargetPath>,
    pub disposition: RetargetFileDisposition,
    pub reason: RetargetFileReason,
}

pub(crate) fn validate_file_effects(
    actions: &[RetargetAction],
    exclusions: &[RetargetPolicyExcludedFile],
    effects: &[RetargetFileEffect],
) -> Result<(), RetargetError> {
    if actions.len().checked_add(exclusions.len()) != Some(effects.len()) {
        return Err(RetargetError::InvalidFileEffects);
    }
    let actions = actions
        .iter()
        .map(|action| (action.package_file_id(), action))
        .collect::<BTreeMap<_, _>>();
    let exclusions = exclusions
        .iter()
        .map(|file| (file.package_file_id(), file))
        .collect::<BTreeMap<_, _>>();
    let mut seen = BTreeSet::new();
    for effect in effects {
        if !seen.insert(&effect.package_file_id) {
            return Err(RetargetError::InvalidFileEffects);
        }
        let valid = if let Some(action) = actions.get(&effect.package_file_id) {
            effect.source_path == *action.source_relative_path()
                && effect.target_path.as_ref() == Some(action.target_relative_path())
                && match effect.disposition {
                    RetargetFileDisposition::Relocated => {
                        effect.source_id.as_ref() == Some(action.source_id())
                            && effect.source_path.windows_key()
                                != action.target_relative_path().windows_key()
                            && effect.reason == RetargetFileReason::TargetMapping
                    }
                    RetargetFileDisposition::KeptInPlace => {
                        effect.source_id.as_ref() == Some(action.source_id())
                            && effect.source_path.windows_key()
                                == action.target_relative_path().windows_key()
                            && matches!(
                                effect.reason,
                                RetargetFileReason::OriginalTarget
                                    | RetargetFileReason::TextureReference
                                    | RetargetFileReason::UnmappedResource
                            )
                    }
                    RetargetFileDisposition::PackageCompanion => {
                        effect.source_id.is_none()
                            && effect.source_path.windows_key()
                                == action.target_relative_path().windows_key()
                            && effect.reason == RetargetFileReason::PackageResource
                    }
                    _ => false,
                }
        } else if let Some(excluded) = exclusions.get(&effect.package_file_id) {
            effect.source_path == *excluded.original_path()
                && effect.target_path.is_none()
                && matches!(
                    (effect.disposition, effect.reason),
                    (
                        RetargetFileDisposition::PluginCandidate,
                        RetargetFileReason::PluginNotIncluded
                    ) | (
                        RetargetFileDisposition::PolicyExcluded,
                        RetargetFileReason::ExecutablePolicy
                    )
                )
        } else {
            false
        };
        if !valid {
            return Err(RetargetError::InvalidFileEffects);
        }
    }
    Ok(())
}
