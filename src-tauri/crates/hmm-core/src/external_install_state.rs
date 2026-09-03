//! 外部来源 MOD 的安装状态判定（#286）。
//!
//! ## 为什么需要这个模块
//!
//! HMM 原本的「已安装」语义是「**我装的**」——看安装清单里有没有记录。
//! 单一工具场景下它与「游戏里有没有」等价；但第三方管理器（狩技盒子）导入的 MOD
//! 在 HMM 清单里没有记录，于是 `scan_mod` 直接返回 `NotInstalled`，
//! **从不进入扫描环节**——哪怕文件确实躺在游戏目录里。
//!
//! 本模块把判定口径换成「**游戏目录里有没有、是否与导入包一致**」。
//!
//! ## 判定口径（两条硬要求）
//!
//! 1. **不能只看文件是否存在**。反例：两个太刀 MOD 都装 `swo035.mod3`，
//!    狩技盒子装了 A、HMM 装了 B——只判存在会把 A 误判成已安装。
//!    所以必须比对**内容摘要**（size + sha256）。
//! 2. **读不到的文件不能算「一致」，也不能算「缺失」**。它单独成一类，
//!    界面必须呈现——一个读不到的文件很可能就是被改动过的那个。
//!    这条对**两侧**都成立：导入包沙箱副本读不到时同样没有比对基准（#305），
//!    该文件必须留在结果里并标成读不到，而不是从列表里消失。
//!
//! ## 确定性
//!
//! 本模块**不排序**：调用方给什么顺序，就按什么顺序返回每文件状态。
//! 调用方（未来的 IO 层）负责按 `target_path` 排序，保证同样的事实永远得到同样的输出。

use crate::install::InstalledFileSummary;
use sha2::{Digest, Sha256};

/// 由文件内容算出安装摘要。
///
/// **这是新代码应当用的那一份**。仓库里另有 3 份私有副本
/// （`hmm-app/src/install.rs`、`install_recovery.rs`、`reinstall.rs`），
/// 它们逻辑相同但因为是各模块私有的而无法复用。本函数放在
/// `InstalledFileSummary` 所在的 crate，供新代码使用；
/// 那 3 份的收敛属于独立的清理，不在 #286 范围内。
pub fn installed_file_summary(bytes: &[u8]) -> InstalledFileSummary {
    let digest = Sha256::digest(bytes);
    InstalledFileSummary {
        size_bytes: bytes.len() as u64,
        sha256: hex_encode(&digest),
    }
}

fn hex_encode(bytes: &[u8]) -> String {
    use std::fmt::Write;
    bytes.iter().fold(
        String::with_capacity(bytes.len() * 2),
        |mut output, byte| {
            // 写入 String 不会失败，这里没有可恢复的错误路径。
            let _ = write!(output, "{byte:02x}");
            output
        },
    )
}

/// 游戏目录里某个目标文件的观测结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExternalTargetPresence {
    /// 文件不存在。
    Missing,
    /// 文件存在且读到了摘要。
    Present(InstalledFileSummary),
    /// 文件存在但读不到（权限、占用、IO 错误）。**不是缺失，也不是一致。**
    Unreadable,
}

/// 导入包（HMM 沙箱副本）里某个目标文件的观测结果。
///
/// 与 [`ExternalTargetPresence`] 对称，但没有「缺失」变体：能进比对集的文件都是
/// 沙箱扫描刚刚列出来的，读不到只可能是 IO 错误、超出大小上限或副本已损坏。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExternalExpectedSummary {
    /// 读到了摘要，可作比对基准。
    Available(InstalledFileSummary),
    /// 沙箱副本读不到。**没有基准就没有比对**（#305）。
    Unreadable,
}

/// 单个目标文件在「导入包（HMM 沙箱副本）」与「游戏目录」之间的比对结果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExternalFileState {
    /// 游戏目录中的文件与导入包一致。
    Matched,
    /// 游戏目录中没有这个文件。
    Missing,
    /// 文件存在，但内容与导入包不同。
    Changed,
    /// 任一侧读不到（游戏目录文件读失败，或导入包沙箱副本读失败），无法判定。
    /// **不能并入上面任何一类。**
    Unreadable,
}

