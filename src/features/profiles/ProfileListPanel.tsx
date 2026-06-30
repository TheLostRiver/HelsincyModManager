import {
  AlertTriangle,
  CheckCircle2,
  Loader2,
  Pencil,
  Plus,
  RefreshCw,
  Save,
  Trash2,
  User,
  X,
} from "lucide-react";
import { useEffect, useMemo, useRef, useState, type FormEvent } from "react";
import { createPortal } from "react-dom";
import { createProfile, deleteProfile, updateProfile } from "./profileApi";
import type { Profile } from "./profileTypes";

const defaultProfileId = "default";

type ProfileListPanelProps = {
  profiles: Profile[];
  status: "loading" | "ready" | "error";
  selectedProfileId: string | null;
  busyProfileId: string | null;
  onRefresh: () => void;
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

  const clearTransientState = () => {
    setEditingId(null);
    setPendingDeleteId(null);
    setError(null);
  };

  const closeFloatingForm = () => {
    setShowCreateForm(false);
    setEditingId(null);
  };

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
  }, [floatingFormOpen]);

  return (
    <aside className="profile-list-panel" aria-labelledby="profile-list-title">
      <div className="profile-list-panel__floating-root">
        <div className="profile-list-panel__header">
          <div>
            <h2 id="profile-list-title">配置档</h2>
            <span>{status === "ready" ? `${profiles.length} 个槽位` : "读取中"}</span>
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
            <button
              type="button"
              className="profile-icon-button"
              aria-label={showCreateForm ? "关闭新建配置档" : "新建配置档"}
              onClick={() => {
                if (showCreateForm) {
                  closeFloatingForm();
                  return;
                }
                clearTransientState();
                setShowCreateForm(true);
              }}
            >
              {showCreateForm ? <X size={15} /> : <Plus size={15} />}
            </button>
          </div>
        </div>

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
                      <span>配置档信息</span>
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
          <span>正在读取配置档</span>
        </div>
      ) : null}

      {status === "error" ? (
        <div className="profile-list-state is-error" role="alert">
          <AlertTriangle size={18} />
          <span>配置档不可用</span>
          <button type="button" className="profile-action-button" onClick={onRefresh}>
            重试
          </button>
        </div>
      ) : null}

      {status === "ready" ? (
        <div className="profile-list" role="list" aria-label="配置档列表">
          {profiles.map((profile) => (
            <ProfileListItem
              key={profile.id}
              profile={profile}
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
  const cannotDelete = profile.id === defaultProfileId || profile.isActive;

  return (
    <article className={`profile-list-item ${selected ? "is-selected" : ""}`} role="listitem">
      <button type="button" className="profile-list-item__select" onClick={onSelect}>
        <span className="profile-list-item__avatar" aria-hidden="true">
          <User size={17} />
        </span>
        <span className="profile-list-item__copy">
          <strong>{profile.name}</strong>
          <small>{profile.description || profile.id}</small>
        </span>
        {profile.isActive ? (
          <span className="profile-status-pill is-success">
            <CheckCircle2 size={13} />
            当前
          </span>
        ) : null}
      </button>

      <div className="profile-list-item__actions">
        {!profile.isActive ? (
          <button type="button" className="profile-action-button is-primary" onClick={onActivate} disabled={busy}>
            {busy ? "启用中" : "启用"}
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
          title={profile.isActive ? "active profile cannot be deleted" : undefined}
        >
          <Trash2 size={14} />
        </button>
      </div>

      {confirmingDelete && !cannotDelete ? (
        <div className="profile-delete-confirm">
          <span>删除 {profile.name}？</span>
          <div>
            <button type="button" className="profile-action-button is-danger" onClick={onDelete}>
              删除
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
        <span>名称</span>
        <input value={name} onChange={(event) => setName(event.target.value)} autoFocus />
      </label>
      <label className="profile-field">
        <span>描述</span>
        <textarea value={description} rows={4} onChange={(event) => setDescription(event.target.value)} />
      </label>
      <div className="profile-form-actions">
        <button type="submit" className="profile-action-button is-primary" disabled={!name.trim() || submitting}>
          <Plus size={14} />
          {submitting ? "创建中" : "创建"}
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
        <span>名称</span>
        <input value={name} onChange={(event) => setName(event.target.value)} autoFocus />
      </label>
      <label className="profile-field">
        <span>描述</span>
        <textarea value={description} rows={4} onChange={(event) => setDescription(event.target.value)} />
      </label>
      <div className="profile-form-actions">
        <button type="submit" className="profile-action-button is-primary" disabled={!name.trim() || submitting}>
          <Save size={14} />
          {submitting ? "保存中" : "保存"}
        </button>
        <button type="button" className="profile-action-button" onClick={onCancel}>
          取消
        </button>
      </div>
    </form>
  );
}

function getProfileErrorMessage(error: unknown) {
  if (typeof error === "object" && error !== null && "message" in error) {
    const message = String((error as { message?: unknown }).message ?? "").trim();
    if (message) return message;
  }
  if (error instanceof Error && error.message.trim()) return error.message;
  if (typeof error === "string" && error.trim()) return error;
  return "操作失败";
}
