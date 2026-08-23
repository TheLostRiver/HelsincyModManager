import { useCallback, useRef, useState } from "react";
import { useFeedback } from "../../shared/feedback";
import { resolveCopy, useI18n } from "../../shared/i18n";
import { recoveryCenterCopy } from "./recoveryCenterCopy";
import { exportSupportDiagnostics } from "./recoveryDiagnosticsApi";

export type RecoveryDiagnosticsExportState =
  | { status: "idle" }
  | { status: "confirming" }
  | { status: "exporting" };

export function useRecoveryDiagnosticsExport() {
  const { pushToast } = useFeedback();
  const { locale } = useI18n();
  const toastsCopy = resolveCopy(recoveryCenterCopy, locale).diagnosticsToasts;
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
        pushToast({
          eventKey: `recovery.diagnostics.exported.${result.exportId}`,
          title: toastsCopy.exportedTitle,
          message: toastsCopy.exportedMessage({
            fileName: result.fileName,
            size: formatBytes(result.sizeBytes),
            appLogLineCount: result.appLogLineCount,
            debugLogLineCount: result.debugLogLineCount,
            taskLogLineCount: result.taskLogLineCount,
            auditEventCount: result.auditEventCount,
          }),
          tone: "success",
        });
        setState({ status: "idle" });
      })
      .catch(() => {
        pushToast({
          eventKey: "recovery.diagnostics.export.failed",
          title: toastsCopy.failedTitle,
          message: toastsCopy.failedMessage,
          tone: "danger",
        });
        setState({ status: "idle" });
      })
      .finally(() => {
        exportInFlightRef.current = false;
      });
  }, [pushToast, toastsCopy]);

  return {
    state,
    requestExport,
    confirmExport,
    cancelExport,
  };
}

function formatBytes(sizeBytes: number) {
  if (sizeBytes < 1024) return `${sizeBytes} B`;
  if (sizeBytes < 1024 * 1024) return `${(sizeBytes / 1024).toFixed(1)} KB`;
  return `${(sizeBytes / (1024 * 1024)).toFixed(1)} MB`;
}
