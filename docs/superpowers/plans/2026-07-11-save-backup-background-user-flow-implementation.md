# P7.2b 后台自动备份用户流程 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 为 P7.2a Windows Scheduled Task 平台核心增加应用级持久化启停、首次 heartbeat 验证、Settings/Profile 用户状态和统一的 fail-closed 退出流程。

**Architecture:** 以 SQLite 单例行保存应用级用户意图、启用时间和全局 worker heartbeat；`hmm-app` 组合该状态、Scheduled Task read-back 和 Profile 自动备份计划，派生控制状态与退出决策。Tauri 只暴露窄命令和白名单 DTO，前端只展示后端事实；所有备份仍走既有 scheduler/lease/task/backup/audit 链路。

**Tech Stack:** Rust 2021、rusqlite/rusqlite_migration、Tauri 2、React 19、TypeScript、Node test runner、PowerShell verification scripts。

---

## 0. 开始前约束

- 在独立 worktree 和分支 `hy/save-backup-background-ux` 执行，不在带有用户未提交改动的根 worktree 实施。
- 前置设计提交：`c785f4d docs: 设计 P7.2b 后台保护用户流程`。
- 必读：
  - `AGENTS.md`
  - `docs/superpowers/specs/2026-07-11-save-backup-background-user-flow-design.md`
  - `docs/SAVE_BACKUP_BACKGROUND_AUTOMATION_DESIGN.md`
  - `docs/FRONTEND_BACKEND_CONTRACT.md`
  - `docs/TESTING.md`
  - `docs/LOGGING.md`
  - `SECURITY.md`
- 自动测试不得创建、启动、更新或删除真实 Scheduled Task，不得读取真实玩家存档。
- `src-tauri/tauri.windows.conf.json`、NSIS/WiX installer hooks 和自动卸载 cleanup 属于 P7.2c，本计划不得修改。
- 每个任务开始前确认 `git status --short --branch`；只提交该任务列出的文件。

## 1. 文件职责映射

| 文件 | 职责 |
| --- | --- |
| `src-tauri/crates/hmm-core/src/save_backup.rs` | 全局后台设置和值域、`starting` 状态 |
| `src-tauri/crates/hmm-ports/src/save_backup.rs` | 全局设置 repository port |
| `src-tauri/crates/hmm-infra/src/sqlite/migrations/008_save_backup_background_settings.sql` | 单例表 schema |
| `src-tauri/crates/hmm-infra/src/sqlite/save_backup_background_settings_repository.rs` | SQLite repository 实现 |
| `src-tauri/crates/hmm-app/src/save_backup_background.rs` | enable/disable/status 编排 |
| `src-tauri/crates/hmm-app/src/save_backup_background_worker.rs` | 成功完成调度检查后写全局 heartbeat |
| `src-tauri/crates/hmm-app/src/save_backup_exit_guard.rs` | 是否存在自动计划、退出决策和 override 审计 |
| `src-tauri/src/save_backup_dto.rs` | 全局控制白名单 DTO |
| `src-tauri/src/save_backup_commands.rs` | 全局控制薄命令 |
| `src-tauri/src/window_lifecycle_commands.rs` | 后端重检、普通/override 退出、托盘统一事件 |
| `src/features/settings/backgroundProtectionApi.ts` | Settings feature-local typed API |
| `src/features/settings/BackgroundProtectionPanel.tsx` | 唯一正式启停 UI |
| `src/features/profiles/ProfilePage.tsx` | Profile 只读状态 |
| `src/app/window-lifecycle/*` | fail-closed 对话框与当次 override |

---

### Task 1: 定义全局后台状态与 Repository Port

**Files:**
- Modify: `src-tauri/crates/hmm-core/src/save_backup.rs`
- Modify: `src-tauri/crates/hmm-core/src/lib.rs`
- Modify: `src-tauri/crates/hmm-ports/src/save_backup.rs`
- Modify: `src-tauri/crates/hmm-ports/src/lib.rs`

- [x] **Step 1: 先写失败的 core 状态测试**

在 `save_backup.rs` 现有 tests 中加入：

```rust
#[test]
fn background_control_statuses_have_stable_codes() {
    assert_eq!(SaveBackupBackgroundProtectionStatus::Starting.as_str(), "starting");

    let state = SaveBackupBackgroundSettings::disabled();
    assert!(!state.desired_enabled);
    assert_eq!(state.enabled_at, None);
    assert_eq!(state.last_worker_heartbeat_at, None);
}
```

- [x] **Step 2: 运行 RED**

Run:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml -p hmm-core background_control_statuses_have_stable_codes
```

Expected: FAIL，`Starting` / `SaveBackupBackgroundSettings` 尚未定义。

- [x] **Step 3: 增加最小 domain 类型**

在 `save_backup.rs` 增加：

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaveBackupBackgroundSettings {
    pub desired_enabled: bool,
    pub enabled_at: Option<u128>,
    pub last_worker_heartbeat_at: Option<u128>,
    pub updated_at: u128,
}

impl SaveBackupBackgroundSettings {
    pub fn disabled() -> Self {
        Self {
            desired_enabled: false,
            enabled_at: None,
            last_worker_heartbeat_at: None,
            updated_at: 0,
        }
    }
}
```

给 `SaveBackupBackgroundProtectionStatus` 增加 `Starting`，并在 `as_str()` 映射为
`"starting"`。在 `hmm-core/src/lib.rs` re-export `SaveBackupBackgroundSettings`。

- [x] **Step 4: 定义 repository port**

在 `hmm-ports/src/save_backup.rs` 增加：

```rust
pub trait SaveBackupBackgroundSettingsRepository: Send + Sync {
    fn load(&self) -> anyhow::Result<SaveBackupBackgroundSettings>;
    fn begin_enable(&self, enabled_at: u128) -> anyhow::Result<()>;
    fn finish_disable(&self, updated_at: u128) -> anyhow::Result<()>;
    fn record_worker_heartbeat(&self, heartbeat_at: u128) -> anyhow::Result<()>;
}
```

