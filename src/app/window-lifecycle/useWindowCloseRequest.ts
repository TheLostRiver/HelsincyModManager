import { listen } from "@tauri-apps/api/event";
import { useEffect, useRef } from "react";
import type { WindowCloseDialogMode } from "./WindowCloseDialog";
import {
  exitApplication,
  getAppExitGuard,
  hideMainWindowToTray,
  type AppExitGuardReason,
  WINDOW_CLOSE_REQUESTED_EVENT,
} from "./windowLifecycleApi";
import { getWindowLifecycleErrorCode, getWindowLifecycleErrorMessage } from "./windowLifecycleError";
import { loadWindowClosePreference, resolveWindowCloseAction } from "./windowClosePreference";

type UseWindowCloseRequestOptions = {
  onShowDialog: (mode: WindowCloseDialogMode) => void;
  onError: (message: string) => void;
};

type BeforeExit = () => void | Promise<void>;
const MAX_ORDINARY_EXIT_ATTEMPTS = 2;

function confirmationReason(guard: Awaited<ReturnType<typeof getAppExitGuard>>): AppExitGuardReason | null {
  return guard.decision === "confirmation_required" ? guard.reason : null;
}

export async function requestOrdinaryExit(beforeExit?: BeforeExit): Promise<AppExitGuardReason | null> {
  const initialReason = confirmationReason(await getAppExitGuard());
  if (initialReason) return initialReason;

  await beforeExit?.();
  for (let attempt = 0; attempt < MAX_ORDINARY_EXIT_ATTEMPTS; attempt += 1) {
    try {
      await exitApplication(false);
      return null;
    } catch (error) {
      if (getWindowLifecycleErrorCode(error) !== "exit_confirmation_required") throw error;
    }

    const latestReason = confirmationReason(await getAppExitGuard());
    if (latestReason) return latestReason;
  }

  return "status_unavailable";
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
          if (reason) callbacksRef.current.onShowDialog({ kind: "unsafe", reason });
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
