import {
  Archive,
  ArchiveRestore,
  Check,
  ChevronLeft,
  ChevronRight,
  CircleAlert,
  Edit3,
  Eraser,
  Filter,
  Loader2,
  RefreshCw,
  Save,
  Search,
  ShieldCheck,
  X,
} from "lucide-react";
import { useCallback, useEffect, useRef, useState, type ReactNode } from "react";
import { Dialog, useFeedback } from "../../shared/feedback";
import { localeMeta, resolveCopy, useI18n, type Locale } from "../../shared/i18n";
import { SaveRestoreDialog } from "../profiles/SaveRestoreDialog";
import type { SaveBackupSummaryDto } from "../profiles/profileSaveBackupTypes";
import {
  querySaveBackupCenter,
  runSaveBackupRetention,
  updateSaveBackupNote,
} from "./backupCenterApi";
import { backupCenterCopy, type BackupCenterCopy } from "./backupCenterCopy";
import { createPreviewPage } from "./backupsPreviewData";
import type {
  BackupCenterStatus,
  BackupCenterTrigger,
  BackupMaintenanceState,
  SaveBackupCenterPageDto,
  SaveBackupCenterProfileSummaryDto,
  SaveBackupRetentionReportDto,
} from "./backupCenterTypes";

const CURRENT_GAME_ID = "mhw";
const PAGE_LIMIT = 12;
const TRUSTED_AVATAR_HOSTS = new Set([
  "avatars.akamai.steamstatic.com",
  "avatars.steamstatic.com",
]);

type QueryState = {
  profileId: string | null;
  trigger: BackupCenterTrigger | null;
  status: BackupCenterStatus | null;
  search: string;
  offset: number;
};

type PageState =
  | { status: "loading"; page: SaveBackupCenterPageDto | null }
  | { status: "ready"; page: SaveBackupCenterPageDto }
  | { status: "error"; page: SaveBackupCenterPageDto | null; message: string };

function isPlainBrowserRuntime() {
  return typeof window !== "undefined" && !("__TAURI_INTERNALS__" in window);
}

