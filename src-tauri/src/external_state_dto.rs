//! #286 外部 MOD 状态扫描的传输 DTO。
//!
//! 状态枚举用 snake_case 序列化——前端 `externalInstallStatusView.ts` 的
//! `ExternalInstallState` / `ExternalFileState` 字面量就是这套值
//! （`"not_installed"` 等），DTO 直接对齐它，不再做一层映射。

use crate::dto::TaskStartedDto;
use hmm_core::{ExternalFileState, ExternalInstallState, ExternalInstallStateSummary, ModId};
use hmm_runtime::{
    ExternalModAdoptTaskLaunch, ExternalStateScanQuery, ExternalStateScanTaskLaunch,
};
use serde::Serialize;
use std::collections::HashMap;

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

/// `start_external_mod_adopt` 的返回：与扫描同形，只有任务身份与 opaque `modId`。
/// 接管的结果没有独立 getter——成功即等于用户确认的预览，前端完成后重查安装状态。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalModAdoptStartedDto {
    pub task: TaskStartedDto,
    pub mod_id: String,
}

impl From<&ExternalModAdoptTaskLaunch> for ExternalModAdoptStartedDto {
    fn from(launch: &ExternalModAdoptTaskLaunch) -> Self {
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
    /// 占用归因（#286 第三层）：比对集内出现过的**其他** MOD 占用者，按文件序
    /// 首现去重。空数组 = 无占用。哈希判定（`state` 与各计数）不因占用而改变。
    pub occupied_by: Vec<ExternalModStateOccupierDto>,
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
    /// 该路径被哪个其他 MOD 的清单条目认领；键缺席 = 无主或归被扫 MOD 自己。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub claimed_by_mod_id: Option<String>,
    /// 占用者显示名（getter 时按 `get_mod_detail` 取名链解析）；
    /// 解析不到（如 MOD 已删）时键缺席，前端回退显示 id。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub claimed_by_mod_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalModStateOccupierDto {
    pub mod_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mod_name: Option<String>,
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

impl ExternalModStateDto {
    /// 组装 getter 返回。
    ///
    /// 刻意**不是** `From`：占用者显示名要在 getter 时解析（拍板：结果只存
    /// mod_id，名字随查随取才不会陈旧），必须注入解析器。`resolve_name` 对
    /// **去重后的**每个占用者只调用一次；返回 `None`（如 MOD 已删）时名字键
    /// 缺席，前端回退显示 id。
    pub fn from_query(
        query: ExternalStateScanQuery,
        resolve_name: impl Fn(&str) -> Option<String>,
    ) -> Self {
        let ExternalStateScanQuery {
            summary,
            stale,
            last_error,
            display_paths,
            claimed_by,
        } = query;
        let summary =
            summary.map(|summary| summary_dto(summary, display_paths, claimed_by, resolve_name));
        Self {
            summary,
            stale,
            last_error: last_error.map(|error| error.code().to_owned()),
        }
    }
}

fn summary_dto(
    summary: ExternalInstallStateSummary,
    display_paths: Vec<String>,
    claimed_by: Vec<Option<ModId>>,
    resolve_name: impl Fn(&str) -> Option<String>,
) -> ExternalModStateSummaryDto {
    // 名字按占用者去重后各解析一次；多文件同占用者不重复查询。
    let mut resolved_names: HashMap<String, Option<String>> = HashMap::new();
    let mut occupied_by: Vec<ExternalModStateOccupierDto> = Vec::new();
    // `display_paths`、`claimed_by` 与 `files` 同源同序（runtime 的存储不变量）。
    // zip 与迭代器耗尽都是防御：长度万一不一致，短的一侧截断/补 None，
    // 绝不凭空造路径或占用。
    let mut claims = claimed_by.into_iter();
    let files = summary
        .files
        .iter()
        .zip(display_paths)
        .map(|(state, target_path)| {
            let claimant = claims.next().flatten();
            let (claimed_by_mod_id, claimed_by_mod_name) = match claimant {
                Some(mod_id) => {
                    let id = mod_id.as_str().to_owned();
                    let name = resolved_names
                        .entry(id.clone())
                        .or_insert_with(|| resolve_name(&id))
                        .clone();
                    if !occupied_by.iter().any(|occupier| occupier.mod_id == id) {
                        occupied_by.push(ExternalModStateOccupierDto {
                            mod_id: id.clone(),
                            mod_name: name.clone(),
                        });
                    }
                    (Some(id), name)
                }
                None => (None, None),
            };
            ExternalModStateFileDto {
                target_path,
                state: (*state).into(),
                claimed_by_mod_id,
                claimed_by_mod_name,
            }
        })
        .collect();
    ExternalModStateSummaryDto {
        state: summary.state.into(),
        matched_file_count: summary.matched_file_count,
        missing_file_count: summary.missing_file_count,
        changed_file_count: summary.changed_file_count,
        unreadable_file_count: summary.unreadable_file_count,
        occupied_by,
        files,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    fn summary_of(files: Vec<ExternalFileState>) -> ExternalInstallStateSummary {
        ExternalInstallStateSummary {
            state: ExternalInstallState::Installed,
            matched_file_count: files.len(),
            missing_file_count: 0,
            changed_file_count: 0,
            unreadable_file_count: 0,
            files,
        }
    }

    fn query_with_claims(claimed_by: Vec<Option<ModId>>) -> ExternalStateScanQuery {
        ExternalStateScanQuery {
            summary: Some(summary_of(vec![
                ExternalFileState::Matched,
                ExternalFileState::Matched,
                ExternalFileState::Matched,
            ])),
            stale: false,
            last_error: None,
            display_paths: vec![
                "nativePC/a.mod3".to_owned(),
                "nativePC/b.mod3".to_owned(),
                "nativePC/c.mod3".to_owned(),
            ],
            claimed_by,
        }
    }

    #[test]
    fn claims_serialize_with_resolved_names_and_deduped_occupiers() {
        let calls = RefCell::new(Vec::<String>::new());
        // 前两个文件同一占用者：验证去重与「解析器只按占用者调一次」。
        let dto = ExternalModStateDto::from_query(
            query_with_claims(vec![
                Some(ModId::new("mod-flat")),
                Some(ModId::new("mod-flat")),
                None,
            ]),
            |claimant_id| {
                calls.borrow_mut().push(claimant_id.to_owned());
                Some("Flat 武器".to_owned())
            },
        );

        let value = serde_json::to_value(&dto).expect("serialize");
        let files = value["summary"]["files"].as_array().expect("files array");
        assert_eq!(files[0]["claimedByModId"], "mod-flat");
        assert_eq!(files[0]["claimedByModName"], "Flat 武器");
        assert_eq!(files[1]["claimedByModId"], "mod-flat");
        // 无占用文件：两个键都必须缺席，而不是 null。
        assert!(!files[2]
            .as_object()
            .expect("file object")
            .contains_key("claimedByModId"));
        assert!(!files[2]
            .as_object()
            .expect("file object")
            .contains_key("claimedByModName"));

        let occupied = value["summary"]["occupiedBy"]
            .as_array()
            .expect("occupiedBy array");
        assert_eq!(occupied.len(), 1, "同一占用者只报一次");
        assert_eq!(occupied[0]["modId"], "mod-flat");
        assert_eq!(occupied[0]["modName"], "Flat 武器");

        assert_eq!(
            calls.borrow().as_slice(),
            ["mod-flat"],
            "解析器只按占用者调一次"
        );
    }

    #[test]
    fn a_deleted_occupier_keeps_the_id_and_omits_the_name() {
        let dto = ExternalModStateDto::from_query(
            query_with_claims(vec![Some(ModId::new("mod-gone")), None, None]),
            |_| None,
        );

        let value = serde_json::to_value(&dto).expect("serialize");
        let files = value["summary"]["files"].as_array().expect("files array");
        assert_eq!(files[0]["claimedByModId"], "mod-gone");
        assert!(!files[0]
            .as_object()
            .expect("file object")
            .contains_key("claimedByModName"));

        let occupied = value["summary"]["occupiedBy"]
            .as_array()
            .expect("occupiedBy array");
        assert_eq!(occupied[0]["modId"], "mod-gone");
        assert!(!occupied[0]
            .as_object()
            .expect("occupier object")
            .contains_key("modName"));
    }

    #[test]
    fn no_claims_produce_an_empty_occupier_list_and_clean_files() {
        let dto =
            ExternalModStateDto::from_query(query_with_claims(vec![None, None, None]), |_| {
                unreachable!("无占用不得调用解析器")
            });

        let value = serde_json::to_value(&dto).expect("serialize");
        assert_eq!(
            value["summary"]["occupiedBy"],
            serde_json::json!([]),
            "occupiedBy 必须是恒在的空数组，不是缺席键",
        );
        for file in value["summary"]["files"].as_array().expect("files array") {
            assert!(!file
                .as_object()
                .expect("file object")
                .contains_key("claimedByModId"));
        }
    }
}
