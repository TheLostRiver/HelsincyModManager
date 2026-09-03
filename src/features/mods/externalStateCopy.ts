// #286 external mod state: badge tiers + detail-dialog section copy.
//
// Badge wording follows the decisions pinned in issue #286:
// - three tiers, ordered by decision criticality (changed before missing);
// - the minimal tier says "needs attention N" without pretending to know the
//   breakdown — it must not lie in 96px;
// - "external origin" is an orthogonal short label attached to the pill.

import type { LocaleDictionary } from "../../shared/i18n/locales";
import type { ExternalStatusBadgeCopy } from "./externalInstallStatusView";

export type ExternalStateSectionCopy = {
  badge: ExternalStatusBadgeCopy;
  title: string;
  /** One-line promise: read-only judgement based on the game directory. */
  intro: string;
  checkAction: string;
  rescanAction: string;
  scanning: string;
  neverScanned: string;
  /** Shown for the `unknown` state: the comparison set was empty. */
  unknownHint: string;
  staleNotice: string;
  /**
   * Summary line when some compared files are claimed by HMM-managed mods
   * (#286 attribution). Joins the occupier names itself: list separators
   * differ per locale. `names` are already display-name-or-id resolved.
   */
  occupiedNotice: (names: string[], claimedFileCount: number) => string;
  /** Per-file-row claim tag; `name` is already display-name-or-id resolved. */
  fileClaimedBy: (name: string) => string;
  /** Shown instead of the action when no profile is selected. */
  profileRequired: string;
  fileListCaption: string;
  fileHeaderPath: string;
  fileHeaderState: string;
  fileState: {
    matched: string;
    missing: string;
    changed: string;
    unreadable: string;
  };
  errors: {
    stale: string;
    cancelled: string;
    modUnavailable: string;
    gameInstanceUnavailable: string;
    generic: (code: string) => string;
  };
};

/** Stable-code → human message; unknown codes keep the code visible. */
export function externalStateErrorMessage(
  code: string,
  copy: ExternalStateSectionCopy,
): string {
  switch (code) {
    case "external_state_scan_stale":
      return copy.errors.stale;
    case "external_state_scan_cancelled":
      return copy.errors.cancelled;
    case "external_state_scan_mod_unavailable":
      return copy.errors.modUnavailable;
    case "external_state_scan_game_instance_unavailable":
      return copy.errors.gameInstanceUnavailable;
    default:
      return copy.errors.generic(code);
  }
}