导入 domain type，并从 `hmm-ports/src/lib.rs` re-export trait。

- [x] **Step 5: 运行 GREEN 与格式检查**

Run:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml -p hmm-core background_control_statuses_have_stable_codes
cargo test --manifest-path src-tauri/Cargo.toml -p hmm-ports background_registry_errors_have_stable_codes
cargo fmt --manifest-path src-tauri/Cargo.toml --check
```

Expected: 两个测试通过，fmt exit 0。

- [x] **Step 6: 提交**

```powershell
git add src-tauri/crates/hmm-core/src/save_backup.rs src-tauri/crates/hmm-core/src/lib.rs src-tauri/crates/hmm-ports/src/save_backup.rs src-tauri/crates/hmm-ports/src/lib.rs
git commit -m "feat: define background protection settings"
```

---

### Task 2: 实现 SQLite 单例状态

**Files:**
- Create: `src-tauri/crates/hmm-infra/src/sqlite/migrations/008_save_backup_background_settings.sql`
- Create: `src-tauri/crates/hmm-infra/src/sqlite/save_backup_background_settings_repository.rs`
- Create: `src-tauri/crates/hmm-infra/tests/save_backup_background_settings_repository.rs`
- Modify: `src-tauri/crates/hmm-infra/src/sqlite/migrations.rs`
- Modify: `src-tauri/crates/hmm-infra/src/sqlite/mod.rs`
- Modify: `src-tauri/crates/hmm-infra/src/lib.rs`

- [x] **Step 1: 写 repository RED 测试**

测试必须覆盖默认关闭、重新启用清旧 heartbeat、停用清状态、reopen 持久化：

```rust
#[test]
fn background_settings_round_trip_and_reset_old_heartbeat() {
    let (_temp, repo) = settings_repo();
    assert_eq!(repo.load().expect("load default"), SaveBackupBackgroundSettings::disabled());

    repo.begin_enable(1_000).expect("begin enable");
    repo.record_worker_heartbeat(1_100).expect("heartbeat");
    assert_eq!(repo.load().expect("load enabled").last_worker_heartbeat_at, Some(1_100));

    repo.begin_enable(2_000).expect("re-enable");
    let reenabled = repo.load().expect("load re-enabled");
    assert!(reenabled.desired_enabled);
    assert_eq!(reenabled.enabled_at, Some(2_000));
    assert_eq!(reenabled.last_worker_heartbeat_at, None);

    repo.finish_disable(3_000).expect("disable");
    assert_eq!(repo.load().expect("load disabled").enabled_at, None);
}
```

- [x] **Step 2: 运行 RED**

Run:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml -p hmm-infra --test save_backup_background_settings_repository
```

Expected: FAIL，repository/module 尚不存在。

- [x] **Step 3: 增加 migration 008**

```sql
CREATE TABLE save_backup_background_settings (
    singleton_id INTEGER PRIMARY KEY CHECK (singleton_id = 1),
    desired_enabled INTEGER NOT NULL CHECK (desired_enabled IN (0, 1)),
    enabled_at INTEGER NULL,
    last_worker_heartbeat_at INTEGER NULL,
    updated_at INTEGER NOT NULL,
    CHECK (
        (desired_enabled = 0 AND enabled_at IS NULL)
        OR
        (desired_enabled = 1 AND enabled_at IS NOT NULL)
    )
);
```

将 migration 追加到 `migrations.rs`。新增 migration test：先迁移到 version 7、写一条既有
scheduler row，再迁移 latest，断言旧行仍存在且新表无 singleton 行。

- [x] **Step 4: 实现 repository**

实现 `SqliteSaveBackupBackgroundSettingsRepository`，复用
`Arc<Mutex<rusqlite::Connection>>`。关键 SQL：

```sql
INSERT INTO save_backup_background_settings (
    singleton_id, desired_enabled, enabled_at,
    last_worker_heartbeat_at, updated_at
) VALUES (1, 1, ?1, NULL, ?1)
ON CONFLICT(singleton_id) DO UPDATE SET
    desired_enabled = 1,
    enabled_at = excluded.enabled_at,
    last_worker_heartbeat_at = NULL,
    updated_at = excluded.updated_at;
```

`finish_disable` 写 `desired_enabled = 0`、两个 nullable 时间为 NULL；
`record_worker_heartbeat` 使用 `UPDATE ... WHERE singleton_id = 1 AND desired_enabled = 1`，
并要求 exactly one row，否则返回错误，防止未启用时伪造健康。

- [x] **Step 5: 运行 GREEN 和 migration 测试**

```powershell
cargo test --manifest-path src-tauri/Cargo.toml -p hmm-infra --test save_backup_background_settings_repository
cargo test --manifest-path src-tauri/Cargo.toml -p hmm-infra sqlite::migrations
```

Expected: repository tests 全部通过；migration 007 -> latest 保留旧数据。

- [x] **Step 6: 提交**

```powershell
git add src-tauri/crates/hmm-infra/src/sqlite/migrations/008_save_backup_background_settings.sql src-tauri/crates/hmm-infra/src/sqlite/save_backup_background_settings_repository.rs src-tauri/crates/hmm-infra/tests/save_backup_background_settings_repository.rs src-tauri/crates/hmm-infra/src/sqlite/migrations.rs src-tauri/crates/hmm-infra/src/sqlite/mod.rs src-tauri/crates/hmm-infra/src/lib.rs
git commit -m "feat: persist background protection settings"
```

---

### Task 3: 重构 Background Service 的全局启停与健康派生

