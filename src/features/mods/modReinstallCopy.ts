import type { LocaleDictionary } from "../../shared/i18n";
import type { ReinstallFailurePhase, ReinstallTaskPhase } from "./modReinstallTaskState";
import type { ReinstallBlockingReason } from "./modReinstallTypes";

// 重装（同 Mod 换版本）任务与对话框的全部用户可见文案。
// phases / blockingReasons / failureMessages 保持 Record<union, string> 穷尽约束，
// 新增语义码而缺文案时 tsc 失败。

export type ModReinstallCopy = {
  task: {
    phases: Record<ReinstallTaskPhase, string>;
    blockingReasons: Record<ReinstallBlockingReason, string>;
    failureMessages: Record<ReinstallFailurePhase, string>;
    failedFallback: string;
    previewErrors: {
      gameUnsupported: string;
      requestInvalid: string;
      unavailable: string;
    };
    startErrors: {
      planTokenInvalid: string;
      startFailed: string;
    };
  };
  dialog: {
    title: string;
    closeAria: string;
    candidateTitle: string;
    revisionOrigin: (originRevisionId: string, displayRevisionId: string) => string;
    candidateAria: string;
    generatePreview: string;
    loadingCatalog: string;
    loadingPreview: string;
    listenerConnecting: string;
    listenerFailed: string;
    retryListener: string;
    close: string;
    confirm: string;
    starting: string;
    completed: string;
    cancelled: string;
    summaryAria: string;
    codeSeparator: string;
    currentRevision: (revisionId: string) => string;
    currentRevisionUnknown: string;
    candidateRevision: (revisionId: string) => string;
    candidateRevisionUnavailable: string;
    countRetained: string;
    countReplaced: string;
    countAdded: string;
    countStale: string;
    preflightPassed: string;
    blockedNotice: string;
    blockingDetails: {
      candidateNotFound: string;
      previewStale: string;
    };
    cleanupPending: {
      committed: string;
      rollbackRequired: string;
      repairRequired: string;
      statusUnknown: string;
    };
  };
  workflow: {
    noCandidate: string;
    catalogLoadFailed: string;
    invalidTaskState: string;
  };
};

