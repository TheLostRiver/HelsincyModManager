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
import { resolveCopy, useI18n } from "../../shared/i18n";
import { aboutPageCopy, type AboutPageCopy } from "./aboutPageCopy";
import { projectUpdateCheckView } from "./updateCheckView";
import { useUpdateCheck } from "./useUpdateCheck";
import "./AboutPage.css";

// 只保留 href 常量；所有用户可见文本都在 aboutPageCopy 里（I18N-01 试点）。
const ABOUT_LINK_HREFS = {
  releases: "https://github.com/TheLostRiver/HelsincyModManager/releases",
  changelog: "https://github.com/TheLostRiver/HelsincyModManager/blob/main/CHANGELOG.md",
  author: "https://github.com/TheLostRiver",
  repository: "https://github.com/TheLostRiver/HelsincyModManager",
  sponsor: "https://github.com/TheLostRiver/HelsincyModManager/blob/main/docs/SPONSOR.md",
  issues: "https://github.com/TheLostRiver/HelsincyModManager/issues",
} as const;

type AboutLinkId = keyof typeof ABOUT_LINK_HREFS;

// 反馈只存"发生了什么 + 对哪个链接"，渲染时再取当前语言文案：
// 否则切换界面语言后，上一条反馈会滞留在旧语言。
type LinkFeedback =
  | { kind: "idle" }
  | { kind: "openedTab" | "opening" | "openedBrowser" | "failed"; linkId: AboutLinkId };

const projectLinkRows: { id: AboutLinkId; icon: LucideIcon }[] = [
  { id: "author", icon: Code2 },
  { id: "repository", icon: GitFork },
];

const communityLinkRows: { id: AboutLinkId; icon: LucideIcon }[] = [
  { id: "sponsor", icon: HeartHandshake },
  { id: "issues", icon: MessageSquareText },
];

