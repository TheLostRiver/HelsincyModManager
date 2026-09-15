import { resolveCopy, useI18n } from "../../shared/i18n";
import { useModInstallation } from "./ModInstallationProvider";
import "./ModInstallationNotice.css";

const copyByLocale = {
  zh_cn: {
    unavailable: "暂时无法读取当前游戏的 Mod 安装记录。",
    gameUnavailable: "配置游戏目录后即可读取 Mod 安装状态。",
    legacyAmbiguous: "检测到多份旧 Mod 安装记录，暂时无法确认归属。原记录和备份已保留，完成记录核对前不能继续安装或卸载。",
    retry: "重新读取",
  },
  en: {
    unavailable: "Mod installation records for the current game are unavailable.",
    gameUnavailable: "Configure the game directory to read Mod installation status.",
    legacyAmbiguous: "Multiple legacy Mod installation records need reconciliation. Records and backups are preserved; installation and removal are paused until ownership is resolved.",
    retry: "Retry",
  },
  ja: {
    unavailable: "現在のゲームの Mod インストール記録を読み取れません。",
    gameUnavailable: "ゲームフォルダーを設定すると Mod インストール状態を読み取れます。",
    legacyAmbiguous: "複数の旧 Mod インストール記録があり、所有関係の確認が必要です。記録とバックアップは保持されています。確認が済むまでインストールとアンインストールは実行できません。",
    retry: "再読み込み",
  },
};

export function ModInstallationNotice() {
  const { locale } = useI18n();
  const { installationScope, refreshInstallationScope } = useModInstallation();
  if (installationScope.status !== "unavailable") return null;
  const copy = resolveCopy(copyByLocale, locale);
  const gameUnavailable = installationScope.code === "mod_installation_game_unavailable";
  return <div className="mod-installation-notice" role="status">
    <span>{gameUnavailable ? copy.gameUnavailable
      : installationScope.code === "mod_installation_legacy_ambiguous" ? copy.legacyAmbiguous : copy.unavailable}</span>
    {!gameUnavailable && <button type="button" onClick={refreshInstallationScope}>{copy.retry}</button>}
  </div>;
}
