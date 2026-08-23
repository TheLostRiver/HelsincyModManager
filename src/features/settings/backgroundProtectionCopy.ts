import type { LocaleDictionary } from "../../shared/i18n";
import type { BackgroundProtectionStatus } from "./backgroundProtectionTypes";

// 后台保护的全部用户可见文案（I18N-01）。tone/action 是语义不是文案，留在
// backgroundProtectionTypes.ts 的语义表里；这里只有文本与文本模板。

export type BackgroundProtectionStatusText = { label: string; description: string };

export type BackgroundProtectionCopyDict = {
  status: Record<BackgroundProtectionStatus | "unknown", BackgroundProtectionStatusText>;
  errors: {
    permissionRequired: string;
    unsupportedPlatform: string;
    notRegistered: string;
    configurationDrift: string;
    registrationFailed: string;
    workerUnhealthy: string;
    settingsUnavailable: string;
    schedulerUnavailable: string;
    statusUnavailable: string;
    unknown: string;
  };
  duration: {
    underTenth: string;
    seconds: (value: string) => string;
  };
  panel: {
    toggleTitle: string;
    toggleDescription: string;
    switchTitleEnable: string;
    switchTitleDisable: string;
    recheck: string;
    checking: string;
    busyRefresh: string;
    busyEnable: string;
    busyDisable: string;
    operationReady: string;
    elapsed: (duration: string) => string;
    startingHintAuto: string;
    startingHintManual: string;
    refreshWarning: string;
    retryEnable: string;
    retryDisable: string;
    enabling: string;
    disabling: string;
    stopProtection: string;
    loading: BackgroundProtectionStatusText;
    unavailable: BackgroundProtectionStatusText;
    completed: {
      reconciled: string;
      refreshFailed: string;
      enableFailed: string;
      disableFailed: string;
      refreshDone: string;
      enableDone: string;
      disableDone: string;
    };
  };
  toast: {
    autoRefreshedTitle: string;
    refreshedTitle: string;
    refreshedMessage: (description: string, duration: string) => string;
    refreshFailedTitle: string;
    refreshFailedPreservedAuto: (duration: string) => string;
    refreshFailedPreservedManual: (duration: string) => string;
    refreshFailedMessage: (error: string, duration: string) => string;
    enabledTitle: string;
    disabledTitle: string;
    enabledProtectedMessage: (duration: string) => string;
    enabledStartingMessage: (duration: string) => string;
    disabledMessage: (duration: string) => string;
    reconciledMessage: (duration: string) => string;
    enableFailedTitle: string;
    disableFailedTitle: string;
    changeFailedMessage: (error: string, duration: string) => string;
  };
};

