//! 外部来源 MOD 状态扫描的 runtime 装配（#286 切片 2b）。
//!
//! ## 为什么单独一层
//!
//! `hmm-app::external_state_scan` 是纯服务：所有输入都是 trait，可在没有真实
//! 文件系统的情况下测试。但「从 `mod_id` 解出 `package_id` 和沙箱根」「在拿锁前后
//! 各 stat 一次」这些事需要真实的 repository、游戏配置和写锁——本模块负责把它们
//! 装配起来，**判定逻辑一行都不写**。
//!
//! ## 三段式加锁（本条的核心，不能照抄 `ConfiguredInstallRecoveryScanner`）
//!
//! 模板 `scan()` 拿写锁扫全程，是因为它读清单条目——有界的小工作量。
//! 本条要哈希 33+ 个文件（可能几百 MB），而项目有 5 处文档明令禁止在写锁内做长
//! 时间 hash：
//!
//! | 文档 | 原文 |
//! |---|---|
//! | `ARCHITECTURE.md:616` | 不要在持有游戏写锁时做长时间解压或 hash |
//! | `AUTONOMOUS_ITERATION_ROADMAP.md:76` | scan/hash/extract/analyze 不持有写锁 |
//! | `CORE_MOD_LIFECYCLE_PRODUCTIZATION_PLAN.md:100` | 长时间扫描、hash、解压和分析不在持有写锁时执行 |
//! | `BATCH_MOD_LIFECYCLE_DESIGN.md:330` | 锁内只做有界 revalidation，不在锁内重新执行长时间工作 |
//! | `BATCH_MOD_LIFECYCLE_DESIGN.md:515` | 不能持有 game/profile 写锁执行整个 batch |
//!
//! 采用 `BATCH_MOD_LIFECYCLE_DESIGN.md:330` 的答案——**锁内只做有界校验，长活在锁外**：
//!
//! ```text
//! stage 0  锁外   解析 mod_id → package_id → 沙箱根 → 扫描 + 过滤 + 排序
//! stage 1  锁内   admission + try_lock → stat 出指纹 → 释放锁
//! stage 2  锁外   有界并发哈希两侧 → 判定
//! stage 3  锁内   重新 stat 比对 → 漂移则丢弃结果报 Stale
//! ```
//!
//! ### 为什么用 `try_lock` 而不是模板的 `lock()`
//!
//! 「有写入进行中 → 报 `stale`」这个降级语义**只有在 `try_lock` 下才可达**。
//! 用阻塞 `lock()` 的话，安装进行中时本扫描会卡住等待，`Stale` 分支成为死代码
//! ——而「写了护栏却永远走不到」正是本项目反复强调的假绿。
//!
//! ## 降级语义（fail-closed）
//!
//! - 拿不到 admission（`Busy` / `Unavailable`）→ `Stale`，**不是硬失败**
//! - 有写入进行中（`try_lock` 失败）→ `Stale`
//! - 扫描期间文件被改动 → `Stale` 且**丢弃本次结果**
//!
//! `Stale` 只表示「事实可能变了」。用户主动取消是 `Cancelled`，调用方编程错误
//! （`OrderViolation`）是硬失败——三者**不混用**，否则界面会把「已取消」说成
//! 「结果可能过期」。

use std::sync::Arc;

use hmm_app::external_state_scan::{
    ExternalModStateScanService, ExternalStateScanError, ExternalStateScanPrepareRequest,
    PreparedExternalTarget,
};
use hmm_app::GameProfileWriteLockRegistry;
use hmm_core::{ExternalInstallStateSummary, GameId, ModId, ProfileId};
use hmm_games_mhw::MHW_WEAPON_BINARY_MAX_BYTES;
use hmm_infra::{FileSystemInstallGameFileSystem, SandboxModPackageInstallFileScanner};
use hmm_ports::{
    AppClock, CancellationToken, CrossProcessWriteAdmissionError, GameConfigRepository,
    GameFileFingerprint, InstallGameFileInspector, InstallGameFileSystem,
    ModImportResultRepository, ModImportSandboxLocator, ModPackageInstallFileReader,
    ModPackageInstallFileScanner,
};
use std::collections::HashMap;
use std::sync::Mutex;

