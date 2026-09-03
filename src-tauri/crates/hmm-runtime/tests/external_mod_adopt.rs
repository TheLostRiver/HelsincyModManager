//! `ConfiguredExternalModAdopter` / `ExternalModAdoptTaskService` 的行为测试（#286 adopt）。
//!
//! ## 这些用例守的是什么
//!
//! 接管是 #286 唯一有写入的一片，判定本身在 `hmm-app::external_adopt`（纯函数，已有覆盖）。
//! 这里守的是**写事务的边界**：
//!
//! 1. 写出来的必须等于用户确认的那份扫描结果——磁盘或清单任一漂移都拒绝（Stale），
//!    不能「顺手」写一份用户没看过的清单。
//! 2. 唯一的落盘是清单：不碰游戏文件、不写备份、不建 recovery 记录；失败即无副作用。
//! 3. 前置拒绝（无记录 / 含 unreadable / 认领集为空）零副作用，锁都不拿。
//! 4. 复核与写入都在 `(game, profile)` 写锁内；有安装在进行就**等**，不是报 stale。
//! 5. 审计、任务事件只带 id 与计数，不带路径；审计写失败是显式降级，不改写成功事实。
//!
//! ## 装配方式
//!
//! 用**真实**的 `ConfiguredExternalStateScanner` 产出缓存记录，再交给接管器——接管依赖的
//! `game_files` / `fingerprints` / `claimed_by` 三列是否被扫描正确填好，只有端到端才测得到。
//! 锁状态由假件自己探测（与扫描测试同一手法）：探针就是 `write_locks.lock_for(...)`
//! 返回的同一把锁，`try_lock` 失败即说明此刻锁被持有，生产代码不留测试专用分支。
//!
//! 每条用例都跑过控制组：把实现退回去，确认它会变红。

use hmm_app::external_state_scan::DEFAULT_WORKER_LIMIT;
use hmm_app::{
    GameProfileWriteLockRegistry, InstallWriteAdmission, InstallWriteAdmissionError, TaskKind,
    TaskManager, TaskStatus,
};
use hmm_core::{
    installed_file_summary, ExternalFileState, ExternalInstallState, FileLayer,
    GameDirectoryStatus, GameId, GameInstance, InstallManifest, InstallManifestEntry,
    InstallManifestStatus, InstallTargetPath, ModId, PackageFileId, ProfileId,
};
use hmm_ports::{
    AppClock, AuditLogEvent, AuditLogWriter, CancellationToken, CrossProcessWriteAdmissionError,
    GameConfigRepository, GameConfigRepositoryError, GameConfigRepositoryResult,
    GameFileFingerprint, InstallGameFileInspector, InstallGameFileSystem,
    InstallManifestRepository, ModImportResultRepository, ModImportSandboxLocator,
    ModPackageInstallFile, ModPackageInstallFileReadRequest, ModPackageInstallFileReader,
    ModPackageInstallFileScanError, ModPackageInstallFileScanRequest, ModPackageInstallFileScanner,
    NeverCancelled, StoredImportPreviewImage, StoredModImportAnalysis,
};
use hmm_runtime::external_mod_adopt::{
    ConfiguredExternalModAdoptError, ConfiguredExternalModAdoptRequest,
    ConfiguredExternalModAdopter, ExternalModAdoptOutcome,
};
use hmm_runtime::external_mod_adopt_tasks::{
    queued_adopt_event, ExternalModAdoptTaskLaunch, ExternalModAdoptTaskService,
    EXTERNAL_MOD_ADOPT_AUDIT_UNAVAILABLE_CODE, EXTERNAL_MOD_ADOPT_CANCELLED_PHASE,
    EXTERNAL_MOD_ADOPT_COMPLETED_PHASE, EXTERNAL_MOD_ADOPT_FAILED_PHASE,
    EXTERNAL_MOD_ADOPT_PROCESSING_PHASE, EXTERNAL_MOD_ADOPT_QUEUED_PHASE,
};
use hmm_runtime::external_state_scan::{
    ConfiguredExternalStateScanRequest, ConfiguredExternalStateScanner, ExternalStateScanCache,
    ExternalStateScanRecord, GameFileSystemFactory, GameFileSystemHandles,
};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime};
use tempfile::TempDir;

const ALLOWED_ROOTS: &[&str] = &["nativePC"];
const CLOCK_MILLIS: u128 = 1_700_000_000_000;

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

/// 清单假件：记录每一次保存，保存后 `load` 读到的就是刚保存的那份（与真实仓储一致）。
///
/// 保存时跑与真实仓储相同的 `validate()`：接管写出的清单必须能过同一道校验。
/// 探针与扫描测试同一手法，用来证明「保存发生在写锁内」。
#[derive(Default)]
struct RecordingManifestRepository {
    manifest: Mutex<Option<InstallManifest>>,
    saved: Mutex<Vec<InstallManifest>>,
    fail_loads: Mutex<bool>,
    fail_saves: Mutex<bool>,
    probe: Mutex<Option<Arc<Mutex<()>>>>,
    saves_while_locked: AtomicUsize,
    last_save_at: Mutex<Option<Instant>>,
}

impl RecordingManifestRepository {
    fn set_manifest(&self, manifest: InstallManifest) {
        *self.manifest.lock().expect("manifest lock") = Some(manifest);
    }

    fn fail_loads(&self) {
        *self.fail_loads.lock().expect("fail lock") = true;
    }

    fn fail_saves(&self) {
        *self.fail_saves.lock().expect("fail lock") = true;
    }

    fn bind_probe(&self, probe: Arc<Mutex<()>>) {
        *self.probe.lock().expect("probe lock") = Some(probe);
    }

    fn saved(&self) -> Vec<InstallManifest> {
        self.saved.lock().expect("saved lock").clone()
    }

    fn locked_now(&self) -> bool {
        match self.probe.lock().expect("probe lock").as_ref() {
            Some(lock) => lock.try_lock().is_err(),
            None => false,
        }
    }
}

impl InstallManifestRepository for RecordingManifestRepository {
    fn load_manifest(&self, _profile_id: &ProfileId) -> anyhow::Result<Option<InstallManifest>> {
        if *self.fail_loads.lock().expect("fail lock") {
            anyhow::bail!("manifest storage failed");
        }
        Ok(self.manifest.lock().expect("manifest lock").clone())
    }

    fn save_manifest(&self, manifest: &InstallManifest) -> anyhow::Result<()> {
        if self.locked_now() {
            self.saves_while_locked.fetch_add(1, Ordering::SeqCst);
        }
        *self.last_save_at.lock().expect("save time lock") = Some(Instant::now());
        if *self.fail_saves.lock().expect("fail lock") {
            anyhow::bail!("manifest storage failed");
        }
        manifest
            .validate()
            .map_err(|error| anyhow::anyhow!("invalid manifest: {error:?}"))?;
        self.saved
            .lock()
            .expect("saved lock")
            .push(manifest.clone());
        *self.manifest.lock().expect("manifest lock") = Some(manifest.clone());
        Ok(())
    }
}

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

