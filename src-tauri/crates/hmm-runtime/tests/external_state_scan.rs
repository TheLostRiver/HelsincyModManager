//! `ConfiguredExternalStateScanner` 的行为测试。
//!
//! ## 这些用例守的是什么
//!
//! 扫描器跨越三段加锁，最容易出错的不是判定逻辑（那部分在 `hmm-core`，已有覆盖），
//! 而是**锁的边界与降级语义**：
//!
//! 1. 长时间工作（hash）绝不能在写锁内——否则几十 MB 的 IO 会阻塞所有安装。
//! 2. 「有写入进行中」必须报 `Stale`，而不是等待或硬失败。
//! 3. 扫描期间文件被改动必须**丢弃结果**（fail-closed），不能返回可疑事实。
//! 4. 取消、数据过期、编程错误三者**不能互相冒充**。
//!
//! ## 不给生产代码开洞
//!
//! 锁内外的判定不靠生产代码暴露计数器，而是**让假件自己探测**：
//! `RecordingGameFs` 持有的探针就是 `write_locks.lock_for(...)` 返回的**同一把锁**，
//! 它在每次 `read` / `stat` 时 `try_lock` 一次，失败即说明此刻锁被持有。
//! 这样生产代码里不留任何测试专用分支。
//!
//! 每条用例都跑过控制组：把实现退回去，确认它会变红。

use hmm_app::external_state_scan::DEFAULT_WORKER_LIMIT;
use hmm_app::GameProfileWriteLockRegistry;
use hmm_core::{
    ExternalInstallState, GameDirectoryStatus, GameId, GameInstance, InstallTargetPath, ModId,
    ProfileId,
};
use hmm_ports::{
    CancellationToken, GameConfigRepository, GameConfigRepositoryError, GameConfigRepositoryResult,
    GameFileFingerprint, InstallGameFileInspector, InstallGameFileSystem,
    ModImportResultRepository, ModImportSandboxLocator, ModPackageInstallFile,
    ModPackageInstallFileReadRequest, ModPackageInstallFileReader, ModPackageInstallFileScanError,
    ModPackageInstallFileScanRequest, ModPackageInstallFileScanner, NeverCancelled,
    StoredImportPreviewImage, StoredModImportAnalysis,
};
use hmm_runtime::external_state_scan::{
    ConfiguredExternalStateScanError, ConfiguredExternalStateScanRequest,
    ConfiguredExternalStateScanner, GameFileSystemFactory, GameFileSystemHandles,
};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime};
use tempfile::TempDir;

const ALLOWED_ROOTS: &[&str] = &["nativePC"];

// ---------------------------------------------------------------------------
// 假件
// ---------------------------------------------------------------------------

fn game_instance(root: PathBuf) -> GameInstance {
    GameInstance {
        id: "mhw-instance".to_owned(),
        game_id: GameId::mhw(),
        display_name: "Monster Hunter: World".to_owned(),
        root_dir: root,
        status: GameDirectoryStatus::Configured,
        configured_at_unix_millis: 1,
    }
}

struct FakeGameConfigRepository {
    instance: Option<GameInstance>,
}

impl FakeGameConfigRepository {
    fn with_root(root: impl Into<PathBuf>) -> Self {
        Self {
            instance: Some(game_instance(root.into())),
        }
    }

    fn none() -> Self {
        Self { instance: None }
    }
}

impl GameConfigRepository for FakeGameConfigRepository {
    fn load_game_instance(
        &self,
        _game_id: &GameId,
    ) -> GameConfigRepositoryResult<Option<GameInstance>> {
        Ok(self.instance.clone())
    }

    fn save_game_instance(&self, _instance: &GameInstance) -> GameConfigRepositoryResult<()> {
        Err(GameConfigRepositoryError::StorageFailed(
            "tests do not save game instances".to_owned(),
        ))
    }
}

/// 只需实现 `get_analysis`——那是 `mod_id → package_id` 的唯一入口
/// （`hmm-app/src/install.rs:1460` 的 `build_plan_from_imported_mod` 就是这么走的）。
#[derive(Default)]
struct FakeModImportResultRepository {
    analysis: Option<StoredModImportAnalysis>,
}

