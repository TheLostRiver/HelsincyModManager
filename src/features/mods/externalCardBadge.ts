// #286 切片 3b-2：列表卡片消费外部状态扫描结果的投影（方案 A，拍板见 issue #286）。
//
// 结果来源是「会话级共享 state」：详情弹窗每次拿到 getter 结果就上报一份，
// 列表页存进内存 Map，卡片据此显示徽标。**不落盘、不主动失效**——翻页仍在
// （Map 挂在页组件上），路由切换/重启后消失；后端进程内缓存仍在，重开详情即回。
// 这与「工作态不落盘」的既有区分一致。
//
// 门禁与详情弹窗一致：只有 HMM manifest 不认领（not_installed）的 MOD 才谈
// 「外部状态」。已安装/异常态的卡片继续显示既有安装状态——即使 Map 里残留着
// 安装前的扫描结果，装完后事实已由 manifest 接管，残留结果不得再上卡片。
//
// ## 9c：全占用改口
//
// 比对集**全部**被其他 HMM MOD 占用时，哈希徽标会如实报「已安装」——字节一致的
// 两份导入互相看对方装的文件都是「一致」。但那些文件是 HMM 自己另一个 MOD 名下的
// 安装内容，「外部 · 已安装」在这里是误导（维护者真机验收时的第一反应就是误判）。
// 因此卡片改口「已被 X 占用」，且 title/aria **不再带「外部」前缀**。部分占用维持
// 哈希徽标不变，占用细节在弹窗文件明细（9b）。弹窗徽标本身按拍板不改口。

import type { ExternalModStateDto } from "./externalStateApi";
import type { ModInstallStatus } from "./modLibraryTypes";
// 与 externalInstallStatusView 同一手法：type-only import 会被 node 的
// type stripping 擦掉，因此本模块仍可被 node --test 直接加载。
import type { ModViewMode } from "./ModLibraryPage";
// 值导入必须带 .ts 扩展名：本模块被 node --test 直接加载，node 的解析器不做
// 无扩展名补全（type-only 导入被擦除，不受此限）。
import {
  badgeTierForViewMode,
  externalStatusAriaLabel,
  fullyOccupiedBy,
  occupierDisplayName,
  projectExternalStatusBadge,
  type ExternalInstallStateSummary,
  type ExternalStatusBadgeCopy,
  type ExternalStatusCase,
} from "./externalInstallStatusView.ts";

/**
 * 卡片徽标的语义分类：哈希六态之外多一个 `occupied`。
 * 它只存在于卡片层——弹窗的 `ExternalStatusCase` 不含它（弹窗不改口）。
 */
export type ExternalCardBadgeCase = ExternalStatusCase | "occupied";

export type ExternalCardBadge = {
  /** 状态文案位的档位文案；可能被 CSS 截断，但截断顺序已按关键度排好。 */
  text: string;
  /**
   * title/aria 用的完整事实，永不截断：完整档文案 + 过时提示。
   * 哈希徽标带「外部」前缀；全占用改口不带（见文件头）。
   */
  label: string;
  case: ExternalCardBadgeCase;
  /** getter 重新 stat 时发现事实可能漂移；label 已带文字提示，此标志留给样式。 */
  stale: boolean;
};

/**
 * 卡片状态位的徽标投影。返回 null 表示「维持既有安装状态显示」：
 * 要么这不是 manifest 不认领的 MOD，要么本会话还没有它的扫描结果。
 */
export function projectExternalCardBadge(input: {
  installStatus: ModInstallStatus;
  externalState: ExternalModStateDto | null | undefined;
  viewMode: ModViewMode;
  copy: ExternalStatusBadgeCopy;
}): ExternalCardBadge | null {
  const { installStatus, externalState, viewMode, copy } = input;
  if (installStatus !== "not_installed") {
    return null;
  }
  if (!externalState || externalState.summary === null) {
    return null;
  }

  const { text, label: base, case: badgeCase } = projectCardBadgeText(
    externalState.summary,
    viewMode,
    copy,
  );
  const label = externalState.stale ? `${base} · ${copy.staleHint}` : base;

  return { text, label, case: badgeCase, stale: externalState.stale };
}

function projectCardBadgeText(
  summary: ExternalInstallStateSummary,
  viewMode: ModViewMode,
  copy: ExternalStatusBadgeCopy,
): { text: string; label: string; case: ExternalCardBadgeCase } {
  const occupiers = fullyOccupiedBy(summary);
  if (occupiers !== null) {
    const names = occupiers.map(occupierDisplayName);
    return {
      text: copy.occupied[badgeTierForViewMode(viewMode)](names),
      // 占用者是 HMM 自己的 MOD，「外部」前缀在这里就是要消灭的误导，不加。
      label: copy.occupied.full(names),
      case: "occupied",
    };
  }

  const badge = projectExternalStatusBadge(summary, viewMode, copy);
  return {
    text: badge.text,
    label: externalStatusAriaLabel(badge, copy),
    case: badge.case,
  };
}
