import { useCallback, useRef, useState } from "react";
import { exportSupportDiagnostics } from "./recoveryDiagnosticsApi";
import type { SupportDiagnosticsExport } from "./recoveryDiagnosticsTypes";

export type RecoveryDiagnosticsExportState =
  | { status: "idle" }
  | { status: "confirming" }
  | { status: "exporting" }
  | { status: "exported"; result: SupportDiagnosticsExport }
  | { status: "failed" };

export function useRecoveryDiagnosticsExport() {
  const [state, setState] = useState<RecoveryDiagnosticsExportState>({ status: "idle" });
  const exportInFlightRef = useRef(false);

  const requestExport = useCallback(() => {
    if (exportInFlightRef.current) {
      return;
    }

    setState({ status: "confirming" });
  }, []);

  const cancelExport = useCallback(() => {
    if (exportInFlightRef.current) {
      return;
    }

    setState({ status: "idle" });
  }, []);

  const confirmExport = useCallback(() => {
    if (exportInFlightRef.current) {
      return;
    }

    exportInFlightRef.current = true;
    setState({ status: "exporting" });

    void exportSupportDiagnostics()
      .then((result) => {
        setState({ status: "exported", result });
      })
      .catch(() => {
        setState({ status: "failed" });
      })
      .finally(() => {
        exportInFlightRef.current = false;
      });
  }, []);

  return {
    state,
    requestExport,
    confirmExport,
    cancelExport,
  };
}
