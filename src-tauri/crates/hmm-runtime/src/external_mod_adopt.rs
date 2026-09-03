//! 外部 MOD 接管（adopt）的 runtime 装配（#286 最后一片，唯一有写入的一片）。
//!
//! 接管 = **只写安装清单，不碰任何文件**。判定归 `hmm-app::external_adopt`
//! （纯函数），这里负责把它放进写事务的正确位置：
//!
//! ```text
//! 锁外   前置拒绝（零副作用）：取缓存扫描记录 → 无记录 / 含 unreadable / 预览认领集为空 → 拒绝
//! 锁内   跨进程准入（任务感知）→ 进程内写锁（阻塞 lock，写操作不用 try_lock）
//!        → 写入准入（沙箱根 + recovery pending，与 install/uninstall 同一条链）
//!        → 重验（guard ≠ 授权）：stat 指纹 vs 记录 → 当下清单 → 重算认领集 → 与记录推导的预览比对
//!        → 提交屏障（block_task_cancellation）→ 追加条目 → 原子 save_manifest → 审计
//! ```
//!
//! ## 为什么依赖缓存的扫描记录而不是重新哈希
//!
//! 用户在弹窗里确认的是**那一份**扫描结果（认领 N 个、跳过 M 个）。接管必须写出**同一份**，
//! 否则「确认」就名不副实。锁内用 stage-3 同款指纹复核证明磁盘没变、用当下清单重算证明
//! 归属没变，两者任一漂移都拒绝并要求重扫——写出来的永远等于用户看到的。
//! 这也是拍板的口径：「adopt 不得信任缓存的归因副本，执行时锁内重算」——重算的是**归属**
//! （清单事实），复核的是**指纹**（磁盘事实）；哈希本身已被指纹复核背书，不必再做长活。
//!
//! ## 失败即无副作用
//!
//! 唯一的落盘是一次原子 `save_manifest`（tmp + rename + fsync）。它之前的任何失败都没有
//! 中间态，因此不需要 recovery 记录；它失败 = 清单未变。审计写入失败按 install 同款
//! 处理：成功事实不改写，事件带上显式降级码。

use std::collections::BTreeMap;
use std::sync::Arc;

use hmm_app::external_adopt::{
    append_adopted_entries, derive_external_adopt_plan, ExternalAdoptPlan, ExternalAdoptPlanError,
};
use hmm_app::{
    GameProfileWriteLockRegistry, InstallWriteAdmission, InstallWriteAdmissionError, TaskManager,
};
use hmm_core::{ExternalFileState, FileLayer, GameId, InstallTargetPath, ModId, ProfileId};
use hmm_ports::{
    AppClock, AuditLogEvent, AuditLogWriter, AuditWriteFailurePolicy, CancellationToken,
    CrossProcessWriteAdmissionError, GameConfigRepository, InstallGameFileInspector,
    InstallManifestRepository, ModImportResultRepository,
};

use crate::external_state_scan::{
    ExternalStateScanCache, ExternalStateScanRecord, GameFileSystemFactory,
};

/// 接管失败的原因。稳定码经 command 到达前端；不含路径。
///
/// 不是 `Copy`：`InstallWriteAdmissionError` 本身不是。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfiguredExternalModAdoptError {
    /// 游戏目录未配置或读取失败。
    GameInstanceUnavailable,
    /// 该 `mod_id` 的导入记录不存在（扫描之后 MOD 被删了）。
    ModUnavailable,
    /// 本会话没有这个 MOD 的成功扫描记录：先「检查游戏目录」再接管。
    ScanRequired,
    /// 记录里有读不到的文件（规则 3）：残缺事实上不建清单。
    UnreadableFiles,
    /// 没有任何「一致且无主」的文件可认领（规则 4）。
    NothingToAdopt,
    /// 该 MOD 在清单里已有条目，应走重装而不是接管。
    AlreadyInstalled,
    /// 配置档清单处于进行中/失败态，entries 不可信。
    ManifestNotTrusted,
    /// 清单读取失败。
    ManifestUnavailable,
    /// 清单写入失败（原子写未完成，清单未变）。
    ManifestWriteFailed,
    /// 游戏目录侧 stat 失败，无法复核指纹。
    GameFileUnavailable,
    /// 扫描结果到点击接管之间事实变了（指纹漂移、或清单归属变了）：拒绝并要求重扫。
    Stale,
    /// 跨进程写入准入未取得（`write_admission_*` 稳定码沿用跨切面口径）。
    WriteAdmission(CrossProcessWriteAdmissionError),
    /// 写入准入拒绝（沙箱根 / recovery pending），码沿用 install 同一组。
    WriteNotAllowed(InstallWriteAdmissionError),
    /// 用户取消。
    Cancelled,
    /// 编排自身不可用（锁被毒化、任务状态机拒绝、不变量被破坏）。
    Unavailable,
}