impl ModImportResultRepository for FakeModImportResultRepository {
    fn save_analysis(&self, _analysis: &StoredModImportAnalysis) -> anyhow::Result<()> {
        anyhow::bail!("tests do not save analysis")
    }

    fn list_analysis(&self) -> anyhow::Result<Vec<StoredModImportAnalysis>> {
        Ok(self.analysis.clone().into_iter().collect())
    }

    fn get_analysis(&self, mod_id: &str) -> anyhow::Result<Option<StoredModImportAnalysis>> {
        Ok(self
            .analysis
            .clone()
            .filter(|analysis| analysis.mod_id == mod_id))
    }
}

fn analysis(mod_id: &str, package_id: &str) -> StoredModImportAnalysis {
    StoredModImportAnalysis {
        mod_id: mod_id.to_owned(),
        task_id: "task-1".to_owned(),
        package_id: package_id.to_owned(),
        display_name: "External Mod".to_owned(),
        metadata: Default::default(),
        preview_image: StoredImportPreviewImage::Fallback {
            reason: hmm_core::PreviewImageRejectionReason::Missing,
        },
    }
}

struct FixedSandboxLocator(PathBuf);

impl ModImportSandboxLocator for FixedSandboxLocator {
    fn sandbox_root_for_package(&self, _package_id: &str) -> anyhow::Result<PathBuf> {
        Ok(self.0.clone())
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
            return Err(ModPackageInstallFileScanError::Unavailable);
        }
        Ok(self.files.clone())
    }
}

#[derive(Default)]
struct FakePackageReader {
    bytes_by_id: HashMap<String, Vec<u8>>,
}

impl FakePackageReader {
    fn with(entries: &[(&str, &[u8])]) -> Self {
        let mut bytes_by_id = HashMap::new();
        for (id, bytes) in entries {
            bytes_by_id.insert((*id).to_owned(), bytes.to_vec());
        }
        Self { bytes_by_id }
    }
}

impl ModPackageInstallFileReader for FakePackageReader {
    fn read_install_file(
        &self,
        request: ModPackageInstallFileReadRequest<'_>,
    ) -> anyhow::Result<Vec<u8>> {
        self.bytes_by_id
            .get(request.package_file_id.as_str())
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("unknown package file"))
    }
}

/// 游戏目录侧的假实现，同时实现「读写」与「只读观测」两个 trait。
///
/// **它自己探测锁状态**：`probe` 与 scanner 内部 `lock_for` 返回的是同一把锁，
/// 每次 `read` / `stat` 时 `try_lock` 一次，失败即说明此刻写锁被持有。
/// 这样「长时间工作是否在锁内」不需要生产代码配合，也不给它开洞。
struct RecordingGameFs {
    files: Mutex<HashMap<String, Option<Vec<u8>>>>,
    /// 读文件时写锁被持有的次数（应恒为 0）。
    reads_while_locked: AtomicUsize,
    /// stat 时写锁被持有的次数（应恒 > 0）。
    stats_while_locked: AtomicUsize,
    /// 与 scanner 内部写锁同一把的探针（由 `bind_probe` 后绑定）。
    probe: Mutex<Option<Arc<Mutex<()>>>>,
    /// 钩子：第 N 次「读」之后把某个文件换成别的内容，用于制造 stage 3 漂移。
    mutate_after_reads: Option<(usize, String, Vec<u8>)>,
    read_count: AtomicUsize,
}

impl RecordingGameFs {
    fn new(entries: &[(&str, &[u8])]) -> Self {
        let mut files = HashMap::new();
        for (path, bytes) in entries {
            files.insert((*path).to_owned(), Some(bytes.to_vec()));
        }
        Self {
            files: Mutex::new(files),
            reads_while_locked: AtomicUsize::new(0),
            stats_while_locked: AtomicUsize::new(0),
            probe: Mutex::new(None),
            mutate_after_reads: None,
            read_count: AtomicUsize::new(0),
        }
    }

