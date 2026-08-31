//! 应用版本号的解析与比较。
//!
//! 「检查更新」需要回答一个问题：**某个版本号是不是比当前版本新**。
//! 这件事不能用字符串比较——`0.1.0-alpha.10` 按字符串排会小于 `0.1.0-alpha.9`。
//!
//! 这里实现的是 semver 2.0.0 的**优先级规则子集**：
//!
//! 1. 核心版本号 `major.minor.patch` 逐级数值比较；
//! 2. 有预发布标识的版本**小于**同核心版本号的正式版（`0.1.0-alpha.0 < 0.1.0`）；
//! 3. 预发布标识按 `.` 分隔逐段比较：数字段比数值、字母段按 ASCII 字典序，
//!    数字段**小于**字母段，前面都相等时**段多者更大**；
//! 4. `+build` 元数据不参与优先级比较（按 semver 规定忽略）。
//!
//! 解析失败一律走 `Err`，**不做宽松兜底**：宁可报「不知道有没有新版」（静默），
//! 也不能因为把 `0.1.0-alpha.10` 误判成旧版而让该升级的用户一直停在原地。

use thiserror::Error;

/// 版本号解析失败的原因。只用于诊断，不向玩家展示。
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum AppVersionError {
    #[error("version string is empty")]
    Empty,
    #[error("version core is not `major.minor.patch`: {value}")]
    MalformedCore { value: String },
    #[error("version core component is not a non-negative integer: {value}")]
    InvalidCoreComponent { value: String },
    #[error("prerelease identifier is empty: {value}")]
    EmptyPrereleaseIdentifier { value: String },
    #[error("numeric prerelease identifier has a leading zero: {value}")]
    LeadingZeroPrerelease { value: String },
}

/// 预发布标识里的一段。
#[derive(Debug, Clone, PartialEq, Eq)]
enum PrereleaseId {
    /// 纯数字段，按数值比较，且小于任何字母段。
    Numeric(u64),
    /// 字母/数字混合段，按 ASCII 字典序比较。
    Alphanumeric(String),
}

impl Ord for PrereleaseId {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        match (self, other) {
            (Self::Numeric(left), Self::Numeric(right)) => left.cmp(right),
            // 数字段优先级低于字母段（semver 2.0.0 §11.4.3）。
            (Self::Numeric(_), Self::Alphanumeric(_)) => std::cmp::Ordering::Less,
            (Self::Alphanumeric(_), Self::Numeric(_)) => std::cmp::Ordering::Greater,
            (Self::Alphanumeric(left), Self::Alphanumeric(right)) => left.cmp(right),
        }
    }
}

impl PartialOrd for PrereleaseId {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

/// 已解析的应用版本号。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppVersion {
    major: u64,
    minor: u64,
    patch: u64,
    prerelease: Vec<PrereleaseId>,
}

impl AppVersion {
    pub fn new(major: u64, minor: u64, patch: u64) -> Self {
        Self {
            major,
            minor,
            patch,
            prerelease: Vec::new(),
        }
    }

    /// 是否为预发布版本（带 `-alpha.N` / `-beta.N` 之类标识）。
    pub fn is_prerelease(&self) -> bool {
        !self.prerelease.is_empty()
    }

    pub fn major(&self) -> u64 {
        self.major
    }

    pub fn minor(&self) -> u64 {
        self.minor
    }

    pub fn patch(&self) -> u64 {
        self.patch
    }
}

impl Ord for AppVersion {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        let core =
            (self.major, self.minor, self.patch).cmp(&(other.major, other.minor, other.patch));
        if core != std::cmp::Ordering::Equal {
            return core;
        }

        match (self.prerelease.is_empty(), other.prerelease.is_empty()) {
            // 都不带预发布标识：核心版本号相同即相等（上面已比较过）。
            (true, true) => std::cmp::Ordering::Equal,
            // 正式版大于同核心版本号的预发布版。
            (true, false) => std::cmp::Ordering::Greater,
            (false, true) => std::cmp::Ordering::Less,
            // 逐段比较；前面都相等时段多者更大。
            (false, false) => self.prerelease.cmp(&other.prerelease),
        }
    }
}

