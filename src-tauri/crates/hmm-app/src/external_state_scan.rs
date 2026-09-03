//! 外部来源 MOD 的状态扫描服务（#286 切片 2）。
//!
//! 职责：**只读**地把「导入包（HMM 沙箱副本）」与「游戏目录」两侧的文件读出来，
//! 交给 `hmm-core::external_install_state` 判定。本模块不写任何文件、不改任何状态。
//!
//! ## 输入的两个来源都是 trait，因此可注入假实现
//!
//! - 沙箱侧：`ModPackageInstallFileReader::read_install_file`（`Err` = 读不到）
//! - 游戏目录侧：`InstallGameFileSystem::read_game_file`
//!   （`Ok(None)` = 文件不存在，`Err` = 读不到）
//!
//! 两侧的读失败都映射成「读不到」并**留在结果里**，结果与比对集一一对应、同序
//! （#305）。所以本模块可以在**没有真实文件系统**的情况下完整测试。
//!
//! ## 与安装路径保持一致
//!
//! 沙箱扫描可能返回不在 `allowed_roots` 里的 `target_path`。安装流程用
//! `is_installable_target_path` 把这类文件**过滤掉**，这里同样过滤——
//! 否则会出现「装不上但显示已安装」的荒谬结论。

use std::path::Path;
use std::sync::Arc;
use std::thread::available_parallelism;

use anyhow::Result;
use hmm_core::{
    installed_file_summary, summarize_external_install_state, ExternalExpectedSummary,
    ExternalFileObservation, ExternalInstallStateSummary, ExternalTargetPresence, InstallManifest,
    InstallTargetPath, ModId,
};
use hmm_ports::{
    CancellationToken, InstallGameFileSystem, ModPackageInstallFileReadRequest,
    ModPackageInstallFileReader, ModPackageInstallFileScanRequest, ModPackageInstallFileScanner,
};

/// 并行哈希的**上限**。
///
/// 哈希是「读 IO + CPU」混合负载：收益在 2~4 线程就饱和，机械盘上并行随机读
/// 反而更慢。所以取 `min(上限, CPU 数, 文件数)`，而不是有多少文件开多少线程。
pub const DEFAULT_WORKER_LIMIT: usize = 4;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ExternalStateScanError {
    #[error("external mod state scan was cancelled")]
    Cancelled,
    #[error("failed to scan the imported mod package")]
    PackageScanFailed,
}

pub struct ExternalStateScanRequest<'a> {
    pub package_id: &'a str,
    pub sandbox_root: &'a Path,
    /// 该游戏允许的安装根（如 `["nativePC"]`）。
    pub allowed_roots: &'a [String],
    pub cancellation_token: &'a dyn CancellationToken,
}

/// 一个已通过 `allowed_roots` 校验、可直接用于读取的目标文件。
///
/// 做成 owned 是为了让「准备」与「判定」能分开调用：外部 MOD 状态扫描要在两处
/// stat 之间做哈希，调用方需要在**没有锁**的时候持有这份列表（#286 切片 2b）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedExternalTarget {
    /// 归一化后的强类型目标路径，用于读取游戏目录侧。
    pub target_path: InstallTargetPath,
    /// 沙箱内的文件 ID，用于读取导入包副本。
    pub package_file_id: String,
    /// 原始目标路径。**排序与展示都用它**——它与 `target_path` 可能只差大小写
    /// 或分隔符，而展示必须是玩家在包里看到的那个字符串。
    pub display_path: String,
}

/// 只做「准备」所需的输入。
///
/// 与 `ExternalStateScanRequest` 分开，因为准备阶段不需要取消令牌也不需要
/// 游戏目录：它只读导入包沙箱。
pub struct ExternalStateScanPrepareRequest<'a> {
    pub package_id: &'a str,
    pub sandbox_root: &'a Path,
    /// 该游戏允许的安装根（如 `["nativePC"]`）。
    pub allowed_roots: &'a [String],
}

