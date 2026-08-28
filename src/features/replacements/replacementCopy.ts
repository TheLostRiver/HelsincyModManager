import type { LocaleDictionary } from "../../shared/i18n";
import type { WeaponReplacementErrorCode } from "./replacementErrorText";
import type { ReplacementWarning } from "./replacementTypes";
import type { RetargetInstallTaskPhase } from "./replacementWorkflow";

// 替换目标（Armor / Weapon retarget）全部用户可见文案。
// weapon 错误码表保持 Record<WeaponReplacementErrorCode, …> 穷尽约束：
// 每种语言各自成为一道 tsc 闸门，新增码而缺文案时编译失败（配合
// replacementErrorCodeContract.test.mjs 的跨语言集合比对）。
// 文案约束沿用 docs/WEAPON_RETARGET_DESIGN.md 的脱敏要求：
// 只出现稳定码、聚合描述与可执行建议，不回显路径、offset、material 名或二进制内容。

type ErrorCopy = {
  /** 发生了什么。 */
  message: string;
  /** 用户可以采取的下一步动作。 */
  hint?: string;
};

export type ReplacementCopy = {
  warnings: Record<ReplacementWarning, string>;
  block: {
    profileUnavailable: string;
    completedRefreshing: string;
    cleanupPending: string;
    rollbackRequired: string;
    repairRequired: string;
    statusUnknown: string;
  };
  phases: Record<RetargetInstallTaskPhase, string>;
  events: {
    reinstallFailed: string;
    retargetFailed: string;
    refreshFailed: string;
    invalidTaskType: string;
    startFailed: string;
    invalidCancelResult: string;
    cancelFailed: string;
    analysisFallback: string;
    previewFallback: string;
  };
  errors: {
    weapon: Record<WeaponReplacementErrorCode, ErrorCopy>;
    generic: Record<string, ErrorCopy>;
    diagnostic: (code: string) => string;
  };
  panel: {
    analyzing: string;
    retry: string;
    detectionTitle: string;
    resourceCount: (count: number) => string;
    noSources: string;
    warningsAria: string;
    targetsTitle: string;
    targetCount: (count: number) => string;
    searchAria: string;
    searchPlaceholder: string;
    targetsAria: string;
    currentInstalled: string;
    noMatches: string;
    previewLoading: string;
    switchPreviewTitle: string;
    initialPreviewTitle: string;
    actionCount: (count: number) => string;
    factResourceType: string;
    factTargetId: string;
    factActions: string;
    blockingConflicts: (count: number) => string;
    noBlockingConflicts: string;
    blockingConflictHint: string;
    prerequisiteResultsAria: string;
    countRetained: string;
    countReplaced: string;
    countAdded: string;
    countStale: string;
    preflightPassed: string;
    switchBlockedAria: string;
    candidateAlreadyInstalled: string;
    listenerUnavailable: string;
    retryListener: string;
    startingInstall: string;
    cancelling: string;
    cancelTask: string;
    refreshing: string;
    retryRefresh: string;
    previewSwitch: string;
    generatePreview: string;
    confirmSwitch: string;
    installToTarget: string;
  };
};

