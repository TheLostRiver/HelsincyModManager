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

use std::time::{Duration, Instant};

use hmm_core::parse_app_version;
use hmm_ports::{LatestReleaseVersionError, LatestReleaseVersionSource};
use serde::Deserialize;

// 编译期固定的端点：不接受任何调用方输入。
const RELEASE_FEED_URL: &str =
    "https://api.github.com/repos/TheLostRiver/HelsincyModManager/releases";
const RELEASE_PAGE_SIZE: usize = 100;
const MAX_RELEASE_PAGES: usize = 10;
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

        read_complete_release_feed(timeout, |page, remaining| {
            let mut url = reqwest::Url::parse(RELEASE_FEED_URL).map_err(|_| ())?;
            url.query_pairs_mut()
                .append_pair("per_page", &RELEASE_PAGE_SIZE.to_string())
                .append_pair("page", &page.to_string());
            client
                .get(url)
                .header(reqwest::header::USER_AGENT, USER_AGENT)
                .timeout(remaining)
                .send()
                .map_err(|_| ())?
                .error_for_status()
                .map_err(|_| ())?
                .text()
                .map_err(|_| ())
        })
    }
}

// 不完整的列表不能证明“已是最新”。分页共用同一超时预算，超限返回未知而非部分结论。
fn read_complete_release_feed(
    timeout: Duration,
    mut fetch_page: impl FnMut(usize, Duration) -> Result<String, ()>,
) -> Result<String, ()> {
    let started = Instant::now();
    let mut all = Vec::<serde_json::Value>::new();
    for page in 1..=MAX_RELEASE_PAGES {
        let remaining = timeout.checked_sub(started.elapsed()).ok_or(())?;
        if remaining.is_zero() {
            return Err(());
        }
        let body = fetch_page(page, remaining)?;
        let entries: Vec<serde_json::Value> = serde_json::from_str(&body).map_err(|_| ())?;
        if entries.len() > RELEASE_PAGE_SIZE {
            return Err(());
        }
        let complete = entries.len() < RELEASE_PAGE_SIZE;
        all.extend(entries);
        if complete {
            return serde_json::to_string(&all).map_err(|_| ());
        }
    }
    Err(())
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
        include_prereleases: bool,
    ) -> Result<Option<String>, LatestReleaseVersionError> {
        let body = self
            .transport
            .get_release_feed_json(timeout)
            .map_err(|_| LatestReleaseVersionError::Unavailable)?;

        highest_release_version(&body, include_prereleases)
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
fn highest_release_version(
    body: &str,
    include_prereleases: bool,
) -> Result<Option<String>, LatestReleaseVersionError> {
    let entries: Vec<ReleaseFeedEntry> =
        serde_json::from_str(body).map_err(|_| LatestReleaseVersionError::Unavailable)?;

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
        if version.is_prerelease() && !include_prereleases {
            continue;
        }
        let is_higher = best.as_ref().is_none_or(|(current, _)| &version > current);
        if is_higher {
            best = Some((version, tag));
        }
    }

    Ok(best.map(|(_, tag)| tag))
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
                .latest_release_version(Duration::from_millis(1), true)
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
                .latest_release_version(Duration::from_millis(1), true)
                .expect("feed is readable"),
            Some("v0.1.0-alpha.10".to_owned())
        );
    }

    #[test]
    fn drafts_are_never_offered() {
        let source = source_with(&feed(&[("v9.9.9", true), ("v0.1.0", false)]));
        assert_eq!(
            source
                .latest_release_version(Duration::from_millis(1), true)
                .expect("feed is readable"),
            Some("v0.1.0".to_owned())
        );
    }

    #[test]
    fn unusable_tags_are_skipped_without_failing_the_whole_query() {
        let source = source_with(&feed(&[("not-a-version", false), ("v0.1.0", false)]));
        assert_eq!(
            source
                .latest_release_version(Duration::from_millis(1), true)
                .expect("feed is readable"),
            Some("v0.1.0".to_owned())
        );
    }

    #[test]
    fn empty_and_unusable_feeds_yield_no_version() {
        assert_eq!(
            source_with("[]")
                .latest_release_version(Duration::from_millis(1), true)
                .expect("empty feed is still readable"),
            None
        );
        assert_eq!(
            source_with(&feed(&[("not-a-version", false)]))
                .latest_release_version(Duration::from_millis(1), true)
                .expect("feed is readable"),
            None
        );
    }

    #[test]
    fn transport_failures_are_reported_as_unavailable() {
        assert_eq!(
            failing_source().latest_release_version(Duration::from_millis(1), true),
            Err(LatestReleaseVersionError::Unavailable)
        );
    }

    #[test]
    fn missing_optional_fields_do_not_break_parsing() {
        // 字段缺失（而不是 null）也不该让解析失败。
        let source = source_with(r#"[{"name": "no tag here"}]"#);
        assert_eq!(
            source
                .latest_release_version(Duration::from_millis(1), true)
                .expect("payload is readable"),
            None
        );
    }

    #[test]
    fn malformed_feed_is_unavailable_not_an_empty_release_list() {
        assert_eq!(
            source_with(r#"{"message": "Not Found"}"#)
                .latest_release_version(Duration::from_millis(1), true),
            Err(LatestReleaseVersionError::Unavailable)
        );
    }

    #[test]
    fn a_higher_prerelease_does_not_hide_a_stable_update() {
        let source = source_with(&feed(&[("v2.0.0-alpha.1", false), ("v1.1.0", false)]));
        let latest = source
            .latest_release_version(Duration::from_millis(1), false)
            .expect("readable feed");
        assert_eq!(latest.as_deref(), Some("v1.1.0"));
        assert_eq!(
            hmm_core::decide_update("1.0.0", latest.as_deref()),
            hmm_core::UpdateDecision::UpdateAvailable {
                version: "v1.1.0".to_owned()
            }
        );
    }

    #[test]
    fn a_preview_channel_can_still_receive_the_highest_prerelease() {
        assert_eq!(
            source_with(&feed(&[("v2.0.0-alpha.1", false), ("v1.1.0", false)]))
                .latest_release_version(Duration::from_millis(1), true)
                .expect("readable feed"),
            Some("v2.0.0-alpha.1".to_owned())
        );
    }

    #[test]
    fn later_release_pages_are_included_before_version_selection() {
        let mut pages = Vec::new();
        let body = read_complete_release_feed(Duration::from_secs(1), |page, _| {
            pages.push(page);
            Ok(if page == 1 {
                feed(&vec![("v1.0.0", false); RELEASE_PAGE_SIZE])
            } else {
                feed(&[("v1.2.0", false)])
            })
        })
        .expect("complete feed");
        assert_eq!(pages, vec![1, 2]);
        assert_eq!(
            highest_release_version(&body, false),
            Ok(Some("v1.2.0".to_owned()))
        );
    }

    #[test]
    fn failed_later_page_never_yields_a_partial_success() {
        assert!(
            read_complete_release_feed(Duration::from_secs(1), |page, _| {
                if page == 1 {
                    Ok(feed(&vec![("v1.0.0", false); RELEASE_PAGE_SIZE]))
                } else {
                    Err(())
                }
            })
            .is_err()
        );
    }

    #[test]
    fn full_page_budget_never_claims_the_feed_is_complete() {
        let mut count = 0;
        assert!(read_complete_release_feed(Duration::from_secs(1), |_, _| {
            count += 1;
            Ok(feed(&vec![("v1.0.0", false); RELEASE_PAGE_SIZE]))
        })
        .is_err());
        assert_eq!(count, MAX_RELEASE_PAGES);
    }

    #[test]
    fn expired_feed_budget_never_starts_a_request() {
        assert!(read_complete_release_feed(Duration::ZERO, |_, _| {
            panic!("expired budget must stop before HTTP")
        })
        .is_err());
    }
}
