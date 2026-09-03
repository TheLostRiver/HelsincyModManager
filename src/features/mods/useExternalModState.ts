// #286 external mod state: on-demand scan + cached query + adopt for the detail dialog.
//
// The runner may emit the terminal event BEFORE the start invoke resolves in JS
// (the scan takes seconds; the queued response and the terminal event race).
// Mirroring `useExternalImportTaskProgress`, events that arrive while a start is
// pending are buffered per task id and replayed once the task id is known —
// without this the section can get stuck on "scanning" forever.
//
// Adopt (the only write in this family) is a second task flow on the SAME
// listener: one subscription, dispatched by `kind`. Both flows share the
// buffering rules above, so they are expressed once (`TaskFlow`) and the two
// public actions differ only in which command they start and what a terminal
// event means.

import { listen } from "@tauri-apps/api/event";
import { useCallback, useEffect, useRef, useState } from "react";
import {
  getExternalModState,
  startExternalModAdopt,
  startExternalModStateScan,
  type ExternalModStateDto,
} from "./externalStateApi";
import {
  TASK_PROGRESS_EVENT_NAME,
  type TaskKind,
  type TaskProgressEventDto,
} from "./modImportTypes";

export type ExternalModAdoptCompletion = {
  /** The completed event carried `external_mod_adopt_audit_unavailable`. */
  auditDegraded: boolean;
};

