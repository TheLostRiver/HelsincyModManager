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
        await exitApplication(true);
        return;
      }

      const previousPreference = remember ? loadWindowClosePreference() : null;
      let preferenceSaved = false;
      const reason = await requestOrdinaryExit(() => {
        if (remember && !saveWindowClosePreference(undefined, action)) {
          throw new WindowClosePreferenceSaveError();
        }
        preferenceSaved = remember;
      });
      if (reason) {
        const restoreFailed =
          preferenceSaved &&
          previousPreference !== null &&
          !saveWindowClosePreference(undefined, previousPreference);
        setErrorMessage(restoreFailed ? WINDOW_CLOSE_PREFERENCE_SAVE_ERROR : null);
        setMode({ kind: "unsafe", reason });
      }
    } catch (error) {
      setErrorMessage(
        error instanceof WindowClosePreferenceSaveError
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
