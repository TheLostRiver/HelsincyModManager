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

pub trait AuditLogWriter: Send + Sync {
    fn record(&self, event: AuditLogEvent) -> Result<()>;
}
