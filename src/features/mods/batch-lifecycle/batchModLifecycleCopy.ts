import type { LocaleDictionary } from "../../../shared/i18n";
import type {
  BatchModLifecycleAttemptStatus,
  BatchModLifecycleCapabilityDto,
  BatchModLifecycleItemStatus,
  BatchModLifecycleOperation,
} from "./batchModLifecycleTypes";

// 批量生命周期（批量安装/卸载/重装）的全部用户可见文案。
// getter 保持原签名形态但改收 copy：语义分支（能力码、状态码、排除原因）
// 留在这里，未知码 fail-open 回退到稳定兜底或原样透传。

export type BatchModLifecycleCopy = {
  capability: {
    loading: string;
    sandboxForbidden: string;
    unavailable: string;
    nullReason: string;
    unsupported: string;
  };
  operations: Record<BatchModLifecycleOperation, string>;
  itemStatus: Record<BatchModLifecycleItemStatus, string>;
  attemptStatus: Record<BatchModLifecycleAttemptStatus, string>;
  excludedReasons: Record<string, string>;
  excludedReasonFallback: string;
  reasonCodes: Record<string, string>;
  errors: Record<string, string>;
  errorFallback: string;
  previewPanel: {
    summaryAria: string;
    totalCount: (count: number) => string;
    readyCount: (count: number) => string;
    blockedCount: (count: number) => string;
    addedCount: (count: number) => string;
    retainedCount: (count: number) => string;
    replacedCount: (count: number) => string;
    staleCount: (count: number) => string;
    actionCount: (count: number) => string;
    blockedNote: (count: number) => string;
    itemsAria: string;
    itemsTitle: string;
    displayRevision: string;
    layerLabel: string;
    installedRevision: string;
    candidateDisplayRevision: string;
    targetLabel: string;
    switchTo: (targetId: string) => string;
    keepCurrent: string;
    closeAria: string;
    generating: string;
    targetSelectionAria: string;
    targetSelectionTitle: string;
    targetSelectionHint: string;
    targetUnavailable: string;
    targetGroupAria: (modId: string) => string;
    excludedTitle: string;
    excludedItem: (modId: string, reason: string) => string;
    unresolvableAria: string;
    unresolvableTitle: string;
    unresolvableItem: (modId: string) => string;
    policyTitle: string;
    stopOnFailure: string;
    continueOnFailure: string;
    cancel: string;
    generatePreview: string;
    confirmStart: string;
  };
  resultPanel: {
    resultTitle: (operationLabel: string) => string;
    closeAria: string;
    batchIdLabel: (batchId: string) => string;
    summaryAria: string;
    succeededCount: (count: number) => string;
    failedCount: (count: number) => string;
    blockedCount: (count: number) => string;
    skippedCount: (count: number) => string;
    cancelledCount: (count: number) => string;
    recoveryRequiredCount: (count: number) => string;
    evidenceDegraded: string;
    itemsAria: string;
    retryableBadge: string;
    close: string;
    loadMore: string;
    retryFailed: string;
  };
  runningPanel: {
    running: Record<BatchModLifecycleOperation, string>;
  };
};

