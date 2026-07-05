use hmm_ports::{
    GamePrerequisiteIssue, GamePrerequisiteIssueCode, GamePrerequisiteItem,
    GamePrerequisiteItemStatus, GamePrerequisiteReport, GamePrerequisiteReportState,
    GamePrerequisiteSummaryStatus,
};
use serde::Serialize;

use super::error_code_to_string;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GamePrerequisiteReportDto {
    pub game_id: String,
    pub state: String,
    pub summary_status: Option<String>,
    pub items: Vec<GamePrerequisiteItemDto>,
    pub error_code: Option<String>,
    pub message: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GamePrerequisiteItemDto {
    pub id: String,
    pub display_name: String,
    pub status: String,
    pub issues: Vec<GamePrerequisiteIssueDto>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GamePrerequisiteIssueDto {
    pub code: String,
    pub path: String,
}

pub fn prerequisite_report_to_dto(report: GamePrerequisiteReport) -> GamePrerequisiteReportDto {
    GamePrerequisiteReportDto {
        game_id: report.game_id.as_str().to_owned(),
        state: prerequisite_report_state_to_string(report.state),
        summary_status: report
            .summary_status
            .map(prerequisite_summary_status_to_string),
        items: report
            .items
            .into_iter()
            .map(prerequisite_item_to_dto)
            .collect(),
        error_code: report.error_code.map(error_code_to_string),
        message: report.message,
    }
}

fn prerequisite_report_state_to_string(state: GamePrerequisiteReportState) -> String {
    match state {
        GamePrerequisiteReportState::NotConfigured => "not_configured",
        GamePrerequisiteReportState::GameDirectoryInvalid => "game_directory_invalid",
        GamePrerequisiteReportState::RulesUnavailable => "rules_unavailable",
        GamePrerequisiteReportState::Ready => "ready",
    }
    .to_owned()
}

fn prerequisite_summary_status_to_string(status: GamePrerequisiteSummaryStatus) -> String {
    match status {
        GamePrerequisiteSummaryStatus::Verified => "verified",
        GamePrerequisiteSummaryStatus::Warning => "warning",
        GamePrerequisiteSummaryStatus::Error => "error",
    }
    .to_owned()
}

fn prerequisite_item_to_dto(item: GamePrerequisiteItem) -> GamePrerequisiteItemDto {
    GamePrerequisiteItemDto {
        id: item.id,
        display_name: item.display_name,
        status: prerequisite_item_status_to_string(item.status),
        issues: item
            .issues
            .into_iter()
            .map(prerequisite_issue_to_dto)
            .collect(),
    }
}

fn prerequisite_item_status_to_string(status: GamePrerequisiteItemStatus) -> String {
    match status {
        GamePrerequisiteItemStatus::Missing => "missing",
        GamePrerequisiteItemStatus::Misconfigured => "misconfigured",
        GamePrerequisiteItemStatus::InstalledVerified => "installed_verified",
        GamePrerequisiteItemStatus::InstalledUnverified => "installed_unverified",
    }
    .to_owned()
}

fn prerequisite_issue_to_dto(issue: GamePrerequisiteIssue) -> GamePrerequisiteIssueDto {
    GamePrerequisiteIssueDto {
        code: prerequisite_issue_code_to_string(issue.code),
        path: issue.path,
    }
}

fn prerequisite_issue_code_to_string(code: GamePrerequisiteIssueCode) -> String {
    match code {
        GamePrerequisiteIssueCode::MissingRequiredFile => "missing_required_file",
        GamePrerequisiteIssueCode::SignatureUnverified => "signature_unverified",
        GamePrerequisiteIssueCode::ConfigReadFailed => "config_read_failed",
        GamePrerequisiteIssueCode::ConfigInvalidJson => "config_invalid_json",
        GamePrerequisiteIssueCode::ConfigFieldMismatch => "config_field_mismatch",
        GamePrerequisiteIssueCode::RulesUnavailable => "rules_unavailable",
        GamePrerequisiteIssueCode::RulesCorrupted => "rules_corrupted",
    }
    .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prerequisite_report_uses_redacted_relative_paths_in_dto() {
        let dto = prerequisite_report_to_dto(GamePrerequisiteReport::ready(
            hmm_core::GameId::mhw(),
            GamePrerequisiteSummaryStatus::Warning,
            vec![GamePrerequisiteItem {
                id: "crc_bypass".to_owned(),
                display_name: "CRCBypass".to_owned(),
                status: GamePrerequisiteItemStatus::InstalledUnverified,
                issues: vec![GamePrerequisiteIssue::new(
                    GamePrerequisiteIssueCode::SignatureUnverified,
                    "nativePC/plugins/!CRCBypass.dll",
                )],
            }],
        ));

        let value = serde_json::to_value(dto).expect("serialize dto");

        assert_eq!(value["state"], "ready");
        assert_eq!(value["summaryStatus"], "warning");
        assert_eq!(
            value["items"][0]["issues"][0]["path"],
            "nativePC/plugins/!CRCBypass.dll"
        );
        assert!(value.to_string().contains("CRCBypass"));
        assert!(!value.to_string().contains("D:\\\\"));
    }
}