**Files:**
- Modify: `src-tauri/crates/hmm-app/src/save_backup_background.rs`
- Modify: `src-tauri/crates/hmm-app/src/lib.rs`
- Modify: `src-tauri/crates/hmm-app/tests/save_backup_background.rs`

- [x] **Step 1: 用新 Harness 写状态矩阵 RED**

将 Harness 改为注入 fake global settings repository。新增固定边界：

```rust
#[test]
fn exact_registration_waits_for_current_enable_heartbeat() {
    let now = 1_300_000;
    let settings = SaveBackupBackgroundSettings {
        desired_enabled: true,
        enabled_at: Some(1_000_000),
        last_worker_heartbeat_at: None,
        updated_at: 1_000_000,
    };
    let harness = Harness::with_global_settings(now, settings);
    assert_eq!(harness.service.control_status().unwrap().status,
        SaveBackupBackgroundProtectionStatus::Starting);

    harness.clock.set(1_300_001);
    assert_eq!(harness.service.control_status().unwrap().status,
        SaveBackupBackgroundProtectionStatus::WorkerUnhealthy);
}
```

另覆盖 heartbeat == enabled_at、45 分钟 TTL 边界、future、旧启用 heartbeat、drift、
permission、unsupported 和 repository/clock failures。

- [x] **Step 2: 运行 RED**

```powershell
cargo test --manifest-path src-tauri/Cargo.toml -p hmm-app --test save_backup_background
```

Expected: FAIL，constructor 和 `control_status` 仍使用 per-profile scheduler state。

- [x] **Step 3: 定义控制结果与 5 分钟常量**

```rust
pub const SAVE_BACKUP_BACKGROUND_STARTUP_GRACE_MILLIS: u128 = 5 * 60_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaveBackupBackgroundControlStatus {
    pub desired_enabled: bool,
    pub status: SaveBackupBackgroundProtectionStatus,
    pub enabled_at: Option<u128>,
    pub last_heartbeat_at: Option<u128>,
    pub last_error_code: Option<String>,
}
```

`SaveBackupBackgroundService` 改为依赖
`Arc<dyn SaveBackupBackgroundSettingsRepository>`，`control_status()` 按批准规格的顺序派生。
现有 per-profile `status(game_id, profile_id)` 先读取 scheduler enabled；manual/no state 返回
`not_enabled`，自动计划再组合 `control_status()`。

- [x] **Step 4: 写 enable/disable 顺序 RED**

Fake repositories 记录调用：

```rust
#[test]
fn enable_persists_intent_before_register_and_returns_starting() {
    let harness = Harness::enabled_operations();
    let result = harness.service.enable().expect("enable");
    assert_eq!(result.status, SaveBackupBackgroundProtectionStatus::Starting);
    assert_eq!(
        harness.calls(),
        ["settings.begin_enable", "registry.register", "registry.inspect", "audit.record"]
    );
}

#[test]
fn disable_confirms_task_missing_before_persisting_disabled() {
    let harness = Harness::disabled_operations();
    harness.service.disable().expect("disable");
    assert_eq!(
        harness.calls(),
        ["registry.unregister", "registry.inspect", "settings.finish_disable", "audit.record"]
    );
}
```

- [x] **Step 5: 实现 enable/disable**

- `enable()`：clock -> begin_enable -> register -> inspect exact -> audit -> 返回重新查询的
  `starting`。
- `disable()`：unregister -> inspect not registered -> finish_disable -> audit -> not enabled。
- ownership/permission/timeout/read-back 失败时不得写 disabled。
- 保留 desired true 的失败必须能由 `control_status()` 解释并允许重试。

- [x] **Step 6: 运行 GREEN**

```powershell
cargo test --manifest-path src-tauri/Cargo.toml -p hmm-app --test save_backup_background
```

Expected: 全部通过；测试输出包含 starting、partial failure 和调用顺序 cases。

- [x] **Step 7: 提交**

```powershell
git add src-tauri/crates/hmm-app/src/save_backup_background.rs src-tauri/crates/hmm-app/src/lib.rs src-tauri/crates/hmm-app/tests/save_backup_background.rs
git commit -m "feat: orchestrate background protection control"
```

---

### Task 4: Worker 写入全局 Heartbeat

**Files:**
- Modify: `src-tauri/crates/hmm-app/src/save_backup_background_worker.rs`
- Modify: `src-tauri/crates/hmm-app/tests/save_backup_background_worker.rs`

- [x] **Step 1: 写 RED 测试**

将 worker Harness 注入 fake `SaveBackupBackgroundSettingsRepository`：

```rust
#[test]
fn worker_records_one_global_heartbeat_after_completed_cycle() {
    let harness = Harness::new();
    harness.background_settings.enable_at(NOW - 1);
    harness.insert_profile("manual");
    harness.insert_settings(manual_settings("manual"));

    harness.worker().run_once("worker-a").expect("worker runs");

    assert_eq!(harness.background_settings.heartbeats(), vec![NOW]);
}

#[test]
fn profile_list_failure_does_not_record_global_heartbeat() {
    let harness = Harness::new();
    harness.background_settings.enable_at(NOW - 1);
    harness.profile_repository.fail_list();

    harness.worker().run_once("worker-a").expect_err("infrastructure failure");

    assert!(harness.background_settings.heartbeats().is_empty());
}

#[test]
fn disabled_background_intent_makes_worker_a_noop() {
    let harness = Harness::new();
    harness.insert_profile("default");
    harness.insert_settings(daily_settings("default"));

    let summary = harness.worker().run_once("worker-a").expect("disabled no-op");

    assert_eq!(summary.checked_profiles, 0);
    assert!(harness.executor.triggers().is_empty());
    assert!(harness.background_settings.heartbeats().is_empty());
}
```

保留既有 per-profile heartbeat 测试，确保迁移期间不回退 P7.2a 行为。

- [x] **Step 2: 运行 RED**

