#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnosticsEvidenceHealthSnapshot {
    pub task_log_status: String,
    pub audit_log_status: String,
    pub task_log_write_failure_count: u64,
    pub audit_write_failure_count: u64,
    pub audit_write_failure_after_commit_count: u64,
}

pub trait DiagnosticsEvidenceHealth: Send + Sync {
    fn snapshot(&self) -> DiagnosticsEvidenceHealthSnapshot;
    fn record_task_log_write_failure(&self, status: &'static str);
    fn record_audit_write_failure(&self, after_commit: bool);
}
