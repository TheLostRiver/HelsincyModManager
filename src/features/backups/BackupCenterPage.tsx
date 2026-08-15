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
import { SaveRestoreDialog } from "../profiles/SaveRestoreDialog";
import type { SaveBackupSummaryDto } from "../profiles/profileSaveBackupTypes";
import {
  querySaveBackupCenter,
  runSaveBackupRetention,
  updateSaveBackupNote,
} from "./backupCenterApi";
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

function formatDate(timestamp: number) {
  return new Intl.DateTimeFormat("zh-CN", {
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

function triggerLabel(trigger: BackupCenterTrigger) {
  return {
    manual: "手动",
    auto: "自动",
    pre_install: "安装前",
    pre_restore: "恢复前保护",
  }[trigger];
}

function statusLabel(status: BackupCenterStatus) {
  return {
    completed: "可恢复",
    retention_pending: "整理中断",
    retention_partial: "清理未完成",
    deleted_by_retention: "已整理",
    missing: "文件缺失",
    invalid: "记录异常",
  }[status];
}

function statusTone(status: BackupCenterStatus) {
  if (status === "completed") return "success";
  if (status === "retention_partial" || status === "retention_pending") return "warning";
  if (status === "missing" || status === "invalid") return "danger";
  return "neutral";
}

function errorMessage(error: unknown) {
  if (typeof error === "object" && error && "message" in error && typeof error.message === "string") {
    return error.message;
  }
  return "备份中心暂时无法读取，请稍后重试。";
}

function createPreviewPage(query: QueryState): SaveBackupCenterPageDto {
  const backups: SaveBackupSummaryDto[] = [
    {
      backupId: "mhw:default:20260815-120000:manual",
      gameId: CURRENT_GAME_ID,
      profileId: "default",
      trigger: "manual",
      status: "completed",
      fileName: "20260815-120000_mhw_profile-default_manual.zip",
      createdAt: Date.now() - 45 * 60_000,
      sizeBytes: 18_482_944,
      fileCount: 2,
      sourcePathLabel: "synthetic save",
      notes: "Fatalis 配装前",
    },
    {
      backupId: "mhw:default:20260814-090000:pre_restore",
      gameId: CURRENT_GAME_ID,
      profileId: "default",
      trigger: "pre_restore",
      status: "completed",
      fileName: "20260814-090000_mhw_profile-default_pre_restore.zip",
      createdAt: Date.now() - 26 * 60 * 60_000,
      sizeBytes: 17_965_120,
      fileCount: 2,
      sourcePathLabel: "synthetic save",
      notes: "恢复前保护点",
    },
    {
      backupId: "mhw:taichi:20260813-230000:auto",
      gameId: CURRENT_GAME_ID,
      profileId: "taichi",
      trigger: "auto",
      status: "retention_partial",
      fileName: "20260813-230000_mhw_profile-taichi_auto.zip",
      createdAt: Date.now() - 42 * 60 * 60_000,
      sizeBytes: 20_125_696,
      fileCount: 2,
      sourcePathLabel: "synthetic save",
      notes: "等待下次整理重试",
    },
  ];
  const profiles: SaveBackupCenterProfileSummaryDto[] = [
    {
      profileId: "default",
      profileName: "Default 配置档",
      isActive: true,
      steamAccount: {
        accountName: "Synthetic Hunter",
        avatarUrl: null,
        accountLabel: "Steam 12****34",
      },
      retention: { maxCount: 20, maxAgeDays: 30, maxTotalBytes: null },
      backupCount: 2,
      archiveBytes: 36_448_064,
      protectedCount: 1,
      attentionCount: 0,
      budgetSatisfied: true,
    },
    {
      profileId: "taichi",
      profileName: "太刀毕业档",
      isActive: false,
      steamAccount: {
        accountName: null,
        avatarUrl: null,
        accountLabel: "Steam 56****78",
      },
      retention: { maxCount: 12, maxAgeDays: 14, maxTotalBytes: 64 * 1024 * 1024 },
      backupCount: 1,
      archiveBytes: 20_125_696,
      protectedCount: 0,
      attentionCount: 1,
      budgetSatisfied: true,
    },
  ];
  const filtered = backups.filter((backup) => {
    if (query.profileId && backup.profileId !== query.profileId) return false;
    if (query.trigger && backup.trigger !== query.trigger) return false;
    if (query.status && backup.status !== query.status) return false;
    if (query.search) {
      const profile = profiles.find((item) => item.profileId === backup.profileId);
      const haystack = `${profile?.profileName ?? ""} ${backup.notes ?? ""}`.toLowerCase();
      if (!haystack.includes(query.search.toLowerCase())) return false;
    }
    return true;
  });
  return {
    offset: query.offset,
    limit: PAGE_LIMIT,
    totalCount: filtered.length,
    summary: {
      backupCount: filtered.length,
      archiveBytes: filtered.reduce((sum, backup) => sum + backup.sizeBytes, 0),
      protectedCount: filtered.filter((backup) => backup.trigger === "pre_restore").length,
      attentionCount: filtered.filter((backup) => backup.status === "retention_partial").length,
    },
    profiles,
    items: filtered.slice(query.offset, query.offset + PAGE_LIMIT).map((backup) => ({
      profileName: profiles.find((profile) => profile.profileId === backup.profileId)?.profileName ?? backup.profileId,
      backup,
    })),
  };
}

function reportTone(report: SaveBackupRetentionReportDto) {
  if (report.outcome === "failed") return "danger" as const;
  if (report.evidenceDegraded) return "warning" as const;
  return report.outcome === "partial" || report.outcome === "blocked" ? "warning" : "success";
}

function reportLabel(report: SaveBackupRetentionReportDto) {
  const label = report.outcome === "failed"
    ? "整理失败，未删除备份"
    : report.outcome === "blocked"
      ? "整理被保护点阻断"
      : report.outcome === "partial"
        ? "整理部分完成，下次会继续重试"
        : report.outcome === "within_policy"
          ? "已符合保留策略"
          : "整理完成";
  return report.evidenceDegraded ? `${label}，但审计记录不可用` : label;
}

export function BackupCenterPage() {
  const { pushToast } = useFeedback();
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
        if (!cancelled) setPageState((current) => ({ status: "error", page: current.page, message: errorMessage(error) }));
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
          candidateCount: Math.max(0, profile.backupCount - profile.retention.maxCount),
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
        title: reportLabel(report),
        message: `已扫描 ${report.scannedCount} 条，删除 ${report.deletedCount} 条，耗时 ${formatElapsed(elapsedMs)}。${report.evidenceDegraded ? " 清理结果已生效，但本次审计证据不完整。" : ""}`,
      });
    } catch (error) {
      const elapsedMs = Date.now() - startedAt;
      setMaintenance((current) => ({ ...current, [profile.profileId]: { status: "error", elapsedMs, message: errorMessage(error) } }));
      pushToast({
        eventKey: `backup-center.retention.error.${profile.profileId}`,
        tone: "danger",
        title: "整理失败",
        message: `${errorMessage(error)} 已耗时 ${formatElapsed(elapsedMs)}。`,
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
        title: "备注已保存",
        message: note ? "备份记录已更新。" : "备份备注已清空。",
      });
    } catch (error) {
      pushToast({ eventKey: `backup-center.note.error.${backup.backupId}`, tone: "danger", title: "备注保存失败", message: errorMessage(error) });
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
          <h1>存档备份</h1>
          <p>跨配置档查看备份历史、保护点与整理状态。</p>
        </div>
        <button className="backup-icon-button" type="button" title="重新加载" aria-label="重新加载" aria-busy={pageState.status === "loading"} onClick={() => setRefreshToken((value) => value + 1)}>
          <RefreshCw size={17} className={pageState.status === "loading" ? "backup-spin" : undefined} />
        </button>
      </header>

      {page?.summary.attentionCount ? (
        <div className="backup-center-alert is-warning" role="status">
          <CircleAlert size={18} />
          <span>{page.summary.attentionCount} 条备份需要整理或检查。</span>
        </div>
      ) : null}

      {page ? (
        <section className="backup-center-summary" aria-label="备份摘要">
          <SummaryMetric label="备份记录" value={String(page.summary.backupCount)} icon={<Archive size={17} />} />
          <SummaryMetric label="已知空间" value={formatBytes(page.summary.archiveBytes)} icon={<Save size={17} />} />
          <SummaryMetric label="保护点" value={String(page.summary.protectedCount)} icon={<ShieldCheck size={17} />} />
          <SummaryMetric label="需处理" value={String(page.summary.attentionCount)} icon={<CircleAlert size={17} />} tone={page.summary.attentionCount ? "warning" : "normal"} />
        </section>
      ) : null}

      <section className="backup-center-filters" aria-label="筛选备份">
        <div className="backup-filter-label"><Filter size={16} /> 筛选</div>
        <label>
          <span>配置档</span>
          <select value={query.profileId ?? ""} onChange={(event) => setFilter("profileId", event.target.value || null)}>
            <option value="">全部配置档</option>
            {(page?.profiles ?? []).map((profile) => <option key={profile.profileId} value={profile.profileId}>{profile.profileName}</option>)}
          </select>
        </label>
        <label>
          <span>来源</span>
          <select value={query.trigger ?? ""} onChange={(event) => setFilter("trigger", (event.target.value || null) as BackupCenterTrigger | null)}>
            <option value="">全部来源</option>
            <option value="manual">手动</option>
            <option value="auto">自动</option>
            <option value="pre_install">安装前</option>
            <option value="pre_restore">恢复前保护</option>
          </select>
        </label>
        <label>
          <span>状态</span>
          <select value={query.status ?? ""} onChange={(event) => setFilter("status", (event.target.value || null) as BackupCenterStatus | null)}>
            <option value="">全部状态</option>
            <option value="completed">可恢复</option>
            <option value="retention_partial">清理未完成</option>
            <option value="retention_pending">整理中断</option>
            <option value="missing">文件缺失</option>
            <option value="invalid">记录异常</option>
            <option value="deleted_by_retention">已整理</option>
          </select>
        </label>
        <label className="backup-search-control">
          <span>搜索备注或配置档</span>
          <span className="backup-search-input"><Search size={16} /><input value={query.search} maxLength={100} onChange={(event) => setFilter("search", event.target.value)} placeholder="输入关键词" /></span>
        </label>
      </section>

      {page ? (
        <section className="backup-center-workspace">
          <aside className="backup-center-profiles" aria-label="配置档摘要">
            <div className="backup-section-heading"><div><span className="backup-section-kicker">PROFILES</span><h2>配置档摘要</h2></div><span>{page.profiles.length} 个</span></div>
            <div className="backup-profile-list">
              {page.profiles.map((profile) => (
                <ProfileSummaryCard
                  key={profile.profileId}
                  profile={profile}
                  selected={query.profileId === profile.profileId}
                  maintenance={maintenance[profile.profileId] ?? { status: "idle" }}
                  onSelect={() => setFilter("profileId", query.profileId === profile.profileId ? null : profile.profileId)}
                  onMaintain={() => setPendingMaintenanceProfile(profile)}
                />
              ))}
            </div>
          </aside>

          <section className="backup-center-history" aria-label="备份历史">
            <div className="backup-section-heading"><div><span className="backup-section-kicker">HISTORY</span><h2>备份历史</h2></div><span>{page.totalCount} 条</span></div>
            {pageState.status === "error" ? <div className="backup-center-alert is-danger"><CircleAlert size={18} /> {pageState.message}</div> : null}
            {loading ? <LoadingRows /> : page.items.length === 0 ? <EmptyHistory /> : (
              <div className="backup-history-list">
                {page.items.map(({ backup, profileName }) => (
                  <BackupHistoryRow
                    key={backup.backupId}
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
              <span>第 {currentPage} / {totalPages} 页</span>
              <div>
                <button className="backup-icon-button" type="button" title="上一页" aria-label="上一页" disabled={currentPage <= 1} onClick={() => setFilter("offset", Math.max(0, query.offset - PAGE_LIMIT))}><ChevronLeft size={17} /></button>
                <button className="backup-icon-button" type="button" title="下一页" aria-label="下一页" disabled={currentPage >= totalPages} onClick={() => setFilter("offset", query.offset + PAGE_LIMIT)}><ChevronRight size={17} /></button>
              </div>
            </div>
          </section>
        </section>
      ) : pageState.status === "error" ? (
        <div className="backup-center-loading-page is-error" role="alert">
          <CircleAlert size={24} />
          <strong>备份中心暂时不可用</strong>
          <span>{pageState.message}</span>
          <button className="backup-action-button is-primary" type="button" onClick={() => setRefreshToken((value) => value + 1)}><RefreshCw size={15} /> 重试</button>
        </div>
      ) : <div className="backup-center-loading-page" aria-busy="true"><Loader2 className="backup-spin" size={24} /> 正在读取备份中心</div>}

      <Dialog
        open={pendingMaintenanceProfile !== null}
        role="alertdialog"
        title="确认立即整理备份"
        description="将按该配置档已保存的保留策略整理普通备份。最新普通备份和恢复前保护点不会被删除，符合数量、年龄或空间规则的普通备份可能被永久删除。此操作不可撤销。"
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
              取消
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
              确认整理
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

function ProfileSummaryCard({ profile, selected, maintenance, onSelect, onMaintain }: { profile: SaveBackupCenterProfileSummaryDto; selected: boolean; maintenance: BackupMaintenanceState; onSelect: () => void; onMaintain: () => void }) {
  const running = maintenance.status === "running";
  return (
    <article className={`backup-profile-card${selected ? " is-selected" : ""}`}>
      <button className="backup-profile-card__select" type="button" onClick={onSelect} aria-pressed={selected}>
        <BackupProfileAvatar profile={profile} />
        <span className="backup-profile-card__identity"><strong>{profile.profileName}</strong><small>{profile.isActive ? "当前活动配置档" : profile.steamAccount?.accountLabel ?? "未绑定账号摘要"}</small></span>
        {profile.isActive ? <span className="backup-active-dot" title="活动配置档" aria-label="活动配置档" /> : null}
      </button>
      <div className="backup-profile-card__facts"><span><small>记录</small><strong>{profile.backupCount}</strong></span><span><small>空间</small><strong>{formatBytes(profile.archiveBytes)}</strong></span><span><small>策略</small><strong className={profile.budgetSatisfied ? "is-good" : "is-warning"}>{profile.budgetSatisfied ? "正常" : "超预算"}</strong></span></div>
      <div className="backup-profile-card__footer">
        {maintenance.status === "completed" ? <span className={`backup-maintenance-result is-${reportTone(maintenance.report)}`}><Check size={14} /> {reportLabel(maintenance.report)} · {formatElapsed(maintenance.elapsedMs)}</span> : null}
        {maintenance.status === "error" ? <span className="backup-maintenance-result is-danger" title={maintenance.message}><CircleAlert size={14} /> 整理失败 · {formatElapsed(maintenance.elapsedMs)}</span> : null}
        {running ? <span className="backup-maintenance-running"><Loader2 size={14} className="backup-spin" /> {formatElapsed(maintenance.elapsedMs)}</span> : null}
        <button className="backup-action-button" type="button" disabled={running} onClick={onMaintain}><Eraser size={15} /> {running ? "整理中" : "立即整理"}</button>
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

function BackupHistoryRow({ backup, profileName, editing, noteDraft, saving, onEdit, onCancel, onNoteChange, onSave, onRestore }: { backup: SaveBackupSummaryDto; profileName: string; editing: boolean; noteDraft: string; saving: boolean; onEdit: () => void; onCancel: () => void; onNoteChange: (value: string) => void; onSave: () => void; onRestore: () => void }) {
  const tone = statusTone(backup.status);
  return (
    <article className={`backup-history-row is-${tone}`}>
      <div className="backup-history-row__main">
        <div className="backup-history-row__title"><strong>{backup.notes?.trim() || backup.fileName}</strong><span className={`backup-status-badge is-${tone}`}>{statusLabel(backup.status)}</span></div>
        <div className="backup-history-row__meta"><span>{profileName}</span><span>{triggerLabel(backup.trigger)}</span><span>{formatDate(backup.createdAt)}</span><span>{backup.fileCount} 个文件</span><span>{formatBytes(backup.sizeBytes)}</span></div>
        {editing ? <div className="backup-note-editor"><input value={noteDraft} maxLength={200} aria-label="备份备注" onChange={(event) => onNoteChange(event.target.value)} autoFocus /><button className="backup-icon-button is-primary" type="button" title="保存备注" aria-label="保存备注" disabled={saving} onClick={onSave}>{saving ? <Loader2 size={15} className="backup-spin" /> : <Check size={15} />}</button><button className="backup-icon-button" type="button" title="取消编辑" aria-label="取消编辑" disabled={saving} onClick={onCancel}><X size={15} /></button></div> : null}
      </div>
      <div className="backup-history-row__actions">
        {!editing ? <button className="backup-icon-button" type="button" title="编辑备注" aria-label="编辑备注" onClick={onEdit}><Edit3 size={16} /></button> : null}
        {backup.status === "completed" ? <button className="backup-action-button is-restore" type="button" onClick={onRestore}><ArchiveRestore size={15} /> 恢复存档</button> : <span className="backup-action-disabled" title="只有可恢复的备份点才能恢复">不可恢复</span>}
      </div>
    </article>
  );
}

function LoadingRows() {
  return <div className="backup-loading-list" aria-busy="true">{[1, 2, 3].map((item) => <div className="backup-loading-row" key={item}><span /><span /><span /></div>)}</div>;
}

function EmptyHistory() {
  return <div className="backup-empty-state"><Archive size={28} /><strong>暂无符合条件的备份</strong><span>调整筛选条件后再试。</span></div>;
}