pub struct ExternalModStateScanService {
    scanner: Arc<dyn ModPackageInstallFileScanner>,
    package_reader: Arc<dyn ModPackageInstallFileReader>,
    game_fs: Arc<dyn InstallGameFileSystem>,
    max_file_bytes: u64,
    worker_limit: usize,
}

impl ExternalModStateScanService {
    pub fn new(
        scanner: Arc<dyn ModPackageInstallFileScanner>,
        package_reader: Arc<dyn ModPackageInstallFileReader>,
        game_fs: Arc<dyn InstallGameFileSystem>,
        max_file_bytes: u64,
        worker_limit: usize,
    ) -> Self {
        Self {
            scanner,
            package_reader,
            game_fs,
            max_file_bytes,
            worker_limit: worker_limit.max(1),
        }
    }

    /// 扫描并判定（一站式入口）。
    ///
    /// 等价于 [`Self::prepare_targets`] + [`Self::summarize_prepared`]。需要在两次
    /// stat 之间插入别的动作的调用方（如 `hmm-runtime` 的三段式加锁）应当直接用
    /// 那两个方法。
    ///
    /// 返回**按 `target_path` 排序**的结果：并行执行不改变输出顺序，
    /// 因此同样的事实永远得到同样的结果。
    pub fn scan(
        &self,
        request: ExternalStateScanRequest<'_>,
    ) -> Result<ExternalInstallStateSummary, ExternalStateScanError> {
        let token = request.cancellation_token;
        if token.is_cancelled() {
            return Err(ExternalStateScanError::Cancelled);
        }

        let prepared = self.prepare_targets(ExternalStateScanPrepareRequest {
            package_id: request.package_id,
            sandbox_root: request.sandbox_root,
            allowed_roots: request.allowed_roots,
        })?;

        if token.is_cancelled() {
            return Err(ExternalStateScanError::Cancelled);
        }

        self.summarize_prepared(&prepared, request.package_id, request.sandbox_root, token)
    }

    /// 只读导入包沙箱，产出**已排序**的目标文件列表。
    ///
    /// 不碰游戏目录、不读文件内容，因此可以在持有游戏写锁时安全调用——但正常用法
    /// 是在锁**外**调用，把哈希（长时间工作）留在锁外。
    pub fn prepare_targets(
        &self,
        request: ExternalStateScanPrepareRequest<'_>,
    ) -> Result<Vec<PreparedExternalTarget>, ExternalStateScanError> {
        let scanned = self
            .scanner
            .scan_install_files(ModPackageInstallFileScanRequest {
                package_id: request.package_id,
                sandbox_root: request.sandbox_root,
            })
            .map_err(|_| ExternalStateScanError::PackageScanFailed)?;

        // 与安装流程同口径：只保留真正会被装进去的文件。
        // `InstallTargetPath::parse` 同时承担了「过滤越界路径」与「产出强类型路径」两件事——
        // 曾在这里额外写过一层 `is_installable_target_path` 过滤，但它与 parse 判定完全相同，
        // 是冗余的（控制组证明：去掉它测试照样全绿）。`allowed_roots` 由调用方给出，
        // 不在这里写死任何游戏。
        let mut prepared: Vec<PreparedExternalTarget> = scanned
            .into_iter()
            .filter_map(|file| {
                let parsed =
                    InstallTargetPath::parse(&file.target_path, request.allowed_roots).ok()?;
                Some(PreparedExternalTarget {
                    target_path: parsed,
                    package_file_id: file.package_file_id,
                    display_path: file.target_path,
                })
            })
            .collect();
        // 排序放在并行之前，保证输出顺序确定（并行只影响速度，不影响次序）。
        prepared.sort_by(|left, right| left.display_path.cmp(&right.display_path));

        Ok(prepared)
    }

