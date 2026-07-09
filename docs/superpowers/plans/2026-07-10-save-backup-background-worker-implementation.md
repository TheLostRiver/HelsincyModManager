# P7.1 存档自动备份后台 Worker Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在不注册真实 Windows Scheduled Task、不中断既有存档安全链路的前提下，交付可构建、可单次运行、可测试的存档自动备份 headless worker 与后台注册契约。

**Architecture:** 在 `hmm-core` / `hmm-ports` 定义稳定的后台注册状态与 port；在 `hmm-app` 增加按 Profile 枚举、复用既有 scheduler 与 task runner 的单次 worker 用例；在 `src-tauri` 抽取不依赖 WebView 的服务装配并提供只接受 `--once` 的独立 binary。worker 使用既有 SQLite scheduler lease 进行跨进程去重，并且在没有真实平台注册时始终保持 `tray_only`，不宣称 `protected`。

**Tech Stack:** Rust 2021、Tauri 2、SQLite/rusqlite、`uuid`、`dirs` 6、现有 `hmm-core` / `hmm-ports` / `hmm-app` / `hmm-infra` workspace crates。

## Global Constraints

- 不注册、删除或调用真实 Windows Scheduled Task；真实平台注册留给 P7.2。
- worker 只接受固定 `--once`，不接受路径、Profile、游戏 ID、备份根目录、lease owner、manifest 或 Steam ID 参数。
- 不实现第二套备份写入、路径校验、manifest、retention、恢复或游戏运行检测；必须复用既有 save-backup 链路。
- worker 与客户端的跨进程重复执行必须由 `SaveBackupSchedulerStateRepository::acquire_due_lease` 消除；不得依赖 in-memory task registry。
- `Running` 与 `Unknown` 都必须延后，不获取 lease、不启动存档备份。
- heartbeat 不得将 `backgroundProtectionEnabled` 设为 `true`，也不得将 `background_status` 改为 `protected`。
- 所有测试使用 fake ports、固定 clock、临时 SQLite 或人工 fixture；不使用真实 MHW、Steam userdata、玩家存档、游戏进程或系统计划任务。
- 日志、审计字段、摘要与错误消息不得包含完整本地路径、用户名、Steam ID、存档内容、manifest、hash、token、cookie 或原始底层错误。
- 不新增 Tauri command、DTO 或前端开关；本切片不修改公开 frontend/backend contract。

---

## File Structure

| 文件 | 责任 |
| --- | --- |
| `src-tauri/crates/hmm-core/src/save_backup.rs` | 后台注册的稳定领域状态枚举。 |
| `src-tauri/crates/hmm-core/src/lib.rs` | 导出后台注册状态类型。 |
| `src-tauri/crates/hmm-ports/src/save_backup.rs` | `SaveBackupBackgroundRegistry` port。 |
| src-tauri/crates/hmm-ports/src/lib.rs | 导出新 port。 |
| src-tauri/crates/hmm-infra/src/save_backup_background_registry.rs | 不接系统计划任务的安全 fallback registry。 |
| src-tauri/crates/hmm-infra/src/lib.rs | 导出 fallback registry。 |
| src-tauri/crates/hmm-infra/tests/save_backup_background_registry.rs | 验证 fallback 不会错误宣称已注册。 |
| `src-tauri/crates/hmm-app/src/save_backup_background_worker.rs` | 单次 worker use case、聚合结果、稳定错误码与最小审计。 |
| `src-tauri/crates/hmm-app/src/lib.rs` | 导出 worker use case 类型。 |
| `src-tauri/crates/hmm-app/tests/save_backup_background_worker.rs` | fake ports 覆盖枚举、lease、游戏运行保护、失败隔离和 heartbeat。 |
| `src-tauri/src/state.rs` | 从 AppData 路径建立可共享状态；组装 `SaveBackupBackgroundWorker`。 |
| `src-tauri/src/background_worker.rs` | `--once` 参数解析、AppData 解析、无 UI 的 worker 入口。 |
| `src-tauri/src/bin/hmm-save-backup-worker.rs` | 独立 binary `main`，只调用库入口并映射稳定退出码。 |
| `src-tauri/src/lib.rs` | 暴露 worker library entrypoint，不改变 `run()` GUI 路径。 |
| `Cargo.toml` / `src-tauri/Cargo.toml` | 添加直接使用的 `dirs = 6.0.0` 与 `uuid` workspace 依赖。 |
| `docs/SAVE_BACKUP_BACKGROUND_AUTOMATION_DESIGN.md` | 记录 P7.1 worker 已实现但仍非真实后台保障。 |
| `docs/SAVE_BACKUP_BACKGROUND_SCHEDULER_CORE_PLAN.md` | 链接 P7.1 worker 规格，说明 Scheduled Task 仍在 P7.2。 |
| `docs/TESTING.md` | 增加 worker 聚焦测试命令。 |
| `TODO.md` | P7.1 完成后保留“真实后台守护/Scheduled Task 未完成”的诚实状态，并补充 worker 基础已完成。 |