    /// 后绑定探针：必须在 registry 创建之后调用，因为要的是 `lock_for` 返回的那把锁。
    fn bind_probe(&self, probe: Arc<Mutex<()>>) {
        *self.probe.lock().expect("probe lock") = Some(probe);
    }

    /// 在第 `after` 次读之后，把 `path` 的内容换成 `bytes`。
    fn mutate_after_reads(mut self, after: usize, path: &str, bytes: Vec<u8>) -> Self {
        self.mutate_after_reads = Some((after, path.to_owned(), bytes));
        self
    }

    /// 此刻写锁是否被持有。try_lock 失败 == 被持有。
    fn locked_now(&self) -> bool {
        match self.probe.lock().expect("probe lock").as_ref() {
            Some(lock) => lock.try_lock().is_err(),
            None => false,
        }
    }
}

/// 由 size 派生 mtime：长度不同 → mtime 不同。
///
/// 这是 `stat` 的本分——它检测不到「长度相同但内容不同」。测试因此用**改变长度**
/// 来制造漂移，这符合生产语义（真实的 mtime/大小指纹同样如此），不是迁就实现。
fn fake_fingerprint(size_bytes: u64) -> GameFileFingerprint {
    GameFileFingerprint {
        size_bytes,
        modified: Some(
            SystemTime::UNIX_EPOCH + Duration::from_millis(1_700_000_000_000 + size_bytes),
        ),
    }
}

impl InstallGameFileSystem for RecordingGameFs {
    fn read_game_file(&self, target_path: &InstallTargetPath) -> anyhow::Result<Option<Vec<u8>>> {
        if self.locked_now() {
            self.reads_while_locked.fetch_add(1, Ordering::SeqCst);
        }

        let observed = self.read_count.fetch_add(1, Ordering::SeqCst);
        let content = {
            let files = self.files.lock().expect("files lock");
            match files.get(target_path.as_str()) {
                None => None,
                Some(None) => return Err(anyhow::anyhow!("locked")),
                Some(Some(bytes)) => Some(bytes.clone()),
            }
        };

        // 钩子必须在**返回之前**触发，否则文件缺失时提前 return 会跳过它，
        // 「文件从无到有」这类漂移就造不出来（这条用例曾因此假绿）。
        if let Some((after, path, bytes)) = &self.mutate_after_reads {
            if observed >= *after {
                self.files
                    .lock()
                    .expect("files lock")
                    .insert(path.clone(), Some(bytes.clone()));
            }
        }

        Ok(content)
    }

    fn write_game_file(
        &self,
        _target_path: &InstallTargetPath,
        _bytes: &[u8],
    ) -> anyhow::Result<()> {
        anyhow::bail!("external state scan must never write")
    }

    fn remove_game_file(&self, _target_path: &InstallTargetPath) -> anyhow::Result<()> {
        anyhow::bail!("external state scan must never remove")
    }
}

impl InstallGameFileInspector for RecordingGameFs {
    fn stat_game_file(
        &self,
        target_path: &InstallTargetPath,
    ) -> anyhow::Result<Option<GameFileFingerprint>> {
        if self.locked_now() {
            self.stats_while_locked.fetch_add(1, Ordering::SeqCst);
        }
        let files = self.files.lock().expect("files lock");
        match files.get(target_path.as_str()) {
            None => Ok(None),
            Some(None) => Err(anyhow::anyhow!("locked")),
            Some(Some(bytes)) => Ok(Some(fake_fingerprint(bytes.len() as u64))),
        }
    }
}

/// 把同一个 `RecordingGameFs` 同时交给「读写」与「只读观测」两个角色。
struct SharedGameFsFactory(Arc<RecordingGameFs>);

impl GameFileSystemFactory for SharedGameFsFactory {
    fn create(&self, _game_root: &Path) -> GameFileSystemHandles {
        // 先按具体类型 clone 两次，再各自 unsize 成 trait 对象。
        // 直接 `Arc::clone(&shared)` 会因返回类型标注被反向推断成 trait 对象，反而无法 coerce。
        let fs = Arc::clone(&self.0);
        let inspector = Arc::clone(&self.0);
        GameFileSystemHandles { fs, inspector }
    }
}

