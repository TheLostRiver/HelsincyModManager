//! Mod 库删除：从库中移除 logical Mod 并回收其全部存储（#276）。
//!
//! 门禁（全部后端判定，fail closed）：
//! - 任一 profile 的安装清单在可信状态下仍有该 Mod 的条目 → 该 Mod 的文件还在
//!   游戏目录，删除被拒（`blocked_installed`），玩家须先卸载；
//! - 安装清单处于不可信/失败态（planned/committing/rollback/repair）或存在
//!   reinstall recovery 事务 → 状态未决，删除被拒（`blocked_recovery`）。
//!
//! 清理顺序刻意安排：存储回收（沙盒/缩略图）在权威目录删除**之前**——失败时
//! 目录未变、删除干净失败、最多残留待回收文件；目录删除（权威一步）之后的
//! 元数据/分类清理失败只会留下无害的孤儿行，随重试清除。审计历史按治理
//! 约定 append-only 保留，删除只追加事件。

use crate::{ModStorageWriteGate, ModStorageWriteGateError};
use hmm_core::{InstallManifestStatusConsumption, ModId, Profile, ProfileId};
use hmm_ports::{
    AppClock, AuditLogEvent, AuditLogWriter, CategoryRepository, InstallManifestRepository,
    ModImportResultRepository, ModImportSandboxLocator, ModMetadataRepository, ProfileRepository,
    ReinstallRecoveryTransactionRepository, ReplacementSelectionRepository, ThumbnailStore,
};
use std::collections::BTreeMap;
use std::sync::Arc;
use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ModDeletionError {
    #[error("mod is not present in the library")]
    ModNotFound,
    #[error("mod is still installed in profiles: {profiles}")]
    BlockedInstalled { profiles: String },
    #[error("mod has pending install or recovery state and cannot be deleted")]
    BlockedRecovery,
    #[error("mod deletion storage is unavailable")]
    StoreUnavailable,
    /// #275：存储根迁移中或已切换待重启，沙盒回收会撕碎正在复制 / 已作废的根。
    #[error("{0}")]
    StorageWriteFrozen(ModStorageWriteGateError),
}