---

### Task 1: 定义后台注册状态与 Port 契约

**Files:**
- Modify: `src-tauri/crates/hmm-core/src/save_backup.rs`
- Modify: `src-tauri/crates/hmm-core/src/lib.rs`
- Modify: `src-tauri/crates/hmm-ports/src/save_backup.rs`
- Modify: `src-tauri/crates/hmm-ports/src/lib.rs`
- Test: `src-tauri/crates/hmm-core/src/save_backup.rs` 的 `#[cfg(test)]` 模块

**Interfaces:**
- Produces: `SaveBackupBackgroundRegistrationStatus::{NotRegistered, Registered, RegistrationFailed, PermissionRequired, UnsupportedPlatform}`。
- Produces: `SaveBackupBackgroundRegistrationStatus::as_str(&self) -> &'static str`。
- Produces: `SaveBackupBackgroundRegistry: Send + Sync`，包含 `inspect`、`register`、`unregister` 三个无路径、无前端 DTO 的方法。
- Produces: `UnsupportedSaveBackupBackgroundRegistry`；三个方法都返回 `UnsupportedPlatform`，使 P7.1 不会误报平台后台入口已注册。
- Consumes: 已有 `SaveBackupBackgroundProtectionStatus`；两者不能混用：registration 表示平台入口状态，protection 表示用户可见保障状态。

- [x] **Step 1: 写失败的 core 枚举稳定字符串测试**

在 `save_backup.rs` 的测试模块添加：

```rust
#[test]
fn background_registration_statuses_have_stable_codes() {
    assert_eq!(
        SaveBackupBackgroundRegistrationStatus::NotRegistered.as_str(),
        "not_registered"
    );
    assert_eq!(
        SaveBackupBackgroundRegistrationStatus::Registered.as_str(),
        "registered"
    );
    assert_eq!(
        SaveBackupBackgroundRegistrationStatus::RegistrationFailed.as_str(),
        "registration_failed"
    );
    assert_eq!(
        SaveBackupBackgroundRegistrationStatus::PermissionRequired.as_str(),
        "permission_required"
    );
    assert_eq!(
        SaveBackupBackgroundRegistrationStatus::UnsupportedPlatform.as_str(),
        "unsupported_platform"
    );
}
```

- [x] **Step 2: 运行测试，确认因类型不存在失败**

Run:

```powershell
cargo test -p hmm-core background_registration_statuses_have_stable_codes
```

Expected: FAIL，错误包含 `SaveBackupBackgroundRegistrationStatus` 未定义。

- [x] **Step 3: 实现最小领域类型和 port**

在 `hmm-core/src/save_backup.rs` 添加：

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaveBackupBackgroundRegistrationStatus {
    NotRegistered,
    Registered,
    RegistrationFailed,
    PermissionRequired,
    UnsupportedPlatform,
}

impl SaveBackupBackgroundRegistrationStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NotRegistered => "not_registered",
            Self::Registered => "registered",
            Self::RegistrationFailed => "registration_failed",
            Self::PermissionRequired => "permission_required",
            Self::UnsupportedPlatform => "unsupported_platform",
        }
    }
}
```

在 `hmm-ports/src/save_backup.rs` 添加：

```rust
pub trait SaveBackupBackgroundRegistry: Send + Sync {
    fn inspect(&self) -> Result<SaveBackupBackgroundRegistrationStatus>;
    fn register(&self) -> Result<SaveBackupBackgroundRegistrationStatus>;
    fn unregister(&self) -> Result<SaveBackupBackgroundRegistrationStatus>;
}
```

在两个 `lib.rs` 导出新类型与 trait。`register` / `unregister` 不接收命令行、路径或用户输入；未来 Windows adapter 自行解析已安装 worker 的受控位置。

在 `hmm-infra` 添加安全 fallback：

```rust
#[derive(Debug, Default)]
pub struct UnsupportedSaveBackupBackgroundRegistry;

impl SaveBackupBackgroundRegistry for UnsupportedSaveBackupBackgroundRegistry {
    fn inspect(&self) -> Result<SaveBackupBackgroundRegistrationStatus> {
        Ok(SaveBackupBackgroundRegistrationStatus::UnsupportedPlatform)
    }

    fn register(&self) -> Result<SaveBackupBackgroundRegistrationStatus> {
        Ok(SaveBackupBackgroundRegistrationStatus::UnsupportedPlatform)
    }

    fn unregister(&self) -> Result<SaveBackupBackgroundRegistrationStatus> {
        Ok(SaveBackupBackgroundRegistrationStatus::UnsupportedPlatform)
    }
}
```

在 `save_backup_background_registry.rs` 测试三个方法都返回 `UnsupportedPlatform`，并断言其 API 没有 command、path 或用户输入参数。

- [x] **Step 4: 运行聚焦测试和 crate 检查**

Run:

```powershell
cargo test -p hmm-core background_registration_statuses_have_stable_codes
cargo test -p hmm-infra --test save_backup_background_registry
cargo check -p hmm-ports
cargo check -p hmm-infra
```

Expected: 四个命令均成功退出。

- [x] **Step 5: 提交契约切片**

```powershell
git add src-tauri/crates/hmm-core/src/save_backup.rs src-tauri/crates/hmm-core/src/lib.rs src-tauri/crates/hmm-ports/src/save_backup.rs src-tauri/crates/hmm-ports/src/lib.rs src-tauri/crates/hmm-infra/src/save_backup_background_registry.rs src-tauri/crates/hmm-infra/src/lib.rs src-tauri/crates/hmm-infra/tests/save_backup_background_registry.rs
git commit -m "feat: 定义后台备份注册契约"
```

---

### Task 2: 实现单次后台 Worker 用例及应用层测试

**Files:**
- Create: `src-tauri/crates/hmm-app/src/save_backup_background_worker.rs`
- Modify: `src-tauri/crates/hmm-app/src/lib.rs`
- Create: `src-tauri/crates/hmm-app/tests/save_backup_background_worker.rs`

**Interfaces:**
- Consumes: `ProfileRepository::list_all()`、`ProfileSaveSettingsRepository::get_settings()`、`SaveBackupAutoSchedulerService::check_profile()`、`SaveBackupTaskService::start_save_backup_task()`、`SaveBackupTaskRunner::run_save_backup_task()`、`SaveBackupSchedulerStateRepository::record_worker_heartbeat()`、`AuditLogWriter`、`AppClock`。
- Produces: `SaveBackupBackgroundWorker::run_once(&self, worker_instance_id: &str) -> Result<SaveBackupBackgroundWorkerRunSummary, SaveBackupBackgroundWorkerError>`。
- Produces: `SaveBackupBackgroundWorkerError::{ProfileListUnavailable, ClockUnavailable}` 与 `code(&self) -> &'static str`，稳定值分别是 `save_backup_background_profile_list_unavailable`、`save_backup_background_clock_unavailable`。
- Produces: 汇总字段 `checked_profiles`、`started_tasks`、`deferred_profiles`、`failed_profiles`；不返回路径、Profile 名称、Steam ID 或 manifest。

- [x] **Step 1: 写失败的应用层测试**

在新测试文件建立 fake profile/settings/scheduler-state/audit/task executor，并先覆盖以下行为：

