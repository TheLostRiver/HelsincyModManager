import type { LocaleDictionary } from "../../shared/i18n";
import type { AppRouteId } from "../routing/routeTypes";

// routes 的键集必须与 AppRouteId 完全一致（builder 以 AppRouteId 索引，缺键即编译失败）。
type RouteKeyCheck = keyof OnboardingTourCopy["routes"] extends AppRouteId
  ? AppRouteId extends keyof OnboardingTourCopy["routes"]
    ? true
    : never
  : never;

// 新手引导（firstRunTour）全部步骤内容的三语字典（原 I18N-07 范围）。
// 步骤 id / 高亮目标 / 位置等语义留在 firstRunTour.ts；本字典按步骤 id 精确锁定
// 每种语言的标题、描述、要点与提示，缺任一语言的 key 直接编译失败。

export type TourFeatureCopy = {
  title: string;
  description: string;
  bullets?: readonly string[];
  callout?: string;
};

type RouteGuidanceCopy<TFeatureId extends string> = {
  title: string;
  description: string;
  bullets: readonly string[];
  features: Record<TFeatureId, TourFeatureCopy>;
};

export type OnboardingTourCopy = {
  routes: {
    dashboard: RouteGuidanceCopy<
      "dashboard-steam-scan" | "dashboard-manual-directory" | "dashboard-launch-game" | "dashboard-prerequisites"
    >;
    mods: RouteGuidanceCopy<"mods-import" | "mods-library" | "mods-lifecycle">;
    recovery: RouteGuidanceCopy<"recovery-overview" | "recovery-actions" | "recovery-mods">;
    categories: RouteGuidanceCopy<"categories-create" | "categories-manage">;
    profiles: RouteGuidanceCopy<
      | "profiles-list"
      | "profiles-directories"
      | "profiles-manual-backup"
      | "profiles-auto-backup"
      | "profiles-backup-policy"
      | "profiles-history"
    >;
    backups: RouteGuidanceCopy<"backups-filters" | "backups-maintenance" | "backups-history">;
    diagnostics: RouteGuidanceCopy<"diagnostics-actions" | "diagnostics-health" | "diagnostics-logs">;
    settings: RouteGuidanceCopy<"settings-background-protection">;
    about: RouteGuidanceCopy<"about-release" | "about-links">;
  };
  welcome: {
    title: string;
    description: string;
    features: {
      shield: { title: string; description: string };
      layers: { title: string; description: string };
      profiles: { title: string; description: string };
      backup: { title: string; description: string };
    };
    callout: string;
    primaryLabel: string;
  };
  builder: {
    pageLocalCallout: string;
    pageLocalPrimary: string;
    continueLabel: string;
    finishLabel: string;
    waitClickLabel: string;
    navigateTitle: (routeTitle: string) => string;
    navigateDescription: (routeTitle: string) => string;
    navCallout: string;
  };
};

