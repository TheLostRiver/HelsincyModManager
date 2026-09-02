// #286 external mod state: on-demand scan + cached query for the detail dialog.
//
// The runner may emit the terminal event BEFORE the start invoke resolves in JS
// (the scan takes seconds; the queued response and the terminal event race).
// Mirroring `useExternalImportTaskProgress`, events that arrive while a start is
// pending are buffered per task id and replayed once the task id is known —
// without this the section can get stuck on "scanning" forever.

import { listen } from "@tauri-apps/api/event";
import { useCallback, useEffect, useRef, useState } from "react";
import {
  getExternalModState,
  startExternalModStateScan,
  type ExternalModStateDto,
} from "./externalStateApi";
import { TASK_PROGRESS_EVENT_NAME, type TaskProgressEventDto } from "./modImportTypes";

export type ExternalModStateWorkflow = {
  /** Last stored query result; null until the first load answers. */
  state: ExternalModStateDto | null;
  /** True once the initial getter round-trip finished (even on error). */
  loaded: boolean;
  scanning: boolean;
  /** Stable code of a scan that failed to start or finished failed/cancelled. */
  scanErrorCode: string | null;
  /** The progress listener must be ready before a scan may start. */
  listenerReady: boolean;
  startScan: () => void;
  refresh: () => void;
};

function errorCodeFrom(error: unknown, fallback: string): string {
  if (
    typeof error === "object" &&
    error !== null &&
    "code" in error &&
    typeof error.code === "string"
  ) {
    return error.code;
  }
  return fallback;
}

function isTerminal(event: TaskProgressEventDto): boolean {
  return (
    event.status === "completed" ||
    event.status === "failed" ||
    event.status === "cancelled"
  );
}

export function useExternalModState(input: {
  gameId: string;
  profileId: string | null;
  modId: string | null;
  /** The dialog tab is visible; the initial query only runs while active. */
  active: boolean;
}): ExternalModStateWorkflow {
  const { gameId, profileId, modId, active } = input;
  const [state, setState] = useState<ExternalModStateDto | null>(null);
  const [loaded, setLoaded] = useState(false);
  const [scanning, setScanning] = useState(false);
  const [scanErrorCode, setScanErrorCode] = useState<string | null>(null);
  const [listenerReady, setListenerReady] = useState(false);

  const generationRef = useRef(0);
  const taskIdRef = useRef<string | null>(null);
  const startPendingRef = useRef(false);
  const pendingTerminalRef = useRef(new Map<string, TaskProgressEventDto>());
  const requestRef = useRef({ gameId, profileId, modId });
  requestRef.current = { gameId, profileId, modId };

  const refresh = useCallback(() => {
    const request = requestRef.current;
    if (request.modId === null || request.profileId === null) {
      return;
    }
    const generation = generationRef.current;
    void getExternalModState({
      gameId: request.gameId,
      profileId: request.profileId,
      modId: request.modId,
    })
      .then((dto) => {
        if (generationRef.current === generation) {
          setState(dto);
          setLoaded(true);
        }
      })
      .catch(() => {
        // The query is read-only; on transport failure keep whatever we had
        // and let the user retry via the scan action.
        if (generationRef.current === generation) {
          setLoaded(true);
        }
      });
  }, []);

  // Reset per mod and load the stored result (cheap: cache + re-stat).
  useEffect(() => {
    generationRef.current += 1;
    taskIdRef.current = null;
    startPendingRef.current = false;
    pendingTerminalRef.current.clear();
    setState(null);
    setLoaded(false);
    setScanning(false);
    setScanErrorCode(null);
    if (active && modId !== null && profileId !== null) {
      refresh();
    }
  }, [active, gameId, profileId, modId, refresh]);

  const finishScan = useCallback(
    (event: TaskProgressEventDto) => {
      taskIdRef.current = null;
      setScanning(false);
      if (event.status === "failed") {
        setScanErrorCode(event.error ?? "external_state_scan_unavailable");
      } else if (event.status === "cancelled") {
        setScanErrorCode("external_state_scan_cancelled");
      } else {
        setScanErrorCode(null);
      }
      // Success and failure both land in the store (failure keeps the previous
      // summary and records the reason) — re-query either way.
      refresh();
    },
    [refresh],
  );

  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | null = null;

    void listen<TaskProgressEventDto>(TASK_PROGRESS_EVENT_NAME, (event) => {
      if (disposed || event.payload.kind !== "external_state_scan") {
        return;
      }
      if (!isTerminal(event.payload)) {
        return;
      }
      const taskId = taskIdRef.current;
      if (taskId === null) {
        if (startPendingRef.current) {
          pendingTerminalRef.current.set(event.payload.taskId, event.payload);
        }
        return;
      }
      if (event.payload.taskId === taskId) {
        finishScan(event.payload);
      }
    })
      .then((dispose) => {
        if (disposed) {
          dispose();
          return;
        }
        unlisten = dispose;
        setListenerReady(true);
      })
      .catch(() => {
        if (!disposed) {
          setListenerReady(false);
        }
      });

    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [finishScan]);

  const startScan = useCallback(() => {
    const request = requestRef.current;
    if (
      request.modId === null ||
      request.profileId === null ||
      startPendingRef.current ||
      taskIdRef.current !== null
    ) {
      return;
    }
    const generation = generationRef.current;
    startPendingRef.current = true;
    pendingTerminalRef.current.clear();
    setScanning(true);
    setScanErrorCode(null);
    void startExternalModStateScan({
      gameId: request.gameId,
      profileId: request.profileId,
      modId: request.modId,
    })
      .then((started) => {
        if (generationRef.current !== generation) {
          return;
        }
        taskIdRef.current = started.task.taskId;
        const buffered = pendingTerminalRef.current.get(started.task.taskId);
        if (buffered) {
          finishScan(buffered);
        }
      })
      .catch((error: unknown) => {
        if (generationRef.current !== generation) {
          return;
        }
        setScanning(false);
        setScanErrorCode(
          errorCodeFrom(error, "external_state_scan_task_unavailable"),
        );
      })
      .finally(() => {
        if (generationRef.current === generation) {
          startPendingRef.current = false;
          pendingTerminalRef.current.clear();
        }
      });
  }, [finishScan]);

  return {
    state,
    loaded,
    scanning,
    scanErrorCode,
    listenerReady,
    startScan,
    refresh,
  };
}