```rust
#[test]
fn run_once_starts_due_auto_backups_and_records_tray_only_heartbeat() {
    let worker = worker_with_profiles(vec!["default"], schedule_daily());

    let summary = worker.run_once("worker-a").expect("worker runs");

    assert_eq!(summary.checked_profiles, 1);
    assert_eq!(summary.started_tasks, 1);
    assert_eq!(summary.deferred_profiles, 0);
    assert_eq!(summary.failed_profiles, 0);
    assert_eq!(heartbeat_statuses(), vec![SaveBackupBackgroundProtectionStatus::TrayOnly]);
}

#[test]
fn run_once_does_not_start_or_lease_when_game_is_running() {
    let worker = worker_with_game_status(GameRunningStatus::Running);

    let summary = worker.run_once("worker-a").expect("worker runs");

    assert_eq!(summary.started_tasks, 0);
    assert_eq!(lease_requests(), 0);
    assert_eq!(summary.deferred_profiles, 1);
}

#[test]
fn run_once_continues_after_one_profile_fails() {
    let worker = worker_with_one_settings_failure_and_one_due_profile();

    let summary = worker.run_once("worker-a").expect("per-profile failure is isolated");

    assert_eq!(summary.failed_profiles, 1);
    assert_eq!(summary.started_tasks, 1);
}
```

额外覆盖：manual schedule 不进入检查；两个 worker 对同一 due Profile 只有一个获得 lease；`Unknown` 与 `Running` 同样延后；审计字段只含 `game_id`、`profile_id`、`trigger`、`error_code` 等短值。

- [x] **Step 2: 运行测试，确认模块不存在失败**

Run:

```powershell
cargo test -p hmm-app --test save_backup_background_worker
```

Expected: FAIL，错误包含 worker module 或导入类型不存在。

- [x] **Step 3: 实现 `SaveBackupBackgroundWorker`**

实现核心结构：

```rust
pub struct SaveBackupBackgroundWorker {
    game_ids: Vec<GameId>,
    profile_repository: Arc<dyn ProfileRepository>,
    save_settings_repository: Arc<dyn ProfileSaveSettingsRepository>,
    scheduler: Arc<SaveBackupAutoSchedulerService>,
    task_service: Arc<SaveBackupTaskService>,
    task_runner: Arc<SaveBackupTaskRunner>,
    scheduler_state_repository: Arc<dyn SaveBackupSchedulerStateRepository>,
    audit_log: Arc<dyn AuditLogWriter>,
    clock: Arc<dyn AppClock>,
}

#[derive(Debug, thiserror::Error)]
pub enum SaveBackupBackgroundWorkerError {
    #[error("profile list unavailable")]
    ProfileListUnavailable,
    #[error("worker clock unavailable")]
    ClockUnavailable,
}

impl SaveBackupBackgroundWorkerError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::ProfileListUnavailable => "save_backup_background_profile_list_unavailable",
            Self::ClockUnavailable => "save_backup_background_clock_unavailable",
        }
    }
}
```

`run_once` 的最小流程：

```rust
let _worker_started_at = self.clock.now_unix_millis()
    .map_err(|_| SaveBackupBackgroundWorkerError::ClockUnavailable)?;

for profile in self.profile_repository.list_all()
    .map_err(|_| SaveBackupBackgroundWorkerError::ProfileListUnavailable)? {
    let auto_enabled = match self.save_settings_repository.get_settings(&profile.id) {
        Ok(Some(settings)) => settings.schedule.cadence != BackupCadence::Manual,
        Ok(None) => false,
        Err(_) => {
            summary.failed_profiles += 1;
            self.record_profile_error(&game_id, &profile.id, "save_backup_auto_settings_unavailable");
            continue;
        }
    };
    if !auto_enabled {
        continue;
    }

    let check = match self.scheduler.check_profile(SaveBackupAutoCheckRequest {
        game_id: game_id.clone(),
        profile_id: ProfileId::new(profile.id.clone()),
    }) {
        Ok(check) => check,
        Err(error) => {
            summary.failed_profiles += 1;
            self.record_profile_error(&game_id, &profile.id, error.code());
            continue;
        }
    };

    if self.scheduler_state_repository.record_worker_heartbeat(SaveBackupWorkerHeartbeat {
        game_id: game_id.clone(),
        profile_id: ProfileId::new(profile.id.clone()),
        worker_instance_id: worker_instance_id.to_owned(),
        checked_at: check.checked_at,
        status: SaveBackupBackgroundProtectionStatus::TrayOnly,
    }).is_err() {
        summary.failed_profiles += 1;
        self.record_profile_error(&game_id, &profile.id, "save_backup_scheduler_unavailable");
        continue;
    }

    if check.pending_reason.is_some() {
        summary.deferred_profiles += 1;
    }
    if let Some(request) = check.due_task {
        let task = self.task_service.start_save_backup_task(request.clone())
            .map_err(|error| self.record_task_start_failure(&game_id, &profile.id, error))?;
        self.task_runner.run_save_backup_task(&task.task_id, request)
            .map_err(|_| self.record_runner_failure(&game_id, &profile.id))?;
        summary.started_tasks += 1;
    }
    summary.checked_profiles += 1;
}
```

