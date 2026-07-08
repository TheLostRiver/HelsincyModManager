use hmm_core::GameId;

/// 游戏运行状态。检测失败或无法判断时必须返回 `Unknown`，
/// 由调用方保守处理（自动备份延后），不得把失败当成"未运行"。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameRunningStatus {
    Running,
    NotRunning,
    Unknown,
}

pub trait GameRunningDetector: Send + Sync {
    fn game_running_status(&self, game_id: &GameId) -> GameRunningStatus;
}