export const backgroundProtectionCopy = {
  zh_cn: {
    status: {
      not_enabled: { label: "未启用", description: "自动备份只会在客户端运行期间检查。" },
      starting: { label: "正在验证后台保护", description: "后台任务已注册，正在等待首次运行验证。" },
      protected: {
        label: "已保护",
        description: "后台任务与最近一次运行均已验证，退出客户端后仍会继续检查。",
      },
      registration_failed: {
        label: "注册未完成",
        description: "后台任务未通过完整注册检查，当前不能确认退出后仍受保护。",
      },
      worker_unhealthy: {
        label: "后台运行异常",
        description: "后台任务存在，但最近一次运行验证不可用或已经过期。",
      },
      permission_required: {
        label: "需要系统权限",
        description: "当前账户无法完成后台任务注册或检查。",
      },
      unsupported_platform: {
        label: "当前平台不支持",
        description: "此平台暂不支持退出客户端后的系统后台保护。",
      },
      unknown: { label: "状态不可用", description: "无法识别后台保护状态，请重新检查。" },
    },
    errors: {
      permissionRequired: "系统拒绝更新后台任务，请检查当前账户权限后重试。",
      unsupportedPlatform: "当前平台不支持此后台保护方式。",
      notRegistered: "系统后台任务尚未完成注册，请重试启用。",
      configurationDrift: "后台任务配置与当前版本不一致，请重试启用。",
      registrationFailed: "系统后台任务注册失败，请稍后重试。",
      workerUnhealthy: "后台运行验证不可用或已经过期，请重试启用。",
      settingsUnavailable: "后台保护设置暂时无法读取，请重新检查。",
      schedulerUnavailable: "自动备份调度状态暂时不可用，请重新检查。",
      statusUnavailable: "后台保护状态暂时不可用，请重新检查。",
      unknown: "后台保护操作未完成，请重新检查状态后重试。",
    },
    duration: {
      underTenth: "不足 0.1 秒",
      seconds: (value) => `${value} 秒`,
    },
    panel: {
      toggleTitle: "退出后继续保护自动备份",
      toggleDescription: "由系统后台任务定期唤醒现有备份流程，不改变每个 Profile 的备份计划。",
      switchTitleEnable: "开启后台保护",
      switchTitleDisable: "关闭后台保护",
      recheck: "重新检查",
      checking: "检查中",
      busyRefresh: "正在检查系统任务状态，请稍候…",
      busyEnable: "正在启用后台保护，请勿关闭 HMM…",
      busyDisable: "正在关闭后台保护，请勿关闭 HMM…",
      operationReady: "操作就绪",
      elapsed: (duration) => `耗时 ${duration}`,
      startingHintAuto:
        "HMM 正在自动复查；首次后台运行完成后会自动更新为已保护，无需重复点击。在此之前完全退出仍可能失去即时提醒。",
      startingHintManual:
        "后台任务正在等待首次运行验证；需要立即确认时可重新检查，在此之前完全退出仍可能失去即时提醒。",
      refreshWarning:
        "本次检查未完成，当前仍显示最近一次成功确认的状态；可稍后重新检查，正在验证时的自动复查不受影响。",
      retryEnable: "重试启用",
      retryDisable: "重试停用",
      enabling: "正在启用",
      disabling: "正在停用",
      stopProtection: "停用保护",
      loading: { label: "正在读取状态", description: "正在核对后台保护设置与最近运行状态。" },
      unavailable: { label: "状态不可用", description: "暂时无法确认退出客户端后的后台保护状态。" },
      completed: {
        reconciled: "系统状态已自动重新同步",
        refreshFailed: "后台保护检查未完成",
        enableFailed: "后台保护启用未完成",
        disableFailed: "后台保护关闭未完成",
        refreshDone: "后台保护检查完成",
        enableDone: "后台保护启用完成",
        disableDone: "后台保护关闭完成",
      },
    },
    toast: {
      autoRefreshedTitle: "后台保护自动验证已完成",
      refreshedTitle: "后台保护状态已更新",
      refreshedMessage: (description, duration) => `${description}，本次检查耗时 ${duration}。`,
      refreshFailedTitle: "后台保护状态检查失败",
      refreshFailedPreservedAuto: (duration) =>
        `自动复查未完成，后续复查仍会继续。耗时 ${duration}。`,
      refreshFailedPreservedManual: (duration) =>
        `本次检查未完成，仍显示最近一次成功确认的状态；可稍后重试。耗时 ${duration}。`,
      refreshFailedMessage: (error, duration) => `${error} 本次检查耗时 ${duration}。`,
      enabledTitle: "后台保护已启用",
      disabledTitle: "后台保护已关闭",
      enabledProtectedMessage: (duration) => `系统任务与最近一次后台运行均已验证。耗时 ${duration}。`,
      enabledStartingMessage: (duration) =>
        `系统任务已更新，HMM 将立即自动复查并等待首次后台运行验证；无需再次点击检查。耗时 ${duration}。`,
      disabledMessage: (duration) => `退出 HMM 后不再由系统任务检查自动备份。耗时 ${duration}。`,
      reconciledMessage: (duration) =>
        `操作确认曾短暂中断，但系统状态已自动重新读取，无需再次检查。耗时 ${duration}。`,
      enableFailedTitle: "后台保护启用失败",
      disableFailedTitle: "后台保护关闭失败",
      changeFailedMessage: (error, duration) => `${error} 耗时 ${duration}。`,
    },
  },
  en: {
    status: {
      not_enabled: {
        label: "Not enabled",
        description: "Automatic backups are only checked while the client is running.",
      },
      starting: {
        label: "Verifying background protection",
        description: "The background task is registered and waiting for its first verified run.",
      },
      protected: {
        label: "Protected",
        description:
          "The background task and its latest run are both verified; checks continue after you exit the client.",
      },
      registration_failed: {
        label: "Registration incomplete",
        description:
          "The background task failed the full registration check; protection after exit cannot be confirmed right now.",
      },
      worker_unhealthy: {
        label: "Background run unhealthy",
        description:
          "The background task exists, but its latest run verification is unavailable or expired.",
      },
      permission_required: {
        label: "System permission required",
        description: "The current account cannot complete background task registration or checks.",
      },
      unsupported_platform: {
        label: "Not supported on this platform",
        description:
          "This platform does not support system background protection after the client exits.",
      },
      unknown: {
        label: "Status unavailable",
        description: "The background protection status is unrecognized. Please check again.",
      },
    },
    errors: {
      permissionRequired:
        "The system refused to update the background task. Check the current account's permissions and try again.",
      unsupportedPlatform: "This platform does not support this background protection method.",
      notRegistered: "The system background task has not finished registering. Try enabling again.",
      configurationDrift:
        "The background task configuration does not match this version. Try enabling again.",
      registrationFailed: "System background task registration failed. Please try again later.",
      workerUnhealthy: "Background run verification is unavailable or expired. Try enabling again.",
      settingsUnavailable:
        "Background protection settings are temporarily unreadable. Please check again.",
      schedulerUnavailable:
        "The automatic backup scheduler status is temporarily unavailable. Please check again.",
      statusUnavailable:
        "Background protection status is temporarily unavailable. Please check again.",
      unknown: "The background protection operation did not complete. Check the status and try again.",
    },
    duration: {
      underTenth: "under 0.1 s",
      seconds: (value) => `${value} s`,
    },
    panel: {
      toggleTitle: "Keep protecting automatic backups after exit",
      toggleDescription:
        "A system background task periodically wakes the existing backup flow; per-profile backup schedules are unchanged.",
      switchTitleEnable: "Enable background protection",
      switchTitleDisable: "Disable background protection",
      recheck: "Check again",
      checking: "Checking",
      busyRefresh: "Checking the system task status, please wait…",
      busyEnable: "Enabling background protection, please keep HMM open…",
      busyDisable: "Disabling background protection, please keep HMM open…",
      operationReady: "Ready",
      elapsed: (duration) => `Took ${duration}`,
      startingHintAuto:
        "HMM is re-checking automatically; once the first background run completes, the status becomes Protected without further clicks. Fully exiting before then may still lose instant reminders.",
      startingHintManual:
        "The background task is waiting for its first verified run; check again for immediate confirmation. Fully exiting before then may still lose instant reminders.",
      refreshWarning:
        "This check did not complete; the last successfully confirmed status is still shown. You can check again later — automatic re-checks during verification are unaffected.",
      retryEnable: "Retry enabling",
      retryDisable: "Retry disabling",
      enabling: "Enabling",
      disabling: "Disabling",
      stopProtection: "Disable protection",
      loading: {
        label: "Reading status",
        description: "Comparing background protection settings with the latest run status.",
      },
      unavailable: {
        label: "Status unavailable",
        description: "Cannot confirm background protection after exiting the client right now.",
      },
      completed: {
        reconciled: "System state re-synced automatically",
        refreshFailed: "Background protection check incomplete",
        enableFailed: "Enabling background protection incomplete",
        disableFailed: "Disabling background protection incomplete",
        refreshDone: "Background protection check complete",
        enableDone: "Background protection enabled",
        disableDone: "Background protection disabled",
      },
    },
    toast: {
      autoRefreshedTitle: "Automatic background protection verification finished",
      refreshedTitle: "Background protection status updated",
      refreshedMessage: (description, duration) => `${description} This check took ${duration}.`,
      refreshFailedTitle: "Background protection status check failed",
      refreshFailedPreservedAuto: (duration) =>
        `The automatic re-check did not complete; future re-checks will continue. Took ${duration}.`,
      refreshFailedPreservedManual: (duration) =>
        `This check did not complete; the last successfully confirmed status is still shown. You can retry later. Took ${duration}.`,
      refreshFailedMessage: (error, duration) => `${error} This check took ${duration}.`,
      enabledTitle: "Background protection enabled",
      disabledTitle: "Background protection disabled",
      enabledProtectedMessage: (duration) =>
        `The system task and the latest background run are both verified. Took ${duration}.`,
      enabledStartingMessage: (duration) =>
        `The system task is updated; HMM will re-check automatically and wait for the first verified background run — no further clicks needed. Took ${duration}.`,
      disabledMessage: (duration) =>
        `After exiting HMM, automatic backups are no longer checked by the system task. Took ${duration}.`,
      reconciledMessage: (duration) =>
        `The confirmation was briefly interrupted, but the system state was re-read automatically — no further checks needed. Took ${duration}.`,
      enableFailedTitle: "Failed to enable background protection",
      disableFailedTitle: "Failed to disable background protection",
      changeFailedMessage: (error, duration) => `${error} Took ${duration}.`,
    },
  },
  ja: {
    status: {
      not_enabled: {
        label: "無効",
        description: "自動バックアップはクライアントの実行中のみ確認されます。",
      },
      starting: {
        label: "バックグラウンド保護を検証中",
        description: "バックグラウンドタスクは登録済みで、初回実行の検証を待っています。",
      },
      protected: {
        label: "保護中",
        description:
          "バックグラウンドタスクと直近の実行はどちらも検証済みです。クライアント終了後も確認が続きます。",
      },
      registration_failed: {
        label: "登録未完了",
        description:
          "バックグラウンドタスクが完全な登録チェックを通過していないため、終了後の保護は現時点で確認できません。",
      },
      worker_unhealthy: {
        label: "バックグラウンド実行に異常",
        description:
          "バックグラウンドタスクは存在しますが、直近の実行検証が利用できないか期限切れです。",
      },
      permission_required: {
        label: "システム権限が必要",
        description: "現在のアカウントではバックグラウンドタスクの登録や確認を完了できません。",
      },
      unsupported_platform: {
        label: "このプラットフォームは未対応",
        description:
          "このプラットフォームでは、クライアント終了後のシステムバックグラウンド保護に対応していません。",
      },
      unknown: {
        label: "状態を取得できません",
        description: "バックグラウンド保護の状態を認識できません。再確認してください。",
      },
    },
    errors: {
      permissionRequired:
        "システムがバックグラウンドタスクの更新を拒否しました。現在のアカウント権限を確認して再試行してください。",
      unsupportedPlatform: "このプラットフォームはこのバックグラウンド保護方式に対応していません。",
      notRegistered:
        "システムのバックグラウンドタスクの登録が完了していません。有効化を再試行してください。",
      configurationDrift:
        "バックグラウンドタスクの設定が現在のバージョンと一致しません。有効化を再試行してください。",
      registrationFailed:
        "システムのバックグラウンドタスク登録に失敗しました。しばらくしてから再試行してください。",
      workerUnhealthy:
        "バックグラウンド実行の検証が利用できないか期限切れです。有効化を再試行してください。",
      settingsUnavailable:
        "バックグラウンド保護の設定を一時的に読み取れません。再確認してください。",
      schedulerUnavailable:
        "自動バックアップのスケジューラー状態が一時的に利用できません。再確認してください。",
      statusUnavailable:
        "バックグラウンド保護の状態が一時的に利用できません。再確認してください。",
      unknown:
        "バックグラウンド保護の操作が完了しませんでした。状態を再確認してから再試行してください。",
    },
    duration: {
      underTenth: "0.1 秒未満",
      seconds: (value) => `${value} 秒`,
    },
    panel: {
      toggleTitle: "終了後も自動バックアップを保護",
      toggleDescription:
        "システムのバックグラウンドタスクが既存のバックアップ処理を定期的に起動します。各プロファイルのバックアップ計画は変わりません。",
      switchTitleEnable: "バックグラウンド保護を有効化",
      switchTitleDisable: "バックグラウンド保護を無効化",
      recheck: "再確認",
      checking: "確認中",
      busyRefresh: "システムタスクの状態を確認しています。お待ちください…",
      busyEnable: "バックグラウンド保護を有効化しています。HMM を閉じないでください…",
      busyDisable: "バックグラウンド保護を無効化しています。HMM を閉じないでください…",
      operationReady: "操作可能",
      elapsed: (duration) => `所要時間 ${duration}`,
      startingHintAuto:
        "HMM が自動で再確認しています。初回のバックグラウンド実行が完了すると自動的に「保護中」へ更新されます。再クリックは不要です。それまでに完全終了すると即時通知を失う可能性があります。",
      startingHintManual:
        "バックグラウンドタスクは初回実行の検証を待っています。すぐに確認したい場合は再確認してください。それまでに完全終了すると即時通知を失う可能性があります。",
      refreshWarning:
        "今回の確認は完了しませんでした。直近に確認できた状態を表示しています。後で再確認できます。検証中の自動再確認には影響しません。",
      retryEnable: "有効化を再試行",
      retryDisable: "無効化を再試行",
      enabling: "有効化中",
      disabling: "無効化中",
      stopProtection: "保護を無効化",
      loading: {
        label: "状態を読み取り中",
        description: "バックグラウンド保護の設定と直近の実行状態を照合しています。",
      },
      unavailable: {
        label: "状態を取得できません",
        description: "クライアント終了後のバックグラウンド保護状態を現在確認できません。",
      },
      completed: {
        reconciled: "システム状態を自動で再同期しました",
        refreshFailed: "バックグラウンド保護の確認が未完了",
        enableFailed: "バックグラウンド保護の有効化が未完了",
        disableFailed: "バックグラウンド保護の無効化が未完了",
        refreshDone: "バックグラウンド保護の確認が完了",
        enableDone: "バックグラウンド保護の有効化が完了",
        disableDone: "バックグラウンド保護の無効化が完了",
      },
    },
    toast: {
      autoRefreshedTitle: "バックグラウンド保護の自動検証が完了しました",
      refreshedTitle: "バックグラウンド保護の状態を更新しました",
      refreshedMessage: (description, duration) =>
        `${description} 今回の確認の所要時間：${duration}。`,
      refreshFailedTitle: "バックグラウンド保護の状態確認に失敗しました",
      refreshFailedPreservedAuto: (duration) =>
        `自動再確認は完了しませんでした。以降の再確認は継続します。所要時間：${duration}。`,
      refreshFailedPreservedManual: (duration) =>
        `今回の確認は完了しませんでした。直近に確認できた状態を表示しています。後で再試行できます。所要時間：${duration}。`,
      refreshFailedMessage: (error, duration) => `${error} 今回の確認の所要時間：${duration}。`,
      enabledTitle: "バックグラウンド保護を有効化しました",
      disabledTitle: "バックグラウンド保護を無効化しました",
      enabledProtectedMessage: (duration) =>
        `システムタスクと直近のバックグラウンド実行はどちらも検証済みです。所要時間：${duration}。`,
      enabledStartingMessage: (duration) =>
        `システムタスクを更新しました。HMM がすぐに自動再確認し、初回のバックグラウンド実行の検証を待ちます。再クリックは不要です。所要時間：${duration}。`,
      disabledMessage: (duration) =>
        `HMM 終了後は、システムタスクによる自動バックアップの確認を行いません。所要時間：${duration}。`,
      reconciledMessage: (duration) =>
        `操作確認が一時中断しましたが、システム状態を自動で再読込したため、再確認は不要です。所要時間：${duration}。`,
      enableFailedTitle: "バックグラウンド保護の有効化に失敗しました",
      disableFailedTitle: "バックグラウンド保護の無効化に失敗しました",
      changeFailedMessage: (error, duration) => `${error} 所要時間：${duration}。`,
    },
  },
} satisfies LocaleDictionary<BackgroundProtectionCopyDict>;