export function AboutPage() {
  const { locale } = useI18n();
  const copy = resolveCopy(aboutPageCopy, locale);
  const appVersion = useInstalledAppVersion();
  const updateCheck = useUpdateCheck();
  const updateView = projectUpdateCheckView({
    checking: updateCheck.checking,
    status: updateCheck.status,
  });
  const [linkFeedback, setLinkFeedback] = useState<LinkFeedback>({ kind: "idle" });
  const releaseChannel = appVersion.includes("-")
    ? copy.hero.previewChannel
    : copy.hero.stableChannel;

  const feedbackTone =
    linkFeedback.kind === "failed"
      ? "danger"
      : linkFeedback.kind === "idle" || linkFeedback.kind === "opening"
        ? "neutral"
        : "success";
  const feedbackMessage =
    linkFeedback.kind === "idle"
      ? copy.feedback.idle
      : copy.feedback[linkFeedback.kind](copy.linkLabels[linkFeedback.linkId]);

  const openAboutLink = async (
    event: MouseEvent<HTMLAnchorElement>,
    linkId: AboutLinkId,
  ) => {
    if (!isTauri()) {
      setLinkFeedback({ kind: "openedTab", linkId });
      return;
    }

    event.preventDefault();
    setLinkFeedback({ kind: "opening", linkId });

    try {
      await openUrl(ABOUT_LINK_HREFS[linkId]);
      setLinkFeedback({ kind: "openedBrowser", linkId });
    } catch {
      setLinkFeedback({ kind: "failed", linkId });
    }
  };

  return (
    <section className="about-page" aria-labelledby="about-title">
      <header className="about-page__hero">
        <AppBrandMark className="about-page__brand-mark" />
        <div className="about-page__hero-copy">
          <span>{copy.hero.eyebrow}</span>
          <h2 id="about-title">Helsincy Mod Manager</h2>
          <p>{copy.hero.tagline}</p>
        </div>
        <div className="about-page__version" aria-label={copy.hero.versionAria(appVersion)}>
          <span>{copy.hero.versionLabel}</span>
          <strong>v{appVersion}</strong>
          <small>{releaseChannel}</small>
        </div>
      </header>

      <section className="about-page__release" data-tour-id="about.release">
        <div className="about-page__release-icon" aria-hidden="true">
          <RefreshCw size={20} strokeWidth={2.1} />
        </div>
        <div className="about-page__release-copy">
          <h3>{copy.release.title}</h3>
          <p>{copy.release.description}</p>
          {updateView.kind === "checking" ? (
            <p className="about-page__update-status is-checking" role="status">
              {copy.release.checking}
            </p>
          ) : null}
          {updateView.kind === "up_to_date" ? (
            <p className="about-page__update-status is-current" role="status">
              {copy.release.upToDate}
            </p>
          ) : null}
          {updateView.kind === "update_available" ? (
            <p className="about-page__update-status is-available" role="status">
              {copy.release.updateAvailable(updateView.version)}
            </p>
          ) : null}
          {/* `unknown` 刻意什么都不渲染：断网 / 超时 / 接口失败都是常态，静默处理。 */}
        </div>
        <div className="about-page__release-actions">
          {updateView.kind === "update_available" ? (
            <AboutActionLink
              id="releases"
              icon={ExternalLink}
              label={copy.release.download}
              primary
              onOpen={openAboutLink}
            />
          ) : (
            <AboutActionButton
              icon={RefreshCw}
              label={copy.release.checkUpdates}
              busy={updateCheck.checking}
              onTrigger={updateCheck.refresh}
            />
          )}
          <AboutActionLink
            id="changelog"
            icon={History}
            label={copy.release.changelog}
            onOpen={openAboutLink}
          />
        </div>
        <label className="about-page__auto-check">
          <input
            type="checkbox"
            checked={updateCheck.autoCheckEnabled}
            onChange={(event) => updateCheck.setAutoCheckEnabled(event.target.checked)}
          />
          <span>{copy.release.autoCheck}</span>
          <small>{copy.release.autoCheckHint}</small>
        </label>
      </section>

      <div className="about-page__link-columns" data-tour-id="about.links">
        <AboutLinkGroup
          title={copy.groups.project.title}
          description={copy.groups.project.description}
          rows={projectLinkRows}
          copy={copy}
          onOpen={openAboutLink}
        />
        <AboutLinkGroup
          title={copy.groups.community.title}
          description={copy.groups.community.description}
          rows={communityLinkRows}
          copy={copy}
          onOpen={openAboutLink}
        />
      </div>

      <div
        className={`about-page__feedback is-${feedbackTone}`}
        role={feedbackTone === "danger" ? "alert" : "status"}
        aria-live="polite"
      >
        <Info size={16} aria-hidden="true" />
        <span>{feedbackMessage}</span>
      </div>
    </section>
  );
}

type AboutLinkHandler = (
  event: MouseEvent<HTMLAnchorElement>,
  linkId: AboutLinkId,
) => Promise<void>;

function AboutLinkGroup({
  title,
  description,
  rows,
  copy,
  onOpen,
}: {
  title: string;
  description: string;
  rows: { id: AboutLinkId; icon: LucideIcon }[];
  copy: AboutPageCopy;
  onOpen: AboutLinkHandler;
}) {
  return (
    <section className="about-page__link-group">
      <header>
        <h3>{title}</h3>
        <p>{description}</p>
      </header>
      <div className="about-page__link-list">
        {rows.map((row) => {
          const Icon = row.icon;
          const linkCopy = copy.links[row.id as keyof AboutPageCopy["links"]];
          return (
            <a
              key={row.id}
              className="about-page__link-row"
              href={ABOUT_LINK_HREFS[row.id]}
              target="_blank"
              rel="noreferrer"
              onClick={(event) => void onOpen(event, row.id)}
            >
              <span className="about-page__link-icon" aria-hidden="true">
                <Icon size={18} strokeWidth={2.1} />
              </span>
              <span className="about-page__link-copy">
                <strong>{linkCopy.title}</strong>
                <span>{linkCopy.description}</span>
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
      href={ABOUT_LINK_HREFS[id]}
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

function AboutActionButton({
  icon: Icon,
  label,
  busy,
  onTrigger,
}: {
  icon: LucideIcon;
  label: string;
  busy: boolean;
  onTrigger: () => void;
}) {
  return (
    <button
      type="button"
      className="about-page__action is-primary"
      onClick={onTrigger}
      disabled={busy}
    >
      <Icon size={16} aria-hidden="true" />
      <span>{label}</span>
    </button>
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
