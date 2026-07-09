import { useCallback, useState } from "react";
import { WindowCloseDialog } from "./WindowCloseDialog";
import { exitApplication, hideMainWindowToTray } from "./windowLifecycleApi";
import { saveWindowClosePreference, type WindowClosePreference } from "./windowClosePreference";
import { useWindowCloseRequest } from "./useWindowCloseRequest";

function getErrorMessage(error: unknown): string {
  if (error instanceof Error && error.message) return error.message;
  if (typeof error === "string" && error.trim()) return error;
  return "窗口关闭操作失败";
}

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
      if (action === "tray") {
        await hideMainWindowToTray();
        if (remember) saveWindowClosePreference(undefined, action);
        setOpen(false);
        return;
      }

      if (remember) saveWindowClosePreference(undefined, action);
      await exitApplication();
    } catch (error) {
      setErrorMessage(getErrorMessage(error));
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