impl ConfiguredExternalModAdoptError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::GameInstanceUnavailable => "external_mod_adopt_game_instance_unavailable",
            Self::ModUnavailable => "external_mod_adopt_mod_unavailable",
            Self::ScanRequired => "external_mod_adopt_scan_required",
            Self::UnreadableFiles => "external_mod_adopt_unreadable_files",
            Self::NothingToAdopt => "external_mod_adopt_nothing_to_adopt",
            Self::AlreadyInstalled => "external_mod_adopt_already_installed",
            Self::ManifestNotTrusted => "external_mod_adopt_manifest_not_trusted",
            Self::ManifestUnavailable => "external_mod_adopt_manifest_unavailable",
            Self::ManifestWriteFailed => "external_mod_adopt_manifest_write_failed",
            Self::GameFileUnavailable => "external_mod_adopt_game_file_unavailable",
            Self::Stale => "external_mod_adopt_stale",
            Self::WriteAdmission(error) => (*error).code(),
            Self::WriteNotAllowed(error) => error.failure_phase(),
            Self::Cancelled => "external_mod_adopt_cancelled",
            Self::Unavailable => "external_mod_adopt_unavailable",
        }
    }
}

/// 接管成功的产物：只有计数，没有路径（进事件、进审计都安全）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExternalModAdoptOutcome {
    pub claimed_file_count: usize,
    pub skipped_claimed_count: usize,
    pub skipped_changed_count: usize,
    pub skipped_missing_count: usize,
    /// 清单已写成，但审计记录没写进去（显式降级，不改写成功事实）。
    pub audit_degraded: bool,
}

pub struct ConfiguredExternalModAdoptRequest<'a> {
    pub task_id: &'a str,
    pub game_id: &'a GameId,
    pub profile_id: &'a ProfileId,
    pub mod_id: &'a ModId,
    /// 与 `start_install_task` 同形：条目的 layer 由请求给出（前端与安装一样传 base/0）。
    pub layer: &'a FileLayer,
    pub cancellation_token: &'a dyn CancellationToken,
}

/// 外部 MOD 接管器。构造只收长生命周期依赖；游戏目录访问对象按当次加载的 `root_dir` 构造。
pub struct ConfiguredExternalModAdopter {
    game_config_repository: Arc<dyn GameConfigRepository>,
    mod_import_result_repository: Arc<dyn ModImportResultRepository>,
    /// 必须是带投影追踪的那份装饰仓储，否则库列表不会刷新成「已安装」。
    install_manifest_repository: Arc<dyn InstallManifestRepository>,
    write_locks: Arc<GameProfileWriteLockRegistry>,
    write_admission: Arc<dyn InstallWriteAdmission>,
    game_fs_factory: Arc<dyn GameFileSystemFactory>,
    scan_cache: Arc<ExternalStateScanCache>,
    task_manager: Arc<TaskManager>,
    audit_log: Arc<dyn AuditLogWriter>,
    clock: Arc<dyn AppClock>,
}