struct FakeScanner {
    files: Vec<ModPackageInstallFile>,
}

impl ModPackageInstallFileScanner for FakeScanner {
    fn scan_install_files(
        &self,
        _request: ModPackageInstallFileScanRequest<'_>,
    ) -> Result<Vec<ModPackageInstallFile>, ModPackageInstallFileScanError> {
        Ok(self.files.clone())
    }
}

struct FakePackageReader {
    bytes_by_id: HashMap<String, Vec<u8>>,
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

/// 游戏目录侧假件。`read_fails` 里的路径 stat 正常但读取失败——这就是真机上
/// 「文件被游戏独占打开」的样子：元数据读得到，内容读不到。
struct RecordingGameFs {
    files: Mutex<HashMap<String, Vec<u8>>>,
    read_fails: Mutex<HashSet<String>>,
    probe: Mutex<Option<Arc<Mutex<()>>>>,
    stats_while_locked: AtomicUsize,
    read_count: AtomicUsize,
    write_attempts: AtomicUsize,
}

impl RecordingGameFs {
    fn new(entries: &[(&str, &[u8])]) -> Self {
        let mut files = HashMap::new();
        for (path, bytes) in entries {
            files.insert((*path).to_owned(), bytes.to_vec());
        }
        Self {
            files: Mutex::new(files),
            read_fails: Mutex::new(HashSet::new()),
            probe: Mutex::new(None),
            stats_while_locked: AtomicUsize::new(0),
            read_count: AtomicUsize::new(0),
            write_attempts: AtomicUsize::new(0),
        }
    }

    fn bind_probe(&self, probe: Arc<Mutex<()>>) {
        *self.probe.lock().expect("probe lock") = Some(probe);
    }

    fn set_file(&self, path: &str, bytes: &[u8]) {
        self.files
            .lock()
            .expect("files lock")
            .insert(path.to_owned(), bytes.to_vec());
    }

    fn remove_file(&self, path: &str) {
        self.files.lock().expect("files lock").remove(path);
    }

    fn make_unreadable(&self, path: &str) {
        self.read_fails
            .lock()
            .expect("read_fails lock")
            .insert(path.to_owned());
    }

    fn locked_now(&self) -> bool {
        match self.probe.lock().expect("probe lock").as_ref() {
            Some(lock) => lock.try_lock().is_err(),
            None => false,
        }
    }
}

/// 由 size 派生 mtime：长度不同 → 指纹不同。stat 本来就检测不到「等长不同内容」，
/// 所以用例用改变长度制造漂移——这符合生产语义，不是迁就实现。
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
        self.read_count.fetch_add(1, Ordering::SeqCst);
        if self
            .read_fails
            .lock()
            .expect("read_fails lock")
            .contains(target_path.as_str())
        {
            anyhow::bail!("sharing violation");
        }
        Ok(self
            .files
            .lock()
            .expect("files lock")
            .get(target_path.as_str())
            .cloned())
    }

    fn write_game_file(
        &self,
        _target_path: &InstallTargetPath,
        _bytes: &[u8],
    ) -> anyhow::Result<()> {
        self.write_attempts.fetch_add(1, Ordering::SeqCst);
        anyhow::bail!("adopt must never write game files")
    }

    fn remove_game_file(&self, _target_path: &InstallTargetPath) -> anyhow::Result<()> {
        self.write_attempts.fetch_add(1, Ordering::SeqCst);
        anyhow::bail!("adopt must never remove game files")
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
        Ok(self
            .files
            .lock()
            .expect("files lock")
            .get(target_path.as_str())
            .map(|bytes| fake_fingerprint(bytes.len() as u64)))
    }
}

struct SharedGameFsFactory(Arc<RecordingGameFs>);

impl GameFileSystemFactory for SharedGameFsFactory {
    fn create(&self, _game_root: &Path) -> GameFileSystemHandles {
        let fs = Arc::clone(&self.0);
        let inspector = Arc::clone(&self.0);
        GameFileSystemHandles { fs, inspector }
    }
}

struct FixedClock(u128);

impl AppClock for FixedClock {
    fn now_unix_millis(&self) -> anyhow::Result<u128> {
        Ok(self.0)
    }
}

/// 写入准入假件：默认放行；用例可让它以某个稳定码拒绝（沙箱根 / recovery pending）。
#[derive(Default)]
struct FakeWriteAdmission {
    rejection: Mutex<Option<InstallWriteAdmissionError>>,
    calls: AtomicUsize,
}

impl FakeWriteAdmission {
    fn reject_with(&self, error: InstallWriteAdmissionError) {
        *self.rejection.lock().expect("rejection lock") = Some(error);
    }
}

impl InstallWriteAdmission for FakeWriteAdmission {
    fn ensure_write_allowed(
        &self,
        _game_id: &GameId,
        _profile_id: &ProfileId,
    ) -> Result<(), InstallWriteAdmissionError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        match self.rejection.lock().expect("rejection lock").clone() {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }
}

#[derive(Default)]
struct RecordingAuditLog {
    events: Mutex<Vec<AuditLogEvent>>,
    fail: Mutex<bool>,
}

impl RecordingAuditLog {
    fn fail_writes(&self) {
        *self.fail.lock().expect("fail lock") = true;
    }

    fn events(&self) -> Vec<AuditLogEvent> {
        self.events.lock().expect("events lock").clone()
    }
}

impl AuditLogWriter for RecordingAuditLog {
    fn record(&self, event: AuditLogEvent) -> anyhow::Result<()> {
        if *self.fail.lock().expect("fail lock") {
            anyhow::bail!("audit storage failed");
        }
        self.events.lock().expect("events lock").push(event);
        Ok(())
    }
}

struct Cancelled;

impl CancellationToken for Cancelled {
    fn is_cancelled(&self) -> bool {
        true
    }
}

/// 前 `n` 次询问回答「未取消」，之后回答「已取消」：用来命中锁内那一次取消检查。
struct CancelAfter {
    remaining: AtomicUsize,
}

impl CancellationToken for CancelAfter {
    fn is_cancelled(&self) -> bool {
        let previous = self.remaining.load(Ordering::SeqCst);
        if previous == 0 {
            return true;
        }
        self.remaining.store(previous - 1, Ordering::SeqCst);
        false
    }
}

// ---------------------------------------------------------------------------
// 装配
// ---------------------------------------------------------------------------