```powershell
cargo test --manifest-path src-tauri/Cargo.toml -p hmm-app --test save_backup_background_worker
```

Expected: FAIL，worker constructor 尚无 global repository。

- [x] **Step 3: 实现最小写入**

worker 首先 load global settings。`desired_enabled = false` 时立即返回空 summary，不枚举
Profile、不启动任务、不写 heartbeat。enabled 时才执行既有 loop，并在成功完成 profile
枚举和逐 profile 检查后、返回 summary 前执行：

```rust
self.background_settings_repository
    .record_worker_heartbeat(now)
    .map_err(|_| SaveBackupBackgroundWorkerError::HeartbeatUnavailable)?;
```

业务 skip（manual、game running、unknown、source invalid）仍算完成；profile list/clock/global
heartbeat repository failure返回稳定 worker error，不写伪健康。

- [x] **Step 4: 运行 GREEN**

```powershell
cargo test --manifest-path src-tauri/Cargo.toml -p hmm-app --test save_backup_background_worker
cargo test --manifest-path src-tauri/Cargo.toml -p hmm-app --test save_backup_scheduler
```

Expected: 两组通过。

- [x] **Step 5: 提交**

```powershell
git add src-tauri/crates/hmm-app/src/save_backup_background_worker.rs src-tauri/crates/hmm-app/tests/save_backup_background_worker.rs
git commit -m "feat: record global background worker heartbeat"
```

---

### Task 5: 增加应用级退出 Guard

**Files:**
- Create: `src-tauri/crates/hmm-app/src/save_backup_exit_guard.rs`
- Create: `src-tauri/crates/hmm-app/tests/save_backup_exit_guard.rs`
- Modify: `src-tauri/crates/hmm-app/src/lib.rs`

- [x] **Step 1: 写退出决策 RED**

定义期望值并覆盖无自动计划、protected、starting、查询失败：

```rust
#[test]
fn exit_requires_confirmation_when_auto_profile_is_not_protected() {
    let harness = Harness::new(SaveBackupBackgroundProtectionStatus::Starting);
    harness.insert_auto_profile("default");

    let decision = harness.guard.evaluate().expect("decision");

    assert_eq!(decision, SaveBackupExitDecision::ConfirmationRequired {
        reason: SaveBackupExitReason::BackgroundStarting,
    });
}

#[test]
fn no_auto_profile_can_exit_without_background_protection() {
    let harness = Harness::new(SaveBackupBackgroundProtectionStatus::WorkerUnhealthy);
    harness.insert_manual_profile("default");
    assert_eq!(harness.guard.evaluate().unwrap(), SaveBackupExitDecision::Safe);
}
```

- [x] **Step 2: 运行 RED**

```powershell
cargo test --manifest-path src-tauri/Cargo.toml -p hmm-app --test save_backup_exit_guard
```

Expected: FAIL，module/types 尚不存在。

- [x] **Step 3: 实现 guard**