export const replacementCopy = {
  zh_cn: {
    warnings: {
      no_supported_assets: "未检测到受支持的外观资源",
      multiple_sources: "检测到多个源槽位，当前版本不会自动拆分",
      unsupported_source: "包内包含当前版本不支持的源槽位",
      source_matches_target: "源槽位与目标槽位相同",
      weapon_partial_part_set: "武器包只包含部分可选部件，将仅处理已检测到的完整文件对",
    },
    block: {
      profileUnavailable: "当前 Profile 不可用。",
      completedRefreshing: "写入已完成，正在刷新安装状态。",
      cleanupPending: "当前 Profile 有待收尾的重装事务。",
      rollbackRequired: "当前 Profile 需要先完成安装回滚。",
      repairRequired: "当前 Profile 需要先完成人工修复。",
      statusUnknown: "安装状态未知，替换目标写入已阻止。",
    },
    phases: {
      "install.retarget.queued": "等待安装",
      "install.retarget.plan.building": "重建替换计划",
      "install.retarget.commit.processing": "写入并记录安装清单",
      "install.retarget.completed": "替换目标安装完成",
      "install.retarget.failed": "替换目标安装失败",
      "install.cancelled": "替换目标安装已取消",
      "install.reinstall.queued": "等待目标切换",
      "install.reinstall.plan.building": "生成目标切换计划",
      "install.reinstall.preflight.processing": "执行提交前检查",
      "install.reinstall.commit.processing": "提交目标切换",
      "install.reinstall.rollback.processing": "恢复原目标",
      "install.reinstall.completed": "替换目标切换完成",
      "install.reinstall.failed": "替换目标切换失败",
      "install.reinstall.cancelled": "替换目标切换已取消",
    },
    events: {
      reinstallFailed: "目标切换失败，请刷新状态并重新生成预览",
      retargetFailed: "替换目标安装失败，请刷新状态后重试",
      refreshFailed: "安装已完成，但状态刷新失败，请重试。",
      invalidTaskType: "后端返回了无效任务类型",
      startFailed: "替换目标写入任务启动失败",
      invalidCancelResult: "后端返回了无效取消结果，请等待任务状态更新",
      cancelFailed: "无法取消任务，请等待执行结果",
      analysisFallback: "替换目标信息读取失败",
      previewFallback: "替换目标预览失败",
    },
    errors: {
      weapon: {
        weapon_binary_format_invalid: {
          message: "该武器 Mod 的模型文件格式无法识别，可能不是 Iceborne 版本或文件已损坏。",
          hint: "可以重新下载该 Mod，或直接安装到它原本的武器上。",
        },
        weapon_binary_pair_incompatible: {
          message: "该武器 Mod 的模型与材质文件不匹配，无法安全改到其他武器上。",
          hint: "可以重新下载该 Mod，或直接安装到它原本的武器上。",
        },
        weapon_binary_reference_unsafe: {
          message: "该武器 Mod 的材质引用包含不安全的路径，已阻止写入。",
          hint: "请向 Mod 作者反馈，或直接安装到它原本的武器上。",
        },
        weapon_binary_reference_ambiguous: {
          message: "该武器 Mod 的材质引用无法唯一对应到目标武器，已阻止写入。",
          hint: "请向 Mod 作者反馈，或直接安装到它原本的武器上。",
        },
        weapon_binary_path_too_long: {
          message: "改到该目标后材质路径会超出游戏格式上限，已阻止写入。",
          hint: "请改选其他目标武器再试。",
        },
        weapon_transformer_output_invalid: {
          message: "转换结果未通过完整性校验，已阻止写入游戏目录。",
          hint: "请重试；若持续出现，请连同下方诊断码反馈。",
        },
        weapon_source_not_found: {
          message: "该 Mod 中没有找到可识别的武器资源。",
          hint: "请确认它是 nativePC/wp 结构的 MHW:I 武器 Mod。",
        },
        weapon_multiple_source_roots: {
          message: "该压缩包包含多套武器资源，当前版本无法自动重定向。",
          hint: "请拆分成单套武器后分别导入。",
        },
        weapon_mixed_family: {
          message: "该压缩包混合了多种武器类型，当前版本无法自动重定向。",
          hint: "请拆分成单一武器类型后分别导入。",
        },
        weapon_mixed_install_payload: {
          message: "该压缩包除武器资源外还包含其他会写入游戏的内容，已阻止自动重定向。",
          hint: "请只保留武器资源后重新导入。",
        },
        weapon_incomplete_binary_pair: {
          message: "该武器 Mod 缺少成对的模型或材质文件。",
          hint: "请重新下载完整的 Mod 包。",
        },
        weapon_unknown_part: {
          message: "该武器 Mod 包含当前版本无法识别的部件。",
          hint: "当前版本只支持武器主体与已知副件（盾、鞘、副手等）。",
        },
        weapon_unsupported_resource: {
          message: "该武器 Mod 包含当前版本不支持的资源类型。",
          hint: "当前版本只支持 .mod3 与 .mrl3 文件。",
        },
        weapon_unsafe_path: {
          message: "该 Mod 包内存在不安全的文件路径，已阻止处理。",
          hint: "请重新下载该 Mod。",
        },
        weapon_duplicate_asset_path: {
          message: "该 Mod 包内存在重复的文件路径。",
          hint: "请重新下载该 Mod。",
        },
        weapon_case_insensitive_path_collision: {
          message: "该 Mod 包内存在仅大小写不同的冲突文件路径。",
          hint: "请重新下载该 Mod。",
        },
        weapon_invalid_package_file_id: {
          message: "该 Mod 的导入记录已失效。",
          hint: "请重新导入该 Mod。",
        },
        weapon_duplicate_package_file_id: {
          message: "该 Mod 的导入记录存在重复条目。",
          hint: "请重新导入该 Mod。",
        },
        weapon_identity_invalid: {
          message: "无法确认该武器资源的身份，已阻止处理。",
          hint: "请重新导入该 Mod。",
        },
        weapon_cross_family_target: {
          message: "所选目标与该 Mod 的武器类型不同，当前版本不支持跨武器类型改装。",
          hint: "请选择同一类武器作为目标。",
        },
        weapon_unknown_family: {
          message: "该武器类型不在当前支持范围内。",
          hint: "当前版本支持 MHW:I 的 14 类武器。",
        },
        weapon_invalid_main_id: {
          message: "该武器资源编号不符合游戏规范，已阻止处理。",
          hint: "请重新下载该 Mod。",
        },
      },
      generic: {
        replacement_mod_not_found: { message: "未找到已导入的 Mod。" },
        replacement_package_unavailable: { message: "导入包当前不可用。" },
        replacement_source_not_retargetable: {
          message: "该 Mod 不是当前可自动处理的单源外观包。",
        },
        replacement_target_catalog_unavailable: { message: "替换目标目录暂不可用。" },
        replacement_analysis_unavailable: { message: "替换分析暂不可用。" },
        weapon_source_content_unavailable: {
          message: "无法读取受控武器资源。",
          hint: "请重新导入该 Mod。",
        },
        replacement_target_not_found: { message: "所选替换目标已不存在。" },
        replacement_install_state_unavailable: { message: "无法确认当前安装状态。" },
        replacement_initial_install_blocked: {
          message: "当前安装或恢复状态不允许首次替换安装。",
        },
        replacement_installed_binding_unavailable: {
          message: "无法确认当前已安装的替换目标。",
        },
        replacement_target_already_selected: { message: "当前目标已安装。" },
        replacement_preview_unavailable: { message: "替换预览暂不可用。" },
        replacement_install_manifest_unavailable: {
          message: "安装清单暂时不可用，无法确认写入是否安全。",
          hint: "请稍后刷新重试；若持续出现，请先在恢复中心处理未完成的恢复项。",
        },
        replacement_reinstall_preview_unavailable: {
          message: "无法读取目标切换预览的完整信息。",
          hint: "请刷新后重试；若持续出现，请连同当前 Mod 与目标一起反馈。",
        },
        replacement_mod_id_invalid: {
          message: "Mod 标识无效，无法定位该 Mod。",
          hint: "请刷新 Mod 列表后重试。",
        },
        replacement_profile_id_invalid: {
          message: "Profile 标识无效。",
          hint: "请重新选择 Profile 后重试。",
        },
        replacement_target_id_invalid: {
          message: "替换目标标识无效。",
          hint: "请重新选择目标后重试。",
        },
        replacement_layer_invalid: {
          message: "安装层参数无效，已阻止本次操作。",
          hint: "请刷新后重试；若持续出现，请反馈。",
        },
        plan_token_invalid: {
          message: "目标切换预览已失效。",
          hint: "请重新生成预览。",
        },
        task_cannot_be_cancelled: {
          message: "任务已进入提交阶段，无法取消。",
          hint: "请等待执行结果。",
        },
        task_not_found: { message: "无法确认当前任务。", hint: "请刷新状态。" },
        reinstall_catalog_unavailable: { message: "目标切换预览暂不可用。" },
        reinstall_manifest_unavailable: { message: "目标切换预览暂不可用。" },
        reinstall_recovery_unavailable: { message: "目标切换预览暂不可用。" },
        reinstall_candidate_plan_unavailable: { message: "目标切换预览暂不可用。" },
        replacement_unsupported_game: { message: "当前游戏不支持替换目标。" },
      },
      diagnostic: (code: string) => `（诊断码：${code}）`,
    },
    panel: {
      analyzing: "正在分析替换资源",
      retry: "重试",
      detectionTitle: "检测结果",
      resourceCount: (count: number) => `${count} 个资源`,
      noSources: "未检测到可替换的外观槽位。",
      warningsAria: "分析警告",
      targetsTitle: "替换目标",
      targetCount: (count: number) => `${count} 项`,
      searchAria: "搜索替换目标",
      searchPlaceholder: "搜索名称、别名或槽位",
      targetsAria: "替换目标",
      currentInstalled: "当前已安装",
      noMatches: "没有匹配的替换目标。",
      previewLoading: "正在生成预览",
      switchPreviewTitle: "目标切换预览",
      initialPreviewTitle: "写入预览",
      actionCount: (count: number) => `${count} 个动作`,
      factResourceType: "资源类型",
      factTargetId: "目标编号",
      factActions: "写入动作",
      blockingConflicts: (count: number) => `检测到 ${count} 个阻断冲突`,
      noBlockingConflicts: "未检测到阻断冲突",
      blockingConflictHint: "目标文件可能已被其他 Mod 安装占用，请先卸载占用该目标的 Mod 后重试",
      prerequisiteResultsAria: "安装前置检查结果",
      countRetained: "保留",
      countReplaced: "替换",
      countAdded: "新增",
      countStale: "移除旧项",
      preflightPassed: "安全预检通过",
      switchBlockedAria: "目标切换阻断项",
      candidateAlreadyInstalled: "当前目标已安装",
      listenerUnavailable: "任务状态监听不可用",
      retryListener: "重试监听",
      startingInstall: "正在启动安装任务",
      cancelling: "正在取消",
      cancelTask: "取消任务",
      refreshing: "正在刷新安装状态",
      retryRefresh: "重试刷新",
      previewSwitch: "预览目标切换",
      generatePreview: "生成预览",
      confirmSwitch: "确认重装并切换",
      installToTarget: "安装到此目标",
    },
  },
  en: {
    warnings: {
      no_supported_assets: "No supported appearance assets detected",
      multiple_sources: "Multiple source slots detected; this version will not split them automatically",
      unsupported_source: "The package contains source slots not supported by this version",
      source_matches_target: "Source slot is the same as the target slot",
      weapon_partial_part_set: "This weapon package contains only some optional parts; only detected complete file pairs will be processed",
    },
    block: {
      profileUnavailable: "The current profile is unavailable.",
      completedRefreshing: "Write completed; refreshing install status.",
      cleanupPending: "The current profile has a reinstall transaction pending cleanup.",
      rollbackRequired: "The current profile must complete an install rollback first.",
      repairRequired: "The current profile requires manual repair first.",
      statusUnknown: "Install status is unknown; replacement target writes are blocked.",
    },
    phases: {
      "install.retarget.queued": "Waiting to install",
      "install.retarget.plan.building": "Rebuilding replacement plan",
      "install.retarget.commit.processing": "Writing and recording install manifest",
      "install.retarget.completed": "Replacement target installed",
      "install.retarget.failed": "Replacement target install failed",
      "install.cancelled": "Replacement target install cancelled",
      "install.reinstall.queued": "Waiting for target switch",
      "install.reinstall.plan.building": "Building target switch plan",
      "install.reinstall.preflight.processing": "Running pre-commit checks",
      "install.reinstall.commit.processing": "Committing target switch",
      "install.reinstall.rollback.processing": "Restoring original target",
      "install.reinstall.completed": "Replacement target switched",
      "install.reinstall.failed": "Replacement target switch failed",
      "install.reinstall.cancelled": "Replacement target switch cancelled",
    },
    events: {
      reinstallFailed: "Target switch failed. Refresh the status and regenerate the preview.",
      retargetFailed: "Replacement target install failed. Refresh the status and try again.",
      refreshFailed: "Install completed, but the status refresh failed. Please retry.",
      invalidTaskType: "The backend returned an invalid task type",
      startFailed: "Failed to start the replacement target write task",
      invalidCancelResult: "The backend returned an invalid cancel result; wait for the task status to update",
      cancelFailed: "Unable to cancel the task; wait for the result",
      analysisFallback: "Failed to load replacement target info",
      previewFallback: "Replacement target preview failed",
    },
    errors: {
      weapon: {
        weapon_binary_format_invalid: {
          message: "The model file format of this weapon mod is unrecognized; it may not be an Iceborne version or the file is corrupted.",
          hint: "Re-download the mod, or install it on its original weapon.",
        },
        weapon_binary_pair_incompatible: {
          message: "The model and material files of this weapon mod do not match; it cannot be safely retargeted to another weapon.",
          hint: "Re-download the mod, or install it on its original weapon.",
        },
        weapon_binary_reference_unsafe: {
          message: "The material references of this weapon mod contain unsafe paths; the write was blocked.",
          hint: "Report this to the mod author, or install it on its original weapon.",
        },
        weapon_binary_reference_ambiguous: {
          message: "The material references of this weapon mod cannot be uniquely mapped to the target weapon; the write was blocked.",
          hint: "Report this to the mod author, or install it on its original weapon.",
        },
        weapon_binary_path_too_long: {
          message: "After retargeting, the material path would exceed the game's format limit; the write was blocked.",
          hint: "Choose a different target weapon and try again.",
        },
        weapon_transformer_output_invalid: {
          message: "The transformed output failed integrity validation; writing to the game directory was blocked.",
          hint: "Retry; if it persists, report it along with the diagnostic code below.",
        },
        weapon_source_not_found: {
          message: "No recognizable weapon assets were found in this mod.",
          hint: "Confirm it is an MHW:I weapon mod with a nativePC/wp structure.",
        },
        weapon_multiple_source_roots: {
          message: "This archive contains multiple weapon asset sets; this version cannot retarget them automatically.",
          hint: "Split it into single weapon sets and import them separately.",
        },
        weapon_mixed_family: {
          message: "This archive mixes multiple weapon types; this version cannot retarget them automatically.",
          hint: "Split it by weapon type and import them separately.",
        },
        weapon_mixed_install_payload: {
          message: "Besides weapon assets, this archive contains other content that writes to the game; automatic retargeting was blocked.",
          hint: "Keep only the weapon assets and import again.",
        },
        weapon_incomplete_binary_pair: {
          message: "This weapon mod is missing a paired model or material file.",
          hint: "Re-download the complete mod package.",
        },
        weapon_unknown_part: {
          message: "This weapon mod contains parts this version cannot recognize.",
          hint: "This version only supports weapon bodies and known sub-parts (shield, sheath, off-hand, etc.).",
        },
        weapon_unsupported_resource: {
          message: "This weapon mod contains resource types not supported by this version.",
          hint: "This version only supports .mod3 and .mrl3 files.",
        },
        weapon_unsafe_path: {
          message: "The mod package contains unsafe file paths; processing was blocked.",
          hint: "Re-download the mod.",
        },
        weapon_duplicate_asset_path: {
          message: "The mod package contains duplicate file paths.",
          hint: "Re-download the mod.",
        },
        weapon_case_insensitive_path_collision: {
          message: "The mod package contains file paths that conflict only by letter case.",
          hint: "Re-download the mod.",
        },
        weapon_invalid_package_file_id: {
          message: "The import record of this mod is no longer valid.",
          hint: "Re-import the mod.",
        },
        weapon_duplicate_package_file_id: {
          message: "The import record of this mod contains duplicate entries.",
          hint: "Re-import the mod.",
        },
        weapon_identity_invalid: {
          message: "The identity of this weapon asset could not be confirmed; processing was blocked.",
          hint: "Re-import the mod.",
        },
        weapon_cross_family_target: {
          message: "The selected target is a different weapon type from this mod; cross-type retargeting is not supported in this version.",
          hint: "Choose a target of the same weapon type.",
        },
        weapon_unknown_family: {
          message: "This weapon type is outside the currently supported range.",
          hint: "This version supports the 14 MHW:I weapon types.",
        },
        weapon_invalid_main_id: {
          message: "This weapon asset ID does not follow the game's convention; processing was blocked.",
          hint: "Re-download the mod.",
        },
      },
      generic: {
        replacement_mod_not_found: { message: "The imported mod was not found." },
        replacement_package_unavailable: { message: "The import package is currently unavailable." },
        replacement_source_not_retargetable: {
          message: "This mod is not a single-source appearance package that can be handled automatically.",
        },
        replacement_target_catalog_unavailable: { message: "The replacement target catalog is temporarily unavailable." },
        replacement_analysis_unavailable: { message: "Replacement analysis is temporarily unavailable." },
        weapon_source_content_unavailable: {
          message: "The managed weapon assets could not be read.",
          hint: "Re-import the mod.",
        },
        replacement_target_not_found: { message: "The selected replacement target no longer exists." },
        replacement_install_state_unavailable: { message: "The current install status could not be confirmed." },
        replacement_initial_install_blocked: {
          message: "The current install or recovery status does not allow a first replacement install.",
        },
        replacement_installed_binding_unavailable: {
          message: "The currently installed replacement target could not be confirmed.",
        },
        replacement_target_already_selected: { message: "The current target is already installed." },
        replacement_preview_unavailable: { message: "Replacement preview is temporarily unavailable." },
        replacement_install_manifest_unavailable: {
          message: "The install manifest is temporarily unavailable; write safety cannot be confirmed.",
          hint: "Refresh and retry later; if it persists, resolve pending recovery items in the recovery center first.",
        },
        replacement_reinstall_preview_unavailable: {
          message: "Complete target switch preview info could not be read.",
          hint: "Refresh and retry; if it persists, report it along with the current mod and target.",
        },
        replacement_mod_id_invalid: {
          message: "Invalid mod identifier; the mod could not be located.",
          hint: "Refresh the mod list and try again.",
        },
        replacement_profile_id_invalid: {
          message: "Invalid profile identifier.",
          hint: "Re-select the profile and try again.",
        },
        replacement_target_id_invalid: {
          message: "Invalid replacement target identifier.",
          hint: "Re-select the target and try again.",
        },
        replacement_layer_invalid: {
          message: "Invalid install layer parameter; this operation was blocked.",
          hint: "Refresh and retry; if it persists, please report it.",
        },
        plan_token_invalid: {
          message: "The target switch preview has expired.",
          hint: "Regenerate the preview.",
        },
        task_cannot_be_cancelled: {
          message: "The task has entered the commit phase and can no longer be cancelled.",
          hint: "Wait for the result.",
        },
        task_not_found: { message: "The current task could not be confirmed.", hint: "Refresh the status." },
        reinstall_catalog_unavailable: { message: "Target switch preview is temporarily unavailable." },
        reinstall_manifest_unavailable: { message: "Target switch preview is temporarily unavailable." },
        reinstall_recovery_unavailable: { message: "Target switch preview is temporarily unavailable." },
        reinstall_candidate_plan_unavailable: { message: "Target switch preview is temporarily unavailable." },
        replacement_unsupported_game: { message: "The current game does not support replacement targets." },
      },
      diagnostic: (code: string) => ` (diagnostic code: ${code})`,
    },
    panel: {
      analyzing: "Analyzing replacement assets",
      retry: "Retry",
      detectionTitle: "Detection Result",
      resourceCount: (count: number) => `${count} asset${count === 1 ? "" : "s"}`,
      noSources: "No replaceable appearance slots detected.",
      warningsAria: "Analysis warnings",
      targetsTitle: "Replacement Targets",
      targetCount: (count: number) => `${count} item${count === 1 ? "" : "s"}`,
      searchAria: "Search replacement targets",
      searchPlaceholder: "Search names, aliases, or slots",
      targetsAria: "Replacement targets",
      currentInstalled: "Currently installed",
      noMatches: "No matching replacement targets.",
      previewLoading: "Generating preview",
      switchPreviewTitle: "Target Switch Preview",
      initialPreviewTitle: "Write Preview",
      actionCount: (count: number) => `${count} action${count === 1 ? "" : "s"}`,
      factResourceType: "Asset type",
      factTargetId: "Target ID",
      factActions: "Write actions",
      blockingConflicts: (count: number) => `${count} blocking conflict${count === 1 ? "" : "s"} detected`,
      noBlockingConflicts: "No blocking conflicts detected",
      blockingConflictHint: "The target files may already be occupied by another installed mod. Uninstall the mod that owns this target first, then retry.",
      prerequisiteResultsAria: "Install prerequisite check results",
      countRetained: "Retained",
      countReplaced: "Replaced",
      countAdded: "Added",
      countStale: "Stale removed",
      preflightPassed: "Safety preflight passed",
      switchBlockedAria: "Target switch blockers",
      candidateAlreadyInstalled: "The current target is already installed",
      listenerUnavailable: "Task status listener unavailable",
      retryListener: "Retry listener",
      startingInstall: "Starting install task",
      cancelling: "Cancelling",
      cancelTask: "Cancel task",
      refreshing: "Refreshing install status",
      retryRefresh: "Retry refresh",
      previewSwitch: "Preview target switch",
      generatePreview: "Generate preview",
      confirmSwitch: "Confirm reinstall and switch",
      installToTarget: "Install to this target",
    },
  },
  ja: {
    warnings: {
      no_supported_assets: "対応する外観アセットが検出されませんでした",
      multiple_sources: "複数のソーススロットを検出しました。現在のバージョンでは自動分割されません",
      unsupported_source: "パッケージに現在のバージョンが対応していないソーススロットが含まれています",
      source_matches_target: "ソーススロットとターゲットスロットが同一です",
      weapon_partial_part_set: "この武器パッケージには一部のオプションパーツのみが含まれています。検出済みの完全なファイルペアのみ処理します",
    },
    block: {
      profileUnavailable: "現在のプロファイルは利用できません。",
      completedRefreshing: "書き込みが完了しました。インストール状態を更新しています。",
      cleanupPending: "現在のプロファイルには後処理待ちの再インストールトランザクションがあります。",
      rollbackRequired: "現在のプロファイルは先にインストールのロールバックを完了する必要があります。",
      repairRequired: "現在のプロファイルは先に手動修復を完了する必要があります。",
      statusUnknown: "インストール状態が不明のため、置換ターゲットの書き込みをブロックしました。",
    },
    phases: {
      "install.retarget.queued": "インストール待機中",
      "install.retarget.plan.building": "置換プランを再構築中",
      "install.retarget.commit.processing": "書き込みとマニフェスト記録中",
      "install.retarget.completed": "置換ターゲットのインストール完了",
      "install.retarget.failed": "置換ターゲットのインストール失敗",
      "install.cancelled": "置換ターゲットのインストールをキャンセルしました",
      "install.reinstall.queued": "ターゲット切替待機中",
      "install.reinstall.plan.building": "ターゲット切替プランを生成中",
      "install.reinstall.preflight.processing": "コミット前チェックを実行中",
      "install.reinstall.commit.processing": "ターゲット切替をコミット中",
      "install.reinstall.rollback.processing": "元のターゲットを復元中",
      "install.reinstall.completed": "置換ターゲットの切替完了",
      "install.reinstall.failed": "置換ターゲットの切替失敗",
      "install.reinstall.cancelled": "置換ターゲットの切替をキャンセルしました",
    },
    events: {
      reinstallFailed: "ターゲット切替に失敗しました。状態を更新してプレビューを再生成してください",
      retargetFailed: "置換ターゲットのインストールに失敗しました。状態を更新して再試行してください",
      refreshFailed: "インストールは完了しましたが、状態の更新に失敗しました。再試行してください。",
      invalidTaskType: "バックエンドが無効なタスク種別を返しました",
      startFailed: "置換ターゲット書き込みタスクの起動に失敗しました",
      invalidCancelResult: "バックエンドが無効なキャンセル結果を返しました。タスク状態の更新をお待ちください",
      cancelFailed: "タスクをキャンセルできません。実行結果をお待ちください",
      analysisFallback: "置換ターゲット情報の読み込みに失敗しました",
      previewFallback: "置換ターゲットのプレビューに失敗しました",
    },
    errors: {
      weapon: {
        weapon_binary_format_invalid: {
          message: "この武器 Mod のモデルファイル形式を認識できません。Iceborne 版でないか、ファイルが破損している可能性があります。",
          hint: "Mod を再ダウンロードするか、元の武器にそのままインストールしてください。",
        },
        weapon_binary_pair_incompatible: {
          message: "この武器 Mod のモデルとマテリアルのファイルが一致せず、他の武器へ安全に変更できません。",
          hint: "Mod を再ダウンロードするか、元の武器にそのままインストールしてください。",
        },
        weapon_binary_reference_unsafe: {
          message: "この武器 Mod のマテリアル参照に安全でないパスが含まれるため、書き込みをブロックしました。",
          hint: "Mod 作者に報告するか、元の武器にそのままインストールしてください。",
        },
        weapon_binary_reference_ambiguous: {
          message: "この武器 Mod のマテリアル参照をターゲット武器へ一意に対応付けできないため、書き込みをブロックしました。",
          hint: "Mod 作者に報告するか、元の武器にそのままインストールしてください。",
        },
        weapon_binary_path_too_long: {
          message: "このターゲットへ変更するとマテリアルパスがゲームの形式上限を超えるため、書き込みをブロックしました。",
          hint: "別のターゲット武器を選んで再試行してください。",
        },
        weapon_transformer_output_invalid: {
          message: "変換結果が整合性検証を通過しなかったため、ゲームディレクトリへの書き込みをブロックしました。",
          hint: "再試行してください。継続する場合は下の診断コードと併せて報告してください。",
        },
        weapon_source_not_found: {
          message: "この Mod 内に認識可能な武器アセットが見つかりませんでした。",
          hint: "nativePC/wp 構造の MHW:I 武器 Mod であることを確認してください。",
        },
        weapon_multiple_source_roots: {
          message: "このアーカイブには複数の武器アセット一式が含まれており、現在のバージョンでは自動リターゲットできません。",
          hint: "武器一式ごとに分割してから個別にインポートしてください。",
        },
        weapon_mixed_family: {
          message: "このアーカイブには複数の武器種が混在しており、現在のバージョンでは自動リターゲットできません。",
          hint: "武器種ごとに分割してから個別にインポートしてください。",
        },
        weapon_mixed_install_payload: {
          message: "このアーカイブには武器アセット以外にゲームへ書き込まれる内容が含まれるため、自動リターゲットをブロックしました。",
          hint: "武器アセットのみを残して再インポートしてください。",
        },
        weapon_incomplete_binary_pair: {
          message: "この武器 Mod にはペアになるモデルまたはマテリアルのファイルが不足しています。",
          hint: "完全な Mod パッケージを再ダウンロードしてください。",
        },
        weapon_unknown_part: {
          message: "この武器 Mod には現在のバージョンで認識できないパーツが含まれています。",
          hint: "現在のバージョンは武器本体と既知のサブパーツ（盾・鞘・オフハンドなど）のみ対応しています。",
        },
        weapon_unsupported_resource: {
          message: "この武器 Mod には現在のバージョンが対応していないリソース種別が含まれています。",
          hint: "現在のバージョンは .mod3 と .mrl3 ファイルのみ対応しています。",
        },
        weapon_unsafe_path: {
          message: "Mod パッケージ内に安全でないファイルパスが存在するため、処理をブロックしました。",
          hint: "Mod を再ダウンロードしてください。",
        },
        weapon_duplicate_asset_path: {
          message: "Mod パッケージ内に重複するファイルパスが存在します。",
          hint: "Mod を再ダウンロードしてください。",
        },
        weapon_case_insensitive_path_collision: {
          message: "Mod パッケージ内に大文字小文字のみ異なる競合ファイルパスが存在します。",
          hint: "Mod を再ダウンロードしてください。",
        },
        weapon_invalid_package_file_id: {
          message: "この Mod のインポート記録は失効しています。",
          hint: "Mod を再インポートしてください。",
        },
        weapon_duplicate_package_file_id: {
          message: "この Mod のインポート記録に重複エントリがあります。",
          hint: "Mod を再インポートしてください。",
        },
        weapon_identity_invalid: {
          message: "この武器アセットの同一性を確認できないため、処理をブロックしました。",
          hint: "Mod を再インポートしてください。",
        },
        weapon_cross_family_target: {
          message: "選択したターゲットはこの Mod と武器種が異なります。現在のバージョンは武器種をまたぐ改装に対応していません。",
          hint: "同じ武器種のターゲットを選んでください。",
        },
        weapon_unknown_family: {
          message: "この武器種は現在の対応範囲外です。",
          hint: "現在のバージョンは MHW:I の 14 武器種に対応しています。",
        },
        weapon_invalid_main_id: {
          message: "この武器アセットの ID がゲームの規約に従っていないため、処理をブロックしました。",
          hint: "Mod を再ダウンロードしてください。",
        },
      },
      generic: {
        replacement_mod_not_found: { message: "インポート済みの Mod が見つかりませんでした。" },
        replacement_package_unavailable: { message: "インポートパッケージは現在利用できません。" },
        replacement_source_not_retargetable: {
          message: "この Mod は現在自動処理できる単一ソースの外観パッケージではありません。",
        },
        replacement_target_catalog_unavailable: { message: "置換ターゲットのカタログは一時的に利用できません。" },
        replacement_analysis_unavailable: { message: "置換分析は一時的に利用できません。" },
        weapon_source_content_unavailable: {
          message: "管理下の武器アセットを読み取れませんでした。",
          hint: "Mod を再インポートしてください。",
        },
        replacement_target_not_found: { message: "選択した置換ターゲットは既に存在しません。" },
        replacement_install_state_unavailable: { message: "現在のインストール状態を確認できません。" },
        replacement_initial_install_blocked: {
          message: "現在のインストール／復旧状態では初回の置換インストールを実行できません。",
        },
        replacement_installed_binding_unavailable: {
          message: "現在インストール済みの置換ターゲットを確認できません。",
        },
        replacement_target_already_selected: { message: "現在のターゲットは既にインストール済みです。" },
        replacement_preview_unavailable: { message: "置換プレビューは一時的に利用できません。" },
        replacement_install_manifest_unavailable: {
          message: "インストールマニフェストが一時的に利用できず、書き込みの安全性を確認できません。",
          hint: "しばらくしてから更新して再試行してください。継続する場合は先にリカバリーセンターで未完了の復旧を処理してください。",
        },
        replacement_reinstall_preview_unavailable: {
          message: "ターゲット切替プレビューの完全な情報を読み取れませんでした。",
          hint: "更新して再試行してください。継続する場合は現在の Mod とターゲットと併せて報告してください。",
        },
        replacement_mod_id_invalid: {
          message: "Mod 識別子が無効なため、Mod を特定できません。",
          hint: "Mod リストを更新して再試行してください。",
        },
        replacement_profile_id_invalid: {
          message: "プロファイル識別子が無効です。",
          hint: "プロファイルを選び直して再試行してください。",
        },
        replacement_target_id_invalid: {
          message: "置換ターゲット識別子が無効です。",
          hint: "ターゲットを選び直して再試行してください。",
        },
        replacement_layer_invalid: {
          message: "インストールレイヤーのパラメータが無効なため、今回の操作をブロックしました。",
          hint: "更新して再試行してください。継続する場合は報告してください。",
        },
        plan_token_invalid: {
          message: "ターゲット切替プレビューは失効しました。",
          hint: "プレビューを再生成してください。",
        },
        task_cannot_be_cancelled: {
          message: "タスクはコミット段階に入っており、キャンセルできません。",
          hint: "実行結果をお待ちください。",
        },
        task_not_found: { message: "現在のタスクを確認できません。", hint: "状態を更新してください。" },
        reinstall_catalog_unavailable: { message: "ターゲット切替プレビューは一時的に利用できません。" },
        reinstall_manifest_unavailable: { message: "ターゲット切替プレビューは一時的に利用できません。" },
        reinstall_recovery_unavailable: { message: "ターゲット切替プレビューは一時的に利用できません。" },
        reinstall_candidate_plan_unavailable: { message: "ターゲット切替プレビューは一時的に利用できません。" },
        replacement_unsupported_game: { message: "現在のゲームは置換ターゲットに対応していません。" },
      },
      diagnostic: (code: string) => `（診断コード：${code}）`,
    },
    panel: {
      analyzing: "置換アセットを分析中",
      retry: "再試行",
      detectionTitle: "検出結果",
      resourceCount: (count: number) => `${count} 件のアセット`,
      noSources: "置換可能な外観スロットは検出されませんでした。",
      warningsAria: "分析の警告",
      targetsTitle: "置換ターゲット",
      targetCount: (count: number) => `${count} 件`,
      searchAria: "置換ターゲットを検索",
      searchPlaceholder: "名前・別名・スロットで検索",
      targetsAria: "置換ターゲット",
      currentInstalled: "インストール済み",
      noMatches: "一致する置換ターゲットがありません。",
      previewLoading: "プレビューを生成中",
      switchPreviewTitle: "ターゲット切替プレビュー",
      initialPreviewTitle: "書き込みプレビュー",
      actionCount: (count: number) => `${count} 件のアクション`,
      factResourceType: "アセット種別",
      factTargetId: "ターゲット ID",
      factActions: "書き込みアクション",
      blockingConflicts: (count: number) => `${count} 件のブロッキング競合を検出`,
      noBlockingConflicts: "ブロッキング競合は検出されませんでした",
      blockingConflictHint: "対象ファイルは他のインストール済み Mod が使用中の可能性があります。先にその Mod をアンインストールしてから再試行してください。",
      prerequisiteResultsAria: "インストール前提チェック結果",
      countRetained: "保持",
      countReplaced: "置換",
      countAdded: "追加",
      countStale: "旧項目の削除",
      preflightPassed: "安全プリフライトを通過",
      switchBlockedAria: "ターゲット切替のブロック項目",
      candidateAlreadyInstalled: "現在のターゲットは既にインストール済みです",
      listenerUnavailable: "タスク状態リスナーを利用できません",
      retryListener: "リスナーを再試行",
      startingInstall: "インストールタスクを起動中",
      cancelling: "キャンセル中",
      cancelTask: "タスクをキャンセル",
      refreshing: "インストール状態を更新中",
      retryRefresh: "更新を再試行",
      previewSwitch: "ターゲット切替をプレビュー",
      generatePreview: "プレビューを生成",
      confirmSwitch: "再インストールして切替を確定",
      installToTarget: "このターゲットへインストール",
    },
  },
} satisfies LocaleDictionary<ReplacementCopy>;
