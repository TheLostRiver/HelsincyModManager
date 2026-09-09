//! 外部删除安装目标后的受控卸载预览。路径与文件内容仅在卸载执行器内处理。

use super::*;
use hmm_core::InstallManifestStatusConsumption;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MissingTargetUninstallPreview {
    pub remove_file_count: usize,
    pub restore_file_count: usize,
    pub backup_count: usize,
    /// 已缺失的目标数；其中有备份的目标仍会执行恢复，不能与动作数直接相加。
    pub missing_file_count: usize,
    /// 乐观并发校验摘要，不是授权。绑定当前目标、备份与该 Mod 的清单事实。
    pub plan_token: String,
}

impl MissingTargetUninstallPreview {
    pub fn is_plan_token(value: &str) -> bool {
        value
            .strip_prefix("missing-uninstall-v1:")
            .is_some_and(|digest| {
                digest.len() == 64
                    && digest
                        .bytes()
                        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
            })
    }
}

impl UninstallModService {
    /// Runtime 提供配置游戏根的 opaque 摘要，让换目录后的确认失效；不由前端提供。
    #[must_use]
    pub fn with_missing_target_scope_binding(mut self, binding: String) -> Self {
        self.missing_target_scope_binding = Some(binding);
        self
    }

    pub fn preview_missing_target_uninstall(
        &self,
        request: &UninstallModRequest,
    ) -> Result<MissingTargetUninstallPreview, UninstallModError> {
        ensure_game_not_running(
            self.game_running_detector.as_ref(),
            &request.game_id,
            UninstallModError::GameRunning,
            UninstallModError::GameRunningUnknown,
        )?;
        let manifest = self
            .manifest_repository
            .load_manifest(&request.profile_id)
            .map_err(|_| UninstallModError::ManifestUnavailable)?
            .ok_or(UninstallModError::ModNotInstalled)?;
        validate_manifest(&manifest, request)?;
        let entries = manifest
            .entries
            .iter()
            .filter(|entry| entry.mod_id == request.mod_id)
            .cloned()
            .collect();
        let changes = self.prepare_uninstall_changes(entries, true)?;
        project_preview(
            request,
            &manifest,
            &changes,
            self.missing_target_scope_binding.as_deref(),
        )
    }

    /// 只有受控恢复入口可调用；常规卸载仍要求全部目标完整匹配。
    pub fn uninstall_missing_targets(
        &self,
        request: UninstallModRequest,
        expected_plan_token: &str,
    ) -> Result<UninstallModResult, UninstallModError> {
        self.uninstall_mod_internal(request, None, None, Some(expected_plan_token))
    }
}

pub(super) fn validate_manifest(
    manifest: &InstallManifest,
    request: &UninstallModRequest,
) -> Result<(), UninstallModError> {
    if manifest.profile_id != request.profile_id
        || manifest.status.consumption() != InstallManifestStatusConsumption::TrustEntries
        || manifest.validate().is_err()
    {
        return Err(UninstallModError::ManifestStateMismatch);
    }
    let entries: Vec<_> = manifest
        .entries
        .iter()
        .filter(|entry| entry.mod_id == request.mod_id)
        .collect();
    if entries.is_empty() {
        return Err(UninstallModError::ModNotInstalled);
    }
    // Windows 目标可能仅大小写不同；不能把同一文件当作两个独立归属。
    // 只对归属比较建立键，原始目标路径保留给文件系统执行器。
    let targets: BTreeSet<_> = entries
        .iter()
        .map(|entry| entry.target_path.as_str().to_lowercase())
        .collect();
    if targets.len() != entries.len()
        || manifest.entries.iter().any(|entry| {
            entry.mod_id != request.mod_id
                && targets.contains(&entry.target_path.as_str().to_lowercase())
        })
    {
        return Err(UninstallModError::ManifestStateMismatch);
    }
    Ok(())
}

pub(super) fn project_preview(
    request: &UninstallModRequest,
    manifest: &InstallManifest,
    changes: &[PreparedUninstallChange],
    scope_binding: Option<&str>,
) -> Result<MissingTargetUninstallPreview, UninstallModError> {
    let missing_file_count = changes
        .iter()
        .filter(|change| change.current_bytes.is_none())
        .count();
    if missing_file_count == 0 {
        return Err(UninstallModError::TargetStateMismatch);
    }
    let mut ordered: Vec<_> = changes.iter().collect();
    ordered.sort_by_key(|change| &change.entry.target_path);
    let mut hasher = Sha256::new();
    hasher.update(b"hmm-missing-target-uninstall-v1");
    update_optional_hash_str(&mut hasher, scope_binding);
    update_hash_str(&mut hasher, request.game_id.as_str());
    update_hash_str(
        &mut hasher,
        &uninstall_manifest_snapshot_digest(manifest, &request.mod_id),
    );
    for change in ordered {
        update_hash_str(&mut hasher, change.entry.target_path.as_str());
        for bytes in [&change.current_bytes, &change.backup_bytes] {
            match bytes {
                Some(bytes) => {
                    hasher.update([1]);
                    hasher.update(Sha256::digest(bytes));
                }
                None => hasher.update([0]),
            }
        }
    }
    let restore_file_count = changes
        .iter()
        .filter(|change| change.backup_bytes.is_some())
        .count();
    Ok(MissingTargetUninstallPreview {
        remove_file_count: changes
            .iter()
            .filter(|change| change.current_bytes.is_some() && change.backup_bytes.is_none())
            .count(),
        restore_file_count,
        backup_count: restore_file_count,
        missing_file_count,
        plan_token: format!("missing-uninstall-v1:{:x}", hasher.finalize()),
    })
}

#[cfg(test)]
#[path = "install_missing_targets_tests.rs"]
mod tests;
