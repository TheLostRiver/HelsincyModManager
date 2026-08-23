import type { LocaleDictionary } from "../../shared/i18n";
import type { GameLaunchErrorCode } from "./gameLaunchTypes";

// 一键启动（错误码表与启动反馈）的全部用户可见文案。
// useGameLaunch 的 state 只存 errorCode / outcome，文本在渲染时经本字典取。

export type GameLaunchCopy = {
  errors: Record<GameLaunchErrorCode, string>;
  requestSent: string;
};

export const gameLaunchCopy = {
  zh_cn: {
    errors: {
      unsupported_game: "当前版本暂不支持启动该游戏。",
      game_not_configured: "请先配置游戏目录，再启动游戏。",
      storage_corrupted: "游戏配置文件已损坏，无法读取启动配置。",
      storage_failed: "游戏配置读取失败，请检查应用数据目录权限。",
      launcher_unavailable: "系统未能打开游戏启动器。",
      launch_failed: "启动请求发送失败，请稍后重试。",
      unknown: "启动游戏失败，请稍后重试。",
    },
    requestSent: "启动请求已发送。",
  },
  en: {
    errors: {
      unsupported_game: "This version does not support launching this game yet.",
      game_not_configured: "Configure the game directory before launching the game.",
      storage_corrupted: "The game configuration file is corrupted; the launch configuration cannot be read.",
      storage_failed: "Reading the game configuration failed. Check the app data directory permissions.",
      launcher_unavailable: "The system could not open the game launcher.",
      launch_failed: "Sending the launch request failed. Please try again later.",
      unknown: "Launching the game failed. Please try again later.",
    },
    requestSent: "Launch request sent.",
  },
  ja: {
    errors: {
      unsupported_game: "現在のバージョンはこのゲームの起動に未対応です。",
      game_not_configured: "先にゲームディレクトリを設定してから起動してください。",
      storage_corrupted: "ゲーム設定ファイルが破損しており、起動設定を読み取れません。",
      storage_failed: "ゲーム設定の読み取りに失敗しました。アプリデータディレクトリの権限を確認してください。",
      launcher_unavailable: "システムがゲームランチャーを開けませんでした。",
      launch_failed: "起動リクエストの送信に失敗しました。しばらくしてから再試行してください。",
      unknown: "ゲームの起動に失敗しました。しばらくしてから再試行してください。",
    },
    requestSent: "起動リクエストを送信しました。",
  },
} satisfies LocaleDictionary<GameLaunchCopy>;