/// 一次比对所需的输入：`expected` 来自导入包，`actual` 来自游戏目录。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalFileObservation<'a> {
    /// 仅用于诊断与展示，不参与判定。
    pub target_path: &'a str,
    /// 导入包（沙箱副本）里该文件的摘要，或读不到。
    pub expected: ExternalExpectedSummary,
    /// 游戏目录里该文件的实际状态。
    pub actual: ExternalTargetPresence,
}

/// 聚合后的安装状态。
///
/// 取值互斥，且**只描述事实，不含建议**（建议与决定归界面与用户）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExternalInstallState {
    /// 全部文件都在且一致。
    Installed,
    /// 只有缺失，没有改动、没有读不到的。
    Partial,
    /// 只有改动，没有缺失、没有读不到的。
    Changed,
    /// 缺失与改动同时存在；**或存在任何读不到的文件**。
    Mixed,
    /// 全部文件都缺失。
    NotInstalled,
    /// 没有可比对的文件（导入包里没有可安装文件）。
    Unknown,
}

/// 聚合结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalInstallStateSummary {
    pub state: ExternalInstallState,
    pub matched_file_count: usize,
    pub missing_file_count: usize,
    pub changed_file_count: usize,
    pub unreadable_file_count: usize,
    /// 与输入**同序**的每文件结果，供界面展示明细。
    pub files: Vec<ExternalFileState>,
}

/// 判定单个文件。
pub fn classify_external_file(observation: &ExternalFileObservation<'_>) -> ExternalFileState {
    let expected = match &observation.expected {
        ExternalExpectedSummary::Available(expected) => expected,
        // 沙箱副本读不到就没有比对基准：游戏侧在位也判不出一致/改动；游戏侧缺失也
        // 不下「缺失」结论——这次比对本身不完整，不能拿单侧观测去猜（#305）。
        ExternalExpectedSummary::Unreadable => return ExternalFileState::Unreadable,
    };
    match &observation.actual {
        ExternalTargetPresence::Missing => ExternalFileState::Missing,
        // 读不到就是读不到，不能拿 expected 去猜。
        ExternalTargetPresence::Unreadable => ExternalFileState::Unreadable,
        ExternalTargetPresence::Present(actual) => {
            if actual == expected {
                ExternalFileState::Matched
            } else {
                ExternalFileState::Changed
            }
        }
    }
}