export const externalStateCopy = {
  zh_cn: {
    badge: {
      externalOrigin: "外部",
      installed: "已安装",
      notInstalled: "未安装",
      unknown: "无法判定",
      staleHint: "结果可能已过时，建议重新检查",
      partial: {
        full: (n) => `部分安装 · ${n.missing} 个文件缺失`,
        compact: (n) => `缺失 ${n.missing}`,
        minimal: (n) => `需注意 ${n.missing}`,
      },
      changed: {
        full: (n) => `已被改动 · ${n.changed + n.unreadable} 个文件`,
        compact: (n) => `已改动 ${n.changed + n.unreadable}`,
        minimal: (n) => `需注意 ${n.changed + n.unreadable}`,
      },
      mixed: {
        full: (n) => {
          const base = `已被改动 · ${n.changed + n.unreadable} 个文件`;
          return n.missing > 0 ? `${base} · 另有 ${n.missing} 个缺失` : base;
        },
        compact: (n) =>
          n.missing > 0
            ? `已改动 ${n.changed + n.unreadable} · 缺失 ${n.missing}`
            : `已改动 ${n.changed + n.unreadable}`,
        minimal: (n) => `需注意 ${n.changed + n.unreadable + n.missing}`,
      },
    },
    title: "游戏目录状态",
    intro: "以游戏目录的实际文件为准，与本 MOD 的导入包逐个比对。只读检查，不会修改任何文件。",
    checkAction: "检查游戏目录",
    rescanAction: "重新检查",
    scanning: "正在比对导入包与游戏目录…",
    neverScanned:
      "尚未检查过。用其他工具装进游戏的文件不会出现在 HMM 的记录里，点「检查游戏目录」按实际内容判定。",
    unknownHint:
      "导入包里没有解析出任何可安装文件，无法比对。这通常是包的目录结构问题（例如 nativePC 目录的大小写变体）。",
    staleNotice: "游戏目录的文件可能已发生变化，以下结果仅供参考，建议重新检查。",
    occupiedNotice: (names, claimedFileCount) =>
      `比对集中有 ${claimedFileCount} 个文件已被 HMM 名下的 MOD 占用：${names.join("、")}。它们是 HMM 管理的安装内容，不是外部安装的文件。`,
    fileClaimedBy: (name) => `已被「${name}」占用`,
    profileRequired: "当前没有可用的配置档，无法检查。",
    fileListCaption: "文件明细",
    fileHeaderPath: "文件",
    fileHeaderState: "状态",
    fileState: {
      matched: "一致",
      missing: "缺失",
      changed: "已被改动",
      unreadable: "读不到",
    },
    errors: {
      stale: "检查期间有安装正在进行或文件发生了变化，本次结果已丢弃，可稍后重试。",
      cancelled: "检查已取消。",
      modUnavailable: "找不到该 MOD 的导入记录，无法比对。",
      gameInstanceUnavailable: "游戏目录不可用，请先在设置中配置游戏目录。",
      generic: (code) => `检查未完成（${code}）。`,
    },
  },
  en: {
    badge: {
      externalOrigin: "External",
      installed: "Installed",
      notInstalled: "Not installed",
      unknown: "Undetermined",
      staleHint: "May be outdated, check again",
      partial: {
        full: (n) => `Partially installed · ${n.missing} missing`,
        compact: (n) => `Missing ${n.missing}`,
        minimal: (n) => `Attention ${n.missing}`,
      },
      changed: {
        full: (n) => `Modified · ${n.changed + n.unreadable} files`,
        compact: (n) => `Modified ${n.changed + n.unreadable}`,
        minimal: (n) => `Attention ${n.changed + n.unreadable}`,
      },
      mixed: {
        full: (n) => {
          const base = `Modified · ${n.changed + n.unreadable} files`;
          return n.missing > 0 ? `${base} · ${n.missing} missing` : base;
        },
        compact: (n) =>
          n.missing > 0
            ? `Modified ${n.changed + n.unreadable} · missing ${n.missing}`
            : `Modified ${n.changed + n.unreadable}`,
        minimal: (n) => `Attention ${n.changed + n.unreadable + n.missing}`,
      },
    },
    title: "Game directory status",
    intro:
      "Compares this mod's imported package against the actual files in the game directory. Read-only — nothing is modified.",
    checkAction: "Check game directory",
    rescanAction: "Check again",
    scanning: "Comparing the package with the game directory…",
    neverScanned:
      "Not checked yet. Files installed by other tools never appear in HMM's records; run the check to judge by what is actually on disk.",
    unknownHint:
      "The imported package yielded no installable files, so there is nothing to compare. This usually means a package layout problem (for example a case variant of the nativePC directory).",
    staleNotice:
      "Files in the game directory may have changed. Treat the result below as a hint and check again.",
    occupiedNotice: (names, claimedFileCount) =>
      `${claimedFileCount} of the compared files are claimed by HMM-managed mods: ${names.join(", ")}. They are managed installs, not external content.`,
    fileClaimedBy: (name) => `Claimed by "${name}"`,
    profileRequired: "No profile is available, so the check cannot run.",
    fileListCaption: "File details",
    fileHeaderPath: "File",
    fileHeaderState: "State",
    fileState: {
      matched: "Matched",
      missing: "Missing",
      changed: "Modified",
      unreadable: "Unreadable",
    },
    errors: {
      stale:
        "An install was in progress or files changed during the check; this result was discarded. Try again later.",
      cancelled: "Check cancelled.",
      modUnavailable: "No import record exists for this mod, so there is nothing to compare.",
      gameInstanceUnavailable:
        "The game directory is unavailable. Configure it in settings first.",
      generic: (code) => `Check did not finish (${code}).`,
    },
  },
  ja: {
    badge: {
      externalOrigin: "外部",
      installed: "導入済み",
      notInstalled: "未導入",
      unknown: "判定不可",
      staleHint: "結果が古い可能性があります。再確認してください",
      partial: {
        full: (n) => `一部導入 · 欠損 ${n.missing} 件`,
        compact: (n) => `欠損 ${n.missing}`,
        minimal: (n) => `要確認 ${n.missing}`,
      },
      changed: {
        full: (n) => `変更あり · ${n.changed + n.unreadable} 件`,
        compact: (n) => `変更 ${n.changed + n.unreadable}`,
        minimal: (n) => `要確認 ${n.changed + n.unreadable}`,
      },
      mixed: {
        full: (n) => {
          const base = `変更あり · ${n.changed + n.unreadable} 件`;
          return n.missing > 0 ? `${base} · 欠損 ${n.missing} 件` : base;
        },
        compact: (n) =>
          n.missing > 0
            ? `変更 ${n.changed + n.unreadable} · 欠損 ${n.missing}`
            : `変更 ${n.changed + n.unreadable}`,
        minimal: (n) => `要確認 ${n.changed + n.unreadable + n.missing}`,
      },
    },
    title: "ゲームディレクトリの状態",
    intro:
      "ゲームディレクトリの実ファイルを基準に、この MOD のインポートパッケージと照合します。読み取り専用で、ファイルは変更しません。",
    checkAction: "ゲームディレクトリを確認",
    rescanAction: "再確認",
    scanning: "パッケージとゲームディレクトリを照合しています…",
    neverScanned:
      "まだ確認していません。他のツールで導入されたファイルは HMM の記録に現れないため、実際の内容で判定するには確認を実行してください。",
    unknownHint:
      "インポートパッケージから導入可能なファイルを解析できず、照合できません。パッケージ構造の問題（例：nativePC ディレクトリの大文字小文字の違い）が原因のことが多いです。",
    staleNotice:
      "ゲームディレクトリのファイルが変化した可能性があります。以下は参考情報として扱い、再確認してください。",
    occupiedNotice: (names, claimedFileCount) =>
      `比較対象のうち ${claimedFileCount} 件は HMM 管理下の MOD が占有しています：${names.join("、")}。外部導入の内容ではなく、HMM が管理するファイルです。`,
    fileClaimedBy: (name) => `「${name}」が占有中`,
    profileRequired: "利用可能なプロファイルがないため、確認できません。",
    fileListCaption: "ファイル詳細",
    fileHeaderPath: "ファイル",
    fileHeaderState: "状態",
    fileState: {
      matched: "一致",
      missing: "欠損",
      changed: "変更あり",
      unreadable: "読み取り不可",
    },
    errors: {
      stale:
        "確認中にインストールが進行していたか、ファイルが変化したため、今回の結果は破棄されました。後で再試行してください。",
      cancelled: "確認をキャンセルしました。",
      modUnavailable: "この MOD のインポート記録が見つからないため、照合できません。",
      gameInstanceUnavailable:
        "ゲームディレクトリが利用できません。先に設定で構成してください。",
      generic: (code) => `確認が完了しませんでした（${code}）。`,
    },
  },
} satisfies LocaleDictionary<ExternalStateSectionCopy>;
