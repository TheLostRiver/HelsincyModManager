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
use hmm_app::{GameProfileWriteLockRegistry, TaskManager, TaskStatus};
use hmm_core::{
    ExternalFileState, ExternalInstallState, FileLayer, GameDirectoryStatus, GameId, GameInstance,
    InstallManifest, InstallManifestEntry, InstallTargetPath, ModId, PackageFileId, ProfileId,
};
use hmm_ports::{
    AppClock, CancellationToken, GameConfigRepository, GameConfigRepositoryError,
    GameConfigRepositoryResult, GameFileFingerprint, InstallGameFileInspector,
    InstallGameFileSystem, InstallManifestRepository, ModImportResultRepository,
    ModImportSandboxLocator, ModPackageInstallFile, ModPackageInstallFileReadRequest,
    ModPackageInstallFileReader, ModPackageInstallFileScanError, ModPackageInstallFileScanRequest,
    ModPackageInstallFileScanner, NeverCancelled, StoredImportPreviewImage,
    StoredModImportAnalysis,
};
use hmm_runtime::external_state_scan::{
    ConfiguredExternalStateScanError, ConfiguredExternalStateScanRequest,
    ConfiguredExternalStateScanner, ExternalStateScanCache, ExternalStateScanRecord,
    GameFileSystemFactory, GameFileSystemHandles,
};
use hmm_runtime::external_state_scan_tasks::{
    queued_scan_event, ExternalStateScanTaskService, EXTERNAL_STATE_SCAN_CANCELLED_PHASE,
    EXTERNAL_STATE_SCAN_COMPLETED_PHASE, EXTERNAL_STATE_SCAN_FAILED_PHASE,
    EXTERNAL_STATE_SCAN_PROCESSING_PHASE, EXTERNAL_STATE_SCAN_QUEUED_PHASE,
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
    instance: Mutex<Option<GameInstance>>,
}

impl FakeGameConfigRepository {
    fn with_root(root: impl Into<PathBuf>) -> Self {
        Self {
            instance: Mutex::new(Some(game_instance(root.into()))),
        }
    }

    fn none() -> Self {
        Self {
            instance: Mutex::new(None),
        }
    }

    /// 模拟「游戏实例在扫描之后被移除/读不到」。
    fn clear_instance(&self) {
        *self.instance.lock().expect("instance lock") = None;
    }
}

impl GameConfigRepository for FakeGameConfigRepository {
    fn load_game_instance(
        &self,
        _game_id: &GameId,
    ) -> GameConfigRepositoryResult<Option<GameInstance>> {
        Ok(self.instance.lock().expect("instance lock").clone())
    }

    fn save_game_instance(&self, _instance: &GameInstance) -> GameConfigRepositoryResult<()> {
        Err(GameConfigRepositoryError::StorageFailed(
            "tests do not save game instances".to_owned(),
        ))
    }
}

/// 归因（#286 第三层）用的清单假件。默认 `Ok(None)` = 该配置档从未有 HMM 安装。
#[derive(Default)]
struct FakeInstallManifestRepository {
    manifest: Mutex<Option<InstallManifest>>,
    fail: Mutex<bool>,
}

impl FakeInstallManifestRepository {
    fn set_manifest(&self, manifest: InstallManifest) {
        *self.manifest.lock().expect("manifest lock") = Some(manifest);
    }

    /// 模拟清单读取失败（IO 错误），而不是清单不存在。
    fn fail_loads(&self) {
        *self.fail.lock().expect("fail lock") = true;
    }
}

impl InstallManifestRepository for FakeInstallManifestRepository {
    fn load_manifest(&self, _profile_id: &ProfileId) -> anyhow::Result<Option<InstallManifest>> {
        if *self.fail.lock().expect("fail lock") {
            anyhow::bail!("manifest storage failed");
        }
        Ok(self.manifest.lock().expect("manifest lock").clone())
    }