把 `TaskManagerError` 和 runner failure 转换为 profile 级错误：记录稳定短码后继续循环，不能把单一 Profile failure 变成全局失败。只有 `list_all` 和 worker clock 获取失败返回顶层错误。

所有 `AuditLogEvent` 使用固定 `category = "save_backup"`、`operation = "background_worker"`，字段仅包含短 ID、`trigger = "auto"` 与稳定 `error_code`。

- [x] **Step 4: 运行应用层测试与回归测试**

Run:

```powershell
cargo test -p hmm-app --test save_backup_background_worker
cargo test -p hmm-app --test save_backup_scheduler
cargo test -p hmm-app --test save_backup_task
```

Expected: 全部成功退出；测试证明 `TrayOnly` heartbeat 不改变为 `Protected`。

- [x] **Step 5: 提交应用层 worker 切片**

```powershell
git add src-tauri/crates/hmm-app/src/save_backup_background_worker.rs src-tauri/crates/hmm-app/src/lib.rs src-tauri/crates/hmm-app/tests/save_backup_background_worker.rs
git commit -m "feat: 添加存档自动备份后台 worker"
```

---

### Task 3: 抽取无 WebView 的服务装配并提供 `--once` Binary

**Files:**
- Modify: `Cargo.toml`
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/src/state.rs`
- Create: `src-tauri/src/background_worker.rs`
- Create: `src-tauri/src/bin/hmm-save-backup-worker.rs`
- Modify: `src-tauri/src/lib.rs`
- Test: `src-tauri/src/background_worker.rs` 的 `#[cfg(test)]` 模块

**Interfaces:**
- Produces: `AppState::from_app_data_dir(app_data_dir: PathBuf) -> Result<AppState, String>`。
- Produces: `hmm_tauri::run_save_backup_worker_once_from_env() -> Result<(), BackgroundWorkerEntryError>`。
- Produces: `parse_background_worker_args<I, T>(args: I) -> Result<BackgroundWorkerCommand, BackgroundWorkerEntryError>`，唯一合法命令为 `--once`。
- Consumes: `tauri::generate_context!().config().identifier` 与 `dirs::data_dir()`，以 GUI 使用的 app identifier 解析同一 AppData 根目录。

- [x] **Step 1: 写失败的 CLI 参数与退出安全测试**

在 `background_worker.rs` 添加：

```rust
#[test]
fn parses_only_the_once_command() {
    assert_eq!(
        parse_background_worker_args(["hmm-save-backup-worker", "--once"]),
        Ok(BackgroundWorkerCommand::Once)
    );
}

#[test]
fn rejects_paths_and_internal_scheduler_arguments() {
    for args in [
        ["hmm-save-backup-worker", "--save-directory", "C:/save"],
        ["hmm-save-backup-worker", "--profile", "default"],
        ["hmm-save-backup-worker", "--lease-owner", "worker-a"],
    ] {
        let error = parse_background_worker_args(args).expect_err("unsafe argument rejected");
        assert_eq!(error.code(), "save_backup_background_worker_invalid_args");
    }
}
```

- [x] **Step 2: 运行测试，确认入口模块不存在失败**

Run:

```powershell
cargo test -p hmm-tauri background_worker
```

Expected: FAIL，错误包含 `background_worker` 模块或 `parse_background_worker_args` 未定义。

