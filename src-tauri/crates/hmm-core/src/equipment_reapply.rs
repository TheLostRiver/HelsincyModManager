use crate::{InstallManifest, ModId, ModRevisionId, ReplacementBindingSnapshot};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReinstallIntent {
    #[default]
    Standard,
    ReapplyEquipmentTargets,
}

impl ReinstallIntent {
    pub fn is_standard(&self) -> bool {
        *self == Self::Standard
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Standard => "standard",
            Self::ReapplyEquipmentTargets => "reapply_equipment_targets",
        }
    }
}

/// 重新应用只更新既有来源在当前物理目标上的产出，不能新增来源或借机切换目标。
/// 是否存在实际文件差异由完整候选分类确认；本校验不构成独立写入授权。
/// 目标 ID 的旧别名可规范化，但应用层必须先用 catalog 核对它与物理目标身份一致。
pub fn is_same_revision_equipment_reapply(
    manifest: &InstallManifest,
    mod_id: &ModId,
    revision_id: &ModRevisionId,
    candidates: &[ReplacementBindingSnapshot],
) -> bool {
    let mut installed = BTreeMap::new();
    for previous in manifest
        .replacement_bindings
        .iter()
        .filter(|binding| binding.mod_id() == mod_id)
    {
        if previous.profile_id() != &manifest.profile_id
            || previous.revision_id().is_some_and(|id| id != revision_id)
            || previous.source_path_family() != previous.target_path_family()
            || installed
                .insert(previous.binding().source_id(), previous)
                .is_some()
        {
            return false;
        }
    }
    if installed.is_empty() || installed.len() != candidates.len() {
        return false;
    }
    let mut sources = BTreeSet::new();
    let mut binding_ids = BTreeSet::new();
    candidates.iter().all(|candidate| {
        let Some(previous) = installed.get(candidate.binding().source_id()) else {
            return false;
        };
        candidate.mod_id() == mod_id
            && candidate.profile_id() == &manifest.profile_id
            && candidate.revision_id() == Some(revision_id)
            && sources.insert(candidate.binding().source_id())
            && binding_ids.insert(candidate.binding_id())
            && candidate.binding_id() == previous.binding_id()
            && candidate.binding().created_at_unix_millis()
                == previous.binding().created_at_unix_millis()
            && candidate.source_internal_id() == previous.source_internal_id()
            && candidate.source_path_family() == previous.source_path_family()
            && candidate.target_internal_id() == previous.target_internal_id()
            && candidate.target_path_family() == previous.target_path_family()
            && candidate.retarget_kind() == previous.retarget_kind()
    })
}
