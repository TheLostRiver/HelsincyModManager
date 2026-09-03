import type { LocaleDictionary } from "../../shared/i18n";

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
  };
  dialog: {
    revisionTitle: string;
    newTitle: string;
    zipFilterName: string;
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
  toasts: {
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
      invalidArchive: "请选择有效的本地 ZIP 压缩包",
      startFailed: "无法启动导入任务",
      pickerFailed: "无法打开文件选择器",
      invalidStartState: "导入任务返回了无效状态",
      storageFrozenMigration: "存储目录正在迁移，完成后再导入",
      storageFrozenRestart: "存储目录已更改，请先重启 HMM",
    },
    dialog: {
      revisionTitle: "选择新版本 ZIP 压缩包",
      newTitle: "选择 Mod ZIP 压缩包",
      zipFilterName: "ZIP 压缩包",
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
      waitingArchive: "等待选择 ZIP 压缩包",
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
    toasts: {
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
      invalidArchive: "Choose a valid local ZIP archive",
      startFailed: "Cannot start the import task",
      pickerFailed: "Cannot open the file picker",
      invalidStartState: "The import task returned an invalid state",
      storageFrozenMigration: "The storage directory is being migrated; import after it finishes",
      storageFrozenRestart: "The storage directory changed; restart HMM first",
    },
    dialog: {
      revisionTitle: "Choose the new version's ZIP archive",
      newTitle: "Choose the mod's ZIP archive",
      zipFilterName: "ZIP archive",
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
      waitingArchive: "Waiting for a ZIP archive",
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
    toasts: {
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
      invalidArchive: "有効なローカル ZIP アーカイブを選択してください",
      startFailed: "インポートタスクを開始できません",
      pickerFailed: "ファイル選択ダイアログを開けません",
      invalidStartState: "インポートタスクが無効な状態を返しました",
      storageFrozenMigration: "保存フォルダーの移行中です。完了後にインポートしてください",
      storageFrozenRestart: "保存フォルダーが変更されました。先に HMM を再起動してください",
    },
    dialog: {
      revisionTitle: "新バージョンの ZIP アーカイブを選択",
      newTitle: "Mod の ZIP アーカイブを選択",
      zipFilterName: "ZIP アーカイブ",
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
      waitingArchive: "ZIP アーカイブの選択待ち",
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
    toasts: {
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
