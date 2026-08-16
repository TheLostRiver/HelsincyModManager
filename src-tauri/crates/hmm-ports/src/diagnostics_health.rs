#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnosticsEvidenceHealthSnapshot {
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

pub trait DiagnosticsEvidenceHealth: Send + Sync {
    fn snapshot(&self) -> DiagnosticsEvidenceHealthSnapshot;
    fn record_debug_log_event_rejected(&self) {}
    fn record_debug_log_write_failure(&self) {}
    fn record_debug_log_retention_failure(&self) {}
    fn record_task_log_write_failure(&self, status: &'static str);
    fn record_audit_write_failure(&self, after_commit: bool);
    fn record_task_log_retention_failure(&self) {}
    fn record_audit_log_retention_failure(&self) {}
    fn record_log_storage_budget_failure(&self) {}
    fn record_log_storage_budget_unsatisfied(&self) {}
    fn record_log_storage_settings_failure(&self) {}
}
