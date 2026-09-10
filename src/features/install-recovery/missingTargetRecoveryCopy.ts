import type { LocaleDictionary } from "../../shared/i18n";
import type { InstallRecoveryActionKind } from "../mods/modInstallPlanTypes";
import type { RecoveryCenterCopy, RecoveryRepairSummaryCopy } from "./recoveryCenterCopy";

export type MissingTargetRecoveryCopy = {
  action: string;
  busy: string;
  reviewDescription: string;
  safeguards: string[];
  summary: RecoveryRepairSummaryCopy & { blockingReason: string };
  statsMissing: (count: number) => string;
  panel: Partial<RecoveryCenterCopy["page"]["rollbackPanel"]>;
  failures: Partial<RecoveryCenterCopy["rollback"]["failures"]>;
  phases: RecoveryCenterCopy["rollback"]["phases"];
  blockReasons: Record<string, string>;
  taskErrors: Record<string, string>;
};

export const missingTargetRecoveryCopy = {
  zh_cn: {
    action: "预览卸载清理", busy: "正在处理", reviewDescription: "在下方选择文件缺失的 Mod，先检查卸载清理方案。",
    safeguards: ["保留未知或已变化的文件", "先核对原文件备份", "确认后再次检查当前状态"],
    summary: { title: "安装文件缺失", description: "可以预览卸载清理：核对仍存在的文件、恢复原文件备份，并清理缺失文件的安装记录。", actionLabel: "预览卸载清理", blockingReason: "需要先核对当前文件与备份，并由你确认后执行。" },
    statsMissing: (count: number) => `当前已缺失 ${count} 个目标文件`,
    panel: { statusAria: "缺失文件恢复状态", progressAria: "卸载清理进度", previewingTitle: "正在检查卸载清理条件", startingTitle: "正在启动卸载清理",
      blockedTitle: "暂时无法安全卸载清理", blockedDetail: (name: string) => `${name}：请先处理下列问题，再重新检查。`,
      confirmTitle: "确认卸载并清理缺失目标", confirmBody: (name: string) => `将卸载 ${name}：删除仍存在且内容匹配的 Mod 文件、恢复可用的原文件备份，并清理已缺失文件的安装记录。已缺失且没有备份的文件不会被重新创建。`, confirmAction: "确认卸载清理",
      completedTitle: "卸载清理完成", completedBody: (name: string) => `${name} 的安装记录已清理，已重新扫描当前配置档。`, failedTitle: "卸载清理未完成" },
    failures: { previewFailed: "无法验证卸载清理方案，请重新扫描后再试。", startFailed: "无法启动卸载清理任务，请重新预览。", taskFallback: "卸载清理未完成，请重新扫描查看状态。" },
    phases: { "install.recovery.queued": "等待处理", "install.recovery.planning": "核对文件与备份", "install.recovery.processing": "卸载并清理记录", "install.recovery.completed": "卸载清理完成", "install.recovery.failed": "卸载清理未完成" },
    blockReasons: { install_state_unavailable: "安装记录无法验证", target_state_unavailable: "文件已变化或无法验证，请重新扫描", backup_unavailable: "所需备份缺失或无法读取", recovery_pending: "配置档还有未完成的恢复事务，请先处理", preview_required: "请先重新预览卸载清理方案", game_running: "请先关闭游戏", game_running_unknown: "无法确认游戏是否运行，请稍后重试" },
    taskErrors: {
      "install_recovery_failed:stale_preview": "预览后游戏目录、文件、备份或安装记录发生了变化，请重新预览。",
      "install_recovery_failed:game_running": "游戏正在运行，请关闭游戏后重新预览。",
      "install_recovery_failed:game_running_unknown": "无法确认游戏运行状态，请稍后重试。",
      "install_recovery_failed:planning": "当前文件或恢复状态无法验证，请重新扫描并预览。",
      "install_recovery_failed:processing": "卸载清理未完成，请保留现有文件和备份，重新扫描检查状态。",
    },
  },
  en: {
    action: "Review uninstall", busy: "Working", reviewDescription: "Choose a mod with missing files below to review a safe uninstall plan.",
    safeguards: ["Preserve unknown or changed files", "Verify original file backups", "Recheck the state after confirmation"],
    summary: { title: "Installed files are missing", description: "Review an uninstall plan that verifies remaining files, restores available backups and removes records for missing files.", actionLabel: "Review uninstall", blockingReason: "The backend must verify files and backups before you confirm." },
    statsMissing: (count: number) => `${count} target files are currently missing`,
    panel: { statusAria: "Missing file recovery status", progressAria: "Uninstall progress", previewingTitle: "Checking uninstall conditions", startingTitle: "Starting uninstall",
      blockedTitle: "Safe uninstall is currently blocked", blockedDetail: (name: string) => `${name}: resolve the issues below, then check again.`,
      confirmTitle: "Confirm uninstall and cleanup", confirmBody: (name: string) => `Uninstall ${name}: remove matching mod files, restore available original backups and clear records for missing files. Missing files without backups will not be recreated.`, confirmAction: "Confirm uninstall",
      completedTitle: "Uninstall completed", completedBody: (name: string) => `Installation records for ${name} were cleared. The current profile was scanned again.`, failedTitle: "Uninstall did not complete" },
    failures: { previewFailed: "The uninstall plan could not be verified. Scan again and retry.", startFailed: "The uninstall task could not start. Review a new plan.", taskFallback: "Uninstall did not complete. Scan again to inspect the state." },
    phases: { "install.recovery.queued": "Waiting", "install.recovery.planning": "Checking files and backups", "install.recovery.processing": "Uninstalling and clearing records", "install.recovery.completed": "Uninstall completed", "install.recovery.failed": "Uninstall did not complete" },
    blockReasons: { install_state_unavailable: "Installation records cannot be verified", target_state_unavailable: "Files changed or cannot be verified; scan again", backup_unavailable: "A required backup is missing or unreadable", recovery_pending: "Resolve the profile's pending recovery transactions first", preview_required: "Review a new uninstall plan first", game_running: "Close the game first", game_running_unknown: "Game running state is unknown; retry later" },
    taskErrors: {
      "install_recovery_failed:stale_preview": "The game directory, files, backups or installation records changed after preview. Review a new plan.",
      "install_recovery_failed:game_running": "Close the running game and review a new plan.",
      "install_recovery_failed:game_running_unknown": "Game running state is unknown. Retry later.",
      "install_recovery_failed:planning": "Files or recovery state could not be verified. Scan again and review a new plan.",
      "install_recovery_failed:processing": "Uninstall did not complete. Preserve current files and backups, then scan again.",
    },
  },
  ja: {
    action: "アンインストールを確認", busy: "処理中", reviewDescription: "下の一覧からファイルが欠落した Mod を選び、アンインストール内容を確認してください。",
    safeguards: ["不明または変更済みのファイルを保持", "元ファイルのバックアップを検証", "確認後に現在の状態を再検証"],
    summary: { title: "インストール済みファイルが欠落", description: "残っているファイルを照合し、利用可能なバックアップを復元して、欠落ファイルの記録を整理する手順を確認できます。", actionLabel: "アンインストールを確認", blockingReason: "ファイルとバックアップを検証した後、確認して実行してください。" },
    statsMissing: (count: number) => `現在 ${count} 個の対象ファイルが欠落しています`,
    panel: { statusAria: "欠落ファイルの復旧状態", progressAria: "アンインストールの進捗", previewingTitle: "アンインストール条件を確認中", startingTitle: "アンインストールを開始中",
      blockedTitle: "安全にアンインストールできません", blockedDetail: (name: string) => `${name}：下記の問題を解決してから再確認してください。`,
      confirmTitle: "アンインストールと記録整理の確認", confirmBody: (name: string) => `${name} をアンインストールします。一致する Mod ファイルを削除し、利用可能な元ファイルのバックアップを復元して、欠落したファイルの記録を整理します。バックアップがない欠落ファイルは再作成されません。`, confirmAction: "アンインストールを実行",
      completedTitle: "アンインストール完了", completedBody: (name: string) => `${name} のインストール記録を整理し、現在のプロファイルを再スキャンしました。`, failedTitle: "アンインストール未完了" },
    failures: { previewFailed: "アンインストール内容を検証できません。再スキャンしてからお試しください。", startFailed: "処理を開始できません。内容を再確認してください。", taskFallback: "処理が完了していません。再スキャンして状態を確認してください。" },
    phases: { "install.recovery.queued": "待機中", "install.recovery.planning": "ファイルとバックアップを確認中", "install.recovery.processing": "アンインストールと記録整理中", "install.recovery.completed": "アンインストール完了", "install.recovery.failed": "アンインストール未完了" },
    blockReasons: { install_state_unavailable: "インストール記録を検証できません", target_state_unavailable: "ファイルが変更されたか検証できません。再スキャンしてください", backup_unavailable: "必要なバックアップが欠落しているか読み取れません", recovery_pending: "先にプロファイルの未完了の復旧処理を解決してください", preview_required: "先にアンインストール内容を再確認してください", game_running: "先にゲームを終了してください", game_running_unknown: "ゲームの起動状態を確認できません。後ほどお試しください" },
    taskErrors: {
      "install_recovery_failed:stale_preview": "確認後にゲームフォルダー、ファイル、バックアップ、または記録が変わりました。再確認してください。",
      "install_recovery_failed:game_running": "ゲームを終了してから内容を再確認してください。",
      "install_recovery_failed:game_running_unknown": "ゲームの起動状態を確認できません。後ほどお試しください。",
      "install_recovery_failed:planning": "ファイルまたは復旧状態を検証できません。再スキャンして内容を再確認してください。",
      "install_recovery_failed:processing": "アンインストールが完了していません。現在のファイルとバックアップを保持し、再スキャンしてください。",
    },
  },
} satisfies LocaleDictionary<MissingTargetRecoveryCopy>;

export function copyForRecoveryAction(copy: RecoveryCenterCopy, actionKind: InstallRecoveryActionKind): RecoveryCenterCopy {
  if (actionKind !== "uninstall_missing_targets") return copy;
  return { ...copy, page: { ...copy.page, rollbackPanel: { ...copy.page.rollbackPanel, ...copy.missingTargets.panel } },
    rollback: { ...copy.rollback, phases: copy.missingTargets.phases, failures: { ...copy.rollback.failures, ...copy.missingTargets.failures } } };
}
