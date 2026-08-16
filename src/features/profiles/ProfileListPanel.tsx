import {
  AlertTriangle,
  Loader2,
  Pencil,
  Plus,
  RefreshCw,
  Save,
  Trash2,
  X,
} from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState, type FormEvent } from "react";
import { createPortal } from "react-dom";
import { createProfile, deleteProfile, updateProfile } from "./profileApi";
import type { Profile } from "./profileTypes";
import { isProfileDeletable } from "./profileViewModel";

type ProfileListPanelProps = {
  profiles: Profile[];
  status: "loading" | "ready" | "error";
  selectedProfileId: string | null;
  busyProfileId: string | null;
  onRefresh: () => void;
  createRequestToken?: number;
  onSelectProfile: (profileId: string) => void;
  onActivateProfile: (profileId: string) => Promise<void>;
  onProfilesChanged: () => void;
};

export function ProfileListPanel({
  profiles,
  status,
  selectedProfileId,
  busyProfileId,
  onRefresh,
  createRequestToken,
  onSelectProfile,
  onActivateProfile,
  onProfilesChanged,
}: ProfileListPanelProps) {
  const [showCreateForm, setShowCreateForm] = useState(false);
  const [editingId, setEditingId] = useState<string | null>(null);
  const [pendingDeleteId, setPendingDeleteId] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const floatingFormRef = useRef<HTMLDivElement>(null);
  const editingProfile = useMemo(
    () => profiles.find((profile) => profile.id === editingId) ?? null,
    [editingId, profiles],
  );
  const floatingFormOpen = showCreateForm || editingProfile !== null;

  const clearTransientState = useCallback(() => {
    setEditingId(null);
    setPendingDeleteId(null);
    setError(null);
  }, []);

  const closeFloatingForm = useCallback(() => {
    setShowCreateForm(false);
    setEditingId(null);
  }, []);

  useEffect(() => {
    if (!createRequestToken) return;
    clearTransientState();
    setShowCreateForm(true);
  }, [clearTransientState, createRequestToken]);

  useEffect(() => {
    if (!floatingFormOpen) return;

    const handlePointerDown = (event: MouseEvent) => {
      if (floatingFormRef.current?.contains(event.target as Node)) return;
      closeFloatingForm();
    };
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        closeFloatingForm();
      }
    };

    document.addEventListener("mousedown", handlePointerDown);
    document.addEventListener("keydown", handleKeyDown);
    return () => {
      document.removeEventListener("mousedown", handlePointerDown);
      document.removeEventListener("keydown", handleKeyDown);
    };
  }, [closeFloatingForm, floatingFormOpen]);

  return (
    <aside
      className="profile-list-panel glass-card slot-panel"
      aria-labelledby="profile-list-title"
      data-tour-id="profiles.list"
    >
      <div className="profile-list-panel__floating-root">
        <div className="profile-list-panel__header panel-header-row">
          <div>
            <h2 id="profile-list-title">配置卡包</h2>
            <span>{status === "ready" ? `${profiles.length} 个本地配置` : "读取配置中"}</span>
          </div>
          <div className="profile-list-panel__tools">
            <button
              type="button"
              className="profile-icon-button"
              aria-label="刷新配置档"
              onClick={onRefresh}
              disabled={status === "loading"}
            >
              <RefreshCw size={15} />
            </button>
          </div>
        </div>

        {/* Create Card Button placed at the very top of the list */}
        {status === "ready" ? (
          <button
            type="button"
            className="profile-list-create-card-btn"
            onClick={() => {
              clearTransientState();
              setShowCreateForm(true);
            }}
          >
            <Plus size={16} />
            新建独立游戏配置档
          </button>
        ) : null}

        {floatingFormOpen
          ? createPortal(
              <>
                <div className="profile-floating-backdrop" aria-hidden="true" />
                <div
                  className="profile-floating-form"
                  ref={floatingFormRef}
                  role="dialog"
                  aria-modal="true"
                  aria-label={showCreateForm ? "新建配置档" : "编辑配置档"}
                >
                  <div className="profile-floating-form__header">
                    <div>
                      <span>配置工作空间</span>
                      <strong>{showCreateForm ? "新建配置档" : "编辑配置档"}</strong>
                    </div>
                    <button
                      type="button"
                      className="profile-icon-button"
                      aria-label="关闭配置档信息"
                      onClick={closeFloatingForm}
                    >
                      <X size={15} />
                    </button>
                  </div>
                  {showCreateForm ? (
                    <ProfileCreateForm
                      onCancel={closeFloatingForm}
                      onCreated={() => {
                        closeFloatingForm();
                        onProfilesChanged();
                      }}
                      onError={setError}
                    />
                  ) : editingProfile ? (
                    <ProfileEditForm
                      profile={editingProfile}
                      onCancel={closeFloatingForm}
                      onSaved={() => {
                        closeFloatingForm();
                        onProfilesChanged();
                      }}
                      onError={setError}
                    />
                  ) : null}
                </div>
              </>,
              document.body,
            )
          : null}
      </div>

      {error ? (
        <p className="profile-settings-alert" role="alert">
          <AlertTriangle size={14} />
          {error}
        </p>
      ) : null}

      {status === "loading" ? (
        <div className="profile-list-state" role="status">
          <Loader2 className="profile-spinner" size={18} />
          <span>正在加载名片盒...</span>
        </div>
      ) : null}

      {status === "error" ? (
        <div className="profile-list-state is-error" role="alert">
          <AlertTriangle size={18} />
          <span>配置数据加载失败</span>
          <button type="button" className="profile-action-button" onClick={onRefresh}>
            重试
          </button>
        </div>
      ) : null}
      {status === "ready" ? (
        <div className="profile-list slot-stack" role="list" aria-label="配置档列表">
          {profiles.map((profile, index) => (
            <ProfileListItem
              key={profile.id}
              profile={profile}
              index={index}
              selected={profile.id === selectedProfileId}
              busy={profile.id === busyProfileId}
              confirmingDelete={pendingDeleteId === profile.id}
              onSelect={() => onSelectProfile(profile.id)}
              onEdit={() => {
                setEditingId(profile.id);
                setPendingDeleteId(null);
                setShowCreateForm(false);
                setError(null);
              }}
              onActivate={() => {
                setError(null);
                void onActivateProfile(profile.id).catch((err) => setError(getProfileErrorMessage(err)));
              }}
              onStartDelete={() => {
                setPendingDeleteId(profile.id);
                setEditingId(null);
                setShowCreateForm(false);
                setError(null);
              }}
              onCancelDelete={() => setPendingDeleteId(null)}
              onDelete={() => {
                setError(null);
                void deleteProfile(profile.id)
                  .then(() => {
                    setPendingDeleteId(null);
                    onProfilesChanged();
                  })
                  .catch((err) => setError(getProfileErrorMessage(err)));
              }}
            />
          ))}
        </div>
      ) : null}
    </aside>
  );
}