struct Cancelled;

impl CancellationToken for Cancelled {
    fn is_cancelled(&self) -> bool {
        true
    }
}

// ---------------------------------------------------------------------------
// 装配
// ---------------------------------------------------------------------------

struct Harness {
    _temp: TempDir,
    scanner: Arc<ConfiguredExternalStateScanner>,
    game_fs: Arc<RecordingGameFs>,
    /// 与 scanner 内部 `lock_for` 返回的是同一把锁。
    write_lock: Arc<Mutex<()>>,
}

/// 装配一个可扫描的环境。
///
/// `game_fs` 由调用方构造（可带锁探针、可带「读到第 N 次就改文件」的钩子）。
/// 装配完成后 **probe 会被绑到 scanner 内部的那把写锁上**——两者必须是同一把，
/// 否则「此刻锁是否被持有」的探测测不到东西。
fn harness(package_files: &[(&str, &[u8])], game_fs: RecordingGameFs) -> Harness {
    let temp = tempfile::tempdir().expect("temp dir");
    let game_root = temp.path().join("game");
    std::fs::create_dir_all(&game_root).expect("create game root");

    let write_locks = Arc::new(GameProfileWriteLockRegistry::default());
    let write_lock = write_locks.lock_for(&GameId::mhw(), &ProfileId::new("default"));

    let game_fs = Arc::new(game_fs);
    let scanner = Arc::new(ConfiguredExternalStateScanner::new(
        Arc::new(FakeGameConfigRepository::with_root(game_root)),
        Arc::new(FakeModImportResultRepository {
            analysis: Some(analysis("mod-a", "package-a")),
        }),
        Arc::new(FixedSandboxLocator(temp.path().join("sandbox"))),
        ALLOWED_ROOTS
            .iter()
            .map(|root| (*root).to_owned())
            .collect(),
        Arc::clone(&write_locks),
        Arc::new(FakeScanner {
            files: package_files
                .iter()
                .map(|(id, _)| ModPackageInstallFile {
                    package_file_id: (*id).to_owned(),
                    target_path: (*id).to_owned(),
                })
                .collect(),
            fail: false,
        }),
        Arc::new(FakePackageReader::with(package_files)),
        Arc::new(SharedGameFsFactory(Arc::clone(&game_fs))),
        8 * 1024 * 1024,
        DEFAULT_WORKER_LIMIT,
    ));

    Harness {
        _temp: temp,
        scanner,
        game_fs,
        write_lock,
    }
}

impl Harness {
    /// 把 `game_fs` 的锁探针绑到 scanner 内部那把写锁上。
    ///
    /// 必须在构造 `RecordingGameFs` 之后调用：探针要的就是 `lock_for` 返回的
    /// **同一把** `Arc<Mutex<()>>`，早于 registry 存在时拿不到。
    fn with_lock_probe(self) -> Harness {
        let probe = Arc::clone(&self.write_lock);
        self.game_fs.bind_probe(probe);
        self
    }
}

impl Harness {
    fn scan(
        &self,
    ) -> Result<hmm_core::ExternalInstallStateSummary, ConfiguredExternalStateScanError> {
        self.scanner.scan(ConfiguredExternalStateScanRequest {
            game_id: &GameId::mhw(),
            profile_id: &ProfileId::new("default"),
            mod_id: &ModId::new("mod-a"),
            cancellation_token: &NeverCancelled,
        })
    }
}

// ---------------------------------------------------------------------------
// 正向路径
// ---------------------------------------------------------------------------

#[test]
fn matching_files_are_reported_as_installed() {
    let files: &[(&str, &[u8])] = &[("nativePC/a.mod3", b"same")];
    let harness = harness(files, RecordingGameFs::new(files));

    let summary = harness.scan().expect("scan succeeds");

    assert_eq!(summary.state, ExternalInstallState::Installed);
    assert_eq!(summary.matched_file_count, 1);
}

#[test]
fn changed_content_is_reported_as_changed() {
    let files: &[(&str, &[u8])] = &[("nativePC/a.mod3", b"from-package")];
    let harness = harness(
        files,
        RecordingGameFs::new(&[("nativePC/a.mod3", b"from-game")]),
    );

    let summary = harness.scan().expect("scan succeeds");

    assert_eq!(summary.state, ExternalInstallState::Changed);
}

