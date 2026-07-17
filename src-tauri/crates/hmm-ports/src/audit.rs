use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditLogEvent {
    pub timestamp_unix_millis: u128,
    pub category: String,
    pub operation: String,
    pub result: String,
    pub fields: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AuditLogReadRequest {
    pub max_events: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuditWriteFailurePolicy {
    BestEffort,
    ReportAfterCommit,
}

impl AuditWriteFailurePolicy {
    pub fn for_commit_result(result: &str) -> Self {
        if result == "success" {
            Self::ReportAfterCommit
        } else {
            Self::BestEffort
        }
    }
}

pub trait AuditLogWriter: Send + Sync {
    fn record(&self, event: AuditLogEvent) -> Result<()>;

    fn record_with_policy(
        &self,
        event: AuditLogEvent,
        _policy: AuditWriteFailurePolicy,
    ) -> Result<()> {
        self.record(event)
    }
}

pub trait AuditLogReader: Send + Sync {
    fn read_recent_sanitized(&self, request: AuditLogReadRequest) -> Result<Vec<AuditLogEvent>>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commit_result_policy_only_treats_exact_success_as_post_commit() {
        assert_eq!(
            AuditWriteFailurePolicy::for_commit_result("success"),
            AuditWriteFailurePolicy::ReportAfterCommit
        );
        for result in ["failure", "warning", "deferred", "Success", ""] {
            assert_eq!(
                AuditWriteFailurePolicy::for_commit_result(result),
                AuditWriteFailurePolicy::BestEffort
            );
        }
    }
}
