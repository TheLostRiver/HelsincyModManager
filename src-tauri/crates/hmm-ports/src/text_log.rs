use anyhow::Result;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextLogKind {
    App,
    Debug,
    Task,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextLogReadRequest {
    pub kind: TextLogKind,
    pub max_lines: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TextLogLine {
    pub source: String,
    pub line: String,
}

pub trait TextLogReader: Send + Sync {
    fn read_recent_sanitized(&self, request: TextLogReadRequest) -> Result<Vec<TextLogLine>>;
}
