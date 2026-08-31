//! GitHub Releases 作为「最新可用版本」的来源。
//!
//! 照 `steam_profile.rs` 的模式：可注入的 transport + reqwest 实现 + 纯解析函数，
//! 解析部分可以脱离网络单独测。
//!
//! ## 为什么不放在前端做
//!
//! 前端发起请求需要把 `https://api.github.com` 加进 CSP 的 `connect-src`，
//! 那是**放宽整个前端的网络策略**（之后任何前端代码都能往 GitHub 发请求）。
//! 放在 Rust 侧则 CSP 与 Tauri capability **一行都不用改**，且 URL 是编译期常量、
//! 不接受调用方输入，不存在把请求导向任意地址的可能。

use std::time::Duration;

use hmm_core::parse_app_version;
use hmm_ports::{LatestReleaseVersionError, LatestReleaseVersionSource};
use serde::Deserialize;

// 编译期固定的端点：不接受任何调用方输入。
const RELEASE_FEED_URL: &str =
    "https://api.github.com/repos/TheLostRiver/HelsincyModManager/releases?per_page=10";
// GitHub API 要求带 User-Agent，否则返回 403。
const USER_AGENT: &str = "HelsincyModManager-update-check";

/// 发布列表的 HTTP 读取。抽成 trait 是为了让解析逻辑与测试用例都不依赖网络。
pub trait ReleaseFeedHttpTransport: Send + Sync {
    /// 错误不携带任何内部细节（URL、状态码、响应正文），避免外泄。
    fn get_release_feed_json(&self, timeout: Duration) -> Result<String, ()>;
}

pub struct ReqwestReleaseFeedHttpTransport;

impl ReleaseFeedHttpTransport for ReqwestReleaseFeedHttpTransport {
    fn get_release_feed_json(&self, timeout: Duration) -> Result<String, ()> {
        let client = reqwest::blocking::Client::builder()
            .timeout(timeout)
            .build()
            .map_err(|_| ())?;

        client
            .get(RELEASE_FEED_URL)
            .header(reqwest::header::USER_AGENT, USER_AGENT)
            .send()
            .map_err(|_| ())?
            .error_for_status()
            .map_err(|_| ())?
            .text()
            .map_err(|_| ())
    }
}

pub struct GitHubLatestReleaseSource {
    transport: Box<dyn ReleaseFeedHttpTransport>,
}

impl GitHubLatestReleaseSource {
    pub fn new(transport: Box<dyn ReleaseFeedHttpTransport>) -> Self {
        Self { transport }
    }
}

impl LatestReleaseVersionSource for GitHubLatestReleaseSource {
    fn latest_release_version(
        &self,
        timeout: Duration,
    ) -> Result<Option<String>, LatestReleaseVersionError> {
        let body = self
            .transport
            .get_release_feed_json(timeout)
            .map_err(|_| LatestReleaseVersionError::Unavailable)?;

        Ok(highest_release_version(&body))
    }
}

#[derive(Deserialize)]
struct ReleaseFeedEntry {
    tag_name: Option<String>,
    draft: Option<bool>,
}