struct Harness {
    _temp: TempDir,
    scanner: Arc<ConfiguredExternalStateScanner>,
    adopter: Arc<ConfiguredExternalModAdopter>,
    task_manager: Arc<TaskManager>,
    game_fs: Arc<RecordingGameFs>,
    game_config: Arc<FakeGameConfigRepository>,
    manifest_repository: Arc<RecordingManifestRepository>,
    write_admission: Arc<FakeWriteAdmission>,
    audit_log: Arc<RecordingAuditLog>,
    scan_cache: Arc<ExternalStateScanCache>,
    /// 与 scanner / adopter 内部 `lock_for` 返回的是同一把锁。
    write_lock: Arc<Mutex<()>>,
}

/// `package_files`：导入包里的文件（id == 目标路径）。`game_files`：游戏目录里实际有的文件。
fn harness(package_files: &[(&str, &[u8])], game_files: &[(&str, &[u8])]) -> Harness {
    let temp = tempfile::tempdir().expect("temp dir");
    let game_root = temp.path().join("game");
    std::fs::create_dir_all(&game_root).expect("create game root");

    let write_locks = Arc::new(GameProfileWriteLockRegistry::default());
    let write_lock = write_locks.lock_for(&GameId::mhw(), &ProfileId::new("default"));

    let game_fs = Arc::new(RecordingGameFs::new(game_files));
    game_fs.bind_probe(Arc::clone(&write_lock));
    let game_config = Arc::new(FakeGameConfigRepository::with_root(game_root));
    let manifest_repository = Arc::new(RecordingManifestRepository::default());
    manifest_repository.bind_probe(Arc::clone(&write_lock));
    let mod_import_results = Arc::new(FakeModImportResultRepository {
        analysis: Some(analysis("mod-a", "package-a")),
    });
    let clock = Arc::new(FixedClock(CLOCK_MILLIS));
    let scan_cache = Arc::new(ExternalStateScanCache::new(
        Arc::clone(&clock) as Arc<dyn AppClock>
    ));
    let task_manager = Arc::new(TaskManager::new());
    let write_admission = Arc::new(FakeWriteAdmission::default());
    let audit_log = Arc::new(RecordingAuditLog::default());

    let scanner = Arc::new(ConfiguredExternalStateScanner::new(
        Arc::clone(&game_config) as Arc<dyn GameConfigRepository>,
        Arc::clone(&mod_import_results) as Arc<dyn ModImportResultRepository>,
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
        }),
        Arc::new(FakePackageReader {
            bytes_by_id: package_files
                .iter()
                .map(|(id, bytes)| ((*id).to_owned(), bytes.to_vec()))
                .collect(),
        }),
        Arc::new(SharedGameFsFactory(Arc::clone(&game_fs))),
        Arc::clone(&scan_cache),
        8 * 1024 * 1024,
        DEFAULT_WORKER_LIMIT,
    ));

    let adopter = Arc::new(ConfiguredExternalModAdopter::new(
        Arc::clone(&game_config) as Arc<dyn GameConfigRepository>,
        mod_import_results,
        Arc::clone(&manifest_repository) as Arc<dyn InstallManifestRepository>,
        write_locks,
        Arc::clone(&write_admission) as Arc<dyn InstallWriteAdmission>,
        Arc::new(SharedGameFsFactory(Arc::clone(&game_fs))),
        Arc::clone(&scan_cache),
        Arc::clone(&task_manager),
        Arc::clone(&audit_log) as Arc<dyn AuditLogWriter>,
        clock,
    ));

    Harness {
        _temp: temp,
        scanner,
        adopter,
        task_manager,
        game_fs,
        game_config,
        manifest_repository,
        write_admission,
        audit_log,
        scan_cache,
        write_lock,
    }
}

fn target(relative: &str) -> InstallTargetPath {
    let roots: Vec<String> = ALLOWED_ROOTS
        .iter()
        .map(|root| (*root).to_owned())
        .collect();
    InstallTargetPath::parse(relative, &roots).expect("合法目标路径")
}

fn manifest_entry(relative: &str, mod_id: &str) -> InstallManifestEntry {
    InstallManifestEntry {
        target_path: target(relative),
        mod_id: ModId::new(mod_id),
        revision_id: None,
        package_file_id: PackageFileId::new(relative),
        layer: FileLayer::new("base", 0),
        backup_ref: Some("backup-1".to_owned()),
        installed_file: None,
        adopted: false,
    }
}

impl Harness {
    fn scan(&self) -> hmm_core::ExternalInstallStateSummary {
        self.scanner
            .scan(ConfiguredExternalStateScanRequest {
                game_id: &GameId::mhw(),
                profile_id: &ProfileId::new("default"),
                mod_id: &ModId::new("mod-a"),
                cancellation_token: &NeverCancelled,
            })
            .expect("scan succeeds")
    }

    /// 像 runner 那样先把任务登记并置为 running，再直接调接管器——提交屏障要求任务存在且 running。
    fn running_task(&self) -> String {
        let task = self
            .task_manager
            .create_task(TaskKind::ExternalModAdopt)
            .expect("create task");
        self.task_manager
            .start_task(&task.task_id)
            .expect("start task");
        task.task_id
    }

    fn adopt(&self) -> Result<ExternalModAdoptOutcome, ConfiguredExternalModAdoptError> {
        self.adopt_with(&NeverCancelled, "mod-a", &FileLayer::new("base", 0))
    }

    fn adopt_with(
        &self,
        cancellation_token: &dyn CancellationToken,
        mod_id: &str,
        layer: &FileLayer,
    ) -> Result<ExternalModAdoptOutcome, ConfiguredExternalModAdoptError> {
        let task_id = self.running_task();
        self.adopter.adopt(ConfiguredExternalModAdoptRequest {
            task_id: &task_id,
            game_id: &GameId::mhw(),
            profile_id: &ProfileId::new("default"),
            mod_id: &ModId::new(mod_id),
            layer,
            cancellation_token,
        })
    }

    fn cached_record(&self) -> Option<ExternalStateScanRecord> {
        self.scan_cache.record(
            &GameId::mhw(),
            &ProfileId::new("default"),
            &ModId::new("mod-a"),
        )
    }

    fn task_service(&self) -> ExternalModAdoptTaskService {
        ExternalModAdoptTaskService::new(Arc::clone(&self.task_manager), Arc::clone(&self.adopter))
    }
}

fn start_launch(service: &ExternalModAdoptTaskService, mod_id: &str) -> ExternalModAdoptTaskLaunch {
    service
        .start_adopt(
            GameId::mhw(),
            ProfileId::new("default"),
            ModId::new(mod_id),
            FileLayer::new("base", 0),
        )
        .expect("start adopt")
}

