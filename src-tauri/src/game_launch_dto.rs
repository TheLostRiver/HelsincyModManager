use crate::dto::CommandErrorDto;
use hmm_app::GameLaunchServiceError;
use hmm_ports::{GameLaunchError, GameLaunchMethod, GameLaunchReceipt};
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameLaunchReceiptDto {
    pub game_id: String,
    pub method: GameLaunchMethodDto,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GameLaunchMethodDto {
    SteamProtocol,
    DirectExecutable,
}

impl CommandErrorDto {
    pub fn from_game_launch_service_error(error: GameLaunchServiceError) -> Self {
        let (code, message) = match error {
            GameLaunchServiceError::UnsupportedGame => {
                ("unsupported_game", "当前版本暂不支持启动该游戏。")
            }
            GameLaunchServiceError::GameNotConfigured => {
                ("game_not_configured", "请先配置游戏目录，再启动游戏。")
            }
            GameLaunchServiceError::StorageCorrupted => (
                "storage_corrupted",
                "游戏配置文件已损坏，无法读取启动配置。",
            ),
            GameLaunchServiceError::StorageFailed(_) => (
                "storage_failed",
                "游戏配置读取失败，请检查应用数据目录权限。",
            ),
            GameLaunchServiceError::LaunchFailed(GameLaunchError::LauncherUnavailable(_)) => {
                ("launcher_unavailable", "系统未能打开游戏启动器。")
            }
            GameLaunchServiceError::LaunchFailed(GameLaunchError::LaunchFailed(_)) => {
                ("launch_failed", "启动请求发送失败，请稍后重试。")
            }
        };

        Self {
            code: code.to_owned(),
            message: message.to_owned(),
        }
    }
}

impl From<GameLaunchReceipt> for GameLaunchReceiptDto {
    fn from(receipt: GameLaunchReceipt) -> Self {
        Self {
            game_id: receipt.game_id.as_str().to_owned(),
            method: receipt.method.into(),
        }
    }
}

impl From<GameLaunchMethod> for GameLaunchMethodDto {
    fn from(method: GameLaunchMethod) -> Self {
        match method {
            GameLaunchMethod::SteamProtocol => Self::SteamProtocol,
            GameLaunchMethod::DirectExecutable => Self::DirectExecutable,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_game_launch_receipt_without_paths_or_uris() {
        let dto: GameLaunchReceiptDto = GameLaunchReceipt {
            game_id: hmm_core::GameId::mhw(),
            method: GameLaunchMethod::SteamProtocol,
        }
        .into();

        let value = serde_json::to_value(dto).expect("serialize launch receipt");

        assert_eq!(value["gameId"], "mhw");
        assert_eq!(value["method"], "steam_protocol");
        assert!(value.get("path").is_none());
        assert!(value.get("uri").is_none());
        assert!(!value.to_string().contains("steam://"));
    }

    #[test]
    fn maps_missing_game_setup_to_stable_launch_error_code() {
        let error = CommandErrorDto::from_game_launch_service_error(
            GameLaunchServiceError::GameNotConfigured,
        );

        assert_eq!(error.code, "game_not_configured");
        assert!(!error.message.contains("steam://"));
    }
}
