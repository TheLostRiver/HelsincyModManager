use hmm_ports::{DiagnosticsEvidenceHealth, DiagnosticsEvidenceHealthSnapshot};
use std::sync::atomic::{AtomicU64, AtomicU8, Ordering};

#[derive(Default)]
pub struct DiagnosticsEvidenceHealthState {
    debug_log_status: AtomicU8,
    task_log_status: AtomicU8,
    audit_log_status: AtomicU8,
    log_storage_status: AtomicU8,
    debug_log_event_rejected_count: AtomicU64,
    debug_log_write_failure_count: AtomicU64,
    debug_log_retention_failure_count: AtomicU64,
    task_log_write_failure_count: AtomicU64,
    task_log_retention_failure_count: AtomicU64,
    audit_write_failure_count: AtomicU64,
    audit_write_failure_after_commit_count: AtomicU64,
    audit_log_retention_failure_count: AtomicU64,
    log_storage_failure_count: AtomicU64,
    log_storage_unsatisfied_count: AtomicU64,
    log_storage_settings_failure_count: AtomicU64,
}

impl DiagnosticsEvidenceHealth for DiagnosticsEvidenceHealthState {
    fn snapshot(&self) -> DiagnosticsEvidenceHealthSnapshot {
        DiagnosticsEvidenceHealthSnapshot {
            debug_log_status: match self.debug_log_status.load(Ordering::Acquire) {
                0 => "ok",
                1 => "debug_log_retention_failed",
                2 => "debug_log_event_rejected",
                _ => "debug_log_write_failed",
            }
            .to_owned(),
            task_log_status: match self.task_log_status.load(Ordering::Acquire) {
                0 => "ok",
                1 => "task_log_retention_failed",
                _ => "task_log_write_failed",
            }
            .to_owned(),
            audit_log_status: match self.audit_log_status.load(Ordering::Acquire) {
                0 => "ok",
                1 => "audit_log_retention_failed",
                2 => "audit_write_failed",
                _ => "audit_write_failed_after_commit",
            }
            .to_owned(),
            log_storage_status: match self.log_storage_status.load(Ordering::Acquire) {
                0 => "ok",
                1 => "log_storage_settings_unavailable",
                2 => "log_storage_budget_unsatisfied",
                _ => "log_storage_budget_failed",
            }
            .to_owned(),
            debug_log_event_rejected_count: self
                .debug_log_event_rejected_count
                .load(Ordering::Acquire),
            debug_log_write_failure_count: self
                .debug_log_write_failure_count
                .load(Ordering::Acquire),
            debug_log_retention_failure_count: self
                .debug_log_retention_failure_count
                .load(Ordering::Acquire),
            task_log_write_failure_count: self.task_log_write_failure_count.load(Ordering::Acquire),
            task_log_retention_failure_count: self
                .task_log_retention_failure_count
                .load(Ordering::Acquire),
            audit_write_failure_count: self.audit_write_failure_count.load(Ordering::Acquire),
            audit_write_failure_after_commit_count: self
                .audit_write_failure_after_commit_count
                .load(Ordering::Acquire),
            audit_log_retention_failure_count: self
                .audit_log_retention_failure_count
                .load(Ordering::Acquire),
            log_storage_failure_count: self.log_storage_failure_count.load(Ordering::Acquire),
            log_storage_unsatisfied_count: self
                .log_storage_unsatisfied_count
                .load(Ordering::Acquire),
            log_storage_settings_failure_count: self
                .log_storage_settings_failure_count
                .load(Ordering::Acquire),
        }
    }

    fn record_debug_log_event_rejected(&self) {
        self.debug_log_status.fetch_max(2, Ordering::AcqRel);
        self.debug_log_event_rejected_count
            .fetch_add(1, Ordering::AcqRel);
    }

    fn record_debug_log_write_failure(&self) {
        self.debug_log_status.fetch_max(3, Ordering::AcqRel);
        self.debug_log_write_failure_count
            .fetch_add(1, Ordering::AcqRel);
    }

    fn record_debug_log_retention_failure(&self) {
        self.debug_log_status.fetch_max(1, Ordering::AcqRel);
        self.debug_log_retention_failure_count
            .fetch_add(1, Ordering::AcqRel);
    }

    fn record_task_log_write_failure(&self, _status: &'static str) {
        self.task_log_status.fetch_max(2, Ordering::AcqRel);
        self.task_log_write_failure_count
            .fetch_add(1, Ordering::AcqRel);
    }

    fn record_audit_write_failure(&self, after_commit: bool) {
        self.audit_log_status
            .fetch_max(if after_commit { 3 } else { 2 }, Ordering::AcqRel);
        self.audit_write_failure_count
            .fetch_add(1, Ordering::AcqRel);
        if after_commit {
            self.audit_write_failure_after_commit_count
                .fetch_add(1, Ordering::AcqRel);
        }
    }