/// 失败路径共用的「零副作用」断言：没保存清单、没碰游戏文件。
fn assert_no_side_effects(harness: &Harness) {
    assert!(
        harness.manifest_repository.saved().is_empty(),
        "失败不得写清单"
    );
    assert_eq!(
        harness.game_fs.write_attempts.load(Ordering::SeqCst),
        0,
        "接管不得写/删游戏文件"
    );
}

fn failure_audit_code(harness: &Harness) -> Option<String> {
    let events = harness.audit_log.events();
    assert_eq!(events.len(), 1, "失败应恰好记一条审计：{events:?}");
    assert_eq!(events[0].operation, "adopt_external_mod");
    assert_eq!(events[0].result, "failure");
    assert_eq!(
        events[0]
            .fields
            .get("claimed_file_count")
            .map(String::as_str),
        Some("0")
    );
    events[0].fields.get("error_code").cloned()
}

// ---------------------------------------------------------------------------
// 正向路径
// ---------------------------------------------------------------------------

#[test]
fn adopting_a_scanned_mod_writes_manifest_entries_and_nothing_else() {
    let files: &[(&str, &[u8])] = &[
        ("nativePC/a.mod3", b"same-a"),
        ("nativePC/b.mrl3", b"same-b"),
    ];
    let harness = harness(files, files);
    let reads_after_scan = {
        assert_eq!(harness.scan().state, ExternalInstallState::Installed);
        harness.game_fs.read_count.load(Ordering::SeqCst)
    };

    let outcome = harness.adopt().expect("adopt succeeds");

    assert_eq!(
        outcome,
        ExternalModAdoptOutcome {
            claimed_file_count: 2,
            skipped_claimed_count: 0,
            skipped_changed_count: 0,
            skipped_missing_count: 0,
            audit_degraded: false,
        }
    );

    // 唯一的落盘：一次清单保存。
    let saved = harness.manifest_repository.saved();
    assert_eq!(saved.len(), 1, "恰好一次原子保存");
    let manifest = &saved[0];
    assert_eq!(manifest.profile_id, ProfileId::new("default"));
    assert_eq!(manifest.status, InstallManifestStatus::Completed);
    assert_eq!(manifest.completed_at.as_deref(), Some("unix:1700000000"));
    assert_eq!(manifest.entries.len(), 2);
    for (entry, (relative, bytes)) in manifest.entries.iter().zip(files) {
        assert_eq!(entry.target_path, target(relative));
        assert_eq!(entry.mod_id, ModId::new("mod-a"));
        assert_eq!(entry.revision_id, None, "与 GUI 安装同口径：不带修订");
        assert_eq!(entry.package_file_id, PackageFileId::new(*relative));
        assert_eq!(entry.layer, FileLayer::new("base", 0));
        assert_eq!(
            entry.backup_ref, None,
            "文件不是本工具写的，没有可还原的原版"
        );
        assert_eq!(
            entry.installed_file.as_ref(),
            Some(&installed_file_summary(bytes)),
            "卸载前的哈希核对靠它"
        );
        assert!(entry.adopted, "来源标记");
    }

    // 不碰游戏文件：没有写/删，也没有再读（哈希已被扫描记录背书，锁内只 stat）。
    assert_eq!(harness.game_fs.write_attempts.load(Ordering::SeqCst), 0);
    assert_eq!(
        harness.game_fs.read_count.load(Ordering::SeqCst),
        reads_after_scan,
        "接管不得在写锁内重新读文件"
    );

    // 清单已认领它，外部状态记录随之作废。
    assert!(harness.cached_record().is_none(), "成功后缓存记录必须丢弃");
    assert_eq!(
        harness
            .scanner
            .query(
                &GameId::mhw(),
                &ProfileId::new("default"),
                &ModId::new("mod-a")
            )
            .summary,
        None
    );
}

#[test]
fn adopting_preserves_existing_entries_and_manifest_metadata() {
    let files: &[(&str, &[u8])] = &[("nativePC/a.mod3", b"same-a")];
    let harness = harness(files, files);
    let existing = manifest_entry("nativePC/other.mod3", "mod-other");
    harness
        .manifest_repository
        .set_manifest(InstallManifest::completed_with_metadata(
            ProfileId::new("default"),
            vec![existing.clone()],
            Some("hmm-app".to_owned()),
            Some("unix:1".to_owned()),
            Some("unix:2".to_owned()),
            Some("plan-hash-1".to_owned()),
        ));
    harness.scan();

    harness.adopt().expect("adopt succeeds");

    let saved = harness.manifest_repository.saved();
    let manifest = &saved[0];
    assert_eq!(manifest.entries.len(), 2);
    assert_eq!(
        manifest.entries[0], existing,
        "别人的条目原样保留、顺序不变"
    );
    assert_eq!(manifest.entries[1].mod_id, ModId::new("mod-a"));
    assert_eq!(manifest.backend.as_deref(), Some("hmm-app"));
    assert_eq!(manifest.created_at.as_deref(), Some("unix:1"));
    assert_eq!(manifest.plan_hash.as_deref(), Some("plan-hash-1"));
    assert_eq!(
        manifest.completed_at.as_deref(),
        Some("unix:1700000000"),
        "只有 completed_at 前进"
    );
}

#[test]
fn the_layer_of_adopted_entries_comes_from_the_request() {
    let files: &[(&str, &[u8])] = &[("nativePC/a.mod3", b"same-a")];
    let harness = harness(files, files);
    harness.scan();

    harness
        .adopt_with(&NeverCancelled, "mod-a", &FileLayer::new("custom", 7))
        .expect("adopt succeeds");

    let saved = harness.manifest_repository.saved();
    assert_eq!(saved[0].entries[0].layer, FileLayer::new("custom", 7));
}

#[test]
fn skipped_files_are_counted_but_never_written() {
    let package: &[(&str, &[u8])] = &[
        ("nativePC/a.mod3", b"same-a"),
        ("nativePC/b.mod3", b"same-b"),
        ("nativePC/c.mod3", b"same-c"),
        ("nativePC/d.mod3", b"same-d"),
    ];
    // a 一致；b 内容不同；c 缺失；d 一致但已归 mod-other。
    let game: &[(&str, &[u8])] = &[
        ("nativePC/a.mod3", b"same-a"),
        ("nativePC/b.mod3", b"changed"),
        ("nativePC/d.mod3", b"same-d"),
    ];
    let harness = harness(package, game);
    let existing = manifest_entry("nativePC/d.mod3", "mod-other");
    harness
        .manifest_repository
        .set_manifest(InstallManifest::completed(
            ProfileId::new("default"),
            vec![existing.clone()],
        ));
    harness.scan();

    let outcome = harness.adopt().expect("adopt succeeds");

    assert_eq!(
        outcome,
        ExternalModAdoptOutcome {
            claimed_file_count: 1,
            skipped_claimed_count: 1,
            skipped_changed_count: 1,
            skipped_missing_count: 1,
            audit_degraded: false,
        }
    );
    let saved = harness.manifest_repository.saved();
    assert_eq!(
        saved[0]
            .entries
            .iter()
            .map(|entry| (entry.target_path.as_str().to_owned(), entry.mod_id.clone()))
            .collect::<Vec<_>>(),
        [
            ("nativePC/d.mod3".to_owned(), ModId::new("mod-other")),
            ("nativePC/a.mod3".to_owned(), ModId::new("mod-a")),
        ],
        "只认领 a；d 仍归 mod-other，b/c 不进清单"
    );
}