function ProfileListItem({
  profile,
  index,
  selected,
  busy,
  confirmingDelete,
  onSelect,
  onEdit,
  onActivate,
  onStartDelete,
  onCancelDelete,
  onDelete,
}: {
  profile: Profile;
  index: number;
  selected: boolean;
  busy: boolean;
  confirmingDelete: boolean;
  onSelect: () => void;
  onEdit: () => void;
  onActivate: () => void;
  onStartDelete: () => void;
  onCancelDelete: () => void;
  onDelete: () => void;
}) {
  const cannotDelete = !isProfileDeletable(profile);
  const itemClassName = [
    "profile-list-item",
    "slot-item-card",
    selected ? "is-selected" : "",
    profile.isActive ? "is-active-profile" : "",
  ].filter(Boolean).join(" ");

  return (
    <article className={itemClassName} role="listitem">
      {profile.isActive ? <span className="profile-active-pulse-badge" aria-hidden="true" /> : null}

      <button
        type="button"
        className="profile-list-item__select slot-select-btn"
        aria-current={selected ? "true" : undefined}
        onClick={onSelect}
      >
        <div className="slot-meta">
          <span className="slot-num">
            SLOT {String(index + 1).padStart(2, "0")}
          </span>
          <span className={`slot-badge ${profile.isActive ? "is-active-badge" : ""}`}>
            {profile.isActive ? "活动中" : "备用档"}
          </span>
        </div>
        <strong className="slot-title">{profile.name}</strong>
        <span className="slot-desc">{profile.description || "暂无备注描述"}</span>
      </button>

      <div className="profile-list-item__actions">
        {!profile.isActive ? (
          <button type="button" className="profile-action-button is-primary" onClick={onActivate} disabled={busy}>
            {busy ? "激活中" : "激活"}
          </button>
        ) : null}
        <button type="button" className="profile-icon-button" aria-label="编辑配置档" onClick={onEdit}>
          <Pencil size={14} />
        </button>
        <button
          type="button"
          className="profile-icon-button is-danger"
          aria-label="删除配置档"
          onClick={onStartDelete}
          disabled={cannotDelete || busy}
          title={profile.isActive ? "当前配置档不能删除" : undefined}
        >
          <Trash2 size={14} />
        </button>
      </div>

      {confirmingDelete && !cannotDelete ? (
        <div className="profile-delete-confirm">
          <span>确认注销并删除该配置卡？</span>
          <div>
            <button type="button" className="profile-action-button is-danger" onClick={onDelete}>
              注销
            </button>
            <button type="button" className="profile-action-button" onClick={onCancelDelete}>
              取消
            </button>
          </div>
        </div>
      ) : null}
    </article>
  );
}

