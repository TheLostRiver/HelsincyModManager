//! 包内容树的只读查询命令（`#354` 切片 D1）。
//!
//! 只读、不写盘、不建计划。它存在的意义是让界面**先看得见整包**——玩家要挑内容根（D2）、
//! 挑装哪些文件（D3/D4），前提是知道包里有什么。
//!
//! 返回的是**扁平清单**而不是嵌套树：扁平更好序列化、更好 diff，扫描侧本来也是排序过的扁平
//! 结构，建树交给前端。实测语料库里最大的包有 7340 个文件、深度 10，嵌套结构在这个量级上
//! 只会让传输与比对都更贵。

use crate::dto::CommandErrorDto;
use crate::state::AppState;
use hmm_app::{
    PackageContentRoot, PackageContents, PackageContentsQueryError, PackageContentsQueryRequest,
};
use hmm_core::{GameId, ModId};
use serde::{Deserialize, Serialize};
use tauri::State;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageContentsRequestDto {
    pub game_id: Option<String>,
    pub mod_id: Option<String>,
}

/// 内容根解析结果。
///
/// `kind` 三档与后端枚举一一对应，`candidates` 只在 `ambiguous` 时非空：多个 `nativePC` 是
/// **要玩家决定**的状态，不是失败，所以候选如实带出来而不是报错。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageContentRootDto {
    pub kind: &'static str,
    /// 沙箱根相对路径；空串表示沙箱根本身。`ambiguous` 时为 `None`。
    pub path: Option<String>,
    pub candidates: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageContentEntryDto {
    pub package_file_id: String,
    pub size_bytes: u64,
    /// 相对内容根的安装路径；不在内容根之下、或内容根未定时为 `None`。
    pub target_path: Option<String>,
    pub installable: bool,
    /// 命中本游戏的「绝不安装」清单。
    ///
    /// 这是**事实**不是结论：拒绝清单当前只在重定向链路上被强制执行，普通安装链路尚未套用。
    /// UI 因此不能把它直接渲染成「不会被安装」，理由见 `hmm-app` 的
    /// `package_contents_query` 模块头。
    pub rejected_by_game: bool,
    /// 玩家把这个文件勾掉了（`#354` 切片 D3）。
    ///
    /// 与 `installable` / `rejectedByGame` 分开：那两条是「本游戏允许不允许」，这一条是
    /// 「玩家要不要」。合并就说不清「它为什么不装」。
    pub excluded_by_player: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageContentsDto {
    pub content_root: PackageContentRootDto,
    /// 允许被选作内容根的全部目录，与 `contentRoot` 当前是哪个无关。
    ///
    /// 与 `contentRoot` 分开的理由：玩家选定之后 `contentRoot` 会收敛成 `single`，候选若
    /// 跟着消失他就**改不了主意**。这份清单同时是 `set_mod_package_content_root` 的白名单。
    pub candidates: Vec<String>,
    /// 玩家勾掉的 `packageFileId`。整包仍逐条列在 `entries` 里——勾掉不等于看不见。
    pub excluded_files: Vec<String>,
    pub entries: Vec<PackageContentEntryDto>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetPackageFileSelectionRequestDto {
    pub game_id: Option<String>,
    pub mod_id: Option<String>,
    /// 要**排除**的 `packageFileId` 清单。空清单 = 整包都装。
    pub excluded_files: Option<Vec<String>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetPackageContentRootRequestDto {
    pub game_id: Option<String>,
    pub mod_id: Option<String>,
    /// 沙箱根相对路径；空串表示沙箱根本身。必须是 `candidates` 里的一项。
    pub content_root: Option<String>,
}

#[tauri::command]
pub fn get_mod_package_contents(
    request: PackageContentsRequestDto,
    state: State<'_, AppState>,
) -> Result<PackageContentsDto, CommandErrorDto> {
    let request = package_contents_request_from_dto(request)?;
    state
        .package_contents_query
        .query(request)
        .map(Into::into)
        .map_err(package_contents_error_to_command_error)
}

/// 记下玩家为这个包选定的内容根（`#354` 切片 D2）。
///
/// 选择**按包持久化**：提交安装时会从沙箱重建计划，重装同理，选择若只活在预览里，重建那
/// 一刻就没了。设置时即校验白名单，不接受任意路径。
#[tauri::command]
pub fn set_mod_package_content_root(
    request: SetPackageContentRootRequestDto,
    state: State<'_, AppState>,
) -> Result<PackageContentsDto, CommandErrorDto> {
    // `content_root` 允许是空串（沙箱根本身），所以不能走 `required_id`——那会把
    // 「显式选了沙箱根」误判成「没传值」。
    let content_root = request.content_root.unwrap_or_default();
    let query = package_contents_request_from_dto(PackageContentsRequestDto {
        game_id: request.game_id,
        mod_id: request.mod_id,
    })?;

    state
        .package_contents_query
        .choose_content_root(query.clone(), &content_root)
        .map_err(package_contents_error_to_command_error)?;
    // 回读一次：前端拿到的是**设置生效之后**的分档结果，不用自己推演。
    state
        .package_contents_query
        .query(query)
        .map(Into::into)
        .map_err(package_contents_error_to_command_error)
}

/// 撤销选择，回到自动解析。合集包会重新变成「等玩家决定」。
#[tauri::command]
pub fn clear_mod_package_content_root(
    request: PackageContentsRequestDto,
    state: State<'_, AppState>,
) -> Result<PackageContentsDto, CommandErrorDto> {
    let request = package_contents_request_from_dto(request)?;
    state
        .package_contents_query
        .clear_content_root(request.clone())
        .map_err(package_contents_error_to_command_error)?;
    state
        .package_contents_query
        .query(request)
        .map(Into::into)
        .map_err(package_contents_error_to_command_error)
}

fn package_contents_request_from_dto(
    request: PackageContentsRequestDto,
) -> Result<PackageContentsQueryRequest, CommandErrorDto> {
    Ok(PackageContentsQueryRequest {
        game_id: parse_game_id(request.game_id)?,
        mod_id: ModId::new(required_id(
            request.mod_id,
            "package_contents_mod_id_invalid",
            "Mod id is required",
        )?),
    })
}

fn parse_game_id(value: Option<String>) -> Result<GameId, CommandErrorDto> {
    let value = required_id(
        value,
        "package_contents_game_id_invalid",
        "game id is required",
    )?;
    GameId::parse(value).map_err(|_| CommandErrorDto {
        code: "package_contents_game_id_invalid".to_owned(),
        message: "game id is invalid".to_owned(),
    })
}

fn required_id(
    value: Option<String>,
    code: &str,
    message: &str,
) -> Result<String, CommandErrorDto> {
    value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| CommandErrorDto {
            code: code.to_owned(),
            message: message.to_owned(),
        })
}

fn package_contents_error_to_command_error(error: PackageContentsQueryError) -> CommandErrorDto {
    // 码由 `PackageContentsQueryError::code` 统一给出：扫描类失败会原样透出扫描侧已有的
    // 稳定码（符号链接、深度上限等），不在命令层重新发明一套。
    CommandErrorDto {
        code: error.code().to_owned(),
        message: error.to_string(),
    }
}

impl From<PackageContents> for PackageContentsDto {
    fn from(value: PackageContents) -> Self {
        Self {
            content_root: value.content_root.into(),
            candidates: value.candidates,
            excluded_files: value.excluded_files,
            entries: value
                .entries
                .into_iter()
                .map(|entry| PackageContentEntryDto {
                    package_file_id: entry.package_file_id,
                    size_bytes: entry.size_bytes,
                    target_path: entry.target_path,
                    installable: entry.installable,
                    rejected_by_game: entry.rejected_by_game,
                    excluded_by_player: entry.excluded_by_player,
                })
                .collect(),
        }
    }
}

impl From<PackageContentRoot> for PackageContentRootDto {
    fn from(value: PackageContentRoot) -> Self {
        match value {
            PackageContentRoot::Fallback => Self {
                kind: "fallback",
                path: Some(String::new()),
                candidates: Vec::new(),
            },
            PackageContentRoot::Single(path) => Self {
                kind: "single",
                path: Some(path),
                candidates: Vec::new(),
            },
            PackageContentRoot::Ambiguous(candidates) => Self {
                kind: "ambiguous",
                path: None,
                candidates,
            },
        }
    }
}

/// 记下玩家在这个包里勾掉的文件（`#354` 切片 D3）。
///
/// 传的是**要排除的**清单，不是要保留的：空清单 = 整包都装 = 计划逐字不变。用「保留清单」
/// 的话，包重新解压出的新文件会**静默不装**，而少装一个文件装完不报错。
#[tauri::command]
pub fn set_mod_package_file_selection(
    request: SetPackageFileSelectionRequestDto,
    state: State<'_, AppState>,
) -> Result<PackageContentsDto, CommandErrorDto> {
    let excluded = request.excluded_files.unwrap_or_default();
    let query = package_contents_request_from_dto(PackageContentsRequestDto {
        game_id: request.game_id,
        mod_id: request.mod_id,
    })?;

    state
        .package_contents_query
        .exclude_files(query.clone(), &excluded)
        .map_err(package_contents_error_to_command_error)?;
    state
        .package_contents_query
        .query(query)
        .map(Into::into)
        .map_err(package_contents_error_to_command_error)
}

/// 撤销全部勾选，回到「整包都装」。
#[tauri::command]
pub fn clear_mod_package_file_selection(
    request: PackageContentsRequestDto,
    state: State<'_, AppState>,
) -> Result<PackageContentsDto, CommandErrorDto> {
    let request = package_contents_request_from_dto(request)?;
    state
        .package_contents_query
        .clear_excluded_files(request.clone())
        .map_err(package_contents_error_to_command_error)?;
    state
        .package_contents_query
        .query(request)
        .map(Into::into)
        .map_err(package_contents_error_to_command_error)
}

#[cfg(test)]
#[path = "package_contents_commands_tests.rs"]
mod package_contents_commands_tests;