impl ConfiguredExternalModAdopter {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        game_config_repository: Arc<dyn GameConfigRepository>,
        mod_import_result_repository: Arc<dyn ModImportResultRepository>,
        install_manifest_repository: Arc<dyn InstallManifestRepository>,
        write_locks: Arc<GameProfileWriteLockRegistry>,
        write_admission: Arc<dyn InstallWriteAdmission>,
        game_fs_factory: Arc<dyn GameFileSystemFactory>,
        scan_cache: Arc<ExternalStateScanCache>,
        task_manager: Arc<TaskManager>,
        audit_log: Arc<dyn AuditLogWriter>,
        clock: Arc<dyn AppClock>,
    ) -> Self {
        Self {
            game_config_repository,
            mod_import_result_repository,
            install_manifest_repository,
            write_locks,
            write_admission,
            game_fs_factory,
            scan_cache,
            task_manager,
            audit_log,
            clock,
        }
    }

    /// 执行接管。成功与失败都写审计（成功走 `ReportAfterCommit`，失败 best-effort）。
    pub fn adopt(
        &self,
        request: ConfiguredExternalModAdoptRequest<'_>,
    ) -> Result<ExternalModAdoptOutcome, ConfiguredExternalModAdoptError> {
        match self.adopt_inner(&request) {
            Ok((plan, audit_degraded)) => Ok(ExternalModAdoptOutcome {
                claimed_file_count: plan.claims.len(),
                skipped_claimed_count: plan.skipped_claimed_count,
                skipped_changed_count: plan.skipped_changed_count,
                skipped_missing_count: plan.skipped_missing_count,
                audit_degraded,
            }),
            Err(error) => {
                // 取消不是失败，不进审计；其余失败都记（best-effort，不影响返回值）。
                if error != ConfiguredExternalModAdoptError::Cancelled {
                    let _ = self.record_audit(&request, "failure", None, Some(error.code()));
                }
                Err(error)
            }
        }
    }

    fn adopt_inner(
        &self,
        request: &ConfiguredExternalModAdoptRequest<'_>,
    ) -> Result<(ExternalAdoptPlan, bool), ConfiguredExternalModAdoptError> {
        // ---- 锁外：零副作用的前置拒绝 ----
        self.mod_import_result_repository
            .get_analysis(request.mod_id.as_str())
            .map_err(|_| ConfiguredExternalModAdoptError::ModUnavailable)?
            .ok_or(ConfiguredExternalModAdoptError::ModUnavailable)?;

        let record = self
            .scan_cache
            .record(request.game_id, request.profile_id, request.mod_id)
            .ok_or(ConfiguredExternalModAdoptError::ScanRequired)?;
        // 用户看到的预览：这就是接管要写出的东西，锁内重算若与之不同就拒绝。
        let previewed = previewed_claim_set(&record)?;

        if request.cancellation_token.is_cancelled() {
            return Err(ConfiguredExternalModAdoptError::Cancelled);
        }

        // ---- 锁内：准入 → 写锁 → 写入准入 → 重验 → 写 ----
        let _cross_process_guard = match self.write_locks.acquire_cross_process_for_task(
            request.game_id,
            request.profile_id,
            &self.task_manager,
            request.task_id,
        ) {
            Ok(guard) => guard,
            Err(CrossProcessWriteAdmissionError::Cancelled) => {
                return Err(ConfiguredExternalModAdoptError::Cancelled);
            }
            Err(error) => return Err(ConfiguredExternalModAdoptError::WriteAdmission(error)),
        };
        // 写操作用阻塞 lock：有安装在进行就等它结束，再在锁内重验——与 install/uninstall 同。
        let write_lock = self
            .write_locks
            .lock_for(request.game_id, request.profile_id);
        let _guard = write_lock
            .lock()
            .map_err(|_| ConfiguredExternalModAdoptError::Unavailable)?;

        if request.cancellation_token.is_cancelled() {
            return Err(ConfiguredExternalModAdoptError::Cancelled);
        }
        self.write_admission
            .ensure_write_allowed(request.game_id, request.profile_id)
            .map_err(ConfiguredExternalModAdoptError::WriteNotAllowed)?;

        // 重验 1：磁盘事实。锁内重新加载游戏目录——目录改了就会 stat 到别处而与记录指纹不一致。
        let game_instance = self
            .game_config_repository
            .load_game_instance(request.game_id)
            .map_err(|_| ConfiguredExternalModAdoptError::GameInstanceUnavailable)?
            .ok_or(ConfiguredExternalModAdoptError::GameInstanceUnavailable)?;
        let handles = self.game_fs_factory.create(&game_instance.root_dir);
        ensure_fingerprints_unchanged(handles.inspector.as_ref(), &record)?;

        // 重验 2：清单事实，以当下读到的清单重算认领集。
        let manifest = self
            .install_manifest_repository
            .load_manifest(request.profile_id)
            .map_err(|_| ConfiguredExternalModAdoptError::ManifestUnavailable)?;
        let plan = derive_external_adopt_plan(
            request.mod_id,
            &record.prepared,
            &record.summary.files,
            &record.game_files,
            manifest.as_ref(),
        )
        .map_err(map_plan_error)?;
        let derived: Vec<&InstallTargetPath> =
            plan.claims.iter().map(|claim| &claim.target_path).collect();
        if derived != previewed {
            return Err(ConfiguredExternalModAdoptError::Stale);
        }

        // ---- 提交屏障：从这里到 save_manifest 结束不可取消 ----
        if self
            .task_manager
            .block_task_cancellation(request.task_id)
            .is_err()
        {
            if request.cancellation_token.is_cancelled() {
                return Err(ConfiguredExternalModAdoptError::Cancelled);
            }
            return Err(ConfiguredExternalModAdoptError::Unavailable);
        }

        let entries = plan.manifest_entries(request.mod_id, request.layer);
        let merged = append_adopted_entries(
            request.profile_id,
            manifest,
            entries,
            self.manifest_timestamp(),
        )
        .map_err(|_| ConfiguredExternalModAdoptError::Unavailable)?;
        self.install_manifest_repository
            .save_manifest(&merged)
            .map_err(|_| ConfiguredExternalModAdoptError::ManifestWriteFailed)?;

        // 清单已认领它，「外部状态」这个问题不再成立；旧记录留着只会日后误导。
        self.scan_cache
            .forget(request.game_id, request.profile_id, request.mod_id);

        let audit_ok = self.record_audit(request, "success", Some(&plan), None);
        Ok((plan, !audit_ok))
    }

    fn manifest_timestamp(&self) -> String {
        // 与安装提交同一格式（`unix:<秒>`），取不到时钟时退化为 0 而不是失败——
        // 时间戳只是元数据，不该阻断一次已经通过全部重验的写入。
        let seconds = self.clock.now_unix_millis().unwrap_or(0) / 1000;
        format!("unix:{seconds}")
    }

    /// 审计只带 id 与计数：审计写入器会拒绝任何含 `/`、`\` 的值，路径本来就不进。
    fn record_audit(
        &self,
        request: &ConfiguredExternalModAdoptRequest<'_>,
        result: &str,
        plan: Option<&ExternalAdoptPlan>,
        error_code: Option<&str>,
    ) -> bool {
        let timestamp_unix_millis = self.clock.now_unix_millis().unwrap_or_default();
        let mut fields = BTreeMap::new();
        fields.insert("task_id".to_owned(), request.task_id.to_owned());
        fields.insert("game_id".to_owned(), request.game_id.as_str().to_owned());
        fields.insert("mod_id".to_owned(), request.mod_id.as_str().to_owned());
        fields.insert(
            "profile_id".to_owned(),
            request.profile_id.as_str().to_owned(),
        );
        fields.insert(
            "claimed_file_count".to_owned(),
            plan.map(|plan| plan.claims.len())
                .unwrap_or_default()
                .to_string(),
        );
        fields.insert(
            "skipped_claimed_count".to_owned(),
            plan.map(|plan| plan.skipped_claimed_count)
                .unwrap_or_default()
                .to_string(),
        );
        fields.insert(
            "skipped_changed_count".to_owned(),
            plan.map(|plan| plan.skipped_changed_count)
                .unwrap_or_default()
                .to_string(),
        );
        fields.insert(
            "skipped_missing_count".to_owned(),
            plan.map(|plan| plan.skipped_missing_count)
                .unwrap_or_default()
                .to_string(),
        );
        if let Some(error_code) = error_code {
            fields.insert("error_code".to_owned(), error_code.to_owned());
        }

        self.audit_log
            .record_with_policy(
                AuditLogEvent {
                    timestamp_unix_millis,
                    category: "install".to_owned(),
                    operation: "adopt_external_mod".to_owned(),
                    result: result.to_owned(),
                    fields,
                },
                AuditWriteFailurePolicy::for_commit_result(result),
            )
            .is_ok()
    }
}

