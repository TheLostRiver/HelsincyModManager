import { getVersion } from "@tauri-apps/api/app";
import { isTauri } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";
import {
  ExternalLink,
  Code2,
  GitFork,
  HeartHandshake,
  History,
  Info,
  MessageSquareText,
  RefreshCw,
  type LucideIcon,
} from "lucide-react";
import { useEffect, useState, type MouseEvent } from "react";
import packageMetadata from "../../../package.json";
import { AppBrandMark } from "../../app/branding/AppBrandMark";
import "./AboutPage.css";

const ABOUT_LINKS = {
  releases: {
    href: "https://github.com/TheLostRiver/HelsincyModManager/releases",
    label: "GitHub Releases",
  },
  changelog: {
    href: "https://github.com/TheLostRiver/HelsincyModManager/blob/main/CHANGELOG.md",
    label: "更新记录",
  },
  author: {
    href: "https://github.com/TheLostRiver",
    label: "作者 GitHub 主页",
  },
  repository: {
    href: "https://github.com/TheLostRiver/HelsincyModManager",
    label: "HMM 开源仓库",
  },
  sponsor: {
    href: "https://github.com/TheLostRiver/HelsincyModManager/blob/main/docs/SPONSOR.md",
    label: "赞助与支持",
  },
  issues: {
    href: "https://github.com/TheLostRiver/HelsincyModManager/issues",
    label: "GitHub Issues",
  },
} as const;

type AboutLinkId = keyof typeof ABOUT_LINKS;

type LinkFeedback = {
  tone: "neutral" | "success" | "danger";
  message: string;
};

const projectLinks: AboutLinkRowProps[] = [
  {
    id: "author",
    icon: Code2,
    title: "作者 GitHub 主页",
    description: "查看 TheLostRiver 的公开项目与个人主页。",
  },
  {
    id: "repository",
    icon: GitFork,
    title: "HMM 开源仓库",
    description: "查看源码、提交记录、许可证信息和开发进度。",
  },
];

const communityLinks: AboutLinkRowProps[] = [
  {
    id: "sponsor",
    icon: HeartHandshake,
    title: "赞助支持",
    description: "查看赞助方式、用途说明和其他支持项目的方法。",
  },
  {
    id: "issues",
    icon: MessageSquareText,
    title: "意见反馈",
    description: "通过 GitHub Issues 提交缺陷、建议和可复现信息。",
  },
];

export function AboutPage() {
  const appVersion = useInstalledAppVersion();
  const [linkFeedback, setLinkFeedback] = useState<LinkFeedback>({
    tone: "neutral",
    message: "外部链接将在系统默认浏览器中打开。",
  });
  const releaseChannel = appVersion.includes("-") ? "预览通道" : "稳定通道";

  const openAboutLink = async (
    event: MouseEvent<HTMLAnchorElement>,
    linkId: AboutLinkId,
  ) => {
    const link = ABOUT_LINKS[linkId];

    if (!isTauri()) {
      setLinkFeedback({
        tone: "success",
        message: `已在新标签页打开${link.label}。`,
      });
      return;
    }

    event.preventDefault();
    setLinkFeedback({ tone: "neutral", message: `正在打开${link.label}…` });

    try {
      await openUrl(link.href);
      setLinkFeedback({
        tone: "success",
        message: `已在系统浏览器打开${link.label}。`,
      });
    } catch {
      setLinkFeedback({
        tone: "danger",
        message: `${link.label}未能打开，请稍后重试。`,
      });
    }
  };

  return (
    <section className="about-page" aria-labelledby="about-title">
      <header className="about-page__hero">
        <AppBrandMark className="about-page__brand-mark" />
        <div className="about-page__hero-copy">
          <span>关于 HMM</span>
          <h2 id="about-title">Helsincy Mod Manager</h2>
          <p>面向《怪物猎人》系列 PC 版的开源 Mod 管理器。</p>
        </div>
        <div className="about-page__version" aria-label={`当前版本 ${appVersion}`}>
          <span>当前版本</span>
          <strong>v{appVersion}</strong>
          <small>{releaseChannel}</small>
        </div>
      </header>

      <section className="about-page__release" data-tour-id="about.release">
        <div className="about-page__release-icon" aria-hidden="true">
          <RefreshCw size={20} strokeWidth={2.1} />
        </div>
        <div className="about-page__release-copy">
          <h3>版本与更新</h3>
          <p>
            当前尚未启用应用内自动更新。检查更新会打开 GitHub Releases，供你对照版本并下载发布包。
          </p>
        </div>
        <div className="about-page__release-actions">
          <AboutActionLink
            id="releases"
            icon={RefreshCw}
            label="检查更新"
            primary
            onOpen={openAboutLink}
          />
          <AboutActionLink
            id="changelog"
            icon={History}
            label="更新记录"
            onOpen={openAboutLink}
          />
        </div>
      </section>

      <div className="about-page__link-columns" data-tour-id="about.links">
        <AboutLinkGroup title="项目与作者" description="源码、作者和项目开发信息。" links={projectLinks} onOpen={openAboutLink} />
        <AboutLinkGroup title="支持与反馈" description="赞助说明、功能建议和缺陷反馈。" links={communityLinks} onOpen={openAboutLink} />
      </div>

      <div
        className={`about-page__feedback is-${linkFeedback.tone}`}
        role={linkFeedback.tone === "danger" ? "alert" : "status"}
        aria-live="polite"
      >
        <Info size={16} aria-hidden="true" />
        <span>{linkFeedback.message}</span>
      </div>
    </section>
  );
}

