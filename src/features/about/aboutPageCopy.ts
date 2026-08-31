import type { LocaleDictionary } from "../../shared/i18n";

// AboutPage 自有文案（I18N-01 试点）。链接 href 常量不在此处——只有用户可见文本进字典。

export type AboutLinkCopy = { title: string; description: string };

export type AboutPageCopy = {
  hero: {
    eyebrow: string;
    tagline: string;
    versionLabel: string;
    versionAria: (version: string) => string;
    previewChannel: string;
    stableChannel: string;
  };
  release: {
    title: string;
    description: string;
    checkUpdates: string;
    changelog: string;
    checking: string;
    upToDate: string;
    updateAvailable: (version: string) => string;
    autoCheck: string;
    autoCheckHint: string;
    download: string;
    staleNote: string;
  };
  linkLabels: {
    releases: string;
    changelog: string;
    author: string;
    repository: string;
    sponsor: string;
    issues: string;
  };
  groups: {
    project: { title: string; description: string };
    community: { title: string; description: string };
  };
  links: {
    author: AboutLinkCopy;
    repository: AboutLinkCopy;
    sponsor: AboutLinkCopy;
    issues: AboutLinkCopy;
  };
  feedback: {
    idle: string;
    openedTab: (label: string) => string;
    opening: (label: string) => string;
    openedBrowser: (label: string) => string;
    failed: (label: string) => string;
  };
};