定义稳定 enum：

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaveBackupExitReason {
    BackgroundStarting,
    BackgroundNotEnabled,
    RegistrationFailed,
    WorkerUnhealthy,
    PermissionRequired,
    UnsupportedPlatform,
    StatusUnavailable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SaveBackupExitDecision {
    Safe,
    ConfirmationRequired { reason: SaveBackupExitReason },
}
```

`SaveBackupExitGuard` 依赖 `ProfileRepository`、`ProfileSaveSettingsRepository`、
`SaveBackupBackgroundService`、`AuditLogWriter` 和 clock。`evaluate()`：

1. `list_all()`。
2. 逐 profile 读取 settings；任一读取失败返回 confirmation required/status unavailable。
3. 没有 cadence 非 manual -> safe。
4. control status == protected -> safe，其余映射 reason。

`record_override(reason)` 只写 `protection_status` 与 `error_code`；audit failure 返回错误给
Tauri 层记录 sanitized warning，但不得成为永久无法退出的条件。

- [x] **Step 4: 运行 GREEN**

```powershell
cargo test --manifest-path src-tauri/Cargo.toml -p hmm-app --test save_backup_exit_guard
```

Expected: safe/confirmation/failure/审计字段白名单全部通过。

- [x] **Step 5: 提交**

```powershell
git add src-tauri/crates/hmm-app/src/save_backup_exit_guard.rs src-tauri/crates/hmm-app/tests/save_backup_exit_guard.rs src-tauri/crates/hmm-app/src/lib.rs
git commit -m "feat: add save backup exit guard"
```

---

### Task 6: 装配 AppState 并暴露后台控制 Tauri 契约

**Files:**
- Modify: `src-tauri/src/state.rs`
- Modify: `src-tauri/src/save_backup_dto.rs`
- Modify: `src-tauri/src/save_backup_commands.rs`
- Modify: `src-tauri/src/lib.rs`

- [x] **Step 1: 写 DTO 与 command mapping RED**

在 `save_backup_dto.rs` tests 增加：

```rust
#[test]
fn background_control_dto_exposes_only_whitelisted_fields() {
    let dto = SaveBackupBackgroundControlStatusDto::from(
        hmm_app::SaveBackupBackgroundControlStatus {
            desired_enabled: true,
            status: SaveBackupBackgroundProtectionStatus::Starting,
            enabled_at: Some(100),
            last_heartbeat_at: None,
            last_error_code: None,
        },
    );
    let value = serde_json::to_value(dto).expect("serialize");
    assert_eq!(value["status"], "starting");
    assert_eq!(value["desiredEnabled"], true);
    for forbidden in ["taskName", "sid", "workerPath", "leaseOwner", "workerInstanceId"] {
        assert!(value.get(forbidden).is_none());
    }
}
```

在 `save_backup_commands.rs` tests 断言 service error 只映射稳定 code，不携带原始 details。

- [x] **Step 2: 运行 RED**

```powershell
cargo test --manifest-path src-tauri/Cargo.toml -p hmm-tauri save_backup
```

Expected: FAIL，control DTO/commands 尚不存在。

- [x] **Step 3: 装配 SQLite repository**

在 `AppState::from_app_data_dir_with_startup` 创建一个共享
`SqliteSaveBackupBackgroundSettingsRepository`，分别转为 port trait object 并注入：

```rust
let save_backup_background_settings_repository =
    Arc::new(SqliteSaveBackupBackgroundSettingsRepository::new(Arc::clone(&db)));
let settings_for_service: Arc<dyn SaveBackupBackgroundSettingsRepository> =
    save_backup_background_settings_repository.clone();
let settings_for_worker: Arc<dyn SaveBackupBackgroundSettingsRepository> =
    save_backup_background_settings_repository;
```

用新 constructor 装配 `SaveBackupBackgroundService` 和 `SaveBackupBackgroundWorker`。
向 `AppState` 增加 `save_backup_exit_guard: Arc<SaveBackupExitGuard>`，复用 profile/settings、
background service、audit 和 clock。

- [x] **Step 4: 增加白名单 DTO**

```rust
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveBackupBackgroundControlStatusDto {
    pub desired_enabled: bool,
    pub status: SaveBackupBackgroundStatusKindDto,
    pub enabled_at: Option<u64>,
    pub last_heartbeat_at: Option<u64>,
    pub last_error_code: Option<String>,
}
```

给 `SaveBackupBackgroundStatusKindDto` 增加 `Starting`。时间转换沿用现有 u128 -> u64
白名单映射，不包含 task/SID/path/lease/worker id。

- [x] **Step 5: 增加三个 async blocking commands**

```rust
#[tauri::command]
pub async fn get_save_backup_background_control_status(
    state: State<'_, AppState>,
) -> Result<SaveBackupBackgroundControlStatusDto, CommandErrorDto> {
    run_background_control(state, |service| service.control_status()).await
}

#[tauri::command]
pub async fn enable_save_backup_background_protection(
    state: State<'_, AppState>,
) -> Result<SaveBackupBackgroundControlStatusDto, CommandErrorDto> {
    run_background_control(state, |service| service.enable()).await
}

#[tauri::command]
pub async fn disable_save_backup_background_protection(
    state: State<'_, AppState>,
) -> Result<SaveBackupBackgroundControlStatusDto, CommandErrorDto> {
    run_background_control(state, |service| service.disable()).await
}
```

helper 使用 `spawn_blocking`，join failure 返回
`save_backup_background_status_unavailable`。在 `src-tauri/src/lib.rs` 注册三个 command。

- [x] **Step 6: 运行 GREEN 与 composition tests**

```powershell
cargo test --manifest-path src-tauri/Cargo.toml -p hmm-tauri save_backup
cargo test --manifest-path src-tauri/Cargo.toml -p hmm-tauri state::tests
```

Expected: DTO、commands、GUI/headless composition tests 全部通过。

- [x] **Step 7: 提交**

```powershell
git add src-tauri/src/state.rs src-tauri/src/save_backup_dto.rs src-tauri/src/save_backup_commands.rs src-tauri/src/lib.rs
git commit -m "feat: expose background protection controls"
```

---

### Task 7: 后端强制所有退出入口经过 Guard

**Files:**
- Modify: `src-tauri/src/window_lifecycle_commands.rs`
- Modify: `src-tauri/src/lib.rs`

- [x] **Step 1: 写 request 和菜单行为 RED**

在 Rust tests 增加：

```rust
#[test]
fn exit_request_deserializes_explicit_override_flag() {
    let request: ExitAppRequestDto =
        serde_json::from_value(serde_json::json!({ "overrideUnprotected": false }))
            .expect("deserialize");
    assert!(!request.override_unprotected);
}

#[test]
fn exit_guard_dto_serializes_stable_reason_without_raw_details() {
    let dto = AppExitGuardDto::confirmation_required(
        SaveBackupExitReason::BackgroundStarting,
    );
    let value = serde_json::to_value(dto).expect("serialize");
    assert_eq!(value["decision"], "confirmation_required");
    assert_eq!(value["reason"], "background_starting");
}

#[test]
fn tray_exit_uses_the_same_window_close_event() {
    assert_eq!(TRAY_EXIT_REQUEST_EVENT, WINDOW_CLOSE_REQUESTED_EVENT);
}
```

把 exit 决策抽为可单测 helper，覆盖 safe、unsafe ordinary、unsafe override 和 guard error。

- [x] **Step 2: 运行 RED**

```powershell
cargo test --manifest-path src-tauri/Cargo.toml -p hmm-tauri window_lifecycle
```

Expected: FAIL，request/guard helper 尚不存在。

- [x] **Step 3: 实现显式 request 与后端重检**

```rust
#[derive(Debug, Clone, Copy, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExitAppRequestDto {
    pub override_unprotected: bool,
}
```

`exit_app` 注入 `State<'_, AppState>`。普通 unsafe 返回：

```rust
Err(CommandErrorDto {
    code: "exit_confirmation_required".to_owned(),
    message: "exit requires confirmation".to_owned(),
})
```

override true 时再次 evaluate；仍 unsafe 则 best-effort `record_override`，audit failure 只写
sanitized `tracing::warn!("background exit override audit unavailable")`，随后 `app.exit(0)`。
不把 raw error/path 写入 message 或 log。

新增 `get_app_exit_guard` 只读 command，返回：

```rust
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppExitGuardDto {
    pub decision: AppExitDecisionDto,
    pub reason: Option<AppExitReasonDto>,
}
```

decision/reason 使用 snake_case enum。前端不得解析 `exit_app` 的 message；race 导致 generic
`exit_confirmation_required` 时重新查询该 DTO。

- [x] **Step 4: 移除托盘直接退出旁路**