/// 扫描失败的原因。
///
/// 刻意**不含**路径、游戏目录、包名等敏感信息——错误码会经由 command 到达前端。
///
/// 与 `hmm-runtime` 其它错误枚举同口径：手写 `code()`，不实现 `Display`。
/// 上层只按稳定码取词，不拼接错误文本。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfiguredExternalStateScanError {
    /// 游戏目录未配置或读取失败。
    GameInstanceUnavailable,
    /// 该 `mod_id` 在导入记录里不存在，或记录读不出来。
    ModUnavailable,
    /// 沙箱根解析失败（导入记录存在但沙箱已被回收）。
    SandboxUnavailable,
    /// 沙箱扫描失败。
    ///
    /// 与 `hmm-app` 的同名错误区分开：runtime 层需要单独表达「准备工作就没做完」，
    /// 以便上层把它与「做完了但被判定为 stale」分开呈现。
    PackageScanFailed,
    /// 游戏目录侧文件 stat 失败（权限、符号链接、路径穿越）。
    ///
    /// 与沙箱侧失败分开：这里失败意味着**连基线都建不起来**，既不是包的问题
    /// 也不是结果过期，必须如实区分，否则排查时会往错误的方向查。
    GameFileUnavailable,
    /// 用户主动取消了扫描。
    ///
    /// **不是** `Stale`：取消是用户的意图，不是「事实可能变了」。
    Cancelled,
    /// 底层扫描服务读取失败（既非取消、也非准备失败）。
    ScanUnavailable,
    /// 跨进程写入准入的获取顺序被违反。
    ///
    /// **这是调用方 bug，不是运行时状态**，因此不做降级。把它并进 `Stale` 会把
    /// 编程错误伪装成「数据可能过期」，永远查不出来。
    WriteAdmissionOrderViolation,
    /// 结果可能已过期：拿不到准入、有写入进行中、或扫描期间文件被改动。
    ///
    /// 调用方应当**保留并展示上一次的结果**，而不是当作失败清空——这是维护者在
    /// issue #286 里拍板的降级口径。
    Stale,
}

impl ConfiguredExternalStateScanError {
    /// 稳定错误码，供 command 与前端取词使用。
    pub const fn code(self) -> &'static str {
        match self {
            Self::GameInstanceUnavailable => "external_state_scan_game_instance_unavailable",
            Self::ModUnavailable => "external_state_scan_mod_unavailable",
            Self::SandboxUnavailable => "external_state_scan_sandbox_unavailable",
            Self::PackageScanFailed => "external_state_scan_package_scan_failed",
            Self::GameFileUnavailable => "external_state_scan_game_file_unavailable",
            Self::Cancelled => "external_state_scan_cancelled",
            Self::ScanUnavailable => "external_state_scan_unavailable",
            Self::WriteAdmissionOrderViolation => "external_state_scan_admission_order_violation",
            Self::Stale => "external_state_scan_stale",
        }
    }
}

/// 一次成功扫描的产物：判定结果 + 复核所需的指纹与目标列表。
type SuccessfulExternalStateScan = (
    ExternalInstallStateSummary,
    Vec<PreparedExternalTarget>,
    Vec<Option<GameFileFingerprint>>,
);

pub struct ConfiguredExternalStateScanRequest<'a> {
    pub game_id: &'a GameId,
    pub profile_id: &'a ProfileId,
    pub mod_id: &'a ModId,
    pub cancellation_token: &'a dyn CancellationToken,
}

/// 外部 MOD 状态扫描器。
///
/// 构造只收**长生命周期**依赖（与 `ConfiguredInstallRecoveryScanner` 同范式）。
/// `game_fs` / inspector **不存字段**——它们依赖 `game_instance.root_dir`，
/// 必须按当次加载的游戏目录构造，否则游戏目录改了会用到旧路径。
pub struct ConfiguredExternalStateScanner {
    game_config_repository: Arc<dyn GameConfigRepository>,
    mod_import_result_repository: Arc<dyn ModImportResultRepository>,
    sandbox_locator: Arc<dyn ModImportSandboxLocator>,
    allowed_roots: Vec<String>,
    write_locks: Arc<GameProfileWriteLockRegistry>,
    scanner: Arc<dyn ModPackageInstallFileScanner>,
    package_reader: Arc<dyn ModPackageInstallFileReader>,
    game_fs_factory: Arc<dyn GameFileSystemFactory>,
    cache: Arc<ExternalStateScanCache>,
    max_file_bytes: u64,
    worker_limit: usize,
}