/// 用户在弹窗里看到的预览认领集：matched ∧ 记录里无占用者。
///
/// 记录的占用归因是「首条归属」视图，写门禁重算用「任一条目」口径——两者对
/// 「有没有主」的结论一致（排障手册 4.9 讨论的差别只在「主是谁」），所以可以直接比对。
/// 前置拒绝也在这里做：含 unreadable、或预览为空，锁都不用拿。
fn previewed_claim_set(
    record: &ExternalStateScanRecord,
) -> Result<Vec<&InstallTargetPath>, ConfiguredExternalModAdoptError> {
    if record.prepared.len() != record.summary.files.len()
        || record.prepared.len() != record.claimed_by.len()
        || record.prepared.len() != record.game_files.len()
    {
        return Err(ConfiguredExternalModAdoptError::Unavailable);
    }
    if record
        .summary
        .files
        .contains(&ExternalFileState::Unreadable)
    {
        return Err(ConfiguredExternalModAdoptError::UnreadableFiles);
    }
    let previewed: Vec<&InstallTargetPath> = record
        .prepared
        .iter()
        .zip(&record.summary.files)
        .zip(&record.claimed_by)
        .filter(|((_, state), claimed_by)| {
            **state == ExternalFileState::Matched && claimed_by.is_none()
        })
        .map(|((target, _), _)| &target.target_path)
        .collect();
    if previewed.is_empty() {
        return Err(ConfiguredExternalModAdoptError::NothingToAdopt);
    }
    Ok(previewed)
}