#[test]
fn a_successful_adopt_is_audited_with_counts_and_no_paths() {
    let files: &[(&str, &[u8])] = &[
        ("nativePC/a.mod3", b"same-a"),
        ("nativePC/b.mrl3", b"same-b"),
    ];
    let harness = harness(files, files);
    harness.scan();

    harness.adopt().expect("adopt succeeds");

    let events = harness.audit_log.events();
    assert_eq!(events.len(), 1, "成功恰好记一条：{events:?}");
    let event = &events[0];
    assert_eq!(event.timestamp_unix_millis, CLOCK_MILLIS);
    assert_eq!(event.category, "install");
    assert_eq!(event.operation, "adopt_external_mod");
    assert_eq!(event.result, "success");
    let field = |name: &str| event.fields.get(name).map(String::as_str);
    assert!(field("task_id").is_some_and(|id| id.starts_with("external-mod-adopt-")));
    assert_eq!(field("game_id"), Some("mhw"));
    assert_eq!(field("profile_id"), Some("default"));
    assert_eq!(field("mod_id"), Some("mod-a"));
    assert_eq!(field("claimed_file_count"), Some("2"));
    assert_eq!(field("skipped_claimed_count"), Some("0"));
    assert_eq!(field("skipped_changed_count"), Some("0"));
    assert_eq!(field("skipped_missing_count"), Some("0"));
    assert_eq!(field("error_code"), None);
    // 审计写入器会拒绝含路径分隔符的值——这里根本不能出现路径。
    for (name, value) in &event.fields {
        assert!(
            !value.contains('/') && !value.contains('\\') && !value.contains("nativePC"),
            "审计字段 {name} 不得含路径：{value}"
        );
    }
}

#[test]
fn an_audit_write_failure_after_commit_is_reported_as_degraded_not_as_failure() {
    let files: &[(&str, &[u8])] = &[("nativePC/a.mod3", b"same-a")];
    let harness = harness(files, files);
    harness.scan();
    harness.audit_log.fail_writes();

    let outcome = harness
        .adopt()
        .expect("清单已写成，审计失败不能改写成功事实");

    assert!(outcome.audit_degraded, "必须显式报降级");
    assert_eq!(outcome.claimed_file_count, 1);
    assert_eq!(harness.manifest_repository.saved().len(), 1);
    assert!(harness.cached_record().is_none());
}

// ---------------------------------------------------------------------------
// 锁边界
// ---------------------------------------------------------------------------

#[test]
fn fingerprint_recheck_and_manifest_save_happen_inside_the_write_lock() {
    let files: &[(&str, &[u8])] = &[("nativePC/a.mod3", b"same-a")];
    let harness = harness(files, files);
    harness.scan();
    let stats_after_scan = harness.game_fs.stats_while_locked.load(Ordering::SeqCst);

    harness.adopt().expect("adopt succeeds");

    assert!(
        harness.game_fs.stats_while_locked.load(Ordering::SeqCst) > stats_after_scan,
        "复核 stat 必须在写锁内，否则复核与写入之间有窗口"
    );
    assert_eq!(
        harness
            .manifest_repository
            .saves_while_locked
            .load(Ordering::SeqCst),
        1,
        "清单保存必须在写锁内"
    );
    assert!(
        harness.write_lock.try_lock().is_ok(),
        "接管返回后写锁必须已释放"
    );
}

#[test]
fn a_write_in_progress_makes_adopt_wait_instead_of_failing() {
    let files: &[(&str, &[u8])] = &[("nativePC/a.mod3", b"same-a")];
    let harness = harness(files, files);
    harness.scan();

    // 模拟安装进行中：另一个线程持有同一把 (game, profile) 写锁一段时间再放开。
    // 用信道等它**确实拿到锁**再开始接管，不靠 sleep 赌调度顺序。
    let lock = Arc::clone(&harness.write_lock);
    let released_at = Arc::new(Mutex::new(None::<Instant>));
    let released_at_writer = Arc::clone(&released_at);
    let (held_tx, held_rx) = std::sync::mpsc::channel();
    let guard_holder = std::thread::spawn(move || {
        let guard = lock.lock().expect("hold write lock");
        held_tx.send(()).expect("signal lock held");
        std::thread::sleep(Duration::from_millis(200));
        *released_at_writer.lock().expect("released lock") = Some(Instant::now());
        drop(guard);
    });
    held_rx.recv().expect("holder acquired the lock");

    let outcome = harness.adopt();
    guard_holder.join().expect("holder thread");

    assert!(outcome.is_ok(), "写操作等锁而不是报 stale：{outcome:?}");
    let released_at = released_at
        .lock()
        .expect("released lock")
        .expect("released");
    let saved_at = harness
        .manifest_repository
        .last_save_at
        .lock()
        .expect("save time lock")
        .expect("saved");
    assert!(
        saved_at >= released_at,
        "清单保存必须发生在另一方释放写锁之后"
    );
}

// ---------------------------------------------------------------------------
// 前置拒绝（锁外、零副作用）
// ---------------------------------------------------------------------------

#[test]
fn adopt_requires_a_prior_successful_scan() {
    let files: &[(&str, &[u8])] = &[("nativePC/a.mod3", b"same-a")];
    let harness = harness(files, files);

    let error = harness.adopt().expect_err("没有扫描记录不能接管");

    assert_eq!(error, ConfiguredExternalModAdoptError::ScanRequired);
    assert_no_side_effects(&harness);
    assert_eq!(
        harness.write_admission.calls.load(Ordering::SeqCst),
        0,
        "前置拒绝不该走到锁内准入"
    );
    assert_eq!(
        failure_audit_code(&harness).as_deref(),
        Some("external_mod_adopt_scan_required")
    );
}