impl ConfiguredExternalStateScanner {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        game_config_repository: Arc<dyn GameConfigRepository>,
        mod_import_result_repository: Arc<dyn ModImportResultRepository>,
        sandbox_locator: Arc<dyn ModImportSandboxLocator>,
        allowed_roots: Vec<String>,
        write_locks: Arc<GameProfileWriteLockRegistry>,
        scanner: Arc<dyn ModPackageInstallFileScanner>,
        package_reader: Arc<dyn ModPackageInstallFileReader>,
        game_fs_factory: Arc<dyn GameFileSystemFactory>,
        cache: Arc<ExternalStateScanCache>,
        max_file_bytes: u64,
        worker_limit: usize,
    ) -> Self {
        Self {
            game_config_repository,
            mod_import_result_repository,
            sandbox_locator,
            allowed_roots,
            write_locks,
            scanner,
            package_reader,
            game_fs_factory,
            cache,
            max_file_bytes,
            worker_limit: worker_limit.max(1),
        }
    }

    /// 用真实文件系统装配。
    ///
    /// 单文件上限复用 MHW 武器二进制的既有上界（256 MiB），而不是自造一个：
    /// 扫描与安装对「一个 MOD 文件能有多大」应当有同一口径。它不是总量上限——
    /// `read_install_file` 逐文件读并立即收敛成 32 字节摘要，峰值只有单文件大小。
    pub fn with_real_filesystem(
        game_config_repository: Arc<dyn GameConfigRepository>,
        mod_import_result_repository: Arc<dyn ModImportResultRepository>,
        sandbox_locator: Arc<dyn ModImportSandboxLocator>,
        allowed_roots: Vec<String>,
        write_locks: Arc<GameProfileWriteLockRegistry>,
        clock: Arc<dyn AppClock>,
    ) -> Self {
        let scanner: Arc<dyn ModPackageInstallFileScanner> =
            Arc::new(SandboxModPackageInstallFileScanner);
        // 同一个对象同时实现「扫描」与「读取」两个 trait：沙箱侧的扫描结果
        // 与读取路径必须出自同一套判定（#284 的内容根解析）。
        let package_reader: Arc<dyn ModPackageInstallFileReader> =
            Arc::new(SandboxModPackageInstallFileScanner);
        Self::new(
            game_config_repository,
            mod_import_result_repository,
            sandbox_locator,
            allowed_roots,
            write_locks,
            scanner,
            package_reader,
            Arc::new(RealGameFileSystemFactory),
            Arc::new(ExternalStateScanCache::new(clock)),
            MHW_WEAPON_BINARY_MAX_BYTES as u64,
            hmm_app::external_state_scan::DEFAULT_WORKER_LIMIT,
        )
    }

    /// 执行三段式扫描，并把结果（或失败原因）写入缓存。
    ///
    /// 失败时也写：**保留上一次成功的结果**，只记录本次没做成的原因。清空它会让
    /// 玩家在「安装进行中」这类暂时状态下什么都看不到，而那正是本功能最该起作用
    /// 的时刻。
    pub fn scan(
        &self,
        request: ConfiguredExternalStateScanRequest<'_>,
    ) -> Result<ExternalInstallStateSummary, ConfiguredExternalStateScanError> {
        let ConfiguredExternalStateScanRequest {
            game_id,
            profile_id,
            mod_id,
            cancellation_token,
        } = request;

        match self.scan_inner(game_id, profile_id, mod_id, cancellation_token) {
            Ok((summary, prepared, fingerprints)) => {
                let computed_at = self.cache_clock_now();
                self.cache.record_success(
                    game_id,
                    profile_id,
                    mod_id,
                    ExternalStateScanRecord {
                        summary: summary.clone(),
                        prepared,
                        fingerprints,
                        computed_at_unix_millis: computed_at,
                    },
                );
                Ok(summary)
            }
            Err(error) => {
                self.cache
                    .record_failure(game_id, profile_id, mod_id, error);
                Err(error)
            }
        }
    }

    /// 查询结果（含 `stale` 与上次失败原因）。
    ///
    /// 与 [`Self::scan`] 分开：前者是重活（哈希），这个只是重新 stat 一遍，
    /// 因此**很便宜**，可以随界面刷新反复调用。
    ///
    /// 游戏实例读不到时**不丢弃缓存结果**：这是查询期的处境，不是上次扫描的结论，
    /// 不能写进 `last_error`（那个字段的语义是「上次扫描没做成的原因」）。此时无法
    /// stat，也就无法证明结果仍然成立——与 stat 失败同一口径，按 `stale` 报。
    pub fn query(
        &self,
        game_id: &GameId,
        profile_id: &ProfileId,
        mod_id: &ModId,
    ) -> ExternalStateScanQuery {
        // query 不拿锁（见 `ExternalStateScanCache::query` 的说明），
        // 因此这里直接构造 inspector 即可。
        let Ok(Some(game_instance)) = self.game_config_repository.load_game_instance(game_id)
        else {
            return self
                .cache
                .query_without_inspector(game_id, profile_id, mod_id);
        };
        let handles = self.game_fs_factory.create(&game_instance.root_dir);
        self.cache
            .query(game_id, profile_id, mod_id, handles.inspector.as_ref())
    }

    fn cache_clock_now(&self) -> u128 {
        self.cache.now_unix_millis()
    }

    fn scan_inner(
        &self,
        game_id: &GameId,
        profile_id: &ProfileId,
        mod_id: &ModId,
        cancellation_token: &dyn CancellationToken,
    ) -> Result<SuccessfulExternalStateScan, ConfiguredExternalStateScanError> {
        // ---- stage 0（锁外）：解析输入，只读导入包沙箱 ----
        // 先确认游戏目录存在，顺便拿到 root_dir。后续 stat 会再读一次配置：
        // 游戏目录本身可能在扫描期间被改，那必须表现为 Stale 而不是静默用旧路径。
        let game_instance = self
            .game_config_repository
            .load_game_instance(game_id)
            .map_err(|_| ConfiguredExternalStateScanError::GameInstanceUnavailable)?
            .ok_or(ConfiguredExternalStateScanError::GameInstanceUnavailable)?;

        let analysis = self
            .mod_import_result_repository
            .get_analysis(mod_id.as_str())
            .map_err(|_| ConfiguredExternalStateScanError::ModUnavailable)?
            .ok_or(ConfiguredExternalStateScanError::ModUnavailable)?;
        let package_id = analysis.package_id;

        let sandbox_root = self
            .sandbox_locator
            .sandbox_root_for_package(&package_id)
            .map_err(|_| ConfiguredExternalStateScanError::SandboxUnavailable)?;

        let service = self.service_for(&game_instance.root_dir);
        let prepared = service
            .prepare_targets(ExternalStateScanPrepareRequest {
                package_id: &package_id,
                sandbox_root: &sandbox_root,
                allowed_roots: &self.allowed_roots,
            })
            .map_err(scan_prepare_error)?;

        if cancellation_token.is_cancelled() {
            return Err(ConfiguredExternalStateScanError::Cancelled);
        }

        // ---- stage 1（锁内）：只 stat，不做 hash ----
        let before = self.with_game_lock(game_id, profile_id, |inspector| {
            stat_all(inspector, &prepared)
        })?;

        if cancellation_token.is_cancelled() {
            return Err(ConfiguredExternalStateScanError::Cancelled);
        }

        // ---- stage 2（锁外）：长时间工作（读两侧文件 + hash）----
        let summary = service
            .summarize_prepared(&prepared, &package_id, &sandbox_root, cancellation_token)
            .map_err(map_scan_error)?;

        // ---- stage 3（锁内）：复核指纹。有漂移就丢弃结果，绝不返回可疑事实 ----
        let after = self.with_game_lock(game_id, profile_id, |inspector| {
            stat_all(inspector, &prepared)
        })?;

        if !same_fingerprints(&before, &after) {
            return Err(ConfiguredExternalStateScanError::Stale);
        }

        // 存的是 stage 3 **复核通过**的指纹：它才是「这个结果成立时的文件状态」。
        // 存 stage 1 的 `before` 会漏掉扫描期间的改动，而那正是要检测的东西。
        Ok((summary, prepared, after))
    }

    fn service_for(&self, game_root: &std::path::Path) -> ExternalModStateScanService {
        ExternalModStateScanService::new(
            Arc::clone(&self.scanner),
            Arc::clone(&self.package_reader),
            self.game_fs_factory.create(game_root).fs,
            self.max_file_bytes,
            self.worker_limit,
        )
    }

    /// 在跨进程 admission + 进程内写锁内执行**有界**的工作，然后立刻释放。
    ///
    /// 用 `try_lock` 而非 `lock`：拿不到锁意味着有写入正在进行，此时必须返回
    /// `Stale` 而不是等待。等待会让「有写入进行中 → stale」这条分支永不执行。
    fn with_game_lock<T, F>(
        &self,
        game_id: &GameId,
        profile_id: &ProfileId,
        work: F,
    ) -> Result<T, ConfiguredExternalStateScanError>
    where
        F: FnOnce(&dyn InstallGameFileInspector) -> Result<T, ConfiguredExternalStateScanError>,
    {
        let _cross_process_guard = match self.write_locks.acquire_cross_process(game_id, profile_id)
        {
            Ok(guard) => guard,
            // `Busy` 与 `Unavailable` 都是「这次拿不到准入」→ 报 stale（降级，非失败）。
            // `Cancelled` 在这里不可能出现（`acquire_cross_process` 传 `NeverCancelled`），
            // 但显式列出，避免将来改签名时把「用户取消」静默降级成「结果过期」。
            Err(CrossProcessWriteAdmissionError::OrderViolation) => {
                return Err(ConfiguredExternalStateScanError::WriteAdmissionOrderViolation);
            }
            Err(_) => return Err(ConfiguredExternalStateScanError::Stale),
        };

        let write_lock = self.write_locks.lock_for(game_id, profile_id);
        let guard = write_lock
            .try_lock()
            .map_err(|_| ConfiguredExternalStateScanError::Stale)?;

        // 锁内重新加载游戏目录：目录若被改，这里拿到的是新路径，
        // 与 stage 1 的指纹不一致 → stage 3 判为 Stale。fail-closed。
        let game_instance = self
            .game_config_repository
            .load_game_instance(game_id)
            .map_err(|_| ConfiguredExternalStateScanError::GameInstanceUnavailable)?
            .ok_or(ConfiguredExternalStateScanError::GameInstanceUnavailable)?;
        let handles = self.game_fs_factory.create(&game_instance.root_dir);

        let result = work(handles.inspector.as_ref());
        // 工作完成后立刻释放：stat 很快，但绝不让锁的生命周期超出这个作用域。
        drop(guard);
        result
    }
}

