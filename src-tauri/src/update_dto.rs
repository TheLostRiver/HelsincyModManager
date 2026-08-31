use hmm_core::UpdateDecision;
use serde::Serialize;

/// 「检查更新」的结果。
///
/// **这个 DTO 不会失败**：查不到就是 `unknown`，没有 `CommandErrorDto` 分支。
/// 断网、超时、接口变动都是普通用户会遇到的常态，不该有错误弹窗。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppUpdateStatusDto {
    /// 稳定状态值：`up_to_date` / `update_available` / `unknown`。
    pub status: &'static str,
    /// 当前运行的版本号（`cargo` 包版本，如 `0.1.0-alpha.0`）。
    pub current_version: String,
    /// 可用更新的版本号；仅在 `status == "update_available"` 时有值。
    /// 这里放的是**发布标签原文**（可能带 `v` 前缀），界面原样展示。
    pub latest_version: Option<String>,
}

pub const UPDATE_STATUS_UP_TO_DATE: &str = "up_to_date";
pub const UPDATE_STATUS_UPDATE_AVAILABLE: &str = "update_available";
pub const UPDATE_STATUS_UNKNOWN: &str = "unknown";

impl AppUpdateStatusDto {
    pub fn from_decision(current_version: String, decision: UpdateDecision) -> Self {
        match decision {
            UpdateDecision::UpToDate => Self {
                status: UPDATE_STATUS_UP_TO_DATE,
                current_version,
                latest_version: None,
            },
            UpdateDecision::UpdateAvailable { version } => Self {
                status: UPDATE_STATUS_UPDATE_AVAILABLE,
                current_version,
                latest_version: Some(version),
            },
            UpdateDecision::Unknown => Self {
                status: UPDATE_STATUS_UNKNOWN,
                current_version,
                latest_version: None,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_snake_case_status_and_camel_case_fields() {
        let dto = AppUpdateStatusDto::from_decision(
            "0.1.0-alpha.0".to_owned(),
            UpdateDecision::UpdateAvailable {
                version: "v0.2.0".to_owned(),
            },
        );

        let value = serde_json::to_value(&dto).expect("serialize update status");

        assert_eq!(value["status"], "update_available");
        assert_eq!(value["currentVersion"], "0.1.0-alpha.0");
        assert_eq!(value["latestVersion"], "v0.2.0");
    }

    #[test]
    fn only_update_available_carries_a_latest_version() {
        let dto = AppUpdateStatusDto::from_decision("0.1.0".to_owned(), UpdateDecision::UpToDate);
        assert_eq!(dto.status, UPDATE_STATUS_UP_TO_DATE);
        assert_eq!(dto.latest_version, None);

        let dto = AppUpdateStatusDto::from_decision("0.1.0".to_owned(), UpdateDecision::Unknown);
        assert_eq!(dto.status, UPDATE_STATUS_UNKNOWN);
        assert_eq!(dto.latest_version, None);
    }
}
