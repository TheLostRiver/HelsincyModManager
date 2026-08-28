import type { LocaleDictionary } from "../../shared/i18n";
import type { GameSetupErrorCode } from "./gameSetupTypes";

// 游戏目录配置（错误码表、目录动作、候选列表、启动自检通知、toast）的
// 全部用户可见文案。语义推进留在 useGameSetup / gameSetupViewModel：
// state 只存 errorCode / detailKind / 后端透传消息，文本在渲染或 toast 组装时经本字典取。

export type GameSetupCopy = {
  errors: Record<GameSetupErrorCode, string>;
  actions: {
    dialogTitle: string;
    scanSteam: string;
    manualSelect: string;
    changeDirectory: string;
  };
  candidates: {
    listAria: string;
    useDirectory: string;
  };
  startupNotice: {
    title: string;
    detailInvalidCandidate: string;
    detailNotFound: string;
    detailStartupTimeout: string;
  };
  toasts: {
    directorySavedTitle: string;
    directorySavedMessage: string;
    directorySaveFailedTitle: string;
    scanReadyTitle: string;
    scanDetectedSaved: string;
    scanAlreadyReady: string;
    candidatesFoundTitle: string;
    candidatesFoundMessage: string;
    candidatesEmptyTitle: string;
    candidatesEmptyMessage: string;
    scanFailedTitle: string;
    actionFailedTitle: string;
  };
};

