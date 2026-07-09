import { useCallback, useState } from "react";
import { WindowCloseDialog } from "./WindowCloseDialog";
import { exitApplication, hideMainWindowToTray } from "./windowLifecycleApi";
import { getWindowLifecycleErrorMessage } from "./windowLifecycleError";
import { saveWindowClosePreference, type WindowClosePreference } from "./windowClosePreference";
import { useWindowCloseRequest } from "./useWindowCloseRequest";

const WINDOW_CLOSE_PREFERENCE_SAVE_ERROR = "关闭行为偏好保存失败，请检查应用存储权限后重试。";

export function WindowCloseDialogHost() {
  const [open, setOpen] = useState(false);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  const showDialog = useCallback(() => {
    setErrorMessage(null);
    setOpen(true);
  }, []);

  const handleError = useCallback((message: string) => setErrorMessage(message), []);
  useWindowCloseRequest({ onShowDialog: showDialog, onError: handleError });

  const runAction = useCallback(async (action: WindowClosePreference, remember: boolean) => {
    try {
      if (remember && !saveWindowClosePreference(undefined, action)) {
        throw new Error(WINDOW_CLOSE_PREFERENCE_SAVE_ERROR);
      }

      if (action === "tray") {
        await hideMainWindowToTray();
        setOpen(false);
        return;
      }

      await exitApplication();
    } catch (error) {
      setErrorMessage(getWindowLifecycleErrorMessage(error));
      throw error;
    }
  }, []);

  return (
    <WindowCloseDialog
      open={open}
      errorMessage={errorMessage}
      onCancel={() => setOpen(false)}
      onConfirm={runAction}
    />
  );
}
