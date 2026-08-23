import { useCallback, useRef, useState } from "react";
import { resolveCopy, useI18n } from "../../shared/i18n";
import { appShellCopy } from "../appShellCopy";
import { WindowCloseDialog, type WindowCloseDialogMode } from "./WindowCloseDialog";
import { exitApplication, hideMainWindowToTray } from "./windowLifecycleApi";
import { getWindowLifecycleErrorMessage } from "./windowLifecycleError";
import {
  loadWindowClosePreference,
  saveWindowClosePreference,
  type WindowClosePreference,
} from "./windowClosePreference";
import { requestOrdinaryExit, useWindowCloseRequest } from "./useWindowCloseRequest";

class WindowClosePreferenceSaveError extends Error {}

export function WindowCloseDialogHost() {
  const { locale } = useI18n();
  const lifecycleCopy = resolveCopy(appShellCopy, locale).windowLifecycle;
  // 关闭请求回调链经 ref 取词，避免语言切换重建窗口关闭监听。
  const lifecycleCopyRef = useRef(lifecycleCopy);
  lifecycleCopyRef.current = lifecycleCopy;
  const [mode, setMode] = useState<WindowCloseDialogMode | null>(null);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  const showDialog = useCallback((nextMode: WindowCloseDialogMode) => {
    setErrorMessage(null);
    setMode(nextMode);
  }, []);

  const handleError = useCallback((message: string) => setErrorMessage(message), []);
  useWindowCloseRequest({ onShowDialog: showDialog, onError: handleError });

  const runAction = useCallback(async (action: WindowClosePreference, remember: boolean) => {
    if (!mode) return;

    let previousPreference: WindowClosePreference | null = null;
    let preferenceSaved = false;
    const restorePreviousPreference = () => {
      if (!preferenceSaved || previousPreference === null) return true;
      const restored = saveWindowClosePreference(undefined, previousPreference);
      if (restored) preferenceSaved = false;
      return restored;
    };

    try {
      if (action === "tray") {
        if (mode.kind === "normal" && remember && !saveWindowClosePreference(undefined, action)) {
          throw new WindowClosePreferenceSaveError();
        }
        await hideMainWindowToTray();
        setMode(null);
        return;
      }

      if (mode.kind === "unsafe") {
        const result = await exitApplication(true, mode.exitAuthorization);
        if (result.outcome === "confirmation_required") {
          setErrorMessage(null);
          setMode({
            kind: "unsafe",
            reason: result.reason,
            exitAuthorization: result.exitAuthorization,
          });
        }
        if (result.outcome === "blocked") {
          setErrorMessage(null);
          setMode({ kind: "blocked", reason: result.reason });
        }
        return;
      }

      if (mode.kind === "blocked") {
        return;
      }

      previousPreference = remember ? loadWindowClosePreference() : null;
      const confirmation = await requestOrdinaryExit(() => {
        if (remember && !saveWindowClosePreference(undefined, action)) {
          throw new WindowClosePreferenceSaveError();
        }
        preferenceSaved = remember;
      });
      if (confirmation) {
        const restoreFailed = !restorePreviousPreference();
        setErrorMessage(restoreFailed ? lifecycleCopyRef.current.preferenceSaveError : null);
        setMode(confirmation);
      }
    } catch (error) {
      const restoreFailed = !restorePreviousPreference();
      setErrorMessage(
        restoreFailed || error instanceof WindowClosePreferenceSaveError
          ? lifecycleCopyRef.current.preferenceSaveError
          : getWindowLifecycleErrorMessage(error, lifecycleCopyRef.current),
      );
      throw error;
    }
  }, [mode]);

  return (
    <WindowCloseDialog
      mode={mode}
      errorMessage={errorMessage}
      onCancel={() => setMode(null)}
      onConfirm={runAction}
    />
  );
}
