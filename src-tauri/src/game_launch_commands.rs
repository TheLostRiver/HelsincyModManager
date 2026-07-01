use crate::dto::CommandErrorDto;
use crate::game_launch_dto::GameLaunchReceiptDto;
use crate::state::AppState;
use hmm_core::GameId;
use tauri::State;

#[tauri::command]
pub fn launch_game(
    game_id: String,
    state: State<'_, AppState>,
) -> Result<GameLaunchReceiptDto, CommandErrorDto> {
    let game_id = parse_game_id(game_id)?;

    state
        .game_launch
        .launch_game(game_id)
        .map(Into::into)
        .map_err(CommandErrorDto::from_game_launch_service_error)
}

fn parse_game_id(value: String) -> Result<GameId, CommandErrorDto> {
    GameId::parse(value).map_err(|error| CommandErrorDto {
        code: "unsupported_game".to_owned(),
        message: error.to_string(),
    })
}
