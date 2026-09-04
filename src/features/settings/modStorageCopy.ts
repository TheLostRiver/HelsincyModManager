import type { LocaleDictionary } from "../../shared/i18n/locales";

// Mod 存储目录（#275）的全部用户可见文案。后端只出稳定码（mod_storage_dir_* /
// mod_storage_migration_* / 门闩两码），文案在此按码表三语兜底；组件不得硬编码。

/** 目录校验与设置命令的稳定码（契约「Mod 存储目录（#275 切片①）」）。 */
export type ModStorageDirErrorCode =
  | "mod_storage_dir_not_absolute"
  | "mod_storage_dir_unsafe"
  | "mod_storage_dir_filesystem_root"
  | "mod_storage_dir_parent_missing"
  | "mod_storage_dir_not_directory"
  | "mod_storage_dir_link_rejected"
  | "mod_storage_dir_marker_required"
  | "mod_storage_dir_marker_invalid"
  | "mod_storage_dir_not_writable"
  | "mod_storage_dir_overlaps_game_root"
  | "mod_storage_dir_overlaps_current_root"
  | "mod_storage_dir_unavailable";

/** 命令层拒绝码：设置服务、写门闩、迁移登记（契约切片①②）。 */
export type ModStorageCommandErrorCode =
  | "mod_storage_migration_required"
  | "app_settings_unavailable"
  | "mod_library_unavailable"
  | "game_config_unavailable"
  | "mod_storage_migration_in_progress"
  | "mod_storage_restart_required"
  | "mod_storage_migration_imports_active"
  | "mod_storage_migration_task_unavailable";

/**
 * 迁移 failed 事件的 error 码族（契约切片②「终态 error 码」）。
 * `mod_storage_migration_progress_unrecognized` 与 `mod_storage_migration_listener_unavailable`
 * 是前端合成码：事件形状不合契约 / 事件监听建立失败。
 */
export type ModStorageMigrationErrorCode =
  | "mod_storage_migration_source_unavailable"
  | "mod_storage_migration_target_unavailable"
  | "mod_storage_migration_package_unreadable"
  | "mod_storage_migration_copy_failed"
  | "mod_storage_migration_verify_mismatch"
  | "mod_storage_migration_journal_unavailable"
  | "mod_storage_migration_settings_unavailable"
  | "mod_storage_migration_cancelled"
  | "mod_storage_migration_progress_unrecognized"
  | "mod_storage_migration_listener_unavailable";

/** 前端自身的失败：系统目录选择器打不开。 */
export type ModStorageUiErrorCode = "mod_storage_picker_failed";

export type ModStorageErrorCode =
  | ModStorageDirErrorCode
  | ModStorageCommandErrorCode
  | ModStorageMigrationErrorCode
  | ModStorageUiErrorCode;

export type ModStorageDegradedReason =
  | "settings_unreadable"
  | "configured_dir_invalid"
  | "configured_dir_unavailable";

export type ModStorageMigrationPhase =
  | "mod_storage.migration.queued"
  | "mod_storage.migration.copying"
  | "mod_storage.migration.verifying"
  | "mod_storage.migration.switching"
  | "mod_storage.migration.completed"
  | "mod_storage.migration.failed"
  | "mod_storage.migration.cancelling"
  | "mod_storage.migration.cancelled";

