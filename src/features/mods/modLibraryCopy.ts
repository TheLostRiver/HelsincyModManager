import type { LocaleDictionary } from "../../shared/i18n";
import type { ModLibraryQueryErrorCode } from "./modLibraryQueryState";
import type { ModSelectionNotice, ModSelectionResetReason } from "./modSelection";

// Mod 库页第一批文案（I18N-02）：库页、工具栏、分页、查询反馈、卡片、选择、右键菜单、
// 快捷操作面板。Mock 数据（modsLibraryData.ts）里的 Mod 名称/分类是内容不是 UI 文案，
// 不进字典。选择反馈与查询错误只在渲染时取词——reducer/state 层保存语义码与参数。

export type ModLibraryCopy = {
  page: {
    regionLabel: string;
    loadFailedFallback: string;
    queryBusy: string;
    queryErrors: Record<ModLibraryQueryErrorCode, string>;
    planPreview: {
      modNotFound: string;
      analysisUnavailable: string;
      archiveUnavailable: string;
      ambiguousContentRoot: string;
      unsupportedGame: string;
      failed: string;
    };
    lifecycleStart: {
      modNotFound: string;
      analysisUnavailable: string;
      unsupportedUninstall: string;
      unsupportedInstall: string;
      uninstallStartFailed: string;
      installStartFailed: string;
    };
    statusFilter: {
      needProfile: string;
      unsupported: string;
      profileLoading: string;
      selectProfile: string;
    };
    uninstallBlocked: {
      profileChanged: string;
      backendStatusChanged: string;
      backendSummaryChanged: string;
    };
    cardAction: {
      uninstallLabel: string;
      installLabel: string;
      installOrUninstallLabel: string;
      waitInstallTask: string;
      closeBatchFirst: string;
      notInList: string;
      batchSelecting: string;
      selectProfileFirst: string;
      resolveRecoveryFirst: string;
      statusNotActionable: string;
    };
    toasts: {
      profileNotReady: string;
      installInvalidType: string;
      installStartFailedTitle: string;
      uninstallInvalidType: string;
      uninstallStartFailedTitle: string;
    };
  };
  selection: {
    noticeToastTitle: string;
    limitReached: (max: number) => string;
    pageLimitSelect: (newCount: number, remainingSlots: number) => string;
    pageLimitInvert: (resultCount: number, max: number) => string;
    cleared: (count: number) => string;
    exitedBatch: string;
    exitedBatchCleared: (count: number) => string;
    contextResetCleared: (reason: string, count: number) => string;
    contextResetExited: (reason: string) => string;
    resetReasons: Record<ModSelectionResetReason, string>;
  };
  toolbar: {
    showLabels: string;
    hideLabels: string;
    searchPlaceholder: string;
    searchAria: string;
    viewSwitchAria: string;
    classicView: string;
    gridView: string;
    listView: string;
    techView: string;
    filtersAria: string;
  };
  filters: {
    all: string;
    installed: string;
    notInstalled: string;
  };
  pagination: {
    toolbarAria: string;
    emptyRange: string;
    range: (start: number, end: number, total: number) => string;
    busyRange: (range: string) => string;
    perPage: string;
    perPageSizeAria: (size: number) => string;
    perPageCountAria: string;
    items: (count: number) => string;
    pageNavAria: string;
    firstPage: string;
    gotoFirst: string;
    prevPage: string;
    gotoPrev: string;
    nextPage: string;
    gotoNext: string;
    lastPage: string;
    gotoLast: string;
    pageListAria: string;
    pageAria: (page: number) => string;
    jumpTo: (page: number) => string;
    busyLabel: string;
    compactEmpty: string;
    compactRange: (start: number, end: number, total: number) => string;
  };
  queryFeedback: {
    loadingAria: string;
    unavailableTitle: string;
    retry: string;
    filterUnavailableTitle: string;
    viewAllMods: string;
    emptyTitle: string;
    emptyBody: string;
    noMatchTitle: string;
    noMatchBody: string;
    clearFilters: string;
    updatingAria: string;
    retryQueryAria: string;
  };
  card: {
    status: {
      not_installed: string;
      installed: string;
      disabled: string;
      conflict: string;
      committed_cleanup_pending: string;
      cleanup_pending: string;
      rollback_required: string;
      repair_required: string;
      unknown: string;
    };
    statusWithFiles: (status: string, count: number) => string;
    statusWithIssues: (status: string, count: number) => string;
    categorySummary: (names: readonly string[]) => string;
    selectAria: (name: string, categorySummary: string) => string;
    deselectAria: (name: string, categorySummary: string) => string;
    versionLabel: string;
    sizeLabel: string;
    /** 附在状态 pill 内的「外部来源」短标（#286 切片 3b-1）。 */
    externalOriginShort: string;
    /** title/aria 用的全量说明；adapter 展示名来自 externalImportCopy（单一出处）。 */
    externalOriginTitle: (adapterLabel: string) => string;
  };
  contextMenu: {
    installOrUninstall: string;
    statusUnavailable: string;
    infoSettings: string;
    fileModify: string;
    jumpToNexus: string;
    openFolder: string;
  };
  compact: {
    queryBusy: string;
    selectOneFirst: string;
    waitInstallTask: string;
    selectProfileFor: (actionLabel: string) => string;
    actionLabels: {
      previewPlan: string;
      install: string;
      reinstall: string;
      uninstall: string;
      delete: string;
    };
    batchActionLabels: {
      previewPlan: string;
      install: string;
      reinstall: string;
      uninstall: string;
      delete: string;
    };
    buttons: {
      add: string;
      addRevision: string;
      selectAll: string;
      invert: string;
      refresh: string;
      previewPlan: string;
      install: string;
      reinstall: string;
      uninstall: string;
      delete: string;
    };
    previewNeedsInstallable: string;
    installNeedsInstallable: string;
    reinstallNeedsInstalled: string;
    uninstallNeedsInstalled: string;
    deleteNeedsNotInstalled: string;
    exitBatchToImportRevision: string;
    title: string;
    selectedPill: (count: number) => string;
    selectedSummary: (selected: number, max: number, pageSelected: number, pageCount: number) => string;
    selectedOne: string;
    noneSelected: string;
    footerBatchSelected: (selected: number, max: number) => string;
    footerBatchPage: (pageSelected: number, pageCount: number) => string;
    footerSinglePage: (pageSelected: number, pageCount: number) => string;
    enterBatch: string;
    exitBatch: string;
    noSelectedMods: string;
    clearSelectionAria: string;
  };
  backToTop: string;
};

