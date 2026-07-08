use hmm_core::GameId;
use hmm_ports::{GameRunningDetector, GameRunningStatus};
use std::collections::HashMap;

/// 基于 Windows `tasklist` 的游戏运行检测。
///
/// 安全语义：任何失败（未注册进程名、spawn 失败、非零退出、非 Windows 平台）
/// 都返回 `Unknown`，由调度器保守延后自动备份，绝不把失败当成"未运行"。
pub struct TasklistGameRunningDetector {
    process_names: HashMap<GameId, Vec<String>>,
}

impl TasklistGameRunningDetector {
    pub fn new(process_names: HashMap<GameId, Vec<String>>) -> Self {
        Self { process_names }
    }
}

impl GameRunningDetector for TasklistGameRunningDetector {
    fn game_running_status(&self, game_id: &GameId) -> GameRunningStatus {
        let Some(names) = self.process_names.get(game_id) else {
            return GameRunningStatus::Unknown;
        };
        if names.is_empty() {
            return GameRunningStatus::Unknown;
        }

        let mut status = GameRunningStatus::NotRunning;
        for name in names {
            match query_process_running(name) {
                GameRunningStatus::Running => return GameRunningStatus::Running,
                GameRunningStatus::Unknown => status = GameRunningStatus::Unknown,
                GameRunningStatus::NotRunning => {}
            }
        }
        status
    }
}

#[cfg(target_os = "windows")]
fn query_process_running(image_name: &str) -> GameRunningStatus {
    use std::os::windows::process::CommandExt;
    use std::process::Command;

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    let output = Command::new("tasklist")
        .arg("/FI")
        .arg(format!("IMAGENAME eq {image_name}"))
        .arg("/FO")
        .arg("CSV")
        .arg("/NH")
        .creation_flags(CREATE_NO_WINDOW)
        .output();

    match output {
        Ok(output) if output.status.success() => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            tasklist_output_contains_image(&stdout, image_name)
        }
        _ => GameRunningStatus::Unknown,
    }
}

#[cfg(not(target_os = "windows"))]
fn query_process_running(_image_name: &str) -> GameRunningStatus {
    GameRunningStatus::Unknown
}

/// 纯匹配逻辑：tasklist CSV 输出是否包含目标映像名（大小写不敏感）。
/// 不匹配 tasklist 的本地化提示文本（如 "INFO: 没有运行的任务…"）。
fn tasklist_output_contains_image(output: &str, image_name: &str) -> GameRunningStatus {
    let needle = image_name.to_ascii_lowercase();
    let found = output
        .lines()
        .any(|line| line.to_ascii_lowercase().contains(&needle));
    if found {
        GameRunningStatus::Running
    } else {
        GameRunningStatus::NotRunning
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matching_csv_line_reports_running() {
        let output = "\"MonsterHunterWorld.exe\",\"12345\",\"Console\",\"1\",\"1,024 K\"\r\n";
        assert_eq!(
            tasklist_output_contains_image(output, "MonsterHunterWorld.exe"),
            GameRunningStatus::Running
        );
    }

    #[test]
    fn match_is_case_insensitive() {
        let output = "\"monsterhunterworld.EXE\",\"12345\",\"Console\",\"1\",\"1,024 K\"\r\n";
        assert_eq!(
            tasklist_output_contains_image(output, "MonsterHunterWorld.exe"),
            GameRunningStatus::Running
        );
    }

    #[test]
    fn localized_no_task_message_reports_not_running() {
        let output = "信息: 没有运行的任务匹配指定标准。\r\n";
        assert_eq!(
            tasklist_output_contains_image(output, "MonsterHunterWorld.exe"),
            GameRunningStatus::NotRunning
        );
    }

    #[test]
    fn empty_output_reports_not_running() {
        assert_eq!(
            tasklist_output_contains_image("", "MonsterHunterWorld.exe"),
            GameRunningStatus::NotRunning
        );
    }

    #[test]
    fn unregistered_game_reports_unknown() {
        let detector = TasklistGameRunningDetector::new(HashMap::new());
        assert_eq!(
            detector.game_running_status(&GameId::mhw()),
            GameRunningStatus::Unknown
        );
    }

    #[test]
    fn registered_game_with_empty_names_reports_unknown() {
        let detector = TasklistGameRunningDetector::new(HashMap::from([(
            GameId::mhw(),
            Vec::<String>::new(),
        )]));
        assert_eq!(
            detector.game_running_status(&GameId::mhw()),
            GameRunningStatus::Unknown
        );
    }
}