impl PartialOrd for AppVersion {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl std::fmt::Display for AppVersion {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}.{}.{}", self.major, self.minor, self.patch)?;
        if !self.prerelease.is_empty() {
            let rendered = self
                .prerelease
                .iter()
                .map(|id| match id {
                    PrereleaseId::Numeric(value) => value.to_string(),
                    PrereleaseId::Alphanumeric(value) => value.clone(),
                })
                .collect::<Vec<_>>()
                .join(".");
            write!(formatter, "-{rendered}")?;
        }
        Ok(())
    }
}

/// 解析版本号。容忍前导 `v` / `V` 与首尾空白，忽略 `+build` 元数据。
///
/// 其余一切非法输入都返回 `Err`——**不做宽松兜底**（理由见模块文档）。
pub fn parse_app_version(raw: &str) -> Result<AppVersion, AppVersionError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(AppVersionError::Empty);
    }

    // Git 标签惯例：`v0.1.0-alpha.0`。
    let without_prefix = trimmed.trim_start_matches(['v', 'V']);
    // semver 的 build 元数据不参与优先级比较。
    let without_build = without_prefix
        .split_once('+')
        .map_or(without_prefix, |(head, _)| head);

    let (core, prerelease) = match without_build.split_once('-') {
        Some((core, prerelease)) => (core, Some(prerelease)),
        None => (without_build, None),
    };

    let mut parts = core.split('.');
    let major = parse_core_component(parts.next().unwrap_or_default())?;
    let minor =
        parse_core_component(parts.next().ok_or_else(|| AppVersionError::MalformedCore {
            value: trimmed.to_owned(),
        })?)?;
    let patch =
        parse_core_component(parts.next().ok_or_else(|| AppVersionError::MalformedCore {
            value: trimmed.to_owned(),
        })?)?;
    if parts.next().is_some() {
        return Err(AppVersionError::MalformedCore {
            value: trimmed.to_owned(),
        });
    }

    let prerelease = match prerelease {
        None => Vec::new(),
        Some(raw_prerelease) => parse_prerelease(raw_prerelease, trimmed)?,
    };

    Ok(AppVersion {
        major,
        minor,
        patch,
        prerelease,
    })
}

fn parse_core_component(value: &str) -> Result<u64, AppVersionError> {
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(AppVersionError::InvalidCoreComponent {
            value: value.to_owned(),
        });
    }
    value
        .parse::<u64>()
        .map_err(|_| AppVersionError::InvalidCoreComponent {
            value: value.to_owned(),
        })
}

fn parse_prerelease(raw: &str, whole: &str) -> Result<Vec<PrereleaseId>, AppVersionError> {
    if raw.is_empty() {
        return Err(AppVersionError::EmptyPrereleaseIdentifier {
            value: whole.to_owned(),
        });
    }

    raw.split('.')
        .map(|segment| {
            if segment.is_empty() {
                return Err(AppVersionError::EmptyPrereleaseIdentifier {
                    value: whole.to_owned(),
                });
            }
            match segment.parse::<u64>() {
                Ok(value) => {
                    // semver 禁止数字段带前导零，否则 `1.0.0` 与 `01.0.0` 会混淆。
                    if segment.len() > 1 && segment.starts_with('0') {
                        return Err(AppVersionError::LeadingZeroPrerelease {
                            value: whole.to_owned(),
                        });
                    }
                    Ok(PrereleaseId::Numeric(value))
                }
                Err(_) => Ok(PrereleaseId::Alphanumeric(segment.to_owned())),
            }
        })
        .collect()
}

/// 「要不要提示更新」的判定结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateDecision {
    /// 已是最新，或没有可提示的更新。
    UpToDate,
    /// 有可用更新。携带**原始**版本号字符串，供界面原样展示。
    UpdateAvailable { version: String },
    /// 不知道（拿不到最新版本 / 版本号解析不了）。调用方应静默。
    Unknown,
}