/// 一次构造出来的游戏目录访问能力：读写（`fs`）与只读观测（`inspector`）。
pub struct GameFileSystemHandles {
    pub fs: Arc<dyn InstallGameFileSystem>,
    pub inspector: Arc<dyn InstallGameFileInspector>,
}

/// 按游戏目录构造游戏目录访问对象。
///
/// 抽成工厂是为了让测试注入假实现——真实实现是 `FileSystemInstallGameFileSystem`，
/// 它同时实现 `InstallGameFileSystem` 与 `InstallGameFileInspector` 两个 trait，
/// 因此两个句柄指向同一个对象，不存在「读写到 A、stat 到 B」的错位。
pub trait GameFileSystemFactory: Send + Sync {
    fn create(&self, game_root: &std::path::Path) -> GameFileSystemHandles;
}

/// 真实实现：`FileSystemInstallGameFileSystem` 同时实现读写与只读观测两个 trait，
/// 因此两个句柄是**同一个对象**，不存在「读写到 A、stat 到 B」的错位。
pub struct RealGameFileSystemFactory;

impl GameFileSystemFactory for RealGameFileSystemFactory {
    fn create(&self, game_root: &std::path::Path) -> GameFileSystemHandles {
        let concrete = Arc::new(FileSystemInstallGameFileSystem::new(
            game_root.to_path_buf(),
        ));
        // 两个句柄共享同一个 concrete Arc：读写与 stat 背后是同一份状态，
        // 不可能出现「读到 A 文件、stat 到 B 文件」的错位。
        // 先按具体类型 clone，再各自 unsize 成 trait 对象——`Arc::clone(&concrete)`
        // 会因为返回类型标注而被反向推断成 trait 对象，反而无法 coerce。
        let shared = Arc::clone(&concrete);
        let fs: Arc<dyn InstallGameFileSystem> = concrete;
        let inspector: Arc<dyn InstallGameFileInspector> = shared;
        GameFileSystemHandles { fs, inspector }
    }
}

