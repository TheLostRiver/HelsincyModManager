//! 「检查更新」：只**告知**有没有新版本，不下载、不校验、不执行。
//!
//! 与 `docs/release/UPDATER_PLAN.md` 的关系：那份文档约束的是「应用内自动更新」
//! （下载 + 签名校验 + 安装），本条实现的是它的**前置改良**——把「打开 Releases
//! 让用户自己肉眼比对版本号」升级为「直接告诉你有没有新版」，因此不触碰
//! updater 的任何前置条件（无签名、无密钥、无文件写入）。
//!
//! 网络请求发生在 Rust 侧，因此**不需要改 CSP，也不需要新增 capability 权限**：
//! 前端始终拿不到发请求的手段，整体网络策略没有被放宽。

use std::time::Duration;

use hmm_core::{decide_update, parse_app_version, UpdateDecision};
use hmm_infra::{GitHubLatestReleaseSource, ReqwestReleaseFeedHttpTransport};
use hmm_ports::{LatestReleaseVersionError, LatestReleaseVersionSource};

use crate::update_dto::AppUpdateStatusDto;

/// 短超时：宁可放弃这次检查，也不能让用户等。
const RELEASE_CHECK_TIMEOUT: Duration = Duration::from_secs(3);

/// 查询是否有可用更新。
///
/// 这是 `async` 命令：真正的 HTTP 走 `spawn_blocking`（本机 reqwest 只有 blocking
/// feature），避免阻塞 Tauri 的异步运行时。
///
/// 失败收敛为 `unknown`，成功查询但无当前通道可用版本为 `no_release`。
/// 不返回内部网络细节，不写失败日志打扰用户。
#[tauri::command]
pub async fn check_app_update() -> AppUpdateStatusDto {
    let current_version = env!("CARGO_PKG_VERSION").to_owned();

    let latest_version = tauri::async_runtime::spawn_blocking(move || {
        let current = parse_app_version(env!("CARGO_PKG_VERSION"))
            .map_err(|_| LatestReleaseVersionError::Unavailable)?;
        let source = GitHubLatestReleaseSource::new(Box::new(ReqwestReleaseFeedHttpTransport));
        source.latest_release_version(RELEASE_CHECK_TIMEOUT, current.is_prerelease())
    })
    .await
    .unwrap_or(Err(LatestReleaseVersionError::Unavailable));

    project_release_result(current_version, latest_version)
}

fn project_release_result(
    current_version: String,
    result: Result<Option<String>, LatestReleaseVersionError>,
) -> AppUpdateStatusDto {
    match result {
        Ok(Some(latest)) => {
            let decision = decide_update(&current_version, Some(&latest));
            AppUpdateStatusDto::from_decision(current_version, decision)
        }
        Ok(None) => AppUpdateStatusDto::no_release(current_version),
        Err(_) => AppUpdateStatusDto::from_decision(current_version, UpdateDecision::Unknown),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hmm_core::UpdateDecision;

    #[test]
    fn unknown_is_silent_about_the_reason() {
        // 拿不到最新版本时，界面只该看到「不知道」，不该看到任何内部细节。
        let dto = AppUpdateStatusDto::from_decision(
            env!("CARGO_PKG_VERSION").to_owned(),
            UpdateDecision::Unknown,
        );
        assert_eq!(dto.status, "unknown");
        assert_eq!(dto.latest_version, None);
    }

    #[test]
    fn the_timeout_stays_short_enough_to_never_block_the_ui() {
        assert!(RELEASE_CHECK_TIMEOUT <= Duration::from_secs(5));
    }

    #[test]
    fn empty_release_list_is_distinct_from_query_failure() {
        assert_eq!(project_release_result("1.0.0".to_owned(), Ok(None)).status, "no_release");
        assert_eq!(
            project_release_result("1.0.0".to_owned(), Err(LatestReleaseVersionError::Unavailable)).status,
            "unknown"
        );
    }
}