/// 汇总一组文件的比对结果。
///
/// 空输入返回 `Unknown`——「没有可比对的文件」与「全都缺失」不是一回事，
/// 混在一起会让界面把「这个包装不出任何文件」说成「未安装」。
pub fn summarize_external_install_state(
    observations: &[ExternalFileObservation<'_>],
) -> ExternalInstallStateSummary {
    let files: Vec<ExternalFileState> = observations.iter().map(classify_external_file).collect();

    let matched_file_count = files
        .iter()
        .filter(|state| **state == ExternalFileState::Matched)
        .count();
    let missing_file_count = files
        .iter()
        .filter(|state| **state == ExternalFileState::Missing)
        .count();
    let changed_file_count = files
        .iter()
        .filter(|state| **state == ExternalFileState::Changed)
        .count();
    let unreadable_file_count = files
        .iter()
        .filter(|state| **state == ExternalFileState::Unreadable)
        .count();

    if files.is_empty() {
        return ExternalInstallStateSummary {
            state: ExternalInstallState::Unknown,
            matched_file_count,
            missing_file_count,
            changed_file_count,
            unreadable_file_count,
            files,
        };
    }

    let has_missing = missing_file_count > 0;
    let has_changed = changed_file_count > 0;
    let has_unreadable = unreadable_file_count > 0;

    let state = match (has_missing, has_changed, has_unreadable) {
        // 全部缺失。
        (true, false, false) if missing_file_count == files.len() => {
            ExternalInstallState::NotInstalled
        }
        // 全部一致。
        (false, false, false) => ExternalInstallState::Installed,
        // 只有缺失。
        (true, false, false) => ExternalInstallState::Partial,
        // 只有改动。
        (false, true, false) => ExternalInstallState::Changed,
        // 缺失与改动并存，或存在读不到的文件——后者意味着这次判定本身并不完整，
        // 不能报成「已安装」或单纯的某一类。
        _ => ExternalInstallState::Mixed,
    };

    ExternalInstallStateSummary {
        state,
        matched_file_count,
        missing_file_count,
        changed_file_count,
        unreadable_file_count,
        files,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn digest(size: u64, hash: &str) -> InstalledFileSummary {
        InstalledFileSummary {
            size_bytes: size,
            sha256: hash.to_owned(),
        }
    }

    fn observation<'a>(
        path: &'a str,
        expected: InstalledFileSummary,
        actual: ExternalTargetPresence,
    ) -> ExternalFileObservation<'a> {
        ExternalFileObservation {
            target_path: path,
            expected: ExternalExpectedSummary::Available(expected),
            actual,
        }
    }

    /// 沙箱副本读不到的观测（#305）。
    fn unreadable_package_copy(
        path: &str,
        actual: ExternalTargetPresence,
    ) -> ExternalFileObservation<'_> {
        ExternalFileObservation {
            target_path: path,
            expected: ExternalExpectedSummary::Unreadable,
            actual,
        }
    }

    #[test]
    fn installed_file_summary_matches_size_and_content() {
        let summary = installed_file_summary(b"abc");
        assert_eq!(summary.size_bytes, 3);
        // SHA-256("abc") 的已知常量，防止有人改了算法或编码方式。
        assert_eq!(
            summary.sha256,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        // 空文件也要有确定的摘要，不能走「缺失」的分支。
        assert_eq!(installed_file_summary(b"").size_bytes, 0);
    }

    fn matched(path: &str) -> ExternalFileObservation<'_> {
        let summary = digest(10, "aa");
        observation(
            path,
            summary.clone(),
            ExternalTargetPresence::Present(summary),
        )
    }

    #[test]
    fn all_files_matching_is_installed() {
        let observations = vec![matched("a.mod3"), matched("b.mrl3")];
        let summary = summarize_external_install_state(&observations);

        assert_eq!(summary.state, ExternalInstallState::Installed);
        assert_eq!(summary.matched_file_count, 2);
        assert_eq!(summary.missing_file_count, 0);
        assert_eq!(summary.changed_file_count, 0);
    }

    #[test]
    fn every_file_missing_is_not_installed() {
        let observations = vec![
            observation("a.mod3", digest(10, "aa"), ExternalTargetPresence::Missing),
            observation("b.mrl3", digest(20, "bb"), ExternalTargetPresence::Missing),
        ];
        let summary = summarize_external_install_state(&observations);

        assert_eq!(summary.state, ExternalInstallState::NotInstalled);
        assert_eq!(summary.missing_file_count, 2);
    }

    #[test]
    fn some_files_missing_is_partial() {
        let observations = vec![
            matched("a.mod3"),
            observation("b.mrl3", digest(20, "bb"), ExternalTargetPresence::Missing),
        ];
        let summary = summarize_external_install_state(&observations);

        assert_eq!(summary.state, ExternalInstallState::Partial);
        assert_eq!(summary.matched_file_count, 1);
        assert_eq!(summary.missing_file_count, 1);
        assert_eq!(summary.changed_file_count, 0);
    }

    #[test]
    fn changed_content_is_detected_even_when_the_size_matches() {
        // 这条是「不能只看文件是否存在」的延伸：**连 size 相同都不能算一致**。
        // 两个不同 MOD 装同一个 target_path 时，size 完全可能碰巧相同。
        let observations = vec![observation(
            "swo035.mod3",
            digest(136_000, "aa"),
            ExternalTargetPresence::Present(digest(136_000, "bb")),
        )];
        let summary = summarize_external_install_state(&observations);

        assert_eq!(summary.state, ExternalInstallState::Changed);
        assert_eq!(summary.changed_file_count, 1);
        assert_eq!(summary.files, vec![ExternalFileState::Changed]);
    }

    #[test]
    fn a_different_size_is_changed_without_relying_on_the_hash() {
        let observations = vec![observation(
            "a.mod3",
            digest(10, "aa"),
            ExternalTargetPresence::Present(digest(11, "aa")),
        )];
        assert_eq!(
            summarize_external_install_state(&observations).state,
            ExternalInstallState::Changed
        );
    }

    #[test]
    fn missing_and_changed_together_is_mixed() {
        let observations = vec![
            matched("a.mod3"),
            observation("b.mrl3", digest(20, "bb"), ExternalTargetPresence::Missing),
            observation(
                "c.mod3",
                digest(30, "cc"),
                ExternalTargetPresence::Present(digest(30, "zz")),
            ),
        ];
        let summary = summarize_external_install_state(&observations);

        assert_eq!(summary.state, ExternalInstallState::Mixed);
        assert_eq!(summary.missing_file_count, 1);
        assert_eq!(summary.changed_file_count, 1);
        assert_eq!(summary.matched_file_count, 1);
    }

    #[test]
    fn an_unreadable_file_is_never_reported_as_a_clean_state() {
        // 读不到 ≠ 缺失、≠ 一致。哪怕其余文件全部匹配，也不能报「已安装」——
        // 那个读不到的文件很可能正是被改动过的。
        let observations = vec![
            matched("a.mod3"),
            observation(
                "b.mrl3",
                digest(20, "bb"),
                ExternalTargetPresence::Unreadable,
            ),
        ];
        let summary = summarize_external_install_state(&observations);

        assert_eq!(summary.state, ExternalInstallState::Mixed);
        assert_eq!(summary.unreadable_file_count, 1);
        assert_eq!(summary.matched_file_count, 1);
    }

    #[test]
    fn an_unreadable_package_copy_is_unreadable_even_when_the_game_file_is_present() {
        // #305：沙箱副本读不到 = 没有比对基准。游戏目录里那份读到了也判不出一致/改动，
        // 更不能因为「有文件」就报 Matched。
        let observations = vec![
            matched("a.mod3"),
            unreadable_package_copy("b.mrl3", ExternalTargetPresence::Present(digest(20, "bb"))),
        ];
        let summary = summarize_external_install_state(&observations);

        assert_eq!(
            summary.files,
            vec![ExternalFileState::Matched, ExternalFileState::Unreadable]
        );
        assert_eq!(summary.unreadable_file_count, 1);
        assert_eq!(summary.changed_file_count, 0);
        assert_eq!(summary.state, ExternalInstallState::Mixed);
    }

    #[test]
    fn an_unreadable_package_copy_is_not_reported_as_missing() {
        // #305：游戏目录侧缺失是单侧观测，沙箱侧读不到时这次比对本身不完整——
        // 报「缺失」会让聚合态落成干净的 NotInstalled/Partial，掩盖沙箱副本已损坏的事实。
        let observations = vec![unreadable_package_copy(
            "a.mod3",
            ExternalTargetPresence::Missing,
        )];
        let summary = summarize_external_install_state(&observations);

        assert_eq!(summary.files, vec![ExternalFileState::Unreadable]);
        assert_eq!(summary.missing_file_count, 0);
        assert_eq!(summary.unreadable_file_count, 1);
        assert_eq!(summary.state, ExternalInstallState::Mixed);
    }

    #[test]
    fn an_empty_package_is_unknown_not_not_installed() {
        // 「包里没有可比对的文件」与「全都缺失」是两回事，不能混。
        let summary = summarize_external_install_state(&[]);
        assert_eq!(summary.state, ExternalInstallState::Unknown);
        assert!(summary.files.is_empty());
    }

    #[test]
    fn results_follow_the_input_order_so_the_caller_owns_determinism() {
        let observations = vec![
            matched("b.mrl3"),
            observation("a.mod3", digest(10, "aa"), ExternalTargetPresence::Missing),
        ];
        let summary = summarize_external_install_state(&observations);

        // 不做排序：调用方负责排序，这里只保证顺序被原样保留。
        assert_eq!(
            summary.files,
            vec![ExternalFileState::Matched, ExternalFileState::Missing]
        );
    }
}