    fn record_task_log_retention_failure(&self) {
        self.task_log_status.fetch_max(1, Ordering::AcqRel);
        self.task_log_retention_failure_count
            .fetch_add(1, Ordering::AcqRel);
    }

    fn record_audit_log_retention_failure(&self) {
        self.audit_log_status.fetch_max(1, Ordering::AcqRel);
        self.audit_log_retention_failure_count
            .fetch_add(1, Ordering::AcqRel);
    }

    fn record_log_storage_budget_failure(&self) {
        self.log_storage_status.fetch_max(3, Ordering::AcqRel);
        self.log_storage_failure_count
            .fetch_add(1, Ordering::AcqRel);
    }

    fn record_log_storage_budget_unsatisfied(&self) {
        self.log_storage_status.fetch_max(2, Ordering::AcqRel);
        self.log_storage_unsatisfied_count
            .fetch_add(1, Ordering::AcqRel);
    }

    fn record_log_storage_settings_failure(&self) {
        self.log_storage_status.fetch_max(1, Ordering::AcqRel);
        self.log_storage_settings_failure_count
            .fetch_add(1, Ordering::AcqRel);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_distinguishes_task_audit_and_post_commit_degradation() {
        let health = DiagnosticsEvidenceHealthState::default();
        health.record_debug_log_retention_failure();
        health.record_debug_log_event_rejected();
        health.record_debug_log_write_failure();
        health.record_task_log_retention_failure();
        health.record_audit_log_retention_failure();
        health.record_task_log_write_failure("task_log_write_failed");
        health.record_audit_write_failure(false);
        health.record_audit_write_failure(true);
        health.record_log_storage_settings_failure();
        health.record_log_storage_budget_unsatisfied();
        health.record_log_storage_budget_failure();

        let snapshot = health.snapshot();
        assert_eq!(snapshot.debug_log_status, "debug_log_write_failed");
        assert_eq!(snapshot.debug_log_event_rejected_count, 1);
        assert_eq!(snapshot.debug_log_write_failure_count, 1);
        assert_eq!(snapshot.debug_log_retention_failure_count, 1);
        assert_eq!(snapshot.task_log_status, "task_log_write_failed");
        assert_eq!(snapshot.audit_log_status, "audit_write_failed_after_commit");
        assert_eq!(snapshot.task_log_write_failure_count, 1);
        assert_eq!(snapshot.task_log_retention_failure_count, 1);
        assert_eq!(snapshot.audit_write_failure_count, 2);
        assert_eq!(snapshot.audit_write_failure_after_commit_count, 1);
        assert_eq!(snapshot.audit_log_retention_failure_count, 1);
        assert_eq!(snapshot.log_storage_status, "log_storage_budget_failed");
        assert_eq!(snapshot.log_storage_failure_count, 1);
        assert_eq!(snapshot.log_storage_unsatisfied_count, 1);
        assert_eq!(snapshot.log_storage_settings_failure_count, 1);
    }

    #[test]
    fn retention_failure_never_downgrades_a_more_severe_write_failure() {
        let health = DiagnosticsEvidenceHealthState::default();
        health.record_debug_log_write_failure();
        health.record_debug_log_retention_failure();
        health.record_task_log_write_failure("task_log_write_failed");
        health.record_audit_write_failure(true);
        health.record_task_log_retention_failure();
        health.record_audit_log_retention_failure();
        health.record_log_storage_budget_failure();
        health.record_log_storage_settings_failure();

        let snapshot = health.snapshot();
        assert_eq!(snapshot.debug_log_status, "debug_log_write_failed");
        assert_eq!(snapshot.debug_log_write_failure_count, 1);
        assert_eq!(snapshot.debug_log_retention_failure_count, 1);
        assert_eq!(snapshot.task_log_status, "task_log_write_failed");
        assert_eq!(snapshot.audit_log_status, "audit_write_failed_after_commit");
        assert_eq!(snapshot.task_log_write_failure_count, 1);
        assert_eq!(snapshot.task_log_retention_failure_count, 1);
        assert_eq!(snapshot.audit_write_failure_count, 1);
        assert_eq!(snapshot.audit_write_failure_after_commit_count, 1);
        assert_eq!(snapshot.audit_log_retention_failure_count, 1);
        assert_eq!(snapshot.log_storage_status, "log_storage_budget_failed");
        assert_eq!(snapshot.log_storage_failure_count, 1);
        assert_eq!(snapshot.log_storage_settings_failure_count, 1);
    }
}
