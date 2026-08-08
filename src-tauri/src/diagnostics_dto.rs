use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticsPageSnapshotDto {
    pub platform_summary: Option<hmm_ports::DiagnosticsEnvironmentSummary>,
    pub platform_status: String,
    pub app_log_status: String,
    pub debug_log_status: String,
    pub task_log_status: String,
    pub audit_log_status: String,
    pub app_log_lines: Vec<hmm_ports::TextLogLine>,
    pub debug_log_lines: Vec<hmm_ports::TextLogLine>,
    pub task_log_lines: Vec<hmm_ports::TextLogLine>,
    pub audit_events: Vec<hmm_ports::AuditLogEvent>,
    pub evidence_health: DiagnosticsEvidenceHealthDto,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticsEvidenceHealthDto {
    pub debug_log_status: String,
    pub task_log_status: String,
    pub audit_log_status: String,
    pub log_storage_status: String,
    pub debug_log_event_rejected_count: u64,
    pub debug_log_write_failure_count: u64,
    pub debug_log_retention_failure_count: u64,
    pub task_log_write_failure_count: u64,
    pub task_log_retention_failure_count: u64,
    pub audit_write_failure_count: u64,
    pub audit_write_failure_after_commit_count: u64,
    pub audit_log_retention_failure_count: u64,
    pub log_storage_failure_count: u64,
    pub log_storage_unsatisfied_count: u64,
    pub log_storage_settings_failure_count: u64,
}

impl From<hmm_app::DiagnosticsPageSnapshot> for DiagnosticsPageSnapshotDto {
    fn from(snapshot: hmm_app::DiagnosticsPageSnapshot) -> Self {
        Self {
            platform_summary: snapshot.platform_summary,
            platform_status: snapshot.platform_status,
            app_log_status: snapshot.app_log_status,
            debug_log_status: snapshot.debug_log_status,
            task_log_status: snapshot.task_log_status,
            audit_log_status: snapshot.audit_log_status,
            app_log_lines: snapshot.app_log_lines,
            debug_log_lines: snapshot.debug_log_lines,
            task_log_lines: snapshot.task_log_lines,
            audit_events: snapshot.audit_events,
            evidence_health: DiagnosticsEvidenceHealthDto {
                debug_log_status: snapshot.evidence_health.debug_log_status,
                task_log_status: snapshot.evidence_health.task_log_status,
                audit_log_status: snapshot.evidence_health.audit_log_status,
                log_storage_status: snapshot.evidence_health.log_storage_status,
                debug_log_event_rejected_count: snapshot
                    .evidence_health
                    .debug_log_event_rejected_count,
                debug_log_write_failure_count: snapshot
                    .evidence_health
                    .debug_log_write_failure_count,
                debug_log_retention_failure_count: snapshot
                    .evidence_health
                    .debug_log_retention_failure_count,
                task_log_write_failure_count: snapshot.evidence_health.task_log_write_failure_count,
                task_log_retention_failure_count: snapshot
                    .evidence_health
                    .task_log_retention_failure_count,
                audit_write_failure_count: snapshot.evidence_health.audit_write_failure_count,
                audit_write_failure_after_commit_count: snapshot
                    .evidence_health
                    .audit_write_failure_after_commit_count,
                audit_log_retention_failure_count: snapshot
                    .evidence_health
                    .audit_log_retention_failure_count,
                log_storage_failure_count: snapshot.evidence_health.log_storage_failure_count,
                log_storage_unsatisfied_count: snapshot
                    .evidence_health
                    .log_storage_unsatisfied_count,
                log_storage_settings_failure_count: snapshot
                    .evidence_health
                    .log_storage_settings_failure_count,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_diagnostics_page_snapshot_without_path_fields() {
        let dto: DiagnosticsPageSnapshotDto = hmm_app::DiagnosticsPageSnapshot {
            platform_summary: None,
            platform_status: "environment_unavailable".to_owned(),
            app_log_status: "ok".to_owned(),
            debug_log_status: "ok".to_owned(),
            task_log_status: "task_log_read_failed".to_owned(),
            audit_log_status: "ok".to_owned(),
            app_log_lines: vec![hmm_ports::TextLogLine {
                source: "app-2026-07-17.jsonl".to_owned(),
                line: "safe event".to_owned(),
            }],
            debug_log_lines: vec![],
            task_log_lines: vec![],
            audit_events: vec![],
            evidence_health: hmm_ports::DiagnosticsEvidenceHealthSnapshot {
                debug_log_status: "ok".to_owned(),
                task_log_status: "task_log_write_failed".to_owned(),
                audit_log_status: "ok".to_owned(),
                log_storage_status: "log_storage_budget_unsatisfied".to_owned(),
                debug_log_event_rejected_count: 0,
                debug_log_write_failure_count: 0,
                debug_log_retention_failure_count: 0,
                task_log_write_failure_count: 1,
                task_log_retention_failure_count: 2,
                audit_write_failure_count: 0,
                audit_write_failure_after_commit_count: 0,
                audit_log_retention_failure_count: 3,
                log_storage_failure_count: 4,
                log_storage_unsatisfied_count: 5,
                log_storage_settings_failure_count: 6,
            },
        }
        .into();
        let value = serde_json::to_value(dto).expect("serialize diagnostics page snapshot");
        assert_eq!(value["taskLogStatus"], "task_log_read_failed");
        assert_eq!(value["evidenceHealth"]["taskLogWriteFailureCount"], 1);
        assert_eq!(value["evidenceHealth"]["taskLogRetentionFailureCount"], 2);
        assert_eq!(value["evidenceHealth"]["auditLogRetentionFailureCount"], 3);
        assert_eq!(
            value["evidenceHealth"]["logStorageStatus"],
            "log_storage_budget_unsatisfied"
        );
        assert_eq!(value["evidenceHealth"]["logStorageFailureCount"], 4);
        for forbidden in ["path", "rawError", "C:/", "\\Users\\"] {
            assert!(!value.to_string().contains(forbidden));
        }
    }
}
