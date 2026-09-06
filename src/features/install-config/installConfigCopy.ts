import type { LocaleDictionary } from "../../shared/i18n";
import type { PackageContentRootKind } from "./packageContentsTypes";

/*
 * 「安装配置」覆盖层的全部用户可见文案（`#354` 切片 D4）。
 *
 * 措辞上有一条硬规矩：`installable` / `rejectedByGame` / `excludedByPlayer` 是**三条互相
 * 独立的事实**，文案不得把它们合并成单一的「会不会装」。拒绝清单当前只在重定向链路上被
 * 强制执行，普通安装链路尚未套用——合并必然在其中一条链路上给出与实际相反的答案。
 * 所以 `rejectedByGame` 的说明句显式点出「重定向安装」这个前提，而不是笼统写「不会安装」。
 */

export type InstallConfigFactCopy = {
  label: string;
  detail: string;
};

export type InstallConfigCopy = {
  page: {
    /* 面板标题是 Mod 名（玩家要确认的是「哪个包」），「安装配置」这层身份由图标与入口菜单承担。 */
    description: string;
    treeAria: string;
    /** 页头摘要：整包统计。 */
    summary: (input: { fileCount: number; installableCount: number }) => string;
    summaryRejected: (count: number) => string;
    summaryExcluded: (count: number) => string;
  };
  contentRoot: {
    heading: string;
    kind: Record<PackageContentRootKind, string>;
    /** `single` 用；`fallback` 与 `ambiguous` 各有自己的说明句。 */
    path: (path: string) => string;
    fallbackDetail: string;
    ambiguousDetail: (candidateCount: number) => string;
  };
  facts: {
    notInstallable: InstallConfigFactCopy;
    rejectedByGame: InstallConfigFactCopy;
    excludedByPlayer: InstallConfigFactCopy;
  };
  tree: {
    directoryAria: (input: { name: string; fileCount: number }) => string;
    fileCount: (count: number) => string;
    expand: string;
    collapse: string;
  };
  actions: {
    save: string;
    saveAndClose: string;
    saving: string;
    discard: string;
    /** 页脚常驻提示：草稿与已保存状态一致时显示。 */
    saved: string;
    unsaved: string;
    saveFailed: string;
    confirmCloseDetail: string;
    keepEditing: string;
    discardAndClose: string;
  };
  states: {
    loading: string;
    empty: string;
    failedTitle: string;
    failedDetail: string;
    retry: string;
  };
};

