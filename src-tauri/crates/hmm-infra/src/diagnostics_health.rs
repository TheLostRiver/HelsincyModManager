use hmm_ports::{DiagnosticsEvidenceHealth, DiagnosticsEvidenceHealthSnapshot};
use std::sync::atomic::{AtomicU64, AtomicU8, Ordering};

#[derive(Default)]
pub struct DiagnosticsEvidenceHealthState {
    task_log_status: AtomicU8,
    audit_log_status: AtomicU8,
    task_log_write_failure_count: AtomicU64,
    audit_write_failure_count: AtomicU64,
    audit_write_failure_after_commit_count: AtomicU64,
}

impl DiagnosticsEvidenceHealth for DiagnosticsEvidenceHealthState {
    fn snapshot(&self) -> DiagnosticsEvidenceHealthSnapshot {
        DiagnosticsEvidenceHealthSnapshot {
            task_log_status: match self.task_log_status.load(Ordering::Acquire) {
                0 => "ok",
                _ => "task_log_write_failed",
            }
            .to_owned(),
            audit_log_status: match self.audit_log_status.load(Ordering::Acquire) {
                0 => "ok",
                1 => "audit_write_failed",
                _ => "audit_write_failed_after_commit",
            }
            .to_owned(),
            task_log_write_failure_count: self.task_log_write_failure_count.load(Ordering::Acquire),
            audit_write_failure_count: self.audit_write_failure_count.load(Ordering::Acquire),
            audit_write_failure_after_commit_count: self
                .audit_write_failure_after_commit_count
                .load(Ordering::Acquire),
        }
    }

    fn record_task_log_write_failure(&self, _status: &'static str) {
        self.task_log_status.store(1, Ordering::Release);
        self.task_log_write_failure_count
            .fetch_add(1, Ordering::AcqRel);
    }

    fn record_audit_write_failure(&self, after_commit: bool) {
        self.audit_log_status
            .fetch_max(if after_commit { 2 } else { 1 }, Ordering::AcqRel);
        self.audit_write_failure_count
            .fetch_add(1, Ordering::AcqRel);
        if after_commit {
            self.audit_write_failure_after_commit_count
                .fetch_add(1, Ordering::AcqRel);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_distinguishes_task_audit_and_post_commit_degradation() {
        let health = DiagnosticsEvidenceHealthState::default();
        health.record_task_log_write_failure("task_log_write_failed");
        health.record_audit_write_failure(false);
        health.record_audit_write_failure(true);

        let snapshot = health.snapshot();
        assert_eq!(snapshot.task_log_status, "task_log_write_failed");
        assert_eq!(snapshot.audit_log_status, "audit_write_failed_after_commit");
        assert_eq!(snapshot.task_log_write_failure_count, 1);
        assert_eq!(snapshot.audit_write_failure_count, 2);
        assert_eq!(snapshot.audit_write_failure_after_commit_count, 1);
    }
}
