// 外部来源 MOD 的状态徽标投影（#286）。**纯逻辑，不含文案**——文案在
// `modLibraryCopy`，这里只决定「用哪条、带哪些数字、按什么顺序」。
//
// ## 为什么分三档
//
// 卡片上的状态徽标只有**一个** pill，且 `.mod-card__status-label` 是
// `text-overflow: ellipsis`——过长会被 `…` 静默截断。实测各视图 pill 的可用宽度：
//
// | 视图 | pill 最大宽 | 档位 |
// |---|---|---|
// | tech | 独立全宽状态行（`mod-card__tech-status`） | 完整 |
// | classic / grid | 176px（窄屏断点降到 146 / 126px） | 精简 |
// | list | 96px（海报固定 120px） | 极简 |
//
// ## 顺序即优先级（关键）
//
// 同一档位里，「已改动」永远排在「缺失」**前面**。这样即使被截断，
// 先丢的也是次要信息——因为「已改动」要求用户做**破坏性选择**（覆盖会丢掉当前文件），
// 而「缺失」只是补不补装的问题。
//
// ## 极简档不撒谎
//
// 极简档只说「需注意 N」，**不假装知道分类**。它不声称「N 个缺失」，
// 因为 96px 里放不下分类信息，宁可少说也不说错。

export type ExternalFileState = "matched" | "missing" | "changed" | "unreadable";

export type ExternalInstallState =
  | "installed"
  | "partial"
  | "changed"
  | "mixed"
  | "not_installed"
  | "unknown";

export type ExternalInstallFileFact = {
  targetPath: string;
  state: ExternalFileState;
  /** 占用归因（#286 第三层）：该路径被哪个其他 MOD 的清单条目认领；缺席 = 无主。 */
  claimedByModId?: string;
  /** 占用者显示名（后端 getter 时解析）；MOD 已删时缺席，展示回退 id。 */
  claimedByModName?: string;
};

/** 比对集内出现过的占用者（按文件序首现去重，后端已算好）。 */
export type ExternalStateOccupier = {
  modId: string;
  modName?: string;
};

export type ExternalInstallStateSummary = {
  state: ExternalInstallState;
  matchedFileCount: number;
  missingFileCount: number;
  changedFileCount: number;
  unreadableFileCount: number;
  /** 空数组 = 无占用。哈希判定（state 与各计数）不因占用而改变。 */
  occupiedBy: ExternalStateOccupier[];
  files: ExternalInstallFileFact[];
};

// 复用 `ModLibraryPage` 里已有的定义，不另立一份——否则加视图模式时这里会静默漂移。
// 用 `import type`：node 的 type stripping 会在运行时擦掉它，因此从 .tsx 导入也不会
// 影响本模块被 node --test 直接加载（已用探针实测确认）。
import type { ModViewMode } from "./ModLibraryPage";

export type StatusBadgeTier = "full" | "compact" | "minimal";

/**
 * 徽标的语义分类。
 *
 * 与后端的 `ExternalInstallState` 一一对应，但**去掉了「读不到」这一类**：
 * 读不到的文件归进「与预期不符」，因为它同样需要用户留意，只是措辞不同。
 */
export type ExternalStatusCase =
  | "installed"
  | "not_installed"
  | "unknown"
  | "partial"
  | "changed"
  | "mixed";

export type ExternalStatusNumbers = {
  /** 内容被改动的文件数。 */
  changed: number;
  /** 读不到的文件数（可能是被占用，也可能就是被改动的那个）。 */
  unreadable: number;
  /** 游戏目录里缺失的文件数。 */
  missing: number;
};

export type ExternalStatusBadgeCopy = {
  /** 「外部来源」短标，说明不是 HMM 装的。 */
  externalOrigin: string;
  installed: string;
  notInstalled: string;
  unknown: string;
  /**
   * 追加在卡片 title/aria 末尾的过时提示（getter 重新 stat 发现指纹漂移）。
   * 弹窗的 `staleNotice` 措辞带「以下结果」这种版面指代，不能复用到卡片上。
   */
  staleHint: string;
  /** 只有缺失。 */
  partial: Record<StatusBadgeTier, (numbers: ExternalStatusNumbers) => string>;
  /** 只有「与预期不符」，没有缺失。 */
  changed: Record<StatusBadgeTier, (numbers: ExternalStatusNumbers) => string>;
  /** 两者并存。 */
  mixed: Record<StatusBadgeTier, (numbers: ExternalStatusNumbers) => string>;
  /**
   * 比对集**全部**被其他 HMM MOD 占用时的卡片改口（#286 9c）。`names` 已按
   * 「显示名 ?? id」回退且非空；单占用者带名字、多占用者报数量由文案自己分支。
   * 只有卡片投影（externalCardBadge）消费它——弹窗按拍板维持哈希徽标 + 占用提示行。
   */
  occupied: Record<StatusBadgeTier, (names: string[]) => string>;
};

export type ExternalStatusBadge = {
  /** 按档位渲染的文案；可能被 CSS 截断，但**顺序已按关键度排好**。 */
  text: string;
  /** 完整描述：档位无关，供 title / aria-label，永不截断。 */
  detail: string;
  tier: StatusBadgeTier;
  case: ExternalStatusCase;
};