`MENU_EXIT_ID` 分支执行：

```rust
show_main_window(app);
if let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) {
    let _ = window.emit(WINDOW_CLOSE_REQUESTED_EVENT, ());
}
```

不得保留任何 `MENU_EXIT_ID => app.exit(0)`。

- [x] **Step 5: 运行 GREEN**

```powershell
cargo test --manifest-path src-tauri/Cargo.toml -p hmm-tauri window_lifecycle
```

Expected: 普通 unsafe 被拒绝；override、safe、tray event tests 通过。

- [x] **Step 6: 提交**

```powershell
git add src-tauri/src/window_lifecycle_commands.rs src-tauri/src/lib.rs
git commit -m "feat: guard application exit"
```

---

### Task 8: Settings 唯一正式后台保护开关

**Files:**
- Create: `src/features/settings/backgroundProtectionTypes.ts`
- Create: `src/features/settings/backgroundProtectionApi.ts`
- Create: `src/features/settings/BackgroundProtectionPanel.tsx`
- Create: `src/features/settings/backgroundProtectionApi.test.mjs`
- Create: `src/features/settings/backgroundProtectionPanel.test.mjs`
- Modify: `src/features/settings/SettingsPage.tsx`
- Modify: `src/features/settings/SettingsPage.css`

- [x] **Step 1: 写 typed API RED**

```javascript
test("background protection API uses only global narrow commands", () => {
  const source = readProjectFile("src/features/settings/backgroundProtectionApi.ts");
  assert.match(source, /get_save_backup_background_control_status/);
  assert.match(source, /enable_save_backup_background_protection/);
  assert.match(source, /disable_save_backup_background_protection/);
  assert.doesNotMatch(source, /taskName|workerPath|PowerShell|sid/);
});
```

- [x] **Step 2: 运行 RED**

```powershell
node --test src/features/settings/backgroundProtectionApi.test.mjs
```

Expected: FAIL，API file 不存在。

- [x] **Step 3: 实现 types 与 API**

```ts
export type BackgroundProtectionStatus =
  | "not_enabled"
  | "starting"
  | "protected"
  | "registration_failed"
  | "worker_unhealthy"
  | "permission_required"
  | "unsupported_platform";

export type BackgroundProtectionControlDto = {
  desiredEnabled: boolean;
  status: BackgroundProtectionStatus;
  enabledAt: number | null;
  lastHeartbeatAt: number | null;
  lastErrorCode: string | null;
};
```

`backgroundProtectionApi.ts` 使用 feature-local `invoke`，三个函数均无参数。

- [x] **Step 4: 写面板行为 RED**

source tests 至少锁定：

```javascript
assert.match(source, /role="status"/);
assert.match(source, /aria-live="polite"/);
assert.match(source, /status === "starting"/);
assert.match(source, /status === "unsupported_platform"/);
assert.match(source, /onChange/);
assert.match(source, /disabled=\{busy/);
```

另用纯 helper test 覆盖 status -> label/tone/action 映射。

- [x] **Step 5: 实现 BackgroundProtectionPanel**

组件自行 load/refresh，toggle 操作期间保持稳定尺寸并禁用重复操作：

```tsx
<label className="setting-row">
  <span className="setting-row__copy">
    <strong>退出后继续保护自动备份</strong>
    <span>{copy.description}</span>
  </span>
  <input
    type="checkbox"
    checked={state.desiredEnabled}
    disabled={busy || state.status === "unsupported_platform"}
    onChange={() => void changeProtection(!state.desiredEnabled)}
  />
  <span className="setting-switch" aria-hidden="true" />
</label>
```

`starting` 文案必须是“正在验证后台保护”，`protected` 才能写“已保护”。失败显示重试或停用；
不能显示 raw error message。

- [x] **Step 6: 接入 Settings 并修正文案**

在“存档备份”section 顶部渲染 `BackgroundProtectionPanel`。将 hero 总括文案改为：

> 后台保护与窗口关闭偏好会正式保存；其余标记为预览的选项只在当前会话中生效。

不把全局开关加入 session-preview `SettingsState` 或 reset preview。

- [x] **Step 7: 运行 GREEN**

```powershell
node --test src/features/settings/backgroundProtectionApi.test.mjs src/features/settings/backgroundProtectionPanel.test.mjs
cmd /c corepack pnpm run typecheck
cmd /c corepack pnpm run lint
```

Expected: tests、typecheck、lint 通过。

- [x] **Step 8: 提交**

```powershell
git add src/features/settings/backgroundProtectionTypes.ts src/features/settings/backgroundProtectionApi.ts src/features/settings/BackgroundProtectionPanel.tsx src/features/settings/backgroundProtectionApi.test.mjs src/features/settings/backgroundProtectionPanel.test.mjs src/features/settings/SettingsPage.tsx src/features/settings/SettingsPage.css
git commit -m "feat: add background protection settings"
```

---

### Task 9: Profile 只读显示全局保护

**Files:**
- Modify: `src/features/profiles/profileSaveBackupTypes.ts`
- Modify: `src/features/profiles/ProfilePage.tsx`
- Modify: `src/features/profiles/ProfilePage.css`
- Modify: `src/features/profiles/profileFrontendIntegration.test.mjs`

- [x] **Step 1: 写 RED source/view-model tests**

```javascript
test("profile background status supports starting without an enable toggle", () => {
  const types = readProjectFile("src/features/profiles/profileSaveBackupTypes.ts");
  const page = readProjectFile("src/features/profiles/ProfilePage.tsx");
  assert.match(types, /"starting"/);
  assert.match(page, /正在验证后台保护/);
  assert.doesNotMatch(page, /enable_save_backup_background_protection/);
  assert.doesNotMatch(page, /disable_save_backup_background_protection/);
});
```

- [x] **Step 2: 运行 RED**