#[test]
fn paths_outside_the_allowed_roots_are_ignored() {
    // 与安装同口径：装不进去的文件不参与「是否已安装」的判定，
    // 否则会出现「装不上却显示已安装」。
    let files: &[(&str, &[u8])] = &[
        ("nativePC/a.mod3", b"same"),
        ("outside/b.mod3", b"different"),
    ];
    let harness = harness(files, RecordingGameFs::new(&[("nativePC/a.mod3", b"same")]));

    let summary = harness.scan().expect("scan succeeds");

    assert_eq!(summary.files.len(), 1, "越界路径必须被过滤掉");
    assert_eq!(summary.state, ExternalInstallState::Installed);
}

// ---------------------------------------------------------------------------
// 锁的边界：这是本模块最要紧的两条
// ---------------------------------------------------------------------------

#[test]
fn hashing_never_happens_while_the_write_lock_is_held() {
    // 守的是项目 5 处文档明令的规则（ARCHITECTURE.md:616 等）。
    // 若实现退化成「拿写锁扫全程」，`reads_while_locked` 会 > 0，这条立刻变红。
    let files: &[(&str, &[u8])] = &[("nativePC/a.mod3", b"same"), ("nativePC/b.mod3", b"same")];
    let harness = harness(files, RecordingGameFs::new(files)).with_lock_probe();

    harness.scan().expect("scan succeeds");

    assert_eq!(
        harness.game_fs.reads_while_locked.load(Ordering::SeqCst),
        0,
        "哈希（读文件）发生在写锁内，违反「不要在持有游戏写锁时做长时间 hash」"
    );
}

#[test]
fn stat_happens_inside_the_write_lock() {
    // 与上一条互为对照：stat **必须**在锁内，否则 stage 1/3 的前后比对没有意义。
    // 两条一起才证明「锁内只有 stat，锁外才有 hash」——只写任一条都是半个护栏。
    let files: &[(&str, &[u8])] = &[("nativePC/a.mod3", b"same")];
    let harness = harness(files, RecordingGameFs::new(files)).with_lock_probe();

    harness.scan().expect("scan succeeds");

    assert!(
        harness.game_fs.stats_while_locked.load(Ordering::SeqCst) > 0,
        "stat 必须在写锁内进行，否则前后两次指纹比对毫无意义"
    );
}

// ---------------------------------------------------------------------------
// 降级语义：有写入进行中 → Stale（不等待、不硬失败）
// ---------------------------------------------------------------------------

#[test]
fn a_write_in_progress_is_reported_as_stale_instead_of_blocking() {
    let files: &[(&str, &[u8])] = &[("nativePC/a.mod3", b"same")];
    let harness = harness(files, RecordingGameFs::new(files));

    // 模拟安装正在进行：持有 (game, profile) 写锁。
    let _guard = harness.write_lock.lock().expect("hold write lock");

    let started = Instant::now();
    let error = harness.scan().expect_err("有写入进行中必须返回 Stale");
    let elapsed = started.elapsed();

    assert_eq!(error, ConfiguredExternalStateScanError::Stale);
    assert_eq!(error.code(), "external_state_scan_stale");
    // 关键：不能是阻塞等待。若实现用 `lock()` 而非 `try_lock()`，这里会卡住。
    assert!(
        elapsed < Duration::from_secs(2),
        "扫描阻塞了 {elapsed:?}——应当立刻返回 Stale，而不是等锁"
    );
}

// ---------------------------------------------------------------------------
// fail-closed：扫描期间被改动 → 丢弃结果
// ---------------------------------------------------------------------------

#[test]
fn files_changed_during_the_scan_are_reported_as_stale_and_the_result_is_discarded() {
    let files: &[(&str, &[u8])] = &[("nativePC/a.mod3", b"same")];
    // 第一次读之后把内容换成**长度不同**的，保证 stat 指纹一定变化。
    let game_fs = RecordingGameFs::new(files).mutate_after_reads(
        0,
        "nativePC/a.mod3",
        b"much-longer".to_vec(),
    );
    let harness = harness(files, game_fs);

    let error = harness.scan().expect_err("扫描期间被改动必须返回 Stale");

    assert_eq!(error, ConfiguredExternalStateScanError::Stale);
}