    /// 对已准备好的目标列表做有界并发哈希与判定。
    ///
    /// 这是长时间工作（读文件 + hash），**不得在游戏写锁内调用**。
    pub fn summarize_prepared(
        &self,
        prepared: &[PreparedExternalTarget],
        package_id: &str,
        sandbox_root: &Path,
        cancellation_token: &dyn CancellationToken,
    ) -> Result<ExternalInstallStateSummary, ExternalStateScanError> {
        if cancellation_token.is_cancelled() {
            return Err(ExternalStateScanError::Cancelled);
        }

        let observations =
            self.observe_targets(prepared, package_id, sandbox_root, cancellation_token)?;

        Ok(summarize_external_install_state(&observations))
    }

    /// 逐文件读取两侧摘要。**有界并发**，并按输入顺序回收结果。
    fn observe_targets<'a>(
        &self,
        targets: &'a [PreparedExternalTarget],
        package_id: &str,
        sandbox_root: &Path,
        token: &dyn CancellationToken,
    ) -> Result<Vec<ExternalFileObservation<'a>>, ExternalStateScanError> {
        if targets.is_empty() {
            return Ok(Vec::new());
        }

        let worker_count = self.worker_count_for(targets.len());
        let ranges = split_ranges(targets.len(), worker_count);

        // 每个 worker 处理一段连续区间，把结果（带原始下标）交回来。
        // `thread::scope` 可以借用当前栈，因此闭包不需要 'static。
        let mut collected: Vec<(usize, ExternalExpectedSummary, ExternalTargetPresence)> =
            Vec::with_capacity(targets.len());

        std::thread::scope(|scope| {
            let handles: Vec<_> = ranges
                .into_iter()
                .map(|range| {
                    scope.spawn(
                        move || -> Vec<(usize, ExternalExpectedSummary, ExternalTargetPresence)> {
                            let mut chunk = Vec::with_capacity(range.len());
                            for index in range {
                                if token.is_cancelled() {
                                    break;
                                }
                                let target = &targets[index];
                                let expected =
                                    self.expected_summary(target, package_id, sandbox_root);
                                let actual = self.read_target_presence(&target.target_path);
                                chunk.push((index, expected, actual));
                            }
                            chunk
                        },
                    )
                })
                .collect();

            for handle in handles {
                // worker panic 时不能假装没发生：缺了一段结果就会得出错误的判定，
                // 所以整次扫描失败（fail-closed）。
                match handle.join() {
                    Ok(chunk) => collected.extend(chunk),
                    Err(_) => return Err(ExternalStateScanError::PackageScanFailed),
                }
            }
            Ok(())
        })?;

        if token.is_cancelled() {
            return Err(ExternalStateScanError::Cancelled);
        }
        // 取消之外的缺项一律按失败处理，绝不用不完整的事实去判定。
        if collected.len() != targets.len() {
            return Err(ExternalStateScanError::PackageScanFailed);
        }

        collected.sort_by_key(|(index, _, _)| *index);

        // 与 `targets` 一一对应、同序。这里曾用 filter_map 把沙箱侧读失败的文件丢掉
        // （#305）：结果比 `prepared` 短一项，而调用方按位置配路径与占用者，被丢文件
        // 之后的每个状态都会配到错的文件上。读不到的文件必须留在原位、标成读不到。
        Ok(collected
            .into_iter()
            .map(|(index, expected, actual)| ExternalFileObservation {
                target_path: &targets[index].display_path,
                expected,
                actual,
            })
            .collect())
    }

    /// 读取游戏目录侧：不存在 / 读到了 / 读不到。
    fn read_target_presence(&self, target_path: &InstallTargetPath) -> ExternalTargetPresence {
        match self.game_fs.read_game_file(target_path) {
            Ok(None) => ExternalTargetPresence::Missing,
            Ok(Some(bytes)) => ExternalTargetPresence::Present(installed_file_summary(&bytes)),
            Err(_) => ExternalTargetPresence::Unreadable,
        }
    }

    /// 读取导入包（沙箱）侧摘要：读到了 / 读不到，与游戏目录侧对称。
    fn expected_summary(
        &self,
        target: &PreparedExternalTarget,
        package_id: &str,
        sandbox_root: &Path,
    ) -> ExternalExpectedSummary {
        match self
            .package_reader
            .read_install_file(ModPackageInstallFileReadRequest {
                package_id,
                sandbox_root,
                package_file_id: &hmm_core::PackageFileId::new(target.package_file_id.clone()),
                max_bytes: self.max_file_bytes,
            }) {
            Ok(bytes) => ExternalExpectedSummary::Available(installed_file_summary(&bytes)),
            Err(_) => ExternalExpectedSummary::Unreadable,
        }
    }

    fn worker_count_for(&self, file_count: usize) -> usize {
        let cpus = available_parallelism()
            .map(|value| value.get())
            .unwrap_or(1);
        self.worker_limit.min(cpus).min(file_count).max(1)
    }
}