export const onboardingTourCopy = {
  zh_cn: {
    routes: {
      dashboard: {
        title: "工作台",
        description: "这里汇总当前游戏、目录识别、前置环境和首次设置进度。",
        bullets: ["先识别游戏目录，再检查前置环境。", "引导只说明入口，不会自动扫描或启动游戏。"],
        features: {
          "dashboard-steam-scan": {
            title: "自动扫描 Steam",
            description: "优先让 HMM 从 Steam 库中查找《怪物猎人：世界 冰原》的安装目录。",
            bullets: ["扫描只读取 Steam 库配置。", "找到候选后仍会校验游戏目录。"],
          },
          "dashboard-manual-directory": {
            title: "手动选择游戏目录",
            description: "自动扫描没有找到游戏时，可以直接选择包含 MonsterHunterWorld.exe 的目录。",
          },
          "dashboard-launch-game": {
            title: "启动游戏",
            description: "游戏目录通过校验后，可以从工作台直接启动当前游戏。",
            callout: "目录尚未配置时按钮会保持不可用。",
          },
          "dashboard-prerequisites": {
            title: "检查前置环境",
            description: "这里只读检查 Stracker's Loader 与 CRCBypass 的文件、配置和已知签名。",
            bullets: ["缺失或配置错误会阻断需要前置的安装。", "“重新检查”不会写入游戏目录。"],
          },
        },
      },
      mods: {
        title: "Mod 管理",
        description: "这里用于导入、筛选并管理当前配置档中的 Mod。",
        bullets: ["安装前会先生成预览。", "引导不会替你导入、安装或卸载任何 Mod。"],
        features: {
          "mods-import": {
            title: "把 Mod 导入资料库",
            description: "“导入 Mod”用于添加新包；选择单个 Mod 后还可以导入新版本。",
            bullets: ["导入只建立受控资料记录。", "从第三方管理器迁移也从快捷操作区开始。"],
          },
          "mods-library": {
            title: "从资料库选择 Mod",
            description: "这里显示当前查询结果；选择项目后才能查看详情或执行批量生命周期操作。",
            bullets: ["卡片和列表会显示安装状态。", "空列表时先检查筛选条件或导入 Mod。"],
          },
          "mods-lifecycle": {
            title: "先选择，再预览安装计划",
            description: "选择 Mod 后，快捷操作区会提供预览计划、安装、真正重装和卸载。",
            bullets: ["先看预览中的目标文件、冲突和阻断原因。", "写入操作仍会经过原有确认与恢复机制。"],
          },
        },
      },
      recovery: {
        title: "恢复中心",
        description: "这里集中显示托管安装健康状态和需要人工处理的恢复事项。",
        bullets: ["先查看健康摘要，再处理异常条目。", "回滚仍需经过原有预览与确认流程。"],
        features: {
          "recovery-overview": {
            title: "先看恢复健康摘要",
            description: "摘要会区分正常、需处理、未知和托管文件数量，帮助判断是否需要人工介入。",
          },
          "recovery-actions": {
            title: "按建议处理异常",
            description: "人工处理区会给出推荐动作；需要回滚时仍会先生成受控预览。",
            bullets: ["刷新用于重新读取事实。", "诊断导出会先确认并保持脱敏。"],
          },
          "recovery-mods": {
            title: "定位具体 Mod",
            description: "配置完成后，托管 Mod 列表会显示每个项目的文件、备份和问题数，并只在需要时提供回滚入口。",
            callout: "当前列表不可用时，高亮区域会说明还缺少哪些前置条件。",
          },
        },
      },
      categories: {
        title: "分类与标签",
        description: "这里用于整理 Mod 分类和标签，让大型模组库更容易筛选。",
        bullets: ["分类只负责组织信息。", "实际安装状态仍由 Mod 管理页面维护。"],
        features: {
          "categories-create": {
            title: "新建分类",
            description: "分类是整理 Mod 的信息容器，可以设置名称、颜色和排序，不会改变安装文件。",
            bullets: ["例如按外观、武器或语音分类。", "标签功能尚未开放时不会误导为可用。"],
          },
          "categories-manage": {
            title: "搜索、排序与批量整理",
            description: "主列表可搜索分类、改变排序，并对选中的分类批量设置颜色或删除。",
            bullets: ["批量删除会经过确认。", "删除分类不会直接卸载其中的 Mod。"],
          },
        },
      },
      profiles: {
        title: "存档备份",
        description: "这里管理不同游戏场景使用的配置档、存档目录与备份计划。",
        bullets: ["同一时间只有一个活动配置档。", "删除或切换仍使用页面原有确认流程。"],
        features: {
          "profiles-list": {
            title: "新建、选择与激活配置档",
            description: "配置档是一套独立的 Mod 与存档管理场景，例如主线、联机或测试环境。",
            bullets: ["新建后先选择，再设为活动配置档。", "后续安装和存档操作都归属当前活动配置档。"],
          },
          "profiles-directories": {
            title: "设置存档与备份目录",
            description: "游戏存档是需要保护的源目录，备份目录是 HMM 存放归档包和清单的位置。",
            bullets: ["可以自动检测存档目录，也可以手动选择。", "两个目录都通过校验后再保存设置。"],
          },
          "profiles-manual-backup": {
            title: "立即创建手动备份",
            description: "需要测试 Mod 或准备恢复前，可为当前配置档立即创建一个受控存档归档点。",
            callout: "引导只说明入口，不会替你创建备份。",
          },
          "profiles-auto-backup": {
            title: "查看自动备份运行状态",
            description: "这里显示最近检查、下次计划和后台保护状态，并提供手动检查入口。",
            bullets: ["自动备份计划属于当前配置档。", "退出后的系统保护需要在设置页单独启用。"],
          },
          "profiles-backup-policy": {
            title: "安排计划与保留策略",
            description: "这里设置手动、每日或每周计划，以及数量、天数和空间上限。",
            bullets: ["恢复前安全备份默认开启。", "整理操作会保留受保护的恢复前备份。"],
          },
          "profiles-history": {
            title: "从备份历史恢复存档",
            description: "每条备份记录直接提供“恢复存档”入口，并显示来源、状态、时间和文件数。",
            bullets: ["恢复前会再次确认。", "默认先创建独立的恢复前安全备份。"],
          },
        },
      },
      backups: {
        title: "备份整理",
        description: "这里集中筛选、整理并恢复已有备份。",
        bullets: ["恢复存档前默认创建独立安全备份。", "引导不会创建备份或执行恢复。"],
        features: {
          "backups-filters": {
            title: "跨配置档筛选备份",
            description: "按配置档、来源、状态或备注筛选，快速找到手动、自动和恢复前保护点。",
          },
          "backups-maintenance": {
            title: "查看配额并整理备份",
            description: "配置档摘要显示数量、空间和需处理项；“立即整理”会按已保存的保留策略执行。",
            bullets: ["整理前会弹出确认。", "恢复前保护点不会按普通备份规则删除。"],
          },
          "backups-history": {
            title: "编辑备注或恢复存档",
            description: "备份历史直接显示恢复入口，也可以修改备注来帮助以后识别关键存档点。",
            callout: "恢复是有副作用的操作，引导不会触发。",
          },
        },
      },
      diagnostics: {
        title: "日志与诊断",
        description: "这里显示经过校验和脱敏的运行状态，并提供受控诊断导出。",
        bullets: ["页面不会显示原始本地路径。", "导出操作仍需由你主动确认。"],
        features: {
          "diagnostics-actions": {
            title: "刷新或导出诊断包",
            description: "刷新会重新读取安全摘要；导出会先要求确认，再由后端生成脱敏支持材料。",
          },
          "diagnostics-health": {
            title: "检查证据健康状态",
            description: "健康卡汇总平台、App、Debug、Task、Audit 日志和日志空间是否可用。",
          },
          "diagnostics-logs": {
            title: "阅读安全日志与审计摘要",
            description: "日志区只展示允许显示的内容；稳定错误码和任务标识可用于反馈问题。",
          },
        },
      },
      settings: {
        title: "设置",
        description: "这里调整外观、关闭行为、前置检查和存档后台保护等选项。",
        bullets: ["这里只说明退出后的存档后台保护。", "配置档自己的备份计划仍在存档备份页维护。"],
        features: {
          "settings-background-protection": {
            title: "管理退出后的后台保护",
            description: "开启后，系统任务会在 HMM 退出后继续唤醒现有自动备份流程。",
            bullets: ["开关不会修改配置档自己的备份计划。", "“重新检查”只验证系统任务与 worker 状态。"],
          },
        },
      },
      about: {
        title: "关于 HMM",
        description: "这里集中显示当前版本、发布入口、项目地址、赞助说明和意见反馈渠道。",
        bullets: ["当前版本来自安装产物。", "所有外部入口都会在系统浏览器中打开。"],
        features: {
          "about-release": {
            title: "查看版本与更新",
            description: "先确认当前 HMM 版本，再前往 GitHub Releases 检查新版本或阅读更新记录。",
          },
          "about-links": {
            title: "找到项目支持入口",
            description: "作者主页、开源仓库、赞助说明和 Issues 都集中在这里。",
            bullets: ["功能建议和缺陷请提交到 Issues。", "敏感安全问题请按仓库安全策略私下报告。"],
          },
        },
      },
    },
    welcome: {
      title: "欢迎使用 HMM",
      description: "安全、清晰地管理《怪物猎人：世界 冰原》Mod，并保留可追踪、可恢复的操作记录。",
      features: {
        shield: { title: "安全安装", description: "安装前预览变更，失败时保留回滚与恢复证据。" },
        layers: { title: "Mod 管理", description: "集中导入、安装、卸载和真正重装 Mod。" },
        profiles: { title: "配置档", description: "将 Mod、游戏与存档操作归入明确的使用场景。" },
        backup: { title: "存档保护", description: "创建备份点，并通过受控流程恢复玩家存档。" },
      },
      callout: "接下来从当前页面开始，按高亮提示亲自完成导航。",
      primaryLabel: "开始引导",
    },
    builder: {
      pageLocalCallout: "接下来只介绍当前页面的重要功能，不会自动进入其他页面或执行操作。",
      pageLocalPrimary: "查看本页功能",
      continueLabel: "继续",
      finishLabel: "完成引导",
      waitClickLabel: "等待点击",
      navigateTitle: (routeTitle: string) => `进入${routeTitle}`,
      navigateDescription: (routeTitle: string) =>
        `请点击左侧高亮的“${routeTitle}”，进入页面后引导会自动继续。`,
      navCallout: "只有高亮目标可以操作；引导不会替你点击。",
    },
  },
  en: {
    routes: {
      dashboard: {
        title: "Workbench",
        description: "This page aggregates the current game, directory identification, prerequisites, and first-run setup progress.",
        bullets: ["Identify the game directory first, then check the prerequisites.", "The tour only explains entries; it never scans or launches the game for you."],
        features: {
          "dashboard-steam-scan": {
            title: "Auto-scan Steam",
            description: "Let HMM look up the Monster Hunter World: Iceborne installation directory from the Steam library first.",
            bullets: ["The scan only reads the Steam library configuration.", "Candidates are still validated as game directories."],
          },
          "dashboard-manual-directory": {
            title: "Select the game directory manually",
            description: "When auto-scan finds nothing, pick the directory containing MonsterHunterWorld.exe directly.",
          },
          "dashboard-launch-game": {
            title: "Launch the game",
            description: "Once the game directory passes validation, the current game can be launched right from the workbench.",
            callout: "The button stays disabled until the directory is configured.",
          },
          "dashboard-prerequisites": {
            title: "Check the prerequisites",
            description: "This read-only check covers the files, configuration, and known signatures of Stracker's Loader and CRCBypass.",
            bullets: ["Missing or misconfigured prerequisites block installs that need them.", "\"Recheck\" never writes to the game directory."],
          },
        },
      },
      mods: {
        title: "Mod management",
        description: "Import, filter, and manage the mods of the current profile here.",
        bullets: ["A preview is generated before installing.", "The tour never imports, installs, or uninstalls mods for you."],
        features: {
          "mods-import": {
            title: "Import mods into the library",
            description: "\"Import mod\" adds new packages; with a single mod selected you can also import a new version.",
            bullets: ["Importing only creates controlled library records.", "Migration from third-party managers also starts from the quick actions area."],
          },
          "mods-library": {
            title: "Pick mods from the library",
            description: "The current query results appear here; select items to view details or run batch lifecycle actions.",
            bullets: ["Cards and the list show install states.", "With an empty list, check the filters or import mods first."],
          },
          "mods-lifecycle": {
            title: "Select first, then preview the install plan",
            description: "With mods selected, the quick actions area offers plan preview, install, true reinstall, and uninstall.",
            bullets: ["Review target files, conflicts, and blockers in the preview first.", "Write operations still go through the existing confirmation and recovery flows."],
          },
        },
      },
      recovery: {
        title: "Recovery Center",
        description: "Managed install health and recovery items needing manual handling are gathered here.",
        bullets: ["Read the health summary first, then handle abnormal entries.", "Rollbacks still go through the existing preview and confirmation flow."],
        features: {
          "recovery-overview": {
            title: "Start with the recovery health summary",
            description: "The summary distinguishes healthy, action-needed, unknown, and managed file counts to help judge whether manual intervention is needed.",
          },
          "recovery-actions": {
            title: "Handle anomalies as recommended",
            description: "The manual handling area suggests actions; rollbacks still generate a controlled preview first.",
            bullets: ["Refresh re-reads the facts.", "Diagnostics export confirms first and stays redacted."],
          },
          "recovery-mods": {
            title: "Locate a specific mod",
            description: "Once configured, the managed mod list shows the file, backup, and issue counts per item, offering rollback entries only when needed.",
            callout: "When the list is unavailable, the highlighted area explains which prerequisites are still missing.",
          },
        },
      },
      categories: {
        title: "Categories & tags",
        description: "Organize mod categories and tags here to keep large libraries filterable.",
        bullets: ["Categories only organize information.", "Actual install states are still maintained on the mod management page."],
        features: {
          "categories-create": {
            title: "Create a category",
            description: "A category is an informational container for organizing mods — name, color, and order never change installed files.",
            bullets: ["For example: appearance, weapons, or voice packs.", "The tags feature is not misrepresented as available before it opens."],
          },
          "categories-manage": {
            title: "Search, sort, and batch-organize",
            description: "The main list supports searching, reordering, and batch color or delete actions on selected categories.",
            bullets: ["Batch deletion asks for confirmation.", "Deleting a category never uninstalls the mods in it."],
          },
        },
      },
      profiles: {
        title: "Save backups",
        description: "Manage the profiles, save data directories, and backup schedules used by different play scenarios.",
        bullets: ["Only one profile is active at a time.", "Deleting or switching still uses the page's existing confirmation flows."],
        features: {
          "profiles-list": {
            title: "Create, select, and activate profiles",
            description: "A profile is an independent mod and save data management scenario — e.g. main story, multiplayer, or testing.",
            bullets: ["After creating, select it and then make it the active profile.", "Later installs and save data operations belong to the current active profile."],
          },
          "profiles-directories": {
            title: "Set the save data and backup directories",
            description: "The game save data is the protected source directory; the backup directory is where HMM stores archives and manifests.",
            bullets: ["The save data directory can be auto-detected or selected manually.", "Save the settings after both directories pass validation."],
          },
          "profiles-manual-backup": {
            title: "Create a manual backup now",
            description: "Before testing mods or preparing a restore, create a controlled save data archive point for the current profile immediately.",
            callout: "The tour only explains the entry; it never creates backups for you.",
          },
          "profiles-auto-backup": {
            title: "Review the auto backup runtime",
            description: "The last check, next schedule, and background protection state appear here, with a manual check entry.",
            bullets: ["The auto backup schedule belongs to the current profile.", "System protection after exit is enabled separately on the Settings page."],
          },
          "profiles-backup-policy": {
            title: "Arrange schedules and retention",
            description: "Set manual, daily, or weekly schedules here, plus the count, age, and space limits.",
            bullets: ["The pre-restore safety backup is on by default.", "Pruning keeps protected pre-restore backups."],
          },
          "profiles-history": {
            title: "Restore save data from the backup history",
            description: "Each backup record offers a direct \"Restore save data\" entry, showing the source, status, time, and file count.",
            bullets: ["Restores are confirmed again beforehand.", "An independent pre-restore safety backup is created first by default."],
          },
        },
      },
      backups: {
        title: "Backup maintenance",
        description: "Filter, prune, and restore existing backups in one place.",
        bullets: ["An independent safety backup is created before restoring by default.", "The tour never creates backups or performs restores."],
        features: {
          "backups-filters": {
            title: "Filter backups across profiles",
            description: "Filter by profile, source, status, or notes to quickly find manual, auto, and pre-restore protection points.",
          },
          "backups-maintenance": {
            title: "Review quotas and prune backups",
            description: "Profile summaries show counts, space, and action-needed items; \"Prune now\" runs by the saved retention policy.",
            bullets: ["Pruning asks for confirmation first.", "Pre-restore protection points are not deleted by regular backup rules."],
          },
          "backups-history": {
            title: "Edit notes or restore save data",
            description: "The backup history shows restore entries directly; notes can be edited to identify key save points later.",
            callout: "Restoring has side effects; the tour never triggers it.",
          },
        },
      },
      diagnostics: {
        title: "Logs & diagnostics",
        description: "Verified and redacted runtime state appears here, with a controlled diagnostics export.",
        bullets: ["The page never shows raw local paths.", "Exports still require your explicit confirmation."],
        features: {
          "diagnostics-actions": {
            title: "Refresh or export diagnostics",
            description: "Refresh re-reads the safe summary; exporting asks for confirmation first, then the backend generates redacted support material.",
          },
          "diagnostics-health": {
            title: "Check evidence health",
            description: "Health cards aggregate the availability of the platform, App, Debug, Task, and Audit logs plus log storage.",
          },
          "diagnostics-logs": {
            title: "Read safe logs and audit summaries",
            description: "The log area only shows permitted content; stable error codes and task identifiers can be used when reporting issues.",
          },
        },
      },
      settings: {
        title: "Settings",
        description: "Adjust appearance, close behavior, prerequisite checks, and save data background protection here.",
        bullets: ["Only the post-exit save data background protection is explained here.", "Each profile's own backup schedule is still maintained on the save backups page."],
        features: {
          "settings-background-protection": {
            title: "Manage post-exit background protection",
            description: "When enabled, a system task keeps waking the existing auto backup flow after HMM exits.",
            bullets: ["The toggle never modifies a profile's own backup schedule.", "\"Recheck\" only verifies the system task and worker state."],
          },
        },
      },
      about: {
        title: "About HMM",
        description: "The current version, release entry, project links, sponsorship notes, and feedback channels are gathered here.",
        bullets: ["The current version comes from the installed artifact.", "All external entries open in the system browser."],
        features: {
          "about-release": {
            title: "Check the version and updates",
            description: "Confirm the current HMM version first, then visit GitHub Releases for new versions or changelogs.",
          },
          "about-links": {
            title: "Find the project support entries",
            description: "The author page, open-source repository, sponsorship notes, and Issues are all here.",
            bullets: ["Submit feature requests and defects to Issues.", "Report sensitive security issues privately per the repository security policy."],
          },
        },
      },
    },
    welcome: {
      title: "Welcome to HMM",
      description: "Manage Monster Hunter World: Iceborne mods safely and clearly, with traceable, recoverable operation records.",
      features: {
        shield: { title: "Safe installs", description: "Preview changes before installing; rollback and recovery evidence is kept on failure." },
        layers: { title: "Mod management", description: "Import, install, uninstall, and truly reinstall mods in one place." },
        profiles: { title: "Profiles", description: "Group mod, game, and save data operations into clear play scenarios." },
        backup: { title: "Save protection", description: "Create backup points and restore player save data through controlled flows." },
      },
      callout: "Starting from the current page, follow the highlights and navigate yourself.",
      primaryLabel: "Start the tour",
    },
    builder: {
      pageLocalCallout: "Only this page's key features are introduced next — no automatic navigation or actions.",
      pageLocalPrimary: "View this page's features",
      continueLabel: "Continue",
      finishLabel: "Finish the tour",
      waitClickLabel: "Waiting for your click",
      navigateTitle: (routeTitle: string) => `Go to ${routeTitle}`,
      navigateDescription: (routeTitle: string) =>
        `Click the highlighted "${routeTitle}" on the left; the tour continues automatically once the page opens.`,
      navCallout: "Only the highlighted target is interactive; the tour never clicks for you.",
    },
  },
  ja: {
    routes: {
      dashboard: {
        title: "ワークベンチ",
        description: "現在のゲーム、ディレクトリ識別、前提環境、初回セットアップの進捗をまとめて表示します。",
        bullets: ["まずゲームディレクトリを識別し、次に前提環境を確認します。", "ガイドは入口の説明のみで、自動スキャンやゲーム起動は行いません。"],
        features: {
          "dashboard-steam-scan": {
            title: "Steam を自動スキャン",
            description: "まず HMM に Steam ライブラリから『モンスターハンターワールド：アイスボーン』のインストールディレクトリを探させます。",
            bullets: ["スキャンは Steam ライブラリ設定の読み取りのみです。", "候補が見つかってもゲームディレクトリとして検証します。"],
          },
          "dashboard-manual-directory": {
            title: "ゲームディレクトリを手動選択",
            description: "自動スキャンで見つからない場合は、MonsterHunterWorld.exe を含むディレクトリを直接選択できます。",
          },
          "dashboard-launch-game": {
            title: "ゲームを起動",
            description: "ゲームディレクトリが検証を通過すると、ワークベンチから現在のゲームを直接起動できます。",
            callout: "ディレクトリ未設定の間、ボタンは無効のままです。",
          },
          "dashboard-prerequisites": {
            title: "前提環境を確認",
            description: "Stracker's Loader と CRCBypass のファイル・設定・既知の署名を読み取り専用で確認します。",
            bullets: ["欠落や設定ミスは前提が必要なインストールを遮断します。", "「再チェック」はゲームディレクトリへ書き込みません。"],
          },
        },
      },
      mods: {
        title: "Mod 管理",
        description: "現在のプロファイルの Mod をインポート・絞り込み・管理します。",
        bullets: ["インストール前にプレビューを生成します。", "ガイドが代わりにインポート・インストール・アンインストールすることはありません。"],
        features: {
          "mods-import": {
            title: "Mod をライブラリへインポート",
            description: "「Mod をインポート」で新しいパッケージを追加します。単一の Mod を選択すると新バージョンのインポートも可能です。",
            bullets: ["インポートは管理されたライブラリ記録の作成のみです。", "サードパーティ管理ツールからの移行もクイック操作エリアから始まります。"],
          },
          "mods-library": {
            title: "ライブラリから Mod を選択",
            description: "ここに現在のクエリ結果が表示されます。項目を選択すると詳細表示や一括ライフサイクル操作ができます。",
            bullets: ["カードと一覧にインストール状態が表示されます。", "一覧が空の場合は、まず絞り込み条件を確認するか Mod をインポートしてください。"],
          },
          "mods-lifecycle": {
            title: "選択してからインストール計画をプレビュー",
            description: "Mod を選択すると、クイック操作エリアで計画プレビュー・インストール・真の再インストール・アンインストールを実行できます。",
            bullets: ["まずプレビューで対象ファイル・競合・遮断理由を確認してください。", "書き込み操作は従来の確認・復旧フローを経由します。"],
          },
        },
      },
      recovery: {
        title: "リカバリーセンター",
        description: "管理対象インストールの健全性と人手対応が必要な復旧事項を集約表示します。",
        bullets: ["まず健全性サマリーを確認し、その後異常項目を処理します。", "ロールバックは従来のプレビュー・確認フローを経由します。"],
        features: {
          "recovery-overview": {
            title: "まず復旧健全性サマリーを確認",
            description: "サマリーは正常・要対応・不明・管理対象ファイル数を区別し、人手介入の要否判断を助けます。",
          },
          "recovery-actions": {
            title: "推奨に従って異常を処理",
            description: "人手対応エリアが推奨アクションを提示します。ロールバックが必要な場合も先に管理されたプレビューを生成します。",
            bullets: ["更新は事実の再読込に使います。", "診断エクスポートは先に確認し、マスキングを維持します。"],
          },
          "recovery-mods": {
            title: "特定の Mod を特定",
            description: "設定完了後、管理対象 Mod 一覧に各項目のファイル・バックアップ・問題数が表示され、必要な場合のみロールバック入口を提供します。",
            callout: "一覧が利用できない場合、ハイライト領域にどの前提条件が不足しているか表示されます。",
          },
        },
      },
      categories: {
        title: "カテゴリとタグ",
        description: "Mod のカテゴリとタグを整理し、大規模な Mod ライブラリを絞り込みやすくします。",
        bullets: ["カテゴリは情報の整理のみを担当します。", "実際のインストール状態は Mod 管理ページで維持されます。"],
        features: {
          "categories-create": {
            title: "カテゴリを新規作成",
            description: "カテゴリは Mod を整理する情報コンテナで、名前・色・並び順を設定でき、インストール済みファイルは変更しません。",
            bullets: ["例：外見・武器・ボイスで分類。", "タグ機能は開放前に利用可能と誤認させません。"],
          },
          "categories-manage": {
            title: "検索・並び替え・一括整理",
            description: "メイン一覧ではカテゴリの検索・並び替えと、選択カテゴリへの一括色設定・削除ができます。",
            bullets: ["一括削除は確認を経由します。", "カテゴリの削除で中の Mod がアンインストールされることはありません。"],
          },
        },
      },
      profiles: {
        title: "セーブバックアップ",
        description: "プレイシナリオごとのプロファイル、セーブデータディレクトリ、バックアップ計画を管理します。",
        bullets: ["同時にアクティブなプロファイルは 1 つだけです。", "削除や切り替えはページ既存の確認フローを使用します。"],
        features: {
          "profiles-list": {
            title: "プロファイルの作成・選択・有効化",
            description: "プロファイルは独立した Mod・セーブデータ管理シナリオです。例：メインストーリー、マルチプレイ、テスト環境。",
            bullets: ["作成後にまず選択し、その後アクティブに設定します。", "以降のインストールとセーブデータ操作は現在のアクティブプロファイルに属します。"],
          },
          "profiles-directories": {
            title: "セーブデータとバックアップのディレクトリを設定",
            description: "ゲームセーブデータは保護すべきソースディレクトリ、バックアップディレクトリは HMM がアーカイブとマニフェストを保存する場所です。",
            bullets: ["セーブデータディレクトリは自動検出も手動選択も可能です。", "両方のディレクトリが検証を通過してから設定を保存します。"],
          },
          "profiles-manual-backup": {
            title: "今すぐ手動バックアップを作成",
            description: "Mod のテストや復元準備の前に、現在のプロファイルの管理されたセーブデータアーカイブポイントをすぐに作成できます。",
            callout: "ガイドは入口の説明のみで、代わりにバックアップを作成することはありません。",
          },
          "profiles-auto-backup": {
            title: "自動バックアップの実行状況を確認",
            description: "最終確認・次回予定・バックグラウンド保護の状態が表示され、手動確認の入口もあります。",
            bullets: ["自動バックアップ計画は現在のプロファイルに属します。", "終了後のシステム保護は設定ページで個別に有効化します。"],
          },
          "profiles-backup-policy": {
            title: "計画と保持ポリシーを設定",
            description: "手動・毎日・毎週の計画と、数・日数・容量の上限をここで設定します。",
            bullets: ["復元前セーフティバックアップは既定で有効です。", "整理操作は保護された復元前バックアップを保持します。"],
          },
          "profiles-history": {
            title: "バックアップ履歴からセーブデータを復元",
            description: "各バックアップ記録に「セーブデータを復元」の入口があり、ソース・状態・時刻・ファイル数が表示されます。",
            bullets: ["復元前に再度確認します。", "既定では独立した復元前セーフティバックアップを先に作成します。"],
          },
        },
      },
      backups: {
        title: "バックアップ整理",
        description: "既存バックアップの絞り込み・整理・復元をまとめて行います。",
        bullets: ["復元前には既定で独立したセーフティバックアップを作成します。", "ガイドがバックアップ作成や復元を実行することはありません。"],
        features: {
          "backups-filters": {
            title: "プロファイル横断でバックアップを絞り込み",
            description: "プロファイル・ソース・状態・メモで絞り込み、手動・自動・復元前保護ポイントを素早く見つけます。",
          },
          "backups-maintenance": {
            title: "クォータを確認してバックアップを整理",
            description: "プロファイル概要に数・容量・要対応項目が表示されます。「今すぐ整理」は保存済みの保持ポリシーに従って実行されます。",
            bullets: ["整理前に確認ダイアログが表示されます。", "復元前保護ポイントは通常バックアップのルールでは削除されません。"],
          },
          "backups-history": {
            title: "メモの編集またはセーブデータの復元",
            description: "バックアップ履歴に復元入口が直接表示され、メモを編集して重要なセーブポイントを後で識別しやすくできます。",
            callout: "復元は副作用のある操作のため、ガイドは実行しません。",
          },
        },
      },
      diagnostics: {
        title: "ログと診断",
        description: "検証・マスキング済みの実行状態を表示し、管理された診断エクスポートを提供します。",
        bullets: ["ページに生のローカルパスは表示されません。", "エクスポートはあなたの明示的な確認が必要です。"],
        features: {
          "diagnostics-actions": {
            title: "診断バンドルの更新またはエクスポート",
            description: "更新は安全なサマリーを再読込します。エクスポートは先に確認を求め、その後バックエンドがマスキング済みサポート資料を生成します。",
          },
          "diagnostics-health": {
            title: "証跡の健全性を確認",
            description: "健全性カードはプラットフォーム・App・Debug・Task・Audit ログとログ容量の可用性をまとめます。",
          },
          "diagnostics-logs": {
            title: "安全なログと監査サマリーを読む",
            description: "ログ領域は表示が許可された内容のみ表示します。安定エラーコードとタスク識別子は問題報告に使えます。",
          },
        },
      },
      settings: {
        title: "設定",
        description: "外観・閉じる動作・前提チェック・セーブデータのバックグラウンド保護などを調整します。",
        bullets: ["ここでは終了後のセーブデータバックグラウンド保護のみ説明します。", "各プロファイル自身のバックアップ計画はセーブバックアップページで維持します。"],
        features: {
          "settings-background-protection": {
            title: "終了後のバックグラウンド保護を管理",
            description: "有効にすると、HMM 終了後もシステムタスクが既存の自動バックアップフローを起動し続けます。",
            bullets: ["スイッチはプロファイル自身のバックアップ計画を変更しません。", "「再チェック」はシステムタスクと worker の状態のみ検証します。"],
          },
        },
      },
      about: {
        title: "HMM について",
        description: "現在のバージョン、リリース入口、プロジェクトのリンク、支援の説明、フィードバック窓口をまとめて表示します。",
        bullets: ["現在のバージョンはインストール産物に由来します。", "外部入口はすべてシステムブラウザで開きます。"],
        features: {
          "about-release": {
            title: "バージョンと更新を確認",
            description: "まず現在の HMM バージョンを確認し、その後 GitHub Releases で新バージョンや更新履歴を確認します。",
          },
          "about-links": {
            title: "プロジェクト支援の入口を見つける",
            description: "作者ページ、オープンソースリポジトリ、支援の説明、Issues がここに集約されています。",
            bullets: ["機能要望と不具合は Issues へ提出してください。", "機密性の高いセキュリティ問題はリポジトリのセキュリティポリシーに従い非公開で報告してください。"],
          },
        },
      },
    },
    welcome: {
      title: "HMM へようこそ",
      description: "『モンスターハンターワールド：アイスボーン』の Mod を安全かつ明快に管理し、追跡・復旧可能な操作記録を保持します。",
      features: {
        shield: { title: "安全なインストール", description: "インストール前に変更をプレビューし、失敗時はロールバックと復旧証跡を保持します。" },
        layers: { title: "Mod 管理", description: "Mod のインポート・インストール・アンインストール・真の再インストールを一元化します。" },
        profiles: { title: "プロファイル", description: "Mod・ゲーム・セーブデータ操作を明確な利用シナリオへまとめます。" },
        backup: { title: "セーブ保護", description: "バックアップポイントを作成し、管理されたフローでプレイヤーのセーブデータを復元します。" },
      },
      callout: "次は現在のページから、ハイライトの案内に従って自分で操作してください。",
      primaryLabel: "ガイドを開始",
    },
    builder: {
      pageLocalCallout: "この後は現在のページの重要機能のみ紹介します。他のページへの自動移動や操作は行いません。",
      pageLocalPrimary: "このページの機能を見る",
      continueLabel: "続行",
      finishLabel: "ガイドを完了",
      waitClickLabel: "クリック待ち",
      navigateTitle: (routeTitle: string) => `${routeTitle}へ移動`,
      navigateDescription: (routeTitle: string) =>
        `左側でハイライトされた「${routeTitle}」をクリックしてください。ページが開くとガイドは自動的に続行します。`,
      navCallout: "操作できるのはハイライトされた対象のみです。ガイドが代わりにクリックすることはありません。",
    },
  },
} satisfies LocaleDictionary<OnboardingTourCopy>;

const routeKeyCheck: RouteKeyCheck = true;
void routeKeyCheck;