/// stage-3 同款复核：任一文件出现、消失或改动都算漂移。
fn ensure_fingerprints_unchanged(
    inspector: &dyn InstallGameFileInspector,
    record: &ExternalStateScanRecord,
) -> Result<(), ConfiguredExternalModAdoptError> {
    if record.fingerprints.len() != record.prepared.len() {
        return Err(ConfiguredExternalModAdoptError::Unavailable);
    }
    for (target, recorded) in record.prepared.iter().zip(&record.fingerprints) {
        let current = inspector
            .stat_game_file(&target.target_path)
            .map_err(|_| ConfiguredExternalModAdoptError::GameFileUnavailable)?;
        let unchanged = match (recorded, &current) {
            (Some(recorded), Some(current)) => recorded.matches(current),
            (None, None) => true,
            _ => false,
        };
        if !unchanged {
            return Err(ConfiguredExternalModAdoptError::Stale);
        }
    }
    Ok(())
}

fn map_plan_error(error: ExternalAdoptPlanError) -> ConfiguredExternalModAdoptError {
    match error {
        ExternalAdoptPlanError::UnreadableFiles { .. } => {
            ConfiguredExternalModAdoptError::UnreadableFiles
        }
        ExternalAdoptPlanError::NothingToAdopt => ConfiguredExternalModAdoptError::NothingToAdopt,
        ExternalAdoptPlanError::AlreadyInstalled => {
            ConfiguredExternalModAdoptError::AlreadyInstalled
        }
        ExternalAdoptPlanError::ManifestNotTrusted { .. } => {
            ConfiguredExternalModAdoptError::ManifestNotTrusted
        }
        ExternalAdoptPlanError::MissingGameFileSummary
        | ExternalAdoptPlanError::InconsistentFacts => ConfiguredExternalModAdoptError::Unavailable,
    }
}