    fn save_manifest(&self, _manifest: &InstallManifest) -> anyhow::Result<()> {
        anyhow::bail!("tests do not save manifests")
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

    /// 替换某个文件的内容（`None` = 存在但 stat/读失败）。
    fn set_file(&self, path: &str, bytes: Option<Vec<u8>>) {
        self.files
            .lock()
            .expect("files lock")
            .insert(path.to_owned(), bytes);
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

/// 固定时钟：让 `computedAt` 可预测，淘汰顺序因此可断言。
struct FixedClock(Mutex<u128>);

impl FixedClock {
    fn new(millis: u128) -> Self {
        Self(Mutex::new(millis))
    }
}

impl AppClock for FixedClock {
    fn now_unix_millis(&self) -> anyhow::Result<u128> {
        Ok(*self.0.lock().expect("clock lock"))
    }
}

/// 永远失败的时钟：用于验证取不到时间时不崩、且退化为「最旧」。
struct BrokenClock;

impl AppClock for BrokenClock {
    fn now_unix_millis(&self) -> anyhow::Result<u128> {
        anyhow::bail!("clock unavailable")
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
    game_config: Arc<FakeGameConfigRepository>,
    /// 归因用清单假件；默认「从未安装」，用例可注入条目或让读取失败。
    manifest_repository: Arc<FakeInstallManifestRepository>,
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
    let game_config = Arc::new(FakeGameConfigRepository::with_root(game_root));
    let manifest_repository = Arc::new(FakeInstallManifestRepository::default());
    let scanner = Arc::new(ConfiguredExternalStateScanner::new(
        Arc::clone(&game_config) as Arc<dyn GameConfigRepository>,
        Arc::new(FakeModImportResultRepository {
            analysis: Some(analysis("mod-a", "package-a")),
        }),
        Arc::new(FixedSandboxLocator(temp.path().join("sandbox"))),
        Arc::clone(&manifest_repository) as Arc<dyn InstallManifestRepository>,
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
        Arc::new(ExternalStateScanCache::new(Arc::new(FixedClock::new(
            1_000,
        )))),
        8 * 1024 * 1024,
        DEFAULT_WORKER_LIMIT,
    ));

    Harness {
        _temp: temp,
        scanner,
        game_fs,
        game_config,
        manifest_repository,
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
        ConfiguredExternalStateScanError::ManifestUnavailable,
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
        "external_state_scan_manifest_unavailable",
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
        Arc::new(FakeInstallManifestRepository::default()),
        ALLOWED_ROOTS
            .iter()
            .map(|root| (*root).to_owned())
            .collect(),
        Arc::new(GameProfileWriteLockRegistry::default()),
        Arc::new(FakeScanner::default()),
        Arc::new(FakePackageReader::default()),
        Arc::new(SharedGameFsFactory(Arc::new(RecordingGameFs::new(&[])))),
        Arc::new(ExternalStateScanCache::new(Arc::new(FixedClock::new(
            1_000,
        )))),
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
        Arc::new(FakeInstallManifestRepository::default()),
        ALLOWED_ROOTS
            .iter()
            .map(|root| (*root).to_owned())
            .collect(),
        Arc::new(GameProfileWriteLockRegistry::default()),
        Arc::new(FakeScanner::default()),
        Arc::new(FakePackageReader::default()),
        Arc::new(SharedGameFsFactory(Arc::new(RecordingGameFs::new(&[])))),
        Arc::new(ExternalStateScanCache::new(Arc::new(FixedClock::new(
            1_000,
        )))),
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
        Arc::new(FakeInstallManifestRepository::default()),
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
        Arc::new(ExternalStateScanCache::new(Arc::new(FixedClock::new(
            1_000,
        )))),
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

// ---------------------------------------------------------------------------
// 占用归因（#286 第三层）
// ---------------------------------------------------------------------------

fn manifest_entry(target: &str, mod_id: &str) -> InstallManifestEntry {
    let roots: Vec<String> = ALLOWED_ROOTS
        .iter()
        .map(|root| (*root).to_owned())
        .collect();
    InstallManifestEntry {
        target_path: InstallTargetPath::parse(target, &roots).expect("合法目标路径"),
        mod_id: ModId::new(mod_id),
        revision_id: None,
        package_file_id: PackageFileId::new(target),
        layer: FileLayer::new("base", 0),
        backup_ref: None,
        installed_file: None,
        adopted: false,
    }
}

#[test]
fn claims_from_the_manifest_are_recorded_and_returned_in_order() {
    let files: &[(&str, &[u8])] = &[("nativePC/a.mod3", b"same"), ("nativePC/b.mod3", b"same")];
    let harness = harness(files, RecordingGameFs::new(files));
    // a 归其他 MOD（mod-flat）；b 归被扫 MOD 自己——自己名下不算占用。
    harness
        .manifest_repository
        .set_manifest(InstallManifest::completed(
            ProfileId::new("default"),
            vec![
                manifest_entry("nativePC/a.mod3", "mod-flat"),
                manifest_entry("nativePC/b.mod3", "mod-a"),
            ],
        ));

    harness.scan().expect("scan succeeds");
    let query = harness.scanner.query(
        &GameId::mhw(),
        &ProfileId::new("default"),
        &ModId::new("mod-a"),
    );

    // 与 display_paths 同序：哈希判定不因占用而改变，归因是正交事实。
    assert_eq!(
        query.display_paths,
        vec!["nativePC/a.mod3".to_owned(), "nativePC/b.mod3".to_owned()],
    );
    assert_eq!(query.claimed_by, vec![Some(ModId::new("mod-flat")), None]);
    assert_eq!(
        query.summary.expect("summary present").state,
        ExternalInstallState::Installed,
    );
}

#[test]
fn claims_without_any_manifest_are_all_none() {
    let files: &[(&str, &[u8])] = &[("nativePC/a.mod3", b"same")];
    let harness = harness(files, RecordingGameFs::new(files));

    harness.scan().expect("scan succeeds");
    let query = harness.scanner.query(
        &GameId::mhw(),
        &ProfileId::new("default"),
        &ModId::new("mod-a"),
    );

    // 清单不存在 = 从未有 HMM 安装：长度仍与文件对齐，全为 None。
    assert_eq!(query.claimed_by, vec![None]);
}

#[test]
fn a_manifest_read_failure_fails_the_scan_and_keeps_the_previous_result() {
    let files: &[(&str, &[u8])] = &[("nativePC/a.mod3", b"same")];
    let harness = harness(files, RecordingGameFs::new(files));
    harness.scan().expect("first scan succeeds");

    harness.manifest_repository.fail_loads();
    let error = harness.scan().expect_err("清单读失败必须让扫描失败");

    // fail-closed：静默把占用报成「无占用」会复刻「外部已安装」的误导。
    assert_eq!(error, ConfiguredExternalStateScanError::ManifestUnavailable);
    assert_eq!(error.code(), "external_state_scan_manifest_unavailable");

    // 上次成功结果保留，失败原因如实记录——与其他失败同一降级口径。
    let query = harness.scanner.query(
        &GameId::mhw(),
        &ProfileId::new("default"),
        &ModId::new("mod-a"),
    );
    assert!(query.summary.is_some(), "上次成功结果必须保留");
    assert_eq!(
        query.last_error,
        Some(ConfiguredExternalStateScanError::ManifestUnavailable),
    );
}

// ---------------------------------------------------------------------------
// 结果存储
// ---------------------------------------------------------------------------

/// 直接测缓存本身，不经 scanner——这样每条断言只承担一个变量。
fn cache_with_clock(clock: Arc<FixedClock>) -> Arc<ExternalStateScanCache> {
    Arc::new(ExternalStateScanCache::new(clock))
}

#[test]
fn a_successful_scan_is_retrievable_and_not_stale() {
    let files: &[(&str, &[u8])] = &[("nativePC/a.mod3", b"same")];
    let harness = harness(files, RecordingGameFs::new(files));

    harness.scan().expect("scan succeeds");
    let query = harness.scanner.query(
        &GameId::mhw(),
        &ProfileId::new("default"),
        &ModId::new("mod-a"),
    );

    assert!(query.summary.is_some(), "成功扫描后必须能查到结果");
    assert!(!query.stale, "刚扫完不该是 stale");
    assert_eq!(query.last_error, None, "成功时不该有失败原因");
}

#[test]
fn a_changed_file_makes_the_cached_result_stale_and_keeps_the_old_summary() {
    // 这条守的是「stale 但保留旧结果」——清空它会让玩家什么也看不到。
    let files: &[(&str, &[u8])] = &[("nativePC/a.mod3", b"same")];
    let harness = harness(files, RecordingGameFs::new(files));
    harness.scan().expect("scan succeeds");

    // 扫完之后改文件（长度不同，stat 指纹必定变化）。
    harness
        .game_fs
        .set_file("nativePC/a.mod3", Some(b"much-longer-content".to_vec()));
    let query = harness.scanner.query(
        &GameId::mhw(),
        &ProfileId::new("default"),
        &ModId::new("mod-a"),
    );

    assert!(query.stale, "文件被改动后必须报 stale");
    // 关键：结果**保留**，不是清空。
    assert!(query.summary.is_some(), "stale 时仍要保留上次结果");
    assert_eq!(query.last_error, None, "stale 不等于「上次没扫成」");
}

#[test]
fn a_failed_scan_keeps_the_previous_result_and_records_the_reason() {
    // 这条守的是两个字段的**区分**：stale 与 last_error 是不同处境，
    // 合并成一个会让界面把「压根没扫」说成「可能已变」。
    let files: &[(&str, &[u8])] = &[("nativePC/a.mod3", b"same")];
    let harness = harness(files, RecordingGameFs::new(files));
    harness.scan().expect("first scan succeeds");

    // 持锁 → 第二次扫描必定失败（有写入进行中）。
    let _guard = harness.write_lock.lock().expect("hold write lock");
    let error = harness.scan().expect_err("有写入进行中必须失败");
    drop(_guard);

    assert_eq!(error, ConfiguredExternalStateScanError::Stale);
    let query = harness.scanner.query(
        &GameId::mhw(),
        &ProfileId::new("default"),
        &ModId::new("mod-a"),
    );

    assert_eq!(
        query.last_error,
        Some(ConfiguredExternalStateScanError::Stale),
        "失败原因必须单独记录"
    );
    assert!(query.summary.is_some(), "失败时必须保留上次成功的结果");
    // 文件没动，所以 stale 仍为 false——证明两个字段确实独立。
    assert!(
        !query.stale,
        "失败时 stale 不应被置位：文件并未变化，只是没扫成"
    );
}

#[test]
fn querying_an_unknown_mod_reports_nothing_without_error() {
    let cache = cache_with_clock(Arc::new(FixedClock::new(1_000)));
    let game_fs = RecordingGameFs::new(&[]);

    let query = cache.query(
        &GameId::mhw(),
        &ProfileId::new("default"),
        &ModId::new("never-scanned"),
        &game_fs,
    );

    assert_eq!(query.summary, None);
    assert!(!query.stale, "从没扫过谈不上过期");
    assert_eq!(query.last_error, None, "没扫过不等于失败");
}

#[test]
fn the_cache_evicts_the_oldest_entry_when_over_the_limit() {
    // 守的是容量上限：无上限的进程内缓存是 OOM 风险。
    let cache = Arc::new(ExternalStateScanCache::with_max_entries(
        Arc::new(FixedClock::new(1_000)),
        2,
    ));
    let game_fs = RecordingGameFs::new(&[]);

    for (index, _) in (0..3).enumerate() {
        let millis = 1_000 + u128::try_from(index).expect("index fits");
        cache.record_success(
            &GameId::mhw(),
            &ProfileId::new("default"),
            &ModId::new(format!("mod-{index}")),
            record_at(millis),
        );
    }

    // mod-0 最旧，应被淘汰；mod-1 与 mod-2 保留。
    assert!(
        cache
            .query(
                &GameId::mhw(),
                &ProfileId::new("default"),
                &ModId::new("mod-0"),
                &game_fs
            )
            .summary
            .is_none(),
        "最旧的条目必须被淘汰"
    );
    assert!(cache
        .query(
            &GameId::mhw(),
            &ProfileId::new("default"),
            &ModId::new("mod-1"),
            &game_fs
        )
        .summary
        .is_some());
    assert!(cache
        .query(
            &GameId::mhw(),
            &ProfileId::new("default"),
            &ModId::new("mod-2"),
            &game_fs
        )
        .summary
        .is_some());
}

#[test]
fn eviction_follows_computed_time_not_insertion_order() {
    // 与上面那条互补：按**时间**淘汰而不是按插入顺序。若实现退化成
    // 删第一个插入的，这条会红（mod-0 是最后写入但时间最早）。
    let cache = Arc::new(ExternalStateScanCache::with_max_entries(
        Arc::new(FixedClock::new(1_000)),
        2,
    ));
    let game_fs = RecordingGameFs::new(&[]);

    // 故意乱序：先写时间戳大的，最后写时间戳小的。
    cache.record_success(
        &GameId::mhw(),
        &ProfileId::new("default"),
        &ModId::new("mod-new"),
        record_at(3_000),
    );
    cache.record_success(
        &GameId::mhw(),
        &ProfileId::new("default"),
        &ModId::new("mod-mid"),
        record_at(2_000),
    );
    cache.record_success(
        &GameId::mhw(),
        &ProfileId::new("default"),
        &ModId::new("mod-old"),
        record_at(1_000),
    );

    assert!(
        cache
            .query(
                &GameId::mhw(),
                &ProfileId::new("default"),
                &ModId::new("mod-old"),
                &game_fs
            )
            .summary
            .is_none(),
        "应按 computedAt 淘汰最旧的，而不是按插入顺序删第一个"
    );
    assert!(cache
        .query(
            &GameId::mhw(),
            &ProfileId::new("default"),
            &ModId::new("mod-new"),
            &game_fs
        )
        .summary
        .is_some());
    assert!(cache
        .query(
            &GameId::mhw(),
            &ProfileId::new("default"),
            &ModId::new("mod-mid"),
            &game_fs
        )
        .summary
        .is_some());
}

#[test]
fn a_failed_scan_without_any_previous_result_has_no_summary() {
    // 「从没成功过」与「成功过但这次失败」必须区分：前者界面该显示
    // 「尚未检查」，后者该显示旧结果 + 失败原因。
    let cache = cache_with_clock(Arc::new(FixedClock::new(1_000)));
    let game_fs = RecordingGameFs::new(&[]);

    cache.record_failure(
        &GameId::mhw(),
        &ProfileId::new("default"),
        &ModId::new("mod-a"),
        ConfiguredExternalStateScanError::Stale,
    );
    let query = cache.query(
        &GameId::mhw(),
        &ProfileId::new("default"),
        &ModId::new("mod-a"),
        &game_fs,
    );

    assert_eq!(query.summary, None, "没有成功过就不该有结果");
    assert_eq!(
        query.last_error,
        Some(ConfiguredExternalStateScanError::Stale)
    );
    assert!(!query.stale);
}

#[test]
fn the_same_mod_id_in_different_profiles_does_not_collide() {
    // 键必须含 profile_id：同一个 MOD 在不同 profile 下状态不同，
    // 串了会让玩家看到另一个 profile 的结论。
    let cache = cache_with_clock(Arc::new(FixedClock::new(1_000)));
    let game_fs = RecordingGameFs::new(&[]);

    cache.record_success(
        &GameId::mhw(),
        &ProfileId::new("profile-a"),
        &ModId::new("mod-a"),
        record_at(1_000),
    );

    assert!(cache
        .query(
            &GameId::mhw(),
            &ProfileId::new("profile-a"),
            &ModId::new("mod-a"),
            &game_fs
        )
        .summary
        .is_some());
    assert!(
        cache
            .query(
                &GameId::mhw(),
                &ProfileId::new("profile-b"),
                &ModId::new("mod-a"),
                &game_fs
            )
            .summary
            .is_none(),
        "不同 profile 必须互不干扰"
    );
}

#[test]
fn a_failure_records_a_new_failure_reason_over_the_old_one() {
    // 失败原因要被覆盖：否则玩家会一直看到很久以前那次的原因。
    let cache = cache_with_clock(Arc::new(FixedClock::new(1_000)));
    let game_fs = RecordingGameFs::new(&[]);

    cache.record_failure(
        &GameId::mhw(),
        &ProfileId::new("default"),
        &ModId::new("mod-a"),
        ConfiguredExternalStateScanError::Stale,
    );
    cache.record_failure(
        &GameId::mhw(),
        &ProfileId::new("default"),
        &ModId::new("mod-a"),
        ConfiguredExternalStateScanError::GameFileUnavailable,
    );

    let query = cache.query(
        &GameId::mhw(),
        &ProfileId::new("default"),
        &ModId::new("mod-a"),
        &game_fs,
    );
    assert_eq!(
        query.last_error,
        Some(ConfiguredExternalStateScanError::GameFileUnavailable),
        "新的失败原因必须覆盖旧的"
    );
}

#[test]
fn a_successful_scan_clears_the_previous_failure_reason() {
    // 失败后成功：不该再显示「上次没扫成」。
    let cache = cache_with_clock(Arc::new(FixedClock::new(1_000)));
    let game_fs = RecordingGameFs::new(&[]);

    cache.record_failure(
        &GameId::mhw(),
        &ProfileId::new("default"),
        &ModId::new("mod-a"),
        ConfiguredExternalStateScanError::Stale,
    );
    cache.record_success(
        &GameId::mhw(),
        &ProfileId::new("default"),
        &ModId::new("mod-a"),
        record_at(1_000),
    );

    let query = cache.query(
        &GameId::mhw(),
        &ProfileId::new("default"),
        &ModId::new("mod-a"),
        &game_fs,
    );
    assert_eq!(query.last_error, None, "成功后必须清掉失败原因");
    assert!(query.summary.is_some());
}

#[test]
fn an_unavailable_clock_degrades_to_the_oldest_instead_of_panicking() {
    // 取不到时间不该崩，也不该让功能失效——它只用于淘汰排序。
    let cache = Arc::new(ExternalStateScanCache::with_max_entries(
        Arc::new(BrokenClock),
        2,
    ));
    let game_fs = RecordingGameFs::new(&[]);

    for index in 0..3 {
        cache.record_success(
            &GameId::mhw(),
            &ProfileId::new("default"),
            &ModId::new(format!("mod-{index}")),
            record_at(0),
        );
    }

    // 三条都退化成时间戳 0，淘汰仍能收敛到上限内（不 panic、不无限循环）。
    assert!(cache
        .query(
            &GameId::mhw(),
            &ProfileId::new("default"),
            &ModId::new("mod-2"),
            &game_fs
        )
        .summary
        .is_some());
}

#[test]
fn eviction_is_deterministic_when_timestamps_tie() {
    // **这条是上面那条修强后的版本**，也是本轮唯一真正守住 tie-break 的用例。
    //
    // 背景：时间戳并列是真实场景（时钟取不到时间时全部退化为 0；或同一毫秒内
    // 连续扫多个 MOD）。`min_by_key` 在并列时返回 HashMap **迭代顺序**里的第一个
    // ——那个顺序是随机的，表现为「单独跑过、整组跑挂」的幽灵失败。
    //
    // 为什么上面那条守不住：它只断言 mod-2 还在，而随机顺序下 mod-2 有 2/3
    // 概率留下——实测 8 次里只红 2 次。这种用例比没有更危险，它给的是虚假安全感。
    //
    // 这条改为断言**确定性语义**：时间戳全并列时，被淘汰的必须是 key 最小的。
    // 无论迭代顺序怎么变，这个结论都不变，因此它**必然**能抓到 tie-break 缺失。
    let game_fs = RecordingGameFs::new(&[]);

    for _ in 0..32 {
        let cache = Arc::new(ExternalStateScanCache::with_max_entries(
            Arc::new(FixedClock::new(1_000)),
            2,
        ));
        // 故意用**乱序插入** + **相同时间戳**，最大化暴露迭代顺序的影响。
        for name in ["mod-c", "mod-a", "mod-b"] {
            cache.record_success(
                &GameId::mhw(),
                &ProfileId::new("default"),
                &ModId::new(name),
                record_at(5_000),
            );
        }

        let survivor = |name: &str| {
            cache
                .query(
                    &GameId::mhw(),
                    &ProfileId::new("default"),
                    &ModId::new(name),
                    &game_fs,
                )
                .summary
                .is_some()
        };
        assert!(
            !survivor("mod-a"),
            "时间戳并列时必须淘汰 key 最小的（mod-a），结果却不确定"
        );
        assert!(survivor("mod-b") && survivor("mod-c"));
    }
}

#[test]
fn stat_failure_during_query_is_reported_as_stale() {
    // 拿不到当前事实就无法证明结果仍成立 → 如实说「不确定」，方向是 fail-closed。
    let files: &[(&str, &[u8])] = &[("nativePC/a.mod3", b"same")];
    let harness = harness(files, RecordingGameFs::new(files));
    harness.scan().expect("scan succeeds");

    // 让该文件变成「存在但 stat 失败」。
    harness.game_fs.set_file("nativePC/a.mod3", None);
    let query = harness.scanner.query(
        &GameId::mhw(),
        &ProfileId::new("default"),
        &ModId::new("mod-a"),
    );

    assert!(query.stale, "stat 失败必须报 stale，不能赌它没变");
    assert!(query.summary.is_some(), "仍保留上次结果");
}

/// 构造一条记录。内容无所谓（只测缓存行为），`computed_at` 才是关键。
fn record_at(computed_at_unix_millis: u128) -> ExternalStateScanRecord {
    ExternalStateScanRecord {
        summary: hmm_core::summarize_external_install_state(&[]),
        prepared: Vec::new(),
        fingerprints: Vec::new(),
        claimed_by: Vec::new(),
        game_files: Vec::new(),
        computed_at_unix_millis,
    }
}

// ---------------------------------------------------------------------------
// 查询语义：游戏实例不可用（#307 自审遗留，本轮定死）
// ---------------------------------------------------------------------------

#[test]
fn a_query_without_a_game_instance_keeps_the_summary_and_reports_stale() {
    let files: &[(&str, &[u8])] = &[("nativePC/a.mod3", b"same")];
    let harness = harness(files, RecordingGameFs::new(files));
    harness.scan().expect("scan succeeds");

    harness.game_config.clear_instance();
    let query = harness.scanner.query(
        &GameId::mhw(),
        &ProfileId::new("default"),
        &ModId::new("mod-a"),
    );

    let summary = query.summary.expect("游戏实例读不到时必须保留上次结果");
    assert_eq!(summary.state, ExternalInstallState::Installed);
    assert!(query.stale, "无法 stat 就无法证实结果，必须报 stale");
    assert_eq!(
        query.last_error, None,
        "查询期的处境不得冒充「上次扫描失败原因」"
    );
}

#[test]
fn a_query_without_a_game_instance_and_without_history_reports_nothing() {
    let files: &[(&str, &[u8])] = &[("nativePC/a.mod3", b"same")];
    let harness = harness(files, RecordingGameFs::new(files));

    harness.game_config.clear_instance();
    let query = harness.scanner.query(
        &GameId::mhw(),
        &ProfileId::new("default"),
        &ModId::new("mod-a"),
    );

    assert_eq!(query.summary, None);
    assert!(!query.stale, "从没扫过，谈不上过期");
    assert_eq!(query.last_error, None);
}

// ---------------------------------------------------------------------------
// 查询的文件级明细
// ---------------------------------------------------------------------------

#[test]
fn query_display_paths_align_with_per_file_states() {
    // 一个在且一致、一个缺失：明细必须能指出各自是谁。
    let package: &[(&str, &[u8])] = &[("nativePC/a.mod3", b"same"), ("nativePC/b.mod3", b"gone")];
    let harness = harness(
        package,
        RecordingGameFs::new(&[("nativePC/a.mod3", b"same")]),
    );
    harness.scan().expect("scan succeeds");

    let query = harness.scanner.query(
        &GameId::mhw(),
        &ProfileId::new("default"),
        &ModId::new("mod-a"),
    );

    assert_eq!(query.display_paths, ["nativePC/a.mod3", "nativePC/b.mod3"]);
    let summary = query.summary.expect("summary");
    assert_eq!(
        summary.files,
        [ExternalFileState::Matched, ExternalFileState::Missing],
        "states 与 display_paths 必须同序——错位会把「缺的是 b」说成「缺的是 a」"
    );
}

// ---------------------------------------------------------------------------
// 任务服务（切片 2b-3）
// ---------------------------------------------------------------------------

fn task_service(harness: &Harness) -> (Arc<TaskManager>, ExternalStateScanTaskService) {
    let task_manager = Arc::new(TaskManager::new());
    let service =
        ExternalStateScanTaskService::new(Arc::clone(&task_manager), Arc::clone(&harness.scanner));
    (task_manager, service)
}

fn start_launch(
    service: &ExternalStateScanTaskService,
    mod_id: &str,
) -> hmm_runtime::ExternalStateScanTaskLaunch {
    service
        .start_scan(GameId::mhw(), ProfileId::new("default"), ModId::new(mod_id))
        .expect("start scan")
}

#[test]
fn a_successful_scan_task_completes_with_ordered_phases_and_no_paths_in_events() {
    let files: &[(&str, &[u8])] = &[("nativePC/a.mod3", b"same")];
    let harness = harness(files, RecordingGameFs::new(files));
    let (task_manager, service) = task_service(&harness);

    let launch = start_launch(&service, "mod-a");
    assert!(launch.task.task_id.starts_with("external-state-scan-"));
    let queued = queued_scan_event(&launch);
    assert_eq!(queued.phase, EXTERNAL_STATE_SCAN_QUEUED_PHASE);
    assert_eq!(queued.result_ref.as_deref(), Some("mod-a"));

    let events = service.run_scan(launch.clone()).expect("run scan");

    assert_eq!(
        task_manager.task_status(&launch.task.task_id),
        Some(TaskStatus::Completed)
    );
    assert_eq!(
        events
            .iter()
            .map(|event| event.phase.as_str())
            .collect::<Vec<_>>(),
        [
            EXTERNAL_STATE_SCAN_PROCESSING_PHASE,
            EXTERNAL_STATE_SCAN_COMPLETED_PHASE
        ]
    );
    // 契约红线：事件 payload 不携带任何目标路径——结果只能通过 getter 拿。
    for event in &events {
        assert!(
            !format!("{event:?}").contains("nativePC"),
            "进度事件不得携带目标路径：{event:?}"
        );
        assert_eq!(event.result_ref.as_deref(), Some("mod-a"));
    }
    // 结果已在存储里，getter 可见。
    let query = harness.scanner.query(
        &GameId::mhw(),
        &ProfileId::new("default"),
        &ModId::new("mod-a"),
    );
    assert_eq!(
        query.summary.expect("summary").state,
        ExternalInstallState::Installed
    );
}

#[test]
fn a_cancellation_before_the_runner_starts_yields_only_a_cancelled_event() {
    let files: &[(&str, &[u8])] = &[("nativePC/a.mod3", b"same")];
    let harness = harness(files, RecordingGameFs::new(files));
    let (task_manager, service) = task_service(&harness);

    let launch = start_launch(&service, "mod-a");
    task_manager
        .cancel_task(&launch.task.task_id)
        .expect("cancel task");

    let events = service.run_scan(launch.clone()).expect("run scan");

    assert_eq!(events.len(), 1, "取消后不该再跑扫描阶段");
    assert_eq!(events[0].phase, EXTERNAL_STATE_SCAN_CANCELLED_PHASE);
    assert_eq!(events[0].status, TaskStatus::Cancelled);
    assert_eq!(events[0].error, None, "取消不是失败，不得贴错误码");
    // 取消发生在扫描前，不得留下结果。
    let query = harness.scanner.query(
        &GameId::mhw(),
        &ProfileId::new("default"),
        &ModId::new("mod-a"),
    );
    assert_eq!(query.summary, None);
}

#[test]
fn a_failing_scan_marks_the_task_failed_with_the_stable_error_code() {
    let files: &[(&str, &[u8])] = &[("nativePC/a.mod3", b"same")];
    let harness = harness(files, RecordingGameFs::new(files));
    let (task_manager, service) = task_service(&harness);

    // harness 只登记了 mod-a 的导入分析：扫 mod-b 必然 ModUnavailable。
    let launch = start_launch(&service, "mod-b");
    let events = service.run_scan(launch.clone()).expect("run scan");

    assert_eq!(
        task_manager.task_status(&launch.task.task_id),
        Some(TaskStatus::Failed)
    );
    assert_eq!(
        events
            .iter()
            .map(|event| event.phase.as_str())
            .collect::<Vec<_>>(),
        [
            EXTERNAL_STATE_SCAN_PROCESSING_PHASE,
            EXTERNAL_STATE_SCAN_FAILED_PHASE
        ]
    );
    assert_eq!(
        events[1].error.as_deref(),
        Some("external_state_scan_mod_unavailable"),
        "失败事件必须携带稳定错误码"
    );
    // 失败原因同时进存储，getter 可见。
    let query = harness.scanner.query(
        &GameId::mhw(),
        &ProfileId::new("default"),
        &ModId::new("mod-b"),
    );
    assert_eq!(
        query.last_error,
        Some(ConfiguredExternalStateScanError::ModUnavailable)
    );
}

#[test]
fn a_write_in_progress_fails_the_task_as_stale_instead_of_blocking() {
    let files: &[(&str, &[u8])] = &[("nativePC/a.mod3", b"same")];
    let harness = harness(files, RecordingGameFs::new(files));
    let (task_manager, service) = task_service(&harness);

    let launch = start_launch(&service, "mod-a");
    // 模拟安装进行中：持有同一把 (game, profile) 写锁。
    let guard = harness.write_lock.lock().expect("hold write lock");
    let started = Instant::now();
    let events = service.run_scan(launch.clone()).expect("run scan");
    let elapsed = started.elapsed();
    drop(guard);

    assert!(
        elapsed < Duration::from_secs(5),
        "写锁被持有时必须立刻降级，不能阻塞等待（耗时 {elapsed:?}）"
    );
    assert_eq!(
        task_manager.task_status(&launch.task.task_id),
        Some(TaskStatus::Failed)
    );
    assert_eq!(
        events.last().expect("terminal event").error.as_deref(),
        Some("external_state_scan_stale")
    );
}

#[test]
fn an_unemitted_queued_launch_can_be_aborted_to_a_terminal_state() {
    let files: &[(&str, &[u8])] = &[("nativePC/a.mod3", b"same")];
    let harness = harness(files, RecordingGameFs::new(files));
    let (task_manager, service) = task_service(&harness);

    let launch = start_launch(&service, "mod-a");
    service.abort_queued_scan(&launch).expect("abort");

    assert_eq!(
        task_manager.task_status(&launch.task.task_id),
        Some(TaskStatus::Failed),
        "废弃的 launch 不能停留在 queued——那是前端永远等不到的任务"
    );
    // 已终态后再 abort 是幂等的。
    service.abort_queued_scan(&launch).expect("abort again");
}