export const gameSetupCopy = {
  zh_cn: {
    errors: {
      unsupported_game: "当前版本暂不支持该游戏。",
      directory_not_found: "所选目录不存在。",
      directory_not_absolute: "请选择完整的游戏安装目录，不能使用相对路径。",
      missing_executable: "所选目录缺少 MonsterHunterWorld.exe。",
      storage_failed: "配置保存失败，请检查应用数据目录权限。",
      storage_corrupted: "配置文件已损坏，请先处理应用数据目录中的 games.json。",
      scan_failed: "Steam 候选目录扫描失败，请先手动选择游戏目录。",
      scan_not_implemented: "自动扫描 Steam 尚未启用，请先手动选择目录。",
      unknown: "操作失败，请稍后重试。",
    },
    actions: {
      dialogTitle: "选择《怪物猎人：世界 冰原》游戏目录",
      scanSteam: "自动扫描 Steam",
      manualSelect: "手动选择游戏目录",
      changeDirectory: "更改游戏目录",
    },
    candidates: {
      listAria: "Steam 候选目录",
      useDirectory: "使用此目录",
    },
    startupNotice: {
      title: "需要配置游戏目录",
      detailInvalidCandidate: "Steam 返回了候选目录，但校验未通过。",
      detailNotFound: "没有找到可直接保存的 Steam 安装目录。",
      detailStartupTimeout: "启动自检超时，请重试或手动选择游戏目录。",
    },
    toasts: {
      directorySavedTitle: "游戏目录已保存",
      directorySavedMessage: "目录校验通过，当前游戏实例已准备就绪。",
      directorySaveFailedTitle: "游戏目录保存失败",
      scanReadyTitle: "游戏目录扫描完成",
      scanDetectedSaved: "已自动识别并保存 Steam 游戏目录。",
      scanAlreadyReady: "游戏目录已准备就绪。",
      candidatesFoundTitle: "发现候选游戏目录",
      candidatesFoundMessage: "已发现 Steam 候选目录。",
      candidatesEmptyTitle: "未发现候选游戏目录",
      candidatesEmptyMessage: "未发现 Steam 候选目录，可手动选择游戏目录。",
      scanFailedTitle: "游戏目录扫描失败",
      actionFailedTitle: "游戏目录操作失败",
    },
  },
  en: {
    errors: {
      unsupported_game: "This version does not support this game yet.",
      directory_not_found: "The selected directory does not exist.",
      directory_not_absolute: "Select the full game installation directory; relative paths are not allowed.",
      missing_executable: "The selected directory is missing MonsterHunterWorld.exe.",
      storage_failed: "Saving the configuration failed. Check the app data directory permissions.",
      storage_corrupted: "The configuration file is corrupted. Handle games.json in the app data directory first.",
      scan_failed: "Scanning Steam candidate directories failed. Select the game directory manually first.",
      scan_not_implemented: "Automatic Steam scanning is not enabled yet. Select the directory manually first.",
      unknown: "The operation failed. Please try again later.",
    },
    actions: {
      dialogTitle: "Select the Monster Hunter World: Iceborne game directory",
      scanSteam: "Auto-scan Steam",
      manualSelect: "Select game directory manually",
      changeDirectory: "Change game directory",
    },
    candidates: {
      listAria: "Steam candidate directories",
      useDirectory: "Use this directory",
    },
    startupNotice: {
      title: "Game directory setup required",
      detailInvalidCandidate: "Steam returned candidate directories, but they failed validation.",
      detailNotFound: "No directly savable Steam installation directory was found.",
      detailStartupTimeout: "The startup self-check timed out. Retry or select the game directory manually.",
    },
    toasts: {
      directorySavedTitle: "Game directory saved",
      directorySavedMessage: "The directory passed validation; the current game instance is ready.",
      directorySaveFailedTitle: "Saving game directory failed",
      scanReadyTitle: "Game directory scan finished",
      scanDetectedSaved: "The Steam game directory was detected and saved automatically.",
      scanAlreadyReady: "The game directory is ready.",
      candidatesFoundTitle: "Candidate game directories found",
      candidatesFoundMessage: "Steam candidate directories were found.",
      candidatesEmptyTitle: "No candidate game directories found",
      candidatesEmptyMessage: "No Steam candidate directories were found. You can select the game directory manually.",
      scanFailedTitle: "Game directory scan failed",
      actionFailedTitle: "Game directory operation failed",
    },
  },
  ja: {
    errors: {
      unsupported_game: "現在のバージョンはこのゲームに未対応です。",
      directory_not_found: "選択したディレクトリは存在しません。",
      directory_not_absolute: "完全なゲームインストールディレクトリを選択してください。相対パスは使用できません。",
      missing_executable: "選択したディレクトリに MonsterHunterWorld.exe がありません。",
      storage_failed: "設定の保存に失敗しました。アプリデータディレクトリの権限を確認してください。",
      storage_corrupted: "設定ファイルが破損しています。先にアプリデータディレクトリの games.json を処理してください。",
      scan_failed: "Steam 候補ディレクトリのスキャンに失敗しました。先に手動でゲームディレクトリを選択してください。",
      scan_not_implemented: "Steam の自動スキャンはまだ有効ではありません。先に手動でディレクトリを選択してください。",
      unknown: "操作に失敗しました。しばらくしてから再試行してください。",
    },
    actions: {
      dialogTitle: "『モンスターハンターワールド：アイスボーン』のゲームディレクトリを選択",
      scanSteam: "Steam を自動スキャン",
      manualSelect: "ゲームディレクトリを手動選択",
      changeDirectory: "ゲームディレクトリを変更",
    },
    candidates: {
      listAria: "Steam 候補ディレクトリ",
      useDirectory: "このディレクトリを使用",
    },
    startupNotice: {
      title: "ゲームディレクトリの設定が必要",
      detailInvalidCandidate: "Steam は候補ディレクトリを返しましたが、検証を通過しませんでした。",
      detailNotFound: "そのまま保存できる Steam インストールディレクトリが見つかりませんでした。",
      detailStartupTimeout: "起動時セルフチェックがタイムアウトしました。再試行するか、手動でゲームディレクトリを選択してください。",
    },
    toasts: {
      directorySavedTitle: "ゲームディレクトリを保存",
      directorySavedMessage: "ディレクトリが検証を通過し、現在のゲームインスタンスは準備完了です。",
      directorySaveFailedTitle: "ゲームディレクトリの保存に失敗",
      scanReadyTitle: "ゲームディレクトリのスキャン完了",
      scanDetectedSaved: "Steam ゲームディレクトリを自動検出して保存しました。",
      scanAlreadyReady: "ゲームディレクトリは準備完了です。",
      candidatesFoundTitle: "候補のゲームディレクトリを発見",
      candidatesFoundMessage: "Steam 候補ディレクトリが見つかりました。",
      candidatesEmptyTitle: "候補のゲームディレクトリなし",
      candidatesEmptyMessage: "Steam 候補ディレクトリが見つかりませんでした。手動でゲームディレクトリを選択できます。",
      scanFailedTitle: "ゲームディレクトリのスキャンに失敗",
      actionFailedTitle: "ゲームディレクトリの操作に失敗",
    },
  },
} satisfies LocaleDictionary<GameSetupCopy>;