export const modReinstallCopy = {
  zh_cn: {
    task: {
      phases: {
        "install.reinstall.queued": "等待重装",
        "install.reinstall.plan.building": "生成重装计划",
        "install.reinstall.preflight.processing": "执行提交前检查",
        "install.reinstall.commit.processing": "提交新版本",
        "install.reinstall.rollback.processing": "恢复原版本",
        "install.reinstall.completed": "重装完成",
        "install.reinstall.failed": "重装失败",
        "install.reinstall.cancelled": "重装已取消",
      },
      blockingReasons: {
        prerequisites_blocked: "游戏前置未就绪",
        not_installed: "当前 Mod 尚未安装",
        candidate_not_found: "候选版本不存在",
        candidate_not_ready: "候选版本尚未准备完成",
        candidate_owner_mismatch: "候选版本不属于当前 Mod",
        candidate_already_installed: "候选版本已安装",
        manifest_state_unsafe: "当前安装状态不允许重装",
        installed_revision_unknown: "无法确认当前已安装版本",
        source_unavailable: "候选版本源文件不可用",
        target_missing: "受管目标文件缺失",
        target_changed: "受管目标文件已发生变化",
        target_read_failed: "无法读取受管目标文件",
        backup_missing: "所需备份缺失",
        backup_read_failed: "无法读取所需备份",
        plan_conflict: "重装计划存在冲突",
        cross_mod_target_conflict: "与其他 Mod 的目标文件冲突",
        preview_stale: "预览已过期，请重新生成",
      },
      failureMessages: {
        planning: "无法生成重装计划，请重试",
        preflight: "提交前检查失败，请重新生成预览",
        lock: "当前游戏或配置档正在执行其他写入任务",
        backup: "创建安全快照失败，未提交新版本",
        commit: "提交新版本失败，后端已尝试恢复原状态",
        manifest: "写入安装记录失败，后端已进入受控恢复流程",
        post_commit: "新版本已提交，但收尾尚未完成，请在恢复中心完成收敛",
        rollback: "恢复原版本失败，请在恢复中心处理",
        complete: "重装任务收尾失败，请刷新状态后重试",
      },
      failedFallback: "重装失败，请刷新状态后重试",
      previewErrors: {
        gameUnsupported: "当前游戏不支持重装",
        requestInvalid: "重装请求已失效，请重新选择",
        unavailable: "无法生成重装预览，请稍后重试",
      },
      startErrors: {
        planTokenInvalid: "重装预览已失效，请重新生成",
        startFailed: "无法启动重装任务，请重新生成预览后重试",
      },
    },
    dialog: {
      title: "重装 MOD",
      closeAria: "关闭",
      candidateTitle: "候选版本",
      revisionOrigin: (originRevisionId: string, displayRevisionId: string) =>
        `来源版本 ${originRevisionId} · 展示版本 ${displayRevisionId}`,
      candidateAria: "候选版本",
      generatePreview: "生成预览",
      loadingCatalog: "正在读取版本列表",
      loadingPreview: "正在生成安全预览",
      listenerConnecting: "正在连接任务状态",
      listenerFailed: "任务状态连接不可用，暂不能提交重装。",
      retryListener: "重试连接",
      close: "关闭",
      confirm: "确认重装",
      starting: "正在启动重装任务",
      completed: "重装完成",
      cancelled: "重装已取消",
      summaryAria: "重装计划摘要",
      codeSeparator: "、",
      currentRevision: (revisionId: string) => `当前 ${revisionId}`,
      currentRevisionUnknown: "当前 未知",
      candidateRevision: (revisionId: string) => `候选 ${revisionId}`,
      candidateRevisionUnavailable: "候选 不可用",
      countRetained: "保留",
      countReplaced: "替换",
      countAdded: "新增",
      countStale: "移除旧项",
      preflightPassed: "预检通过，可以提交重装。",
      blockedNotice: "当前预览存在阻断项。",
      blockingDetails: {
        candidateNotFound: "候选版本可能已被移除，请刷新版本列表。",
        previewStale: "重装事实已变化，请重新生成预览。",
      },
      cleanupPending: {
        committed: "新版本已提交，但收尾尚未完成。写入操作已暂停，请前往恢复中心完成收敛。",
        rollbackRequired: "当前重装需要受控恢复，写入操作已暂停。",
        repairRequired: "当前安装状态需要人工处理，写入操作已暂停。",
        statusUnknown: "无法确认当前安装状态，写入操作已暂停。",
      },
    },
    workflow: {
      noCandidate: "当前 MOD 还没有可用候选版本",
      catalogLoadFailed: "无法读取版本列表",
      invalidTaskState: "重装任务返回了无效状态",
    },
  },
  en: {
    task: {
      phases: {
        "install.reinstall.queued": "Waiting to reinstall",
        "install.reinstall.plan.building": "Building reinstall plan",
        "install.reinstall.preflight.processing": "Running pre-commit checks",
        "install.reinstall.commit.processing": "Committing new revision",
        "install.reinstall.rollback.processing": "Restoring original revision",
        "install.reinstall.completed": "Reinstall completed",
        "install.reinstall.failed": "Reinstall failed",
        "install.reinstall.cancelled": "Reinstall cancelled",
      },
      blockingReasons: {
        prerequisites_blocked: "Game prerequisites not ready",
        not_installed: "This mod is not installed yet",
        candidate_not_found: "Candidate revision does not exist",
        candidate_not_ready: "Candidate revision is not ready yet",
        candidate_owner_mismatch: "Candidate revision does not belong to this mod",
        candidate_already_installed: "Candidate revision is already installed",
        manifest_state_unsafe: "The current install state does not allow reinstalling",
        installed_revision_unknown: "The currently installed revision could not be confirmed",
        source_unavailable: "Candidate revision source files are unavailable",
        target_missing: "A managed target file is missing",
        target_changed: "A managed target file has changed",
        target_read_failed: "A managed target file could not be read",
        backup_missing: "A required backup is missing",
        backup_read_failed: "A required backup could not be read",
        plan_conflict: "The reinstall plan has conflicts",
        cross_mod_target_conflict: "Target files conflict with another mod",
        preview_stale: "The preview has expired; regenerate it",
      },
      failureMessages: {
        planning: "Failed to build the reinstall plan. Please retry",
        preflight: "Pre-commit checks failed. Regenerate the preview",
        lock: "The current game or profile is running another write task",
        backup: "Failed to create a safety snapshot; the new revision was not committed",
        commit: "Failed to commit the new revision; the backend attempted to restore the original state",
        manifest: "Failed to write the install record; the backend entered controlled recovery",
        post_commit: "The new revision was committed, but finalization is incomplete. Finish convergence in the Recovery Center",
        rollback: "Failed to restore the original revision. Handle it in the Recovery Center",
        complete: "Reinstall finalization failed. Refresh the status and retry",
      },
      failedFallback: "Reinstall failed. Refresh the status and retry",
      previewErrors: {
        gameUnsupported: "The current game does not support reinstalling",
        requestInvalid: "The reinstall request is no longer valid. Please re-select",
        unavailable: "Failed to generate the reinstall preview. Please try again later",
      },
      startErrors: {
        planTokenInvalid: "The reinstall preview has expired. Regenerate it",
        startFailed: "Failed to start the reinstall task. Regenerate the preview and retry",
      },
    },
    dialog: {
      title: "Reinstall Mod",
      closeAria: "Close",
      candidateTitle: "Candidate Revision",
      revisionOrigin: (originRevisionId: string, displayRevisionId: string) =>
        `Origin revision ${originRevisionId} · Display revision ${displayRevisionId}`,
      candidateAria: "Candidate revision",
      generatePreview: "Generate preview",
      loadingCatalog: "Loading revision list",
      loadingPreview: "Generating safety preview",
      listenerConnecting: "Connecting to task status",
      listenerFailed: "Task status connection unavailable; reinstall cannot be submitted yet.",
      retryListener: "Retry connection",
      close: "Close",
      confirm: "Confirm reinstall",
      starting: "Starting reinstall task",
      completed: "Reinstall completed",
      cancelled: "Reinstall cancelled",
      summaryAria: "Reinstall plan summary",
      codeSeparator: ", ",
      currentRevision: (revisionId: string) => `Current ${revisionId}`,
      currentRevisionUnknown: "Current unknown",
      candidateRevision: (revisionId: string) => `Candidate ${revisionId}`,
      candidateRevisionUnavailable: "Candidate unavailable",
      countRetained: "Retained",
      countReplaced: "Replaced",
      countAdded: "Added",
      countStale: "Stale removed",
      preflightPassed: "Preflight passed; the reinstall can be submitted.",
      blockedNotice: "The current preview has blockers.",
      blockingDetails: {
        candidateNotFound: "The candidate revision may have been removed. Refresh the revision list.",
        previewStale: "Reinstall facts have changed. Regenerate the preview.",
      },
      cleanupPending: {
        committed: "The new revision was committed, but finalization is incomplete. Writes are paused; finish convergence in the Recovery Center.",
        rollbackRequired: "This reinstall requires controlled recovery. Writes are paused.",
        repairRequired: "The current install state requires manual handling. Writes are paused.",
        statusUnknown: "The current install state could not be confirmed. Writes are paused.",
      },
    },
    workflow: {
      noCandidate: "This mod has no available candidate revisions yet",
      catalogLoadFailed: "Failed to load the revision list",
      invalidTaskState: "The reinstall task returned an invalid state",
    },
  },
  ja: {
    task: {
      phases: {
        "install.reinstall.queued": "再インストール待機中",
        "install.reinstall.plan.building": "再インストールプランを生成中",
        "install.reinstall.preflight.processing": "コミット前チェックを実行中",
        "install.reinstall.commit.processing": "新バージョンをコミット中",
        "install.reinstall.rollback.processing": "元のバージョンを復元中",
        "install.reinstall.completed": "再インストール完了",
        "install.reinstall.failed": "再インストール失敗",
        "install.reinstall.cancelled": "再インストールをキャンセルしました",
      },
      blockingReasons: {
        prerequisites_blocked: "ゲームの前提条件が未整備です",
        not_installed: "この Mod はまだインストールされていません",
        candidate_not_found: "候補バージョンが存在しません",
        candidate_not_ready: "候補バージョンの準備がまだ完了していません",
        candidate_owner_mismatch: "候補バージョンはこの Mod のものではありません",
        candidate_already_installed: "候補バージョンは既にインストール済みです",
        manifest_state_unsafe: "現在のインストール状態では再インストールできません",
        installed_revision_unknown: "現在インストール済みのバージョンを確認できません",
        source_unavailable: "候補バージョンのソースファイルを利用できません",
        target_missing: "管理下のターゲットファイルが見つかりません",
        target_changed: "管理下のターゲットファイルが変更されています",
        target_read_failed: "管理下のターゲットファイルを読み取れません",
        backup_missing: "必要なバックアップが見つかりません",
        backup_read_failed: "必要なバックアップを読み取れません",
        plan_conflict: "再インストールプランに競合があります",
        cross_mod_target_conflict: "他の Mod のターゲットファイルと競合しています",
        preview_stale: "プレビューが失効しました。再生成してください",
      },
      failureMessages: {
        planning: "再インストールプランを生成できませんでした。再試行してください",
        preflight: "コミット前チェックに失敗しました。プレビューを再生成してください",
        lock: "現在のゲームまたはプロファイルは別の書き込みタスクを実行中です",
        backup: "安全スナップショットの作成に失敗したため、新バージョンはコミットされていません",
        commit: "新バージョンのコミットに失敗しました。バックエンドが元の状態への復元を試みました",
        manifest: "インストール記録の書き込みに失敗しました。バックエンドは管理された復旧フローに入りました",
        post_commit: "新バージョンはコミットされましたが、後処理が未完了です。復旧センターで収束を完了してください",
        rollback: "元のバージョンの復元に失敗しました。復旧センターで対処してください",
        complete: "再インストールの後処理に失敗しました。状態を更新して再試行してください",
      },
      failedFallback: "再インストールに失敗しました。状態を更新して再試行してください",
      previewErrors: {
        gameUnsupported: "現在のゲームは再インストールに対応していません",
        requestInvalid: "再インストール要求は失効しました。選び直してください",
        unavailable: "再インストールプレビューを生成できません。しばらくしてから再試行してください",
      },
      startErrors: {
        planTokenInvalid: "再インストールプレビューは失効しました。再生成してください",
        startFailed: "再インストールタスクを起動できません。プレビューを再生成して再試行してください",
      },
    },
    dialog: {
      title: "MOD を再インストール",
      closeAria: "閉じる",
      candidateTitle: "候補バージョン",
      revisionOrigin: (originRevisionId: string, displayRevisionId: string) =>
        `取得元バージョン ${originRevisionId} · 表示バージョン ${displayRevisionId}`,
      candidateAria: "候補バージョン",
      generatePreview: "プレビューを生成",
      loadingCatalog: "バージョン一覧を読み込み中",
      loadingPreview: "安全プレビューを生成中",
      listenerConnecting: "タスク状態に接続中",
      listenerFailed: "タスク状態への接続を利用できないため、再インストールを送信できません。",
      retryListener: "接続を再試行",
      close: "閉じる",
      confirm: "再インストールを確定",
      starting: "再インストールタスクを起動中",
      completed: "再インストール完了",
      cancelled: "再インストールをキャンセルしました",
      summaryAria: "再インストールプランの概要",
      codeSeparator: "、",
      currentRevision: (revisionId: string) => `現在 ${revisionId}`,
      currentRevisionUnknown: "現在 不明",
      candidateRevision: (revisionId: string) => `候補 ${revisionId}`,
      candidateRevisionUnavailable: "候補 利用不可",
      countRetained: "保持",
      countReplaced: "置換",
      countAdded: "追加",
      countStale: "旧項目の削除",
      preflightPassed: "プリフライトを通過しました。再インストールを送信できます。",
      blockedNotice: "現在のプレビューにはブロック項目があります。",
      blockingDetails: {
        candidateNotFound: "候補バージョンは削除された可能性があります。バージョン一覧を更新してください。",
        previewStale: "再インストールの前提が変化しました。プレビューを再生成してください。",
      },
      cleanupPending: {
        committed: "新バージョンはコミットされましたが、後処理が未完了です。書き込みは一時停止中です。復旧センターで収束を完了してください。",
        rollbackRequired: "この再インストールには管理された復旧が必要です。書き込みは一時停止中です。",
        repairRequired: "現在のインストール状態は手動対応が必要です。書き込みは一時停止中です。",
        statusUnknown: "現在のインストール状態を確認できません。書き込みは一時停止中です。",
      },
    },
    workflow: {
      noCandidate: "この MOD にはまだ利用可能な候補バージョンがありません",
      catalogLoadFailed: "バージョン一覧を読み取れません",
      invalidTaskState: "再インストールタスクが無効な状態を返しました",
    },
  },
} satisfies LocaleDictionary<ModReinstallCopy>;