impl ModDeletionError {
    /// 前端/调用方的稳定错误码；不得随 message 文案变化。
    pub fn code(&self) -> &'static str {
        match self {
            Self::ModNotFound => "mod_delete_target_not_found",
            Self::BlockedInstalled { .. } => "mod_delete_blocked_installed",
            Self::BlockedRecovery => "mod_delete_blocked_recovery",
            Self::StoreUnavailable => "mod_delete_store_unavailable",
            Self::StorageWriteFrozen(error) => error.code(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModDeletionPreview {
    pub mod_id: ModId,
    pub display_name: String,
    pub revision_count: usize,
    pub category_labels: Vec<String>,
    /// 该 Mod 在哪些 profile 的安装清单里有条目（含未决/失败态）。
    pub affected_profiles: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModDeletionResult {
    pub mod_id: ModId,
    pub removed_revision_count: usize,
    pub removed_package_ids: Vec<String>,
}

pub struct ModDeletionService {
    profiles: Arc<dyn ProfileRepository>,
    install_manifests: Arc<dyn InstallManifestRepository>,
    reinstall_recovery: Arc<dyn ReinstallRecoveryTransactionRepository>,
    replacement_selections: Arc<dyn ReplacementSelectionRepository>,
    import_results: Arc<dyn ModImportResultRepository>,
    sandbox_locator: Arc<dyn ModImportSandboxLocator>,
    thumbnails: Arc<dyn ThumbnailStore>,
    metadata: Arc<dyn ModMetadataRepository>,
    categories: Arc<dyn CategoryRepository>,
    audit_log: Arc<dyn AuditLogWriter>,
    clock: Arc<dyn AppClock>,
    write_gate: Arc<ModStorageWriteGate>,
}

impl ModDeletionService {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        profiles: Arc<dyn ProfileRepository>,
        install_manifests: Arc<dyn InstallManifestRepository>,
        reinstall_recovery: Arc<dyn ReinstallRecoveryTransactionRepository>,
        replacement_selections: Arc<dyn ReplacementSelectionRepository>,
        import_results: Arc<dyn ModImportResultRepository>,
        sandbox_locator: Arc<dyn ModImportSandboxLocator>,
        thumbnails: Arc<dyn ThumbnailStore>,
        metadata: Arc<dyn ModMetadataRepository>,
        categories: Arc<dyn CategoryRepository>,
        audit_log: Arc<dyn AuditLogWriter>,
        clock: Arc<dyn AppClock>,
    ) -> Self {
        Self {
            profiles,
            install_manifests,
            reinstall_recovery,
            replacement_selections,
            import_results,
            sandbox_locator,
            thumbnails,
            metadata,
            categories,
            audit_log,
            clock,
            write_gate: Arc::new(ModStorageWriteGate::new()),
        }
    }

    /// 与迁移任务、其他沙盒写入者共用同一把存储写门闩。
    pub fn with_write_gate(mut self, write_gate: Arc<ModStorageWriteGate>) -> Self {
        self.write_gate = write_gate;
        self
    }

    /// 删除确认弹窗的数据源：删除什么、影响哪些 profile。
    pub fn preview_mod_deletion(
        &self,
        mod_id: &ModId,
    ) -> Result<ModDeletionPreview, ModDeletionError> {
        if self
            .import_results
            .get_mod(mod_id)
            .map_err(|_| ModDeletionError::StoreUnavailable)?
            .is_none()
        {
            return Err(ModDeletionError::ModNotFound);
        }
        let revisions = self
            .import_results
            .list_revisions(mod_id)
            .map_err(|_| ModDeletionError::StoreUnavailable)?;
        let display_name = match self.display_revision(mod_id) {
            Some(display) => revisions
                .iter()
                .find(|revision| revision.revision_id == display.revision_id)
                .map(|revision| revision.display_name.clone())
                .unwrap_or_else(|| mod_id.as_str().to_owned()),
            None => mod_id.as_str().to_owned(),
        };
        let category_labels = self.category_labels(mod_id);
        let gate = self.installation_gate(mod_id)?;
        let mut affected_profiles = gate.installed_profiles.clone();
        affected_profiles.extend(gate.recovery_profiles);

        Ok(ModDeletionPreview {
            mod_id: mod_id.clone(),
            display_name,
            revision_count: revisions.len(),
            category_labels,
            affected_profiles,
        })
    }

    /// 删除一个 logical Mod：门禁通过后按序回收全部存储。
    pub fn delete_mod(&self, mod_id: &ModId) -> Result<ModDeletionResult, ModDeletionError> {
        self.write_gate
            .ensure_open()
            .map_err(ModDeletionError::StorageWriteFrozen)?;
        let revisions = self
            .import_results
            .list_revisions(mod_id)
            .map_err(|_| ModDeletionError::StoreUnavailable)?;
        if revisions.is_empty() {
            return Err(ModDeletionError::ModNotFound);
        }

        let gate = self.installation_gate(mod_id)?;
        if !gate.installed_profiles.is_empty() {
            return Err(ModDeletionError::BlockedInstalled {
                profiles: gate.installed_profiles.join(", "),
            });
        }
        if !gate.recovery_profiles.is_empty() {
            return Err(ModDeletionError::BlockedRecovery);
        }

        // ① 选择意图：该 Mod 已不存在于面板流程，意图一并清除（尽力而为）。
        for profile in &self.profile_list()? {
            let _ = self
                .replacement_selections
                .remove_selection(&Self::profile_id(profile), mod_id);
        }

        // ② 存储回收（权威目录删除之前，失败则整体干净失败）。
        for revision in &revisions {
            self.sandbox_locator
                .cleanup_sandbox_for_package(&revision.package_id)
                .map_err(|_| ModDeletionError::StoreUnavailable)?;
            self.thumbnails
                .remove_package_thumbnails(&revision.package_id)
                .map_err(|_| ModDeletionError::StoreUnavailable)?;
        }

        // ③ 权威目录删除：此后该 Mod 从库中消失。
        let removed_package_ids = self
            .import_results
            .remove_mod_with_revisions(mod_id)
            .map_err(|_| ModDeletionError::StoreUnavailable)?;

        // ④ 元数据 overlay 与分类关联（权威删除之后的孤儿清理）。
        if self.metadata.delete(mod_id.as_str()).is_err() {
            return Err(ModDeletionError::StoreUnavailable);
        }
        if self
            .categories
            .set_mod_categories(mod_id.as_str(), &[])
            .is_err()
        {
            return Err(ModDeletionError::StoreUnavailable);
        }

        self.record_deleted_audit(mod_id, revisions.len(), &removed_package_ids);

        Ok(ModDeletionResult {
            mod_id: mod_id.clone(),
            removed_revision_count: revisions.len(),
            removed_package_ids,
        })
    }

    /// 跨 profile 安装门禁：返回 (已安装事实 profile, 未决/失败态 profile)。
    fn installation_gate(&self, mod_id: &ModId) -> Result<InstallationGate, ModDeletionError> {
        let mut gate = InstallationGate::default();
        for profile in &self.profile_list()? {
            let profile_id = Self::profile_id(profile);

            // reinstall recovery 事务未决：target switch 的中间状态，
            // 删除会撕碎恢复语义，一律 fail closed。独立于清单存在性判定。
            if self
                .reinstall_recovery
                .list_transactions(&profile_id)
                .map_err(|_| ModDeletionError::StoreUnavailable)?
                .iter()
                .any(|transaction| &transaction.mod_id == mod_id)
            {
                gate.recovery_profiles.push(profile.id.clone());
            }

            let Some(manifest) = self
                .install_manifests
                .load_manifest(&profile_id)
                .map_err(|_| ModDeletionError::StoreUnavailable)?
            else {
                continue;
            };
            let has_entries = manifest.entries.iter().any(|entry| &entry.mod_id == mod_id);
            if has_entries {
                match manifest.status.consumption() {
                    InstallManifestStatusConsumption::TrustEntries => {
                        gate.installed_profiles.push(profile.id.clone());
                    }
                    _ => gate.recovery_profiles.push(profile.id.clone()),
                }
            }
        }
        gate.recovery_profiles.sort();
        gate.recovery_profiles.dedup();
        Ok(gate)
    }

    fn profile_list(&self) -> Result<Vec<Profile>, ModDeletionError> {
        self.profiles
            .list_all()
            .map_err(|_| ModDeletionError::StoreUnavailable)
    }

    fn profile_id(profile: &Profile) -> ProfileId {
        ProfileId::new(&profile.id)
    }

    fn display_revision(&self, mod_id: &ModId) -> Option<hmm_ports::StoredModRevision> {
        let logical_mod = self.import_results.get_mod(mod_id).ok().flatten()?;
        self.import_results
            .get_revision(&logical_mod.display_revision_id)
            .ok()
            .flatten()
    }

    fn category_labels(&self, mod_id: &ModId) -> Vec<String> {
        self.categories
            .list_mod_category_pairs()
            .unwrap_or_default()
            .into_iter()
            .filter(|(category_mod_id, _)| category_mod_id == mod_id.as_str())
            .map(|(_, category)| category.name)
            .collect()
    }

    fn record_deleted_audit(
        &self,
        mod_id: &ModId,
        revision_count: usize,
        removed_package_ids: &[String],
    ) {
        // 审计写入是 best-effort：删除的权威状态已经落库，审计写失败不应把
        // 已完成的删除报成失败。策略与安装审计的降级语义一致。
        let timestamp = self.clock.now_unix_millis().unwrap_or_default();
        let event = AuditLogEvent {
            timestamp_unix_millis: timestamp,
            category: "library".to_owned(),
            operation: "delete_mod".to_owned(),
            result: "success".to_owned(),
            fields: BTreeMap::from([
                ("mod_id".to_owned(), mod_id.as_str().to_owned()),
                ("revision_count".to_owned(), revision_count.to_string()),
                (
                    "removed_package_count".to_owned(),
                    removed_package_ids.len().to_string(),
                ),
            ]),
        };
        let _ = self
            .audit_log
            .record_with_policy(event, hmm_ports::AuditWriteFailurePolicy::BestEffort);
    }
}

#[derive(Debug, Default)]
struct InstallationGate {
    installed_profiles: Vec<String>,
    recovery_profiles: Vec<String>,
}

#[cfg(test)]
mod tests;