/// 逐文件 stat，按输入顺序返回。
fn stat_all(
    inspector: &dyn InstallGameFileInspector,
    prepared: &[PreparedExternalTarget],
) -> Result<Vec<Option<GameFileFingerprint>>, ConfiguredExternalStateScanError> {
    prepared
        .iter()
        .map(|target| {
            inspector
                .stat_game_file(&target.target_path)
                .map_err(|_| ConfiguredExternalStateScanError::GameFileUnavailable)
        })
        .collect()
}

/// 两次指纹序列是否完全一致。
///
/// 逐项比较而不是整体比较：任一项变化（出现、消失、改内容）都算漂移。
fn same_fingerprints(
    before: &[Option<GameFileFingerprint>],
    after: &[Option<GameFileFingerprint>],
) -> bool {
    if before.len() != after.len() {
        return false;
    }
    before
        .iter()
        .zip(after.iter())
        .all(|(left, right)| match (left, right) {
            (Some(left), Some(right)) => left.matches(right),
            (None, None) => true,
            // 文件从无到有、从有到无，都算被改动过。
            _ => false,
        })
}

fn scan_prepare_error(error: ExternalStateScanError) -> ConfiguredExternalStateScanError {
    match error {
        ExternalStateScanError::Cancelled => ConfiguredExternalStateScanError::Cancelled,
        ExternalStateScanError::PackageScanFailed => {
            ConfiguredExternalStateScanError::PackageScanFailed
        }
    }
}