/// 与 `prepared` 同序的「被其他 MOD 占用」归因（#286 第三层归因）。
///
/// 数据源是安装清单——它是「当前磁盘上这一路径归谁」的事实来源；归属视图取
/// `first_manifest_entry_by_target`（同一路径首条，见其文档对畸形态的说明）。
/// 被扫 MOD 自己名下的条目不算占用——正常门禁下未安装的 MOD 不该有条目，
/// 防御性排除是为了让语义在任何输入下都成立。清单缺席（该配置档从未有
/// HMM 安装）= 全部无占用。
///
/// 只做查表，不读文件。调用方负责在与指纹复验一致的锁窗口内取得清单快照，
/// 否则归因可能描述另一个时刻的磁盘。
pub fn claimed_by_other_mods(
    manifest: Option<&InstallManifest>,
    prepared: &[PreparedExternalTarget],
    scanned_mod_id: &ModId,
) -> Vec<Option<ModId>> {
    let Some(manifest) = manifest else {
        return vec![None; prepared.len()];
    };
    let owners = crate::install::first_manifest_entry_by_target(manifest);
    prepared
        .iter()
        .map(|target| {
            owners
                .get(&target.target_path)
                .filter(|entry| entry.mod_id != *scanned_mod_id)
                .map(|entry| entry.mod_id.clone())
        })
        .collect()
}

/// 把 `total` 切成 `parts` 段，尽量均匀；前面的段多分余数。
fn split_ranges(total: usize, parts: usize) -> Vec<std::ops::Range<usize>> {
    let parts = parts.max(1).min(total).max(1);
    let base = total / parts;
    let remainder = total % parts;
    let mut ranges = Vec::with_capacity(parts);
    let mut start = 0;
    for index in 0..parts {
        let size = base + usize::from(index < remainder);
        ranges.push(start..start + size);
        start += size;
    }
    ranges
}

#[cfg(test)]
mod tests {
    use super::*;
    use hmm_ports::{
        ModPackageInstallFile, ModPackageInstallFileScanError, ModPackageInstallFileScanRequest,
    };
    use std::collections::HashMap;
    use std::sync::Mutex;

    struct NeverCancelled;
    impl CancellationToken for NeverCancelled {
        fn is_cancelled(&self) -> bool {
            false
        }
    }

    struct Cancelled;
    impl CancellationToken for Cancelled {
        fn is_cancelled(&self) -> bool {
            true
        }
    }

    #[derive(Default)]
    struct FakeScanner {
        files: Vec<ModPackageInstallFile>,
        fail: bool,
    }

    impl ModPackageInstallFileScanner for FakeScanner {
        fn scan_install_files(
            &self,
            _request: ModPackageInstallFileScanRequest<'_>,
        ) -> Result<Vec<ModPackageInstallFile>, ModPackageInstallFileScanError> {
            if self.fail {
                return Err(ModPackageInstallFileScanError::DepthLimitExceeded);
            }
            Ok(self.files.clone())
        }
    }

    #[derive(Default)]
    struct FakePackageReader {
        bytes_by_id: HashMap<String, Vec<u8>>,
        /// 记录并发访问，用于验证「有界并发」真的发生了。
        concurrent_peak: Mutex<usize>,
        in_flight: Mutex<usize>,
    }