export const modLibraryCopy = {
  zh_cn: {
    page: {
      regionLabel: "模组库",
      loadFailedFallback: "Mod 列表加载失败，请稍后重试",
      queryBusy: "Mod 列表正在更新，请稍候",
      queryErrors: {
        game_id_invalid: "游戏标识无效",
        profile_id_empty: "当前配置无效",
        mod_library_filter_invalid: "筛选条件无效",
        mod_library_sort_invalid: "排序方式无效",
        mod_library_page_invalid: "页码无效",
        mod_library_page_size_unsupported: "每页数量不受支持",
        mod_library_search_too_long: "搜索内容过长",
        mod_library_category_not_found: "所选分类已不存在",
        mod_library_profile_context_required: "请先选择游戏配置",
        mod_library_unavailable: "Mod 库暂时不可用",
        mod_library_status_unavailable: "安装状态暂时不可用",
      },
      planPreview: {
        modNotFound: "未找到已导入的 Mod",
        analysisUnavailable: "无法读取导入分析",
        archiveUnavailable: "无法读取导入文件",
        ambiguousContentRoot: "包内有多个 nativePC 目录，请拆分后分别导入",
        unsupportedGame: "当前游戏不支持安装计划预览",
        failed: "安装计划预览失败",
      },
      lifecycleStart: {
        modNotFound: "未找到已导入的 Mod",
        analysisUnavailable: "无法读取导入分析",
        unsupportedUninstall: "当前游戏不支持卸载任务",
        unsupportedInstall: "当前游戏不支持安装任务",
        uninstallStartFailed: "卸载任务启动失败",
        installStartFailed: "安装任务启动失败",
      },
      statusFilter: {
        needProfile: "请先选择可用的配置档，再查看安装状态筛选。",
        unsupported: "当前安装状态筛选不受支持，请选择其他筛选条件。",
        profileLoading: "配置档加载中",
        selectProfile: "选择配置档后可用",
      },
      uninstallBlocked: {
        profileChanged: "配置档状态已变化，当前不能安全卸载。",
        backendStatusChanged: "后端安装状态已变化，请关闭并刷新后重试。",
        backendSummaryChanged: "后端安装摘要已变化，请关闭并刷新后重试。",
      },
      cardAction: {
        uninstallLabel: "卸载 Mod",
        installLabel: "安装 Mod",
        installOrUninstallLabel: "安装 / 卸载 Mod",
        waitInstallTask: "请等待当前安装任务完成",
        closeBatchFirst: "请先完成或关闭当前批量操作",
        notInList: "当前 Mod 不在列表中",
        batchSelecting: "批量选择中，请使用上方批量操作",
        selectProfileFirst: "选择配置档后可执行此操作",
        resolveRecoveryFirst: "请先处理安装恢复状态",
        statusNotActionable: "当前安装状态不可执行此操作",
      },
      toasts: {
        profileNotReady: "配置档尚未就绪",
        installInvalidType: "安装任务返回了无效类型",
        installStartFailedTitle: "安装任务启动失败",
        uninstallInvalidType: "卸载任务返回了无效类型",
        uninstallStartFailedTitle: "卸载任务启动失败",
      },
    },
    selection: {
      noticeToastTitle: "批量选择",
      limitReached: (max) => `每批最多选择 ${max} 个 Mod，取消一项后可继续添加。`,
      pageLimitSelect: (newCount, remainingSlots) =>
        `选择本页需要新增 ${newCount} 项，当前仅剩 ${remainingSlots} 个名额。`,
      pageLimitInvert: (resultCount, max) =>
        `反选本页后将选择 ${resultCount} 项，超过每批 ${max} 项上限。`,
      cleared: (count) => `已清空 ${count} 项选择。`,
      exitedBatch: "已退出批量选择。",
      exitedBatchCleared: (count) => `已退出批量选择，并清空 ${count} 项选择。`,
      contextResetCleared: (reason, count) => `${reason}，已清空 ${count} 项选择。`,
      contextResetExited: (reason) => `${reason}，已退出批量选择。`,
      resetReasons: {
        "query-changed": "查询条件已变化",
        "filters-changed": "筛选条件已变化",
        "search-changed": "搜索条件已变化",
        "query-reset": "查询条件已重置",
        "library-refreshed": "Mod 库已刷新",
        "profile-changed": "活动配置档已变化",
        "batch-completed": "批量操作已完成",
      },
    },
    toolbar: {
      showLabels: "显示分类标签",
      hideLabels: "隐藏分类标签",
      searchPlaceholder: "搜索 Mod 名称、作者或标签…",
      searchAria: "搜索 Mod",
      viewSwitchAria: "排版视图切换",
      classicView: "经典简约视图",
      gridView: "增强网格视图",
      listView: "紧凑列表视图",
      techView: "机能数据面板视图",
      filtersAria: "Mod 筛选",
    },
    filters: {
      all: "全部",
      installed: "已安装",
      notInstalled: "未安装",
    },
    pagination: {
      toolbarAria: "Mod 库分页工具栏",
      emptyRange: "当前没有匹配的 Mod",
      range: (start, end, total) => `显示第 ${start} 至 ${end} 项，共 ${total} 项`,
      busyRange: (range) => `正在更新结果。${range}`,
      perPage: "每页",
      perPageSizeAria: (size) => `每页显示 ${size} 项`,
      perPageCountAria: "每页显示数量",
      items: (count) => `${count} 项`,
      pageNavAria: "Mod 库页码",
      firstPage: "第一页",
      gotoFirst: "前往第一页",
      prevPage: "上一页",
      gotoPrev: "前往上一页",
      nextPage: "下一页",
      gotoNext: "前往下一页",
      lastPage: "最后一页",
      gotoLast: "前往最后一页",
      pageListAria: "可选页码",
      pageAria: (page) => `第 ${page} 页`,
      jumpTo: (page) => `跳至第 ${page} 页`,
      busyLabel: "更新中",
      compactEmpty: "0 项",
      compactRange: (start, end, total) => `${start}–${end} / ${total}`,
    },
    queryFeedback: {
      loadingAria: "正在加载 Mod 库",
      unavailableTitle: "Mod 库暂时不可用",
      retry: "重试",
      filterUnavailableTitle: "当前筛选暂不可用",
      viewAllMods: "查看全部 Mod",
      emptyTitle: "尚未导入 Mod",
      emptyBody: "Mod 库当前为空。",
      noMatchTitle: "没有匹配的 Mod",
      noMatchBody: "当前搜索与筛选条件没有结果。",
      clearFilters: "清除条件",
      updatingAria: "正在更新 Mod 列表",
      retryQueryAria: "重试 Mod 库查询",
    },
    card: {
      status: {
        not_installed: "未安装",
        installed: "已安装",
        disabled: "已禁用",
        conflict: "存在冲突",
        committed_cleanup_pending: "重装待收尾",
        cleanup_pending: "恢复待清理",
        rollback_required: "需要回滚",
        repair_required: "需要修复",
        unknown: "状态未知",
      },
      statusWithFiles: (status, count) => `${status} · ${count} 文件`,
      statusWithIssues: (status, count) => `${status} · ${count} 项`,
      categorySummary: (names) => `，分类：${names.join("、")}`,
      selectAria: (name, categorySummary) => `选择 ${name}${categorySummary}`,
      deselectAria: (name, categorySummary) => `取消选择 ${name}${categorySummary}`,
      versionLabel: "版本: ",
      sizeLabel: "大小: ",
      externalOriginShort: "外部",
      externalOriginTitle: (adapterLabel) => `外部来源：${adapterLabel}`,
    },
    contextMenu: {
      installOrUninstall: "安装 / 卸载 Mod",
      statusUnavailable: "当前 Mod 状态不可用",
      infoSettings: "MOD 信息设置",
      fileModify: "MOD 文件修改",
      jumpToNexus: "跳到 NexusMods",
      openFolder: "打开 MOD 文件夹",
    },
    compact: {
      queryBusy: "Mod 列表正在更新，请稍候",
      selectOneFirst: "请先选择一个 MOD",
      waitInstallTask: "请等待当前安装任务完成",
      selectProfileFor: (actionLabel) => `选择配置档后可${actionLabel}`,
      actionLabels: {
        previewPlan: "预览安装计划",
        install: "安装",
        reinstall: "重装",
        uninstall: "卸载",
        delete: "删除",
      },
      batchActionLabels: {
        previewPlan: "预览批量计划",
        install: "批量安装",
        reinstall: "批量重装",
        uninstall: "批量卸载",
        delete: "批量删除",
      },
      buttons: {
        add: "导入 Mod",
        addRevision: "导入新版本",
        selectAll: "选择本页",
        invert: "反选本页",
        refresh: "刷新",
        previewPlan: "预览安装计划",
        install: "安装选中 MOD",
        reinstall: "重装选中 MOD",
        uninstall: "卸载选中 MOD",
        delete: "删除选中 MOD",
      },
      previewNeedsInstallable: "仅未安装且状态安全的 MOD 可预览安装计划",
      installNeedsInstallable: "仅未安装且状态安全的 MOD 可安装",
      reinstallNeedsInstalled: "仅已安装且状态安全的 MOD 可重装",
      uninstallNeedsInstalled: "仅已安装且状态安全的 MOD 可卸载",
      deleteNeedsNotInstalled: "仅未安装的 MOD 可删除；已安装的请先卸载",
      exitBatchToImportRevision: "退出批量选择后可导入新版本",
      title: "快捷操作",
      selectedPill: (count) => `已选 ${count}`,
      selectedSummary: (selected, max, pageSelected, pageCount) =>
        `已选 ${selected} / ${max}，本页已选 ${pageSelected} / ${pageCount} 项`,
      selectedOne: "已选择 1 项",
      noneSelected: "尚未选择 Mod",
      footerBatchSelected: (selected, max) => `已选 ${selected} / ${max}`,
      footerBatchPage: (pageSelected, pageCount) => `本页已选 ${pageSelected} / ${pageCount} 项`,
      footerSinglePage: (pageSelected, pageCount) => `本页已选 ${pageSelected} / 当前页 ${pageCount} 项`,
      enterBatch: "批量选择",
      exitBatch: "退出批量选择",
      noSelectedMods: "当前没有已选 Mod",
      clearSelectionAria: "清空选择",
    },
    backToTop: "返回顶部",
  },
  en: {
    page: {
      regionLabel: "Mod library",
      loadFailedFallback: "Failed to load the mod list. Please try again later.",
      queryBusy: "The mod list is updating, please wait",
      queryErrors: {
        game_id_invalid: "Invalid game identifier",
        profile_id_empty: "Invalid current profile",
        mod_library_filter_invalid: "Invalid filter",
        mod_library_sort_invalid: "Invalid sort order",
        mod_library_page_invalid: "Invalid page number",
        mod_library_page_size_unsupported: "Unsupported page size",
        mod_library_search_too_long: "Search text is too long",
        mod_library_category_not_found: "The selected category no longer exists",
        mod_library_profile_context_required: "Select a game profile first",
        mod_library_unavailable: "The mod library is temporarily unavailable",
        mod_library_status_unavailable: "Install status is temporarily unavailable",
      },
      planPreview: {
        modNotFound: "Imported mod not found",
        analysisUnavailable: "Cannot read the import analysis",
        archiveUnavailable: "Cannot read the imported file",
        ambiguousContentRoot:
          "The package contains more than one nativePC directory; split it and import separately",
        unsupportedGame: "This game does not support install plan preview",
        failed: "Install plan preview failed",
      },
      lifecycleStart: {
        modNotFound: "Imported mod not found",
        analysisUnavailable: "Cannot read the import analysis",
        unsupportedUninstall: "This game does not support uninstall tasks",
        unsupportedInstall: "This game does not support install tasks",
        uninstallStartFailed: "Failed to start the uninstall task",
        installStartFailed: "Failed to start the install task",
      },
      statusFilter: {
        needProfile: "Select an available profile first to use install-status filters.",
        unsupported: "This install-status filter is not supported. Choose another filter.",
        profileLoading: "Profile loading",
        selectProfile: "Available after selecting a profile",
      },
      uninstallBlocked: {
        profileChanged: "The profile state changed; uninstalling is not safe right now.",
        backendStatusChanged: "The backend install status changed. Close and refresh, then retry.",
        backendSummaryChanged: "The backend install summary changed. Close and refresh, then retry.",
      },
      cardAction: {
        uninstallLabel: "Uninstall mod",
        installLabel: "Install mod",
        installOrUninstallLabel: "Install / uninstall mod",
        waitInstallTask: "Wait for the current install task to finish",
        closeBatchFirst: "Finish or close the current batch operation first",
        notInList: "This mod is not in the list",
        batchSelecting: "Batch selection active — use the batch actions above",
        selectProfileFirst: "Select a profile to run this action",
        resolveRecoveryFirst: "Resolve the install recovery state first",
        statusNotActionable: "This action is unavailable in the current install status",
      },
      toasts: {
        profileNotReady: "The profile is not ready yet",
        installInvalidType: "The install task returned an invalid type",
        installStartFailedTitle: "Failed to start the install task",
        uninstallInvalidType: "The uninstall task returned an invalid type",
        uninstallStartFailedTitle: "Failed to start the uninstall task",
      },
    },
    selection: {
      noticeToastTitle: "Batch selection",
      limitReached: (max) =>
        `Each batch can select at most ${max} mods. Deselect one to add more.`,
      pageLimitSelect: (newCount, remainingSlots) =>
        `Selecting this page adds ${newCount} items, but only ${remainingSlots} slots remain.`,
      pageLimitInvert: (resultCount, max) =>
        `Inverting this page would select ${resultCount} items, exceeding the ${max}-per-batch limit.`,
      cleared: (count) => `Cleared ${count} selected items.`,
      exitedBatch: "Exited batch selection.",
      exitedBatchCleared: (count) => `Exited batch selection and cleared ${count} selected items.`,
      contextResetCleared: (reason, count) => `${reason}; cleared ${count} selected items.`,
      contextResetExited: (reason) => `${reason}; exited batch selection.`,
      resetReasons: {
        "query-changed": "The query changed",
        "filters-changed": "Filters changed",
        "search-changed": "The search changed",
        "query-reset": "The query was reset",
        "library-refreshed": "The mod library was refreshed",
        "profile-changed": "The active profile changed",
        "batch-completed": "The batch operation finished",
      },
    },
    toolbar: {
      showLabels: "Show category labels",
      hideLabels: "Hide category labels",
      searchPlaceholder: "Search mod names, authors, or tags…",
      searchAria: "Search mods",
      viewSwitchAria: "Layout view switch",
      classicView: "Classic view",
      gridView: "Enhanced grid view",
      listView: "Compact list view",
      techView: "Tech panel view",
      filtersAria: "Mod filters",
    },
    filters: {
      all: "All",
      installed: "Installed",
      notInstalled: "Not installed",
    },
    pagination: {
      toolbarAria: "Mod library pagination toolbar",
      emptyRange: "No matching mods",
      range: (start, end, total) => `Showing ${start}–${end} of ${total}`,
      busyRange: (range) => `Updating results. ${range}`,
      perPage: "Per page",
      perPageSizeAria: (size) => `Show ${size} per page`,
      perPageCountAria: "Items per page",
      items: (count) => `${count} items`,
      pageNavAria: "Mod library pages",
      firstPage: "First page",
      gotoFirst: "Go to first page",
      prevPage: "Previous page",
      gotoPrev: "Go to previous page",
      nextPage: "Next page",
      gotoNext: "Go to next page",
      lastPage: "Last page",
      gotoLast: "Go to last page",
      pageListAria: "Available pages",
      pageAria: (page) => `Page ${page}`,
      jumpTo: (page) => `Jump to page ${page}`,
      busyLabel: "Updating",
      compactEmpty: "0 items",
      compactRange: (start, end, total) => `${start}–${end} / ${total}`,
    },
    queryFeedback: {
      loadingAria: "Loading the mod library",
      unavailableTitle: "The mod library is temporarily unavailable",
      retry: "Retry",
      filterUnavailableTitle: "This filter is temporarily unavailable",
      viewAllMods: "View all mods",
      emptyTitle: "No mods imported yet",
      emptyBody: "The mod library is currently empty.",
      noMatchTitle: "No matching mods",
      noMatchBody: "No results for the current search and filters.",
      clearFilters: "Clear filters",
      updatingAria: "Updating the mod list",
      retryQueryAria: "Retry the mod library query",
    },
    card: {
      status: {
        not_installed: "Not installed",
        installed: "Installed",
        disabled: "Disabled",
        conflict: "Conflicts",
        committed_cleanup_pending: "Reinstall finishing",
        cleanup_pending: "Recovery cleanup pending",
        rollback_required: "Rollback required",
        repair_required: "Repair required",
        unknown: "Status unknown",
      },
      statusWithFiles: (status, count) => `${status} · ${count} files`,
      statusWithIssues: (status, count) => `${status} · ${count} issues`,
      categorySummary: (names) => `, categories: ${names.join(", ")}`,
      selectAria: (name, categorySummary) => `Select ${name}${categorySummary}`,
      deselectAria: (name, categorySummary) => `Deselect ${name}${categorySummary}`,
      versionLabel: "Version: ",
      sizeLabel: "Size: ",
      externalOriginShort: "External",
      externalOriginTitle: (adapterLabel) => `External source: ${adapterLabel}`,
    },
    contextMenu: {
      installOrUninstall: "Install / uninstall mod",
      statusUnavailable: "Mod status unavailable",
      infoSettings: "Mod info settings",
      fileModify: "Mod file editing",
      jumpToNexus: "Open on NexusMods",
      openFolder: "Open mod folder",
    },
    compact: {
      queryBusy: "The mod list is updating, please wait",
      selectOneFirst: "Select a mod first",
      waitInstallTask: "Wait for the current install task to finish",
      selectProfileFor: (actionLabel) => `Select a profile to ${actionLabel}`,
      actionLabels: {
        previewPlan: "preview the install plan",
        install: "install",
        reinstall: "reinstall",
        uninstall: "uninstall",
        delete: "delete",
      },
      batchActionLabels: {
        previewPlan: "Preview batch plan",
        install: "Batch install",
        reinstall: "Batch reinstall",
        uninstall: "Batch uninstall",
        delete: "Batch delete",
      },
      buttons: {
        add: "Import mod",
        addRevision: "Import new version",
        selectAll: "Select page",
        invert: "Invert page",
        refresh: "Refresh",
        previewPlan: "Preview install plan",
        install: "Install selected mods",
        reinstall: "Reinstall selected mods",
        uninstall: "Uninstall selected mods",
        delete: "Delete selected mods",
      },
      previewNeedsInstallable: "Only uninstalled mods in a safe state can preview the install plan",
      installNeedsInstallable: "Only uninstalled mods in a safe state can be installed",
      reinstallNeedsInstalled: "Only installed mods in a safe state can be reinstalled",
      uninstallNeedsInstalled: "Only installed mods in a safe state can be uninstalled",
      deleteNeedsNotInstalled: "Only mods that are not installed can be deleted; uninstall installed ones first",
      exitBatchToImportRevision: "Exit batch selection to import a new version",
      title: "Quick actions",
      selectedPill: (count) => `Selected ${count}`,
      selectedSummary: (selected, max, pageSelected, pageCount) =>
        `Selected ${selected} / ${max}; ${pageSelected} / ${pageCount} on this page`,
      selectedOne: "1 item selected",
      noneSelected: "No mods selected",
      footerBatchSelected: (selected, max) => `Selected ${selected} / ${max}`,
      footerBatchPage: (pageSelected, pageCount) => `Selected ${pageSelected} / ${pageCount} on this page`,
      footerSinglePage: (pageSelected, pageCount) => `Selected ${pageSelected} of ${pageCount} on this page`,
      enterBatch: "Batch select",
      exitBatch: "Exit batch selection",
      noSelectedMods: "No mods are selected",
      clearSelectionAria: "Clear selection",
    },
    backToTop: "Back to top",
  },
  ja: {
    page: {
      regionLabel: "Mod ライブラリ",
      loadFailedFallback: "Mod リストの読み込みに失敗しました。しばらくしてから再試行してください。",
      queryBusy: "Mod リストを更新しています。お待ちください",
      queryErrors: {
        game_id_invalid: "ゲーム ID が無効です",
        profile_id_empty: "現在のプロファイルが無効です",
        mod_library_filter_invalid: "フィルターが無効です",
        mod_library_sort_invalid: "並び順が無効です",
        mod_library_page_invalid: "ページ番号が無効です",
        mod_library_page_size_unsupported: "ページサイズが未対応です",
        mod_library_search_too_long: "検索テキストが長すぎます",
        mod_library_category_not_found: "選択したカテゴリは存在しません",
        mod_library_profile_context_required: "先にゲームプロファイルを選択してください",
        mod_library_unavailable: "Mod ライブラリは一時的に利用できません",
        mod_library_status_unavailable: "インストール状態は一時的に利用できません",
      },
      planPreview: {
        modNotFound: "インポート済みの Mod が見つかりません",
        analysisUnavailable: "インポート解析を読み取れません",
        archiveUnavailable: "インポートファイルを読み取れません",
        ambiguousContentRoot:
          "パッケージ内に nativePC ディレクトリが複数あります。分割してから個別にインポートしてください",
        unsupportedGame: "このゲームはインストール計画プレビューに対応していません",
        failed: "インストール計画のプレビューに失敗しました",
      },
      lifecycleStart: {
        modNotFound: "インポート済みの Mod が見つかりません",
        analysisUnavailable: "インポート解析を読み取れません",
        unsupportedUninstall: "このゲームはアンインストールタスクに対応していません",
        unsupportedInstall: "このゲームはインストールタスクに対応していません",
        uninstallStartFailed: "アンインストールタスクの開始に失敗しました",
        installStartFailed: "インストールタスクの開始に失敗しました",
      },
      statusFilter: {
        needProfile: "インストール状態フィルターを使うには、先に利用可能なプロファイルを選択してください。",
        unsupported: "このインストール状態フィルターは未対応です。別のフィルターを選択してください。",
        profileLoading: "プロファイル読み込み中",
        selectProfile: "プロファイル選択後に利用可能",
      },
      uninstallBlocked: {
        profileChanged: "プロファイルの状態が変化したため、現在は安全にアンインストールできません。",
        backendStatusChanged: "バックエンドのインストール状態が変化しました。閉じて更新してから再試行してください。",
        backendSummaryChanged: "バックエンドのインストールサマリーが変化しました。閉じて更新してから再試行してください。",
      },
      cardAction: {
        uninstallLabel: "Mod をアンインストール",
        installLabel: "Mod をインストール",
        installOrUninstallLabel: "Mod をインストール / アンインストール",
        waitInstallTask: "現在のインストールタスクの完了をお待ちください",
        closeBatchFirst: "先に現在の一括操作を完了するか閉じてください",
        notInList: "この Mod はリストにありません",
        batchSelecting: "一括選択中です。上部の一括操作をご利用ください",
        selectProfileFirst: "プロファイル選択後に実行できます",
        resolveRecoveryFirst: "先にインストール復旧状態を処理してください",
        statusNotActionable: "現在のインストール状態ではこの操作を実行できません",
      },
      toasts: {
        profileNotReady: "プロファイルの準備ができていません",
        installInvalidType: "インストールタスクが無効な型を返しました",
        installStartFailedTitle: "インストールタスクの開始に失敗しました",
        uninstallInvalidType: "アンインストールタスクが無効な型を返しました",
        uninstallStartFailedTitle: "アンインストールタスクの開始に失敗しました",
      },
    },
    selection: {
      noticeToastTitle: "一括選択",
      limitReached: (max) =>
        `1 回の一括選択は最大 ${max} 件です。1 件解除すると追加できます。`,
      pageLimitSelect: (newCount, remainingSlots) =>
        `このページを選択すると ${newCount} 件追加されますが、残り枠は ${remainingSlots} 件です。`,
      pageLimitInvert: (resultCount, max) =>
        `このページを反転選択すると ${resultCount} 件になり、1 回あたり ${max} 件の上限を超えます。`,
      cleared: (count) => `${count} 件の選択を解除しました。`,
      exitedBatch: "一括選択を終了しました。",
      exitedBatchCleared: (count) => `一括選択を終了し、${count} 件の選択を解除しました。`,
      contextResetCleared: (reason, count) => `${reason}。${count} 件の選択を解除しました。`,
      contextResetExited: (reason) => `${reason}。一括選択を終了しました。`,
      resetReasons: {
        "query-changed": "検索条件が変化しました",
        "filters-changed": "フィルターが変化しました",
        "search-changed": "検索内容が変化しました",
        "query-reset": "検索条件をリセットしました",
        "library-refreshed": "Mod ライブラリを更新しました",
        "profile-changed": "アクティブなプロファイルが変化しました",
        "batch-completed": "一括操作が完了しました",
      },
    },
    toolbar: {
      showLabels: "カテゴリラベルを表示",
      hideLabels: "カテゴリラベルを非表示",
      searchPlaceholder: "Mod 名・作者・タグを検索…",
      searchAria: "Mod を検索",
      viewSwitchAria: "レイアウト切り替え",
      classicView: "クラシック表示",
      gridView: "拡張グリッド表示",
      listView: "コンパクトリスト表示",
      techView: "テクニカルパネル表示",
      filtersAria: "Mod フィルター",
    },
    filters: {
      all: "すべて",
      installed: "インストール済み",
      notInstalled: "未インストール",
    },
    pagination: {
      toolbarAria: "Mod ライブラリのページネーション",
      emptyRange: "一致する Mod はありません",
      range: (start, end, total) => `${start}～${end} 件目を表示（全 ${total} 件）`,
      busyRange: (range) => `結果を更新しています。${range}`,
      perPage: "表示件数",
      perPageSizeAria: (size) => `1 ページに ${size} 件表示`,
      perPageCountAria: "1 ページの表示件数",
      items: (count) => `${count} 件`,
      pageNavAria: "Mod ライブラリのページ",
      firstPage: "最初のページ",
      gotoFirst: "最初のページへ",
      prevPage: "前のページ",
      gotoPrev: "前のページへ",
      nextPage: "次のページ",
      gotoNext: "次のページへ",
      lastPage: "最後のページ",
      gotoLast: "最後のページへ",
      pageListAria: "選択可能なページ",
      pageAria: (page) => `${page} ページ目`,
      jumpTo: (page) => `${page} ページ目へジャンプ`,
      busyLabel: "更新中",
      compactEmpty: "0 件",
      compactRange: (start, end, total) => `${start}–${end} / ${total}`,
    },
    queryFeedback: {
      loadingAria: "Mod ライブラリを読み込み中",
      unavailableTitle: "Mod ライブラリは一時的に利用できません",
      retry: "再試行",
      filterUnavailableTitle: "このフィルターは一時的に利用できません",
      viewAllMods: "すべての Mod を表示",
      emptyTitle: "Mod はまだインポートされていません",
      emptyBody: "Mod ライブラリは現在空です。",
      noMatchTitle: "一致する Mod がありません",
      noMatchBody: "現在の検索・フィルター条件に一致する結果はありません。",
      clearFilters: "条件をクリア",
      updatingAria: "Mod リストを更新中",
      retryQueryAria: "Mod ライブラリの検索を再試行",
    },
    card: {
      status: {
        not_installed: "未インストール",
        installed: "インストール済み",
        disabled: "無効",
        conflict: "競合あり",
        committed_cleanup_pending: "再インストール仕上げ待ち",
        cleanup_pending: "復旧クリーンアップ待ち",
        rollback_required: "ロールバックが必要",
        repair_required: "修復が必要",
        unknown: "状態不明",
      },
      statusWithFiles: (status, count) => `${status} · ${count} ファイル`,
      statusWithIssues: (status, count) => `${status} · ${count} 件`,
      categorySummary: (names) => `、カテゴリ：${names.join("、")}`,
      selectAria: (name, categorySummary) => `${name} を選択${categorySummary}`,
      deselectAria: (name, categorySummary) => `${name} の選択を解除${categorySummary}`,
      versionLabel: "バージョン: ",
      sizeLabel: "サイズ: ",
      externalOriginShort: "外部",
      externalOriginTitle: (adapterLabel) => `外部ソース：${adapterLabel}`,
    },
    contextMenu: {
      installOrUninstall: "Mod をインストール / アンインストール",
      statusUnavailable: "Mod の状態を取得できません",
      infoSettings: "Mod 情報設定",
      fileModify: "Mod ファイル編集",
      jumpToNexus: "NexusMods で開く",
      openFolder: "Mod フォルダーを開く",
    },
    compact: {
      queryBusy: "Mod リストを更新しています。お待ちください",
      selectOneFirst: "先に Mod を 1 件選択してください",
      waitInstallTask: "現在のインストールタスクの完了をお待ちください",
      selectProfileFor: (actionLabel) => `プロファイル選択後に${actionLabel}できます`,
      actionLabels: {
        previewPlan: "インストール計画をプレビュー",
        install: "インストール",
        reinstall: "再インストール",
        uninstall: "アンインストール",
        delete: "削除",
      },
      batchActionLabels: {
        previewPlan: "一括計画をプレビュー",
        install: "一括インストール",
        reinstall: "一括再インストール",
        uninstall: "一括アンインストール",
        delete: "一括削除",
      },
      buttons: {
        add: "Mod をインポート",
        addRevision: "新バージョンをインポート",
        selectAll: "このページを選択",
        invert: "このページを反転選択",
        refresh: "更新",
        previewPlan: "インストール計画をプレビュー",
        install: "選択した Mod をインストール",
        reinstall: "選択した Mod を再インストール",
        uninstall: "選択した Mod をアンインストール",
        delete: "選択した Mod を削除",
      },
      previewNeedsInstallable:
        "未インストールかつ安全な状態の Mod のみインストール計画をプレビューできます",
      installNeedsInstallable: "未インストールかつ安全な状態の Mod のみインストールできます",
      reinstallNeedsInstalled: "インストール済みかつ安全な状態の Mod のみ再インストールできます",
      uninstallNeedsInstalled: "インストール済みかつ安全な状態の Mod のみアンインストールできます",
      deleteNeedsNotInstalled: "未インストールの Mod のみ削除できます。インストール済みは先にアンインストールしてください",
      exitBatchToImportRevision: "一括選択を終了すると新バージョンをインポートできます",
      title: "クイック操作",
      selectedPill: (count) => `選択中 ${count}`,
      selectedSummary: (selected, max, pageSelected, pageCount) =>
        `選択中 ${selected} / ${max}、このページで ${pageSelected} / ${pageCount} 件`,
      selectedOne: "1 件選択中",
      noneSelected: "Mod が選択されていません",
      footerBatchSelected: (selected, max) => `選択中 ${selected} / ${max}`,
      footerBatchPage: (pageSelected, pageCount) => `このページで ${pageSelected} / ${pageCount} 件選択中`,
      footerSinglePage: (pageSelected, pageCount) => `このページで ${pageSelected} / ${pageCount} 件選択中`,
      enterBatch: "一括選択",
      exitBatch: "一括選択を終了",
      noSelectedMods: "選択中の Mod はありません",
      clearSelectionAria: "選択をクリア",
    },
    backToTop: "トップへ戻る",
  },
} satisfies LocaleDictionary<ModLibraryCopy>;

/** 结构化选择反馈 -> 当前语言文本；reducer 不产出文本，渲染方统一走这里。 */
export function renderModSelectionNotice(
  notice: ModSelectionNotice,
  selection: ModLibraryCopy["selection"],
): string {
  switch (notice.code) {
    case "mod_selection_limit_reached":
      return selection.limitReached(notice.maxCount);
    case "mod_selection_page_limit_exceeded":
      return notice.variant === "select-page"
        ? selection.pageLimitSelect(notice.newCount, notice.remainingSlots)
        : selection.pageLimitInvert(notice.resultCount, notice.maxCount);
    case "mod_selection_cleared":
      if (notice.exitedBatch) {
        return notice.clearedCount > 0
          ? selection.exitedBatchCleared(notice.clearedCount)
          : selection.exitedBatch;
      }
      return selection.cleared(notice.clearedCount);
    case "mod_selection_context_reset": {
      const reason = selection.resetReasons[notice.reason];
      return notice.clearedCount > 0
        ? selection.contextResetCleared(reason, notice.clearedCount)
        : selection.contextResetExited(reason);
    }
  }
}