- [x] **Step 3: 实现共享状态装配与 binary 入口**

在 workspace 根 `Cargo.toml` 添加：

```toml
dirs = "6.0.0"
```

在 `src-tauri/Cargo.toml` 添加：

```toml
dirs.workspace = true
uuid.workspace = true
```

将 `AppState::new(app_handle)` 调整为只解析 app data 根目录后调用：

```rust
pub fn new(app_handle: &AppHandle) -> Result<Self, String> {
    let app_data_dir = app_handle.path().app_data_dir()
        .map_err(|error| format!("failed to resolve app data dir: {error}"))?;
    Self::from_app_data_dir(app_data_dir)
}

pub fn from_app_data_dir(app_data_dir: PathBuf) -> Result<Self, String> {
    // 将当前 AppState::new 中从数据库打开到 struct literal 返回的既有构造语句原样移动到这里；所有路径均从参数 app_data_dir 派生，并在 struct literal 中新增 save_backup_background_worker 字段。
}
```

在 state 中把已建立的 profile repository、scheduler state repository、scheduler、task service、task runner 和 audit/clock 组装为 `SaveBackupBackgroundWorker`，并新增字段：

```rust
pub save_backup_background_worker: Arc<SaveBackupBackgroundWorker>,
```

新增 `background_worker.rs`：

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackgroundWorkerCommand { Once }

pub fn parse_background_worker_args<I, T>(args: I) -> Result<BackgroundWorkerCommand, BackgroundWorkerEntryError>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString>,
{
    let args = args.into_iter().map(Into::into).collect::<Vec<_>>();
    match args.as_slice() {
        [_, flag] if flag.to_str() == Some("--once") => Ok(BackgroundWorkerCommand::Once),
        _ => Err(BackgroundWorkerEntryError::InvalidArgs),
    }
}

#[derive(Debug)]
pub enum BackgroundWorkerEntryError {
    InvalidArgs,
    AppDataUnavailable,
    StateUnavailable,
    WorkerFailed(&'static str),
}

impl BackgroundWorkerEntryError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::InvalidArgs => "save_backup_background_worker_invalid_args",
            Self::AppDataUnavailable => "save_backup_background_app_data_unavailable",
            Self::StateUnavailable => "save_backup_background_state_unavailable",
            Self::WorkerFailed(code) => code,
        }
    }
}