export type ModStorageCopy = {
  section: {
    title: string;
    description: string;
  };
  current: {
    title: string;
    defaultBadge: string;
    customBadge: string;
    libraryEmpty: string;
    libraryHasPackages: string;
    pendingChange: (directory: string) => string;
    pendingDefault: string;
    loading: string;
    reload: string;
  };
  degraded: Record<ModStorageDegradedReason, string> & { title: string };
  restart: {
    title: string;
    message: string;
  };
  /** 导入 / 删除入口在门闩冻结时的禁用原因（按 writesFrozen 取词，不复算）。 */
  frozen: {
    migration: string;
    restart_required: string;
  };
  actions: {
    choose: string;
    restoreDefault: string;
    pickerTitle: string;
    cancelMigration: string;
    cancelling: string;
    cancelUnavailable: string;
    retryListener: string;
    dismiss: string;
    busy: string;
  };
  confirm: {
    closeAria: string;
    cancel: string;
    setTitle: string;
    setBody: (directory: string) => string;
    setConfirm: string;
    migrateTitle: string;
    migrateBody: (directory: string) => string;
    migrateStepCopy: string;
    migrateStepFreeze: string;
    migrateStepRestart: string;
    migrateConfirm: string;
    defaultDirectoryLabel: string;
  };
  migration: {
    title: string;
    phases: Record<ModStorageMigrationPhase, string>;
    unrecognizedPhase: string;
    progress: (current: string, total: string) => string;
    completedTitle: string;
    completedMessage: string;
    cancelledTitle: string;
    cancelledMessage: string;
    failedTitle: string;
    cancelFailedTitle: string;
  };
  errors: Record<ModStorageErrorCode, string> & { unknown: (code: string) => string };
};