#[test]
fn unreadable_files_block_adoption_entirely() {
    let files: &[(&str, &[u8])] = &[
        ("nativePC/a.mod3", b"same-a"),
        ("nativePC/b.mrl3", b"same-b"),
    ];
    let harness = harness(files, files);
    // b 被游戏独占打开：stat 正常、读取失败 → 扫描记为 Unreadable。
    harness.game_fs.make_unreadable("nativePC/b.mrl3");
    let summary = harness.scan();
    assert_eq!(
        summary.files,
        [ExternalFileState::Matched, ExternalFileState::Unreadable]
    );

    let error = harness.adopt().expect_err("残缺事实上不建清单");

    assert_eq!(error, ConfiguredExternalModAdoptError::UnreadableFiles);
    assert_no_side_effects(&harness);
    assert_eq!(harness.write_admission.calls.load(Ordering::SeqCst), 0);
    assert!(
        harness.cached_record().is_some(),
        "记录保留，用户可在解锁后重扫"
    );
}

#[test]
fn nothing_to_adopt_when_no_file_is_both_matched_and_unclaimed() {
    let package: &[(&str, &[u8])] = &[
        ("nativePC/a.mod3", b"same-a"),
        ("nativePC/b.mod3", b"same-b"),
    ];
    // a 内容不同；b 一致但已归别人。
    let game: &[(&str, &[u8])] = &[
        ("nativePC/a.mod3", b"changed"),
        ("nativePC/b.mod3", b"same-b"),
    ];
    let harness = harness(package, game);
    harness
        .manifest_repository
        .set_manifest(InstallManifest::completed(
            ProfileId::new("default"),
            vec![manifest_entry("nativePC/b.mod3", "mod-other")],
        ));
    harness.scan();

    let error = harness.adopt().expect_err("没有可认领的文件");

    assert_eq!(error, ConfiguredExternalModAdoptError::NothingToAdopt);
    assert_no_side_effects(&harness);
    assert_eq!(harness.write_admission.calls.load(Ordering::SeqCst), 0);
    assert_eq!(
        failure_audit_code(&harness).as_deref(),
        Some("external_mod_adopt_nothing_to_adopt")
    );
}

#[test]
fn an_inconsistent_cached_record_is_refused_as_unavailable() {
    let files: &[(&str, &[u8])] = &[("nativePC/a.mod3", b"same-a")];
    let harness = harness(files, files);
    harness.scan();
    // 人为制造一份列长不齐的记录（编程错误的形态）。
    let mut record = harness.cached_record().expect("record");
    record.game_files.clear();
    harness.scan_cache.record_success(
        &GameId::mhw(),
        &ProfileId::new("default"),
        &ModId::new("mod-a"),
        record,
    );

    let error = harness.adopt().expect_err("不变量被破坏不能继续");

    assert_eq!(error, ConfiguredExternalModAdoptError::Unavailable);
    assert_no_side_effects(&harness);
    assert_eq!(
        harness.write_admission.calls.load(Ordering::SeqCst),
        0,
        "列长不齐在锁外就该发现，不该走到准入"
    );
}

#[test]
fn an_unknown_mod_is_refused_before_anything_else() {
    let files: &[(&str, &[u8])] = &[("nativePC/a.mod3", b"same-a")];
    let harness = harness(files, files);
    harness.scan();

    let error = harness
        .adopt_with(&NeverCancelled, "mod-b", &FileLayer::new("base", 0))
        .expect_err("未知 MOD");

    assert_eq!(error, ConfiguredExternalModAdoptError::ModUnavailable);
    assert_no_side_effects(&harness);
}

#[test]
fn a_cancellation_before_the_lock_is_reported_as_cancelled_and_not_audited() {
    let files: &[(&str, &[u8])] = &[("nativePC/a.mod3", b"same-a")];
    let harness = harness(files, files);
    harness.scan();

    let error = harness
        .adopt_with(&Cancelled, "mod-a", &FileLayer::new("base", 0))
        .expect_err("已取消");

    assert_eq!(error, ConfiguredExternalModAdoptError::Cancelled);
    assert_no_side_effects(&harness);
    assert!(
        harness.audit_log.events().is_empty(),
        "取消不是失败，不进审计"
    );
}

#[test]
fn a_cancellation_that_lands_after_the_lock_is_still_honoured_before_writing() {
    let files: &[(&str, &[u8])] = &[("nativePC/a.mod3", b"same-a")];
    let harness = harness(files, files);
    harness.scan();
    // 第一次询问（锁外）未取消，第二次（锁内）已取消。
    let token = CancelAfter {
        remaining: AtomicUsize::new(1),
    };

    let error = harness
        .adopt_with(&token, "mod-a", &FileLayer::new("base", 0))
        .expect_err("锁内取消检查");

    assert_eq!(error, ConfiguredExternalModAdoptError::Cancelled);
    assert_no_side_effects(&harness);
    assert_eq!(
        harness.write_admission.calls.load(Ordering::SeqCst),
        0,
        "取消检查在写入准入之前"
    );
}

// ---------------------------------------------------------------------------
// 锁内重验（guard ≠ 授权）
// ---------------------------------------------------------------------------

#[test]
fn a_file_changed_after_the_scan_makes_adopt_stale() {
    let files: &[(&str, &[u8])] = &[
        ("nativePC/a.mod3", b"same-a"),
        ("nativePC/b.mrl3", b"same-b"),
    ];
    let harness = harness(files, files);
    harness.scan();
    harness
        .game_fs
        .set_file("nativePC/b.mrl3", b"same-b-but-longer");

    let error = harness.adopt().expect_err("指纹漂移");

    assert_eq!(error, ConfiguredExternalModAdoptError::Stale);
    assert_no_side_effects(&harness);
    assert!(
        harness.cached_record().is_some(),
        "记录保留：getter 会把它报成 stale"
    );
    assert_eq!(
        failure_audit_code(&harness).as_deref(),
        Some("external_mod_adopt_stale")
    );
}

#[test]
fn a_file_that_disappeared_after_the_scan_makes_adopt_stale() {
    let files: &[(&str, &[u8])] = &[
        ("nativePC/a.mod3", b"same-a"),
        ("nativePC/b.mrl3", b"same-b"),
    ];
    let harness = harness(files, files);
    harness.scan();
    harness.game_fs.remove_file("nativePC/b.mrl3");

    let error = harness.adopt().expect_err("文件消失");

    assert_eq!(error, ConfiguredExternalModAdoptError::Stale);
    assert_no_side_effects(&harness);
}

#[test]
fn a_file_that_appeared_after_the_scan_makes_adopt_stale() {
    let package: &[(&str, &[u8])] = &[
        ("nativePC/a.mod3", b"same-a"),
        ("nativePC/b.mrl3", b"same-b"),
    ];
    let game: &[(&str, &[u8])] = &[("nativePC/a.mod3", b"same-a")];
    let harness = harness(package, game);
    harness.scan();
    harness.game_fs.set_file("nativePC/b.mrl3", b"same-b");

    let error = harness.adopt().expect_err("文件出现");

    assert_eq!(error, ConfiguredExternalModAdoptError::Stale);
    assert_no_side_effects(&harness);
}