    impl FakePackageReader {
        fn with(entries: &[(&str, &[u8])]) -> Self {
            let mut bytes_by_id = HashMap::new();
            for (id, bytes) in entries {
                bytes_by_id.insert((*id).to_owned(), bytes.to_vec());
            }
            Self {
                bytes_by_id,
                ..Default::default()
            }
        }
    }

    impl ModPackageInstallFileReader for FakePackageReader {
        fn read_install_file(
            &self,
            request: ModPackageInstallFileReadRequest<'_>,
        ) -> Result<Vec<u8>> {
            let mut in_flight = self.in_flight.lock().expect("in-flight counter");
            *in_flight += 1;
            let mut peak = self.concurrent_peak.lock().expect("peak counter");
            *peak = (*peak).max(*in_flight);
            drop(peak);
            drop(in_flight);

            let result = self
                .bytes_by_id
                .get(request.package_file_id.as_str())
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("unknown package file"));
            let mut in_flight = self.in_flight.lock().expect("in-flight counter");
            *in_flight -= 1;
            result
        }
    }

    #[derive(Default)]
    struct FakeGameFs {
        /// `None` 表示读失败（权限/占用）。
        files: HashMap<String, Option<Vec<u8>>>,
    }

    impl InstallGameFileSystem for FakeGameFs {
        fn read_game_file(
            &self,
            target_path: &hmm_core::InstallTargetPath,
        ) -> anyhow::Result<Option<Vec<u8>>> {
            match self.files.get(target_path.as_str()) {
                None => Ok(None),
                Some(None) => Err(anyhow::anyhow!("locked")),
                Some(Some(bytes)) => Ok(Some(bytes.clone())),
            }
        }
        fn write_game_file(
            &self,
            _target_path: &hmm_core::InstallTargetPath,
            _bytes: &[u8],
        ) -> anyhow::Result<()> {
            unreachable!("扫描服务不得写文件")
        }
        fn remove_game_file(
            &self,
            _target_path: &hmm_core::InstallTargetPath,
        ) -> anyhow::Result<()> {
            unreachable!("扫描服务不得删除文件")
        }
    }

    fn file(id: &str, target: &str) -> ModPackageInstallFile {
        ModPackageInstallFile {
            package_file_id: id.to_owned(),
            target_path: target.to_owned(),
        }
    }

    // 注：上面的 `service` 里 reader 参数没被用上，是因为 Arc 需要具体类型；
    // 下面用 `service_with` 组装真正想要的 reader。
    fn service_with(
        scanner: Arc<dyn ModPackageInstallFileScanner>,
        reader: Arc<dyn ModPackageInstallFileReader>,
        game: Arc<dyn InstallGameFileSystem>,
        worker_limit: usize,
    ) -> ExternalModStateScanService {
        ExternalModStateScanService::new(scanner, reader, game, 8 * 1024 * 1024, worker_limit)
    }

    #[test]
    fn matching_files_are_reported_as_installed() {
        let scanner = FakeScanner {
            files: vec![file("a", "nativePC/wp/swo/swo035.mod3")],
            fail: false,
        };
        let reader = FakePackageReader::with(&[("a", b"same")]);
        let mut game = FakeGameFs::default();
        game.files.insert(
            "nativePC/wp/swo/swo035.mod3".to_owned(),
            Some(b"same".to_vec()),
        );

        let summary = service_with(Arc::new(scanner), Arc::new(reader), Arc::new(game), 4)
            .scan(ExternalStateScanRequest {
                package_id: "pkg",
                sandbox_root: Path::new("sandbox"),
                allowed_roots: &["nativePC".to_owned()],
                cancellation_token: &NeverCancelled,
            })
            .expect("scan succeeds");

        assert_eq!(summary.state, hmm_core::ExternalInstallState::Installed);
        assert_eq!(summary.matched_file_count, 1);
    }

    #[test]
    fn different_content_is_changed_not_installed() {
        let scanner = FakeScanner {
            files: vec![file("a", "nativePC/wp/swo/swo035.mod3")],
            fail: false,
        };
        let reader = FakePackageReader::with(&[("a", b"from-package")]);
        let mut game = FakeGameFs::default();
        game.files.insert(
            "nativePC/wp/swo/swo035.mod3".to_owned(),
            Some(b"from-game".to_vec()),
        );

        let summary = service_with(Arc::new(scanner), Arc::new(reader), Arc::new(game), 4)
            .scan(ExternalStateScanRequest {
                package_id: "pkg",
                sandbox_root: Path::new("sandbox"),
                allowed_roots: &["nativePC".to_owned()],
                cancellation_token: &NeverCancelled,
            })
            .expect("scan succeeds");

        assert_eq!(summary.state, hmm_core::ExternalInstallState::Changed);
    }

    #[test]
    fn missing_and_unreadable_are_distinguished() {
        let scanner = FakeScanner {
            files: vec![
                file("missing", "nativePC/a.mod3"),
                file("locked", "nativePC/b.mod3"),
            ],
            fail: false,
        };
        let reader = FakePackageReader::with(&[("missing", b"x"), ("locked", b"y")]);
        let mut game = FakeGameFs::default();
        // 只放一个「读失败」的文件，另一个保持缺失。
        game.files.insert("nativePC/b.mod3".to_owned(), None);

        let summary = service_with(Arc::new(scanner), Arc::new(reader), Arc::new(game), 4)
            .scan(ExternalStateScanRequest {
                package_id: "pkg",
                sandbox_root: Path::new("sandbox"),
                allowed_roots: &["nativePC".to_owned()],
                cancellation_token: &NeverCancelled,
            })
            .expect("scan succeeds");

        assert_eq!(summary.missing_file_count, 1);
        assert_eq!(summary.unreadable_file_count, 1);
        assert_eq!(summary.state, hmm_core::ExternalInstallState::Mixed);
    }

    #[test]
    fn an_unreadable_package_copy_stays_in_place_as_unreadable() {
        // #305：沙箱侧读失败的文件曾被 filter_map 丢掉——结果比比对集短一项，而调用方
        // 按位置配路径与占用者，被丢文件之后的状态全部错位。b 的沙箱副本读不到
        // （reader 里没有它），它必须留在第 2 位并标成读不到，c 仍在第 3 位。
        let scanner = FakeScanner {
            files: vec![
                file("a", "nativePC/a.mod3"),
                file("b", "nativePC/b.mod3"),
                file("c", "nativePC/c.mod3"),
            ],
            fail: false,
        };
        let reader = FakePackageReader::with(&[("a", b"same"), ("c", b"from-package")]);
        let mut game = FakeGameFs::default();
        game.files
            .insert("nativePC/a.mod3".to_owned(), Some(b"same".to_vec()));
        game.files
            .insert("nativePC/b.mod3".to_owned(), Some(b"whatever".to_vec()));
        game.files
            .insert("nativePC/c.mod3".to_owned(), Some(b"from-game".to_vec()));

        let summary = service_with(Arc::new(scanner), Arc::new(reader), Arc::new(game), 4)
            .scan(ExternalStateScanRequest {
                package_id: "pkg",
                sandbox_root: Path::new("sandbox"),
                allowed_roots: &["nativePC".to_owned()],
                cancellation_token: &NeverCancelled,
            })
            .expect("scan succeeds");

        assert_eq!(
            summary.files,
            vec![
                hmm_core::ExternalFileState::Matched,
                hmm_core::ExternalFileState::Unreadable,
                hmm_core::ExternalFileState::Changed,
            ],
            "结果必须与比对集等长、同序：读不到的文件留在原位"
        );
        assert_eq!(summary.unreadable_file_count, 1);
        assert_eq!(summary.state, hmm_core::ExternalInstallState::Mixed);
    }

    #[test]
    fn paths_outside_the_allowed_roots_are_ignored_like_install_does() {
        // 与安装同口径：装不进去的文件不该参与「是否已安装」的判定。
        let scanner = FakeScanner {
            files: vec![file("ok", "nativePC/a.mod3"), file("bad", "outside/a.mod3")],
            fail: false,
        };
        let reader = FakePackageReader::with(&[("ok", b"x"), ("bad", b"x")]);
        let mut game = FakeGameFs::default();
        game.files
            .insert("nativePC/a.mod3".to_owned(), Some(b"x".to_vec()));

        let summary = service_with(Arc::new(scanner), Arc::new(reader), Arc::new(game), 4)
            .scan(ExternalStateScanRequest {
                package_id: "pkg",
                sandbox_root: Path::new("sandbox"),
                allowed_roots: &["nativePC".to_owned()],
                cancellation_token: &NeverCancelled,
            })
            .expect("scan succeeds");

        assert_eq!(summary.matched_file_count, 1);
        assert_eq!(summary.files.len(), 1);
        assert_eq!(summary.state, hmm_core::ExternalInstallState::Installed);
    }

    #[test]
    fn results_follow_sorted_target_order_not_the_scan_order() {
        // 输入故意倒序：z 在前、a 在后。若实现没有排序，输出会是 [Changed, Matched]；
        // 排序后应为 [Matched(a), Missing(z)]。
        let scanner = FakeScanner {
            files: vec![file("z", "nativePC/z.mod3"), file("a", "nativePC/a.mod3")],
            fail: false,
        };
        let reader = FakePackageReader::with(&[("z", b"z"), ("a", b"a")]);
        let mut game = FakeGameFs::default();
        // z 在游戏目录里是另一份内容；a 完全缺失。
        game.files
            .insert("nativePC/z.mod3".to_owned(), Some(b"other".to_vec()));

        let summary = service_with(Arc::new(scanner), Arc::new(reader), Arc::new(game), 4)
            .scan(ExternalStateScanRequest {
                package_id: "pkg",
                sandbox_root: Path::new("sandbox"),
                allowed_roots: &["nativePC".to_owned()],
                cancellation_token: &NeverCancelled,
            })
            .expect("scan succeeds");

        // 顺序本身就是契约：调用方按位置展示明细，顺序变了界面就错位。
        assert_eq!(
            summary.files,
            vec![
                hmm_core::ExternalFileState::Missing,
                hmm_core::ExternalFileState::Changed
            ]
        );
        assert_eq!(summary.missing_file_count, 1);
        assert_eq!(summary.changed_file_count, 1);
    }

    #[test]
    fn cancellation_before_reading_fails_instead_of_returning_a_partial_truth() {
        let scanner = FakeScanner {
            files: vec![file("a", "nativePC/a.mod3")],
            fail: false,
        };
        let reader = FakePackageReader::with(&[("a", b"x")]);
        let mut game = FakeGameFs::default();
        game.files
            .insert("nativePC/a.mod3".to_owned(), Some(b"x".to_vec()));

        let error = service_with(Arc::new(scanner), Arc::new(reader), Arc::new(game), 4)
            .scan(ExternalStateScanRequest {
                package_id: "pkg",
                sandbox_root: Path::new("sandbox"),
                allowed_roots: &["nativePC".to_owned()],
                cancellation_token: &Cancelled,
            })
            .expect_err("取消必须失败");

        assert_eq!(error, ExternalStateScanError::Cancelled);
    }

    #[test]
    fn a_failing_package_scan_is_reported() {
        let scanner = FakeScanner {
            files: vec![],
            fail: true,
        };
        let error = service_with(
            Arc::new(scanner),
            Arc::new(FakePackageReader::default()),
            Arc::new(FakeGameFs::default()),
            4,
        )
        .scan(ExternalStateScanRequest {
            package_id: "pkg",
            sandbox_root: Path::new("sandbox"),
            allowed_roots: &["nativePC".to_owned()],
            cancellation_token: &NeverCancelled,
        })
        .expect_err("扫描失败必须上报");

        assert_eq!(error, ExternalStateScanError::PackageScanFailed);
    }

    #[test]
    fn worker_count_is_bounded_by_the_limit_cpus_and_file_count() {
        // 有界并发：不能因为文件多就无限开线程。
        assert_eq!(split_ranges(10, 4).len(), 4);
        // 文件数少于上限时不开空线程。
        assert_eq!(split_ranges(2, 4).len(), 2);
        // 切分必须覆盖全部下标且不重叠。
        let ranges = split_ranges(7, 3);
        let covered: Vec<usize> = ranges.iter().flat_map(|range| range.clone()).collect();
        assert_eq!(covered, vec![0, 1, 2, 3, 4, 5, 6]);
    }

    // ---- 第三层归因（#286）：claimed_by_other_mods ----

    fn prepared_target(relative: &str) -> PreparedExternalTarget {
        let roots = vec!["nativePC".to_owned()];
        PreparedExternalTarget {
            target_path: InstallTargetPath::parse(relative, &roots).expect("合法目标路径"),
            package_file_id: relative.to_owned(),
            display_path: relative.to_owned(),
        }
    }

    fn manifest_entry_for(
        target: &InstallTargetPath,
        mod_id: &str,
    ) -> hmm_core::InstallManifestEntry {
        hmm_core::InstallManifestEntry {
            target_path: target.clone(),
            mod_id: ModId::new(mod_id),
            revision_id: None,
            package_file_id: hmm_core::PackageFileId::new("pkg/file"),
            layer: hmm_core::FileLayer::new("base", 0),
            backup_ref: None,
            installed_file: None,
            adopted: false,
        }
    }

    #[test]
    fn attribution_reports_other_mod_claims_and_skips_self_and_unowned() {
        let claimed = prepared_target("nativePC/wp/one/one001.mod3");
        let self_owned = prepared_target("nativePC/wp/one/one001.mrl3");
        let unowned = prepared_target("nativePC/wp/one/one001.tex");
        let manifest = InstallManifest::completed(
            hmm_core::ProfileId::new("default"),
            vec![
                manifest_entry_for(&claimed.target_path, "mod-flat"),
                manifest_entry_for(&self_owned.target_path, "mod-scanned"),
            ],
        );

        let claims = claimed_by_other_mods(
            Some(&manifest),
            &[claimed, self_owned, unowned],
            &ModId::new("mod-scanned"),
        );

        // 与 prepared 同序：他主条目报占用者，自己名下与无主路径为 None。
        assert_eq!(claims, vec![Some(ModId::new("mod-flat")), None, None],);
    }

    #[test]
    fn attribution_without_manifest_reports_no_claims() {
        let prepared = vec![
            prepared_target("nativePC/wp/a.mod3"),
            prepared_target("nativePC/wp/b.mod3"),
        ];

        let claims = claimed_by_other_mods(None, &prepared, &ModId::new("mod-scanned"));

        // 清单缺席 = 该配置档从未有 HMM 安装：长度仍与 prepared 对齐，全为 None。
        assert_eq!(claims, vec![None, None]);
    }

    #[test]
    fn attribution_uses_the_first_manifest_entry_per_target() {
        let target = prepared_target("nativePC/wp/a.mod3");
        // 同一路径两条异主条目属于畸形态；归因按首条报告（见
        // first_manifest_entry_by_target 的口径说明），不做修复判定。
        let manifest = InstallManifest::completed(
            hmm_core::ProfileId::new("default"),
            vec![
                manifest_entry_for(&target.target_path, "mod-first"),
                manifest_entry_for(&target.target_path, "mod-second"),
            ],
        );

        let claims = claimed_by_other_mods(Some(&manifest), &[target], &ModId::new("mod-scanned"));

        assert_eq!(claims, vec![Some(ModId::new("mod-first"))]);
    }
}