export const modStorageCopy = {
  zh_cn: {
    section: {
      title: "Mod 存储目录",
      description:
        "已导入 Mod 的解包内容存放在这里。可以改到其他磁盘；目录清单、缩略图与安装记录始终留在应用数据目录。",
    },
    current: {
      title: "当前目录",
      defaultBadge: "默认",
      customBadge: "自定义",
      libraryEmpty: "库为空，切换目录无需迁移。",
      libraryHasPackages: "库中已有 Mod，切换目录会把它们迁移到新位置。",
      pendingChange: (directory) => `已设置为 ${directory}，重启后生效。`,
      pendingDefault: "已设置为默认目录，重启后生效。",
      loading: "正在读取存储目录设置…",
      reload: "重新读取",
    },
    degraded: {
      title: "存储目录状态异常",
      settings_unreadable: "无法读取设置文件，本次启动使用默认目录。请检查应用数据目录后重启。",
      configured_dir_invalid: "已保存的目录不是合法路径，本次启动使用默认目录。请重新选择目录。",
      configured_dir_unavailable:
        "已配置的目录当前不可用（磁盘未连接、目录被移动或标记文件损坏）。在它恢复之前无法导入或读取 Mod 内容；请接回磁盘，或重新选择目录。",
    },
    restart: {
      title: "需要重启",
      message: "存储目录已更改，重启 HMM 后生效。重启前无法导入或删除 Mod。",
    },
    frozen: {
      migration: "存储目录正在迁移，完成后再操作。",
      restart_required: "存储目录已更改，请先重启 HMM。",
    },
    actions: {
      choose: "选择新目录…",
      restoreDefault: "恢复默认目录",
      pickerTitle: "选择 Mod 存储目录",
      cancelMigration: "取消迁移",
      cancelling: "正在取消…",
      cancelUnavailable: "正在切换设置，此时不能取消。",
      retryListener: "重新连接进度",
      dismiss: "知道了",
      busy: "正在处理…",
    },
    confirm: {
      closeAria: "关闭确认",
      cancel: "取消",
      setTitle: "切换存储目录",
      setBody: (directory) =>
        `将把 Mod 存储目录设为 ${directory}。库当前为空，无需迁移；更改在重启 HMM 后生效，重启前无法导入 Mod。`,
      setConfirm: "切换目录",
      migrateTitle: "迁移 Mod 库",
      migrateBody: (directory) => `将把库中全部 Mod 迁移到 ${directory}。`,
      migrateStepCopy:
        "逐个复制并校验每个 Mod；任一失败即整体撤销，原目录不受影响。迁移期间两处会同时占用磁盘空间。",
      migrateStepFreeze: "迁移期间以及完成后到重启之前，无法导入或删除 Mod；安装与预览不受影响。",
      migrateStepRestart: "全部校验通过后自动切换设置；重启 HMM 后启用新目录，并在此时清理原目录中的旧副本。",
      migrateConfirm: "开始迁移",
      defaultDirectoryLabel: "默认目录",
    },
    migration: {
      title: "正在迁移 Mod 库",
      phases: {
        "mod_storage.migration.queued": "等待开始",
        "mod_storage.migration.copying": "正在复制",
        "mod_storage.migration.verifying": "正在校验",
        "mod_storage.migration.switching": "正在切换设置",
        "mod_storage.migration.completed": "迁移完成",
        "mod_storage.migration.failed": "迁移失败",
        "mod_storage.migration.cancelling": "正在取消，清理已复制的内容",
        "mod_storage.migration.cancelled": "已取消",
      },
      unrecognizedPhase: "阶段不可识别",
      progress: (current, total) => `（${current} / ${total} 个 Mod）`,
      completedTitle: "Mod 库迁移完成",
      completedMessage: "重启 HMM 后启用新目录；重启前无法导入或删除 Mod。",
      cancelledTitle: "已取消迁移",
      cancelledMessage: "已复制的内容已清理，存储目录未改变。",
      failedTitle: "迁移未完成",
      cancelFailedTitle: "无法取消迁移",
    },
    errors: {
      mod_storage_dir_not_absolute: "请选择一个完整的绝对路径。",
      mod_storage_dir_unsafe: "路径中不能包含「.」或「..」。",
      mod_storage_dir_filesystem_root: "不能直接使用磁盘根目录，请在其下新建一个文件夹。",
      mod_storage_dir_parent_missing: "该目录的上级目录不存在。",
      mod_storage_dir_not_directory: "所选路径不是文件夹。",
      mod_storage_dir_link_rejected: "该目录或其上级是链接 / 联接点，不能作为存储目录。",
      mod_storage_dir_marker_required: "该文件夹已有其他内容。请选择空文件夹，或此前由 HMM 使用过的目录。",
      mod_storage_dir_marker_invalid: "该目录的 HMM 标记文件已损坏，请选择其他目录。",
      mod_storage_dir_not_writable: "该目录不可写入，请检查权限或选择其他位置。",
      mod_storage_dir_overlaps_game_root: "存储目录不能位于游戏目录之内，也不能包含游戏目录。",
      mod_storage_dir_overlaps_current_root: "与当前存储目录相同或互相包含，请选择其他目录。",
      mod_storage_dir_unavailable: "无法检查该目录，请稍后重试。",
      mod_storage_migration_required: "库中已有 Mod，请通过迁移更改目录。",
      app_settings_unavailable: "应用设置暂时不可用，请稍后重试。",
      mod_library_unavailable: "Mod 库暂时不可用，请稍后重试。",
      game_config_unavailable: "游戏目录配置暂时不可用，请稍后重试。",
      mod_storage_migration_in_progress: "存储目录正在迁移，完成后再操作。",
      mod_storage_restart_required: "存储目录已更改，请先重启 HMM。",
      mod_storage_migration_imports_active: "有导入任务正在进行，请等待其完成后再迁移。",
      mod_storage_migration_task_unavailable: "无法登记迁移任务，请稍后重试。",
      mod_storage_migration_source_unavailable: "无法读取当前存储目录，迁移已撤销。",
      mod_storage_migration_target_unavailable: "无法写入目标目录，迁移已撤销。",
      mod_storage_migration_package_unreadable: "某个 Mod 的内容无法读取（存在链接或异常条目），迁移已撤销。",
      mod_storage_migration_copy_failed: "复制过程中出错，迁移已撤销。",
      mod_storage_migration_verify_mismatch: "复制结果与源内容不一致，迁移已撤销。",
      mod_storage_migration_journal_unavailable: "无法写入迁移记录，未进行任何复制。",
      mod_storage_migration_settings_unavailable: "全部内容已复制，但设置保存失败；副本已清理，目录未改变。",
      mod_storage_migration_cancelled: "迁移已取消，存储目录未改变。",
      mod_storage_migration_progress_unrecognized: "迁移进度不可识别，请重新读取设置确认状态。",
      mod_storage_migration_listener_unavailable: "无法接收迁移进度，请重新连接后再试。",
      mod_storage_picker_failed: "无法打开目录选择器。",
      unknown: (code) => `操作失败（${code}）。`,
    },
  },
  en: {
    section: {
      title: "Mod storage directory",
      description:
        "Unpacked contents of imported mods live here. You can move them to another drive; the catalog, thumbnails and install records always stay in the app data directory.",
    },
    current: {
      title: "Current directory",
      defaultBadge: "Default",
      customBadge: "Custom",
      libraryEmpty: "The library is empty; switching needs no migration.",
      libraryHasPackages: "The library holds mods; switching migrates them to the new location.",
      pendingChange: (directory) => `Set to ${directory}; takes effect after a restart.`,
      pendingDefault: "Set to the default directory; takes effect after a restart.",
      loading: "Reading storage directory settings…",
      reload: "Reload",
    },
    degraded: {
      title: "Storage directory needs attention",
      settings_unreadable:
        "The settings file could not be read; this session uses the default directory. Check the app data directory and restart.",
      configured_dir_invalid:
        "The saved directory is not a valid path; this session uses the default directory. Choose a directory again.",
      configured_dir_unavailable:
        "The configured directory is currently unavailable (drive disconnected, directory moved, or marker file damaged). Mods cannot be imported or read until it is back; reconnect the drive or choose another directory.",
    },
    restart: {
      title: "Restart required",
      message: "The storage directory changed and takes effect after restarting HMM. Mods cannot be imported or deleted until then.",
    },
    frozen: {
      migration: "The storage directory is being migrated; wait for it to finish.",
      restart_required: "The storage directory changed; restart HMM first.",
    },
    actions: {
      choose: "Choose new directory…",
      restoreDefault: "Restore default directory",
      pickerTitle: "Choose the mod storage directory",
      cancelMigration: "Cancel migration",
      cancelling: "Cancelling…",
      cancelUnavailable: "The setting is being switched; cancelling is no longer possible.",
      retryListener: "Reconnect progress",
      dismiss: "Got it",
      busy: "Working…",
    },
    confirm: {
      closeAria: "Close confirmation",
      cancel: "Cancel",
      setTitle: "Switch storage directory",
      setBody: (directory) =>
        `The mod storage directory will be set to ${directory}. The library is empty, so no migration is needed; the change takes effect after restarting HMM, and mods cannot be imported until then.`,
      setConfirm: "Switch directory",
      migrateTitle: "Migrate the mod library",
      migrateBody: (directory) => `Every mod in the library will be migrated to ${directory}.`,
      migrateStepCopy:
        "Each mod is copied and verified one by one; any failure undoes the whole migration and the original directory is untouched. Both locations use disk space while migrating.",
      migrateStepFreeze:
        "Mods cannot be imported or deleted during the migration and until the restart; installing and previews keep working.",
      migrateStepRestart:
        "Once every mod is verified the setting switches automatically; the new directory is used after restarting HMM, and the old copies are cleaned up then.",
      migrateConfirm: "Start migration",
      defaultDirectoryLabel: "the default directory",
    },
    migration: {
      title: "Migrating the mod library",
      phases: {
        "mod_storage.migration.queued": "Waiting to start",
        "mod_storage.migration.copying": "Copying",
        "mod_storage.migration.verifying": "Verifying",
        "mod_storage.migration.switching": "Switching the setting",
        "mod_storage.migration.completed": "Migration complete",
        "mod_storage.migration.failed": "Migration failed",
        "mod_storage.migration.cancelling": "Cancelling, cleaning up copied contents",
        "mod_storage.migration.cancelled": "Cancelled",
      },
      unrecognizedPhase: "Unrecognized stage",
      progress: (current, total) => ` (${current} / ${total} mods)`,
      completedTitle: "Mod library migrated",
      completedMessage: "The new directory is used after restarting HMM; mods cannot be imported or deleted until then.",
      cancelledTitle: "Migration cancelled",
      cancelledMessage: "Copied contents were cleaned up; the storage directory is unchanged.",
      failedTitle: "Migration did not finish",
      cancelFailedTitle: "Cannot cancel the migration",
    },
    errors: {
      mod_storage_dir_not_absolute: "Choose a full absolute path.",
      mod_storage_dir_unsafe: "The path must not contain “.” or “..” segments.",
      mod_storage_dir_filesystem_root: "A drive root cannot be used directly; create a folder inside it.",
      mod_storage_dir_parent_missing: "The parent directory does not exist.",
      mod_storage_dir_not_directory: "The selected path is not a folder.",
      mod_storage_dir_link_rejected: "The directory or one of its parents is a link / junction and cannot be used.",
      mod_storage_dir_marker_required: "The folder already has other contents. Choose an empty folder or one HMM used before.",
      mod_storage_dir_marker_invalid: "The HMM marker file in this directory is damaged; choose another directory.",
      mod_storage_dir_not_writable: "The directory is not writable; check permissions or choose another location.",
      mod_storage_dir_overlaps_game_root: "The storage directory cannot be inside the game directory or contain it.",
      mod_storage_dir_overlaps_current_root: "Same as or nested with the current storage directory; choose another one.",
      mod_storage_dir_unavailable: "The directory could not be checked; try again later.",
      mod_storage_migration_required: "The library holds mods; change the directory through a migration.",
      app_settings_unavailable: "App settings are temporarily unavailable; try again later.",
      mod_library_unavailable: "The mod library is temporarily unavailable; try again later.",
      game_config_unavailable: "The game directory configuration is temporarily unavailable; try again later.",
      mod_storage_migration_in_progress: "The storage directory is being migrated; wait for it to finish.",
      mod_storage_restart_required: "The storage directory changed; restart HMM first.",
      mod_storage_migration_imports_active: "An import task is running; wait for it to finish before migrating.",
      mod_storage_migration_task_unavailable: "The migration task could not be registered; try again later.",
      mod_storage_migration_source_unavailable: "The current storage directory could not be read; the migration was undone.",
      mod_storage_migration_target_unavailable: "The target directory could not be written; the migration was undone.",
      mod_storage_migration_package_unreadable: "A mod's contents could not be read (link or unexpected entry); the migration was undone.",
      mod_storage_migration_copy_failed: "Copying failed; the migration was undone.",
      mod_storage_migration_verify_mismatch: "The copied contents did not match the source; the migration was undone.",
      mod_storage_migration_journal_unavailable: "The migration journal could not be written; nothing was copied.",
      mod_storage_migration_settings_unavailable: "Everything was copied but the setting could not be saved; the copies were cleaned up and the directory is unchanged.",
      mod_storage_migration_cancelled: "The migration was cancelled; the storage directory is unchanged.",
      mod_storage_migration_progress_unrecognized: "The migration progress was not recognized; reload the settings to confirm the state.",
      mod_storage_migration_listener_unavailable: "Migration progress cannot be received; reconnect and try again.",
      mod_storage_picker_failed: "Cannot open the directory picker.",
      unknown: (code) => `The operation failed (${code}).`,
    },
  },
  ja: {
    section: {
      title: "Mod 保存フォルダー",
      description:
        "インポートした Mod の展開内容を保存する場所です。別のドライブに移せます。カタログ、サムネイル、インストール記録は常にアプリデータフォルダーに残ります。",
    },
    current: {
      title: "現在のフォルダー",
      defaultBadge: "既定",
      customBadge: "カスタム",
      libraryEmpty: "ライブラリは空です。切り替えに移行は不要です。",
      libraryHasPackages: "ライブラリに Mod があります。切り替えると新しい場所へ移行します。",
      pendingChange: (directory) => `${directory} に設定済み。再起動後に有効になります。`,
      pendingDefault: "既定のフォルダーに設定済み。再起動後に有効になります。",
      loading: "保存フォルダー設定を読み込んでいます…",
      reload: "再読み込み",
    },
    degraded: {
      title: "保存フォルダーに問題があります",
      settings_unreadable:
        "設定ファイルを読み込めなかったため、今回は既定のフォルダーを使用しています。アプリデータフォルダーを確認して再起動してください。",
      configured_dir_invalid:
        "保存されたフォルダーが有効なパスではないため、今回は既定のフォルダーを使用しています。フォルダーを選び直してください。",
      configured_dir_unavailable:
        "設定済みのフォルダーが現在利用できません（ドライブ未接続、フォルダー移動、マーカーファイル破損）。復旧するまで Mod のインポートや読み取りはできません。ドライブを接続し直すか、別のフォルダーを選んでください。",
    },
    restart: {
      title: "再起動が必要です",
      message: "保存フォルダーが変更されました。HMM を再起動すると有効になります。それまで Mod のインポートと削除はできません。",
    },
    frozen: {
      migration: "保存フォルダーの移行中です。完了してから操作してください。",
      restart_required: "保存フォルダーが変更されました。先に HMM を再起動してください。",
    },
    actions: {
      choose: "新しいフォルダーを選択…",
      restoreDefault: "既定のフォルダーに戻す",
      pickerTitle: "Mod 保存フォルダーを選択",
      cancelMigration: "移行をキャンセル",
      cancelling: "キャンセル中…",
      cancelUnavailable: "設定の切り替え中のため、キャンセルできません。",
      retryListener: "進捗を再接続",
      dismiss: "了解",
      busy: "処理中…",
    },
    confirm: {
      closeAria: "確認を閉じる",
      cancel: "キャンセル",
      setTitle: "保存フォルダーを切り替え",
      setBody: (directory) =>
        `Mod 保存フォルダーを ${directory} に設定します。ライブラリは空のため移行は不要です。変更は HMM の再起動後に有効になり、それまで Mod をインポートできません。`,
      setConfirm: "フォルダーを切り替え",
      migrateTitle: "Mod ライブラリを移行",
      migrateBody: (directory) => `ライブラリ内のすべての Mod を ${directory} に移行します。`,
      migrateStepCopy:
        "Mod を 1 つずつコピーして検証します。いずれかが失敗すると移行全体を取り消し、元のフォルダーはそのまま残ります。移行中は両方の場所でディスク容量を使用します。",
      migrateStepFreeze:
        "移行中と、完了後から再起動までの間は Mod のインポートと削除ができません。インストールとプレビューは引き続き利用できます。",
      migrateStepRestart:
        "すべての Mod の検証が終わると設定が自動で切り替わります。HMM を再起動すると新しいフォルダーが使われ、そのときに元のフォルダーの古いコピーを削除します。",
      migrateConfirm: "移行を開始",
      defaultDirectoryLabel: "既定のフォルダー",
    },
    migration: {
      title: "Mod ライブラリを移行中",
      phases: {
        "mod_storage.migration.queued": "開始待ち",
        "mod_storage.migration.copying": "コピー中",
        "mod_storage.migration.verifying": "検証中",
        "mod_storage.migration.switching": "設定を切り替え中",
        "mod_storage.migration.completed": "移行完了",
        "mod_storage.migration.failed": "移行失敗",
        "mod_storage.migration.cancelling": "キャンセル中（コピー済みの内容を削除しています）",
        "mod_storage.migration.cancelled": "キャンセル済み",
      },
      unrecognizedPhase: "不明な段階",
      progress: (current, total) => `（${current} / ${total} 個の Mod）`,
      completedTitle: "Mod ライブラリの移行が完了",
      completedMessage: "HMM を再起動すると新しいフォルダーが使われます。それまで Mod のインポートと削除はできません。",
      cancelledTitle: "移行をキャンセルしました",
      cancelledMessage: "コピー済みの内容を削除しました。保存フォルダーは変わっていません。",
      failedTitle: "移行は完了しませんでした",
      cancelFailedTitle: "移行をキャンセルできません",
    },
    errors: {
      mod_storage_dir_not_absolute: "完全な絶対パスを選んでください。",
      mod_storage_dir_unsafe: "パスに「.」や「..」を含めることはできません。",
      mod_storage_dir_filesystem_root: "ドライブのルートは直接使用できません。その下にフォルダーを作成してください。",
      mod_storage_dir_parent_missing: "そのフォルダーの親フォルダーが存在しません。",
      mod_storage_dir_not_directory: "選択したパスはフォルダーではありません。",
      mod_storage_dir_link_rejected: "そのフォルダーまたはその親がリンク / ジャンクションのため使用できません。",
      mod_storage_dir_marker_required: "そのフォルダーには既に他の内容があります。空のフォルダーか、以前 HMM が使っていたフォルダーを選んでください。",
      mod_storage_dir_marker_invalid: "このフォルダーの HMM マーカーファイルが破損しています。別のフォルダーを選んでください。",
      mod_storage_dir_not_writable: "そのフォルダーに書き込めません。権限を確認するか別の場所を選んでください。",
      mod_storage_dir_overlaps_game_root: "保存フォルダーはゲームフォルダーの中に置くことも、ゲームフォルダーを含むこともできません。",
      mod_storage_dir_overlaps_current_root: "現在の保存フォルダーと同一か入れ子になっています。別のフォルダーを選んでください。",
      mod_storage_dir_unavailable: "そのフォルダーを確認できませんでした。しばらくしてから再試行してください。",
      mod_storage_migration_required: "ライブラリに Mod があります。移行でフォルダーを変更してください。",
      app_settings_unavailable: "アプリ設定が一時的に利用できません。しばらくしてから再試行してください。",
      mod_library_unavailable: "Mod ライブラリが一時的に利用できません。しばらくしてから再試行してください。",
      game_config_unavailable: "ゲームフォルダーの設定が一時的に利用できません。しばらくしてから再試行してください。",
      mod_storage_migration_in_progress: "保存フォルダーの移行中です。完了してから操作してください。",
      mod_storage_restart_required: "保存フォルダーが変更されました。先に HMM を再起動してください。",
      mod_storage_migration_imports_active: "インポートタスクが実行中です。完了を待ってから移行してください。",
      mod_storage_migration_task_unavailable: "移行タスクを登録できませんでした。しばらくしてから再試行してください。",
      mod_storage_migration_source_unavailable: "現在の保存フォルダーを読み取れないため、移行を取り消しました。",
      mod_storage_migration_target_unavailable: "移行先フォルダーに書き込めないため、移行を取り消しました。",
      mod_storage_migration_package_unreadable: "いずれかの Mod の内容を読み取れない（リンクや想定外の項目がある）ため、移行を取り消しました。",
      mod_storage_migration_copy_failed: "コピー中にエラーが発生したため、移行を取り消しました。",
      mod_storage_migration_verify_mismatch: "コピー結果が元の内容と一致しないため、移行を取り消しました。",
      mod_storage_migration_journal_unavailable: "移行記録を書き込めませんでした。コピーは行われていません。",
      mod_storage_migration_settings_unavailable: "すべてコピーしましたが設定を保存できませんでした。コピーは削除し、フォルダーは変わっていません。",
      mod_storage_migration_cancelled: "移行はキャンセルされました。保存フォルダーは変わっていません。",
      mod_storage_migration_progress_unrecognized: "移行の進捗を認識できませんでした。設定を再読み込みして状態を確認してください。",
      mod_storage_migration_listener_unavailable: "移行の進捗を受信できません。再接続してから再試行してください。",
      mod_storage_picker_failed: "フォルダー選択ダイアログを開けません。",
      unknown: (code) => `操作に失敗しました（${code}）。`,
    },
  },
} satisfies LocaleDictionary<ModStorageCopy>;