#[test]
fn a_file_appearing_during_the_scan_is_reported_as_stale() {
    // 缺失 → 出现，同样算漂移。只测「内容变了」会漏掉这一类
    // （排障手册 4.5：单侧断言是假绿的温床，两个方向都要覆盖）。
    let files: &[(&str, &[u8])] = &[("nativePC/a.mod3", b"same")];
    let game_fs = RecordingGameFs::new(&[]).mutate_after_reads(
        0,
        "nativePC/a.mod3",
        b"appeared-later".to_vec(),
    );
    let harness = harness(files, game_fs);

    let error = harness.scan().expect_err("文件从无到有必须返回 Stale");

    assert_eq!(error, ConfiguredExternalStateScanError::Stale);
}

// ---------------------------------------------------------------------------
// 语义不能互相冒充
// ---------------------------------------------------------------------------

#[test]
fn cancellation_is_reported_as_cancelled_not_stale() {
    let files: &[(&str, &[u8])] = &[("nativePC/a.mod3", b"same")];
    let harness = harness(files, RecordingGameFs::new(files));

    let error = harness
        .scanner
        .scan(ConfiguredExternalStateScanRequest {
            game_id: &GameId::mhw(),
            profile_id: &ProfileId::new("default"),
            mod_id: &ModId::new("mod-a"),
            cancellation_token: &Cancelled,
        })
        .expect_err("取消必须失败");

    // 若把 Cancelled 并进 Stale，这条会红——它会让界面把「已取消」
    // 说成「结果可能过期」。
    assert_eq!(error, ConfiguredExternalStateScanError::Cancelled);
    assert_eq!(error.code(), "external_state_scan_cancelled");
}

#[test]
fn error_codes_are_stable_and_distinct() {
    // 稳定码会经由 command 到前端取词。这里**逐个**钉住——用数量下限
    // （如 `len() >= 8`）的那种断言删掉一个 key 也不会红（排障手册 4.6）。
    let codes = [
        ConfiguredExternalStateScanError::GameInstanceUnavailable,
        ConfiguredExternalStateScanError::ModUnavailable,
        ConfiguredExternalStateScanError::SandboxUnavailable,
        ConfiguredExternalStateScanError::PackageScanFailed,
        ConfiguredExternalStateScanError::GameFileUnavailable,
        ConfiguredExternalStateScanError::Cancelled,
        ConfiguredExternalStateScanError::ScanUnavailable,
        ConfiguredExternalStateScanError::WriteAdmissionOrderViolation,
        ConfiguredExternalStateScanError::Stale,
    ]
    .map(|error| error.code());

    let expected = [
        "external_state_scan_game_instance_unavailable",
        "external_state_scan_mod_unavailable",
        "external_state_scan_sandbox_unavailable",
        "external_state_scan_package_scan_failed",
        "external_state_scan_game_file_unavailable",
        "external_state_scan_cancelled",
        "external_state_scan_unavailable",
        "external_state_scan_admission_order_violation",
        "external_state_scan_stale",
    ];
    assert_eq!(codes, expected, "稳定错误码变了");

    // 必须唯一，否则前端按码取词会串行。
    let mut unique = expected.to_vec();
    unique.sort_unstable();
    unique.dedup();
    assert_eq!(unique.len(), expected.len(), "存在重复的稳定错误码");
}

// ---------------------------------------------------------------------------
// 输入缺失
// ---------------------------------------------------------------------------