/// 从发布列表 JSON 里挑出**版本号最高**的那个标签。
///
/// 三个刻意的取舍：
///
/// 1. **不依赖接口返回顺序**。GitHub 的列表顺序不是我们该依赖的契约，
///    所以自己比较出最大值。
/// 2. **跳过草稿**。未发布的版本不能拿来提示用户。
/// 3. **解析不了的标签直接忽略**（而不是让整次查询失败）——一个不合规范的
///    旧标签不该让「检查更新」整体失效。
fn highest_release_version(body: &str) -> Option<String> {
    let entries: Vec<ReleaseFeedEntry> = serde_json::from_str(body).ok()?;

    let mut best: Option<(hmm_core::AppVersion, String)> = None;
    for entry in entries {
        if entry.draft.unwrap_or(false) {
            continue;
        }
        let Some(tag) = entry.tag_name else {
            continue;
        };
        let Ok(version) = parse_app_version(&tag) else {
            continue;
        };
        let is_higher = best.as_ref().is_none_or(|(current, _)| &version > current);
        if is_higher {
            best = Some((version, tag));
        }
    }

    best.map(|(_, tag)| tag)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct StubTransport {
        result: Result<String, ()>,
    }

    impl ReleaseFeedHttpTransport for StubTransport {
        fn get_release_feed_json(&self, _timeout: Duration) -> Result<String, ()> {
            self.result.clone()
        }
    }

    fn source_with(body: &str) -> GitHubLatestReleaseSource {
        GitHubLatestReleaseSource::new(Box::new(StubTransport {
            result: Ok(body.to_owned()),
        }))
    }

    fn failing_source() -> GitHubLatestReleaseSource {
        GitHubLatestReleaseSource::new(Box::new(StubTransport { result: Err(()) }))
    }

    fn feed(tags: &[(&str, bool)]) -> String {
        let entries = tags
            .iter()
            .map(|(tag, draft)| {
                format!(
                    r#"{{"tag_name": "{}", "draft": {}, "prerelease": false}}"#,
                    tag, draft
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        format!("[{entries}]")
    }

    #[test]
    fn picks_the_highest_version_regardless_of_response_order() {
        // 顺序颠倒也要挑出 0.2.0——不能依赖接口的返回顺序。
        let source = source_with(&feed(&[
            ("v0.1.0", false),
            ("v0.2.0", false),
            ("v0.1.5", false),
        ]));
        assert_eq!(
            source
                .latest_release_version(Duration::from_millis(1))
                .expect("feed is readable"),
            Some("v0.2.0".to_owned())
        );
    }

    #[test]
    fn prerelease_ordering_is_numeric_not_lexicographic() {
        // alpha.10 必须赢过 alpha.9（字符串比较会得出相反结论）。
        let source = source_with(&feed(&[
            ("v0.1.0-alpha.9", false),
            ("v0.1.0-alpha.10", false),
        ]));
        assert_eq!(
            source
                .latest_release_version(Duration::from_millis(1))
                .expect("feed is readable"),
            Some("v0.1.0-alpha.10".to_owned())
        );
    }

    #[test]
    fn drafts_are_never_offered() {
        let source = source_with(&feed(&[("v9.9.9", true), ("v0.1.0", false)]));
        assert_eq!(
            source
                .latest_release_version(Duration::from_millis(1))
                .expect("feed is readable"),
            Some("v0.1.0".to_owned())
        );
    }

    #[test]
    fn unusable_tags_are_skipped_without_failing_the_whole_query() {
        let source = source_with(&feed(&[("not-a-version", false), ("v0.1.0", false)]));
        assert_eq!(
            source
                .latest_release_version(Duration::from_millis(1))
                .expect("feed is readable"),
            Some("v0.1.0".to_owned())
        );
    }

    #[test]
    fn empty_and_unusable_feeds_yield_no_version() {
        assert_eq!(
            source_with("[]")
                .latest_release_version(Duration::from_millis(1))
                .expect("empty feed is still readable"),
            None
        );
        assert_eq!(
            source_with(&feed(&[("not-a-version", false)]))
                .latest_release_version(Duration::from_millis(1))
                .expect("feed is readable"),
            None
        );
        // 响应不是数组（例如接口换了形状）也不该 panic。
        assert_eq!(
            source_with(r#"{"message": "Not Found"}"#)
                .latest_release_version(Duration::from_millis(1))
                .expect("payload is readable"),
            None
        );
    }

    #[test]
    fn transport_failures_are_reported_as_unavailable() {
        assert_eq!(
            failing_source().latest_release_version(Duration::from_millis(1)),
            Err(LatestReleaseVersionError::Unavailable)
        );
    }

    #[test]
    fn missing_optional_fields_do_not_break_parsing() {
        // 字段缺失（而不是 null）也不该让解析失败。
        let source = source_with(r#"[{"name": "no tag here"}]"#);
        assert_eq!(
            source
                .latest_release_version(Duration::from_millis(1))
                .expect("payload is readable"),
            None
        );
    }
}