type AboutLinkRowProps = {
  id: AboutLinkId;
  icon: LucideIcon;
  title: string;
  description: string;
};

type AboutLinkHandler = (
  event: MouseEvent<HTMLAnchorElement>,
  linkId: AboutLinkId,
) => Promise<void>;

function AboutLinkGroup({
  title,
  description,
  links,
  onOpen,
}: {
  title: string;
  description: string;
  links: AboutLinkRowProps[];
  onOpen: AboutLinkHandler;
}) {
  return (
    <section className="about-page__link-group">
      <header>
        <h3>{title}</h3>
        <p>{description}</p>
      </header>
      <div className="about-page__link-list">
        {links.map((link) => {
          const Icon = link.icon;
          return (
            <a
              key={link.id}
              className="about-page__link-row"
              href={ABOUT_LINKS[link.id].href}
              target="_blank"
              rel="noreferrer"
              onClick={(event) => void onOpen(event, link.id)}
            >
              <span className="about-page__link-icon" aria-hidden="true">
                <Icon size={18} strokeWidth={2.1} />
              </span>
              <span className="about-page__link-copy">
                <strong>{link.title}</strong>
                <span>{link.description}</span>
              </span>
              <ExternalLink size={16} aria-hidden="true" />
            </a>
          );
        })}
      </div>
    </section>
  );
}

function AboutActionLink({
  id,
  icon: Icon,
  label,
  primary = false,
  onOpen,
}: {
  id: AboutLinkId;
  icon: LucideIcon;
  label: string;
  primary?: boolean;
  onOpen: AboutLinkHandler;
}) {
  return (
    <a
      className={`about-page__action ${primary ? "is-primary" : ""}`}
      href={ABOUT_LINKS[id].href}
      target="_blank"
      rel="noreferrer"
      onClick={(event) => void onOpen(event, id)}
    >
      <Icon size={16} aria-hidden="true" />
      <span>{label}</span>
      <ExternalLink size={14} aria-hidden="true" />
    </a>
  );
}

function useInstalledAppVersion() {
  const [version, setVersion] = useState(packageMetadata.version);

  useEffect(() => {
    if (!isTauri()) {
      return;
    }

    let active = true;
    void getVersion()
      .then((installedVersion) => {
        if (active && installedVersion.trim()) {
          setVersion(installedVersion);
        }
      })
      .catch(() => {
        // Keep the build-time package version when runtime metadata is unavailable.
      });

    return () => {
      active = false;
    };
  }, []);

  return version;
}
