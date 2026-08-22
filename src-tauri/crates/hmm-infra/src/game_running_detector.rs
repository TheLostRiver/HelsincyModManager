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
        detect_registered_processes(&self.process_names, game_id, query_process_running)
    }
}

/// 基于 Unix-like `pgrep` 的游戏运行检测。
///
/// 仅用于非 Windows 平台的 best-effort 检测：`pgrep` 调用失败仍返回 `Unknown`，
/// 只有命令明确返回“无匹配”时才视为 `NotRunning`。
/// Windows 运行时仍应使用 `TasklistGameRunningDetector`。
pub struct PgrepGameRunningDetector {
    process_names: HashMap<GameId, Vec<String>>,
}

impl PgrepGameRunningDetector {
    pub fn new(process_names: HashMap<GameId, Vec<String>>) -> Self {
        Self { process_names }
    }
}

impl GameRunningDetector for PgrepGameRunningDetector {
    fn game_running_status(&self, game_id: &GameId) -> GameRunningStatus {
        detect_registered_processes(&self.process_names, game_id, query_pgrep_process_running)
    }
}

fn detect_registered_processes(
    process_names: &HashMap<GameId, Vec<String>>,
    game_id: &GameId,
    query: fn(&str) -> GameRunningStatus,
) -> GameRunningStatus {
    let Some(names) = process_names.get(game_id) else {
        return GameRunningStatus::Unknown;
    };
    if names.is_empty() {
        return GameRunningStatus::Unknown;
    }

    let mut status = GameRunningStatus::NotRunning;
    for name in names {
        match query(name) {
            GameRunningStatus::Running => return GameRunningStatus::Running,
            GameRunningStatus::Unknown => status = GameRunningStatus::Unknown,
            GameRunningStatus::NotRunning => {}
        }
    }
    status
}

#[cfg(target_os = "windows")]
fn query_process_running(image_name: &str) -> GameRunningStatus {
    query_process_running_with_retries(image_name, query_tasklist_once)
}

/// tasklist spawn 在高并发/杀毒扫描负载下可能瞬态失败。直接把瞬态失败
/// 报成 `Unknown` 会让安装闸门在真实可判定的时刻误拒，因此做有界重试；
/// 只有连续 `TASKLIST_QUERY_ATTEMPTS` 次都失败才落到 `Unknown`。
#[cfg(any(target_os = "windows", test))]
const TASKLIST_QUERY_ATTEMPTS: usize = 3;

#[cfg(any(target_os = "windows", test))]
fn query_process_running_with_retries<F>(image_name: &str, query_once: F) -> GameRunningStatus
where
    F: Fn(&str) -> Option<GameRunningStatus>,
{
    for _ in 0..TASKLIST_QUERY_ATTEMPTS {
        if let Some(status) = query_once(image_name) {
            return status;
        }
    }
    GameRunningStatus::Unknown
}

#[cfg(target_os = "windows")]
fn query_tasklist_once(image_name: &str) -> Option<GameRunningStatus> {
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
            Some(tasklist_output_contains_image(&stdout, image_name))
        }
        _ => None,
    }
}

#[cfg(not(target_os = "windows"))]
fn query_process_running(_image_name: &str) -> GameRunningStatus {
    GameRunningStatus::Unknown
}

#[cfg(not(target_os = "windows"))]
fn query_pgrep_process_running(image_name: &str) -> GameRunningStatus {
    use std::process::Command;

    if image_name.trim().is_empty() || image_name.starts_with('-') {
        return GameRunningStatus::Unknown;
    }

    match Command::new("pgrep")
        .arg("-f")
        .arg(pgrep_literal_pattern(image_name))
        .output()
    {
        Ok(output) => {
            pgrep_status_to_game_running_status(output.status.success(), output.status.code())
        }
        Err(_) => GameRunningStatus::Unknown,
    }
}

#[cfg(target_os = "windows")]
fn query_pgrep_process_running(_image_name: &str) -> GameRunningStatus {
    GameRunningStatus::Unknown
}

#[cfg(any(not(target_os = "windows"), test))]
fn pgrep_literal_pattern(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for ch in value.chars() {
        if matches!(
            ch,
            '.' | '[' | ']' | '\\' | '(' | ')' | '*' | '+' | '?' | '{' | '}' | '|' | '^' | '$'
        ) {
            escaped.push('\\');
        }
        escaped.push(ch);
    }
    escaped
}

#[cfg(any(not(target_os = "windows"), test))]
fn pgrep_status_to_game_running_status(success: bool, code: Option<i32>) -> GameRunningStatus {
    if success {
        return GameRunningStatus::Running;
    }

    match code {
        Some(1) => GameRunningStatus::NotRunning,
        _ => GameRunningStatus::Unknown,
    }
}

/// 纯匹配逻辑：tasklist CSV 输出是否包含目标映像名（大小写不敏感）。
/// 不匹配 tasklist 的本地化提示文本（如 "INFO: 没有运行的任务…"）。
#[cfg(any(target_os = "windows", test))]
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
    fn transient_tasklist_failure_is_retried_before_reporting_unknown() {
        let attempts = std::cell::Cell::new(0u32);
        let flaky_once = |_: &str| {
            attempts.set(attempts.get() + 1);
            if attempts.get() == 1 {
                None
            } else {
                Some(GameRunningStatus::NotRunning)
            }
        };
        assert_eq!(
            query_process_running_with_retries("MonsterHunterWorld.exe", flaky_once),
            GameRunningStatus::NotRunning
        );
        assert_eq!(attempts.get(), 2);
    }

    #[test]
    fn persistent_tasklist_failure_still_reports_unknown() {
        let attempts = std::cell::Cell::new(0u32);
        let always_failing = |_: &str| {
            attempts.set(attempts.get() + 1);
            None
        };
        assert_eq!(
            query_process_running_with_retries("MonsterHunterWorld.exe", always_failing),
            GameRunningStatus::Unknown
        );
        assert_eq!(attempts.get(), TASKLIST_QUERY_ATTEMPTS as u32);
    }

    #[test]
    fn running_status_is_returned_immediately_without_retry() {
        let attempts = std::cell::Cell::new(0u32);
        let running_once = |_: &str| {
            attempts.set(attempts.get() + 1);
            Some(GameRunningStatus::Running)
        };
        assert_eq!(
            query_process_running_with_retries("MonsterHunterWorld.exe", running_once),
            GameRunningStatus::Running
        );
        assert_eq!(attempts.get(), 1);
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

    #[test]
    fn pgrep_literal_pattern_escapes_regex_metacharacters() {
        assert_eq!(
            pgrep_literal_pattern("MonsterHunterWorld.exe"),
            "MonsterHunterWorld\\.exe"
        );
        assert_eq!(
            pgrep_literal_pattern("Game+[test](x)"),
            "Game\\+\\[test\\]\\(x\\)"
        );
    }

    #[test]
    fn pgrep_success_reports_running() {
        assert_eq!(
            pgrep_status_to_game_running_status(true, Some(0)),
            GameRunningStatus::Running
        );
    }

    #[test]
    fn pgrep_no_match_reports_not_running() {
        assert_eq!(
            pgrep_status_to_game_running_status(false, Some(1)),
            GameRunningStatus::NotRunning
        );
    }

    #[test]
    fn pgrep_error_reports_unknown() {
        assert_eq!(
            pgrep_status_to_game_running_status(false, Some(2)),
            GameRunningStatus::Unknown
        );
    }
}
