import type { AppRouteId } from "../routing/routeTypes";
import type { TourDefinition, TourStep } from "../../shared/onboarding/tourTypes";

export const ONBOARDING_ROUTE_ORDER: readonly AppRouteId[] = [
  "dashboard",
  "mods",
  "recovery",
  "profiles",
  "backups",
  "diagnostics",
  "settings",
];

type TourFeatureStep = {
  id: string;
  title: string;
  description: string;
  target: string;
  fallbackTarget?: string;
  placement?: TourStep["placement"];
  bullets?: readonly string[];
  callout?: string;
  spotlightPadding?: number;
};

type RouteGuidance = {
  title: string;
  description: string;
  bullets: readonly string[];
  featureSteps: readonly TourFeatureStep[];
};

const routeGuidance: Record<AppRouteId, RouteGuidance> = {
  dashboard: {
    title: "工作台",
    description: "这里汇总当前游戏、目录识别、前置环境和首次设置进度。",
    bullets: ["先确认游戏目录状态。", "右侧状态区会提示当前最需要处理的事项。"],
    featureSteps: [
      {
        id: "dashboard-directory",
        title: "识别游戏目录",
        description: "先让 HMM 找到《怪物猎人：世界 冰原》的安装目录，后续 Mod 操作才会解锁。",
        target: "dashboard.directory-actions",
        fallbackTarget: "dashboard.game-setup",
        placement: "bottom-start",
        bullets: ["自动扫描会查找 Steam 库。", "自动结果不正确时再手动选择目录。"],
      },
      {
        id: "dashboard-prerequisites",
        title: "检查前置环境",
        description: "这里只读检查 Stracker's Loader 与 CRCBypass 的文件、配置和已知签名。",
        target: "dashboard.prerequisites",
        fallbackTarget: "dashboard.game-setup",
        placement: "right-start",
        bullets: ["缺失或配置错误会阻断需要前置的安装。", "“重新检查”不会写入游戏目录。"],
      },
      {
        id: "dashboard-status",
        title: "看懂设置状态",
        description: "右侧状态区把目录、配置档、前置环境和下一步行动汇总在一起。",
        target: "dashboard.setup-status",
        placement: "left-start",
        bullets: ["优先处理标记为等待或错误的项目。", "状态满足后再进入 Mod 管理。"],
      },
    ],
  },
  mods: {
    title: "Mod 管理",
    description: "这里用于导入、筛选并管理当前配置档中的 Mod。",
    bullets: ["安装前会先生成预览。", "引导不会替你导入、安装或卸载任何 Mod。"],
    featureSteps: [
      {
        id: "mods-toolbar",
        title: "搜索、筛选与切换视图",
        description: "工具栏用于按名称、作者、标签和状态缩小范围，并切换适合当前窗口的列表布局。",
        target: "mods.toolbar",
        placement: "bottom-start",
        bullets: ["搜索框按 Enter 可立即提交。", "筛选和视图只改变浏览方式，不会安装 Mod。"],
      },
      {
        id: "mods-import",
        title: "把 Mod 导入资料库",
        description: "“导入 Mod”用于添加新包；选择单个 Mod 后还可以导入新版本。",
        target: "mods.import-action",
        fallbackTarget: "mods.actions",
        placement: "bottom-start",
        bullets: ["导入只建立受控资料记录。", "从第三方管理器迁移也从快捷操作区开始。"],
      },
      {
        id: "mods-lifecycle",
        title: "先选择，再预览安装计划",
        description: "选择 Mod 后，快捷操作区会提供预览计划、安装、真正重装和卸载。",
        target: "mods.actions",
        placement: "bottom-start",
        bullets: ["先看预览中的目标文件、冲突和阻断原因。", "写入操作仍会经过原有确认与恢复机制。"],
      },
      {
        id: "mods-library",
        title: "从资料库选择 Mod",
        description: "这里显示当前查询结果；选择项目后才能查看详情或执行批量生命周期操作。",
        target: "mods.library",
        placement: "top-start",
        bullets: ["卡片和列表会显示安装状态。", "空列表时先检查筛选条件或导入 Mod。"],
      },
    ],
  },
  recovery: {
    title: "恢复中心",
    description: "这里集中显示托管安装健康状态和需要人工处理的恢复事项。",
    bullets: ["先查看健康摘要，再处理异常条目。", "回滚仍需经过原有预览与确认流程。"],
    featureSteps: [
      {
        id: "recovery-overview",
        title: "先看恢复健康摘要",
        description: "摘要会区分正常、需处理、未知和托管文件数量，帮助判断是否需要人工介入。",
        target: "recovery.overview",
        fallbackTarget: "recovery.state",
        placement: "bottom-start",
      },
      {
        id: "recovery-actions",
        title: "按建议处理异常",
        description: "人工处理区会给出推荐动作；需要回滚时仍会先生成受控预览。",
        target: "recovery.manual-actions",
        fallbackTarget: "recovery.actions",
        placement: "top-start",
        bullets: ["刷新用于重新读取事实。", "诊断导出会先确认并保持脱敏。"],
      },
      {
        id: "recovery-mods",
        title: "定位具体 Mod",
        description: "配置完成后，托管 Mod 列表会显示每个项目的文件、备份和问题数，并只在需要时提供回滚入口。",
        target: "recovery.mods",
        fallbackTarget: "recovery.state-detail",
        placement: "top-start",
        callout: "当前列表不可用时，高亮区域会说明还缺少哪些前置条件。",
      },
    ],
  },
  categories: {
    title: "分类与标签",
    description: "这里用于整理 Mod 分类和标签，让大型模组库更容易筛选。",
    bullets: ["分类只负责组织信息。", "实际安装状态仍由 Mod 管理页面维护。"],
    featureSteps: [
      {
        id: "categories-create",
        title: "新建分类",
        description: "分类是整理 Mod 的信息容器，可以设置名称、颜色和排序，不会改变安装文件。",
        target: "categories.create",
        placement: "bottom-start",
        bullets: ["例如按外观、武器或语音分类。", "标签功能尚未开放时不会误导为可用。"],
      },
      {
        id: "categories-manage",
        title: "搜索、排序与批量整理",
        description: "主列表可搜索分类、改变排序，并对选中的分类批量设置颜色或删除。",
        target: "categories.manage",
        placement: "top-start",
        bullets: ["批量删除会经过确认。", "删除分类不会直接卸载其中的 Mod。"],
      },
    ],
  },
  profiles: {
    title: "存档备份",
    description: "这里管理不同游戏场景使用的配置档、存档目录与备份计划。",
    bullets: ["同一时间只有一个活动配置档。", "删除或切换仍使用页面原有确认流程。"],
    featureSteps: [
      {
        id: "profiles-list",
        title: "新建、选择与激活配置档",
        description: "配置档是一套独立的 Mod 与存档管理场景，例如主线、联机或测试环境。",
        target: "profiles.list",
        placement: "right-start",
        bullets: ["新建后先选择，再设为活动配置档。", "后续安装和存档操作都归属当前活动配置档。"],
      },
      {
        id: "profiles-directories",
        title: "设置存档与备份目录",
        description: "游戏存档是需要保护的源目录，备份目录是 HMM 存放归档包和清单的位置。",
        target: "profiles.save-directories",
        fallbackTarget: "profiles.settings",
        placement: "left-start",
        bullets: ["可以自动检测存档目录，也可以手动选择。", "两个目录都通过校验后再保存设置。"],
      },
      {
        id: "profiles-manual-backup",
        title: "立即创建手动备份",
        description: "需要测试 Mod 或准备恢复前，可为当前配置档立即创建一个受控存档归档点。",
        target: "profiles.manual-backup",
        fallbackTarget: "profiles.settings",
        placement: "right-start",
        callout: "引导只说明入口，不会替你创建备份。",
      },
      {
        id: "profiles-auto-backup",
        title: "查看自动备份运行状态",
        description: "这里显示最近检查、下次计划和后台保护状态，并提供手动检查入口。",
        target: "profiles.auto-backup",
        fallbackTarget: "profiles.settings",
        placement: "right-start",
        bullets: ["自动备份计划属于当前配置档。", "退出后的系统保护需要在设置页单独启用。"],
      },
      {
        id: "profiles-backup-policy",
        title: "安排计划与保留策略",
        description: "这里设置手动、每日或每周计划，以及数量、天数和空间上限。",
        target: "profiles.backup-policy",
        fallbackTarget: "profiles.settings",
        placement: "right-start",
        bullets: ["恢复前安全备份默认开启。", "整理操作会保留受保护的恢复前备份。"],
      },
      {
        id: "profiles-history",
        title: "从备份历史恢复存档",
        description: "每条备份记录直接提供“恢复存档”入口，并显示来源、状态、时间和文件数。",
        target: "profiles.backup-history",
        fallbackTarget: "profiles.settings",
        placement: "top-start",
        bullets: ["恢复前会再次确认。", "默认先创建独立的恢复前安全备份。"],
      },
    ],
  },
  backups: {
    title: "备份整理",
    description: "这里集中筛选、整理并恢复已有备份。",
    bullets: ["恢复存档前默认创建独立安全备份。", "引导不会创建备份或执行恢复。"],
    featureSteps: [
      {
        id: "backups-filters",
        title: "跨配置档筛选备份",
        description: "按配置档、来源、状态或备注筛选，快速找到手动、自动和恢复前保护点。",
        target: "backups.filters",
        placement: "bottom-start",
      },
      {
        id: "backups-maintenance",
        title: "查看配额并整理备份",
        description: "配置档摘要显示数量、空间和需处理项；“立即整理”会按已保存的保留策略执行。",
        target: "backups.profiles",
        fallbackTarget: "page.backups",
        placement: "right-start",
        bullets: ["整理前会弹出确认。", "恢复前保护点不会按普通备份规则删除。"],
      },
      {
        id: "backups-history",
        title: "编辑备注或恢复存档",
        description: "备份历史直接显示恢复入口，也可以修改备注来帮助以后识别关键存档点。",
        target: "backups.history",
        fallbackTarget: "page.backups",
        placement: "left-start",
        callout: "恢复是有副作用的操作，引导不会触发。",
      },
    ],
  },
  diagnostics: {
    title: "日志与诊断",
    description: "这里显示经过校验和脱敏的运行状态，并提供受控诊断导出。",
    bullets: ["页面不会显示原始本地路径。", "导出操作仍需由你主动确认。"],
    featureSteps: [
      {
        id: "diagnostics-actions",
        title: "刷新或导出诊断包",
        description: "刷新会重新读取安全摘要；导出会先要求确认，再由后端生成脱敏支持材料。",
        target: "diagnostics.actions",
        placement: "bottom-start",
      },
      {
        id: "diagnostics-health",
        title: "检查证据健康状态",
        description: "健康卡汇总平台、App、Debug、Task、Audit 日志和日志空间是否可用。",
        target: "diagnostics.health",
        fallbackTarget: "diagnostics.state",
        placement: "bottom-start",
      },
      {
        id: "diagnostics-logs",
        title: "阅读安全日志与审计摘要",
        description: "日志区只展示允许显示的内容；稳定错误码和任务标识可用于反馈问题。",
        target: "diagnostics.logs",
        fallbackTarget: "page.diagnostics",
        placement: "top-start",
      },
    ],
  },
  settings: {
    title: "设置",
    description: "这里调整外观、关闭行为、前置检查和存档后台保护等选项。",
    bullets: ["正式保存项会明确反馈结果。", "预览设置与持久化设置会分别标识。"],
    featureSteps: [
      {
        id: "settings-appearance",
        title: "调整界面与启动偏好",
        description: "紧凑面板、减少动效和启动页面属于界面偏好；标记为预览的项目只影响当前会话。",
        target: "settings.appearance",
        placement: "left-start",
      },
      {
        id: "settings-window-behavior",
        title: "选择窗口关闭行为",
        description: "这里决定点击关闭按钮时询问、收起到托盘或完全退出。",
        target: "settings.window-behavior",
        placement: "left-start",
        bullets: ["窗口关闭偏好会正式保存。", "它与后台保护是否启用是两件独立的事。"],
      },
      {
        id: "settings-prerequisites",
        title: "在设置页重新检查前置",
        description: "前置环境区域与工作台使用同一套只读检查结果，可在修复文件后重新检查。",
        target: "settings.prerequisites",
        placement: "left-start",
      },
      {
        id: "settings-background-protection",
        title: "管理退出后的后台保护",
        description: "开启后，系统任务会在 HMM 退出后继续唤醒现有自动备份流程。",
        target: "settings.background-protection",
        fallbackTarget: "settings.save-backup",
        placement: "left-start",
        bullets: ["开关不会修改配置档自己的备份计划。", "“重新检查”只验证系统任务与 worker 状态。"],
      },
    ],
  },
};

