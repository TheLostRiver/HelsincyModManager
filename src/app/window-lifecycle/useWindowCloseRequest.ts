import { listen } from "@tauri-apps/api/event";
import { useEffect } from "react";
import { exitApplication, hideMainWindowToTray, WINDOW_CLOSE_REQUESTED_EVENT } from "./windowLifecycleApi";
import { loadWindowClosePreference, resolveWindowCloseAction } from "./windowClosePreference";

type UseWindowCloseRequestOptions = {
  onShowDialog: () => void;
  onError: (message: string) => void;
};

function getErrorMessage(error: unknown): string {
  if (error instanceof Error && error.message) return error.message;
  if (typeof error === "string" && error.trim()) return error;
  return "窗口关闭操作失败";
}

export function useWindowCloseRequest({ onShowDialog, onError }: UseWindowCloseRequestOptions) {
  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;

    void listen(WINDOW_CLOSE_REQUESTED_EVENT, () => {
      const action = resolveWindowCloseAction(loadWindowClosePreference());
      if (action === "show_dialog") {
        onShowDialog();
        return;
      }

      const command = action === "hide_to_tray" ? hideMainWindowToTray : exitApplication;
      void command().catch((error: unknown) => {
        onError(getErrorMessage(error));
        onShowDialog();
      });
    })
      .then((dispose) => {
        if (disposed) dispose();
        else unlisten = dispose;
      })
      .catch(() => {
        // Plain browser previews do not provide the Tauri event bridge.
      });

    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [onError, onShowDialog]);
}