#[test]
fn a_manifest_claim_added_after_the_scan_makes_adopt_stale() {
    let files: &[(&str, &[u8])] = &[
        ("nativePC/a.mod3", b"same-a"),
        ("nativePC/b.mrl3", b"same-b"),
    ];
    let harness = harness(files, files);
    harness.scan();
    // 磁盘没变、指纹复核会过；但清单里 b 被别的 MOD 认领了——认领集从 {a,b} 变成 {a}。
    harness
        .manifest_repository
        .set_manifest(InstallManifest::completed(
            ProfileId::new("default"),
            vec![manifest_entry("nativePC/b.mrl3", "mod-other")],
        ));

    let error = harness.adopt().expect_err("清单归属漂移");

    assert_eq!(
        error,
        ConfiguredExternalModAdoptError::Stale,
        "写出的必须等于用户确认的那份，不能悄悄少写一条"
    );
    assert_no_side_effects(&harness);
}

#[test]
fn a_mod_installed_after_the_scan_is_refused_as_already_installed() {
    let files: &[(&str, &[u8])] = &[("nativePC/a.mod3", b"same-a")];
    let harness = harness(files, files);
    harness.scan();
    harness
        .manifest_repository
        .set_manifest(InstallManifest::completed(
            ProfileId::new("default"),
            vec![manifest_entry("nativePC/a.mod3", "mod-a")],
        ));

    let error = harness.adopt().expect_err("已安装应走重装");

    assert_eq!(error, ConfiguredExternalModAdoptError::AlreadyInstalled);
    assert_no_side_effects(&harness);
}

#[test]
fn an_untrusted_manifest_refuses_adoption() {
    let files: &[(&str, &[u8])] = &[("nativePC/a.mod3", b"same-a")];
    let harness = harness(files, files);
    harness.scan();
    let mut manifest = InstallManifest::completed(
        ProfileId::new("default"),
        vec![manifest_entry("nativePC/other.mod3", "mod-other")],
    );
    manifest.status = InstallManifestStatus::Committing;
    harness.manifest_repository.set_manifest(manifest);

    let error = harness.adopt().expect_err("进行中的清单 entries 不可信");

    assert_eq!(error, ConfiguredExternalModAdoptError::ManifestNotTrusted);
    assert_no_side_effects(&harness);
}

#[test]
fn a_manifest_read_failure_inside_the_lock_fails_closed() {
    let files: &[(&str, &[u8])] = &[("nativePC/a.mod3", b"same-a")];
    let harness = harness(files, files);
    harness.scan();
    harness.manifest_repository.fail_loads();

    let error = harness.adopt().expect_err("清单读不到不能当成不存在");

    assert_eq!(error, ConfiguredExternalModAdoptError::ManifestUnavailable);
    assert_no_side_effects(&harness);
}

#[test]
fn a_game_instance_removed_after_the_scan_is_refused() {
    let files: &[(&str, &[u8])] = &[("nativePC/a.mod3", b"same-a")];
    let harness = harness(files, files);
    harness.scan();
    harness.game_config.clear_instance();

    let error = harness.adopt().expect_err("游戏目录没了");

    assert_eq!(
        error,
        ConfiguredExternalModAdoptError::GameInstanceUnavailable
    );
    assert_no_side_effects(&harness);
}

// ---------------------------------------------------------------------------
// 准入与写失败
// ---------------------------------------------------------------------------

#[test]
fn write_admission_rejections_surface_with_the_install_error_codes() {
    let files: &[(&str, &[u8])] = &[("nativePC/a.mod3", b"same-a")];
    let harness = harness(files, files);
    harness.scan();
    harness
        .write_admission
        .reject_with(InstallWriteAdmissionError::RecoveryPending);

    let error = harness.adopt().expect_err("准入拒绝");

    assert_eq!(
        error,
        ConfiguredExternalModAdoptError::WriteNotAllowed(
            InstallWriteAdmissionError::RecoveryPending
        )
    );
    assert_eq!(
        error.code(),
        "recovery_pending",
        "与 install/uninstall 同一组码"
    );
    assert_no_side_effects(&harness);
    assert_eq!(
        failure_audit_code(&harness).as_deref(),
        Some("recovery_pending")
    );
}

#[test]
fn a_manifest_write_failure_leaves_no_trace_and_keeps_the_record_for_retry() {
    let files: &[(&str, &[u8])] = &[("nativePC/a.mod3", b"same-a")];
    let harness = harness(files, files);
    harness.scan();
    harness.manifest_repository.fail_saves();

    let error = harness.adopt().expect_err("原子写失败");

    assert_eq!(error, ConfiguredExternalModAdoptError::ManifestWriteFailed);
    assert_no_side_effects(&harness);
    assert!(
        harness.cached_record().is_some(),
        "清单未变，记录不能丢——用户可以直接重试"
    );
    assert_eq!(
        failure_audit_code(&harness).as_deref(),
        Some("external_mod_adopt_manifest_write_failed")
    );
}

#[test]
fn error_codes_are_stable_and_distinct() {
    let own_codes = [
        (
            ConfiguredExternalModAdoptError::GameInstanceUnavailable,
            "external_mod_adopt_game_instance_unavailable",
        ),
        (
            ConfiguredExternalModAdoptError::ModUnavailable,
            "external_mod_adopt_mod_unavailable",
        ),
        (
            ConfiguredExternalModAdoptError::ScanRequired,
            "external_mod_adopt_scan_required",
        ),
        (
            ConfiguredExternalModAdoptError::UnreadableFiles,
            "external_mod_adopt_unreadable_files",
        ),
        (
            ConfiguredExternalModAdoptError::NothingToAdopt,
            "external_mod_adopt_nothing_to_adopt",
        ),
        (
            ConfiguredExternalModAdoptError::AlreadyInstalled,
            "external_mod_adopt_already_installed",
        ),
        (
            ConfiguredExternalModAdoptError::ManifestNotTrusted,
            "external_mod_adopt_manifest_not_trusted",
        ),
        (
            ConfiguredExternalModAdoptError::ManifestUnavailable,
            "external_mod_adopt_manifest_unavailable",
        ),
        (
            ConfiguredExternalModAdoptError::ManifestWriteFailed,
            "external_mod_adopt_manifest_write_failed",
        ),
        (
            ConfiguredExternalModAdoptError::GameFileUnavailable,
            "external_mod_adopt_game_file_unavailable",
        ),
        (
            ConfiguredExternalModAdoptError::Stale,
            "external_mod_adopt_stale",
        ),
        (
            ConfiguredExternalModAdoptError::Cancelled,
            "external_mod_adopt_cancelled",
        ),
        (
            ConfiguredExternalModAdoptError::Unavailable,
            "external_mod_adopt_unavailable",
        ),
    ];
    let mut seen = HashSet::new();
    for (error, expected) in own_codes {
        assert_eq!(error.code(), expected);
        assert!(seen.insert(expected), "码重复：{expected}");
    }
    // 跨切面的码沿用既有口径，不另起一套。
    assert_eq!(
        ConfiguredExternalModAdoptError::WriteAdmission(CrossProcessWriteAdmissionError::Busy)
            .code(),
        CrossProcessWriteAdmissionError::Busy.code()
    );
    assert_eq!(
        ConfiguredExternalModAdoptError::WriteNotAllowed(
            InstallWriteAdmissionError::SafetyRejected
        )
        .code(),
        "write_safety_rejected"
    );
    assert_eq!(
        ConfiguredExternalModAdoptError::WriteNotAllowed(
            InstallWriteAdmissionError::RecoveryUnavailable
        )
        .code(),
        "recovery_unavailable"
    );
}

