import { listen } from "@tauri-apps/api/event";
import { useEffect, useRef } from "react";
import { exitApplication, hideMainWindowToTray, WINDOW_CLOSE_REQUESTED_EVENT } from "./windowLifecycleApi";
import { getWindowLifecycleErrorMessage } from "./windowLifecycleError";
import { loadWindowClosePreference, resolveWindowCloseAction } from "./windowClosePreference";

type UseWindowCloseRequestOptions = {
  onShowDialog: () => void;
  onError: (message: string) => void;
};

export function useWindowCloseRequest({ onShowDialog, onError }: UseWindowCloseRequestOptions) {
  const callbacksRef = useRef({ onShowDialog, onError });
  callbacksRef.current = { onShowDialog, onError };

  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;

    void listen(WINDOW_CLOSE_REQUESTED_EVENT, () => {
      const action = resolveWindowCloseAction(loadWindowClosePreference());
      if (action === "show_dialog") {
        callbacksRef.current.onShowDialog();
        return;
      }

      const command = action === "hide_to_tray" ? hideMainWindowToTray : exitApplication;
      void command().catch((error: unknown) => {
        callbacksRef.current.onError(getWindowLifecycleErrorMessage(error));
        callbacksRef.current.onShowDialog();
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
  }, []);
}
