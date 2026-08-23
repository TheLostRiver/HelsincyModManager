import type { LocaleDictionary } from "../../shared/i18n";
import type { ProfileSaveRestoreRunningPhase } from "./profileSaveRestoreTaskState";

// 存档恢复（预览、确认、任务进度、终态 toast 与警告码）的全部用户可见文案。
// 语义推进留在 profileSaveRestoreTaskState：state 只存 phase/errorCode/warningCodes，
// 文本在渲染或 toast 组装时经本字典取。

export type SaveRestoreCodeCopy = {
  byCode: Record<string, string>;
  fallback: string;
};

export type SaveRestoreCopy = {
  phases: Record<ProfileSaveRestoreRunningPhase, string>;
  errors: SaveRestoreCodeCopy;
  warnings: SaveRestoreCodeCopy;
  cancelErrors: SaveRestoreCodeCopy;
  dialog: {
    title: string;
    description: string;
    previewing: string;
    preparingChannel: string;
    listenerFailed: string;
    factBackupPoint: string;
    factFiles: string;
    factFileCount: (count: number) => string;
    factUncompressedSize: string;
    protectionOnTitle: string;
    protectionOnHint: string;
    protectionOffTitle: string;
    protectionOffHint: string;
    highRiskConfirmLabel: string;
    startingTask: string;
    cancellingTask: string;
    completedInline: string;
    recoveryRequiredTitle: string;
    recoveryRequiredSuffix: string;
    cancelledInline: string;
    footerDone: string;
    footerCancelling: string;
    footerStarting: string;
    footerCommitting: string;
    footerCancelRestore: string;
    footerCancel: string;
    footerConfirm: string;
  };
  toasts: {
    completedTitle: string;
    completedEvidenceTitle: string;
    completedMessage: string;
    recoveryRequiredTitle: string;
    failedTitle: string;
    cancelledTitle: string;
    cancelledMessage: string;
    cancelRejectedTitle: string;
  };
};