```powershell
node --test src/features/profiles/profileFrontendIntegration.test.mjs
```

Expected: FAIL，`starting` 和动态 badge 尚未实现。

- [x] **Step 3: 更新只读状态**

- 在 union 增加 `"starting"`。
- `getBackgroundProtectionCopy` 增加 starting 文案。
- badge 按当前状态派生：protected -> “退出后受保护”；starting -> “等待后台验证”；其他
  自动计划 -> “仅客户端运行时”；manual -> “未启用自动备份”。
- failure hint 提供 Settings 导航，不提供本地 toggle。
- preview fixture 使用 `starting` 或 `tray_only`，不能伪造 protected heartbeat。

- [x] **Step 4: 运行 GREEN**

```powershell
node --test src/features/profiles/profileFrontendIntegration.test.mjs src/features/profiles/profileApi.test.mjs
cmd /c corepack pnpm run typecheck
```

Expected: tests/typecheck 通过。

- [x] **Step 5: 提交**

```powershell
git add src/features/profiles/profileSaveBackupTypes.ts src/features/profiles/ProfilePage.tsx src/features/profiles/ProfilePage.css src/features/profiles/profileFrontendIntegration.test.mjs
git commit -m "feat: show profile background protection"
```

---

### Task 10: 前端统一普通退出与危险 Override

**Files:**
- Modify: `src/app/window-lifecycle/windowLifecycleApi.ts`
- Modify: `src/app/window-lifecycle/windowLifecycleError.ts`
- Modify: `src/app/window-lifecycle/useWindowCloseRequest.ts`
- Modify: `src/app/window-lifecycle/WindowCloseDialogHost.tsx`
- Modify: `src/app/window-lifecycle/WindowCloseDialog.tsx`
- Modify: `src/app/window-lifecycle/WindowCloseDialog.css`
- Modify: `src/app/window-lifecycle/windowLifecycleUi.test.mjs`
- Modify: `src/app/window-lifecycle/windowClosePreference.test.mjs`

- [x] **Step 1: 写 API/偏好 RED**

```javascript
test("ordinary and override exits use explicit flags", () => {
  const api = readProjectFile("src/app/window-lifecycle/windowLifecycleApi.ts");
  assert.match(api, /exitApplication\(overrideUnprotected = false\)/);
  assert.match(api, /overrideUnprotected/);
  assert.match(api, /get_app_exit_guard/);
});

test("unsafe dialog cannot persist exit preference", () => {
  const dialog = readProjectFile("src/app/window-lifecycle/WindowCloseDialog.tsx");
  assert.match(dialog, /mode === "unsafe"/);
  assert.match(dialog, /mode === "normal".*remember/s);
});
```

- [x] **Step 2: 运行 RED**

```powershell
node --test src/app/window-lifecycle/windowLifecycleUi.test.mjs src/app/window-lifecycle/windowClosePreference.test.mjs
```

Expected: FAIL，API 无 request，dialog 无 unsafe mode。

- [x] **Step 3: 更新 API 与普通关闭 hook**

```ts
export function exitApplication(overrideUnprotected = false): Promise<void> {
  return invoke<void>("exit_app", {
    request: { overrideUnprotected },
  });
}
```

增加 `getAppExitGuard(): Promise<AppExitGuardDto>`。`useWindowCloseRequest` 对 remembered
exit 先读 guard；safe 才调用 `exitApplication(false)`，confirmation required 直接用结构化
reason 打开 unsafe dialog。`exitApplication(false)` 若因 race 返回 generic
`exit_confirmation_required`，重新查询 guard，不解析 error message。

- [x] **Step 4: 扩展 Dialog state**

定义：

```ts
export type AppExitGuardReason =
  | "background_starting"
  | "background_not_enabled"
  | "registration_failed"
  | "worker_unhealthy"
  | "permission_required"
  | "unsupported_platform"
  | "status_unavailable";

export type WindowCloseDialogMode =
  | { kind: "normal" }
  | { kind: "unsafe"; reason: AppExitGuardReason };
```

unsafe mode：

- 标题“后台保护尚未就绪”。
- 主操作/初始焦点为“留在托盘”。
- “仍然退出”调用 `exitApplication(true)`。
- 不渲染 remember checkbox。
- Escape/overlay close 只取消。
- starting 文案明确任务仍会在约 1 分钟后尝试运行，但尚未验证。

normal mode 保留现有 ask/tray/exit 和 remember 行为。

- [x] **Step 5: 映射稳定错误码**

`windowLifecycleError.ts` 只解析：

- `exit_confirmation_required`
- `window_hide_failed`

危险原因文案只映射 `AppExitGuardDto.reason`。不得把 raw backend message 直接展示。
override audit failure 不会由后端阻止退出。

- [x] **Step 6: 运行 GREEN 与 frontend build**

```powershell
node --test src/app/window-lifecycle/windowLifecycleUi.test.mjs src/app/window-lifecycle/windowClosePreference.test.mjs
cmd /c corepack pnpm run typecheck
cmd /c corepack pnpm run lint
cmd /c corepack pnpm run build
```

Expected: tests/typecheck/lint/build 全部通过。

- [ ] **Step 7: Browser/manual visual smoke**

执行记录：已用受控 browser harness 检查 normal、`starting`、`worker_unhealthy` 在 `1440x900`、`1366x768`、`1280x800`、`960x640` 的布局、默认 tray 焦点、unsafe no-remember、Escape/关闭按钮和 console；浏览器控制层未触发 Tab/Shift+Tab 原生默认焦点移动，也未在真实 WebView 手工复验，因此本项保持未勾选。

在 `1440x900`、`1366x768`、`1280x800` 和最小窗口 `960x640` 检查：

- normal dialog。
- starting unsafe dialog。
- worker unhealthy unsafe dialog。
- 键盘 Tab/Shift+Tab/Escape。
- 文本不重叠，主操作默认为 tray，unsafe 不出现 remember。

