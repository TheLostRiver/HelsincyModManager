import { listen } from "@tauri-apps/api/event";
import { useEffect, useRef } from "react";
import type { WindowCloseDialogMode } from "./WindowCloseDialog";
import {
  exitApplication,
  hideMainWindowToTray,
  WINDOW_CLOSE_REQUESTED_EVENT,
} from "./windowLifecycleApi";
import { getWindowLifecycleErrorMessage } from "./windowLifecycleError";
import { loadWindowClosePreference, resolveWindowCloseAction } from "./windowClosePreference";

type UseWindowCloseRequestOptions = {
  onShowDialog: (mode: WindowCloseDialogMode) => void;
  onError: (message: string) => void;
};

type BeforeExit = () => void | Promise<void>;
type ExitConfirmation = Extract<WindowCloseDialogMode, { kind: "unsafe" }>;

export async function requestOrdinaryExit(beforeExit?: BeforeExit): Promise<ExitConfirmation | null> {
  await beforeExit?.();
  const result = await exitApplication(false);
  return result.outcome === "confirmation_required"
    ? {
        kind: "unsafe",
        reason: result.reason,
        exitAuthorization: result.exitAuthorization,
      }
    : null;
}

export function useWindowCloseRequest({ onShowDialog, onError }: UseWindowCloseRequestOptions) {
  const callbacksRef = useRef({ onShowDialog, onError });
  callbacksRef.current = { onShowDialog, onError };

  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;

    void listen(WINDOW_CLOSE_REQUESTED_EVENT, () => {
      const action = resolveWindowCloseAction(loadWindowClosePreference());
      if (action === "show_dialog") {
        callbacksRef.current.onShowDialog({ kind: "normal" });
        return;
      }

      if (action === "hide_to_tray") {
        void hideMainWindowToTray().catch((error: unknown) => {
          callbacksRef.current.onShowDialog({ kind: "normal" });
          callbacksRef.current.onError(getWindowLifecycleErrorMessage(error));
        });
        return;
      }

      void requestOrdinaryExit()
        .then((reason) => {
          if (reason) callbacksRef.current.onShowDialog(reason);
        })
        .catch((error: unknown) => {
          callbacksRef.current.onShowDialog({ kind: "normal" });
          callbacksRef.current.onError(getWindowLifecycleErrorMessage(error));
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