type BuildOnboardingTourOptions = {
  includeWelcome?: boolean;
};

export function buildOnboardingTour(
  startRouteId: AppRouteId,
  { includeWelcome = false }: BuildOnboardingTourOptions = {},
): TourDefinition {
  const routeOrder = rotateRoutesFrom(startRouteId);
  const steps: TourStep[] = includeWelcome ? [welcomeStep] : [];

  routeOrder.forEach((routeId, index) => {
    const guidance = routeGuidance[routeId];
    const isLastRoute = index === routeOrder.length - 1;

    steps.push({
      id: `page-${routeId}`,
      title: guidance.title,
      description: guidance.description,
      target: `page.${routeId}`,
      placement: "bottom-start",
      bullets: guidance.bullets,
      callout: "接下来会逐一高亮本页最重要的操作区；这里只说明用途，不会执行操作。",
      primaryLabel: "查看本页功能",
      spotlightPadding: 0,
      interaction: "blocked",
      advance: { kind: "controls" },
    });

    guidance.featureSteps.forEach((feature, featureIndex) => {
      const isLastFeature = featureIndex === guidance.featureSteps.length - 1;
      const isFinalTourStep = isLastRoute && isLastFeature;

      steps.push({
        ...feature,
        primaryLabel: isFinalTourStep ? "完成引导" : "继续",
        spotlightPadding: feature.spotlightPadding ?? 6,
        interaction: "blocked",
        advance: isFinalTourStep ? { kind: "terminal" } : { kind: "controls" },
      });
    });

    if (!isLastRoute) {
      const nextRouteId = routeOrder[index + 1];
      const nextGuidance = routeGuidance[nextRouteId];
      steps.push({
        id: `navigate-${nextRouteId}`,
        title: `进入${nextGuidance.title}`,
        description: `请点击左侧高亮的“${nextGuidance.title}”，进入页面后引导会自动继续。`,
        target: `nav.${nextRouteId}`,
        placement: "right-start",
        callout: "只有高亮目标可以操作；引导不会替你点击。",
        primaryLabel: "等待点击",
        spotlightPadding: 5,
        interaction: "target-only",
        advance: { kind: "route-change", expectedRouteId: nextRouteId },
      });
    }
  });

  return {
    id: "hmm.first-run",
    contentVersion: 4,
    steps,
  };
}