export const saveRestoreCopy = {
  zh_cn: {
    phases: {
      "save_restore.queued": "等待恢复任务",
      "save_restore.preparing": "正在校验并准备存档",
      "save_restore.revalidating": "正在复核目标状态",
      "save_restore.pre_restore_backup": "正在创建恢复前安全备份",
      "save_restore.committing": "正在替换并校验存档",
    },
    errors: {
      byCode: {
        save_restore_profile_missing: "配置档已不存在，请刷新后重试。",
        save_restore_backup_missing: "所选备份记录已不存在，请刷新备份历史。",
        save_restore_backup_unavailable: "所选备份当前不可用于恢复。",
        save_restore_target_unset: "当前配置档尚未设置存档目录。",
        save_restore_target_invalid: "当前配置档的存档目录无效，请先重新设置。",
        save_restore_game_running: "游戏仍在运行，请完全退出游戏后重试。",
        save_restore_game_running_unknown: "无法确认游戏是否已退出，恢复已安全阻断。",
        save_restore_source_invalid: "备份归档或清单未通过安全校验。",
        save_restore_backup_directory_unavailable: "备份目录当前不可读取。",
        save_restore_archive_unavailable: "备份归档文件当前不可读取。",
        save_restore_manifest_unavailable: "备份清单当前不可读取。",
        save_restore_manifest_invalid: "备份清单无效，不能用于恢复。",
        save_restore_archive_invalid: "备份归档无效，不能用于恢复。",
        save_restore_hash_mismatch: "备份内容校验不一致，恢复已停止。",
        save_restore_path_unsafe: "备份包含不安全路径，恢复已停止。",
        save_restore_size_limit_exceeded: "备份内容超过恢复安全限制。",
        save_restore_staging_unavailable: "无法创建受控恢复暂存区。",
        save_restore_recovery_required: "恢复未能安全收敛，已保留恢复证据。",
        save_restore_transaction_unavailable: "无法持久化恢复事务，恢复已安全停止。",
        save_restore_clock_unavailable: "无法建立可靠的恢复时间事实。",
        save_restore_token_issue_failed: "无法创建恢复预览凭证，请重新打开面板。",
        save_restore_token_invalid: "恢复预览凭证无效，请重新打开面板。",
        save_restore_token_expired: "恢复预览已过期，请重新打开面板。",
        save_restore_token_stale: "恢复预览后的事实已变化，请重新打开面板。",
        save_restore_confirmation_required: "恢复需要明确确认。",
        save_restore_high_risk_confirmation_required: "关闭恢复前安全备份时需要额外确认。",
        save_restore_pre_restore_backup_invalid: "恢复前安全备份未通过校验，未写入当前存档。",
        save_restore_facts_changed: "存档或备份事实已变化，请重新预览。",
        save_restore_lock_unavailable: "当前配置档正在执行其他写入操作。",
        save_restore_prepared_missing: "恢复暂存内容已失效，请重新预览。",
        save_restore_target_unavailable: "目标存档目录当前不可用。",
        save_restore_target_unsafe: "目标存档目录未通过安全校验。",
        save_restore_target_changed: "目标存档在预览后发生变化，请重新预览。",
        save_restore_commit_failed: "恢复提交失败，当前存档未被视为成功恢复。",
        save_restore_rolled_back: "恢复未完成，已自动恢复到操作前存档。",
        save_backup_history_unavailable: "恢复前安全备份失败，未写入当前存档。",
      },
      fallback: "存档恢复失败，当前存档未被视为成功恢复。",
    },
    warnings: {
      byCode: {
        save_restore_evidence_degraded: "任务或审计证据记录不完整，请保留诊断信息。",
        save_restore_recovery_cleanup_failed: "恢复证据未能自动清理，请保留现场并联系支持。",
        save_restore_recovery_evidence_unsafe: "恢复证据需要人工检查，请保留现场并联系支持。",
        save_restore_target_unavailable: "收尾时目标目录暂时不可用，请保留现场并联系支持。",
      },
      fallback: "恢复收尾证据需要检查，请保留现场并联系支持。",
    },
    cancelErrors: {
      byCode: {
        task_cannot_be_cancelled: "恢复已进入提交阶段，必须先完成提交或回滚收尾。",
        task_not_found: "恢复任务已结束或不再可取消。",
      },
      fallback: "取消请求未被接受，恢复任务仍按当前状态继续。",
    },
    dialog: {
      title: "恢复存档",
      description: "恢复会替换当前配置档的存档内容，请核对备份点与保护策略。",
      previewing: "正在校验归档与目标存档...",
      preparingChannel: "正在建立恢复进度通道...",
      listenerFailed: "无法订阅恢复进度，恢复尚未启动。请关闭面板后重试。",
      factBackupPoint: "备份点",
      factFiles: "文件",
      factFileCount: (count: number) => `${count} 个`,
      factUncompressedSize: "解压大小",
      protectionOnTitle: "恢复前安全备份已开启",
      protectionOnHint: "提交前会先创建独立保护点，失败时停止恢复。",
      protectionOffTitle: "恢复前安全备份已关闭",
      protectionOffHint: "本次恢复没有自动保护点，风险更高。",
      highRiskConfirmLabel: "我理解当前未启用恢复前安全备份，并确认继续。",
      startingTask: "正在启动恢复任务",
      cancellingTask: "正在取消恢复任务",
      completedInline: "恢复完成，当前存档已经过提交后校验。",
      recoveryRequiredTitle: "恢复需要人工收敛",
      recoveryRequiredSuffix: "请保留当前现场并联系支持，暂不要继续恢复。",
      cancelledInline: "恢复任务已取消，未继续进入玩家文件提交。",
      footerDone: "完成",
      footerCancelling: "正在取消",
      footerStarting: "正在启动",
      footerCommitting: "正在提交",
      footerCancelRestore: "取消恢复",
      footerCancel: "取消",
      footerConfirm: "确认恢复",
    },
    toasts: {
      completedTitle: "存档恢复完成",
      completedEvidenceTitle: "存档已恢复，证据需检查",
      completedMessage: "目标存档已通过校验并完成替换。",
      recoveryRequiredTitle: "存档恢复需要人工处理",
      failedTitle: "存档恢复失败",
      cancelledTitle: "已取消存档恢复",
      cancelledMessage: "未进入提交阶段的恢复工作已停止。",
      cancelRejectedTitle: "当前阶段无法取消",
    },
  },
  en: {
    phases: {
      "save_restore.queued": "Waiting for restore task",
      "save_restore.preparing": "Validating and preparing save data",
      "save_restore.revalidating": "Re-checking target state",
      "save_restore.pre_restore_backup": "Creating pre-restore safety backup",
      "save_restore.committing": "Replacing and validating save data",
    },
    errors: {
      byCode: {
        save_restore_profile_missing: "The profile no longer exists. Refresh and try again.",
        save_restore_backup_missing: "The selected backup record no longer exists. Refresh the backup history.",
        save_restore_backup_unavailable: "The selected backup is currently not available for restore.",
        save_restore_target_unset: "The current profile has no save data directory yet.",
        save_restore_target_invalid: "The save data directory of the current profile is invalid. Set it again first.",
        save_restore_game_running: "The game is still running. Exit the game completely and try again.",
        save_restore_game_running_unknown: "Could not confirm whether the game has exited. The restore was safely blocked.",
        save_restore_source_invalid: "The backup archive or manifest failed the safety validation.",
        save_restore_backup_directory_unavailable: "The backup directory is currently unreadable.",
        save_restore_archive_unavailable: "The backup archive file is currently unreadable.",
        save_restore_manifest_unavailable: "The backup manifest is currently unreadable.",
        save_restore_manifest_invalid: "The backup manifest is invalid and cannot be used for restore.",
        save_restore_archive_invalid: "The backup archive is invalid and cannot be used for restore.",
        save_restore_hash_mismatch: "Backup content verification mismatched. The restore was stopped.",
        save_restore_path_unsafe: "The backup contains unsafe paths. The restore was stopped.",
        save_restore_size_limit_exceeded: "Backup content exceeds the restore safety limit.",
        save_restore_staging_unavailable: "Could not create the controlled restore staging area.",
        save_restore_recovery_required: "The restore did not converge safely. Recovery evidence was preserved.",
        save_restore_transaction_unavailable: "Could not persist the restore transaction. The restore stopped safely.",
        save_restore_clock_unavailable: "Could not establish reliable restore time facts.",
        save_restore_token_issue_failed: "Could not create a restore preview token. Reopen the panel.",
        save_restore_token_invalid: "The restore preview token is invalid. Reopen the panel.",
        save_restore_token_expired: "The restore preview has expired. Reopen the panel.",
        save_restore_token_stale: "Facts changed after the restore preview. Reopen the panel.",
        save_restore_confirmation_required: "The restore requires explicit confirmation.",
        save_restore_high_risk_confirmation_required: "Additional confirmation is required when the pre-restore safety backup is disabled.",
        save_restore_pre_restore_backup_invalid: "The pre-restore safety backup failed validation. The current save data was not written.",
        save_restore_facts_changed: "Save data or backup facts changed. Preview again.",
        save_restore_lock_unavailable: "The current profile is performing another write operation.",
        save_restore_prepared_missing: "The staged restore content is no longer valid. Preview again.",
        save_restore_target_unavailable: "The target save data directory is currently unavailable.",
        save_restore_target_unsafe: "The target save data directory failed the safety validation.",
        save_restore_target_changed: "The target save data changed after the preview. Preview again.",
        save_restore_commit_failed: "The restore commit failed. The current save data is not considered restored.",
        save_restore_rolled_back: "The restore did not finish and was automatically rolled back to the previous save data.",
        save_backup_history_unavailable: "The pre-restore safety backup failed. The current save data was not written.",
      },
      fallback: "Save data restore failed. The current save data is not considered restored.",
    },
    warnings: {
      byCode: {
        save_restore_evidence_degraded: "Task or audit evidence records are incomplete. Keep the diagnostics.",
        save_restore_recovery_cleanup_failed: "Recovery evidence could not be cleaned up automatically. Preserve the scene and contact support.",
        save_restore_recovery_evidence_unsafe: "Recovery evidence needs manual inspection. Preserve the scene and contact support.",
        save_restore_target_unavailable: "The target directory was temporarily unavailable during finalization. Preserve the scene and contact support.",
      },
      fallback: "Restore finalization evidence needs inspection. Preserve the scene and contact support.",
    },
    cancelErrors: {
      byCode: {
        task_cannot_be_cancelled: "The restore has entered the commit phase and must finish committing or rolling back first.",
        task_not_found: "The restore task has ended or can no longer be cancelled.",
      },
      fallback: "The cancel request was not accepted. The restore task continues in its current state.",
    },
    dialog: {
      title: "Restore save data",
      description: "Restoring replaces the save data of the current profile. Review the backup point and protection policy.",
      previewing: "Validating archive and target save data...",
      preparingChannel: "Establishing restore progress channel...",
      listenerFailed: "Could not subscribe to restore progress; the restore has not started. Close the panel and try again.",
      factBackupPoint: "Backup point",
      factFiles: "Files",
      factFileCount: (count: number) => `${count}`,
      factUncompressedSize: "Uncompressed size",
      protectionOnTitle: "Pre-restore safety backup is on",
      protectionOnHint: "An independent protection point is created before committing; the restore stops on failure.",
      protectionOffTitle: "Pre-restore safety backup is off",
      protectionOffHint: "This restore has no automatic protection point and carries higher risk.",
      highRiskConfirmLabel: "I understand the pre-restore safety backup is disabled and confirm to continue.",
      startingTask: "Starting restore task",
      cancellingTask: "Cancelling restore task",
      completedInline: "Restore completed. The current save data passed post-commit validation.",
      recoveryRequiredTitle: "Restore needs manual convergence",
      recoveryRequiredSuffix: "Preserve the current scene and contact support. Do not continue restoring for now.",
      cancelledInline: "The restore task was cancelled before committing player files.",
      footerDone: "Done",
      footerCancelling: "Cancelling",
      footerStarting: "Starting",
      footerCommitting: "Committing",
      footerCancelRestore: "Cancel restore",
      footerCancel: "Cancel",
      footerConfirm: "Confirm restore",
    },
    toasts: {
      completedTitle: "Save data restore completed",
      completedEvidenceTitle: "Save data restored; evidence needs review",
      completedMessage: "The target save data passed validation and was replaced.",
      recoveryRequiredTitle: "Save data restore needs manual handling",
      failedTitle: "Save data restore failed",
      cancelledTitle: "Save data restore cancelled",
      cancelledMessage: "Restore work that had not entered the commit phase was stopped.",
      cancelRejectedTitle: "Cannot cancel at this phase",
    },
  },
  ja: {
    phases: {
      "save_restore.queued": "復元タスクを待機中",
      "save_restore.preparing": "セーブデータを検証・準備中",
      "save_restore.revalidating": "対象の状態を再確認中",
      "save_restore.pre_restore_backup": "復元前セーフティバックアップを作成中",
      "save_restore.committing": "セーブデータを置き換えて検証中",
    },
    errors: {
      byCode: {
        save_restore_profile_missing: "プロファイルは存在しません。更新してから再試行してください。",
        save_restore_backup_missing: "選択したバックアップ記録は存在しません。バックアップ履歴を更新してください。",
        save_restore_backup_unavailable: "選択したバックアップは現在復元に利用できません。",
        save_restore_target_unset: "現在のプロファイルにはセーブデータディレクトリが未設定です。",
        save_restore_target_invalid: "現在のプロファイルのセーブデータディレクトリが無効です。先に設定し直してください。",
        save_restore_game_running: "ゲームがまだ実行中です。完全に終了してから再試行してください。",
        save_restore_game_running_unknown: "ゲームの終了を確認できません。復元は安全に遮断されました。",
        save_restore_source_invalid: "バックアップのアーカイブまたはマニフェストが安全検証を通過しませんでした。",
        save_restore_backup_directory_unavailable: "バックアップディレクトリを現在読み取れません。",
        save_restore_archive_unavailable: "バックアップのアーカイブファイルを現在読み取れません。",
        save_restore_manifest_unavailable: "バックアップマニフェストを現在読み取れません。",
        save_restore_manifest_invalid: "バックアップマニフェストが無効なため、復元に使用できません。",
        save_restore_archive_invalid: "バックアップアーカイブが無効なため、復元に使用できません。",
        save_restore_hash_mismatch: "バックアップ内容の検証が一致しません。復元を停止しました。",
        save_restore_path_unsafe: "バックアップに安全でないパスが含まれています。復元を停止しました。",
        save_restore_size_limit_exceeded: "バックアップ内容が復元の安全上限を超えています。",
        save_restore_staging_unavailable: "管理された復元ステージング領域を作成できません。",
        save_restore_recovery_required: "復元が安全に収束せず、復旧証跡を保全しました。",
        save_restore_transaction_unavailable: "復元トランザクションを永続化できず、復元は安全に停止しました。",
        save_restore_clock_unavailable: "信頼できる復元時刻を確立できません。",
        save_restore_token_issue_failed: "復元プレビュートークンを作成できません。パネルを開き直してください。",
        save_restore_token_invalid: "復元プレビュートークンが無効です。パネルを開き直してください。",
        save_restore_token_expired: "復元プレビューの有効期限が切れました。パネルを開き直してください。",
        save_restore_token_stale: "プレビュー後に事実が変化しました。パネルを開き直してください。",
        save_restore_confirmation_required: "復元には明示的な確認が必要です。",
        save_restore_high_risk_confirmation_required: "復元前セーフティバックアップを無効にする場合は追加確認が必要です。",
        save_restore_pre_restore_backup_invalid: "復元前セーフティバックアップが検証を通過せず、現在のセーブデータへは書き込みませんでした。",
        save_restore_facts_changed: "セーブデータまたはバックアップの事実が変化しました。再度プレビューしてください。",
        save_restore_lock_unavailable: "現在のプロファイルは別の書き込み操作を実行中です。",
        save_restore_prepared_missing: "復元のステージング内容が無効になりました。再度プレビューしてください。",
        save_restore_target_unavailable: "対象のセーブデータディレクトリが現在利用できません。",
        save_restore_target_unsafe: "対象のセーブデータディレクトリが安全検証を通過しませんでした。",
        save_restore_target_changed: "プレビュー後に対象のセーブデータが変化しました。再度プレビューしてください。",
        save_restore_commit_failed: "復元のコミットに失敗しました。現在のセーブデータは復元成功とは見なされません。",
        save_restore_rolled_back: "復元は完了せず、操作前のセーブデータへ自動的に戻しました。",
        save_backup_history_unavailable: "復元前セーフティバックアップに失敗し、現在のセーブデータへは書き込みませんでした。",
      },
      fallback: "セーブデータの復元に失敗しました。現在のセーブデータは復元成功とは見なされません。",
    },
    warnings: {
      byCode: {
        save_restore_evidence_degraded: "タスクまたは監査証跡の記録が不完全です。診断情報を保全してください。",
        save_restore_recovery_cleanup_failed: "復旧証跡を自動整理できませんでした。現場を保全してサポートに連絡してください。",
        save_restore_recovery_evidence_unsafe: "復旧証跡には人手での確認が必要です。現場を保全してサポートに連絡してください。",
        save_restore_target_unavailable: "終了処理中に対象ディレクトリが一時的に利用できませんでした。現場を保全してサポートに連絡してください。",
      },
      fallback: "復元終了処理の証跡に確認が必要です。現場を保全してサポートに連絡してください。",
    },
    cancelErrors: {
      byCode: {
        task_cannot_be_cancelled: "復元はコミット段階に入っており、コミットまたはロールバックの完了が先に必要です。",
        task_not_found: "復元タスクは終了したか、キャンセルできなくなりました。",
      },
      fallback: "キャンセル要求は受け付けられませんでした。復元タスクは現在の状態のまま継続します。",
    },
    dialog: {
      title: "セーブデータを復元",
      description: "復元は現在のプロファイルのセーブデータを置き換えます。バックアップポイントと保護ポリシーを確認してください。",
      previewing: "アーカイブと対象セーブデータを検証中...",
      preparingChannel: "復元進捗チャンネルを確立中...",
      listenerFailed: "復元進捗を購読できず、復元は開始されていません。パネルを閉じて再試行してください。",
      factBackupPoint: "バックアップポイント",
      factFiles: "ファイル",
      factFileCount: (count: number) => `${count} 件`,
      factUncompressedSize: "展開後サイズ",
      protectionOnTitle: "復元前セーフティバックアップは有効",
      protectionOnHint: "コミット前に独立した保護ポイントを作成し、失敗時は復元を停止します。",
      protectionOffTitle: "復元前セーフティバックアップは無効",
      protectionOffHint: "今回の復元には自動保護ポイントがなく、リスクが高くなります。",
      highRiskConfirmLabel: "復元前セーフティバックアップが無効であることを理解し、続行を確認します。",
      startingTask: "復元タスクを開始中",
      cancellingTask: "復元タスクをキャンセル中",
      completedInline: "復元が完了し、現在のセーブデータはコミット後検証を通過しました。",
      recoveryRequiredTitle: "復元には人手での収束が必要です",
      recoveryRequiredSuffix: "現場を保全してサポートに連絡し、当面は復元を続行しないでください。",
      cancelledInline: "復元タスクはキャンセルされ、プレイヤーファイルのコミットへは進みませんでした。",
      footerDone: "完了",
      footerCancelling: "キャンセル中",
      footerStarting: "開始中",
      footerCommitting: "コミット中",
      footerCancelRestore: "復元をキャンセル",
      footerCancel: "キャンセル",
      footerConfirm: "復元を確認",
    },
    toasts: {
      completedTitle: "セーブデータの復元が完了",
      completedEvidenceTitle: "復元済み。証跡の確認が必要",
      completedMessage: "対象のセーブデータは検証を通過し、置き換えが完了しました。",
      recoveryRequiredTitle: "セーブデータの復元に人手対応が必要",
      failedTitle: "セーブデータの復元に失敗",
      cancelledTitle: "セーブデータの復元をキャンセル",
      cancelledMessage: "コミット段階に入っていない復元作業を停止しました。",
      cancelRejectedTitle: "現在の段階ではキャンセルできません",
    },
  },
} satisfies LocaleDictionary<SaveRestoreCopy>;