// 「是否外部来源」**不进**这个结构：本模块只在外部 MOD 上被调用，
// 把它做成字段就会变成恒为 true 的冗余事实，还暗示它可能是 false。
// 需要展示「外部」标记时直接用 `copy.externalOrigin`（见 externalStatusAriaLabel）。

/**
 * 视图 → 档位。
 *
 * 依据实测宽度（见文件头），不是拍脑袋：
 * list 视图海报固定 120px，是**最窄**的，所以它用极简档。
 */
export function badgeTierForViewMode(viewMode: ModViewMode): StatusBadgeTier {
  if (viewMode === "tech") {
    return "full";
  }
  if (viewMode === "list") {
    return "minimal";
  }
  return "compact";
}

/** 从后端事实里取出徽标需要的三个数字。 */
export function externalStatusNumbers(
  summary: ExternalInstallStateSummary,
): ExternalStatusNumbers {
  return {
    changed: summary.changedFileCount,
    unreadable: summary.unreadableFileCount,
    missing: summary.missingFileCount,
  };
}

/**
 * 后端的 `state` → 徽标语义分类。
 *
 * 后端已经把「读不到」并入 `mixed`（见 `external_install_state.rs`）；
 * 这里只做一次扁平映射，不重新判定——**判定归后端，投影归前端**。
 */
export function externalStatusCase(
  state: ExternalInstallState,
): ExternalStatusCase {
  switch (state) {
    case "installed":
      return "installed";
    case "not_installed":
      return "not_installed";
    case "partial":
      return "partial";
    case "changed":
      return "changed";
    case "mixed":
      return "mixed";
    default:
      return "unknown";
  }
}

/**
 * 生成徽标。
 *
 * `detail` 永远用**完整档**文案，因此窄视图虽然显示极简文案，
 * 悬停提示与读屏拿到的仍是完整事实。
 */
export function projectExternalStatusBadge(
  summary: ExternalInstallStateSummary,
  viewMode: ModViewMode,
  copy: ExternalStatusBadgeCopy,
): ExternalStatusBadge {
  const tier = badgeTierForViewMode(viewMode);
  const badgeCase = externalStatusCase(summary.state);
  const numbers = externalStatusNumbers(summary);

  const text = badgeText(badgeCase, tier, numbers, copy);
  const detail = badgeText(badgeCase, "full", numbers, copy);

  return { text, detail, tier, case: badgeCase };
}

function badgeText(
  badgeCase: ExternalStatusCase,
  tier: StatusBadgeTier,
  numbers: ExternalStatusNumbers,
  copy: ExternalStatusBadgeCopy,
): string {
  switch (badgeCase) {
    case "installed":
      return copy.installed;
    case "not_installed":
      return copy.notInstalled;
    case "unknown":
      return copy.unknown;
    case "partial":
      return copy.partial[tier](numbers);
    case "changed":
      return copy.changed[tier](numbers);
    default:
      return copy.mixed[tier](numbers);
  }
}

/**
 * pill 上的完整无障碍标签：外部来源 + 完整事实。
 *
 * 徽标图标本身不承担语义（`aria-hidden`），所以读屏用户全靠这个串。
 */
export function externalStatusAriaLabel(
  badge: ExternalStatusBadge,
  copy: ExternalStatusBadgeCopy,
): string {
  return `${copy.externalOrigin} · ${badge.detail}`;
}

/**
 * 占用者展示名：解析不到（如 MOD 已删）回退 id。
 *
 * 回退规则集中在这一处——事实必须始终可见，绝不因为名字缺席显示空白。
 */
export function occupierDisplayName(occupier: ExternalStateOccupier): string {
  return occupier.modName ?? occupier.modId;
}

/** 文件行的占用者展示名；该文件无占用时为 `null`（行内不渲染标签）。 */
export function fileClaimantDisplayName(fact: ExternalInstallFileFact): string | null {
  if (fact.claimedByModId === undefined) {
    return null;
  }
  return fact.claimedByModName ?? fact.claimedByModId;
}

/**
 * 比对集是否**全部**被其他 HMM MOD 占用（#286 9c 卡片改口的判据）。
 * 成立时返回去重后的占用者列表（后端已按文件序首现去重），否则 `null`。
 *
 * 两条边界都刻意收紧为「不成立」：
 * - 比对集为空（unknown 态）——空集上的 every 恒真，但它什么都没证明；
 * - 每个文件都带占用标记、`occupiedBy` 却为空——DTO 自相矛盾，
 *   宁可退回哈希徽标也不凭空说「已被 0 个 MOD 占用」。
 *
 * 部分占用（哪怕只差一个文件）也返回 `null`：卡片维持哈希徽标，细节看弹窗。
 */
export function fullyOccupiedBy(
  summary: ExternalInstallStateSummary,
): ExternalStateOccupier[] | null {
  if (summary.files.length === 0 || summary.occupiedBy.length === 0) {
    return null;
  }
  if (!summary.files.every((file) => file.claimedByModId !== undefined)) {
    return null;
  }
  return summary.occupiedBy;
}
