export type DiagnosticsEnvironmentSummary = {
  appVersion: string;
  os: string;
  arch: string;
  gameAdapterIds: string[];
};

export type DiagnosticsTextLine = { source: string; line: string };
export type DiagnosticsAuditEvent = {
  timestampUnixMillis: number;
  category: string;
  operation: string;
  result: string;
  fields: Record<string, string>;
};

export type DiagnosticsPageSnapshot = {
  platformSummary: DiagnosticsEnvironmentSummary | null;
  platformStatus: string;
  appLogStatus: string;
  taskLogStatus: string;
  auditLogStatus: string;
  appLogLines: DiagnosticsTextLine[];
  taskLogLines: DiagnosticsTextLine[];
  auditEvents: DiagnosticsAuditEvent[];
  evidenceHealth: {
    taskLogStatus: string;
    auditLogStatus: string;
    taskLogWriteFailureCount: number;
    auditWriteFailureCount: number;
    auditWriteFailureAfterCommitCount: number;
  };
};

export type SupportDiagnosticsExport = {
  exportId: string;
  fileName: string;
  sizeBytes: number;
  appLogLineCount: number;
  taskLogLineCount: number;
  auditEventCount: number;
};
