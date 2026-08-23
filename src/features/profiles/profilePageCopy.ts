import type { LocaleDictionary } from "../../shared/i18n";
import type { SaveBackupSummaryDto } from "./profileSaveBackupTypes";

// 存档备份页（页头、保存状态、活动存档/手动备份/自动备份运行期/后台保护/
// 备份历史各面板与页级 toast）的全部用户可见文案。
// 页头 eyebrow「Profile Workspace」等英文装饰为设计元素，不进字典。

export type ProfilePageCopy = {
  header: {
    title: string;
    subtitle: string;
    actionsAria: string;
    syncRefresh: string;
    createSlot: string;
  };
  saveAction: {
    saving: string;
    dirty: string;
    synced: string;
    notReady: string;
    savingButton: string;
    saveButton: string;
  };
  settingsStates: {
    detailAria: string;
    idle: string;
    loading: string;
    retry: string;
    unavailableFallback: string;
    saveFailedFallback: string;
  };
  blockedReasons: {
    selectProfile: string;
    settingsUnavailable: string;
    settingsLoadingBackup: string;
    settingsLoadingRestore: string;
    linkValidDirectory: string;
    saveSettingsFirst: string;
    backupTaskRunning: string;
    savingSettings: string;
  };
  activeSave: {
    title: string;
    noProfile: string;
    waitingDirectory: string;
  };
  manualBackup: {
    title: string;
    hint: string;
    starting: string;
    lastCompleted: string;
    cancelled: string;
    ready: string;
    runningButton: string;
    startButton: string;
    noteTemplate: (profileName: string) => string;
  };
  autoBackup: {
    title: string;
    checking: string;
    manualOnly: string;
    deferredGameRunning: string;
    deferredGameUnknown: string;
    queued: string;
    due: string;
    notDue: string;
    waiting: string;
    lastCheck: (value: string) => string;
    nextDue: (value: string) => string;
    neverChecked: string;
    waitingSchedule: string;
    checkNow: string;
    goToSettings: string;
    checkFailedFallback: string;
  };
  background: {
    badgeManual: string;
    badgeProtected: string;
    badgeStarting: string;
    badgeClientOnly: string;
    manualLabel: string;
    manualHint: string;
    loadingLabel: string;
    loadingHint: string;
    unavailableLabel: string;
    unavailableHint: string;
    lastSuccess: (value: string) => string;
    protectedLabel: string;
    protectedHint: string;
    startingLabel: string;
    startingHint: string;
    trayOnlyLabel: string;
    trayOnlyHint: string;
    registrationFailedLabel: string;
    registrationFailedHint: string;
    workerUnhealthyLabel: string;
    workerUnhealthyHint: string;
    permissionRequiredLabel: string;
    permissionRequiredHint: string;
    unsupportedLabel: string;
    unsupportedHint: string;
    notEnabledLabel: string;
    notEnabledHint: string;
  };
  history: {
    title: string;
    refreshing: string;
    count: (count: number) => string;
    refreshAria: string;
    restoreBlocked: (reason: string) => string;
    filterSr: string;
    filterPlaceholder: string;
    listAria: string;
    metaSize: string;
    metaCreatedAt: string;
    fileCount: (count: number) => string;
    restoreAria: (name: string) => string;
    notCompletedTitle: string;
    restoreTitle: string;
    restore: string;
    emptyTitle: string;
    emptyHint: string;
    unavailableFallback: string;
  };
  trigger: Record<SaveBackupSummaryDto["trigger"], string>;
  backupStatus: Record<SaveBackupSummaryDto["status"], string>;
  toasts: {
    refreshFailedTitle: string;
    refreshFailedMessage: string;
    completedTitle: string;
    completedMessage: string;
    admissionBusyTitle: string;
    failedTitle: string;
  };
  time: {
    justNow: string;
    minutesAgo: (minutes: number) => string;
    hoursAgo: (hours: number) => string;
    daysAgo: (days: number) => string;
    none: string;
  };
};