export function rotateRoutesFrom(startRouteId: AppRouteId) {
  const startIndex = ONBOARDING_ROUTE_ORDER.indexOf(startRouteId);
  if (startIndex < 0) return [...ONBOARDING_ROUTE_ORDER];
  return [
    ...ONBOARDING_ROUTE_ORDER.slice(startIndex),
    ...ONBOARDING_ROUTE_ORDER.slice(0, startIndex),
  ];
}

const welcomeStep: TourStep = {
  id: "welcome",
  title: "欢迎使用 HMM",
  description: "安全、清晰地管理《怪物猎人：世界 冰原》Mod，并保留可追踪、可恢复的操作记录。",
  features: [
    { icon: "shield", title: "安全安装", description: "安装前预览变更，失败时保留回滚与恢复证据。" },
    { icon: "layers", title: "Mod 管理", description: "集中导入、安装、卸载和真正重装 Mod。" },
    { icon: "profiles", title: "配置档", description: "将 Mod、游戏与存档操作归入明确的使用场景。" },
    { icon: "backup", title: "存档保护", description: "创建备份点，并通过受控流程恢复玩家存档。" },
  ],
  callout: "接下来从当前页面开始，按高亮提示亲自完成导航。",
  primaryLabel: "开始引导",
  interaction: "blocked",
  advance: { kind: "controls" },
};