/// stage 2 的错误映射。
///
/// 与 `scan_prepare_error` 分开：这里失败意味着**哈希过程中**出了问题，
/// 不是「包扫不出来」。虽然最终都收敛到 `ScanUnavailable`，但保留两个入口
/// 是为了将来到 command 层时能分开计数与取词。
fn map_scan_error(error: ExternalStateScanError) -> ConfiguredExternalStateScanError {
    match error {
        ExternalStateScanError::Cancelled => ConfiguredExternalStateScanError::Cancelled,
        ExternalStateScanError::PackageScanFailed => {
            ConfiguredExternalStateScanError::ScanUnavailable
        }
    }
}

// ---------------------------------------------------------------------------
// 结果存储
// ---------------------------------------------------------------------------

/// 进程内结果缓存的条目数上限。
///
/// 取 512 的依据（不是随手定的数）：Mod 库单页最大 96 条
/// （`hmm-app::mod_library_query::ALLOWED_PAGE_SIZES`），而本功能是**按需**扫描
/// ——只在打开详情或点检查时触发，不做进每次翻页。512 约为 5 个满页的余量，
/// 足够覆盖来回翻页与详情跳转。
///
/// 单条约 1.7 KB（33 文件的 MOD，估算值），512 条约 0.8 MB——**估算，未实测**。
/// 即便按 10 倍保守估计也不到 10 MB，远不足以构成 OOM 风险。
pub const DEFAULT_EXTERNAL_STATE_CACHE_MAX_ENTRIES: usize = 512;

/// 一次已完成的扫描结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalStateScanRecord {
    /// 判定结果（聚合计数 + 每文件状态）。
    pub summary: ExternalInstallStateSummary,
    /// 复核所需的目标列表。
    ///
    /// **不能只存 fingerprint**：getter 重新 stat 必须知道去 stat 哪些路径，
    /// 而 fingerprint 是与 `prepared` 同序的裸序列，脱离了它就没有路径信息。
    /// issue 的原始设计只列了 `fingerprint`，这里补上这个必要字段。
    pub prepared: Vec<PreparedExternalTarget>,
    /// stage 3 复核通过时的指纹快照，供 getter 比对。
    pub fingerprints: Vec<Option<GameFileFingerprint>>,
    pub computed_at_unix_millis: u128,
}

/// 查询结果。
///
/// ## 两个字段为什么分开
///
/// `stale` 与 `last_error` 描述**完全不同的处境**，合并成一个会让界面无法区分：
///
/// - `stale = true`：**上次扫成功了**，但之后文件可能变了 → 展示旧结果，提示"可能已变"
/// - `last_error = Some(..)`：**上次根本没扫成**（准入/锁/期间被改动）→ 展示旧结果，
///   提示"上次没扫成，原因 X"
///
/// 合起来会让玩家看到"可能已变"，而实际是"压根没扫"——这正是项目反复告诫的
/// 「两侧各自全绿、中间语义丢失」（排障手册 4.7）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalStateScanQuery {
    /// 上一次成功的判定结果。**没有成功扫过时为 `None`**。
    pub summary: Option<ExternalInstallStateSummary>,
    /// 结果可能已过期：getter 重新 stat 与存档指纹不一致。
    ///
    /// 为 `true` 时 `summary` 仍是上次的结果——**保留而不是清空**，这是维护者在
    /// issue #286 里拍板的降级口径（"有写入进行中 → stale 并保留上次结果"）。
    pub stale: bool,
    /// 上次扫描没做成的原因（稳定错误码）。成功时为 `None`。
    pub last_error: Option<ConfiguredExternalStateScanError>,
    /// 与 `summary.files` **同序**的展示路径（导入包里的原始字符串）。
    ///
    /// `ExternalInstallStateSummary.files` 只有状态没有路径——路径在扫描时的
    /// `prepared` 里。界面的文件级明细两者都要，所以这里按同一顺序补上。
    /// `summary` 为 `None` 时恒为空。
    pub display_paths: Vec<String>,
}

