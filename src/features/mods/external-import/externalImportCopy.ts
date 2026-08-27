import type { LocaleDictionary } from "../../../shared/i18n";

// 外部导入（第三方管理器迁移）全部用户可见文案（I18N-02）。
// code→文本 的映射表原样收敛到这里（key 与后端事件/错误码一致）；
// 语义判断（阶段推进、可重试识别、终态归类）留在各 state 模型，模型函数只收字典。

export type ExternalImportCopy = {
  scan: {
    phases: Record<string, string>;
    scanning: string;
    errors: Record<string, string>;
    fallbackError: string;
  };
  progress: {
    phases: Record<string, string>;
    unrecognized: string;
  };
  result: {
    status: Record<string, string>;
    reasons: Record<string, string>;
    unknownReason: string;
    retryable: string;
    batchStatus: Record<string, string>;
    unknownBatchStatus: string;
    errors: Record<string, string>;
    fallbackError: string;
  };
  selection: {
    errors: Record<string, string>;
    fallbackError: string;
  };
  preview: {
    status: Record<string, string>;
    rescan: string;
    conflicts: Record<string, string>;
    needsReview: string;
    unnamed: string;
    fileCount: (count: string) => string;
  };
  action: {
    trigger: string;
    dialogTitle: string;
    dialogFallbackDescription: string;
    scanningToastTitle: string;
    scanFailedToastTitle: string;
    scanCancelledToastTitle: string;
    scanCancelledToastMessage: string;
    cancelScanFailedTitle: string;
    choosingSource: string;
    creatingScanTask: string;
    cancelling: string;
    cancelScan: string;
    retryListener: string;
    chooseSource: string;
    /** 常驻底栏的主操作:带上已选数量,玩家不必滚回列表顶部确认选了多少。 */
    startImportWithCount: (count: string) => string;
    sourceEyebrow: string;
    sourceNotChosen: string;
    connectingScanStatus: string;
    scanCancelled: string;
  };
  candidate: {
    keepBothHint: string;
    ignoreInvalidHint: string;
    savingSelection: string;
    conflictLabel: string;
    conflictPlaceholder: string;
    categoryLabel: string;
    categoryNone: string;
    unselectable: string;
  };
  selectionPanel: {
    sealing: string;
    progressCount: (current: string, total: string) => string;
    cancelPending: string;
    cancelImport: string;
    cancellingSafely: string;
    completedReadingResults: string;
    viewImportHistory: string;
    closeAndReturn: string;
    cancelledNoInference: string;
    incompleteReadingResults: string;
    candidateEyebrow: string;
    selectedCount: (count: string) => string;
    creatingSnapshot: string;
    editable: string;
    sealed: string;
    expired: string;
    progressListenerUnavailable: string;
    retryProgressListener: string;
    reloadCategories: string;
    loadingSnapshot: string;
    reloadCandidates: string;
    noCandidates: string;
    noImportableHint: string;
    scanReturned: (total: string) => string;
    loadedCount: (loaded: string, total: string) => string;
    selectAllImportable: string;
    deselectLoaded: string;
    loadingMore: string;
    loadMoreCandidates: string;
    importOnlyTitle: string;
    importOnlyDescription: string;
    startBatchImport: string;
  };
  resultPanel: {
    title: string;
    readingDetails: string;
    reloadResults: string;
    resultEyebrow: string;
    loadedCount: (loaded: string, total: string) => string;
    emptyBatch: string;
    retryingHint: string;
    noDetailsTitle: string;
    noDetailsBody: string;
    summaryAria: string;
    imported: (count: string) => string;
    alreadyImported: (count: string) => string;
    skipped: (count: string) => string;
    blocked: (count: string) => string;
    failed: (count: string) => string;
    cancelled: (count: string) => string;
    modId: (id: string) => string;
    retryableBadge: string;
    loadingMore: string;
    loadMoreResults: string;
    creatingRetryTask: string;
    retryRecoverable: string;
  };
  workflow: {
    resultsLoadedTitle: string;
    resultsLoadedMessage: string;
    refreshFailedTitle: string;
    refreshFailedMessage: string;
    importingTitle: string;
    importingProgress: (current: string, total: string) => string;
    importCompletedTitle: string;
    importCompletedMessage: string;
    importCancelledTitle: string;
    importCancelledMessage: string;
    importIncompleteTitle: string;
    cancelImportFailedTitle: string;
  };
  history: {
    tabCurrent: string;
    tabHistory: string;
    tablistAria: string;
    historyTriggerAria: string;
    title: string;
    loading: string;
    loadedCount: (loaded: string, total: string) => string;
    emptyTitle: string;
    emptyHint: string;
    adapters: Record<string, string>;
    unknownAdapter: string;
    states: {
      running: string;
      scanning: string;
      scanOnly: string;
      scanFailed: string;
      completed: string;
      completedWithErrors: string;
      incomplete: string;
      cancelled: string;
    };
    candidateCount: (count: string) => string;
    noResults: string;
    viewDetails: string;
    hideDetails: string;
    detailLoading: string;
    reloadDetail: string;
    loadMoreDetails: string;
    loadingMore: string;
    loadMore: string;
    reload: string;
    retryHint: string;
    retentionHint: string;
    time: {
      justNow: string;
      minutesAgo: (count: string) => string;
      hoursAgo: (count: string) => string;
      daysAgo: (count: string) => string;
    };
    errors: Record<string, string>;
    fallbackError: string;
  };
};

