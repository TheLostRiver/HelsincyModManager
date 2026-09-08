import type { LocaleDictionary } from "../../shared/i18n";
import type { ModImportArchiveKeptCode } from "./modImportTaskState";

// Mod 导入动作与任务阶段文案（I18N-02）。阶段 key 与后端事件名一一对应，
// 语义判断（阶段推进、失败识别）留在 modImportTaskState，这里只有文本。

export type ModImportCopy = {
  errors: {
    invalidArchive: string;
    startFailed: string;
    pickerFailed: string;
    invalidStartState: string;
    storageFrozenMigration: string;
    storageFrozenRestart: string;
    unsupportedArchiveFormat: string;
    notAnArchive: string;
    archiveEncrypted: string;
    archiveMultiVolume: string;
  };
  dialog: {
    revisionTitle: string;
    newTitle: string;
    archiveFilterName: string;
  };
  action: {
    pickArchive: string;
    starting: string;
    preparing: string;
    reconnectRevision: string;
    reconnectImport: string;
    continueRevision: string;
    continueImport: string;
    retryRevision: string;
    retryImport: string;
  };
  status: {
    waitingArchive: string;
    creatingTask: string;
    revisionDone: string;
    importDone: string;
    cancelled: string;
    running: string;
    unavailable: string;
    listenerFailedHint: string;
  };
  phases: {
    queued: string;
    cancelled: string;
    unpackStarted: string;
    unpackCompleted: string;
    unpackFailed: string;
    previewImageProcessing: string;
    previewImageFallback: string;
    analyzeProcessing: string;
    commitProcessing: string;
    prepareCompleted: string;
    importing: string;
    failedRetryHint: string;
  };
  /** T22 / #366：拖拽导入的清单浮层。 */
  drop: {
    hint: string;
    title: string;
    checking: (count: number) => string;
    selectAll: string;
    selectedSummary: (selected: number, total: number) => string;
    blockedSummary: (count: number) => string;
    confirm: string;
    cancel: string;
    close: string;
    nothingImportable: string;
    warnNoGameContent: string;
    tooMany: (count: number, limit: number) => string;
    running: (index: number, total: number) => string;
    doneAllSucceeded: (count: number) => string;
    donePartial: (succeeded: number, failed: number) => string;
    doneAllFailed: (count: number) => string;
    previewFailed: string;
  };
  /** #275 ④：导入成功但源压缩包没删时的提示，按 mod_import_archive_kept_* 码取词。 */
  archiveKept: Record<ModImportArchiveKeptCode, string>;
  toasts: {
    archiveKeptTitle: string;
    revisionDoneTitle: string;
    revisionDoneMessage: string;
    importDoneTitle: string;
    importDoneMessage: string;
    refreshFailedTitle: string;
    refreshFailedMessage: string;
    importFailedTitle: string;
    importCancelledTitle: string;
    importCancelledMessage: string;
    importingRevisionTitle: string;
    importingTitle: string;
  };
};

