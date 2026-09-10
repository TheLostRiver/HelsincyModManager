//! 有可信 manifest、但正常安装后目标缺失时的恢复入口。

use super::*;
use crate::UninstallModRequest;
use hmm_core::GameId;

impl InstallRecoveryActionPreviewService {
    pub fn with_missing_target_uninstall(
        mut self,
        game_id: GameId,
        uninstaller: UninstallModService,
        reinstall_repository: Arc<dyn ReinstallRecoveryTransactionRepository>,
    ) -> Self {
        self.missing_target_uninstaller = Some((game_id, uninstaller));
        self.reinstall_recovery_repository = Some(reinstall_repository);
        self
    }

    pub(super) fn preview_missing_target_uninstall(
        &self,
        request: InstallRecoveryActionPreviewRequest,
    ) -> Result<InstallRecoveryActionPreview, InstallRecoveryActionPreviewError> {
        let Some((game_id, uninstaller)) = &self.missing_target_uninstaller else {
            return Err(InstallRecoveryActionPreviewError::PreviewUnavailable);
        };
        if let Err(reason) = ensure_no_pending_recovery(
            &request.profile_id,
            self.recovery_record_repository.as_ref(),
            self.reinstall_recovery_repository.as_deref(),
        ) {
            return Ok(blocked_recovery_action_preview(
                request,
                0,
                0,
                0,
                [(reason, 1)],
            ));
        }
        let preview = uninstaller.preview_missing_target_uninstall(&UninstallModRequest {
            game_id: game_id.clone(),
            profile_id: request.profile_id.clone(),
            mod_id: request.mod_id.clone(),
        });
        match preview {
            Ok(preview) => Ok(InstallRecoveryActionPreview {
                profile_id: request.profile_id,
                mod_id: request.mod_id,
                action_kind: request.action_kind,
                availability: InstallRecoveryActionAvailability::Available,
                remove_file_count: preview.remove_file_count,
                restore_file_count: preview.restore_file_count,
                backup_count: preview.backup_count,
                blocking_issue_count: 0,
                blocking_reasons: vec![],
                missing_file_count: preview.missing_file_count,
                plan_token: Some(preview.plan_token),
            }),
            Err(error) => Ok(blocked_recovery_action_preview(
                request,
                0,
                0,
                0,
                [(uninstall_block_reason(&error), 1)],
            )),
        }
    }
}

impl InstallRecoveryActionService {
    pub fn with_missing_target_uninstall(
        mut self,
        game_id: GameId,
        uninstaller: UninstallModService,
        reinstall_repository: Arc<dyn ReinstallRecoveryTransactionRepository>,
    ) -> Self {
        self.missing_target_uninstaller = Some((game_id, uninstaller));
        self.reinstall_recovery_repository = Some(reinstall_repository);
        self
    }

    pub(super) fn uninstall_missing_targets(
        &self,
        request: InstallRecoveryActionRequest,
    ) -> Result<InstallRecoveryActionResult, InstallRecoveryActionError> {
        let (game_id, uninstaller) = self
            .missing_target_uninstaller
            .as_ref()
            .ok_or(InstallRecoveryActionError::ActionUnavailable)?;
        ensure_no_pending_recovery(
            &request.profile_id,
            self.recovery_record_repository.as_ref(),
            self.reinstall_recovery_repository.as_deref(),
        )
        .map_err(|reason| blocked_recovery_action_error([(reason, 1)]))?;
        let token = request
            .plan_token
            .as_deref()
            .filter(|token| crate::MissingTargetUninstallPreview::is_plan_token(token))
            .ok_or_else(|| {
                blocked_recovery_action_error([(
                    InstallRecoveryActionBlockReason::PreviewRequired,
                    1,
                )])
            })?;
        let result = uninstaller
            .uninstall_missing_targets(
                UninstallModRequest {
                    game_id: game_id.clone(),
                    profile_id: request.profile_id.clone(),
                    mod_id: request.mod_id.clone(),
                },
                token,
            )
            .map_err(InstallRecoveryActionError::MissingTargetUninstall)?;
        Ok(InstallRecoveryActionResult {
            profile_id: request.profile_id,
            mod_id: request.mod_id,
            action_kind: request.action_kind,
            remove_file_count: result.removed_file_count,
            restore_file_count: result.restored_file_count,
            backup_count: result.restored_file_count,
        })
    }
}

fn ensure_no_pending_recovery(
    profile_id: &ProfileId,
    records: &dyn InstallRecoveryRecordRepository,
    reinstalls: Option<&dyn ReinstallRecoveryTransactionRepository>,
) -> Result<(), InstallRecoveryActionBlockReason> {
    let records = records
        .list_records(profile_id)
        .map_err(|_| InstallRecoveryActionBlockReason::InstallStateUnavailable)?;
    if records
        .iter()
        .any(|record| record.profile_id != *profile_id)
    {
        return Err(InstallRecoveryActionBlockReason::InstallStateUnavailable);
    }
    if records.iter().any(|record| {
        !matches!(
            record.status,
            InstallRecoveryRecordStatus::Completed | InstallRecoveryRecordStatus::RolledBack
        )
    }) {
        return Err(InstallRecoveryActionBlockReason::RecoveryPending);
    }
    let transactions = reinstalls
        .ok_or(InstallRecoveryActionBlockReason::InstallStateUnavailable)?
        .list_transactions(profile_id)
        .map_err(|_| InstallRecoveryActionBlockReason::InstallStateUnavailable)?;
    if !transactions.is_empty() {
        return Err(InstallRecoveryActionBlockReason::RecoveryPending);
    }
    Ok(())
}

fn uninstall_block_reason(error: &UninstallModError) -> InstallRecoveryActionBlockReason {
    match error {
        UninstallModError::MissingInstalledFileSummary => {
            InstallRecoveryActionBlockReason::MissingInstalledFileSummary
        }
        UninstallModError::TargetStateMismatch => {
            InstallRecoveryActionBlockReason::TargetStateUnavailable
        }
        UninstallModError::BackupUnavailable => InstallRecoveryActionBlockReason::BackupUnavailable,
        UninstallModError::GameRunning => InstallRecoveryActionBlockReason::GameRunning,
        UninstallModError::GameRunningUnknown => {
            InstallRecoveryActionBlockReason::GameRunningUnknown
        }
        _ => InstallRecoveryActionBlockReason::InstallStateUnavailable,
    }
}