export const installConfigCopy = {
  zh_cn: {
    page: {
      description: "看清这个包里有什么，再决定装哪些、从哪一层开始装。",
      treeAria: "包内容树",
      summary: ({ fileCount, installableCount }) =>
        `共 ${fileCount} 个文件，其中 ${installableCount} 个在安装范围内`,
      summaryRejected: (count) => `${count} 个在游戏拒绝清单内`,
      summaryExcluded: (count) => `${count} 个已被你勾掉`,
    },
    contentRoot: {
      heading: "内容根",
      kind: {
        single: "已确定",
        fallback: "包的根目录",
        ambiguous: "待指定",
      },
      path: (path) => `从 ${path} 开始算安装路径`,
      fallbackDetail: "包里没有多余的包装目录，直接从根目录开始算安装路径。",
      ambiguousDetail: (candidateCount) =>
        `这个包里有 ${candidateCount} 个可以当作内容根的目录，需要你指定用哪一个。`,
    },
    facts: {
      notInstallable: {
        label: "不在安装范围",
        detail: "这个文件不在内容根之下，或者它的路径不被本游戏接受，因此不会进入安装计划。",
      },
      rejectedByGame: {
        label: "游戏拒绝清单",
        detail: "重定向安装会跳过这个文件。普通安装链路目前尚未套用这份清单。",
      },
      excludedByPlayer: {
        label: "已勾掉",
        detail: "你把这个文件排除了，它不会进入安装计划。",
      },
    },
    tree: {
      directoryAria: ({ name, fileCount }) => `目录 ${name}，含 ${fileCount} 个文件`,
      fileCount: (count) => `${count} 个文件`,
      expand: "展开",
      collapse: "折叠",
    },
    actions: {
      save: "保存选择",
      saveAndClose: "保存并关闭",
      saving: "正在保存…",
      discard: "放弃改动",
      saved: "选择已保存，安装时按此执行。",
      unsaved: "有未保存的改动。",
      saveFailed: "保存失败，选择没有生效。请重试。",
      confirmCloseDetail: "有未保存的改动，关掉就没了。",
      keepEditing: "继续编辑",
      discardAndClose: "放弃并关闭",
    },
    states: {
      loading: "正在读取包内容…",
      empty: "这个包里没有文件。",
      failedTitle: "读不到包内容",
      failedDetail: "包可能已被移动或删除。重新导入之后再试。",
      retry: "重试",
    },
  },
  en: {
    page: {
      description: "See what is inside the package, then decide what to install and where it starts from.",
      treeAria: "Package contents tree",
      summary: ({ fileCount, installableCount }) =>
        `${fileCount} files, ${installableCount} of them within the install scope`,
      summaryRejected: (count) => `${count} on the game reject list`,
      summaryExcluded: (count) => `${count} excluded by you`,
    },
    contentRoot: {
      heading: "Content root",
      kind: {
        single: "Resolved",
        fallback: "Package root",
        ambiguous: "Needs a choice",
      },
      path: (path) => `Install paths are computed from ${path}`,
      fallbackDetail: "The package has no extra wrapper directory, so install paths start at its root.",
      ambiguousDetail: (candidateCount) =>
        `This package has ${candidateCount} directories that could serve as the content root. Pick one.`,
    },
    facts: {
      notInstallable: {
        label: "Outside install scope",
        detail:
          "This file sits outside the content root, or its path is not accepted by this game, so it will not enter the install plan.",
      },
      rejectedByGame: {
        label: "Game reject list",
        detail: "Retarget installs skip this file. The plain install path does not apply this list yet.",
      },
      excludedByPlayer: {
        label: "Excluded",
        detail: "You excluded this file, so it will not enter the install plan.",
      },
    },
    tree: {
      directoryAria: ({ name, fileCount }) => `Directory ${name}, ${fileCount} files`,
      fileCount: (count) => `${count} files`,
      expand: "Expand",
      collapse: "Collapse",
    },
    actions: {
      save: "Save selection",
      saveAndClose: "Save and close",
      saving: "Saving…",
      discard: "Discard changes",
      saved: "Selection saved; installs will follow it.",
      unsaved: "You have unsaved changes.",
      saveFailed: "Saving failed, so the selection did not take effect. Try again.",
      confirmCloseDetail: "You have unsaved changes. Closing discards them.",
      keepEditing: "Keep editing",
      discardAndClose: "Discard and close",
    },
    states: {
      loading: "Reading package contents…",
      empty: "This package contains no files.",
      failedTitle: "Cannot read package contents",
      failedDetail: "The package may have been moved or deleted. Import it again and retry.",
      retry: "Retry",
    },
  },
  ja: {
    page: {
      description: "パッケージの中身を確認してから、何をどこから入れるか決めます。",
      treeAria: "パッケージ内容ツリー",
      summary: ({ fileCount, installableCount }) =>
        `全 ${fileCount} ファイル中 ${installableCount} 件がインストール対象範囲内`,
      summaryRejected: (count) => `${count} 件がゲームの拒否リストに該当`,
      summaryExcluded: (count) => `${count} 件を除外済み`,
    },
    contentRoot: {
      heading: "コンテンツルート",
      kind: {
        single: "確定済み",
        fallback: "パッケージのルート",
        ambiguous: "選択が必要",
      },
      path: (path) => `${path} を起点にインストールパスを算出します`,
      fallbackDetail: "余分なラッパーディレクトリがないため、ルートから直接インストールパスを算出します。",
      ambiguousDetail: (candidateCount) =>
        `このパッケージにはコンテンツルートになり得るディレクトリが ${candidateCount} 件あります。どれを使うか指定してください。`,
    },
    facts: {
      notInstallable: {
        label: "対象範囲外",
        detail:
          "コンテンツルートの外にあるか、このゲームが受け付けないパスのため、インストール計画に入りません。",
      },
      rejectedByGame: {
        label: "ゲームの拒否リスト",
        detail: "リターゲットインストールではスキップされます。通常のインストール経路にはこのリストがまだ適用されていません。",
      },
      excludedByPlayer: {
        label: "除外済み",
        detail: "除外したファイルのため、インストール計画に入りません。",
      },
    },
    tree: {
      directoryAria: ({ name, fileCount }) => `ディレクトリ ${name}、${fileCount} ファイル`,
      fileCount: (count) => `${count} ファイル`,
      expand: "展開",
      collapse: "折りたたむ",
    },
    actions: {
      save: "選択を保存",
      saveAndClose: "保存して閉じる",
      saving: "保存中…",
      discard: "変更を破棄",
      saved: "選択を保存しました。インストールはこの内容に従います。",
      unsaved: "未保存の変更があります。",
      saveFailed: "保存に失敗したため、選択は反映されていません。再試行してください。",
      confirmCloseDetail: "未保存の変更があります。閉じると失われます。",
      keepEditing: "編集を続ける",
      discardAndClose: "破棄して閉じる",
    },
    states: {
      loading: "パッケージ内容を読み込み中…",
      empty: "このパッケージにファイルはありません。",
      failedTitle: "パッケージ内容を読み取れません",
      failedDetail: "パッケージが移動または削除された可能性があります。再インポートしてからお試しください。",
      retry: "再試行",
    },
  },
} satisfies LocaleDictionary<InstallConfigCopy>;
