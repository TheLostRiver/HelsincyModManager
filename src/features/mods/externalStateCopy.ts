// #286 external mod state: badge tiers + detail-dialog section copy.
//
// Badge wording follows the decisions pinned in issue #286:
// - three tiers, ordered by decision criticality (changed before missing);
// - the minimal tier says "needs attention N" without pretending to know the
//   breakdown — it must not lie in 96px;
// - "external origin" is an orthogonal short label attached to the pill;
// - `occupied` (9c) replaces the hash badge on the card when every compared
//   file is claimed by other HMM-managed mods. Full tier reads as a sentence
//   and names a single occupier (count for several); the compact tier puts
//   the fact first and the name last, so ellipsis truncation drops the name
//   (fully available in title/aria) rather than the fact; the minimal tier
//   states the fact only — 96px has no room for a name.

import type { LocaleDictionary } from "../../shared/i18n/locales";
import type { ExternalAdoptBlockedReason, ExternalAdoptCounts } from "./externalAdoptView";
import type { ExternalStatusBadgeCopy } from "./externalInstallStatusView";

/**
 * Every stable code an adopt can end with (contract: 「#286 外部 MOD 接管」).
 * Keyed as a type so all three locales must spell out each one — a missing key
 * would otherwise fall back to the generic message and only show on a real device.
 * `external_mod_adopt_audit_unavailable` is not here: it rides on a *completed*
 * event (adopt succeeded, audit did not) and has its own notice.
 */
export type ExternalAdoptErrorCode =
  | "external_mod_adopt_game_instance_unavailable"
  | "external_mod_adopt_mod_unavailable"
  | "external_mod_adopt_scan_required"
  | "external_mod_adopt_unreadable_files"
  | "external_mod_adopt_nothing_to_adopt"
  | "external_mod_adopt_already_installed"
  | "external_mod_adopt_manifest_not_trusted"
  | "external_mod_adopt_manifest_unavailable"
  | "external_mod_adopt_manifest_write_failed"
  | "external_mod_adopt_game_file_unavailable"
  | "external_mod_adopt_stale"
  | "external_mod_adopt_cancelled"
  | "external_mod_adopt_unavailable"
  | "external_mod_adopt_task_unavailable"
  | "recovery_pending"
  | "recovery_unavailable"
  | "write_safety_rejected"
  | "write_admission_busy"
  | "write_admission_cancelled"
  | "write_admission_order_violation"
  | "write_admission_unavailable";

export type ExternalAdoptCopy = {
  /** Action button label; the count is the claimable file count. */
  action: (claimable: number) => string;
  /** Action button label before a scan produced anything claimable. */
  actionIdle: string;
  adopting: string;
  /** Hint under the actions while the button is disabled; `no_summary` shows no hint. */
  blocked: Record<Exclude<ExternalAdoptBlockedReason, "no_summary">, string>;
  confirm: {
    title: string;
    closeAria: string;
    /** What adopt does — and does not — do. */
    body: (claimable: number) => string;
    /** Only rendered when something is skipped; lists the non-zero reasons. */
    skipped: (counts: ExternalAdoptCounts) => string;
    /** The one consequence the player must know before confirming. */
    uninstallWarning: string;
    cancel: string;
    confirm: (claimable: number) => string;
  };
  /** Dialog status message after the manifest was written. */
  completed: (claimable: number) => string;
  /** Appended when the completed event carries `external_mod_adopt_audit_unavailable`. */
  completedAuditDegraded: string;
  errors: Record<ExternalAdoptErrorCode, string> & {
    generic: (code: string) => string;
  };
};