pub fn run_save_backup_worker_once_from_env() -> Result<(), BackgroundWorkerEntryError> {
    parse_background_worker_args(std::env::args_os())?;
    let context = tauri::generate_context!();
    let app_data_dir = dirs::data_dir()
        .map(|path| path.join(&context.config().identifier))
        .ok_or(BackgroundWorkerEntryError::AppDataUnavailable)?;
    let state = AppState::from_app_data_dir(app_data_dir)
        .map_err(|_| BackgroundWorkerEntryError::StateUnavailable)?;
    let worker_id = format!("worker-{}", uuid::Uuid::new_v4());
    state.save_backup_background_worker.run_once(&worker_id)
        .map_err(|error| BackgroundWorkerEntryError::WorkerFailed(error.code()))?;
    Ok(())
}
```

入口只输出稳定错误码。`src/bin/hmm-save-backup-worker.rs`：

```rust
fn main() {
    if let Err(error) = hmm_tauri::run_save_backup_worker_once_from_env() {
        eprintln!("{}", error.code());
        std::process::exit(1);
    }
}
```

在 `src-tauri/src/lib.rs` 增加 `mod background_worker;` 并公开包装函数；保留 GUI `run()` 不变。不得在 worker 路径调用 `tauri::Builder`、`run()`、`register_thumbnail_protocol` 或 window lifecycle 注册。

- [x] **Step 4: 运行 headless 与 Tauri 回归检查**

Run:

```powershell
cargo test -p hmm-tauri background_worker
cargo check -p hmm-tauri --bin hmm-save-backup-worker
cargo test -p hmm-tauri save_backup
```

Expected: 所有命令成功退出；`--once` 解析测试通过，任何路径/内部参数被拒绝。

- [x] **Step 5: 提交 headless 入口切片**

```powershell
git add Cargo.toml src-tauri/Cargo.toml src-tauri/src/state.rs src-tauri/src/background_worker.rs src-tauri/src/bin/hmm-save-backup-worker.rs src-tauri/src/lib.rs
git commit -m "feat: 添加单次后台备份 worker 入口"
```

---

### Task 4: 同步文档、聚焦验证并做交付前自审

**Files:**
- Modify: `docs/SAVE_BACKUP_BACKGROUND_AUTOMATION_DESIGN.md`
- Modify: `docs/SAVE_BACKUP_BACKGROUND_SCHEDULER_CORE_PLAN.md`
- Modify: `docs/TESTING.md`
- Modify: `docs/superpowers/plans/2026-07-10-save-backup-background-worker-implementation.md`（勾选实际完成步骤）

**Interfaces:**
- Produces: 文档明确说明 P7.1 是 headless worker 与 contract 基础，P7.2 才是 Windows Scheduled Task 注册与真实 `protected` 状态。
- Produces: `docs/TESTING.md` 中可复制的 worker 聚焦验证命令。

- [x] **Step 1: 写文档一致性失败检查**

已运行：

```powershell
rg -n "P7.1|headless worker|Scheduled Task|protected|tray_only" docs/SAVE_BACKUP_BACKGROUND_AUTOMATION_DESIGN.md docs/SAVE_BACKUP_BACKGROUND_SCHEDULER_CORE_PLAN.md
```

Expected: 当前文档没有将 P7.1 的实际完成情况与 P7.2 的平台注册边界完整区分，作为同步前基线。

- [x] **Step 2: 更新设计、测试与总任务文档**

在两个后台设计文档加入以下不可变语义：

```text
P7.1 已提供单次 headless worker、持久化 lease 复用、heartbeat 和 fake registry contract。
P7.1 不注册真实 Windows Scheduled Task；因此不把 worker heartbeat 视为系统级后台保护，不显示 protected。
P7.2 才负责用户级 Scheduled Task 注册、移除、健康确认、设置开关和退出前提示。
```

在 `docs/TESTING.md` 增加：

```powershell
cargo test -p hmm-app --test save_backup_background_worker
cargo test -p hmm-app --test save_backup_scheduler
cargo test -p hmm-app --test save_backup_task
cargo test -p hmm-infra --test save_backup_scheduler_repository
cargo test -p hmm-tauri background_worker
cargo check -p hmm-tauri --bin hmm-save-backup-worker
```

- [x] **Step 3: 运行聚焦测试与完整验证**

Run:

```powershell
cargo test -p hmm-app --test save_backup_background_worker
cargo test -p hmm-infra --test save_backup_scheduler_repository
cargo test -p hmm-tauri background_worker
cargo check -p hmm-tauri --bin hmm-save-backup-worker
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify.ps1
```

Expected: 每条命令成功退出。若某条命令无法运行，记录精确命令、失败输出摘要、是否由本切片造成，以及未覆盖的风险；不得报告通过。

- [x] **Step 4: 运行最终安全与产物检查**

Run:

```powershell
git diff --check
git status --short
```

Expected: `git diff --check` 成功；状态中只含本切片的四份文档。

- [ ] **Step 5: 提交文档与验证同步**

```powershell
git add docs/SAVE_BACKUP_BACKGROUND_AUTOMATION_DESIGN.md docs/SAVE_BACKUP_BACKGROUND_SCHEDULER_CORE_PLAN.md docs/TESTING.md docs/superpowers/plans/2026-07-10-save-backup-background-worker-implementation.md
git commit -m "docs: 同步后台备份 worker 验证边界"
```

---

## Final Review Checklist

- [ ] 只存在一个存档写入路径，worker 没有直接执行文件复制、删除、重命名或路径计算。
- [ ] worker 不初始化 WebView 或 Tauri GUI runtime。
- [ ] worker 与客户端的重复执行由 SQLite lease 去重。
- [ ] `Running` / `Unknown` 不会启动自动备份。
- [ ] heartbeat 不会造成 `protected` 假阳性。
- [ ] 所有错误、日志和审计字段都不泄漏敏感数据。
- [ ] P7.2 的 Scheduled Task 注册、设置开关和退出提示仍保持未完成。