function ProfileCreateForm({
  onCreated,
  onCancel,
  onError,
}: {
  onCreated: () => void;
  onCancel: () => void;
  onError: (message: string | null) => void;
}) {
  const [name, setName] = useState("");
  const [description, setDescription] = useState("");
  const [submitting, setSubmitting] = useState(false);

  const submit = (event: FormEvent) => {
    event.preventDefault();
    if (!name.trim()) return;
    setSubmitting(true);
    onError(null);
    void createProfile({ name: name.trim(), description: description.trim() || null })
      .then(onCreated)
      .catch((err) => onError(getProfileErrorMessage(err)))
      .finally(() => setSubmitting(false));
  };

  return (
    <form className="profile-inline-form" onSubmit={submit}>
      <label className="profile-field">
        <span>配置卡名称</span>
        <input value={name} onChange={(event) => setName(event.target.value)} autoFocus />
      </label>
      <label className="profile-field">
        <span>描述与备注</span>
        <textarea value={description} rows={4} onChange={(event) => setDescription(event.target.value)} />
      </label>
      <div className="profile-form-actions">
        <button type="submit" className="profile-action-button is-primary" disabled={!name.trim() || submitting}>
          <Plus size={14} />
          {submitting ? "正在创建..." : "确认创建"}
        </button>
        <button type="button" className="profile-action-button" onClick={onCancel}>
          取消
        </button>
      </div>
    </form>
  );
}

function ProfileEditForm({
  profile,
  onSaved,
  onCancel,
  onError,
}: {
  profile: Profile;
  onSaved: () => void;
  onCancel: () => void;
  onError: (message: string | null) => void;
}) {
  const [name, setName] = useState(profile.name);
  const [description, setDescription] = useState(profile.description ?? "");
  const [submitting, setSubmitting] = useState(false);

  const submit = (event: FormEvent) => {
    event.preventDefault();
    if (!name.trim()) return;
    setSubmitting(true);
    onError(null);
    void updateProfile({
      profileId: profile.id,
      name: name.trim(),
      description: description.trim() || null,
    })
      .then(onSaved)
      .catch((err) => onError(getProfileErrorMessage(err)))
      .finally(() => setSubmitting(false));
  };

  return (
    <form className="profile-inline-form" onSubmit={submit}>
      <label className="profile-field">
        <span>配置卡名称</span>
        <input value={name} onChange={(event) => setName(event.target.value)} autoFocus />
      </label>
      <label className="profile-field">
        <span>描述与备注</span>
        <textarea value={description} rows={4} onChange={(event) => setDescription(event.target.value)} />
      </label>
      <div className="profile-form-actions">
        <button type="submit" className="profile-action-button is-primary" disabled={!name.trim() || submitting}>
          <Save size={14} />
          {submitting ? "正在同步..." : "保存更改"}
        </button>
        <button type="button" className="profile-action-button" onClick={onCancel}>
          取消
        </button>
      </div>
    </form>
  );
}

function getProfileErrorMessage(error: unknown): string {
  if (typeof error === "object" && error !== null && "message" in error) {
    const message = String((error as { message?: unknown }).message ?? "").trim();
    if (message) return message;
  }
  if (error instanceof Error && error.message.trim()) return error.message;
  if (typeof error === "string" && error.trim()) return error;
  return "操作失败";
}