/// 结合当前版本与查到的最新版本，给出「是否提示更新」。
///
/// 规则（除版本先后外还有一条通道规则）：
///
/// - **当前是正式版时不提示预发布版本**。预览通道只面向已经在预览通道上的用户，
///   否则稳定版用户会被引导去装 alpha。通道划分本身仍是未决问题
///   （见 `docs/release/UPDATER_PLAN.md`），这里先取保守的一侧。
/// - 解析失败一律 `Unknown`——宁可不提示，也不拿不可靠的版本号去打扰用户。
pub fn decide_update(current: &str, latest: Option<&str>) -> UpdateDecision {
    let Some(latest) = latest else {
        return UpdateDecision::Unknown;
    };
    let (Ok(current), Ok(candidate)) = (parse_app_version(current), parse_app_version(latest))
    else {
        return UpdateDecision::Unknown;
    };

    if candidate <= current {
        return UpdateDecision::UpToDate;
    }

    // 稳定通道不接收预发布提示。
    if !current.is_prerelease() && candidate.is_prerelease() {
        return UpdateDecision::UpToDate;
    }

    UpdateDecision::UpdateAvailable {
        version: latest.to_owned(),
    }
}

/// `candidate` 是否严格新于 `current`。
///
/// **任一侧解析失败都返回 `false`**：宁可漏报也不能误报——
/// 误报会让用户去下一个不存在的版本，漏报只是维持现状（且调用方本就应对
/// 「不知道」静默处理）。
pub fn is_newer_version(candidate: &str, current: &str) -> bool {
    match (parse_app_version(candidate), parse_app_version(current)) {
        (Ok(candidate), Ok(current)) => candidate > current,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn version(raw: &str) -> AppVersion {
        parse_app_version(raw).unwrap_or_else(|error| panic!("{raw}: {error}"))
    }

    #[test]
    fn parses_the_plain_core_version() {
        let parsed = version("0.1.0");
        assert_eq!((parsed.major(), parsed.minor(), parsed.patch()), (0, 1, 0));
        assert!(!parsed.is_prerelease());
    }

    #[test]
    fn tolerates_a_leading_v_and_surrounding_whitespace() {
        // Git 标签是 `v0.1.0-alpha.0`，而应用版本是 `0.1.0-alpha.0`，两者必须等价。
        assert_eq!(version("v0.1.0-alpha.0"), version("0.1.0-alpha.0"));
        assert_eq!(version("  0.1.0  "), version("0.1.0"));
        assert_eq!(version("V1.2.3"), version("1.2.3"));
    }

    #[test]
    fn ignores_build_metadata_in_precedence() {
        assert_eq!(version("0.1.0+build.5"), version("0.1.0"));
        assert!(!is_newer_version("0.1.0+build.9", "0.1.0+build.1"));
    }

    #[test]
    fn prerelease_sequence_compares_numerically_not_lexically() {
        // 这条是整件事的起因：按字符串比，`-alpha.10` 会小于 `-alpha.9`。
        assert!(is_newer_version("0.1.0-alpha.10", "0.1.0-alpha.9"));
        assert!(is_newer_version("0.1.0-alpha.9", "0.1.0-alpha.0"));
        assert!(!is_newer_version("0.1.0-alpha.9", "0.1.0-alpha.10"));
    }

    #[test]
    fn a_release_outranks_the_same_core_prerelease() {
        assert!(is_newer_version("0.1.0", "0.1.0-alpha.0"));
        assert!(!is_newer_version("0.1.0-alpha.0", "0.1.0"));
        // 稳定版发布后，alpha 用户应当被提示升级。
        assert!(is_newer_version("0.2.0", "0.1.0-alpha.99"));
    }

    #[test]
    fn core_components_compare_before_the_prerelease() {
        assert!(is_newer_version("0.1.1-alpha.0", "0.1.0"));
        assert!(is_newer_version("1.0.0-alpha.0", "0.9.9"));
        assert!(!is_newer_version("0.1.0", "0.1.1-alpha.0"));
    }

    #[test]
    fn mixed_prerelease_identifiers_follow_semver_precedence() {
        // 数字段小于字母段；字母段按 ASCII 字典序。
        assert!(is_newer_version("0.1.0-alpha.1", "0.1.0-alpha.0"));
        assert!(is_newer_version("0.1.0-beta", "0.1.0-alpha.99"));
        assert!(is_newer_version("0.1.0-rc.1", "0.1.0-beta.9"));
        // 前面都相等时段多者更大。
        assert!(is_newer_version("0.1.0-alpha.0.1", "0.1.0-alpha.0"));
    }

    #[test]
    fn identical_versions_are_not_newer() {
        assert!(!is_newer_version("0.1.0", "0.1.0"));
        assert!(!is_newer_version("0.1.0-alpha.0", "0.1.0-alpha.0"));
    }

    #[test]
    fn unparsable_versions_never_report_an_update() {
        // 保守方向：解析不了就说「没有更新」，绝不误报。
        for candidate in [
            "",
            "not-a-version",
            "0.1",
            "0.1.x",
            "0.1.0-alpha.01",
            "v",
            "0..0",
        ] {
            assert!(
                !is_newer_version(candidate, "0.1.0-alpha.0"),
                "{candidate} 不应被判为更新"
            );
            assert!(
                !is_newer_version("9.9.9", candidate),
                "与非法版本 {candidate} 比较时不应报更新"
            );
        }
    }

    #[test]
    fn rejects_core_shapes_that_are_not_major_minor_patch() {
        assert!(matches!(
            parse_app_version("0.1"),
            Err(AppVersionError::MalformedCore { .. })
        ));
        assert!(matches!(
            parse_app_version("0.1.0.0"),
            Err(AppVersionError::MalformedCore { .. })
        ));
        assert!(matches!(
            parse_app_version("0.1.x"),
            Err(AppVersionError::InvalidCoreComponent { .. })
        ));
        assert!(matches!(parse_app_version(""), Err(AppVersionError::Empty)));
    }

    #[test]
    fn decide_update_reports_the_newer_version() {
        assert_eq!(
            decide_update("0.1.0-alpha.0", Some("0.1.0-alpha.1")),
            UpdateDecision::UpdateAvailable {
                version: "0.1.0-alpha.1".to_owned()
            }
        );
        // 稳定版发布后，alpha 用户应当被提示。
        assert_eq!(
            decide_update("0.1.0-alpha.0", Some("0.1.0")),
            UpdateDecision::UpdateAvailable {
                version: "0.1.0".to_owned()
            }
        );
    }

    #[test]
    fn decide_update_stays_quiet_when_nothing_is_newer() {
        assert_eq!(
            decide_update("0.1.0-alpha.0", Some("0.1.0-alpha.0")),
            UpdateDecision::UpToDate
        );
        assert_eq!(
            decide_update("0.2.0", Some("0.1.0")),
            UpdateDecision::UpToDate
        );
    }

    #[test]
    fn decide_update_does_not_push_prereleases_to_stable_users() {
        // 关键通道规则：稳定版用户不该被引导去装 alpha。
        assert_eq!(
            decide_update("0.1.0", Some("0.2.0-alpha.1")),
            UpdateDecision::UpToDate
        );
        // 但已经在预览通道的用户要能看到下一个 alpha。
        assert_eq!(
            decide_update("0.1.0-alpha.0", Some("0.2.0-alpha.1")),
            UpdateDecision::UpdateAvailable {
                version: "0.2.0-alpha.1".to_owned()
            }
        );
    }

    #[test]
    fn decide_update_is_unknown_when_information_is_missing_or_unusable() {
        // 拿不到最新版本。
        assert_eq!(
            decide_update("0.1.0-alpha.0", None),
            UpdateDecision::Unknown
        );
        // 版本号解析不了（含空串）。
        assert_eq!(
            decide_update("0.1.0", Some("not-a-version")),
            UpdateDecision::Unknown
        );
        assert_eq!(decide_update("", Some("0.2.0")), UpdateDecision::Unknown);
    }

    #[test]
    fn display_round_trips_through_parse() {
        for raw in ["0.1.0", "0.1.0-alpha.0", "1.2.3-beta.10"] {
            assert_eq!(version(raw).to_string(), raw);
        }
    }
}