export type ExternalStateSectionCopy = {
  adopt: ExternalAdoptCopy;
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

function isKnownAdoptErrorCode(
  code: string,
  errors: ExternalAdoptCopy["errors"],
): code is ExternalAdoptErrorCode {
  return code !== "generic" && Object.prototype.hasOwnProperty.call(errors, code);
}

/** Adopt stable-code → human message; unknown codes keep the code visible. */
export function externalAdoptErrorMessage(code: string, copy: ExternalStateSectionCopy): string {
  const errors = copy.adopt.errors;
  return isKnownAdoptErrorCode(code, errors) ? errors[code] : errors.generic(code);
}

export const externalStateCopy = {
  zh_cn: {
    adopt: {
      action: (claimable) => `接管 ${claimable} 个文件`,
      actionIdle: "接管为 HMM 管理",
      adopting: "正在接管…",
      blocked: {
        unknown: "没有可比对的文件，无法接管。",
        unreadable: "有文件读不到（可能正被游戏或其他程序占用），关闭它们并重新检查后才能接管。",
        stale: "检查结果可能已过时，重新检查后才能接管。",
        nothing_to_adopt: "没有可接管的文件：需要文件与导入包一致，且未被其他 MOD 占用。",
      },
      confirm: {
        title: "接管为 HMM 管理",
        closeAria: "关闭接管确认",
        body: (claimable) =>
          `将把游戏目录里与本 MOD 导入包一致的 ${claimable} 个文件登记为 HMM 管理的安装内容。只写入安装记录，不会复制、修改或删除任何文件。`,
        skipped: (counts) => {
          const parts: string[] = [];
          if (counts.skippedChanged > 0) parts.push(`已被改动 ${counts.skippedChanged} 个`);
          if (counts.skippedMissing > 0) parts.push(`缺失 ${counts.skippedMissing} 个`);
          if (counts.skippedClaimed > 0) parts.push(`已被其他 MOD 占用 ${counts.skippedClaimed} 个`);
          return `以下文件不会被接管：${parts.join("、")}。`;
        },
        uninstallWarning:
          "接管后卸载会直接删除这些文件，且无法自动还原原版——它们不是由 HMM 写入的，没有备份。需要恢复原版时，请用 HMM 重新安装本 MOD 后再卸载，或用 Steam 验证游戏文件完整性。",
        cancel: "取消",
        confirm: (claimable) => `确认接管 ${claimable} 个文件`,
      },
      completed: (claimable) => `已接管 ${claimable} 个文件，现在由 HMM 管理。`,
      completedAuditDegraded: "审计记录写入失败，不影响接管结果。",
      errors: {
        external_mod_adopt_game_instance_unavailable: "游戏目录不可用，请先在设置中配置游戏目录。",
        external_mod_adopt_mod_unavailable: "找不到该 MOD 的导入记录，无法接管。",
        external_mod_adopt_scan_required: "请先「检查游戏目录」，接管以那次检查结果为准。",
        external_mod_adopt_unreadable_files:
          "有文件读不到（可能正被游戏或其他程序占用），关闭它们并重新检查后再试。",
        external_mod_adopt_nothing_to_adopt:
          "没有可接管的文件：需要文件与导入包一致，且未被其他 MOD 占用。",
        external_mod_adopt_already_installed: "该 MOD 已由 HMM 管理，不需要接管；如需更新请使用重装。",
        external_mod_adopt_manifest_not_trusted:
          "该配置档的安装记录处于进行中或异常状态，请先到恢复中心处理。",
        external_mod_adopt_manifest_unavailable: "读取安装记录失败，请稍后重试。",
        external_mod_adopt_manifest_write_failed: "写入安装记录失败，未做任何更改，可直接重试。",
        external_mod_adopt_game_file_unavailable: "无法读取游戏目录的文件信息，请稍后重试。",
        external_mod_adopt_stale:
          "检查之后游戏目录或安装记录发生了变化，本次未接管。请重新检查后再试。",
        external_mod_adopt_cancelled: "接管已取消，未做任何更改。",
        external_mod_adopt_unavailable: "接管暂时不可用，请稍后重试。",
        external_mod_adopt_task_unavailable: "无法启动接管任务，请稍后重试。",
        recovery_pending: "接管被待处理的恢复状态阻断，请先到恢复中心处理。",
        recovery_unavailable: "恢复状态暂时无法确认，暂不能接管。",
        write_safety_rejected: "当前配置不允许写入该游戏目录，接管已阻止。",
        write_admission_busy: "另一项操作正在使用该游戏目录，请稍后重试。",
        write_admission_cancelled: "等待写入许可时任务被取消。",
        write_admission_order_violation: "写入许可顺序校验失败，请重启应用后重试。",
        write_admission_unavailable: "无法取得写入许可，请稍后重试。",
        generic: (code) => `接管未完成（${code}）。`,
      },
    },
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
      occupied: {
        full: (names) =>
          names.length === 1 ? `已被「${names[0]}」占用` : `已被 ${names.length} 个 MOD 占用`,
        compact: (names) =>
          names.length === 1 ? `已被占用 · ${names[0]}` : `已被占用 · ${names.length} 个 MOD`,
        minimal: () => "已被占用",
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
    adopt: {
      action: (claimable) => `Adopt ${claimable} files`,
      actionIdle: "Adopt into HMM",
      adopting: "Adopting…",
      blocked: {
        unknown: "There are no files to compare, so nothing can be adopted.",
        unreadable:
          "Some files are unreadable (possibly held open by the game or another program). Close them and check again before adopting.",
        stale: "The check result may be outdated. Check again before adopting.",
        nothing_to_adopt:
          "Nothing to adopt: files must match the imported package and not be claimed by another mod.",
      },
      confirm: {
        title: "Adopt into HMM",
        closeAria: "Close adopt confirmation",
        body: (claimable) =>
          `${claimable} files in the game directory that match this mod's imported package will be recorded as an HMM-managed install. Only the install record is written — no file is copied, modified or deleted.`,
        skipped: (counts) => {
          const parts: string[] = [];
          if (counts.skippedChanged > 0) parts.push(`${counts.skippedChanged} modified`);
          if (counts.skippedMissing > 0) parts.push(`${counts.skippedMissing} missing`);
          if (counts.skippedClaimed > 0) parts.push(`${counts.skippedClaimed} claimed by other mods`);
          return `Not adopted: ${parts.join(", ")}.`;
        },
        uninstallWarning:
          "Uninstalling after adoption deletes these files and cannot restore the originals — HMM never wrote them, so there is no backup. To get the originals back, reinstall this mod with HMM and uninstall it, or verify the game files through Steam.",
        cancel: "Cancel",
        confirm: (claimable) => `Adopt ${claimable} files`,
      },
      completed: (claimable) => `Adopted ${claimable} files; this mod is now managed by HMM.`,
      completedAuditDegraded: "The audit record could not be written; the adoption itself is unaffected.",
      errors: {
        external_mod_adopt_game_instance_unavailable:
          "The game directory is unavailable. Configure it in settings first.",
        external_mod_adopt_mod_unavailable: "No import record exists for this mod, so it cannot be adopted.",
        external_mod_adopt_scan_required:
          "Run “Check game directory” first — adoption uses that check result.",
        external_mod_adopt_unreadable_files:
          "Some files are unreadable (possibly held open by the game or another program). Close them, check again and retry.",
        external_mod_adopt_nothing_to_adopt:
          "Nothing to adopt: files must match the imported package and not be claimed by another mod.",
        external_mod_adopt_already_installed:
          "This mod is already managed by HMM; use reinstall to update it.",
        external_mod_adopt_manifest_not_trusted:
          "This profile's install record is in progress or in an abnormal state. Resolve it in the recovery center first.",
        external_mod_adopt_manifest_unavailable: "Reading the install record failed. Try again later.",
        external_mod_adopt_manifest_write_failed:
          "Writing the install record failed; nothing was changed. You can retry right away.",
        external_mod_adopt_game_file_unavailable:
          "File information in the game directory could not be read. Try again later.",
        external_mod_adopt_stale:
          "The game directory or the install record changed after the check; nothing was adopted. Check again and retry.",
        external_mod_adopt_cancelled: "Adoption cancelled; nothing was changed.",
        external_mod_adopt_unavailable: "Adoption is temporarily unavailable. Try again later.",
        external_mod_adopt_task_unavailable: "The adopt task could not be started. Try again later.",
        recovery_pending:
          "Adoption is blocked by a pending recovery state. Resolve it in the recovery center first.",
        recovery_unavailable: "The recovery state cannot be confirmed right now, so adoption is blocked.",
        write_safety_rejected: "The current configuration does not allow writing to this game directory; adoption was blocked.",
        write_admission_busy: "Another operation is using this game directory. Try again later.",
        write_admission_cancelled: "The task was cancelled while waiting for write admission.",
        write_admission_order_violation:
          "Write admission ordering check failed. Restart the app and try again.",
        write_admission_unavailable: "Write admission could not be obtained. Try again later.",
        generic: (code) => `Adoption did not finish (${code}).`,
      },
    },
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
      occupied: {
        full: (names) =>
          names.length === 1 ? `Claimed by "${names[0]}"` : `Claimed by ${names.length} mods`,
        compact: (names) =>
          names.length === 1 ? `Claimed · ${names[0]}` : `Claimed · ${names.length} mods`,
        minimal: () => "Claimed",
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
    adopt: {
      action: (claimable) => `${claimable} 件を引き継ぐ`,
      actionIdle: "HMM の管理に引き継ぐ",
      adopting: "引き継いでいます…",
      blocked: {
        unknown: "照合できるファイルがないため、引き継げません。",
        unreadable:
          "読み取れないファイルがあります（ゲームや他のプログラムが開いている可能性）。閉じてから再確認すると引き継げます。",
        stale: "確認結果が古い可能性があります。再確認してから引き継いでください。",
        nothing_to_adopt:
          "引き継げるファイルがありません。インポートパッケージと一致し、他の MOD が占有していないファイルが必要です。",
      },
      confirm: {
        title: "HMM の管理に引き継ぐ",
        closeAria: "引き継ぎの確認を閉じる",
        body: (claimable) =>
          `ゲームディレクトリ内でこの MOD のインポートパッケージと一致する ${claimable} 件を、HMM 管理の導入内容として記録します。書き込むのは導入記録のみで、ファイルのコピー・変更・削除は行いません。`,
        skipped: (counts) => {
          const parts: string[] = [];
          if (counts.skippedChanged > 0) parts.push(`変更あり ${counts.skippedChanged} 件`);
          if (counts.skippedMissing > 0) parts.push(`欠損 ${counts.skippedMissing} 件`);
          if (counts.skippedClaimed > 0) parts.push(`他の MOD が占有 ${counts.skippedClaimed} 件`);
          return `次のファイルは引き継ぎません：${parts.join("、")}。`;
        },
        uninstallWarning:
          "引き継ぎ後にアンインストールすると、これらのファイルは削除され、元のファイルを自動で復元できません。HMM が書き込んだものではないためバックアップがありません。元に戻す必要がある場合は、HMM でこの MOD を再インストールしてからアンインストールするか、Steam でゲームファイルの整合性を確認してください。",
        cancel: "キャンセル",
        confirm: (claimable) => `${claimable} 件を引き継ぐ`,
      },
      completed: (claimable) => `${claimable} 件を引き継ぎました。この MOD は HMM が管理します。`,
      completedAuditDegraded: "監査記録の書き込みに失敗しましたが、引き継ぎ自体には影響しません。",
      errors: {
        external_mod_adopt_game_instance_unavailable:
          "ゲームディレクトリが利用できません。先に設定で構成してください。",
        external_mod_adopt_mod_unavailable: "この MOD のインポート記録が見つからないため、引き継げません。",
        external_mod_adopt_scan_required:
          "先に「ゲームディレクトリを確認」を実行してください。引き継ぎはその確認結果に基づきます。",
        external_mod_adopt_unreadable_files:
          "読み取れないファイルがあります（ゲームや他のプログラムが開いている可能性）。閉じて再確認してから再試行してください。",
        external_mod_adopt_nothing_to_adopt:
          "引き継げるファイルがありません。インポートパッケージと一致し、他の MOD が占有していないファイルが必要です。",
        external_mod_adopt_already_installed:
          "この MOD はすでに HMM が管理しています。更新する場合は再インストールを使用してください。",
        external_mod_adopt_manifest_not_trusted:
          "このプロファイルの導入記録が処理中または異常な状態です。先にリカバリーセンターで対処してください。",
        external_mod_adopt_manifest_unavailable: "導入記録の読み取りに失敗しました。後で再試行してください。",
        external_mod_adopt_manifest_write_failed:
          "導入記録の書き込みに失敗しました。変更は行われていないため、すぐに再試行できます。",
        external_mod_adopt_game_file_unavailable:
          "ゲームディレクトリのファイル情報を読み取れません。後で再試行してください。",
        external_mod_adopt_stale:
          "確認後にゲームディレクトリまたは導入記録が変化したため、今回は引き継ぎませんでした。再確認してから再試行してください。",
        external_mod_adopt_cancelled: "引き継ぎをキャンセルしました。変更はありません。",
        external_mod_adopt_unavailable: "引き継ぎは一時的に利用できません。後で再試行してください。",
        external_mod_adopt_task_unavailable: "引き継ぎタスクを開始できませんでした。後で再試行してください。",
        recovery_pending:
          "未処理のリカバリー状態により引き継ぎがブロックされています。先にリカバリーセンターで対処してください。",
        recovery_unavailable: "リカバリー状態を確認できないため、現在は引き継げません。",
        write_safety_rejected: "現在の構成ではこのゲームディレクトリへの書き込みが許可されていないため、引き継ぎをブロックしました。",
        write_admission_busy: "別の操作がこのゲームディレクトリを使用中です。後で再試行してください。",
        write_admission_cancelled: "書き込み許可を待機中にタスクがキャンセルされました。",
        write_admission_order_violation:
          "書き込み許可の順序チェックに失敗しました。アプリを再起動して再試行してください。",
        write_admission_unavailable: "書き込み許可を取得できませんでした。後で再試行してください。",
        generic: (code) => `引き継ぎが完了しませんでした（${code}）。`,
      },
    },
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
      occupied: {
        full: (names) =>
          names.length === 1 ? `「${names[0]}」が占有中` : `${names.length} 個の MOD が占有中`,
        compact: (names) =>
          names.length === 1 ? `占有中 · ${names[0]}` : `占有中 · ${names.length} 個`,
        minimal: () => "占有中",
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
