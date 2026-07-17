import { useCallback, useRef, useState } from "react";
import { useFeedback } from "../../shared/feedback";
import { exportSupportDiagnostics } from "./recoveryDiagnosticsApi";

export type RecoveryDiagnosticsExportState =
  | { status: "idle" }
  | { status: "confirming" }
  | { status: "exporting" };

export function useRecoveryDiagnosticsExport() {
  const { pushToast } = useFeedback();
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
          title: "诊断包已导出",
          message: `${result.fileName}，${formatBytes(result.sizeBytes)}；App 日志 ${result.appLogLineCount} 行，任务日志 ${result.taskLogLineCount} 行，审计事件 ${result.auditEventCount} 条。`,
          tone: "success",
        });
        setState({ status: "idle" });
      })
      .catch(() => {
        pushToast({
          eventKey: "recovery.diagnostics.export.failed",
          title: "诊断导出失败",
          message: "诊断包暂时不可用，请稍后重试并保留当前恢复中心状态。",
          tone: "danger",
        });
        setState({ status: "idle" });
      })
      .finally(() => {
        exportInFlightRef.current = false;
      });
  }, [pushToast]);

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