/// 按 `(game_id, profile_id, mod_id)` 缓存扫描结果。
///
/// ## 为什么是进程内内存，而不是持久化
///
/// 这是**派生的观测结果**，不是事实来源——事实在游戏目录和导入包里，重算一遍即可
/// （维护者的原话："重新 stat 很便宜"）。持久化它反而要处理失效与迁移，
/// 且一旦与真实文件不一致就成了误导。
///
/// ## 淘汰策略：按时间，不按 LRU
///
/// 沿用 `InMemoryPendingSaveDirectoryCandidateStore` 的 sweep 范式。项目里**没有**
/// LRU 先例，不为此新引入一种机制。
pub struct ExternalStateScanCache {
    entries: Mutex<HashMap<CacheKey, CacheEntry>>,
    clock: Arc<dyn AppClock>,
    max_entries: usize,
}

/// 键里同时带 game_id：同一个 mod_id 在不同游戏下是不同东西。
type CacheKey = (String, String, String);

#[derive(Clone)]
struct CacheEntry {
    record: Option<ExternalStateScanRecord>,
    last_error: Option<ConfiguredExternalStateScanError>,
}

impl ExternalStateScanCache {
    pub fn new(clock: Arc<dyn AppClock>) -> Self {
        Self::with_max_entries(clock, DEFAULT_EXTERNAL_STATE_CACHE_MAX_ENTRIES)
    }

    pub fn with_max_entries(clock: Arc<dyn AppClock>, max_entries: usize) -> Self {
        Self {
            entries: Mutex::new(HashMap::new()),
            clock,
            max_entries: max_entries.max(1),
        }
    }

    /// 当前时间。失败时返回 0——时间戳只用于淘汰排序，取不到时退化成
    /// 「最旧」是安全的（它只会让该条目更早被淘汰，不会破坏正确性）。
    pub(crate) fn now_unix_millis(&self) -> u128 {
        self.clock.now_unix_millis().unwrap_or(0)
    }

    pub fn record_success(
        &self,
        game_id: &GameId,
        profile_id: &ProfileId,
        mod_id: &ModId,
        record: ExternalStateScanRecord,
    ) {
        let mut entries = match self.entries.lock() {
            Ok(entries) => entries,
            Err(_) => return,
        };
        let key = cache_key(game_id, profile_id, mod_id);
        entries.insert(
            key,
            CacheEntry {
                record: Some(record),
                last_error: None,
            },
        );
        sweep_to_limit(&mut entries, self.max_entries);
    }

    /// 记录一次失败的扫描。**保留上一次成功的结果**——清空它会让玩家在
    /// 安装进行中这类暂时状态下什么也看不到。
    pub fn record_failure(
        &self,
        game_id: &GameId,
        profile_id: &ProfileId,
        mod_id: &ModId,
        error: ConfiguredExternalStateScanError,
    ) {
        let mut entries = match self.entries.lock() {
            Ok(entries) => entries,
            Err(_) => return,
        };
        let key = cache_key(game_id, profile_id, mod_id);
        let previous_record = entries.get(&key).and_then(|entry| entry.record.clone());
        entries.insert(
            key,
            CacheEntry {
                record: previous_record,
                last_error: Some(error),
            },
        );
        sweep_to_limit(&mut entries, self.max_entries);
    }

    /// 查询结果。
    ///
    /// `stale` 由**重新 stat** 判定（不拿锁，见下），因此需要 stat 所需的
    /// `prepared` 与指纹快照。
    ///
    /// ## 为什么不拿锁
    ///
    /// 拿锁会引入两个问题：①getter 可能卡住（正是 issue 要避免的）；②装了锁就
    /// 看不到并发修改，而**看到并发修改恰恰是这次 stat 的目的**。所以这里刻意
    /// 让它与安装并发跑，读到什么就报什么。
    ///
    /// 代价是 stat 可能落在写入中途（撕裂状态）。这在语义上是可接受的：
    /// 它只会**多报** stale，不会少报——fail-closed 方向正确。
    pub fn query(
        &self,
        game_id: &GameId,
        profile_id: &ProfileId,
        mod_id: &ModId,
        inspector: &dyn InstallGameFileInspector,
    ) -> ExternalStateScanQuery {
        self.query_with(game_id, profile_id, mod_id, |record| {
            self.is_stale(record, inspector)
        })
    }