export const modImportCopy = {
  zh_cn: {
    errors: {
      invalidArchive: "请选择有效的本地压缩包",
      startFailed: "无法启动导入任务",
      pickerFailed: "无法打开文件选择器",
      invalidStartState: "导入任务返回了无效状态",
      storageFrozenMigration: "存储目录正在迁移，完成后再导入",
      storageFrozenRestart: "存储目录已更改，请先重启 HMM",
      unsupportedArchiveFormat: "HMM 目前支持 ZIP、RAR、7Z 压缩包，请先转成其中一种再导入",
      notAnArchive: "这个文件不是压缩包，请选择 Mod 的压缩包",
      archiveEncrypted: "这个压缩包有密码，HMM 无法解开。请先解压去掉密码，再重新打包导入",
      archiveMultiVolume: "这是分卷压缩包，HMM 只拿到了其中一卷。请先在本地解压合并，再重新打包导入",
    },
    dialog: {
      revisionTitle: "选择新版本压缩包",
      newTitle: "选择 Mod 压缩包",
      archiveFilterName: "压缩包（ZIP / RAR / 7Z）",
    },
    action: {
      pickArchive: "选择压缩包...",
      starting: "启动导入...",
      preparing: "准备导入...",
      reconnectRevision: "导入新版本",
      reconnectImport: "导入 Mod",
      continueRevision: "继续导入新版本",
      continueImport: "继续导入 Mod",
      retryRevision: "重试导入新版本",
      retryImport: "重试导入 Mod",
    },
    status: {
      waitingArchive: "等待选择压缩包",
      creatingTask: "正在创建导入任务",
      revisionDone: "新版本导入完成，版本列表已更新",
      importDone: "导入完成，Mod 列表将自动刷新",
      cancelled: "导入已取消",
      running: "正在执行导入任务",
      unavailable: "导入任务状态不可用",
      listenerFailedHint: "导入服务暂时不可用，点击后将自动重连并继续",
    },
    phases: {
      queued: "等待导入",
      cancelled: "导入已取消",
      unpackStarted: "正在安全解包",
      unpackCompleted: "安全解包完成",
      unpackFailed: "安全解包失败",
      previewImageProcessing: "正在处理预览图",
      previewImageFallback: "预览图已使用回退方案",
      analyzeProcessing: "正在分析 Mod",
      commitProcessing: "正在保存导入结果",
      prepareCompleted: "导入完成",
      importing: "正在导入",
      failedRetryHint: "导入失败，请检查压缩包后重试",
    },
    drop: {
      hint: "松开鼠标，加入待导入清单",
      title: "待导入的压缩包",
      checking: (count) => `正在检查 ${count} 个文件…`,
      selectAll: "全选",
      selectedSummary: (selected, total) => `已选 ${selected} / ${total} 个`,
      blockedSummary: (count) => `${count} 个读不了，已跳过`,
      confirm: "开始导入",
      cancel: "取消",
      close: "关闭",
      nothingImportable: "这些文件都没法导入，换一批再试。",
      tooMany: (count, limit) => `一次最多拖 ${limit} 个，这次拖了 ${count} 个。分几批来吧。`,
      warnNoGameContent: "没找到本游戏的内容目录，可能装不出东西。确认没问题就勾上，照样能导入。",
      running: (index, total) => `正在导入第 ${index} / ${total} 个…`,
      doneAllSucceeded: (count) => `${count} 个 Mod 已导入。`,
      donePartial: (succeeded, failed) => `${succeeded} 个已导入，${failed} 个失败。`,
      doneAllFailed: (count) => `${count} 个都没能导入。`,
      previewFailed: "检查拖入的文件时出错了，重新拖一次试试。",
    },
    archiveKept: {
      mod_import_archive_kept_not_regular_file: "原始压缩包不是普通文件（目录、链接或联接点），已保留。",
      mod_import_archive_kept_protected_location: "原始压缩包位于游戏目录、Mod 存储目录或应用数据目录内，已保留。",
      mod_import_archive_kept_changed: "原始压缩包在导入期间被替换或已不存在，未删除任何文件。",
      mod_import_archive_kept_unavailable: "无法确认原始压缩包的状态，已保留。",
      mod_import_archive_kept_remove_failed: "原始压缩包删除失败（可能被占用或没有权限），已保留。",
    },
    toasts: {
      archiveKeptTitle: "已导入，原始压缩包未删除",
      revisionDoneTitle: "新版本导入完成",
      revisionDoneMessage: "版本列表已更新。",
      importDoneTitle: "Mod 导入完成",
      importDoneMessage: "Mod 列表已更新。",
      refreshFailedTitle: "导入完成，列表刷新失败",
      refreshFailedMessage: "文件已导入，但当前列表未能刷新，请重新扫描或稍后重试。",
      importFailedTitle: "Mod 导入失败",
      importCancelledTitle: "Mod 导入已取消",
      importCancelledMessage: "未继续写入新的 Mod 版本。",
      importingRevisionTitle: "正在导入新版本",
      importingTitle: "正在导入 Mod",
    },
  },
  en: {
    errors: {
      invalidArchive: "Choose a valid local archive",
      startFailed: "Cannot start the import task",
      pickerFailed: "Cannot open the file picker",
      invalidStartState: "The import task returned an invalid state",
      storageFrozenMigration: "The storage directory is being migrated; import after it finishes",
      storageFrozenRestart: "The storage directory changed; restart HMM first",
      unsupportedArchiveFormat:
        "HMM supports ZIP, RAR and 7Z archives. Convert the file to one of them and import it again.",
      notAnArchive: "This file is not an archive. Choose the mod's archive instead.",
      archiveEncrypted:
        "This archive is password protected and HMM cannot open it. Extract it, remove the password, repack and import again.",
      archiveMultiVolume:
        "This is a multi-volume archive and HMM only received one volume. Extract it locally, repack it as a single archive and import again.",
    },
    dialog: {
      revisionTitle: "Choose the new version's archive",
      newTitle: "Choose the mod's archive",
      archiveFilterName: "Archive (ZIP / RAR / 7Z)",
    },
    action: {
      pickArchive: "Choose archive…",
      starting: "Starting import…",
      preparing: "Preparing import…",
      reconnectRevision: "Import new version",
      reconnectImport: "Import mod",
      continueRevision: "Continue importing the new version",
      continueImport: "Continue importing the mod",
      retryRevision: "Retry importing the new version",
      retryImport: "Retry importing the mod",
    },
    status: {
      waitingArchive: "Waiting for an archive",
      creatingTask: "Creating the import task",
      revisionDone: "New version imported; the version list is updated",
      importDone: "Import finished; the mod list refreshes automatically",
      cancelled: "Import cancelled",
      running: "Running the import task",
      unavailable: "Import task status unavailable",
      listenerFailedHint: "The import service is temporarily unavailable; click to reconnect and continue",
    },
    phases: {
      queued: "Waiting to import",
      cancelled: "Import cancelled",
      unpackStarted: "Safely unpacking",
      unpackCompleted: "Safe unpack finished",
      unpackFailed: "Safe unpack failed",
      previewImageProcessing: "Processing the preview image",
      previewImageFallback: "Preview image used a fallback",
      analyzeProcessing: "Analyzing the mod",
      commitProcessing: "Saving the import result",
      prepareCompleted: "Import finished",
      importing: "Importing",
      failedRetryHint: "Import failed. Check the archive and retry.",
    },
    drop: {
      hint: "Drop to add these to the import list",
      title: "Archives to import",
      checking: (count) => `Checking ${count} file(s)…`,
      selectAll: "Select all",
      selectedSummary: (selected, total) => `${selected} of ${total} selected`,
      blockedSummary: (count) => `${count} unreadable, skipped`,
      confirm: "Start import",
      cancel: "Cancel",
      close: "Close",
      nothingImportable: "None of these files can be imported. Try a different set.",
      tooMany: (count, limit) => `Up to ${limit} files at a time; you dropped ${count}. Try smaller batches.`,
      warnNoGameContent: "No game content folder found — this may install nothing. Tick it anyway if you know it is fine.",
      running: (index, total) => `Importing ${index} of ${total}…`,
      doneAllSucceeded: (count) => `${count} mod(s) imported.`,
      donePartial: (succeeded, failed) => `${succeeded} imported, ${failed} failed.`,
      doneAllFailed: (count) => `None of the ${count} could be imported.`,
      previewFailed: "Could not check the dropped files. Try dropping them again.",
    },
    archiveKept: {
      mod_import_archive_kept_not_regular_file: "The original archive is not a regular file (directory, link or junction); it was kept.",
      mod_import_archive_kept_protected_location: "The original archive lies inside the game, mod storage or app data directory; it was kept.",
      mod_import_archive_kept_changed: "The original archive was replaced or removed during the import; nothing was deleted.",
      mod_import_archive_kept_unavailable: "The original archive could not be checked; it was kept.",
      mod_import_archive_kept_remove_failed: "The original archive could not be deleted (in use or no permission); it was kept.",
    },
    toasts: {
      archiveKeptTitle: "Imported; the original archive was kept",
      revisionDoneTitle: "New version imported",
      revisionDoneMessage: "The version list is updated.",
      importDoneTitle: "Mod imported",
      importDoneMessage: "The mod list is updated.",
      refreshFailedTitle: "Imported, but the list failed to refresh",
      refreshFailedMessage:
        "The file was imported, but the list could not refresh. Rescan or try again later.",
      importFailedTitle: "Mod import failed",
      importCancelledTitle: "Mod import cancelled",
      importCancelledMessage: "No new mod version was written.",
      importingRevisionTitle: "Importing the new version",
      importingTitle: "Importing the mod",
    },
  },
  ja: {
    errors: {
      invalidArchive: "有効なローカルアーカイブを選択してください",
      startFailed: "インポートタスクを開始できません",
      pickerFailed: "ファイル選択ダイアログを開けません",
      invalidStartState: "インポートタスクが無効な状態を返しました",
      storageFrozenMigration: "保存フォルダーの移行中です。完了後にインポートしてください",
      storageFrozenRestart: "保存フォルダーが変更されました。先に HMM を再起動してください",
      unsupportedArchiveFormat:
        "HMM は ZIP・RAR・7Z アーカイブに対応しています。いずれかに変換してからインポートしてください",
      notAnArchive: "このファイルはアーカイブではありません。Mod のアーカイブを選択してください",
      archiveEncrypted:
        "このアーカイブにはパスワードが設定されており、HMM では開けません。展開してパスワードを解除し、再圧縮してからインポートしてください",
      archiveMultiVolume:
        "これは分割アーカイブで、HMM は 1 巻しか受け取っていません。ローカルで展開・結合し、単一のアーカイブに再圧縮してからインポートしてください",
    },
    dialog: {
      revisionTitle: "新バージョンのアーカイブを選択",
      newTitle: "Mod のアーカイブを選択",
      archiveFilterName: "アーカイブ（ZIP / RAR / 7Z）",
    },
    action: {
      pickArchive: "アーカイブを選択…",
      starting: "インポートを開始…",
      preparing: "インポートを準備中…",
      reconnectRevision: "新バージョンをインポート",
      reconnectImport: "Mod をインポート",
      continueRevision: "新バージョンのインポートを続行",
      continueImport: "Mod のインポートを続行",
      retryRevision: "新バージョンのインポートを再試行",
      retryImport: "Mod のインポートを再試行",
    },
    status: {
      waitingArchive: "アーカイブの選択待ち",
      creatingTask: "インポートタスクを作成中",
      revisionDone: "新バージョンのインポートが完了し、バージョン一覧を更新しました",
      importDone: "インポートが完了しました。Mod リストは自動的に更新されます",
      cancelled: "インポートをキャンセルしました",
      running: "インポートタスクを実行中",
      unavailable: "インポートタスクの状態を取得できません",
      listenerFailedHint: "インポートサービスは一時的に利用できません。クリックすると自動再接続して続行します",
    },
    phases: {
      queued: "インポート待ち",
      cancelled: "インポートをキャンセルしました",
      unpackStarted: "安全に展開しています",
      unpackCompleted: "安全な展開が完了",
      unpackFailed: "安全な展開に失敗",
      previewImageProcessing: "プレビュー画像を処理中",
      previewImageFallback: "プレビュー画像はフォールバックを使用",
      analyzeProcessing: "Mod を解析中",
      commitProcessing: "インポート結果を保存中",
      prepareCompleted: "インポート完了",
      importing: "インポート中",
      failedRetryHint: "インポートに失敗しました。アーカイブを確認して再試行してください。",
    },
    drop: {
      hint: "ドロップしてインポート一覧に追加",
      title: "インポートする書庫",
      checking: (count) => `${count} 件を確認しています…`,
      selectAll: "すべて選択",
      selectedSummary: (selected, total) => `${total} 件中 ${selected} 件を選択`,
      blockedSummary: (count) => `${count} 件は読み込めないためスキップ`,
      confirm: "インポート開始",
      cancel: "キャンセル",
      close: "閉じる",
      nothingImportable: "どのファイルもインポートできません。別のファイルでお試しください。",
      tooMany: (count, limit) => `一度にドロップできるのは ${limit} 件までです（今回は ${count} 件）。分けてお試しください。`,
      warnNoGameContent: "ゲームのコンテンツフォルダーが見つかりません。何もインストールされない可能性がありますが、問題なければチェックしてインポートできます。",
      running: (index, total) => `${total} 件中 ${index} 件目をインポート中…`,
      doneAllSucceeded: (count) => `${count} 件の Mod をインポートしました。`,
      donePartial: (succeeded, failed) => `${succeeded} 件成功、${failed} 件失敗。`,
      doneAllFailed: (count) => `${count} 件すべてインポートできませんでした。`,
      previewFailed: "ドロップしたファイルを確認できませんでした。もう一度ドロップしてください。",
    },
    archiveKept: {
      mod_import_archive_kept_not_regular_file: "元のアーカイブが通常のファイルではない（フォルダー、リンク、ジャンクション）ため保持しました。",
      mod_import_archive_kept_protected_location: "元のアーカイブがゲーム、Mod 保存、またはアプリデータのフォルダー内にあるため保持しました。",
      mod_import_archive_kept_changed: "元のアーカイブがインポート中に置き換えられたか存在しないため、何も削除していません。",
      mod_import_archive_kept_unavailable: "元のアーカイブの状態を確認できなかったため保持しました。",
      mod_import_archive_kept_remove_failed: "元のアーカイブを削除できなかった（使用中または権限なし）ため保持しました。",
    },
    toasts: {
      archiveKeptTitle: "インポート済み。元のアーカイブは保持されました",
      revisionDoneTitle: "新バージョンのインポートが完了",
      revisionDoneMessage: "バージョン一覧を更新しました。",
      importDoneTitle: "Mod のインポートが完了",
      importDoneMessage: "Mod リストを更新しました。",
      refreshFailedTitle: "インポート完了、一覧の更新に失敗",
      refreshFailedMessage:
        "ファイルはインポートされましたが、一覧を更新できませんでした。再スキャンするか、しばらくしてから再試行してください。",
      importFailedTitle: "Mod のインポートに失敗",
      importCancelledTitle: "Mod のインポートをキャンセル",
      importCancelledMessage: "新しい Mod バージョンは書き込まれていません。",
      importingRevisionTitle: "新バージョンをインポート中",
      importingTitle: "Mod をインポート中",
    },
  },
} satisfies LocaleDictionary<ModImportCopy>;