// ---------------------------------------------------------------------------
// 任务服务
// ---------------------------------------------------------------------------

#[test]
fn a_successful_adopt_task_completes_with_ordered_phases_and_no_paths_in_events() {
    let files: &[(&str, &[u8])] = &[("nativePC/a.mod3", b"same-a")];
    let harness = harness(files, files);
    harness.scan();
    let service = harness.task_service();

    let launch = start_launch(&service, "mod-a");
    assert!(launch.task.task_id.starts_with("external-mod-adopt-"));
    assert_eq!(launch.task.kind, TaskKind::ExternalModAdopt);
    let queued = queued_adopt_event(&launch);
    assert_eq!(queued.phase, EXTERNAL_MOD_ADOPT_QUEUED_PHASE);
    assert_eq!(queued.status, TaskStatus::Queued);
    assert_eq!(queued.result_ref.as_deref(), Some("mod-a"));

    let events = service.run_adopt(launch.clone()).expect("run adopt");

    assert_eq!(
        harness.task_manager.task_status(&launch.task.task_id),
        Some(TaskStatus::Completed)
    );
    assert_eq!(
        events
            .iter()
            .map(|event| (event.phase.as_str(), event.status))
            .collect::<Vec<_>>(),
        [
            (EXTERNAL_MOD_ADOPT_PROCESSING_PHASE, TaskStatus::Running),
            (EXTERNAL_MOD_ADOPT_COMPLETED_PHASE, TaskStatus::Completed),
        ]
    );
    for event in &events {
        assert_eq!(event.kind, TaskKind::ExternalModAdopt);
        assert_eq!(event.result_ref.as_deref(), Some("mod-a"));
        assert_eq!(event.error, None);
        assert!(
            !format!("{event:?}").contains("nativePC"),
            "进度事件不得携带目标路径：{event:?}"
        );
    }
    assert_eq!(harness.manifest_repository.saved().len(), 1);
}

#[test]
fn a_failing_adopt_task_carries_the_stable_error_code() {
    let files: &[(&str, &[u8])] = &[("nativePC/a.mod3", b"same-a")];
    let harness = harness(files, files);
    let service = harness.task_service();

    // 没扫过就接管：ScanRequired。
    let launch = start_launch(&service, "mod-a");
    let events = service.run_adopt(launch.clone()).expect("run adopt");

    assert_eq!(
        harness.task_manager.task_status(&launch.task.task_id),
        Some(TaskStatus::Failed)
    );
    assert_eq!(
        events
            .iter()
            .map(|event| event.phase.as_str())
            .collect::<Vec<_>>(),
        [
            EXTERNAL_MOD_ADOPT_PROCESSING_PHASE,
            EXTERNAL_MOD_ADOPT_FAILED_PHASE
        ]
    );
    assert_eq!(
        events[1].error.as_deref(),
        Some("external_mod_adopt_scan_required")
    );
    assert_eq!(events[1].status, TaskStatus::Failed);
    assert_no_side_effects(&harness);
}

#[test]
fn a_cancellation_before_the_runner_starts_yields_only_a_cancelled_event() {
    let files: &[(&str, &[u8])] = &[("nativePC/a.mod3", b"same-a")];
    let harness = harness(files, files);
    harness.scan();
    let service = harness.task_service();

    let launch = start_launch(&service, "mod-a");
    harness
        .task_manager
        .cancel_task(&launch.task.task_id)
        .expect("cancel task");

    let events = service.run_adopt(launch.clone()).expect("run adopt");

    assert_eq!(events.len(), 1, "取消后不该再进入接管阶段");
    assert_eq!(events[0].phase, EXTERNAL_MOD_ADOPT_CANCELLED_PHASE);
    assert_eq!(events[0].status, TaskStatus::Cancelled);
    assert_eq!(events[0].error, None, "取消不是失败，不得贴错误码");
    assert_no_side_effects(&harness);
    assert!(harness.audit_log.events().is_empty());
}

#[test]
fn an_audit_degraded_completion_carries_the_explicit_degradation_code() {
    let files: &[(&str, &[u8])] = &[("nativePC/a.mod3", b"same-a")];
    let harness = harness(files, files);
    harness.scan();
    harness.audit_log.fail_writes();
    let service = harness.task_service();

    let launch = start_launch(&service, "mod-a");
    let events = service.run_adopt(launch.clone()).expect("run adopt");

    assert_eq!(
        harness.task_manager.task_status(&launch.task.task_id),
        Some(TaskStatus::Completed),
        "清单已写成就是成功"
    );
    let completed = events.last().expect("terminal event");
    assert_eq!(completed.phase, EXTERNAL_MOD_ADOPT_COMPLETED_PHASE);
    assert_eq!(completed.status, TaskStatus::Completed);
    assert_eq!(
        completed.error.as_deref(),
        Some(EXTERNAL_MOD_ADOPT_AUDIT_UNAVAILABLE_CODE),
        "审计缺失必须显式告知，而不是静默"
    );
}

#[test]
fn an_unemitted_queued_launch_can_be_aborted_to_a_terminal_state() {
    let files: &[(&str, &[u8])] = &[("nativePC/a.mod3", b"same-a")];
    let harness = harness(files, files);
    let service = harness.task_service();

    let launch = start_launch(&service, "mod-a");
    service.abort_queued_adopt(&launch).expect("abort");

    assert_eq!(
        harness.task_manager.task_status(&launch.task.task_id),
        Some(TaskStatus::Failed),
        "废弃的 launch 不能停留在 queued"
    );
    service.abort_queued_adopt(&launch).expect("abort again");
}