export type ExternalModStateWorkflow = {
  /** Last stored query result; null until the first load answers. */
  state: ExternalModStateDto | null;
  /** True once the initial getter round-trip finished (even on error). */
  loaded: boolean;
  scanning: boolean;
  /** Stable code of a scan that failed to start or finished failed/cancelled. */
  scanErrorCode: string | null;
  adopting: boolean;
  /** Stable code of an adopt that failed to start or finished failed/cancelled. */
  adoptErrorCode: string | null;
  /** The progress listener must be ready before a scan or adopt may start. */
  listenerReady: boolean;
  startScan: () => void;
  /** Writes manifest entries for the scanned, matched, unclaimed files. Confirm first. */
  startAdopt: () => void;
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

/** Mutable bookkeeping of one background task flow (scan or adopt). */
type TaskFlow = {
  kind: TaskKind;
  taskId: string | null;
  startPending: boolean;
  /** Terminal events that raced ahead of the start response, by task id. */
  pendingTerminal: Map<string, TaskProgressEventDto>;
};

function newTaskFlow(kind: TaskKind): TaskFlow {
  return { kind, taskId: null, startPending: false, pendingTerminal: new Map() };
}

function resetTaskFlow(flow: TaskFlow): void {
  flow.taskId = null;
  flow.startPending = false;
  flow.pendingTerminal.clear();
}

/** A start is in flight or a task is running: nothing else may start meanwhile. */
function isFlowActive(flow: TaskFlow): boolean {
  return flow.startPending || flow.taskId !== null;
}

/**
 * Routes a terminal event of this flow's kind: dispatch when it is ours,
 * buffer when our start is still pending, ignore otherwise (foreign task).
 */
function acceptTerminalEvent(
  flow: TaskFlow,
  event: TaskProgressEventDto,
  finish: (event: TaskProgressEventDto) => void,
): void {
  if (flow.taskId === null) {
    if (flow.startPending) {
      flow.pendingTerminal.set(event.taskId, event);
    }
    return;
  }
  if (event.taskId === flow.taskId) {
    finish(event);
  }
}

export function useExternalModState(input: {
  gameId: string;
  profileId: string | null;
  modId: string | null;
  /** The dialog tab is visible; the initial query only runs while active. */
  active: boolean;
  /**
   * Mirrors every stored result that reaches this hook (initial cached load
   * and post-scan re-query) so a page-level session store can keep a copy for
   * the list cards (#286 3b-2, option A).
   */
  onResult?: (modId: string, state: ExternalModStateDto) => void;
  /**
   * The manifest now claims this mod. Called after the stored result was
   * re-queried (the backend drops the record, so the session store sees the
   * mod as never-scanned instead of keeping a stale "externally installed").
   */
  onAdoptCompleted?: (completion: ExternalModAdoptCompletion) => void | Promise<void>;
}): ExternalModStateWorkflow {
  const { gameId, profileId, modId, active, onResult, onAdoptCompleted } = input;
  const [state, setState] = useState<ExternalModStateDto | null>(null);
  const [loaded, setLoaded] = useState(false);
  const [scanning, setScanning] = useState(false);
  const [scanErrorCode, setScanErrorCode] = useState<string | null>(null);
  const [adopting, setAdopting] = useState(false);
  const [adoptErrorCode, setAdoptErrorCode] = useState<string | null>(null);
  const [listenerReady, setListenerReady] = useState(false);

  const generationRef = useRef(0);
  const scanFlowRef = useRef<TaskFlow>(newTaskFlow("external_state_scan"));
  const adoptFlowRef = useRef<TaskFlow>(newTaskFlow("external_mod_adopt"));
  const requestRef = useRef({ gameId, profileId, modId });
  requestRef.current = { gameId, profileId, modId };
  const onResultRef = useRef(onResult);
  onResultRef.current = onResult;
  const onAdoptCompletedRef = useRef(onAdoptCompleted);
  onAdoptCompletedRef.current = onAdoptCompleted;

  const refresh = useCallback(() => {
    const { gameId: requestGameId, profileId: requestProfileId, modId: requestModId } =
      requestRef.current;
    if (requestModId === null || requestProfileId === null) {
      return;
    }
    const generation = generationRef.current;
    void getExternalModState({
      gameId: requestGameId,
      profileId: requestProfileId,
      modId: requestModId,
    })
      .then((dto) => {
        // Report even when the dialog moved on to another mod (generation
        // drift): the (modId -> result) pair itself is still a valid fact for
        // the session store, only this hook's local state must not change.
        onResultRef.current?.(requestModId, dto);
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
    resetTaskFlow(scanFlowRef.current);
    resetTaskFlow(adoptFlowRef.current);
    setState(null);
    setLoaded(false);
    setScanning(false);
    setScanErrorCode(null);
    setAdopting(false);
    setAdoptErrorCode(null);
    if (active && modId !== null && profileId !== null) {
      refresh();
    }
  }, [active, gameId, profileId, modId, refresh]);

  const finishScan = useCallback(
    (event: TaskProgressEventDto) => {
      scanFlowRef.current.taskId = null;
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

  const finishAdopt = useCallback(
    (event: TaskProgressEventDto) => {
      adoptFlowRef.current.taskId = null;
      setAdopting(false);
      if (event.status === "failed") {
        setAdoptErrorCode(event.error ?? "external_mod_adopt_unavailable");
        // A stale rejection means the stored result no longer matches disk;
        // the getter's re-stat surfaces that as `stale` — re-query to show it.
        refresh();
        return;
      }
      if (event.status === "cancelled") {
        setAdoptErrorCode("external_mod_adopt_cancelled");
        return;
      }
      setAdoptErrorCode(null);
      // The backend dropped this mod's scan record: re-query so the session
      // store stops saying "externally installed" for a mod HMM now manages.
      refresh();
      void onAdoptCompletedRef.current?.({
        // Completed events carry an error only for the explicit audit degradation.
        auditDegraded: event.error !== null,
      });
    },
    [refresh],
  );

  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | null = null;

    void listen<TaskProgressEventDto>(TASK_PROGRESS_EVENT_NAME, (event) => {
      if (disposed || !isTerminal(event.payload)) {
        return;
      }
      if (event.payload.kind === scanFlowRef.current.kind) {
        acceptTerminalEvent(scanFlowRef.current, event.payload, finishScan);
      } else if (event.payload.kind === adoptFlowRef.current.kind) {
        acceptTerminalEvent(adoptFlowRef.current, event.payload, finishAdopt);
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
  }, [finishAdopt, finishScan]);

  /**
   * Starts one flow: marks the start pending (so racing terminal events are
   * buffered), invokes the command, then binds the task id and replays a
   * buffered terminal event if one arrived first. Generation drift (the dialog
   * switched mods meanwhile) discards everything — a start belongs to the mod
   * it was issued for.
   */
  const launch = useCallback(
    (
      flow: TaskFlow,
      start: (request: {
        gameId: string;
        profileId: string;
        modId: string;
      }) => Promise<{ task: { taskId: string } }>,
      finish: (event: TaskProgressEventDto) => void,
      setBusy: (busy: boolean) => void,
      setErrorCode: (code: string | null) => void,
      startFailureCode: string,
    ) => {
      const request = requestRef.current;
      if (request.modId === null || request.profileId === null || isFlowActive(flow)) {
        return;
      }
      const generation = generationRef.current;
      flow.startPending = true;
      flow.pendingTerminal.clear();
      setBusy(true);
      setErrorCode(null);
      void start({
        gameId: request.gameId,
        profileId: request.profileId,
        modId: request.modId,
      })
        .then((started) => {
          if (generationRef.current !== generation) {
            return;
          }
          flow.taskId = started.task.taskId;
          const buffered = flow.pendingTerminal.get(started.task.taskId);
          if (buffered) {
            finish(buffered);
          }
        })
        .catch((error: unknown) => {
          if (generationRef.current !== generation) {
            return;
          }
          setBusy(false);
          setErrorCode(errorCodeFrom(error, startFailureCode));
        })
        .finally(() => {
          if (generationRef.current === generation) {
            flow.startPending = false;
            flow.pendingTerminal.clear();
          }
        });
    },
    [],
  );

  // One background task per section at a time. Adopt consumes the stored scan
  // record, so a scan in flight would replace it under the confirmation the
  // user just gave; the reverse guard keeps the state machine symmetric.
  const startScan = useCallback(() => {
    if (isFlowActive(adoptFlowRef.current)) {
      return;
    }
    // A fresh scan supersedes whatever the last adopt attempt reported.
    setAdoptErrorCode(null);
    launch(
      scanFlowRef.current,
      startExternalModStateScan,
      finishScan,
      setScanning,
      setScanErrorCode,
      "external_state_scan_task_unavailable",
    );
  }, [finishScan, launch]);

  const startAdopt = useCallback(() => {
    if (isFlowActive(scanFlowRef.current)) {
      return;
    }
    launch(
      adoptFlowRef.current,
      startExternalModAdopt,
      finishAdopt,
      setAdopting,
      setAdoptErrorCode,
      "external_mod_adopt_task_unavailable",
    );
  }, [finishAdopt, launch]);

  return {
    state,
    loaded,
    scanning,
    scanErrorCode,
    adopting,
    adoptErrorCode,
    listenerReady,
    startScan,
    startAdopt,
    refresh,
  };
}
