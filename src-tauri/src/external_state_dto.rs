//! #286 外部 MOD 状态扫描的传输 DTO。
//!
//! 状态枚举用 snake_case 序列化——前端 `externalInstallStatusView.ts` 的
//! `ExternalInstallState` / `ExternalFileState` 字面量就是这套值
//! （`"not_installed"` 等），DTO 直接对齐它，不再做一层映射。

use crate::dto::TaskStartedDto;
use hmm_core::{ExternalFileState, ExternalInstallState, ExternalInstallStateSummary};
use hmm_runtime::{ExternalStateScanQuery, ExternalStateScanTaskLaunch};
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalStateScanStartedDto {
    pub task: TaskStartedDto,
    pub mod_id: String,
}

impl From<&ExternalStateScanTaskLaunch> for ExternalStateScanStartedDto {
    fn from(launch: &ExternalStateScanTaskLaunch) -> Self {
        Self {
            task: launch.task.clone().into(),
            mod_id: launch.mod_id.as_str().to_owned(),
        }
    }
}

/// `get_external_mod_state` 的返回。
///
/// `summary` 与 `lastError` 可以同时存在：上次扫成功过、之后又扫失败了一次。
/// 二者语义正交（见 runtime `ExternalStateScanQuery` 的说明），前端不要合并展示。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalModStateDto {
    pub summary: Option<ExternalModStateSummaryDto>,
    pub stale: bool,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalModStateSummaryDto {
    pub state: ExternalInstallStateDto,
    pub matched_file_count: usize,
    pub missing_file_count: usize,
    pub changed_file_count: usize,
    pub unreadable_file_count: usize,
    /// 文件级明细。路径是导入包里的原始展示字符串（相对路径），
    /// **不是**游戏目录的绝对路径——getter 允许携带相对目标路径，
    /// 但绝对本地路径仍然不出后端。
    pub files: Vec<ExternalModStateFileDto>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalModStateFileDto {
    pub target_path: String,
    pub state: ExternalFileStateDto,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ExternalInstallStateDto {
    Installed,
    Partial,
    Changed,
    Mixed,
    NotInstalled,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ExternalFileStateDto {
    Matched,
    Missing,
    Changed,
    Unreadable,
}

impl From<ExternalInstallState> for ExternalInstallStateDto {
    fn from(state: ExternalInstallState) -> Self {
        match state {
            ExternalInstallState::Installed => Self::Installed,
            ExternalInstallState::Partial => Self::Partial,
            ExternalInstallState::Changed => Self::Changed,
            ExternalInstallState::Mixed => Self::Mixed,
            ExternalInstallState::NotInstalled => Self::NotInstalled,
            ExternalInstallState::Unknown => Self::Unknown,
        }
    }
}

impl From<ExternalFileState> for ExternalFileStateDto {
    fn from(state: ExternalFileState) -> Self {
        match state {
            ExternalFileState::Matched => Self::Matched,
            ExternalFileState::Missing => Self::Missing,
            ExternalFileState::Changed => Self::Changed,
            ExternalFileState::Unreadable => Self::Unreadable,
        }
    }
}

impl From<ExternalStateScanQuery> for ExternalModStateDto {
    fn from(query: ExternalStateScanQuery) -> Self {
        let summary = query
            .summary
            .map(|summary| summary_dto(summary, query.display_paths));
        Self {
            summary,
            stale: query.stale,
            last_error: query.last_error.map(|error| error.code().to_owned()),
        }
    }
}

fn summary_dto(
    summary: ExternalInstallStateSummary,
    display_paths: Vec<String>,
) -> ExternalModStateSummaryDto {
    // `display_paths` 与 `files` 同源同序（runtime 的存储不变量）。zip 是防御：
    // 万一长度不一致，短的一侧截断，绝不凭空造路径。
    let files = summary
        .files
        .iter()
        .zip(display_paths)
        .map(|(state, target_path)| ExternalModStateFileDto {
            target_path,
            state: (*state).into(),
        })
        .collect();
    ExternalModStateSummaryDto {
        state: summary.state.into(),
        matched_file_count: summary.matched_file_count,
        missing_file_count: summary.missing_file_count,
        changed_file_count: summary.changed_file_count,
        unreadable_file_count: summary.unreadable_file_count,
        files,
    }
}