export const aboutPageCopy = {
  zh_cn: {
    hero: {
      eyebrow: "关于 HMM",
      tagline: "面向《怪物猎人》系列 PC 版的开源 Mod 管理器。",
      versionLabel: "当前版本",
      versionAria: (version) => `当前版本 ${version}`,
      previewChannel: "预览通道",
      stableChannel: "稳定通道",
    },
    release: {
      title: "版本与更新",
      description:
        "当前尚未启用应用内自动更新：HMM 只会告诉你有没有新版本，下载仍需前往 GitHub Releases。",
      checkUpdates: "检查更新",
      changelog: "更新记录",
      checking: "正在检查更新…",
      upToDate: "已是最新版本",
      updateAvailable: (version) => `${version} 可用`,
      autoCheck: "自动检查更新",
      autoCheckHint: "打开本页时检查，24 小时内不重复查询。",
      download: "前往下载页",
      staleNote: "上次检查失败，以下为上次的结果。",
    },
    linkLabels: {
      releases: "GitHub Releases",
      changelog: "更新记录",
      author: "作者 GitHub 主页",
      repository: "HMM 开源仓库",
      sponsor: "赞助与支持",
      issues: "GitHub Issues",
    },
    groups: {
      project: { title: "项目与作者", description: "源码、作者和项目开发信息。" },
      community: { title: "支持与反馈", description: "赞助说明、功能建议和缺陷反馈。" },
    },
    links: {
      author: {
        title: "作者 GitHub 主页",
        description: "查看 TheLostRiver 的公开项目与个人主页。",
      },
      repository: {
        title: "HMM 开源仓库",
        description: "查看源码、提交记录、许可证信息和开发进度。",
      },
      sponsor: {
        title: "赞助支持",
        description: "查看赞助方式、用途说明和其他支持项目的方法。",
      },
      issues: {
        title: "意见反馈",
        description: "通过 GitHub Issues 提交缺陷、建议和可复现信息。",
      },
    },
    feedback: {
      idle: "外部链接将在系统默认浏览器中打开。",
      openedTab: (label) => `已在新标签页打开${label}。`,
      opening: (label) => `正在打开${label}…`,
      openedBrowser: (label) => `已在系统浏览器打开${label}。`,
      failed: (label) => `${label}未能打开，请稍后重试。`,
    },
  },
  en: {
    hero: {
      eyebrow: "About HMM",
      tagline: "An open-source mod manager for the Monster Hunter series on PC.",
      versionLabel: "Current version",
      versionAria: (version) => `Current version ${version}`,
      previewChannel: "Preview channel",
      stableChannel: "Stable channel",
    },
    release: {
      title: "Version & updates",
      description:
        "In-app auto-update is not enabled yet: HMM only tells you whether a newer version exists - downloads still go through GitHub Releases.",
      checkUpdates: "Check for updates",
      changelog: "Changelog",
      checking: "Checking for updates…",
      upToDate: "You are on the latest version",
      updateAvailable: (version) => `${version} is available`,
      autoCheck: "Check for updates automatically",
      autoCheckHint: "Checks when this page opens, at most once every 24 hours.",
      download: "Go to downloads",
      staleNote: "The last check failed - showing the previous result.",
    },
    linkLabels: {
      releases: "GitHub Releases",
      changelog: "the changelog",
      author: "the author's GitHub profile",
      repository: "the HMM repository",
      sponsor: "the sponsorship page",
      issues: "GitHub Issues",
    },
    groups: {
      project: {
        title: "Project & author",
        description: "Source code, author, and project development info.",
      },
      community: {
        title: "Support & feedback",
        description: "Sponsorship details, feature suggestions, and bug reports.",
      },
    },
    links: {
      author: {
        title: "Author's GitHub profile",
        description: "Browse TheLostRiver's public projects and profile.",
      },
      repository: {
        title: "HMM repository",
        description: "Browse the source, commit history, license, and progress.",
      },
      sponsor: {
        title: "Sponsor the project",
        description: "See sponsorship options, how funds are used, and other ways to help.",
      },
      issues: {
        title: "Feedback",
        description: "File bugs, suggestions, and reproducible reports via GitHub Issues.",
      },
    },
    feedback: {
      idle: "External links open in your default browser.",
      openedTab: (label) => `Opened ${label} in a new tab.`,
      opening: (label) => `Opening ${label}…`,
      openedBrowser: (label) => `Opened ${label} in your browser.`,
      failed: (label) => `Couldn't open ${label}. Please try again later.`,
    },
  },
  ja: {
    hero: {
      eyebrow: "HMM について",
      tagline: "『モンスターハンター』シリーズ PC 版向けのオープンソース Mod マネージャー。",
      versionLabel: "現在のバージョン",
      versionAria: (version) => `現在のバージョン ${version}`,
      previewChannel: "プレビュー版",
      stableChannel: "安定版",
    },
    release: {
      title: "バージョンと更新",
      description:
        "アプリ内自動更新は未対応です。HMM は新しいバージョンがあるかどうかのみを通知し、ダウンロードは GitHub Releases から行います。",
      checkUpdates: "更新を確認",
      changelog: "更新履歴",
      checking: "更新を確認しています…",
      upToDate: "最新バージョンです",
      updateAvailable: (version) => `${version} が利用可能です`,
      autoCheck: "更新を自動的に確認する",
      autoCheckHint: "このページを開いたときに確認します。24 時間以内は再確認しません。",
      download: "ダウンロードページへ",
      staleNote: "前回の確認に失敗したため、前回の結果を表示しています。",
    },
    linkLabels: {
      releases: "GitHub Releases",
      changelog: "更新履歴",
      author: "作者の GitHub",
      repository: "HMM リポジトリ",
      sponsor: "スポンサーページ",
      issues: "GitHub Issues",
    },
    groups: {
      project: {
        title: "プロジェクトと作者",
        description: "ソースコード、作者、開発情報。",
      },
      community: {
        title: "サポートとフィードバック",
        description: "支援方法、機能提案、不具合報告。",
      },
    },
    links: {
      author: {
        title: "作者の GitHub",
        description: "TheLostRiver の公開プロジェクトとプロフィールを見る。",
      },
      repository: {
        title: "HMM リポジトリ",
        description: "ソース、コミット履歴、ライセンス、開発進捗を見る。",
      },
      sponsor: {
        title: "プロジェクトを支援",
        description: "支援方法、使途の説明、その他の支援手段を見る。",
      },
      issues: {
        title: "フィードバック",
        description: "GitHub Issues から不具合・提案・再現情報を報告。",
      },
    },
    feedback: {
      idle: "外部リンクは既定のブラウザで開きます。",
      openedTab: (label) => `新しいタブで${label}を開きました。`,
      opening: (label) => `${label}を開いています…`,
      openedBrowser: (label) => `ブラウザで${label}を開きました。`,
      failed: (label) => `${label}を開けませんでした。しばらくしてから再試行してください。`,
    },
  },
} satisfies LocaleDictionary<AboutPageCopy>;