#[test]
fn a_missing_game_instance_is_reported_before_touching_the_sandbox() {
    let temp = tempfile::tempdir().expect("temp dir");
    let scanner = ConfiguredExternalStateScanner::new(
        Arc::new(FakeGameConfigRepository::none()),
        Arc::new(FakeModImportResultRepository {
            analysis: Some(analysis("mod-a", "package-a")),
        }),
        Arc::new(FixedSandboxLocator(temp.path().join("sandbox"))),
        ALLOWED_ROOTS
            .iter()
            .map(|root| (*root).to_owned())
            .collect(),
        Arc::new(GameProfileWriteLockRegistry::default()),
        Arc::new(FakeScanner::default()),
        Arc::new(FakePackageReader::default()),
        Arc::new(SharedGameFsFactory(Arc::new(RecordingGameFs::new(&[])))),
        8 * 1024 * 1024,
        DEFAULT_WORKER_LIMIT,
    );

    let error = scanner
        .scan(ConfiguredExternalStateScanRequest {
            game_id: &GameId::mhw(),
            profile_id: &ProfileId::new("default"),
            mod_id: &ModId::new("mod-a"),
            cancellation_token: &NeverCancelled,
        })
        .expect_err("游戏目录未配置必须失败");

    assert_eq!(
        error,
        ConfiguredExternalStateScanError::GameInstanceUnavailable
    );
}

#[test]
fn an_unknown_mod_id_is_reported_without_scanning() {
    let temp = tempfile::tempdir().expect("temp dir");
    let game_root = temp.path().join("game");
    std::fs::create_dir_all(&game_root).expect("create game root");
    let scanner = ConfiguredExternalStateScanner::new(
        Arc::new(FakeGameConfigRepository::with_root(game_root)),
        // 记录里只有 mod-a，这里查 mod-b。
        Arc::new(FakeModImportResultRepository {
            analysis: Some(analysis("mod-a", "package-a")),
        }),
        Arc::new(FixedSandboxLocator(temp.path().join("sandbox"))),
        ALLOWED_ROOTS
            .iter()
            .map(|root| (*root).to_owned())
            .collect(),
        Arc::new(GameProfileWriteLockRegistry::default()),
        Arc::new(FakeScanner::default()),
        Arc::new(FakePackageReader::default()),
        Arc::new(SharedGameFsFactory(Arc::new(RecordingGameFs::new(&[])))),
        8 * 1024 * 1024,
        DEFAULT_WORKER_LIMIT,
    );

    let error = scanner
        .scan(ConfiguredExternalStateScanRequest {
            game_id: &GameId::mhw(),
            profile_id: &ProfileId::new("default"),
            mod_id: &ModId::new("mod-b"),
            cancellation_token: &NeverCancelled,
        })
        .expect_err("未知 mod_id 必须失败");

    assert_eq!(error, ConfiguredExternalStateScanError::ModUnavailable);
}

#[test]
fn a_failing_package_scan_is_reported() {
    let temp = tempfile::tempdir().expect("temp dir");
    let game_root = temp.path().join("game");
    std::fs::create_dir_all(&game_root).expect("create game root");
    let scanner = ConfiguredExternalStateScanner::new(
        Arc::new(FakeGameConfigRepository::with_root(game_root)),
        Arc::new(FakeModImportResultRepository {
            analysis: Some(analysis("mod-a", "package-a")),
        }),
        Arc::new(FixedSandboxLocator(temp.path().join("sandbox"))),
        ALLOWED_ROOTS
            .iter()
            .map(|root| (*root).to_owned())
            .collect(),
        Arc::new(GameProfileWriteLockRegistry::default()),
        Arc::new(FakeScanner {
            files: vec![],
            fail: true,
        }),
        Arc::new(FakePackageReader::default()),
        Arc::new(SharedGameFsFactory(Arc::new(RecordingGameFs::new(&[])))),
        8 * 1024 * 1024,
        DEFAULT_WORKER_LIMIT,
    );

    let error = scanner
        .scan(ConfiguredExternalStateScanRequest {
            game_id: &GameId::mhw(),
            profile_id: &ProfileId::new("default"),
            mod_id: &ModId::new("mod-a"),
            cancellation_token: &NeverCancelled,
        })
        .expect_err("沙箱扫描失败必须上报");

    // 与 GameFileUnavailable 区分：这是包的问题，不是游戏目录的问题。
    assert_eq!(error, ConfiguredExternalStateScanError::PackageScanFailed);
}
