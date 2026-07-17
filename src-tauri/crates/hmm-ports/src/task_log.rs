use crate::PortResult;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskLogRecord {
    pub timestamp_unix_millis: u128,
    pub task_id: String,
    pub kind: String,
    pub status: String,
    pub phase: String,
    pub current: Option<u64>,
    pub total: Option<u64>,
    pub duration_ms: Option<u64>,
    pub error_code: Option<String>,
}

pub trait TaskLogWriter: Send + Sync {
    fn record(&self, record: TaskLogRecord) -> PortResult<()>;
}