export const profilePageCopy = {
  zh_cn: {
    header: {
      title: "存档备份",
      subtitle: "管理当前游戏实例的多套存档配置、目录映射与自动备份策略",
      actionsAria: "配置档操作",
      syncRefresh: "同步刷新",
      createSlot: "新建配置槽",
    },
    saveAction: {
      saving: "正在保存设置",
      dirty: "有未保存的更改",
      synced: "设置已同步",
      notReady: "设置未就绪",
      savingButton: "保存中",
      saveButton: "保存设置",
    },
    settingsStates: {
      detailAria: "配置档详情与存档目录",
      idle: "选择配置档后显示存档设置",
      loading: "正在读取存档设置",
      retry: "重试",
      unavailableFallback: "存档设置不可用",
      saveFailedFallback: "保存失败",
    },
    blockedReasons: {
      selectProfile: "请选择配置档",
      settingsUnavailable: "存档设置不可用",
      settingsLoadingBackup: "读取存档设置后可备份",
      settingsLoadingRestore: "读取存档设置后可恢复",
      linkValidDirectory: "请先关联有效存档目录",
      saveSettingsFirst: "请先保存存档设置",
      backupTaskRunning: "备份任务正在执行",
      savingSettings: "正在保存存档设置",
    },
    activeSave: {
      title: "活动存档与自动策略",
      noProfile: "未选择配置档",
      waitingDirectory: "等待关联存档源目录",
    },
    manualBackup: {
      title: "手动备份",
      hint: "立即为当前配置档创建一个受控存档归档点。",
      starting: "正在启动备份任务",
      lastCompleted: "最近一次备份完成",
      cancelled: "备份任务已取消",
      ready: "可以创建手动备份",
      runningButton: "备份中",
      startButton: "立即归档当前存档",
      noteTemplate: (profileName: string) => `手动备份：${profileName}`,
    },
    autoBackup: {
      title: "自动备份运行期",
      checking: "正在检查自动备份计划",
      manualOnly: "当前配置为仅手动备份",
      deferredGameRunning: "游戏运行中，自动备份已延后",
      deferredGameUnknown: "暂时无法确认游戏状态，备份已延后",
      queued: "自动备份已排队",
      due: "自动备份计划已到期",
      notDue: "自动备份计划尚未到期",
      waiting: "等待自动备份检查",
      lastCheck: (value: string) => `最近检查：${value}`,
      nextDue: (value: string) => `下次计划：${value}`,
      neverChecked: "尚未检查",
      waitingSchedule: "等待调度信息",
      checkNow: "立即检查",
      goToSettings: "前往设置处理",
      checkFailedFallback: "自动备份检查失败",
    },
    background: {
      badgeManual: "未启用自动备份",
      badgeProtected: "退出后受保护",
      badgeStarting: "等待后台验证",
      badgeClientOnly: "仅客户端运行时",
      manualLabel: "未启用自动备份",
      manualHint: "此 Profile 使用手动备份，不参与后台调度",
      loadingLabel: "正在读取后台保护状态",
      loadingHint: "查询后台备份保障的最近记录",
      unavailableLabel: "后台保护状态不可用",
      unavailableHint: "暂时无法读取调度状态，自动备份仍按客户端计划执行",
      lastSuccess: (value: string) => `上次成功备份：${value}`,
      protectedLabel: "已受后台保护",
      protectedHint: "退出主客户端后仍会继续检查备份计划",
      startingLabel: "正在验证后台保护",
      startingHint: "后台任务已注册，正在等待首次运行验证",
      trayOnlyLabel: "仅客户端运行期保护",
      trayOnlyHint: "退出主客户端后自动备份暂不受后台保障",
      registrationFailedLabel: "后台保护注册失败",
      registrationFailedHint: "计划任务或自启动注册失败，退出客户端后不会自动备份",
      workerUnhealthyLabel: "后台保护异常",
      workerUnhealthyHint: "后台守护最近没有心跳，请重新检查备份计划",
      permissionRequiredLabel: "需要系统权限",
      permissionRequiredHint: "当前环境需要额外权限才能启用后台保护",
      unsupportedLabel: "当前平台暂不支持后台保护",
      unsupportedHint: "自动备份仅在客户端运行时执行",
      notEnabledLabel: "未启用后台保护",
      notEnabledHint: "自动备份仅在客户端运行时执行",
    },
    history: {
      title: "备份历史点",
      refreshing: "刷新中",
      count: (count: number) => `${count} 个归档包`,
      refreshAria: "刷新备份历史",
      restoreBlocked: (reason: string) => `恢复暂不可用：${reason}`,
      filterSr: "筛选备份历史",
      filterPlaceholder: "输入备份备注以筛选历史...",
      listAria: "备份历史",
      metaSize: "大小",
      metaCreatedAt: "归档时间",
      fileCount: (count: number) => `${count} 个文件`,
      restoreAria: (name: string) => `恢复存档：${name}`,
      notCompletedTitle: "该备份尚未完成，不能恢复",
      restoreTitle: "预览并恢复此存档",
      restore: "恢复存档",
      emptyTitle: "暂无存档备份",
      emptyHint: "完成首次归档后会在这里显示历史点。",
      unavailableFallback: "备份历史不可用",
    },
    trigger: {
      auto: "自动备份",
      pre_install: "安装前备份",
      pre_restore: "恢复前安全备份",
      manual: "手动备份",
    },
    backupStatus: {
      completed: "已完成",
      retention_pending: "待保留策略清理",
      retention_partial: "保留清理未完成",
      deleted_by_retention: "已按保留策略清理",
      missing: "文件缺失",
      invalid: "需要检查",
    },
    toasts: {
      refreshFailedTitle: "备份完成，历史刷新失败",
      refreshFailedMessage: "备份任务已完成，但当前历史列表未能刷新，请稍后重试。",
      completedTitle: "存档备份完成",
      completedMessage: "新的备份历史点已经写入当前配置档。",
      admissionBusyTitle: "存档操作正在进行",
      failedTitle: "存档备份失败",
    },
    time: {
      justNow: "刚刚",
      minutesAgo: (minutes: number) => `${minutes} 分钟前`,
      hoursAgo: (hours: number) => `${hours} 小时前`,
      daysAgo: (days: number) => `${days} 天前`,
      none: "暂无",
    },
  },
  en: {
    header: {
      title: "Save data backup",
      subtitle: "Manage multiple save data profiles, directory mappings, and auto backup policies for the current game instance",
      actionsAria: "Profile actions",
      syncRefresh: "Sync refresh",
      createSlot: "New profile slot",
    },
    saveAction: {
      saving: "Saving settings",
      dirty: "Unsaved changes",
      synced: "Settings synced",
      notReady: "Settings not ready",
      savingButton: "Saving",
      saveButton: "Save settings",
    },
    settingsStates: {
      detailAria: "Profile details and save data directories",
      idle: "Select a profile to show save data settings",
      loading: "Loading save data settings",
      retry: "Retry",
      unavailableFallback: "Save data settings unavailable",
      saveFailedFallback: "Save failed",
    },
    blockedReasons: {
      selectProfile: "Select a profile first",
      settingsUnavailable: "Save data settings unavailable",
      settingsLoadingBackup: "Backup becomes available after settings load",
      settingsLoadingRestore: "Restore becomes available after settings load",
      linkValidDirectory: "Link a valid save data directory first",
      saveSettingsFirst: "Save the save data settings first",
      backupTaskRunning: "A backup task is running",
      savingSettings: "Saving save data settings",
    },
    activeSave: {
      title: "Active save data and auto policy",
      noProfile: "No profile selected",
      waitingDirectory: "Waiting for a linked save data source directory",
    },
    manualBackup: {
      title: "Manual backup",
      hint: "Create a controlled save data archive point for the current profile now.",
      starting: "Starting backup task",
      lastCompleted: "Last backup completed",
      cancelled: "Backup task cancelled",
      ready: "Ready to create a manual backup",
      runningButton: "Backing up",
      startButton: "Archive current save data now",
      noteTemplate: (profileName: string) => `Manual backup: ${profileName}`,
    },
    autoBackup: {
      title: "Auto backup runtime",
      checking: "Checking the auto backup schedule",
      manualOnly: "This profile is configured for manual backup only",
      deferredGameRunning: "Game is running; auto backup deferred",
      deferredGameUnknown: "Cannot confirm game state for now; backup deferred",
      queued: "Auto backup queued",
      due: "Auto backup schedule is due",
      notDue: "Auto backup schedule is not due yet",
      waiting: "Waiting for the auto backup check",
      lastCheck: (value: string) => `Last check: ${value}`,
      nextDue: (value: string) => `Next due: ${value}`,
      neverChecked: "Not checked yet",
      waitingSchedule: "Waiting for schedule info",
      checkNow: "Check now",
      goToSettings: "Open settings to resolve",
      checkFailedFallback: "Auto backup check failed",
    },
    background: {
      badgeManual: "Auto backup off",
      badgeProtected: "Protected after exit",
      badgeStarting: "Awaiting background verification",
      badgeClientOnly: "Client runtime only",
      manualLabel: "Auto backup not enabled",
      manualHint: "This profile uses manual backup and does not join background scheduling",
      loadingLabel: "Reading background protection state",
      loadingHint: "Querying the latest background backup guarantee records",
      unavailableLabel: "Background protection state unavailable",
      unavailableHint: "Scheduler state is temporarily unreadable; auto backup still follows the client-side plan",
      lastSuccess: (value: string) => `Last successful backup: ${value}`,
      protectedLabel: "Protected in the background",
      protectedHint: "Backup schedules keep being checked after the main client exits",
      startingLabel: "Verifying background protection",
      startingHint: "The background task is registered and waiting for its first verified run",
      trayOnlyLabel: "Client-runtime protection only",
      trayOnlyHint: "Auto backup is not yet guaranteed in the background after the main client exits",
      registrationFailedLabel: "Background protection registration failed",
      registrationFailedHint: "Scheduled task or autostart registration failed; no auto backup after the client exits",
      workerUnhealthyLabel: "Background protection unhealthy",
      workerUnhealthyHint: "The background worker has no recent heartbeat; re-check the backup schedule",
      permissionRequiredLabel: "System permission required",
      permissionRequiredHint: "This environment needs extra permissions to enable background protection",
      unsupportedLabel: "Background protection is not supported on this platform",
      unsupportedHint: "Auto backup only runs while the client is running",
      notEnabledLabel: "Background protection not enabled",
      notEnabledHint: "Auto backup only runs while the client is running",
    },
    history: {
      title: "Backup history points",
      refreshing: "Refreshing",
      count: (count: number) => `${count} archive${count === 1 ? "" : "s"}`,
      refreshAria: "Refresh backup history",
      restoreBlocked: (reason: string) => `Restore unavailable: ${reason}`,
      filterSr: "Filter backup history",
      filterPlaceholder: "Type backup notes to filter history...",
      listAria: "Backup history",
      metaSize: "Size",
      metaCreatedAt: "Archived at",
      fileCount: (count: number) => `${count} file${count === 1 ? "" : "s"}`,
      restoreAria: (name: string) => `Restore save data: ${name}`,
      notCompletedTitle: "This backup is not completed and cannot be restored",
      restoreTitle: "Preview and restore this save data",
      restore: "Restore save data",
      emptyTitle: "No save data backups yet",
      emptyHint: "History points appear here after the first archive completes.",
      unavailableFallback: "Backup history unavailable",
    },
    trigger: {
      auto: "Auto backup",
      pre_install: "Pre-install backup",
      pre_restore: "Pre-restore safety backup",
      manual: "Manual backup",
    },
    backupStatus: {
      completed: "Completed",
      retention_pending: "Retention pruning pending",
      retention_partial: "Retention pruning incomplete",
      deleted_by_retention: "Pruned by retention policy",
      missing: "File missing",
      invalid: "Needs inspection",
    },
    toasts: {
      refreshFailedTitle: "Backup completed; history refresh failed",
      refreshFailedMessage: "The backup task completed, but the current history list failed to refresh. Try again later.",
      completedTitle: "Save data backup completed",
      completedMessage: "A new backup history point was written to the current profile.",
      admissionBusyTitle: "A save data operation is in progress",
      failedTitle: "Save data backup failed",
    },
    time: {
      justNow: "just now",
      minutesAgo: (minutes: number) => `${minutes} min ago`,
      hoursAgo: (hours: number) => `${hours} h ago`,
      daysAgo: (days: number) => `${days} d ago`,
      none: "None",
    },
  },
  ja: {
    header: {
      title: "セーブデータバックアップ",
      subtitle: "現在のゲームインスタンスの複数セーブデータ構成・ディレクトリ対応付け・自動バックアップポリシーを管理",
      actionsAria: "プロファイル操作",
      syncRefresh: "同期更新",
      createSlot: "プロファイルスロットを作成",
    },
    saveAction: {
      saving: "設定を保存中",
      dirty: "未保存の変更があります",
      synced: "設定は同期済み",
      notReady: "設定は未準備",
      savingButton: "保存中",
      saveButton: "設定を保存",
    },
    settingsStates: {
      detailAria: "プロファイル詳細とセーブデータディレクトリ",
      idle: "プロファイルを選択するとセーブデータ設定を表示します",
      loading: "セーブデータ設定を読み込み中",
      retry: "再試行",
      unavailableFallback: "セーブデータ設定を利用できません",
      saveFailedFallback: "保存に失敗しました",
    },
    blockedReasons: {
      selectProfile: "プロファイルを選択してください",
      settingsUnavailable: "セーブデータ設定を利用できません",
      settingsLoadingBackup: "設定の読み込み後にバックアップできます",
      settingsLoadingRestore: "設定の読み込み後に復元できます",
      linkValidDirectory: "先に有効なセーブデータディレクトリを関連付けてください",
      saveSettingsFirst: "先にセーブデータ設定を保存してください",
      backupTaskRunning: "バックアップタスクを実行中です",
      savingSettings: "セーブデータ設定を保存中です",
    },
    activeSave: {
      title: "アクティブセーブデータと自動ポリシー",
      noProfile: "プロファイル未選択",
      waitingDirectory: "セーブデータソースディレクトリの関連付け待ち",
    },
    manualBackup: {
      title: "手動バックアップ",
      hint: "現在のプロファイルの管理されたセーブデータアーカイブポイントをすぐに作成します。",
      starting: "バックアップタスクを開始中",
      lastCompleted: "直近のバックアップが完了",
      cancelled: "バックアップタスクをキャンセルしました",
      ready: "手動バックアップを作成できます",
      runningButton: "バックアップ中",
      startButton: "現在のセーブデータを今すぐアーカイブ",
      noteTemplate: (profileName: string) => `手動バックアップ：${profileName}`,
    },
    autoBackup: {
      title: "自動バックアップ実行状況",
      checking: "自動バックアップ計画を確認中",
      manualOnly: "現在の構成は手動バックアップのみです",
      deferredGameRunning: "ゲーム実行中のため自動バックアップを延期しました",
      deferredGameUnknown: "ゲームの状態を確認できないため、バックアップを延期しました",
      queued: "自動バックアップをキューに追加しました",
      due: "自動バックアップ計画が期限到来",
      notDue: "自動バックアップ計画はまだ期限前です",
      waiting: "自動バックアップ確認を待機中",
      lastCheck: (value: string) => `最終確認：${value}`,
      nextDue: (value: string) => `次回予定：${value}`,
      neverChecked: "未確認",
      waitingSchedule: "スケジュール情報を待機中",
      checkNow: "今すぐ確認",
      goToSettings: "設定を開いて対処",
      checkFailedFallback: "自動バックアップの確認に失敗しました",
    },
    background: {
      badgeManual: "自動バックアップ無効",
      badgeProtected: "終了後も保護",
      badgeStarting: "バックグラウンド検証待ち",
      badgeClientOnly: "クライアント実行時のみ",
      manualLabel: "自動バックアップは無効",
      manualHint: "このプロファイルは手動バックアップを使用し、バックグラウンドのスケジュールには参加しません",
      loadingLabel: "バックグラウンド保護の状態を読み込み中",
      loadingHint: "バックグラウンドバックアップ保証の最新記録を照会しています",
      unavailableLabel: "バックグラウンド保護の状態を取得できません",
      unavailableHint: "スケジューラ状態を一時的に読み取れませんが、自動バックアップはクライアント側計画どおり実行されます",
      lastSuccess: (value: string) => `前回成功したバックアップ：${value}`,
      protectedLabel: "バックグラウンドで保護中",
      protectedHint: "メインクライアント終了後もバックアップ計画の確認を続けます",
      startingLabel: "バックグラウンド保護を検証中",
      startingHint: "バックグラウンドタスクは登録済みで、初回実行の検証を待っています",
      trayOnlyLabel: "クライアント実行時のみ保護",
      trayOnlyHint: "メインクライアント終了後の自動バックアップはまだバックグラウンドで保証されません",
      registrationFailedLabel: "バックグラウンド保護の登録に失敗",
      registrationFailedHint: "スケジュールタスクまたは自動起動の登録に失敗したため、クライアント終了後は自動バックアップされません",
      workerUnhealthyLabel: "バックグラウンド保護に異常",
      workerUnhealthyHint: "バックグラウンドワーカーの直近のハートビートがありません。バックアップ計画を再確認してください",
      permissionRequiredLabel: "システム権限が必要",
      permissionRequiredHint: "現在の環境でバックグラウンド保護を有効にするには追加の権限が必要です",
      unsupportedLabel: "このプラットフォームはバックグラウンド保護に未対応",
      unsupportedHint: "自動バックアップはクライアント実行中のみ動作します",
      notEnabledLabel: "バックグラウンド保護は無効",
      notEnabledHint: "自動バックアップはクライアント実行中のみ動作します",
    },
    history: {
      title: "バックアップ履歴ポイント",
      refreshing: "更新中",
      count: (count: number) => `アーカイブ ${count} 件`,
      refreshAria: "バックアップ履歴を更新",
      restoreBlocked: (reason: string) => `復元は一時的に利用できません：${reason}`,
      filterSr: "バックアップ履歴を絞り込み",
      filterPlaceholder: "バックアップメモで履歴を絞り込み...",
      listAria: "バックアップ履歴",
      metaSize: "サイズ",
      metaCreatedAt: "アーカイブ日時",
      fileCount: (count: number) => `${count} ファイル`,
      restoreAria: (name: string) => `セーブデータを復元：${name}`,
      notCompletedTitle: "このバックアップは未完了のため復元できません",
      restoreTitle: "このセーブデータをプレビューして復元",
      restore: "セーブデータを復元",
      emptyTitle: "セーブデータのバックアップはまだありません",
      emptyHint: "最初のアーカイブが完了すると、ここに履歴ポイントが表示されます。",
      unavailableFallback: "バックアップ履歴を利用できません",
    },
    trigger: {
      auto: "自動バックアップ",
      pre_install: "インストール前バックアップ",
      pre_restore: "復元前セーフティバックアップ",
      manual: "手動バックアップ",
    },
    backupStatus: {
      completed: "完了",
      retention_pending: "保持ポリシーの整理待ち",
      retention_partial: "保持整理が未完了",
      deleted_by_retention: "保持ポリシーにより整理済み",
      missing: "ファイル欠落",
      invalid: "要確認",
    },
    toasts: {
      refreshFailedTitle: "バックアップ完了・履歴更新に失敗",
      refreshFailedMessage: "バックアップタスクは完了しましたが、現在の履歴一覧を更新できませんでした。しばらくしてから再試行してください。",
      completedTitle: "セーブデータのバックアップが完了",
      completedMessage: "新しいバックアップ履歴ポイントを現在のプロファイルへ書き込みました。",
      admissionBusyTitle: "セーブデータ操作が進行中",
      failedTitle: "セーブデータのバックアップに失敗",
    },
    time: {
      justNow: "たった今",
      minutesAgo: (minutes: number) => `${minutes} 分前`,
      hoursAgo: (hours: number) => `${hours} 時間前`,
      daysAgo: (days: number) => `${days} 日前`,
      none: "なし",
    },
  },
} satisfies LocaleDictionary<ProfilePageCopy>;
