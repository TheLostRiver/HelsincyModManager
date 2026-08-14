import { useCallback, useState } from "react";
import { WindowCloseDialog, type WindowCloseDialogMode } from "./WindowCloseDialog";
import { exitApplication, hideMainWindowToTray } from "./windowLifecycleApi";
import { getWindowLifecycleErrorMessage } from "./windowLifecycleError";
import {
  loadWindowClosePreference,
  saveWindowClosePreference,
  type WindowClosePreference,
} from "./windowClosePreference";
import { requestOrdinaryExit, useWindowCloseRequest } from "./useWindowCloseRequest";

const WINDOW_CLOSE_PREFERENCE_SAVE_ERROR = "关闭行为偏好保存失败，请检查应用存储权限后重试。";

class WindowClosePreferenceSaveError extends Error {}

export function WindowCloseDialogHost() {
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
        setErrorMessage(restoreFailed ? WINDOW_CLOSE_PREFERENCE_SAVE_ERROR : null);
        setMode(confirmation);
      }
    } catch (error) {
      const restoreFailed = !restorePreviousPreference();
      setErrorMessage(
        restoreFailed || error instanceof WindowClosePreferenceSaveError
          ? WINDOW_CLOSE_PREFERENCE_SAVE_ERROR
          : getWindowLifecycleErrorMessage(error),
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
