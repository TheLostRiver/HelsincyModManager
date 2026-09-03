import { AlertTriangle, FileX2, LoaderCircle, RefreshCw } from "lucide-react";
import { useEffect, useId, useRef, useState } from "react";
import { Dialog } from "../../shared/feedback";
import { resolveCopy, useI18n } from "../../shared/i18n";
import { getModImportSettings, setModImportSettings } from "./modImportSettingsApi";
import { modImportSettingsCopy, type ModImportSettingsCopy } from "./modImportSettingsCopy";
import {
  getModImportSettingsErrorMessage,
  isModImportSettingsDto,
  type ModImportSettingsState,
} from "./modImportSettingsTypes";

/**
 * "Delete the original archive after import" (#275 slice 4). Turning it on is the only
 * destructive choice on this page, so it goes through an alertdialog first; turning it off
 * saves immediately.
 */
export function ModImportSettingsPanel() {
  const { locale } = useI18n();
  const copy = resolveCopy(modImportSettingsCopy, locale);
  const [state, setState] = useState<ModImportSettingsState>({ status: "loading" });
  const [saving, setSaving] = useState(false);
  const [confirming, setConfirming] = useState(false);
  const mountedRef = useRef(false);
  const descriptionId = useId();

  const load = () => {
    setState({ status: "loading" });
    void getModImportSettings()
      .then((settings) => {
        if (!mountedRef.current) {
          return;
        }
        setState(
          isModImportSettingsDto(settings)
            ? { status: "ready", settings, saveFailed: false }
            : { status: "error" },
        );
      })
      .catch(() => {
        if (mountedRef.current) {
          setState({ status: "error" });
        }
      });
  };

  useEffect(() => {
    mountedRef.current = true;
    load();
    return () => {
      mountedRef.current = false;
    };
  }, []);

  const save = (deleteArchiveAfterImport: boolean) => {
    if (state.status !== "ready" || saving) {
      return;
    }
    setSaving(true);
    void setModImportSettings(deleteArchiveAfterImport)
      .then((settings) => {
        if (!mountedRef.current) {
          return;
        }
        setState(
          isModImportSettingsDto(settings)
            ? { status: "ready", settings, saveFailed: false }
            : { status: "error" },
        );
      })
      .catch(() => {
        if (mountedRef.current && state.status === "ready") {
          setState({ status: "ready", settings: state.settings, saveFailed: true });
        }
      })
      .finally(() => {
        if (mountedRef.current) {
          setSaving(false);
        }
      });
  };

  if (state.status === "loading") {
    return (
      <div className="mod-import-settings-panel" role="status" aria-busy="true">
        <LoaderCircle className="mod-import-settings-panel__spinner" size={16} aria-hidden="true" />
        <span>{copy.loading}</span>
      </div>
    );
  }

  if (state.status === "error") {
    return (
      <div className="settings-callout mod-import-settings-panel__error" role="alert">
        <AlertTriangle size={16} strokeWidth={2.1} />
        <span>{getModImportSettingsErrorMessage("load", locale)}</span>
        <button type="button" onClick={load}>
          <RefreshCw size={14} aria-hidden="true" />
          {copy.recheck}
        </button>
      </div>
    );
  }

  const enabled = state.settings.deleteArchiveAfterImport;
  return (
    <div className="mod-import-settings-panel" aria-busy={saving}>
      <label className="setting-row mod-import-settings-panel__toggle">
        <span className="setting-row__copy">
          <strong>{copy.toggleTitle}</strong>
          <span>{copy.toggleDescription}</span>
        </span>
        <input
          type="checkbox"
          checked={enabled}
          disabled={saving}
          aria-describedby={descriptionId}
          onChange={(event) => {
            // Enabling deletes user files from now on: confirm first. Disabling is safe.
            if (event.currentTarget.checked) {
              setConfirming(true);
            } else {
              save(false);
            }
          }}
        />
        <span className="setting-switch" aria-hidden="true" />
      </label>
      <span id={descriptionId} className="mod-import-settings-panel__status" role="status" aria-live="polite">
        {saving ? copy.saving : enabled ? copy.enabled : copy.disabled}
      </span>
      {enabled ? (
        <div className="settings-callout mod-import-settings-panel__note" role="note">
          <FileX2 size={16} strokeWidth={2.1} />
          <span>{copy.enabledNote}</span>
        </div>
      ) : null}
      {state.saveFailed ? (
        <div className="settings-callout mod-import-settings-panel__error" role="alert">
          <AlertTriangle size={16} strokeWidth={2.1} />
          <span>{getModImportSettingsErrorMessage("save", locale)}</span>
        </div>
      ) : null}
      <EnableConfirmDialog
        open={confirming}
        copy={copy}
        onCancel={() => setConfirming(false)}
        onConfirm={() => {
          setConfirming(false);
          save(true);
        }}
      />
    </div>
  );
}

type EnableConfirmDialogProps = {
  open: boolean;
  copy: ModImportSettingsCopy;
  onCancel: () => void;
  onConfirm: () => void;
};

function EnableConfirmDialog({ open, copy, onCancel, onConfirm }: EnableConfirmDialogProps) {
  const cancelButtonRef = useRef<HTMLButtonElement | null>(null);
  return (
    <Dialog
      open={open}
      role="alertdialog"
      title={copy.confirm.title}
      description={copy.confirm.body}
      icon={<FileX2 size={20} />}
      onClose={onCancel}
      closeLabel={copy.confirm.closeAria}
      closeOnBackdrop={false}
      initialFocusRef={cancelButtonRef}
      footer={
        <>
          <button ref={cancelButtonRef} type="button" className="mod-storage-panel__button" onClick={onCancel}>
            {copy.confirm.cancel}
          </button>
          <button type="button" className="mod-storage-panel__button is-danger" onClick={onConfirm}>
            {copy.confirm.confirm}
          </button>
        </>
      }
    >
      <ul className="mod-storage-panel__steps">
        <li>{copy.confirm.pointConsumed}</li>
        <li>{copy.confirm.pointCrossVolume}</li>
        <li>{copy.confirm.pointProtected}</li>
      </ul>
    </Dialog>
  );
}
