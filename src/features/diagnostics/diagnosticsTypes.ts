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
  debugLogStatus: string;
  taskLogStatus: string;
  auditLogStatus: string;
  appLogLines: DiagnosticsTextLine[];
  debugLogLines: DiagnosticsTextLine[];
  taskLogLines: DiagnosticsTextLine[];
  auditEvents: DiagnosticsAuditEvent[];
  evidenceHealth: {
    debugLogStatus: string;
    taskLogStatus: string;
    auditLogStatus: string;
    logStorageStatus: string;
    debugLogEventRejectedCount: number;
    debugLogWriteFailureCount: number;
    debugLogRetentionFailureCount: number;
    taskLogWriteFailureCount: number;
    taskLogRetentionFailureCount: number;
    auditWriteFailureCount: number;
    auditWriteFailureAfterCommitCount: number;
    auditLogRetentionFailureCount: number;
    logStorageFailureCount: number;
    logStorageUnsatisfiedCount: number;
    logStorageSettingsFailureCount: number;
  };
};

export type SupportDiagnosticsExport = {
  exportId: string;
  fileName: string;
  sizeBytes: number;
  appLogLineCount: number;
  debugLogLineCount: number;
  taskLogLineCount: number;
  auditEventCount: number;
  debugLogStatus: string;
  taskLogStatus: string;
  auditLogStatus: string;
  logStorageStatus: string;
  debugLogEventRejectedCount: number;
  debugLogWriteFailureCount: number;
  debugLogRetentionFailureCount: number;
  taskLogWriteFailureCount: number;
  taskLogRetentionFailureCount: number;
  auditWriteFailureCount: number;
  auditWriteFailureAfterCommitCount: number;
  auditLogRetentionFailureCount: number;
  logStorageFailureCount: number;
  logStorageUnsatisfiedCount: number;
  logStorageSettingsFailureCount: number;
};