    /// 拿不到游戏目录（inspector 无从构造）时的查询。
    ///
    /// 无法 stat ⇒ 无法证明结果仍然成立 ⇒ 只要有结果就按 `stale` 报
    /// （与 stat 失败同一口径，fail-closed）。`last_error` 保持上次扫描的事实，
    /// 不用查询期的处境去覆盖它。
    pub fn query_without_inspector(
        &self,
        game_id: &GameId,
        profile_id: &ProfileId,
        mod_id: &ModId,
    ) -> ExternalStateScanQuery {
        self.query_with(game_id, profile_id, mod_id, |_record| true)
    }

    fn query_with(
        &self,
        game_id: &GameId,
        profile_id: &ProfileId,
        mod_id: &ModId,
        is_record_stale: impl Fn(&ExternalStateScanRecord) -> bool,
    ) -> ExternalStateScanQuery {
        let snapshot = {
            let Ok(entries) = self.entries.lock() else {
                return empty_scan_query();
            };
            entries
                .get(&cache_key(game_id, profile_id, mod_id))
                .cloned()
        };

        let Some(entry) = snapshot else {
            return empty_scan_query();
        };

        let stale = match &entry.record {
            Some(record) => is_record_stale(record),
            // 从没成功扫过，谈不上"过期"。
            None => false,
        };

        let (summary, display_paths) = match entry.record {
            Some(record) => (
                Some(record.summary),
                record
                    .prepared
                    .into_iter()
                    .map(|target| target.display_path)
                    .collect(),
            ),
            None => (None, Vec::new()),
        };

        ExternalStateScanQuery {
            summary,
            stale,
            last_error: entry.last_error,
            display_paths,
        }
    }

    /// 重新 stat 并与存档指纹比对。
    ///
    /// **stat 失败按 stale 处理**：拿不到当前事实就无法证明结果仍然成立，
    /// 与其赌它没变，不如如实说"不确定"（fail-closed）。
    fn is_stale(
        &self,
        record: &ExternalStateScanRecord,
        inspector: &dyn InstallGameFileInspector,
    ) -> bool {
        match stat_all(inspector, &record.prepared) {
            Ok(current) => !same_fingerprints(&record.fingerprints, &current),
            Err(_) => true,
        }
    }
}

fn empty_scan_query() -> ExternalStateScanQuery {
    ExternalStateScanQuery {
        summary: None,
        stale: false,
        last_error: None,
        display_paths: Vec::new(),
    }
}

fn cache_key(game_id: &GameId, profile_id: &ProfileId, mod_id: &ModId) -> CacheKey {
    (
        game_id.as_str().to_owned(),
        profile_id.as_str().to_owned(),
        mod_id.as_str().to_owned(),
    )
}

/// 超出上限时按 `computed_at_unix_millis` 淘汰最旧的，直到回到上限内。
///
/// 只在插入后调用：条目数每次最多 +1，因此至多淘汰一个就够——但写成循环是
/// 为了让 `max_entries` 被调小（或通过测试注入更小值）时也能收敛。
/// 按「谁最该被淘汰」排序：先比 `computedAt`，**相同时再比 key**。
///
/// tie-break 不是装饰：时间戳相同是真实场景（时钟取不到时间时全部退化为 0，
/// 或同一个毫秒内连续扫多个 MOD）。`min_by_key` 在并列时返回 HashMap 迭代
/// 顺序里的第一个，而那个顺序是**随机的**——不 tie-break 的话淘汰结果不确定，
/// 表现为「单独跑过、整组跑挂」的幽灵失败。
fn eviction_candidate(entry: (&CacheKey, &CacheEntry)) -> Option<(u128, CacheKey)> {
    let record = entry.1.record.as_ref()?;
    Some((record.computed_at_unix_millis, entry.0.clone()))
}

fn sweep_to_limit(entries: &mut HashMap<CacheKey, CacheEntry>, max_entries: usize) {
    while entries.len() > max_entries {
        let oldest = entries
            .iter()
            .filter_map(eviction_candidate)
            .min_by(|left, right| left.0.cmp(&right.0).then_with(|| left.1.cmp(&right.1)))
            .map(|(_, key)| key);

        match oldest {
            Some(key) => {
                entries.remove(&key);
            }
            // 没有任何条目带 computedAt（全是纯失败记录）：它们没有时间语义，
            // 但**仍要按 key 排序**取最小，保证淘汰是确定的而不是随机的。
            None => match entries.keys().min().cloned() {
                Some(key) => {
                    entries.remove(&key);
                }
                None => break,
            },
        }
    }
}