export const batchModLifecycleCopy = {
  zh_cn: {
    capability: {
      loading: "正在确认批量操作权限，请稍候",
      sandboxForbidden: "当前版本仅允许在受控测试环境执行批量操作",
      unavailable: "无法确认批量操作权限，请刷新后重试",
      nullReason: "批量操作当前不可用",
      unsupported: "当前环境不支持批量操作",
    },
    operations: {
      install: "批量安装",
      uninstall: "批量卸载",
      reinstall: "批量重装",
    },
    itemStatus: {
      running: "执行中",
      succeeded: "成功",
      blocked: "已阻止",
      failed: "失败",
      recovery_required: "需要恢复",
      cancelled: "已取消",
      skipped: "已跳过",
    },
    attemptStatus: {
      sealed: "已封存",
      queued: "排队中",
      running: "执行中",
      stopping: "停止中",
      completed: "全部成功",
      completed_with_errors: "部分成功",
      blocked: "已被阻止",
      cancelled: "已取消",
      recovery_required: "需要恢复",
      interrupted: "已中断",
      failed: "失败",
    },
    excludedReasons: {
      already_installed: "已安装，不参与本次安装",
      not_installed: "未安装，不参与本次卸载/重装",
      installed_revision_unavailable: "已安装但缺少版本信息（旧格式清单），无法参与",
    },
    excludedReasonFallback: "不参与本次操作",
    reasonCodes: {
      stopped_after_item_failure: "因前一项失败而停止",
      cancelled_before_start: "开始前已取消",
      batch_item_plan_stale: "单项计划已过期",
      source_revision_changed: "来源版本已变化",
      manifest_changed: "安装清单已变化",
      target_changed: "目标文件已变化",
      rollback_succeeded: "已回滚",
      recovery_required: "需要恢复",
    },
    errors: {
      batch_no_applicable_items: "选中的 Mod 均不适用于该操作，或无法读取版本信息",
      batch_facts_unavailable: "无法读取安装状态或版本信息",
      batch_replacement_facts_unavailable: "同版本重装所需的目标信息不可用",
      batch_input_invalid: "批量请求不合法",
      batch_duplicate_item: "批量请求包含重复的 Mod",
      batch_resource_limit_exceeded: "批量请求超出资源上限（最多 100 项）",
      batch_global_target_conflict: "多个 Mod 的目标文件互相冲突",
      batch_plan_blocked: "批量计划被阻止执行",
      batch_plan_stale: "批量计划已过期，请重新预览",
      batch_plan_expired: "批量计划已过期，请重新预览",
      batch_token_invalid: "批量操作凭证无效",
      batch_retry_unavailable: "当前没有可重试的项",
      batch_attempt_stale: "已有更新的执行尝试，请刷新结果",
      batch_result_unavailable: "无法读取批量执行结果",
      batch_journal_unavailable: "批量执行记录不可用",
      batch_evidence_unavailable: "批量执行证据不可用",
      sandbox_batch_production_forbidden: "批量操作仅在测试环境可用",
      batch_internal_error: "批量操作失败，请稍后重试",
    },
    errorFallback: "批量操作失败",
    previewPanel: {
      summaryAria: "批量计划摘要",
      totalCount: (count: number) => `共 ${count} 项`,
      readyCount: (count: number) => `可执行 ${count} 项`,
      blockedCount: (count: number) => `被阻止 ${count} 项`,
      addedCount: (count: number) => `新增 ${count}`,
      retainedCount: (count: number) => `保留 ${count}`,
      replacedCount: (count: number) => `替换 ${count}`,
      staleCount: (count: number) => `过期 ${count}`,
      actionCount: (count: number) => `动作 ${count}`,
      blockedNote: (count: number) =>
        `${count} 项因版本或目标冲突被阻止；继续执行时将跳过这些项。`,
      itemsAria: "批量计划逐项明细",
      itemsTitle: "逐项计划",
      displayRevision: "展示版本",
      layerLabel: "层级",
      installedRevision: "已安装版本",
      candidateDisplayRevision: "候选展示版本",
      targetLabel: "目标",
      switchTo: (targetId: string) => `切换至 ${targetId}`,
      keepCurrent: "保持当前目标",
      closeAria: "关闭",
      generating: "正在生成批量计划…",
      targetSelectionAria: "批量重装目标选择",
      targetSelectionTitle: "选择需要切换的外观目标",
      targetSelectionHint: "同一版本的重装需要为每个可重定向 Mod 选择一个不同于当前目标的目标。",
      targetUnavailable: "当前 Mod 没有可用的目标切换选项，无法参加本次同版本重装。",
      targetGroupAria: (modId: string) => `${modId} 的替换目标`,
      excludedTitle: "不参与本次操作的项",
      excludedItem: (modId: string, reason: string) => `${modId}：${reason}`,
      unresolvableAria: "无法解析的项",
      unresolvableTitle: "无法解析版本的项",
      unresolvableItem: (modId: string) => `${modId}：无法读取版本信息`,
      policyTitle: "执行策略",
      stopOnFailure: "遇到失败即停止（推荐）",
      continueOnFailure: "跳过失败项继续",
      cancel: "取消",
      generatePreview: "生成批量预览",
      confirmStart: "确认并开始",
    },
    resultPanel: {
      resultTitle: (operationLabel: string) => `${operationLabel}结果`,
      closeAria: "关闭",
      batchIdLabel: (batchId: string) => `批次 ${batchId}`,
      summaryAria: "批量结果汇总",
      succeededCount: (count: number) => `成功 ${count}`,
      failedCount: (count: number) => `失败 ${count}`,
      blockedCount: (count: number) => `被阻止 ${count}`,
      skippedCount: (count: number) => `跳过 ${count}`,
      cancelledCount: (count: number) => `取消 ${count}`,
      recoveryRequiredCount: (count: number) => `需恢复 ${count}`,
      evidenceDegraded: "部分执行证据健康度下降，请前往恢复中心检查。",
      itemsAria: "逐项结果",
      retryableBadge: "可重试",
      close: "关闭",
      loadMore: "加载更多",
      retryFailed: "重试失败项",
    },
    runningPanel: {
      running: {
        install: "正在执行批量安装…",
        uninstall: "正在执行批量卸载…",
        reinstall: "正在执行批量重装…",
      },
    },
  },
  en: {
    capability: {
      loading: "Confirming batch operation permission, please wait",
      sandboxForbidden: "This version only allows batch operations in a controlled test environment",
      unavailable: "Batch operation permission could not be confirmed. Refresh and retry",
      nullReason: "Batch operations are currently unavailable",
      unsupported: "The current environment does not support batch operations",
    },
    operations: {
      install: "Batch Install",
      uninstall: "Batch Uninstall",
      reinstall: "Batch Reinstall",
    },
    itemStatus: {
      running: "Running",
      succeeded: "Succeeded",
      blocked: "Blocked",
      failed: "Failed",
      recovery_required: "Recovery required",
      cancelled: "Cancelled",
      skipped: "Skipped",
    },
    attemptStatus: {
      sealed: "Sealed",
      queued: "Queued",
      running: "Running",
      stopping: "Stopping",
      completed: "All succeeded",
      completed_with_errors: "Partially succeeded",
      blocked: "Blocked",
      cancelled: "Cancelled",
      recovery_required: "Recovery required",
      interrupted: "Interrupted",
      failed: "Failed",
    },
    excludedReasons: {
      already_installed: "Already installed; excluded from this install",
      not_installed: "Not installed; excluded from this uninstall/reinstall",
      installed_revision_unavailable: "Installed but missing revision info (legacy manifest); cannot participate",
    },
    excludedReasonFallback: "Excluded from this operation",
    reasonCodes: {
      stopped_after_item_failure: "Stopped after a previous item failed",
      cancelled_before_start: "Cancelled before start",
      batch_item_plan_stale: "Item plan expired",
      source_revision_changed: "Source revision changed",
      manifest_changed: "Install manifest changed",
      target_changed: "Target file changed",
      rollback_succeeded: "Rolled back",
      recovery_required: "Recovery required",
    },
    errors: {
      batch_no_applicable_items: "None of the selected mods are applicable to this operation, or revision info could not be read",
      batch_facts_unavailable: "Install status or revision info could not be read",
      batch_replacement_facts_unavailable: "Target info required for same-revision reinstall is unavailable",
      batch_input_invalid: "The batch request is invalid",
      batch_duplicate_item: "The batch request contains duplicate mods",
      batch_resource_limit_exceeded: "The batch request exceeds the resource limit (up to 100 items)",
      batch_global_target_conflict: "Target files of multiple mods conflict with each other",
      batch_plan_blocked: "The batch plan was blocked from executing",
      batch_plan_stale: "The batch plan has expired. Preview again",
      batch_plan_expired: "The batch plan has expired. Preview again",
      batch_token_invalid: "The batch operation token is invalid",
      batch_retry_unavailable: "There are no retryable items right now",
      batch_attempt_stale: "A newer attempt exists. Refresh the results",
      batch_result_unavailable: "Batch execution results could not be read",
      batch_journal_unavailable: "The batch execution journal is unavailable",
      batch_evidence_unavailable: "Batch execution evidence is unavailable",
      sandbox_batch_production_forbidden: "Batch operations are only available in test environments",
      batch_internal_error: "Batch operation failed. Please try again later",
    },
    errorFallback: "Batch operation failed",
    previewPanel: {
      summaryAria: "Batch plan summary",
      totalCount: (count: number) => `${count} total`,
      readyCount: (count: number) => `${count} executable`,
      blockedCount: (count: number) => `${count} blocked`,
      addedCount: (count: number) => `Added ${count}`,
      retainedCount: (count: number) => `Retained ${count}`,
      replacedCount: (count: number) => `Replaced ${count}`,
      staleCount: (count: number) => `Stale ${count}`,
      actionCount: (count: number) => `Actions ${count}`,
      blockedNote: (count: number) =>
        `${count} item${count === 1 ? " is" : "s are"} blocked by revision or target conflicts; they will be skipped when continuing.`,
      itemsAria: "Batch plan item details",
      itemsTitle: "Per-item Plan",
      displayRevision: "Display revision",
      layerLabel: "Layer",
      installedRevision: "Installed revision",
      candidateDisplayRevision: "Candidate display revision",
      targetLabel: "Target",
      switchTo: (targetId: string) => `Switch to ${targetId}`,
      keepCurrent: "Keep current target",
      closeAria: "Close",
      generating: "Generating batch plan…",
      targetSelectionAria: "Batch reinstall target selection",
      targetSelectionTitle: "Choose the appearance targets to switch",
      targetSelectionHint: "A same-revision reinstall requires choosing, for each retargetable mod, a target different from its current one.",
      targetUnavailable: "This mod has no available target switch options and cannot join this same-revision reinstall.",
      targetGroupAria: (modId: string) => `Replacement targets for ${modId}`,
      excludedTitle: "Items excluded from this operation",
      excludedItem: (modId: string, reason: string) => `${modId}: ${reason}`,
      unresolvableAria: "Unresolvable items",
      unresolvableTitle: "Items with unresolvable revisions",
      unresolvableItem: (modId: string) => `${modId}: revision info could not be read`,
      policyTitle: "Execution Policy",
      stopOnFailure: "Stop on failure (recommended)",
      continueOnFailure: "Skip failed items and continue",
      cancel: "Cancel",
      generatePreview: "Generate batch preview",
      confirmStart: "Confirm and start",
    },
    resultPanel: {
      resultTitle: (operationLabel: string) => `${operationLabel} Results`,
      closeAria: "Close",
      batchIdLabel: (batchId: string) => `Batch ${batchId}`,
      summaryAria: "Batch result summary",
      succeededCount: (count: number) => `Succeeded ${count}`,
      failedCount: (count: number) => `Failed ${count}`,
      blockedCount: (count: number) => `Blocked ${count}`,
      skippedCount: (count: number) => `Skipped ${count}`,
      cancelledCount: (count: number) => `Cancelled ${count}`,
      recoveryRequiredCount: (count: number) => `Recovery required ${count}`,
      evidenceDegraded: "Some execution evidence health has degraded. Check the Recovery Center.",
      itemsAria: "Per-item results",
      retryableBadge: "Retryable",
      close: "Close",
      loadMore: "Load more",
      retryFailed: "Retry failed items",
    },
    runningPanel: {
      running: {
        install: "Running batch install…",
        uninstall: "Running batch uninstall…",
        reinstall: "Running batch reinstall…",
      },
    },
  },
  ja: {
    capability: {
      loading: "バッチ操作の権限を確認しています。お待ちください",
      sandboxForbidden: "現在のバージョンでは、バッチ操作は管理されたテスト環境でのみ実行できます",
      unavailable: "バッチ操作の権限を確認できません。更新して再試行してください",
      nullReason: "バッチ操作は現在利用できません",
      unsupported: "現在の環境はバッチ操作に対応していません",
    },
    operations: {
      install: "一括インストール",
      uninstall: "一括アンインストール",
      reinstall: "一括再インストール",
    },
    itemStatus: {
      running: "実行中",
      succeeded: "成功",
      blocked: "ブロック済み",
      failed: "失敗",
      recovery_required: "復旧が必要",
      cancelled: "キャンセル済み",
      skipped: "スキップ済み",
    },
    attemptStatus: {
      sealed: "確定済み",
      queued: "待機中",
      running: "実行中",
      stopping: "停止中",
      completed: "すべて成功",
      completed_with_errors: "一部成功",
      blocked: "ブロック済み",
      cancelled: "キャンセル済み",
      recovery_required: "復旧が必要",
      interrupted: "中断済み",
      failed: "失敗",
    },
    excludedReasons: {
      already_installed: "インストール済みのため、今回のインストールの対象外です",
      not_installed: "未インストールのため、今回のアンインストール／再インストールの対象外です",
      installed_revision_unavailable: "インストール済みですがバージョン情報が欠落しています（旧形式マニフェスト）。参加できません",
    },
    excludedReasonFallback: "今回の操作の対象外です",
    reasonCodes: {
      stopped_after_item_failure: "前の項目の失敗により停止しました",
      cancelled_before_start: "開始前にキャンセルされました",
      batch_item_plan_stale: "項目プランが失効しました",
      source_revision_changed: "ソースバージョンが変化しました",
      manifest_changed: "インストールマニフェストが変化しました",
      target_changed: "ターゲットファイルが変化しました",
      rollback_succeeded: "ロールバック済み",
      recovery_required: "復旧が必要",
    },
    errors: {
      batch_no_applicable_items: "選択した Mod はいずれもこの操作に適用できないか、バージョン情報を読み取れません",
      batch_facts_unavailable: "インストール状態またはバージョン情報を読み取れません",
      batch_replacement_facts_unavailable: "同一バージョン再インストールに必要なターゲット情報を利用できません",
      batch_input_invalid: "バッチ要求が不正です",
      batch_duplicate_item: "バッチ要求に重複する Mod が含まれています",
      batch_resource_limit_exceeded: "バッチ要求がリソース上限（最大 100 件）を超えています",
      batch_global_target_conflict: "複数の Mod のターゲットファイルが互いに競合しています",
      batch_plan_blocked: "バッチプランの実行がブロックされました",
      batch_plan_stale: "バッチプランが失効しました。再度プレビューしてください",
      batch_plan_expired: "バッチプランが失効しました。再度プレビューしてください",
      batch_token_invalid: "バッチ操作トークンが無効です",
      batch_retry_unavailable: "現在再試行できる項目はありません",
      batch_attempt_stale: "より新しい実行が存在します。結果を更新してください",
      batch_result_unavailable: "バッチ実行結果を読み取れません",
      batch_journal_unavailable: "バッチ実行記録を利用できません",
      batch_evidence_unavailable: "バッチ実行証跡を利用できません",
      sandbox_batch_production_forbidden: "バッチ操作はテスト環境でのみ利用できます",
      batch_internal_error: "バッチ操作に失敗しました。しばらくしてから再試行してください",
    },
    errorFallback: "バッチ操作に失敗しました",
    previewPanel: {
      summaryAria: "バッチプランの概要",
      totalCount: (count: number) => `合計 ${count} 件`,
      readyCount: (count: number) => `実行可能 ${count} 件`,
      blockedCount: (count: number) => `ブロック ${count} 件`,
      addedCount: (count: number) => `追加 ${count}`,
      retainedCount: (count: number) => `保持 ${count}`,
      replacedCount: (count: number) => `置換 ${count}`,
      staleCount: (count: number) => `失効 ${count}`,
      actionCount: (count: number) => `アクション ${count}`,
      blockedNote: (count: number) =>
        `${count} 件がバージョンまたはターゲットの競合によりブロックされました。続行時はこれらをスキップします。`,
      itemsAria: "バッチプランの項目別詳細",
      itemsTitle: "項目別プラン",
      displayRevision: "表示バージョン",
      layerLabel: "レイヤー",
      installedRevision: "インストール済みバージョン",
      candidateDisplayRevision: "候補の表示バージョン",
      targetLabel: "ターゲット",
      switchTo: (targetId: string) => `${targetId} へ切替`,
      keepCurrent: "現在のターゲットを維持",
      closeAria: "閉じる",
      generating: "バッチプランを生成中…",
      targetSelectionAria: "一括再インストールのターゲット選択",
      targetSelectionTitle: "切り替える外観ターゲットを選択",
      targetSelectionHint: "同一バージョンの再インストールでは、リターゲット可能な各 Mod に現在と異なるターゲットを選ぶ必要があります。",
      targetUnavailable: "この Mod には利用可能なターゲット切替オプションがなく、今回の同一バージョン再インストールに参加できません。",
      targetGroupAria: (modId: string) => `${modId} の置換ターゲット`,
      excludedTitle: "今回の操作の対象外の項目",
      excludedItem: (modId: string, reason: string) => `${modId}：${reason}`,
      unresolvableAria: "解決できない項目",
      unresolvableTitle: "バージョンを解決できない項目",
      unresolvableItem: (modId: string) => `${modId}：バージョン情報を読み取れません`,
      policyTitle: "実行ポリシー",
      stopOnFailure: "失敗したら停止（推奨）",
      continueOnFailure: "失敗した項目をスキップして続行",
      cancel: "キャンセル",
      generatePreview: "バッチプレビューを生成",
      confirmStart: "確定して開始",
    },
    resultPanel: {
      resultTitle: (operationLabel: string) => `${operationLabel}の結果`,
      closeAria: "閉じる",
      batchIdLabel: (batchId: string) => `バッチ ${batchId}`,
      summaryAria: "バッチ結果の概要",
      succeededCount: (count: number) => `成功 ${count}`,
      failedCount: (count: number) => `失敗 ${count}`,
      blockedCount: (count: number) => `ブロック ${count}`,
      skippedCount: (count: number) => `スキップ ${count}`,
      cancelledCount: (count: number) => `キャンセル ${count}`,
      recoveryRequiredCount: (count: number) => `要復旧 ${count}`,
      evidenceDegraded: "一部の実行証跡の健全性が低下しています。復旧センターで確認してください。",
      itemsAria: "項目別の結果",
      retryableBadge: "再試行可",
      close: "閉じる",
      loadMore: "さらに読み込む",
      retryFailed: "失敗した項目を再試行",
    },
    runningPanel: {
      running: {
        install: "一括インストールを実行中…",
        uninstall: "一括アンインストールを実行中…",
        reinstall: "一括再インストールを実行中…",
      },
    },
  },
} satisfies LocaleDictionary<BatchModLifecycleCopy>;