function formatBytes(bytes: number) {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KiB`;
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MiB`;
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(2)} GiB`;
}

function formatDate(timestamp: number, locale: Locale) {
  return new Intl.DateTimeFormat(localeMeta[locale].bcp47, {
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  }).format(new Date(timestamp));
}

function formatElapsed(milliseconds: number) {
  return `${(milliseconds / 1000).toFixed(1)}s`;
}

function lastValidPageOffset(totalCount: number) {
  return totalCount <= 0 ? 0 : Math.floor((totalCount - 1) / PAGE_LIMIT) * PAGE_LIMIT;
}

function trustedSteamAvatarUrl(value: string | null) {
  if (!value) return null;
  try {
    const url = new URL(value);
    if (
      url.protocol !== "https:" ||
      url.port ||
      url.username ||
      url.password ||
      !TRUSTED_AVATAR_HOSTS.has(url.hostname.toLowerCase())
    ) {
      return null;
    }
    return value;
  } catch {
    return null;
  }
}

function triggerLabel(trigger: BackupCenterTrigger, copy: BackupCenterCopy) {
  return copy.triggers[trigger];
}

function statusLabel(status: BackupCenterStatus, copy: BackupCenterCopy) {
  return copy.statuses[status];
}

function statusTone(status: BackupCenterStatus) {
  if (status === "completed") return "success";
  if (status === "retention_partial" || status === "retention_pending") return "warning";
  if (status === "missing" || status === "invalid") return "danger";
  return "neutral";
}

function errorMessage(error: unknown, fallback: string) {
  if (typeof error === "object" && error && "message" in error && typeof error.message === "string") {
    return error.message;
  }
  return fallback;
}

function reportTone(report: SaveBackupRetentionReportDto) {
  if (report.outcome === "failed") return "danger" as const;
  if (report.evidenceDegraded) return "warning" as const;
  return report.outcome === "partial" || report.outcome === "blocked" ? "warning" : "success";
}

function reportLabel(report: SaveBackupRetentionReportDto, copy: BackupCenterCopy) {
  const label = copy.report.outcomes[report.outcome];
  return report.evidenceDegraded ? `${label}${copy.report.evidenceDegradedSuffix}` : label;
}

export function BackupCenterPage() {
  const { pushToast } = useFeedback();
  const { locale } = useI18n();
  const copy = resolveCopy(backupCenterCopy, locale);
  // 数据加载 effect 经 ref 取词：copy 一旦进入 loadPage 依赖链，切换语言就会重拉后端。
  const copyRef = useRef(copy);
  copyRef.current = copy;
  const previewMode = isPlainBrowserRuntime();
  const [query, setQuery] = useState<QueryState>({
    profileId: null,
    trigger: null,
    status: null,
    search: "",
    offset: 0,
  });
  const [pageState, setPageState] = useState<PageState>({ status: "loading", page: null });
  const [refreshToken, setRefreshToken] = useState(0);
  const [restoreBackup, setRestoreBackup] = useState<SaveBackupSummaryDto | null>(null);
  const [editingNoteId, setEditingNoteId] = useState<string | null>(null);
  const [noteDraft, setNoteDraft] = useState("");
  const [savingNoteId, setSavingNoteId] = useState<string | null>(null);
  const [maintenance, setMaintenance] = useState<Record<string, BackupMaintenanceState>>({});
  const [pendingMaintenanceProfile, setPendingMaintenanceProfile] = useState<SaveBackupCenterProfileSummaryDto | null>(null);
  const cancelMaintenanceRef = useRef<HTMLButtonElement>(null);

  const loadPage = useCallback(() => {
    let cancelled = false;
    setPageState((current) => ({ status: "loading", page: current.page ?? null }));
    const request = previewMode
      ? Promise.resolve(createPreviewPage(query))
      : querySaveBackupCenter({
        gameId: CURRENT_GAME_ID,
        profileId: query.profileId,
        trigger: query.trigger,
        status: query.status,
        search: query.search || null,
        offset: query.offset,
        limit: PAGE_LIMIT,
      });
    void request
      .then((page) => {
        if (!cancelled) setPageState({ status: "ready", page });
      })
      .catch((error: unknown) => {
        if (!cancelled) setPageState((current) => ({ status: "error", page: current.page, message: errorMessage(error, copyRef.current.errors.unavailableFallback) }));
      });
    return () => {
      cancelled = true;
    };
  }, [previewMode, query]);

  useEffect(() => loadPage(), [loadPage, refreshToken]);

  useEffect(() => {
    if (pageState.status !== "ready") return;
    const normalizedOffset = lastValidPageOffset(pageState.page.totalCount);
    if (pageState.page.offset > normalizedOffset) {
      setQuery((current) => current.offset === normalizedOffset
        ? current
        : { ...current, offset: normalizedOffset });
    }
  }, [pageState]);

  useEffect(() => {
    const timer = window.setInterval(() => {
      const now = Date.now();
      setMaintenance((current) => {
        let changed = false;
        const next = { ...current };
        for (const [profileId, state] of Object.entries(current)) {
          if (state.status === "running") {
            next[profileId] = { ...state, elapsedMs: now - state.startedAt };
            changed = true;
          }
        }
        return changed ? next : current;
      });
    }, 100);
    return () => window.clearInterval(timer);
  }, []);

  function setFilter<K extends keyof QueryState>(key: K, value: QueryState[K]) {
    setQuery((current) => ({ ...current, [key]: value, ...(key !== "offset" ? { offset: 0 } : {}) }));
  }

  async function runMaintenance(profile: SaveBackupCenterProfileSummaryDto) {
    const startedAt = Date.now();
    setMaintenance((current) => ({ ...current, [profile.profileId]: { status: "running", elapsedMs: 0, startedAt } }));
    try {
      const report = previewMode
        ? await Promise.resolve<SaveBackupRetentionReportDto>({
          outcome: "completed",
          evidenceDegraded: false,
          scannedCount: profile.backupCount,
          protectedCount: profile.protectedCount,
          problemCount: profile.attentionCount,
          candidateCount:
            profile.retention.maxCount === 0
              ? 0
              : Math.max(0, profile.backupCount - profile.retention.maxCount),
          deletedCount: 0,
          partialCount: 0,
          blockedCount: 0,
          archiveBytesBefore: profile.archiveBytes,
          archiveBytesAfter: profile.archiveBytes,
          releasedBytes: 0,
          maxTotalBytes: profile.retention.maxTotalBytes,
          budgetSatisfied: profile.budgetSatisfied,
        })
        : await runSaveBackupRetention({ gameId: CURRENT_GAME_ID, profileId: profile.profileId });
      const elapsedMs = Date.now() - startedAt;
      setMaintenance((current) => ({ ...current, [profile.profileId]: { status: "completed", elapsedMs, report } }));
      setRefreshToken((value) => value + 1);
      pushToast({
        eventKey: `backup-center.retention.${profile.profileId}.${report.outcome}`,
        tone: reportTone(report),
        title: reportLabel(report, copy),
        message: `${copy.toasts.maintenanceMessage({ scannedCount: report.scannedCount, deletedCount: report.deletedCount, elapsed: formatElapsed(elapsedMs) })}${report.evidenceDegraded ? copy.toasts.maintenanceEvidenceSuffix : ""}`,
      });
    } catch (error) {
      const elapsedMs = Date.now() - startedAt;
      setMaintenance((current) => ({ ...current, [profile.profileId]: { status: "error", elapsedMs, message: errorMessage(error, copy.errors.unavailableFallback) } }));
      pushToast({
        eventKey: `backup-center.retention.error.${profile.profileId}`,
        tone: "danger",
        title: copy.toasts.maintenanceFailedTitle,
        message: copy.toasts.maintenanceFailedMessage(errorMessage(error, copy.errors.unavailableFallback), formatElapsed(elapsedMs)),
      });
    }
  }

  async function saveNote(backup: SaveBackupSummaryDto) {
    const note = noteDraft.trim() || null;
    setSavingNoteId(backup.backupId);
    try {
      if (previewMode) {
        setPageState((current) => current.status !== "ready" ? current : {
          status: "ready",
          page: {
            ...current.page,
            items: current.page.items.map((item) => item.backup.backupId === backup.backupId
              ? { ...item, backup: { ...item.backup, notes: note } }
              : item),
          },
        });
      } else {
        await updateSaveBackupNote({ gameId: CURRENT_GAME_ID, profileId: backup.profileId, backupId: backup.backupId, note });
        setRefreshToken((value) => value + 1);
      }
      setEditingNoteId(null);
      pushToast({
        eventKey: `backup-center.note.${backup.backupId}`,
        tone: "success",
        title: copy.toasts.noteSavedTitle,
        message: note ? copy.toasts.noteSavedMessage : copy.toasts.noteClearedMessage,
      });
    } catch (error) {
      pushToast({ eventKey: `backup-center.note.error.${backup.backupId}`, tone: "danger", title: copy.toasts.noteFailedTitle, message: errorMessage(error, copy.errors.unavailableFallback) });
    } finally {
      setSavingNoteId(null);
    }
  }

  const page = pageState.status === "ready" ? pageState.page : pageState.page;
  const totalPages = page ? Math.max(1, Math.ceil(page.totalCount / PAGE_LIMIT)) : 1;
  const currentPage = page ? Math.floor(page.offset / PAGE_LIMIT) + 1 : 1;
  const loading = pageState.status === "loading" && !page;

  return (
    <main className="backup-center-page">
      <header className="backup-center-header">
        <div>
          <span className="backup-center-eyebrow"><Archive size={14} /> BACKUP CENTER</span>
          <h1>{copy.page.title}</h1>
          <p>{copy.page.subtitle}</p>
        </div>
        <button className="backup-icon-button" type="button" title={copy.page.reloadAria} aria-label={copy.page.reloadAria} aria-busy={pageState.status === "loading"} onClick={() => setRefreshToken((value) => value + 1)}>
          <RefreshCw size={17} className={pageState.status === "loading" ? "backup-spin" : undefined} />
        </button>
      </header>

      {page ? (
        <section className="backup-center-overview" aria-label={copy.page.overviewAria}>
          <SummaryMetric label={copy.page.metricBackups} value={String(page.summary.backupCount)} icon={<Archive size={15} />} />
          <SummaryMetric label={copy.page.metricSpace} value={formatBytes(page.summary.archiveBytes)} icon={<Save size={15} />} />
          <SummaryMetric label={copy.page.metricProtected} value={String(page.summary.protectedCount)} icon={<ShieldCheck size={15} />} />
          <SummaryMetric label={copy.page.metricAttention} value={String(page.summary.attentionCount)} icon={<CircleAlert size={15} />} tone={page.summary.attentionCount ? "warning" : "normal"} />
        </section>
      ) : null}

      <section className="backup-center-filters" aria-label={copy.page.filtersAria} data-tour-id="backups.filters">
        <div className="backup-filter-label"><Filter size={16} /> {copy.page.filterLabel}</div>
        <label>
          <span>{copy.page.filterProfile}</span>
          <select value={query.profileId ?? ""} onChange={(event) => setFilter("profileId", event.target.value || null)}>
            <option value="">{copy.page.filterAllProfiles}</option>
            {(page?.profiles ?? []).map((profile) => <option key={profile.profileId} value={profile.profileId}>{profile.profileName}</option>)}
          </select>
        </label>
        <label>
          <span>{copy.page.filterTrigger}</span>
          <select value={query.trigger ?? ""} onChange={(event) => setFilter("trigger", (event.target.value || null) as BackupCenterTrigger | null)}>
            <option value="">{copy.page.filterAllTriggers}</option>
            <option value="manual">{copy.triggers.manual}</option>
            <option value="auto">{copy.triggers.auto}</option>
            <option value="pre_install">{copy.triggers.pre_install}</option>
            <option value="pre_restore">{copy.triggers.pre_restore}</option>
          </select>
        </label>
        <label>
          <span>{copy.page.filterStatus}</span>
          <select value={query.status ?? ""} onChange={(event) => setFilter("status", (event.target.value || null) as BackupCenterStatus | null)}>
            <option value="">{copy.page.filterAllStatuses}</option>
            <option value="completed">{copy.statuses.completed}</option>
            <option value="retention_partial">{copy.statuses.retention_partial}</option>
            <option value="retention_pending">{copy.statuses.retention_pending}</option>
            <option value="missing">{copy.statuses.missing}</option>
            <option value="invalid">{copy.statuses.invalid}</option>
            <option value="deleted_by_retention">{copy.statuses.deleted_by_retention}</option>
          </select>
        </label>
        <label className="backup-search-control">
          <span>{copy.page.filterSearch}</span>
          <span className="backup-search-input"><Search size={16} /><input value={query.search} maxLength={100} onChange={(event) => setFilter("search", event.target.value)} placeholder={copy.page.filterSearchPlaceholder} /></span>
        </label>
      </section>

      {page ? (
        <section className="backup-center-workspace">
          <aside className="backup-center-profiles" aria-label={copy.page.profilesAria} data-tour-id="backups.profiles">
            <div className="backup-section-heading"><div><span className="backup-section-kicker">PROFILES</span><h2>{copy.page.profilesTitle}</h2></div><span>{copy.page.profilesCount(page.profiles.length)}</span></div>
            <div className="backup-profile-list">
              {page.profiles.map((profile) => (
                <ProfileSummaryCard
                  key={profile.profileId}
                  copy={copy}
                  profile={profile}
                  selected={query.profileId === profile.profileId}
                  maintenance={maintenance[profile.profileId] ?? { status: "idle" }}
                  onSelect={() => setFilter("profileId", query.profileId === profile.profileId ? null : profile.profileId)}
                  onMaintain={() => setPendingMaintenanceProfile(profile)}
                />
              ))}
            </div>
          </aside>

          <section className="backup-center-history" aria-label={copy.page.historyAria} data-tour-id="backups.history">
            <div className="backup-section-heading"><div><span className="backup-section-kicker">HISTORY</span><h2>{copy.page.historyTitle}</h2></div><span>{copy.page.historyCount(page.totalCount)}</span></div>
            {pageState.status === "error" ? <div className="backup-center-alert is-danger"><CircleAlert size={18} /> {pageState.message}</div> : null}
            {loading ? <LoadingRows /> : page.items.length === 0 ? <EmptyHistory copy={copy} /> : (
              <div className="backup-history-list">
                {page.items.map(({ backup, profileName }) => (
                  <BackupHistoryRow
                    key={backup.backupId}
                    copy={copy}
                    locale={locale}
                    backup={backup}
                    profileName={profileName}
                    editing={editingNoteId === backup.backupId}
                    noteDraft={editingNoteId === backup.backupId ? noteDraft : backup.notes ?? ""}
                    saving={savingNoteId === backup.backupId}
                    onEdit={() => { setEditingNoteId(backup.backupId); setNoteDraft(backup.notes ?? ""); }}
                    onCancel={() => setEditingNoteId(null)}
                    onNoteChange={setNoteDraft}
                    onSave={() => void saveNote(backup)}
                    onRestore={() => setRestoreBackup(backup)}
                  />
                ))}
              </div>
            )}
            <div className="backup-pagination">
              <span>{copy.page.pagination(currentPage, totalPages)}</span>
              <div>
                <button className="backup-icon-button" type="button" title={copy.page.prevPage} aria-label={copy.page.prevPage} disabled={currentPage <= 1} onClick={() => setFilter("offset", Math.max(0, query.offset - PAGE_LIMIT))}><ChevronLeft size={17} /></button>
                <button className="backup-icon-button" type="button" title={copy.page.nextPage} aria-label={copy.page.nextPage} disabled={currentPage >= totalPages} onClick={() => setFilter("offset", query.offset + PAGE_LIMIT)}><ChevronRight size={17} /></button>
              </div>
            </div>
          </section>
        </section>
      ) : pageState.status === "error" ? (
        <div className="backup-center-loading-page is-error" role="alert">
          <CircleAlert size={24} />
          <strong>{copy.page.unavailableTitle}</strong>
          <span>{pageState.message}</span>
          <button className="backup-action-button is-primary" type="button" onClick={() => setRefreshToken((value) => value + 1)}><RefreshCw size={15} /> {copy.page.retry}</button>
        </div>
      ) : <div className="backup-center-loading-page" aria-busy="true"><Loader2 className="backup-spin" size={24} /> {copy.page.loadingPage}</div>}

      <Dialog
        open={pendingMaintenanceProfile !== null}
        role="alertdialog"
        title={copy.maintenanceDialog.title}
        description={copy.maintenanceDialog.description}
        initialFocusRef={cancelMaintenanceRef}
        onClose={() => setPendingMaintenanceProfile(null)}
        footer={
          <>
            <button
              ref={cancelMaintenanceRef}
              type="button"
              className="backup-action-button"
              onClick={() => setPendingMaintenanceProfile(null)}
            >
              {copy.maintenanceDialog.cancel}
            </button>
            <button
              type="button"
              className="backup-action-button is-primary"
              onClick={() => {
                const profile = pendingMaintenanceProfile;
                setPendingMaintenanceProfile(null);
                if (profile) void runMaintenance(profile);
              }}
            >
              <Eraser size={15} />
              {copy.maintenanceDialog.confirm}
            </button>
          </>
        }
      />

      {restoreBackup ? (
        <SaveRestoreDialog
          backup={restoreBackup}
          profileId={restoreBackup.profileId}
          previewMode={previewMode}
          onClose={() => setRestoreBackup(null)}
          onCompleted={() => { setRestoreBackup(null); setRefreshToken((value) => value + 1); }}
        />
      ) : null}
    </main>
  );
}

function SummaryMetric({ label, value, icon, tone = "normal" }: { label: string; value: string; icon: ReactNode; tone?: "normal" | "warning" }) {
  return <div className={`backup-summary-metric is-${tone}`}><span className="backup-summary-metric__icon">{icon}</span><span><small>{label}</small><strong>{value}</strong></span></div>;
}

function ProfileSummaryCard({ copy, profile, selected, maintenance, onSelect, onMaintain }: { copy: BackupCenterCopy; profile: SaveBackupCenterProfileSummaryDto; selected: boolean; maintenance: BackupMaintenanceState; onSelect: () => void; onMaintain: () => void }) {
  const running = maintenance.status === "running";
  return (
    <article className={`backup-profile-card${selected ? " is-selected" : ""}`}>
      <button className="backup-profile-card__select" type="button" onClick={onSelect} aria-pressed={selected}>
        <BackupProfileAvatar profile={profile} />
        <span className="backup-profile-card__identity"><strong>{profile.profileName}</strong><small>{profile.isActive ? copy.profileCard.activeIdentity : profile.steamAccount?.accountLabel ?? copy.profileCard.unboundIdentity}</small></span>
        {profile.isActive ? <span className="backup-active-dot" title={copy.profileCard.activeDotAria} aria-label={copy.profileCard.activeDotAria} /> : null}
      </button>
      <div className="backup-profile-card__facts"><span><small>{copy.profileCard.factRecords}</small><strong>{profile.backupCount}</strong></span><span><small>{copy.profileCard.factSpace}</small><strong>{formatBytes(profile.archiveBytes)}</strong></span><span><small>{copy.profileCard.factPolicy}</small><strong className={profile.budgetSatisfied ? "is-good" : "is-warning"}>{profile.budgetSatisfied ? copy.profileCard.policyOk : copy.profileCard.policyOverBudget}</strong></span></div>
      <div className="backup-profile-card__footer">
        {maintenance.status === "completed" ? <span className={`backup-maintenance-result is-${reportTone(maintenance.report)}`}><Check size={14} /> {reportLabel(maintenance.report, copy)} · {formatElapsed(maintenance.elapsedMs)}</span> : null}
        {maintenance.status === "error" ? <span className="backup-maintenance-result is-danger" title={maintenance.message}><CircleAlert size={14} /> {copy.profileCard.maintainFailed} · {formatElapsed(maintenance.elapsedMs)}</span> : null}
        {running ? <span className="backup-maintenance-running"><Loader2 size={14} className="backup-spin" /> {formatElapsed(maintenance.elapsedMs)}</span> : null}
        <button className="backup-action-button" type="button" disabled={running} onClick={onMaintain}><Eraser size={15} /> {running ? copy.profileCard.maintaining : copy.profileCard.maintainNow}</button>
      </div>
    </article>
  );
}

function BackupProfileAvatar({ profile }: { profile: SaveBackupCenterProfileSummaryDto }) {
  const rawAvatarUrl = profile.steamAccount?.avatarUrl ?? null;
  const avatarUrl = trustedSteamAvatarUrl(rawAvatarUrl);
  const [loadFailed, setLoadFailed] = useState(false);
  useEffect(() => setLoadFailed(false), [avatarUrl]);
  const fallback = profile.steamAccount?.accountName?.slice(0, 1) ?? profile.profileName.slice(0, 1);

  return (
    <span className="backup-profile-avatar" aria-hidden="true">
      {avatarUrl && !loadFailed ? (
        <img
          src={avatarUrl}
          alt=""
          loading="lazy"
          decoding="async"
          referrerPolicy="no-referrer"
          onError={() => setLoadFailed(true)}
        />
      ) : fallback}
    </span>
  );
}

function BackupHistoryRow({ copy, locale, backup, profileName, editing, noteDraft, saving, onEdit, onCancel, onNoteChange, onSave, onRestore }: { copy: BackupCenterCopy; locale: Locale; backup: SaveBackupSummaryDto; profileName: string; editing: boolean; noteDraft: string; saving: boolean; onEdit: () => void; onCancel: () => void; onNoteChange: (value: string) => void; onSave: () => void; onRestore: () => void }) {
  const tone = statusTone(backup.status);
  return (
    <article className={`backup-history-row is-${tone}`}>
      <div className="backup-history-row__main">
        <div className="backup-history-row__title"><strong>{backup.notes?.trim() || backup.fileName}</strong><span className={`backup-status-badge is-${tone}`}>{statusLabel(backup.status, copy)}</span></div>
        <div className="backup-history-row__meta"><span>{profileName}</span><span>{triggerLabel(backup.trigger, copy)}</span><span>{formatDate(backup.createdAt, locale)}</span><span>{copy.historyRow.fileCount(backup.fileCount)}</span><span>{formatBytes(backup.sizeBytes)}</span></div>
        {editing ? <div className="backup-note-editor"><input value={noteDraft} maxLength={200} aria-label={copy.historyRow.noteAria} onChange={(event) => onNoteChange(event.target.value)} autoFocus /><button className="backup-icon-button is-primary" type="button" title={copy.historyRow.saveNoteAria} aria-label={copy.historyRow.saveNoteAria} disabled={saving} onClick={onSave}>{saving ? <Loader2 size={15} className="backup-spin" /> : <Check size={15} />}</button><button className="backup-icon-button" type="button" title={copy.historyRow.cancelEditAria} aria-label={copy.historyRow.cancelEditAria} disabled={saving} onClick={onCancel}><X size={15} /></button></div> : null}
      </div>
      <div className="backup-history-row__actions">
        {!editing ? <button className="backup-icon-button" type="button" title={copy.historyRow.editNoteAria} aria-label={copy.historyRow.editNoteAria} onClick={onEdit}><Edit3 size={16} /></button> : null}
        {backup.status === "completed" ? <button className="backup-action-button is-restore" type="button" onClick={onRestore}><ArchiveRestore size={15} /> {copy.historyRow.restore}</button> : <span className="backup-action-disabled" title={copy.historyRow.notRestorableHint}>{copy.historyRow.notRestorable}</span>}
      </div>
    </article>
  );
}

function LoadingRows() {
  return <div className="backup-loading-list" aria-busy="true">{[1, 2, 3].map((item) => <div className="backup-loading-row" key={item}><span /><span /><span /></div>)}</div>;
}

function EmptyHistory({ copy }: { copy: BackupCenterCopy }) {
  return <div className="backup-empty-state"><Archive size={28} /><strong>{copy.page.emptyTitle}</strong><span>{copy.page.emptyHint}</span></div>;
}