记录截图或人工结果；未运行时在最终交接明确原因。

- [x] **Step 8: 提交**

```powershell
git add src/app/window-lifecycle/windowLifecycleApi.ts src/app/window-lifecycle/windowLifecycleError.ts src/app/window-lifecycle/useWindowCloseRequest.ts src/app/window-lifecycle/WindowCloseDialogHost.tsx src/app/window-lifecycle/WindowCloseDialog.tsx src/app/window-lifecycle/WindowCloseDialog.css src/app/window-lifecycle/windowLifecycleUi.test.mjs src/app/window-lifecycle/windowClosePreference.test.mjs
git commit -m "feat: warn before unprotected exit"
```

---

### Task 11: 同步契约、门禁与最终复审

**Files:**
- Modify: `docs/SAVE_BACKUP_BACKGROUND_AUTOMATION_DESIGN.md`
- Modify: `docs/FRONTEND_BACKEND_CONTRACT.md`
- Modify: `docs/TESTING.md`
- Modify: `docs/LOGGING.md`
- Modify: `docs/release/发布与产物说明.md`
- Modify: `TODO.md`
- Modify: `docs/superpowers/plans/2026-07-11-save-backup-background-user-flow-implementation.md`

- [x] **Step 1: 同步正式文档**

必须写明：

- Settings 全局开关与 Profile 只读边界。
- global SQLite desired/enable time/heartbeat。
- `starting` 5 分钟与 `protected` 45 分钟 TTL。
- 三个控制 commands、`get_app_exit_guard`、`AppExitGuardDto` 和 `ExitAppRequestDto`。
- `background_exit_override` 审计字段白名单。
- 普通自动测试禁止真实 Scheduled Task。
- 安装态 runtime acceptance 未完成。
- P7.2c NSIS/WiX cleanup 仍未实现。

- [x] **Step 2: 更新 TODO 状态但不提前宣称完成**

将 P7.2b 项标记为已实现仅在所有代码/测试任务完成后；仍保留：

```markdown
- [ ] P7.2a Windows 安装态 runtime acceptance
- [ ] P7.2c NSIS/WiX owned Scheduled Task 自动卸载 cleanup
```

T8 总状态必须继续是进行中。

- [x] **Step 3: 运行全部聚焦测试**

```powershell
cargo test --manifest-path src-tauri/Cargo.toml -p hmm-core background
cargo test --manifest-path src-tauri/Cargo.toml -p hmm-ports background
cargo test --manifest-path src-tauri/Cargo.toml -p hmm-infra --test save_backup_background_settings_repository
cargo test --manifest-path src-tauri/Cargo.toml -p hmm-infra --test save_backup_scheduler_repository
cargo test --manifest-path src-tauri/Cargo.toml -p hmm-app --test save_backup_background
cargo test --manifest-path src-tauri/Cargo.toml -p hmm-app --test save_backup_background_worker
cargo test --manifest-path src-tauri/Cargo.toml -p hmm-app --test save_backup_exit_guard
cargo test --manifest-path src-tauri/Cargo.toml -p hmm-tauri save_backup
cargo test --manifest-path src-tauri/Cargo.toml -p hmm-tauri window_lifecycle
node --test src/features/settings/backgroundProtectionApi.test.mjs src/features/settings/backgroundProtectionPanel.test.mjs
node --test src/features/profiles/profileFrontendIntegration.test.mjs src/features/profiles/profileApi.test.mjs
node --test src/app/window-lifecycle/windowLifecycleUi.test.mjs src/app/window-lifecycle/windowClosePreference.test.mjs
```

Expected: 全部 exit 0；真实 task smoke 保持 ignored/未运行。

- [x] **Step 4: 运行完整验证**

先确保 dev sidecar 存在：

```powershell
cmd /c corepack pnpm run prepare:save-backup-worker-sidecar:dev
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify.ps1
```

Expected: `Verification passed.`。

- [x] **Step 5: 本地 review gate**

```powershell
git status --short --branch
git diff --check
git diff origin/main...HEAD --name-status
git diff origin/main...HEAD --stat
git ls-files --others --exclude-standard
```

逐项确认：

- 无 `.planning/`、sidecar exe、`target/`、`dist/` 或真实数据进入 diff。
- 无 raw path/SID/task/PowerShell/lease/worker id 进入 DTO、UI 或 audit。
- Settings 是唯一开关，Profile 无 enable/disable。
- tray menu 不直接 exit。
- unsafe override 不写 localStorage preference。
- P7.2c installer 文件没有变化。

- [x] **Step 6: 更新计划执行状态并提交文档**

只把实际完成的 checkbox 从 `[ ]` 改为 `[x]`。未执行的 Windows/manual/visual gate 保持未勾选并写明原因。

```powershell
git add docs/SAVE_BACKUP_BACKGROUND_AUTOMATION_DESIGN.md docs/FRONTEND_BACKEND_CONTRACT.md docs/TESTING.md docs/LOGGING.md docs/release/发布与产物说明.md TODO.md docs/superpowers/plans/2026-07-11-save-backup-background-user-flow-implementation.md
git commit -m "docs: record P7.2b background protection gate"
```

## 2. 交付声明边界

完成本计划且自动化验证通过后，可以声明：

- P7.2b 应用级后台保护启停与状态 UI 已实现。
- 所有 GUI 真正退出入口具有 fail-closed 警告和单次 override。
- 自动化覆盖 SQLite、app service、worker、Tauri contract 和 frontend state。

仍不能声明：

- 真实 Windows Scheduled Task runtime acceptance 已通过。
- 安装 bundle 中 sibling worker 已验收。
- NSIS/WiX 自动卸载 cleanup 已实现或通过。
- Linux / Steam Deck 退出后后台保护可用。
