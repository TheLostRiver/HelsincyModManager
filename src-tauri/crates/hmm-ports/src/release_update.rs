//! 「最新可用版本」的只读来源。
//!
//! 与 `SteamAccountProfileClient` 同构：把外部只读查询定义成端口，
//! 实现放在 `hmm-infra`，测试里用假实现替换。
//!
//! **查询失败在这里是常态而不是异常**——断网、超时、接口变动、仓库还没有已发布
//! 版本，都会发生在普通用户身上。调用方提供不含内部细节的查询结果，不弹错误、不写
//! 失败日志打扰用户，也不让应用进入降级状态。

use std::time::Duration;
use thiserror::Error;

/// 拿不到最新版本号。
///
/// 只有一个变体是**有意的**：不区分网络失败、超时、响应不可解析等具体原因，
/// 避免把内部细节（URL、HTTP 状态、响应片段）带到上层。
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum LatestReleaseVersionError {
    #[error("the latest release version is unavailable")]
    Unavailable,
}

pub trait LatestReleaseVersionSource: Send + Sync {
    /// 返回所选通道中**版本号最高**的已发布版本原始字符串；正式通道不含预发布标签。
    ///
    /// - `Ok(Some(version))`：查到了可用版本号（`version` 已能被引擎解析，
    ///   可能带 `v` 前缀，由调用方决定怎么展示）。
    /// - `Ok(None)`：拿到了发布列表，但里面没有可用的版本号
    ///   （仓库还没有已发布版本，或标签都不合规范）。
    /// - `Err(Unavailable)`：没拿到发布列表。
    fn latest_release_version(
        &self,
        timeout: Duration,
        include_prereleases: bool,
    ) -> Result<Option<String>, LatestReleaseVersionError>;
}
