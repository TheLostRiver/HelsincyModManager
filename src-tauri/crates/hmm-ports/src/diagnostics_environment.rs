use anyhow::Result;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticsEnvironmentSummary {
    pub app_version: String,
    pub os: String,
    pub arch: String,
    pub game_adapter_ids: Vec<String>,
}

pub trait DiagnosticsEnvironmentProvider: Send + Sync {
    fn summarize(&self) -> Result<DiagnosticsEnvironmentSummary>;
}