export const externalImportCopy = {
  zh_cn: {
    scan: {
      phases: {
        "external_import.scan.queued": "等待只读扫描",
        "external_import.scan.discovering": "正在发现候选",
        "external_import.scan.fingerprinting": "正在分析候选",
        "external_import.scan.completed": "扫描完成",
        "external_import.scan.failed": "扫描失败",
        "external_import.scan.cancelled": "扫描已取消",
        "mod_import.cancelled": "正在取消扫描",
      },
      scanning: "正在扫描",
      errors: {
        external_import_source_picker_unavailable: "无法打开来源选择器",
        external_import_source_unavailable: "来源不可用，请重新选择",
        external_import_source_id_invalid: "来源标识无效，请重新选择",
        external_import_task_unavailable: "扫描任务不可用，请重新选择来源",
        external_import_batch_unavailable: "扫描预览不可用，请重新扫描",
        external_import_scan_failed: "扫描未完成，请重新选择来源后重试",
        external_import_clock_unavailable: "扫描状态不可用，请稍后重试",
        external_import_preview_cursor_invalid: "预览页状态无效，请重新扫描",
        external_import_preview_limit_invalid: "预览请求无效，请重新扫描",
        external_import_progress_unrecognized: "扫描状态不可识别，已停止继续操作",
        external_import_preview_invalid: "预览数据不可识别，请重新扫描",
        external_import_listener_unavailable: "扫描状态监听不可用，请重试",
      },
      fallbackError: "扫描未完成，请重新选择来源后重试",
    },
    progress: {
      phases: {
        "external_import.import.queued": "等待批量导入",
        "external_import.import.materializing": "正在重新校验并物化候选",
        "external_import.import.preparing": "正在分析内部导入包",
        "external_import.import.persisting": "正在保存 Mod 目录事实",
        "external_import.import.completed": "批量导入完成",
        "external_import.import.failed": "批量导入失败",
        "external_import.import.cancelled": "批量导入已取消",
        "mod_import.cancelled": "正在安全取消导入",
      },
      unrecognized: "导入状态不可识别",
    },
    result: {
      status: {
        imported: "已导入",
        already_imported: "已存在",
        skipped: "已跳过",
        blocked: "已阻断",
        failed: "导入失败",
        cancelled: "已取消",
      },
      reasons: {
        already_imported: "内容已存在",
        duplicate_in_batch: "批次内重复",
        name_collision: "名称冲突",
        structure_invalid: "目录结构不可用",
        metadata_invalid: "元数据不可用",
        unsupported_entry: "包含不支持的条目",
        resource_limit_exceeded: "超出资源限制",
        source_unreadable: "来源不可读取",
        source_changed: "来源已变化",
        selection_revision_conflict: "选择版本已变化",
        selection_empty: "选择为空",
        selection_mutation_empty: "没有选择变更",
        selection_mutation_limit_exceeded: "选择变更超限",
        selection_total_limit_exceeded: "选择总数超限",
        selection_resource_limit_exceeded: "选择资源超限",
        selection_candidate_invalid: "候选状态已变化",
        selection_expired: "选择已过期",
        selection_closed: "选择已封存",
        selection_revision_overflow: "选择版本不可用",
      },
      unknownReason: "结果原因不可识别",
      retryable: "可重试",
      batchStatus: {
        completed: "全部完成",
        completed_with_errors: "部分完成",
        failed: "任务失败，已保留结果",
        cancelled: "任务已取消，已保留结果",
      },
      unknownBatchStatus: "批次状态不可识别",
      errors: {
        external_import_batch_unavailable: "批量导入结果不可用，请重新扫描",
        external_import_batch_not_startable: "当前批次没有可重试项",
        external_import_result_cursor_invalid: "结果分页位置不可用，请重新载入",
        external_import_result_limit_invalid: "结果分页大小不可用，请重新载入",
        external_import_result_request_invalid: "结果请求不可用，请重新载入",
        external_import_selection_unavailable: "已封存的选择不可用，请重新扫描",
        external_import_source_unavailable: "来源已失效，请重新选择来源后重试",
        external_import_task_unavailable: "导入任务不可用，请稍后重试",
        external_import_result_invalid: "批量导入结果不可识别，请重新扫描",
      },
      fallbackError: "无法读取批量导入结果，请稍后重试",
    },
    selection: {
      errors: {
        external_import_selection_unavailable: "候选选择不可用，请重新扫描",
        external_import_batch_unavailable: "导入批次不可用，请重新扫描",
        external_import_batch_not_startable: "当前批次不能启动，请重新扫描",
        external_import_catalog_unavailable: "Mod 目录暂时不可用，请稍后重试",
        external_import_category_unavailable: "分类不可用，请重新载入分类",
        external_import_clock_unavailable: "选择状态不可用，请稍后重试",
        selection_revision_conflict: "选择已发生变化，已重新载入",
        selection_empty: "请至少选择一个候选",
        selection_mutation_empty: "没有需要更新的候选",
        selection_mutation_limit_exceeded: "本次选择变更过多，请分批操作",
        selection_total_limit_exceeded: "选择数量超出批次限制",
        selection_resource_limit_exceeded: "选择内容超出资源限制",
        selection_candidate_invalid: "候选状态已变化，请重新载入",
        selection_expired: "选择已过期，请重新扫描",
        selection_closed: "选择已封存，不能继续修改",
        external_import_selection_invalid: "选择数据不可识别，请重新扫描",
        external_import_task_unavailable: "导入任务不可用，请重试",
        external_import_progress_unrecognized: "导入状态不可识别，已停止继续操作",
      },
      fallbackError: "无法更新候选选择，请重新载入后重试",
    },
    preview: {
      status: {
        ready: "可导入",
        already_imported: "已存在",
        duplicate_in_batch: "批次重复",
        name_collision: "名称冲突",
        structure_invalid: "结构不可用",
        metadata_invalid: "元数据不可用",
        unsupported_entry: "不支持的条目",
        resource_limit_exceeded: "超出资源限制",
        source_unreadable: "来源不可读取",
        payload_missing: "载荷不在盒子库中",
      },
      rescan: "需要重新扫描",
      conflicts: {
        content_duplicate: "内容重复",
        name_collision: "同名冲突",
      },
      needsReview: "需要复核",
      unnamed: "未命名候选",
      fileCount: (count) => `${count} 个文件`,
    },
    action: {
      trigger: "迁移第三方 Mod",
      dialogTitle: "第三方 Mod 迁移",
      dialogFallbackDescription: "只读扫描与候选预览",
      scanningToastTitle: "正在扫描第三方来源",
      scanFailedToastTitle: "第三方来源扫描失败",
      scanCancelledToastTitle: "第三方来源扫描已取消",
      scanCancelledToastMessage: "未创建可导入选择。",
      cancelScanFailedTitle: "无法取消扫描",
      choosingSource: "正在选择来源",
      creatingScanTask: "正在创建扫描任务",
      cancelling: "正在取消",
      cancelScan: "取消扫描",
      retryListener: "重试状态监听",
      chooseSource: "选择来源",
      startImportWithCount: (count) => `开始导入 ${count} 项`,
      sourceEyebrow: "来源",
      sourceNotChosen: "尚未选择",
      connectingScanStatus: "正在连接扫描状态",
      scanCancelled: "扫描已取消",
    },
    candidate: {
      keepBothHint: "保留两者并创建新的 Mod",
      ignoreInvalidHint: "忽略无效元数据并继续导入",
      savingSelection: "正在保存候选选择",
      conflictLabel: "冲突处理",
      conflictPlaceholder: "请选择明确处理方式",
      categoryLabel: "导入分类",
      categoryNone: "不分配分类",
      unselectable: "此候选由后端标记为不可选择，需要重新扫描或处理来源问题。",
    },
    selectionPanel: {
      sealing: "正在封存选择并创建批量导入任务",
      progressCount: (current, total) => `（${current} / ${total}）`,
      cancelPending: "正在请求取消",
      cancelImport: "取消导入",
      cancellingSafely: "正在安全取消；等待批量导入专用终态",
      completedReadingResults: "批量导入已完成。正在读取下方的权威结果明细。",
      viewImportHistory: "查看导入记录",
      closeAndReturn: "关闭并回到 Mod 库",
      cancelledNoInference: "批量导入已取消；本页面不会根据聚合计数推断部分成功结果。",
      incompleteReadingResults: "批量导入未完成。正在读取已保留结果与可恢复操作。",
      candidateEyebrow: "候选选择",
      selectedCount: (count) => `已选择 ${count} 项`,
      creatingSnapshot: "正在创建选择快照",
      editable: "可编辑",
      sealed: "已封存",
      expired: "已过期",
      progressListenerUnavailable: "无法监听批量导入进度，启动操作已禁用。",
      retryProgressListener: "重试进度监听",
      reloadCategories: "重新加载分类",
      loadingSnapshot: "正在读取选择快照与候选预览",
      reloadCandidates: "重新加载候选",
      noCandidates: "没有可显示的候选",
      noImportableHint:
        "没有可导入项。请选择直接包含数字编号 Mod 文件夹的目录(狩技盒子通常是安装目录下的 Mods_582010);标注「载荷不在盒子库中」的 Mod 需要用原始压缩包重新导入。",
      scanReturned: (total) => `扫描共返回 ${total} 项。`,
      loadedCount: (loaded, total) => `已加载 ${loaded} / ${total} 项`,
      selectAllImportable: "全选可导入",
      deselectLoaded: "取消选择",
      loadingMore: "正在载入",
      loadMoreCandidates: "载入更多候选",
      importOnlyTitle: "仅导入到 HMM Mod 库",
      importOnlyDescription: "不会安装、启用或写入游戏目录。",
      startBatchImport: "开始批量导入",
    },
    resultPanel: {
      title: "批量导入结果",
      readingDetails: "正在读取服务端确认的结果明细",
      reloadResults: "重新读取结果",
      resultEyebrow: "导入结果",
      loadedCount: (loaded, total) => `已载入 ${loaded} / ${total} 项`,
      emptyBatch: "当前批次没有结果项",
      retryingHint: "正在重试可恢复项；下方是上一次权威结果，任务结束后会自动刷新。",
      noDetailsTitle: "没有结果明细",
      noDetailsBody: "批次状态已由后端确认，没有可分页的候选结果。",
      summaryAria: "当前已载入结果汇总",
      imported: (count) => `已导入 ${count}`,
      alreadyImported: (count) => `已存在 ${count}`,
      skipped: (count) => `已跳过 ${count}`,
      blocked: (count) => `已阻断 ${count}`,
      failed: (count) => `失败 ${count}`,
      cancelled: (count) => `取消 ${count}`,
      modId: (id) => `Mod ID：${id}`,
      retryableBadge: "可重试",
      loadingMore: "正在载入",
      loadMoreResults: "载入更多结果",
      creatingRetryTask: "正在创建重试任务",
      retryRecoverable: "重试可恢复项",
    },
    workflow: {
      resultsLoadedTitle: "批量导入结果已载入",
      resultsLoadedMessage: "Mod 列表与服务端确认的结果明细已刷新。",
      refreshFailedTitle: "结果已载入，Mod 列表刷新失败",
      refreshFailedMessage: "导入事实不受影响，请稍后手动刷新 Mod 列表。",
      importingTitle: "正在批量导入 Mod",
      importingProgress: (current, total) => `（${current} / ${total}）`,
      importCompletedTitle: "批量导入已完成",
      importCompletedMessage: "正在读取服务端确认的结果明细。",
      importCancelledTitle: "批量导入已取消",
      importCancelledMessage: "正在读取已保留的权威结果。",
      importIncompleteTitle: "批量导入未完成",
      cancelImportFailedTitle: "无法取消批量导入",
    },
    history: {
      tabCurrent: "本次导入",
      tabHistory: "导入记录",
      tablistAria: "迁移第三方 Mod 视图切换",
      historyTriggerAria: "查看第三方导入记录",
      title: "导入记录",
      loading: "正在读取导入记录…",
      loadedCount: (loaded: string, total: string) => `已载入 ${loaded} / ${total} 个批次`,
      emptyTitle: "还没有导入记录",
      emptyHint: "从狩技盒子完成一次迁移后，批次记录会显示在这里。",
      adapters: {
        hunting_box_directory_v1: "狩技盒子",
      },
      unknownAdapter: "未知来源",
      states: {
        running: "导入进行中",
        scanning: "扫描中",
        scanOnly: "仅扫描未导入",
        scanFailed: "扫描失败",
        completed: "已完成",
        completedWithErrors: "部分完成",
        incomplete: "未完成",
        cancelled: "已取消",
      },
      candidateCount: (count: string) => `候选 ${count} 项`,
      noResults: "该批次没有结果明细",
      viewDetails: "展开明细",
      hideDetails: "收起明细",
      detailLoading: "正在读取批次明细…",
      reloadDetail: "重新读取明细",
      loadMoreDetails: "加载更多明细",
      loadingMore: "正在加载…",
      loadMore: "加载更多记录",
      reload: "重新读取记录",
      retryHint: "历史批次不支持直接重试；如需重试，请重新选择来源目录发起导入。",
      retentionHint: "仅保留最近 50 个已导入批次；仅扫描未导入的批次保留 10 个且不超过 7 天。",
      time: {
        justNow: "刚刚",
        minutesAgo: (count: string) => `${count} 分钟前`,
        hoursAgo: (count: string) => `${count} 小时前`,
        daysAgo: (count: string) => `${count} 天前`,
      },
      errors: {
        external_import_history_cursor_invalid: "导入记录分页游标无效，请重新读取",
        external_import_history_limit_invalid: "导入记录分页大小无效，请重新读取",
        external_import_history_request_invalid: "导入记录请求无效，请重新读取",
        external_import_batch_unavailable: "导入记录暂时不可用，请稍后重试",
        external_import_history_invalid: "导入记录数据校验未通过，请重新读取",
        external_import_result_invalid: "批次明细数据校验未通过，请重新读取",
      },
      fallbackError: "无法读取导入记录，请稍后重试",
    },
  },
  en: {
    scan: {
      phases: {
        "external_import.scan.queued": "Waiting for read-only scan",
        "external_import.scan.discovering": "Discovering candidates",
        "external_import.scan.fingerprinting": "Analyzing candidates",
        "external_import.scan.completed": "Scan finished",
        "external_import.scan.failed": "Scan failed",
        "external_import.scan.cancelled": "Scan cancelled",
        "mod_import.cancelled": "Cancelling the scan",
      },
      scanning: "Scanning",
      errors: {
        external_import_source_picker_unavailable: "Cannot open the source picker",
        external_import_source_unavailable: "The source is unavailable; choose it again",
        external_import_source_id_invalid: "Invalid source identifier; choose it again",
        external_import_task_unavailable: "The scan task is unavailable; choose the source again",
        external_import_batch_unavailable: "The scan preview is unavailable; rescan",
        external_import_scan_failed: "The scan did not finish; choose the source again and retry",
        external_import_clock_unavailable: "Scan status is unavailable; try again later",
        external_import_preview_cursor_invalid: "The preview page state is invalid; rescan",
        external_import_preview_limit_invalid: "The preview request is invalid; rescan",
        external_import_progress_unrecognized: "Unrecognized scan status; further actions stopped",
        external_import_preview_invalid: "Unrecognized preview data; rescan",
        external_import_listener_unavailable: "The scan status listener is unavailable; retry",
      },
      fallbackError: "The scan did not finish; choose the source again and retry",
    },
    progress: {
      phases: {
        "external_import.import.queued": "Waiting for batch import",
        "external_import.import.materializing": "Revalidating and materializing candidates",
        "external_import.import.preparing": "Analyzing the internal import package",
        "external_import.import.persisting": "Saving mod catalog facts",
        "external_import.import.completed": "Batch import finished",
        "external_import.import.failed": "Batch import failed",
        "external_import.import.cancelled": "Batch import cancelled",
        "mod_import.cancelled": "Safely cancelling the import",
      },
      unrecognized: "Unrecognized import status",
    },
    result: {
      status: {
        imported: "Imported",
        already_imported: "Already present",
        skipped: "Skipped",
        blocked: "Blocked",
        failed: "Import failed",
        cancelled: "Cancelled",
      },
      reasons: {
        already_imported: "Content already exists",
        duplicate_in_batch: "Duplicate within the batch",
        name_collision: "Name collision",
        structure_invalid: "Directory structure unusable",
        metadata_invalid: "Metadata unusable",
        unsupported_entry: "Contains unsupported entries",
        resource_limit_exceeded: "Resource limit exceeded",
        source_unreadable: "Source unreadable",
        source_changed: "Source changed",
        selection_revision_conflict: "Selection revision changed",
        selection_empty: "Selection is empty",
        selection_mutation_empty: "No selection changes",
        selection_mutation_limit_exceeded: "Too many selection changes",
        selection_total_limit_exceeded: "Selection total limit exceeded",
        selection_resource_limit_exceeded: "Selection resource limit exceeded",
        selection_candidate_invalid: "Candidate state changed",
        selection_expired: "Selection expired",
        selection_closed: "Selection sealed",
        selection_revision_overflow: "Selection revision unavailable",
      },
      unknownReason: "Unrecognized result reason",
      retryable: "Retryable",
      batchStatus: {
        completed: "All finished",
        completed_with_errors: "Partially finished",
        failed: "Task failed; results kept",
        cancelled: "Task cancelled; results kept",
      },
      unknownBatchStatus: "Unrecognized batch status",
      errors: {
        external_import_batch_unavailable: "Batch import results are unavailable; rescan",
        external_import_batch_not_startable: "This batch has no retryable items",
        external_import_result_cursor_invalid: "The result page position is unavailable; reload",
        external_import_result_limit_invalid: "The result page size is unavailable; reload",
        external_import_result_request_invalid: "The result request is unavailable; reload",
        external_import_selection_unavailable: "The sealed selection is unavailable; rescan",
        external_import_source_unavailable: "The source is no longer valid; choose it again and retry",
        external_import_task_unavailable: "The import task is unavailable; try again later",
        external_import_result_invalid: "Unrecognized batch import results; rescan",
      },
      fallbackError: "Cannot read the batch import results; try again later",
    },
    selection: {
      errors: {
        external_import_selection_unavailable: "The candidate selection is unavailable; rescan",
        external_import_batch_unavailable: "The import batch is unavailable; rescan",
        external_import_batch_not_startable: "This batch cannot start; rescan",
        external_import_catalog_unavailable: "The mod catalog is temporarily unavailable; try again later",
        external_import_category_unavailable: "Categories are unavailable; reload categories",
        external_import_clock_unavailable: "Selection status is unavailable; try again later",
        selection_revision_conflict: "The selection changed and was reloaded",
        selection_empty: "Select at least one candidate",
        selection_mutation_empty: "No candidates need updating",
        selection_mutation_limit_exceeded: "Too many selection changes at once; work in smaller batches",
        selection_total_limit_exceeded: "The selection exceeds the batch limit",
        selection_resource_limit_exceeded: "The selection exceeds the resource limit",
        selection_candidate_invalid: "Candidate state changed; reload",
        selection_expired: "The selection expired; rescan",
        selection_closed: "The selection is sealed and can no longer change",
        external_import_selection_invalid: "Unrecognized selection data; rescan",
        external_import_task_unavailable: "The import task is unavailable; retry",
        external_import_progress_unrecognized: "Unrecognized import status; further actions stopped",
      },
      fallbackError: "Cannot update the candidate selection; reload and retry",
    },
    preview: {
      status: {
        ready: "Importable",
        already_imported: "Already present",
        duplicate_in_batch: "Duplicate in batch",
        name_collision: "Name collision",
        structure_invalid: "Structure unusable",
        metadata_invalid: "Metadata unusable",
        unsupported_entry: "Unsupported entry",
        resource_limit_exceeded: "Resource limit exceeded",
        source_unreadable: "Source unreadable",
        payload_missing: "Payload not in the box library",
      },
      rescan: "Rescan required",
      conflicts: {
        content_duplicate: "Duplicate content",
        name_collision: "Same-name conflict",
      },
      needsReview: "Needs review",
      unnamed: "Unnamed candidate",
      fileCount: (count) => `${count} files`,
    },
    action: {
      trigger: "Migrate third-party mods",
      dialogTitle: "Third-party mod migration",
      dialogFallbackDescription: "Read-only scan and candidate preview",
      scanningToastTitle: "Scanning the third-party source",
      scanFailedToastTitle: "Third-party source scan failed",
      scanCancelledToastTitle: "Third-party source scan cancelled",
      scanCancelledToastMessage: "No importable selection was created.",
      cancelScanFailedTitle: "Cannot cancel the scan",
      choosingSource: "Choosing a source",
      creatingScanTask: "Creating the scan task",
      cancelling: "Cancelling",
      cancelScan: "Cancel scan",
      retryListener: "Retry status listener",
      chooseSource: "Choose source",
      startImportWithCount: (count) => `Import ${count} item(s)`,
      sourceEyebrow: "Source",
      sourceNotChosen: "Not chosen yet",
      connectingScanStatus: "Connecting to scan status",
      scanCancelled: "Scan cancelled",
    },
    candidate: {
      keepBothHint: "Keep both and create a new mod",
      ignoreInvalidHint: "Ignore the invalid metadata and continue importing",
      savingSelection: "Saving the candidate selection",
      conflictLabel: "Conflict handling",
      conflictPlaceholder: "Choose an explicit resolution",
      categoryLabel: "Import category",
      categoryNone: "No category",
      unselectable:
        "The backend marked this candidate as unselectable; rescan or resolve the source issue.",
    },
    selectionPanel: {
      sealing: "Sealing the selection and creating the batch import task",
      progressCount: (current, total) => `(${current} / ${total})`,
      cancelPending: "Requesting cancellation",
      cancelImport: "Cancel import",
      cancellingSafely: "Cancelling safely; waiting for the batch import's own terminal state",
      completedReadingResults:
        "Batch import finished. Reading the authoritative result details below.",
      viewImportHistory: "View import history",
      closeAndReturn: "Close and return to the mod library",
      cancelledNoInference:
        "Batch import cancelled; this page never infers partial success from aggregate counts.",
      incompleteReadingResults:
        "Batch import did not finish. Reading the kept results and recoverable actions.",
      candidateEyebrow: "Candidate selection",
      selectedCount: (count) => `${count} selected`,
      creatingSnapshot: "Creating the selection snapshot",
      editable: "Editable",
      sealed: "Sealed",
      expired: "Expired",
      progressListenerUnavailable:
        "Cannot listen to batch import progress; starting is disabled.",
      retryProgressListener: "Retry progress listener",
      reloadCategories: "Reload categories",
      loadingSnapshot: "Reading the selection snapshot and candidate preview",
      reloadCandidates: "Reload candidates",
      noCandidates: "No candidates to show",
      noImportableHint:
        "Nothing can be imported. Pick the folder that directly contains the numbered mod folders (usually Mods_582010 under the hunting box install directory); mods marked \"payload not in the box library\" must be re-imported from their original archives.",
      scanReturned: (total) => `The scan returned ${total} items.`,
      loadedCount: (loaded, total) => `Loaded ${loaded} / ${total}`,
      selectAllImportable: "Select all importable",
      deselectLoaded: "Clear selection",
      loadingMore: "Loading",
      loadMoreCandidates: "Load more candidates",
      importOnlyTitle: "Imports into the HMM mod library only",
      importOnlyDescription: "Nothing is installed, enabled, or written to the game directory.",
      startBatchImport: "Start batch import",
    },
    resultPanel: {
      title: "Batch import results",
      readingDetails: "Reading the server-confirmed result details",
      reloadResults: "Reload results",
      resultEyebrow: "Import results",
      loadedCount: (loaded, total) => `Loaded ${loaded} / ${total}`,
      emptyBatch: "This batch has no result items",
      retryingHint:
        "Retrying recoverable items; below is the previous authoritative result, refreshed when the task ends.",
      noDetailsTitle: "No result details",
      noDetailsBody: "The batch status is backend-confirmed with no pageable candidate results.",
      summaryAria: "Summary of loaded results",
      imported: (count) => `Imported ${count}`,
      alreadyImported: (count) => `Already present ${count}`,
      skipped: (count) => `Skipped ${count}`,
      blocked: (count) => `Blocked ${count}`,
      failed: (count) => `Failed ${count}`,
      cancelled: (count) => `Cancelled ${count}`,
      modId: (id) => `Mod ID: ${id}`,
      retryableBadge: "Retryable",
      loadingMore: "Loading",
      loadMoreResults: "Load more results",
      creatingRetryTask: "Creating the retry task",
      retryRecoverable: "Retry recoverable items",
    },
    workflow: {
      resultsLoadedTitle: "Batch import results loaded",
      resultsLoadedMessage: "The mod list and server-confirmed result details are refreshed.",
      refreshFailedTitle: "Results loaded, but the mod list failed to refresh",
      refreshFailedMessage: "Import facts are unaffected; refresh the mod list manually later.",
      importingTitle: "Batch importing mods",
      importingProgress: (current, total) => `(${current} / ${total})`,
      importCompletedTitle: "Batch import finished",
      importCompletedMessage: "Reading the server-confirmed result details.",
      importCancelledTitle: "Batch import cancelled",
      importCancelledMessage: "Reading the kept authoritative results.",
      importIncompleteTitle: "Batch import incomplete",
      cancelImportFailedTitle: "Cannot cancel the batch import",
    },
    history: {
      tabCurrent: "Current import",
      tabHistory: "Import history",
      tablistAria: "Third-party mod migration views",
      historyTriggerAria: "View third-party import history",
      title: "Import history",
      loading: "Reading import history…",
      loadedCount: (loaded: string, total: string) => `Loaded ${loaded} / ${total} batches`,
      emptyTitle: "No import history yet",
      emptyHint: "Batch records appear here after a migration from the hunting box.",
      adapters: {
        hunting_box_directory_v1: "Hunting box",
      },
      unknownAdapter: "Unknown source",
      states: {
        running: "Import running",
        scanning: "Scanning",
        scanOnly: "Scanned, not imported",
        scanFailed: "Scan failed",
        completed: "Completed",
        completedWithErrors: "Partially completed",
        incomplete: "Incomplete",
        cancelled: "Cancelled",
      },
      candidateCount: (count: string) => `${count} candidates`,
      noResults: "This batch has no result details",
      viewDetails: "Show details",
      hideDetails: "Hide details",
      detailLoading: "Reading batch details…",
      reloadDetail: "Reload details",
      loadMoreDetails: "Load more details",
      loadingMore: "Loading…",
      loadMore: "Load more batches",
      reload: "Reload history",
      retryHint:
        "History batches cannot be retried directly; reselect the source directory to start a new import.",
      retentionHint:
        "Only the latest 50 imported batches are kept; scan-only batches keep 10 for up to 7 days.",
      time: {
        justNow: "Just now",
        minutesAgo: (count: string) => `${count} min ago`,
        hoursAgo: (count: string) => `${count} h ago`,
        daysAgo: (count: string) => `${count} d ago`,
      },
      errors: {
        external_import_history_cursor_invalid: "The history cursor is invalid; reload the list",
        external_import_history_limit_invalid: "The history page size is invalid; reload the list",
        external_import_history_request_invalid: "The history request is invalid; reload the list",
        external_import_batch_unavailable: "Import history is unavailable; try again later",
        external_import_history_invalid: "History data failed validation; reload the list",
        external_import_result_invalid: "Batch details failed validation; reload the details",
      },
      fallbackError: "Cannot read the import history; try again later",
    },
  },
  ja: {
    scan: {
      phases: {
        "external_import.scan.queued": "読み取り専用スキャンを待機中",
        "external_import.scan.discovering": "候補を検出中",
        "external_import.scan.fingerprinting": "候補を解析中",
        "external_import.scan.completed": "スキャン完了",
        "external_import.scan.failed": "スキャン失敗",
        "external_import.scan.cancelled": "スキャンをキャンセルしました",
        "mod_import.cancelled": "スキャンをキャンセル中",
      },
      scanning: "スキャン中",
      errors: {
        external_import_source_picker_unavailable: "ソース選択ダイアログを開けません",
        external_import_source_unavailable: "ソースを利用できません。選び直してください",
        external_import_source_id_invalid: "ソース ID が無効です。選び直してください",
        external_import_task_unavailable: "スキャンタスクを利用できません。ソースを選び直してください",
        external_import_batch_unavailable: "スキャンプレビューを利用できません。再スキャンしてください",
        external_import_scan_failed: "スキャンが完了しませんでした。ソースを選び直して再試行してください",
        external_import_clock_unavailable: "スキャン状態を利用できません。しばらくして再試行してください",
        external_import_preview_cursor_invalid: "プレビューページの状態が無効です。再スキャンしてください",
        external_import_preview_limit_invalid: "プレビュー要求が無効です。再スキャンしてください",
        external_import_progress_unrecognized: "スキャン状態を認識できないため操作を停止しました",
        external_import_preview_invalid: "プレビューデータを認識できません。再スキャンしてください",
        external_import_listener_unavailable: "スキャン状態リスナーを利用できません。再試行してください",
      },
      fallbackError: "スキャンが完了しませんでした。ソースを選び直して再試行してください",
    },
    progress: {
      phases: {
        "external_import.import.queued": "一括インポートを待機中",
        "external_import.import.materializing": "候補を再検証して実体化中",
        "external_import.import.preparing": "内部インポートパッケージを解析中",
        "external_import.import.persisting": "Mod カタログ情報を保存中",
        "external_import.import.completed": "一括インポート完了",
        "external_import.import.failed": "一括インポート失敗",
        "external_import.import.cancelled": "一括インポートをキャンセルしました",
        "mod_import.cancelled": "インポートを安全にキャンセル中",
      },
      unrecognized: "インポート状態を認識できません",
    },
    result: {
      status: {
        imported: "インポート済み",
        already_imported: "既存",
        skipped: "スキップ",
        blocked: "ブロック",
        failed: "インポート失敗",
        cancelled: "キャンセル",
      },
      reasons: {
        already_imported: "内容が既に存在します",
        duplicate_in_batch: "バッチ内で重複",
        name_collision: "名前の競合",
        structure_invalid: "ディレクトリ構造が利用不可",
        metadata_invalid: "メタデータが利用不可",
        unsupported_entry: "未対応の項目を含みます",
        resource_limit_exceeded: "リソース上限を超過",
        source_unreadable: "ソースを読み取れません",
        source_changed: "ソースが変化しました",
        selection_revision_conflict: "選択リビジョンが変化しました",
        selection_empty: "選択が空です",
        selection_mutation_empty: "選択の変更がありません",
        selection_mutation_limit_exceeded: "選択の変更が多すぎます",
        selection_total_limit_exceeded: "選択の総数が上限を超過",
        selection_resource_limit_exceeded: "選択のリソースが上限を超過",
        selection_candidate_invalid: "候補の状態が変化しました",
        selection_expired: "選択の有効期限が切れました",
        selection_closed: "選択は封印済みです",
        selection_revision_overflow: "選択リビジョンが利用不可",
      },
      unknownReason: "結果理由を認識できません",
      retryable: "再試行可能",
      batchStatus: {
        completed: "すべて完了",
        completed_with_errors: "一部完了",
        failed: "タスク失敗（結果は保持）",
        cancelled: "タスクをキャンセル（結果は保持）",
      },
      unknownBatchStatus: "バッチ状態を認識できません",
      errors: {
        external_import_batch_unavailable: "一括インポート結果を利用できません。再スキャンしてください",
        external_import_batch_not_startable: "このバッチに再試行可能な項目はありません",
        external_import_result_cursor_invalid: "結果ページ位置を利用できません。再読込してください",
        external_import_result_limit_invalid: "結果ページサイズを利用できません。再読込してください",
        external_import_result_request_invalid: "結果リクエストを利用できません。再読込してください",
        external_import_selection_unavailable: "封印済みの選択を利用できません。再スキャンしてください",
        external_import_source_unavailable: "ソースが無効になりました。選び直して再試行してください",
        external_import_task_unavailable: "インポートタスクを利用できません。しばらくして再試行してください",
        external_import_result_invalid: "一括インポート結果を認識できません。再スキャンしてください",
      },
      fallbackError: "一括インポート結果を読み取れません。しばらくして再試行してください",
    },
    selection: {
      errors: {
        external_import_selection_unavailable: "候補選択を利用できません。再スキャンしてください",
        external_import_batch_unavailable: "インポートバッチを利用できません。再スキャンしてください",
        external_import_batch_not_startable: "このバッチは開始できません。再スキャンしてください",
        external_import_catalog_unavailable: "Mod カタログが一時的に利用できません。しばらくして再試行してください",
        external_import_category_unavailable: "カテゴリを利用できません。カテゴリを再読込してください",
        external_import_clock_unavailable: "選択状態を利用できません。しばらくして再試行してください",
        selection_revision_conflict: "選択が変化したため再読込しました",
        selection_empty: "候補を 1 つ以上選択してください",
        selection_mutation_empty: "更新が必要な候補はありません",
        selection_mutation_limit_exceeded: "一度の選択変更が多すぎます。分けて操作してください",
        selection_total_limit_exceeded: "選択数がバッチ上限を超えています",
        selection_resource_limit_exceeded: "選択内容がリソース上限を超えています",
        selection_candidate_invalid: "候補の状態が変化しました。再読込してください",
        selection_expired: "選択の有効期限が切れました。再スキャンしてください",
        selection_closed: "選択は封印済みのため変更できません",
        external_import_selection_invalid: "選択データを認識できません。再スキャンしてください",
        external_import_task_unavailable: "インポートタスクを利用できません。再試行してください",
        external_import_progress_unrecognized: "インポート状態を認識できないため操作を停止しました",
      },
      fallbackError: "候補選択を更新できません。再読込してから再試行してください",
    },
    preview: {
      status: {
        ready: "インポート可能",
        already_imported: "既存",
        duplicate_in_batch: "バッチ内重複",
        name_collision: "名前の競合",
        structure_invalid: "構造が利用不可",
        metadata_invalid: "メタデータが利用不可",
        unsupported_entry: "未対応の項目",
        resource_limit_exceeded: "リソース上限超過",
        source_unreadable: "ソース読み取り不可",
        payload_missing: "ペイロードがボックス内にありません",
      },
      rescan: "再スキャンが必要",
      conflicts: {
        content_duplicate: "内容の重複",
        name_collision: "同名の競合",
      },
      needsReview: "要確認",
      unnamed: "名称未設定の候補",
      fileCount: (count) => `${count} ファイル`,
    },
    action: {
      trigger: "サードパーティ Mod を移行",
      dialogTitle: "サードパーティ Mod 移行",
      dialogFallbackDescription: "読み取り専用スキャンと候補プレビュー",
      scanningToastTitle: "サードパーティソースをスキャン中",
      scanFailedToastTitle: "サードパーティソースのスキャンに失敗",
      scanCancelledToastTitle: "サードパーティソースのスキャンをキャンセル",
      scanCancelledToastMessage: "インポート可能な選択は作成されていません。",
      cancelScanFailedTitle: "スキャンをキャンセルできません",
      choosingSource: "ソースを選択中",
      creatingScanTask: "スキャンタスクを作成中",
      cancelling: "キャンセル中",
      cancelScan: "スキャンをキャンセル",
      retryListener: "状態リスナーを再試行",
      chooseSource: "ソースを選択",
      startImportWithCount: (count) => `${count} 件をインポート`,
      sourceEyebrow: "ソース",
      sourceNotChosen: "未選択",
      connectingScanStatus: "スキャン状態に接続中",
      scanCancelled: "スキャンをキャンセルしました",
    },
    candidate: {
      keepBothHint: "両方を保持して新しい Mod を作成",
      ignoreInvalidHint: "無効なメタデータを無視してインポートを続行",
      savingSelection: "候補選択を保存中",
      conflictLabel: "競合の処理",
      conflictPlaceholder: "明示的な処理方法を選択してください",
      categoryLabel: "インポート先カテゴリ",
      categoryNone: "カテゴリを割り当てない",
      unselectable:
        "この候補はバックエンドにより選択不可とされています。再スキャンするか、ソースの問題を解決してください。",
    },
    selectionPanel: {
      sealing: "選択を封印して一括インポートタスクを作成中",
      progressCount: (current, total) => `（${current} / ${total}）`,
      cancelPending: "キャンセルを要求中",
      cancelImport: "インポートをキャンセル",
      cancellingSafely: "安全にキャンセル中。一括インポート専用の終了状態を待っています",
      completedReadingResults: "一括インポートが完了しました。下の確定済み結果明細を読み込んでいます。",
      viewImportHistory: "インポート履歴を表示",
      closeAndReturn: "閉じて Mod ライブラリに戻る",
      cancelledNoInference:
        "一括インポートをキャンセルしました。このページは集計値から部分成功を推測しません。",
      incompleteReadingResults: "一括インポートは未完了です。保持された結果と復旧可能な操作を読み込んでいます。",
      candidateEyebrow: "候補選択",
      selectedCount: (count) => `${count} 件選択中`,
      creatingSnapshot: "選択スナップショットを作成中",
      editable: "編集可能",
      sealed: "封印済み",
      expired: "期限切れ",
      progressListenerUnavailable: "一括インポートの進捗を監視できないため、開始操作は無効です。",
      retryProgressListener: "進捗リスナーを再試行",
      reloadCategories: "カテゴリを再読込",
      loadingSnapshot: "選択スナップショットと候補プレビューを読み込み中",
      reloadCandidates: "候補を再読込",
      noCandidates: "表示できる候補がありません",
      noImportableHint:
        "インポート可能な項目がありません。番号付き Mod フォルダーを直接含むフォルダー(通常は狩技ボックスのインストール先の Mods_582010)を選択してください。「ペイロードがボックス内にありません」と表示された Mod は元のアーカイブからの再インポートが必要です。",
      scanReturned: (total) => `スキャンは ${total} 件を返しました。`,
      loadedCount: (loaded, total) => `${loaded} / ${total} 件を読み込み済み`,
      selectAllImportable: "インポート可能をすべて選択",
      deselectLoaded: "選択を解除",
      loadingMore: "読み込み中",
      loadMoreCandidates: "さらに候補を読み込む",
      importOnlyTitle: "HMM Mod ライブラリへのインポートのみ",
      importOnlyDescription: "インストール・有効化・ゲームディレクトリへの書き込みは行いません。",
      startBatchImport: "一括インポートを開始",
    },
    resultPanel: {
      title: "一括インポート結果",
      readingDetails: "サーバー確認済みの結果明細を読み込み中",
      reloadResults: "結果を再読込",
      resultEyebrow: "インポート結果",
      loadedCount: (loaded, total) => `${loaded} / ${total} 件を読み込み済み`,
      emptyBatch: "このバッチに結果項目はありません",
      retryingHint:
        "復旧可能な項目を再試行中。下は前回の確定結果で、タスク終了後に自動更新されます。",
      noDetailsTitle: "結果明細がありません",
      noDetailsBody: "バッチ状態はバックエンドで確認済みで、ページング可能な候補結果はありません。",
      summaryAria: "読み込み済み結果の集計",
      imported: (count) => `インポート済み ${count}`,
      alreadyImported: (count) => `既存 ${count}`,
      skipped: (count) => `スキップ ${count}`,
      blocked: (count) => `ブロック ${count}`,
      failed: (count) => `失敗 ${count}`,
      cancelled: (count) => `キャンセル ${count}`,
      modId: (id) => `Mod ID：${id}`,
      retryableBadge: "再試行可能",
      loadingMore: "読み込み中",
      loadMoreResults: "さらに結果を読み込む",
      creatingRetryTask: "再試行タスクを作成中",
      retryRecoverable: "復旧可能な項目を再試行",
    },
    workflow: {
      resultsLoadedTitle: "一括インポート結果を読み込みました",
      resultsLoadedMessage: "Mod リストとサーバー確認済みの結果明細を更新しました。",
      refreshFailedTitle: "結果は読み込み済み、Mod リストの更新に失敗",
      refreshFailedMessage: "インポート結果には影響ありません。後で Mod リストを手動で更新してください。",
      importingTitle: "Mod を一括インポート中",
      importingProgress: (current, total) => `（${current} / ${total}）`,
      importCompletedTitle: "一括インポートが完了",
      importCompletedMessage: "サーバー確認済みの結果明細を読み込んでいます。",
      importCancelledTitle: "一括インポートをキャンセル",
      importCancelledMessage: "保持された確定結果を読み込んでいます。",
      importIncompleteTitle: "一括インポートが未完了",
      cancelImportFailedTitle: "一括インポートをキャンセルできません",
    },
    history: {
      tabCurrent: "今回のインポート",
      tabHistory: "インポート履歴",
      tablistAria: "サードパーティ Mod 移行ビューの切り替え",
      historyTriggerAria: "サードパーティのインポート履歴を表示",
      title: "インポート履歴",
      loading: "インポート履歴を読み込んでいます…",
      loadedCount: (loaded: string, total: string) =>
        `${loaded} / ${total} 件のバッチを読み込み済み`,
      emptyTitle: "インポート履歴はまだありません",
      emptyHint: "狩技ボックスからの移行が完了すると、バッチ記録がここに表示されます。",
      adapters: {
        hunting_box_directory_v1: "狩技ボックス",
      },
      unknownAdapter: "不明なソース",
      states: {
        running: "インポート実行中",
        scanning: "スキャン中",
        scanOnly: "スキャンのみ・未インポート",
        scanFailed: "スキャン失敗",
        completed: "完了",
        completedWithErrors: "一部完了",
        incomplete: "未完了",
        cancelled: "キャンセル済み",
      },
      candidateCount: (count: string) => `候補 ${count} 件`,
      noResults: "このバッチには結果明細がありません",
      viewDetails: "明細を表示",
      hideDetails: "明細を閉じる",
      detailLoading: "バッチ明細を読み込んでいます…",
      reloadDetail: "明細を再読み込み",
      loadMoreDetails: "明細をさらに読み込む",
      loadingMore: "読み込み中…",
      loadMore: "さらに読み込む",
      reload: "履歴を再読み込み",
      retryHint:
        "履歴バッチから直接再試行はできません。再試行するにはソースフォルダーを選び直してください。",
      retentionHint:
        "インポート済みバッチは直近 50 件のみ保持されます。スキャンのみのバッチは 10 件・最長 7 日間です。",
      time: {
        justNow: "たった今",
        minutesAgo: (count: string) => `${count} 分前`,
        hoursAgo: (count: string) => `${count} 時間前`,
        daysAgo: (count: string) => `${count} 日前`,
      },
      errors: {
        external_import_history_cursor_invalid:
          "履歴のページカーソルが無効です。再読み込みしてください",
        external_import_history_limit_invalid:
          "履歴のページサイズが無効です。再読み込みしてください",
        external_import_history_request_invalid:
          "履歴リクエストが無効です。再読み込みしてください",
        external_import_batch_unavailable: "インポート履歴を利用できません。後でもう一度お試しください",
        external_import_history_invalid: "履歴データの検証に失敗しました。再読み込みしてください",
        external_import_result_invalid: "バッチ明細の検証に失敗しました。再読み込みしてください",
      },
      fallbackError: "インポート履歴を読み込めません。後でもう一度お試しください",
    },
  },
} satisfies LocaleDictionary<ExternalImportCopy>;