export function getBatchCapabilityUnavailableLabel(
  capability: BatchModLifecycleCapabilityDto | null,
  copy: BatchModLifecycleCopy["capability"],
): string {
  if (capability === null) {
    return copy.loading;
  }
  switch (capability.unavailableReasonCode) {
    case "sandbox_batch_production_forbidden":
      return copy.sandboxForbidden;
    case "batch_capability_unavailable":
      return copy.unavailable;
    case null:
      return copy.nullReason;
    default:
      return copy.unsupported;
  }
}

export function getBatchOperationLabel(
  operation: BatchModLifecycleOperation,
  operations: BatchModLifecycleCopy["operations"],
): string {
  return operations[operation] ?? operations.install;
}

export function getBatchItemStatusLabel(
  status: BatchModLifecycleItemStatus,
  itemStatus: BatchModLifecycleCopy["itemStatus"],
): string {
  return itemStatus[status] ?? status;
}

export function getBatchAttemptStatusLabel(
  status: BatchModLifecycleAttemptStatus,
  attemptStatus: BatchModLifecycleCopy["attemptStatus"],
): string {
  return attemptStatus[status] ?? status;
}

export function getBatchExcludedReasonLabel(
  reason: string,
  copy: Pick<BatchModLifecycleCopy, "excludedReasons" | "excludedReasonFallback">,
): string {
  return copy.excludedReasons[reason] ?? copy.excludedReasonFallback;
}

export function getBatchReasonCodeLabel(
  code: string,
  reasonCodes: BatchModLifecycleCopy["reasonCodes"],
): string {
  return reasonCodes[code] ?? code;
}

export function getBatchErrorLabel(
  code: string,
  copy: Pick<BatchModLifecycleCopy, "errors" | "errorFallback">,
): string {
  return copy.errors[code] ?? copy.errorFallback;
}
