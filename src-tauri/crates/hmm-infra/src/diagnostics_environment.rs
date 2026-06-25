use anyhow::Result;
use hmm_ports::{DiagnosticsEnvironmentProvider, DiagnosticsEnvironmentSummary};

pub struct SystemDiagnosticsEnvironmentProvider {
    app_version: String,
    game_adapter_ids: Vec<String>,
}

impl SystemDiagnosticsEnvironmentProvider {
    pub fn new(app_version: String, game_adapter_ids: Vec<String>) -> Self {
        Self {
            app_version,
            game_adapter_ids,
        }
    }
}

impl DiagnosticsEnvironmentProvider for SystemDiagnosticsEnvironmentProvider {
    fn summarize(&self) -> Result<DiagnosticsEnvironmentSummary> {
        validate_safe_summary_value(&self.app_version)?;
        for game_adapter_id in &self.game_adapter_ids {
            validate_game_adapter_id(game_adapter_id)?;
        }

        Ok(DiagnosticsEnvironmentSummary {
            app_version: self.app_version.clone(),
            os: std::env::consts::OS.to_owned(),
            arch: std::env::consts::ARCH.to_owned(),
            game_adapter_ids: self.game_adapter_ids.clone(),
        })
    }
}

fn validate_game_adapter_id(game_adapter_id: &str) -> Result<()> {
    if game_adapter_id.is_empty()
        || !game_adapter_id.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-' || byte == b'_'
        })
    {
        anyhow::bail!("diagnostics environment contains invalid game adapter id");
    }

    validate_safe_summary_value(game_adapter_id)
}

fn validate_safe_summary_value(value: &str) -> Result<()> {
    if value.is_empty() || value.chars().any(char::is_control) {
        anyhow::bail!("diagnostics environment contains invalid value");
    }

    let lower = value.to_ascii_lowercase();
    const FORBIDDEN_SNIPPETS: &[&str] = &[
        "token",
        "cookie",
        "api_key",
        "raw_path",
        "c:/",
        "c:\\",
        "\\users\\",
        "/users/",
    ];
    if FORBIDDEN_SNIPPETS
        .iter()
        .any(|snippet| lower.contains(snippet))
    {
        anyhow::bail!("diagnostics environment contains sensitive value");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use hmm_ports::DiagnosticsEnvironmentProvider;

    #[test]
    fn system_diagnostics_environment_provider_returns_bounded_platform_summary() {
        let provider = SystemDiagnosticsEnvironmentProvider::new(
            "0.1.0-alpha.0".to_owned(),
            vec!["mhw".to_owned()],
        );

        let summary = provider
            .summarize()
            .expect("diagnostics environment summary");

        assert_eq!(summary.app_version, "0.1.0-alpha.0");
        assert!(!summary.os.is_empty());
        assert!(!summary.arch.is_empty());
        assert_eq!(summary.game_adapter_ids, vec!["mhw"]);
        let serialized = serde_json::to_string(&summary).expect("serialize summary");
        assert!(!serialized.contains("C:/"));
        assert!(!serialized.contains("\\Users\\"));
        assert!(!serialized.contains("/Users/"));
        assert!(!serialized.contains("token"));
        assert!(!serialized.contains("cookie"));
    }
}
