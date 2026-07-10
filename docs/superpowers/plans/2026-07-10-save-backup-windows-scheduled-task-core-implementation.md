# P7.2a Windows Scheduled Task 后台保护核心 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在不新增前端启用入口、不绕过既有存档备份安全链路的前提下，交付 Windows 用户级 Scheduled Task 的受控注册、幂等更新/read-back/移除、独立 worker heartbeat、双条件 `protected` 判定和可打包 worker sidecar。

**Architecture:** `hmm-core` / SQLite 先把 scheduler check 与 worker heartbeat 拆成独立事实；`hmm-app` 用 registry port、repository 和 clock 派生保护状态；`hmm-infra` 以纯 task spec + 固定 PowerShell ScheduledTasks runner 实现 Windows adapter；Tauri 只负责平台装配和 DTO 映射。Scheduled Task 只启动 sibling worker 的固定 `--once`，真正备份继续走 `SaveBackupTaskRunner -> SaveBackupService -> SaveBackupWriter/Repository/AuditLog`。

**Tech Stack:** Rust 2021、Tauri 2.11 配置 schema、SQLite/rusqlite、Windows PowerShell 5.1 `ScheduledTasks` module、Node.js 24、pnpm 11.1.3、现有 `sha2` / `serde` / `serde_json` / `thiserror`，新增 Windows-only `wait-timeout 0.2.1` 与 direct `windows-sys 0.61.2`（仅 `GetSystemDirectoryW`）。

## Global Constraints

- 仅支持 Windows 用户级 Scheduled Task；不得要求管理员权限，不创建 Windows Service。
- task action 只能是内部定位并 canonicalize 的 `hmm-save-backup-worker`，arguments 必须严格等于 `--once`。
- task name、current user SID、worker path、ownership marker、PowerShell script 或 task XML 不进入前端、DTO、普通日志或 Audit Log。
- `SaveBackupBackgroundRegistry` 继续使用无参数 `inspect/register/unregister`；前端、CLI 和外部配置不能提交任意命令、脚本、路径或 task XML。
- 自动化测试只能使用 fake registry/command runner、固定 clock、临时 SQLite/目录和人工 fixture；不得创建、更新、启动或删除真实系统任务。
- Scheduled Task 每 15 分钟触发，并在用户登录后延迟 1 分钟触发；worker freshness TTL 固定为 45 分钟。
- `protected` 必须同时满足：当前 Profile 已启用后台保护、task read-back 完全匹配、`worker_heartbeat_at` 位于 `[now - 45m, now]`。
- `record_worker_heartbeat` 不得写 `background_status` 或复用 scheduler `last_checked_at`。
- P7.2a 不新增 Profile/Settings 开关或退出提示；这些属于 P7.2b。
- P7.2a 不新增 NSIS/WiX uninstall hook；安装器自动清理保留为发布前独立 packaging gate。
- 实现不得新增第二套备份写入、manifest、retention、恢复、game-running detection 或 scheduler lease 逻辑。
- Windows runner 只能执行编译期固定 PowerShell script，使用 `-NoLogo -NoProfile -NonInteractive -Command`，不得使用 `ExecutionPolicy Bypass`。
- runner 必须通过 `GetSystemDirectoryW` 使用系统 PowerShell/`ScheduledTasks.psd1` 绝对路径，不通过 PATH/current directory/`PSModulePath` 搜索 executable 或 module。
- 原始 stdout/stderr、CIM exception message 和完整路径不得写日志；只映射稳定状态/error code。

---

## File Structure

本计划超过 1200 行，因为 `writing-plans` 要求把固定 PowerShell 脚本、跨 task 类型签名、
RED/GREEN 命令和安全 smoke cleanup 写成可直接执行的完整内容。它仍只覆盖一个 P7.2a
上下文；拆成多份会把 port/spec/runner/read-back 的类型契约分散，增加执行时漂移风险。
实现代码仍按下表拆成单一职责文件和八个独立 review gate。

| 文件 | 责任 |
| --- | --- |
| `src-tauri/crates/hmm-core/src/save_backup.rs` | registration drift、独立 heartbeat 字段和稳定领域类型。 |
| `src-tauri/crates/hmm-ports/src/save_backup.rs` | 保持 registry port 无输入，更新 heartbeat repository contract。 |
| `src-tauri/crates/hmm-infra/src/sqlite/migrations/007_save_backup_worker_heartbeat.sql` | 为旧 scheduler state 增加 nullable worker heartbeat。 |
| `src-tauri/crates/hmm-infra/src/sqlite/save_backup_scheduler_repository.rs` | 独立读写 `worker_heartbeat_at`，禁止 heartbeat 覆盖 scheduler/status。 |
| `src-tauri/crates/hmm-app/src/save_backup_background.rs` | 注册生命周期、健康派生、稳定错误码和最小审计。 |
| `src-tauri/crates/hmm-app/tests/save_backup_background.rs` | fake registry/repository/clock 覆盖双条件健康和 register/unregister。 |
| `src-tauri/crates/hmm-infra/src/save_backup_background_registry/mod.rs` | fallback、内部 runner contract 和 module exports。 |
| `src-tauri/crates/hmm-infra/src/save_backup_background_registry/task_spec.rs` | 每用户任务身份、固定 expected spec 和语义比较。 |
| `src-tauri/crates/hmm-infra/src/save_backup_background_registry/powershell.rs` | 固定脚本 command runner、UTF-8 JSON parser、timeout 与 Windows process flags。 |
| `src-tauri/crates/hmm-infra/src/save_backup_background_registry/scheduled_task.ps1` | 编译期嵌入的 identity/inspect/register/unregister ScheduledTasks 脚本。 |
| `src-tauri/crates/hmm-infra/src/save_backup_background_registry/registry.rs` | 平台无关的 registry 生命周期、read-back 和 fake seam。 |
| `src-tauri/crates/hmm-infra/src/save_backup_background_registry/windows.rs` | Windows-only current-exe locator 与 PowerShell production constructor。 |
| `src-tauri/crates/hmm-infra/src/save_backup_background_registry/tests.rs` | pure spec、fake runner 和生命周期测试。 |
| `src-tauri/src/state.rs` | 按平台装配 registry/background service，不把 infra 逻辑放进 command。 |
| `src-tauri/src/save_backup_commands.rs` | status query 转发到 app service。 |
| `src-tauri/src/save_backup_dto.rs` | 从派生 health 映射现有白名单 DTO。 |
| `scripts/prepare-save-backup-worker-sidecar.mjs` | 构建并复制 target-triple sidecar。 |
| `scripts/prepare-save-backup-worker-sidecar.test.mjs` | 纯函数测试 sidecar 命名和 host triple 解析。 |
| `src-tauri/tauri.windows.conf.json` / `package.json` / `.gitignore` | Windows-only externalBin、dev/release hooks 和生成物隔离。 |
| `docs/testing/windows-save-backup-scheduled-task-smoke.md` | 一次性 Windows 账户/VM 人工 smoke 与强制 cleanup。 |

---

### Task 1: 建立注册状态、每用户 Scheduled Task Spec 与语义漂移比较

**Files:**
- Delete: `src-tauri/crates/hmm-infra/src/save_backup_background_registry.rs`
- Modify: `src-tauri/crates/hmm-core/src/save_backup.rs`
- Modify: `src-tauri/crates/hmm-ports/src/save_backup.rs`
- Modify: `src-tauri/crates/hmm-ports/src/lib.rs`
- Create: `src-tauri/crates/hmm-infra/src/save_backup_background_registry/mod.rs`
- Create: `src-tauri/crates/hmm-infra/src/save_backup_background_registry/task_spec.rs`
- Create: `src-tauri/crates/hmm-infra/src/save_backup_background_registry/tests.rs`
- Modify: `src-tauri/crates/hmm-infra/src/lib.rs`
- Modify: `src-tauri/crates/hmm-infra/tests/save_backup_background_registry.rs`

**Interfaces:**
- Produces: `SaveBackupBackgroundRegistrationStatus::ConfigurationDrift` / `"configuration_drift"`。
- Produces: `SaveBackupBackgroundRegistryError::{TaskOwnershipConflict, WorkerBinaryUnavailable, CommandTimeout, CommandInvalidOutput, OperationFailed}` with stable `code()`。
- Produces: ports constant `SAVE_BACKUP_BACKGROUND_REGISTRY_SCHEMA_VERSION: u32 = 1`，供 infra JSON parser 与 app audit 共用。
- Produces: constants `TASK_PATH`, `TASK_OWNER_MARKER`, `TASK_ARGUMENTS`, `LOGON_DELAY`, `PERIODIC_INTERVAL`, `EXECUTION_TIME_LIMIT`。
- Produces: `ScheduledTaskSpec::new(user_sid, canonical_worker_path)`。
- Produces: `ScheduledTaskReadback` 和 `ScheduledTaskSpecMatch::{Exact, OwnedDrift, OwnershipConflict}`。
- Preserves: public `UnsupportedSaveBackupBackgroundRegistry` fallback 行为。

- [ ] **Step 1: 写任务 identity 和逐字段漂移 RED 测试**

先在 core stable-code test 加入：
```rust
assert_eq!(
    SaveBackupBackgroundRegistrationStatus::ConfigurationDrift.as_str(),
    "configuration_drift"
);
```
在 ports test 加入稳定 error code 断言：
```rust
#[test]
fn background_registry_errors_have_stable_codes() {
assert_eq!(SaveBackupBackgroundRegistryError::TaskOwnershipConflict.code(), "save_backup_background_task_ownership_conflict");
assert_eq!(SaveBackupBackgroundRegistryError::WorkerBinaryUnavailable.code(), "save_backup_background_worker_binary_unavailable");
assert_eq!(SaveBackupBackgroundRegistryError::CommandTimeout.code(), "save_backup_background_command_timeout");
assert_eq!(SaveBackupBackgroundRegistryError::CommandInvalidOutput.code(), "save_backup_background_command_invalid_output");
assert_eq!(SaveBackupBackgroundRegistryError::OperationFailed.code(), "save_backup_background_registration_failed");
assert_eq!(SAVE_BACKUP_BACKGROUND_REGISTRY_SCHEMA_VERSION, 1);
}
```
再在 module unit tests 中加入：
```rust
#[test]
fn task_name_is_stable_per_sid_without_exposing_the_sid() {
    let path = std::env::temp_dir().join("hmm-save-backup-worker.exe");
    let first = ScheduledTaskSpec::new("S-1-5-21-100-200-300-400", path.clone()).expect("spec");
    let second = ScheduledTaskSpec::new("S-1-5-21-100-200-300-400", path).expect("spec");
    assert_eq!(first.task_name, second.task_name);
    assert!(first.task_name.starts_with("HelsincyModManager.SaveBackup."));
    assert!(!first.task_name.contains("S-1-5-21"));
    assert_eq!(first.task_name.rsplit('.').next().expect("digest").len(), 16);
}
#[test]
fn exact_readback_matches_and_each_security_field_can_drift() {
    let spec = sample_spec();
    assert_eq!(spec.compare(&exact_readback(&spec)), ScheduledTaskSpecMatch::Exact);
    let mut cases = Vec::new();
    cases.push({ let mut value = exact_readback(&spec); value.task_path = "\\Other\\".into(); value });
    cases.push({ let mut value = exact_readback(&spec); value.action_count = 2; value });
    cases.push({ let mut value = exact_readback(&spec); value.action_arguments = "--once --profile default".into(); value });
    cases.push({ let mut value = exact_readback(&spec); value.action_execute = PathBuf::from(r"C:\other.exe"); value });
    cases.push({ let mut value = exact_readback(&spec); value.action_working_directory = r"C:\Temp".into(); value });
    cases.push({ let mut value = exact_readback(&spec); value.user_sid = "S-1-5-21-9".into(); value });
    cases.push({ let mut value = exact_readback(&spec); value.logon_trigger_count = 0; value });
    cases.push({ let mut value = exact_readback(&spec); value.time_trigger_count = 2; value });
    cases.push({ let mut value = exact_readback(&spec); value.logon_trigger_user_sid = "S-1-5-21-9".into(); value });
    cases.push({ let mut value = exact_readback(&spec); value.logon_trigger_enabled = false; value });
    cases.push({ let mut value = exact_readback(&spec); value.time_trigger_enabled = false; value });
    cases.push({ let mut value = exact_readback(&spec); value.logon_type = "Password".into(); value });
    cases.push({ let mut value = exact_readback(&spec); value.run_level = "Highest".into(); value });
    cases.push({ let mut value = exact_readback(&spec); value.logon_delay = "PT0M".into(); value });
    cases.push({ let mut value = exact_readback(&spec); value.periodic_interval = "PT30M".into(); value });
    cases.push({ let mut value = exact_readback(&spec); value.periodic_duration = "PT1H".into(); value });
    cases.push({ let mut value = exact_readback(&spec); value.multiple_instances = "Parallel".into(); value });
    cases.push({ let mut value = exact_readback(&spec); value.start_when_available = false; value });
    cases.push({ let mut value = exact_readback(&spec); value.allow_start_on_batteries = false; value });
    cases.push({ let mut value = exact_readback(&spec); value.dont_stop_on_batteries = false; value });
    cases.push({ let mut value = exact_readback(&spec); value.wake_to_run = true; value });
    cases.push({ let mut value = exact_readback(&spec); value.run_only_if_network_available = true; value });
    cases.push({ let mut value = exact_readback(&spec); value.execution_time_limit = "PT2H".into(); value });
    cases.push({ let mut value = exact_readback(&spec); value.enabled = false; value });
    for value in cases {
        assert_eq!(spec.compare(&value), ScheduledTaskSpecMatch::OwnedDrift);
    }
}
#[test]
fn foreign_owner_is_not_treated_as_repairable_drift() {
    let spec = sample_spec();
    let mut readback = exact_readback(&spec);
    readback.owner_marker = "another.application/task/v1".to_owned();
    assert_eq!(spec.compare(&readback), ScheduledTaskSpecMatch::OwnershipConflict);
}
fn sample_spec() -> ScheduledTaskSpec {
    ScheduledTaskSpec::new(
        "S-1-5-21-100-200-300-400",
        std::env::temp_dir().join("hmm-save-backup-worker.exe"),
    )
    .expect("sample spec")
}
fn exact_readback(spec: &ScheduledTaskSpec) -> ScheduledTaskReadback {
    ScheduledTaskReadback {
        task_path: spec.task_path.clone(),
        owner_marker: spec.owner_marker.clone(),
        user_sid: spec.user_sid.clone(),
        action_count: 1,
        action_execute: spec.worker_path.clone(),
        action_arguments: spec.action_arguments.clone(),
        action_working_directory: String::new(),
        logon_trigger_count: 1,
        time_trigger_count: 1,
        logon_trigger_user_sid: spec.user_sid.clone(),
        logon_trigger_enabled: true,
        time_trigger_enabled: true,
        logon_delay: spec.logon_delay.clone(),
        periodic_interval: spec.periodic_interval.clone(),
        periodic_duration: String::new(),
        logon_type: "Interactive".to_owned(),
        run_level: "Limited".to_owned(),
        multiple_instances: "IgnoreNew".to_owned(),
        start_when_available: true,
        allow_start_on_batteries: true,
        dont_stop_on_batteries: true,
        wake_to_run: false,
        run_only_if_network_available: false,
        execution_time_limit: spec.execution_time_limit.clone(),
        enabled: true,
    }
}
```
另测拒绝空 SID、`S-`、`S--1`、小写 `s-1-5`、非 `S-` 格式 SID 和相对 worker path。非 `--once` arguments 与额外
action 不作为 constructor 输入；它们必须分别通过 `action_arguments` 和 `action_count`
read-back drift case 证明会被拒绝为 `OwnedDrift`。

- [ ] **Step 2: 运行 RED 测试**

Run:
```powershell
cargo test -p hmm-core background_registration_statuses_have_stable_codes
cargo test -p hmm-ports background_registry_errors_have_stable_codes
cargo test -p hmm-infra save_backup_background_registry::tests
```
Expected: FAIL，错误包含 `ConfigurationDrift`、`ScheduledTaskSpec` 或 `ScheduledTaskSpecMatch` 未定义。

- [ ] **Step 3: 重组 module 并保持 fallback**

先在 core enum 加入：
```rust
ConfigurationDrift,
```
并在 `as_str` 映射为 `"configuration_drift"`。

在 `hmm-ports/save_backup.rs` 定义 typed error，并从 `lib.rs` re-export error/result/schema
constant；把三个 registry method 从 `anyhow::Result` 改为：
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaveBackupBackgroundRegistryError {
    TaskOwnershipConflict,
    WorkerBinaryUnavailable,
    CommandTimeout,
    CommandInvalidOutput,
    OperationFailed,
}
impl SaveBackupBackgroundRegistryError {
    pub fn code(self) -> &'static str {
        match self {
            Self::TaskOwnershipConflict => "save_backup_background_task_ownership_conflict",
            Self::WorkerBinaryUnavailable => "save_backup_background_worker_binary_unavailable",
            Self::CommandTimeout => "save_backup_background_command_timeout",
            Self::CommandInvalidOutput => "save_backup_background_command_invalid_output",
            Self::OperationFailed => "save_backup_background_registration_failed",
        }
    }
}
impl std::fmt::Display for SaveBackupBackgroundRegistryError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.code())
    }
}
impl std::error::Error for SaveBackupBackgroundRegistryError {}
pub type SaveBackupBackgroundRegistryResult<T> =
    std::result::Result<T, SaveBackupBackgroundRegistryError>;
pub const SAVE_BACKUP_BACKGROUND_REGISTRY_SCHEMA_VERSION: u32 = 1;
pub trait SaveBackupBackgroundRegistry: Send + Sync {
    fn inspect(&self) -> SaveBackupBackgroundRegistryResult<SaveBackupBackgroundRegistrationStatus>;
    fn register(&self) -> SaveBackupBackgroundRegistryResult<SaveBackupBackgroundRegistrationStatus>;
    fn unregister(&self) -> SaveBackupBackgroundRegistryResult<SaveBackupBackgroundRegistrationStatus>;
}
```
error 实现 `Display + std::error::Error`，Display 只能返回 `code()`，不携带底层 message。
Task 1 的 `mod.rs` 固定结构：
```rust
mod task_spec;
#[cfg(test)]
mod tests;
use hmm_core::SaveBackupBackgroundRegistrationStatus;
use hmm_ports::{
    SaveBackupBackgroundRegistry, SaveBackupBackgroundRegistryResult,
};
#[derive(Debug, Default)]
pub struct UnsupportedSaveBackupBackgroundRegistry;
impl SaveBackupBackgroundRegistry for UnsupportedSaveBackupBackgroundRegistry {
    fn inspect(&self) -> SaveBackupBackgroundRegistryResult<SaveBackupBackgroundRegistrationStatus> {
        Ok(SaveBackupBackgroundRegistrationStatus::UnsupportedPlatform)
    }
    fn register(&self) -> SaveBackupBackgroundRegistryResult<SaveBackupBackgroundRegistrationStatus> {
        Ok(SaveBackupBackgroundRegistrationStatus::UnsupportedPlatform)
    }
    fn unregister(&self) -> SaveBackupBackgroundRegistryResult<SaveBackupBackgroundRegistrationStatus> {
        Ok(SaveBackupBackgroundRegistrationStatus::UnsupportedPlatform)
    }
}
```
Task 2 再加入 Windows-only `powershell` module；Task 3 加入跨平台 private `registry` module、
Windows-only `windows` constructor 和 public export。这样 fake lifecycle tests 在所有 target 编译，
每个 task 的提交也可独立编译。

- [ ] **Step 4: 实现 pure task spec**

`task_spec.rs` 使用以下完整字段：
```rust
use sha2::{Digest, Sha256};
use std::path::PathBuf;
pub(super) const TASK_OWNER_MARKER: &str = "dev.helsincy.modmanager/save-backup";
pub(super) const TASK_PATH: &str = "\\";
pub(super) const TASK_ARGUMENTS: &str = "--once";
pub(super) const LOGON_DELAY: &str = "PT1M";
pub(super) const PERIODIC_INTERVAL: &str = "PT15M";
pub(super) const EXECUTION_TIME_LIMIT: &str = "PT1H";
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ScheduledTaskSpec {
    pub task_name: String,
    pub task_path: String,
    pub owner_marker: String,
    pub user_sid: String,
    pub worker_path: PathBuf,
    pub action_arguments: String,
    pub logon_delay: String,
    pub periodic_interval: String,
    pub execution_time_limit: String,
}
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct ScheduledTaskReadback {
    pub task_path: String,
    pub owner_marker: String,
    pub user_sid: String,
    pub action_count: u32,
    pub action_execute: PathBuf,
    pub action_arguments: String,
    pub action_working_directory: String,
    pub logon_trigger_count: u32,
    pub time_trigger_count: u32,
    pub logon_trigger_user_sid: String,
    pub logon_trigger_enabled: bool,
    pub time_trigger_enabled: bool,
    pub logon_delay: String,
    pub periodic_interval: String,
    pub periodic_duration: String,
    pub logon_type: String,
    pub run_level: String,
    pub multiple_instances: String,
    pub start_when_available: bool,
    pub allow_start_on_batteries: bool,
    pub dont_stop_on_batteries: bool,
    pub wake_to_run: bool,
    pub run_only_if_network_available: bool,
    pub execution_time_limit: String,
    pub enabled: bool,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ScheduledTaskSpecMatch {
    Exact,
    OwnedDrift,
    OwnershipConflict,
}
impl ScheduledTaskSpec {
    pub fn new(user_sid: &str, worker_path: PathBuf) -> Result<Self, &'static str> {
        let sid_segments = user_sid.split('-').collect::<Vec<_>>();
        let valid_sid = user_sid.len() <= 184
            && sid_segments.len() >= 3
            && sid_segments[0] == "S"
            && sid_segments[1..]
                .iter()
                .all(|segment| !segment.is_empty() && segment.bytes().all(|value| value.is_ascii_digit()));
        if !valid_sid || !worker_path.is_absolute() {
            return Err("invalid scheduled task identity");
        }
        let digest = Sha256::digest(user_sid.as_bytes());
        let suffix = digest[..8].iter().map(|value| format!("{value:02x}")).collect::<String>();
        Ok(Self {
            task_name: format!("HelsincyModManager.SaveBackup.{suffix}"),
            task_path: TASK_PATH.to_owned(),
            owner_marker: TASK_OWNER_MARKER.to_owned(),
            user_sid: user_sid.to_owned(),
            worker_path,
            action_arguments: TASK_ARGUMENTS.to_owned(),
            logon_delay: LOGON_DELAY.to_owned(),
            periodic_interval: PERIODIC_INTERVAL.to_owned(),
            execution_time_limit: EXECUTION_TIME_LIMIT.to_owned(),
        })
    }
    pub fn compare(&self, actual: &ScheduledTaskReadback) -> ScheduledTaskSpecMatch {
        if actual.owner_marker != self.owner_marker {
            return ScheduledTaskSpecMatch::OwnershipConflict;
        }
        let exact = actual.task_path == self.task_path
            && actual.user_sid == self.user_sid
            && actual.action_count == 1
            && actual.action_execute == self.worker_path
            && actual.action_arguments == self.action_arguments
            && actual.action_working_directory.is_empty()
            && actual.logon_trigger_count == 1
            && actual.time_trigger_count == 1
            && actual.logon_trigger_user_sid == self.user_sid
            && actual.logon_trigger_enabled
            && actual.time_trigger_enabled
            && actual.logon_delay == self.logon_delay
            && actual.periodic_interval == self.periodic_interval
            && actual.periodic_duration.is_empty()
            && actual.logon_type.eq_ignore_ascii_case("Interactive")
            && actual.run_level.eq_ignore_ascii_case("Limited")
            && actual.multiple_instances.eq_ignore_ascii_case("IgnoreNew")
            && actual.start_when_available
            && actual.allow_start_on_batteries
            && actual.dont_stop_on_batteries
            && !actual.wake_to_run
            && !actual.run_only_if_network_available
            && actual.execution_time_limit == self.execution_time_limit
            && actual.enabled;
        if exact { ScheduledTaskSpecMatch::Exact } else { ScheduledTaskSpecMatch::OwnedDrift }
    }
}
```
- [ ] **Step 5: 运行 GREEN 和 fallback 回归**

Run:
```powershell
cargo test -p hmm-core background_registration_statuses_have_stable_codes
cargo test -p hmm-ports background_registry_errors_have_stable_codes
cargo test -p hmm-infra save_backup_background_registry::tests
cargo test -p hmm-infra --test save_backup_background_registry
cargo clippy -p hmm-infra --all-targets -- -D warnings
```
Expected: 全部 PASS；non-Windows fallback 仍只返回 unsupported。

- [ ] **Step 6: Commit**
```powershell
git add src-tauri/crates/hmm-core/src/save_backup.rs src-tauri/crates/hmm-ports/src/save_backup.rs src-tauri/crates/hmm-ports/src/lib.rs src-tauri/crates/hmm-infra/src/save_backup_background_registry.rs src-tauri/crates/hmm-infra/src/save_backup_background_registry src-tauri/crates/hmm-infra/src/lib.rs src-tauri/crates/hmm-infra/tests/save_backup_background_registry.rs
git commit -m "refactor: define scheduled task registration spec"
```
---

### Task 2: 实现固定 PowerShell ScheduledTasks Command Runner

**Files:**
- Modify: `Cargo.toml`
- Modify: `src-tauri/crates/hmm-infra/Cargo.toml`
- Create: `src-tauri/crates/hmm-infra/src/save_backup_background_registry/powershell.rs`
- Create: `src-tauri/crates/hmm-infra/src/save_backup_background_registry/scheduled_task.ps1`
- Modify: `src-tauri/crates/hmm-infra/src/save_backup_background_registry/mod.rs`
- Modify: `src-tauri/crates/hmm-infra/src/save_backup_background_registry/tests.rs`

**Interfaces:**
- Produces: private `ScheduledTaskCommand::{Identity, Inspect, Register, Unregister}`。
- Produces: private `ScheduledTaskCommandOutcome::{Identity, Missing, Found, Completed, PermissionRequired, ModuleUnavailable, OwnershipConflict}`；generic failure 直接返回 typed `OperationFailed`，不创建携带原文的 outcome。
- Produces: `PowerShellScheduledTaskCommandRunner` with 15-second timeout and 64 KiB stdout ceiling。
- Produces: `system_powershell_runtime()`，通过 `GetSystemDirectoryW` 返回可信 executable/module absolute paths。
- Consumes: Task 1 的 `ScheduledTaskSpec` / `ScheduledTaskReadback`。

- [ ] **Step 1: 写 runner parser/timeout/forbidden-input RED 测试**

测试固定 JSON schema：
```rust
#[test]
fn parses_versioned_inspect_output_without_exposing_raw_output() {
    let output = br#"{"schemaVersion":1,"status":"found","task":{"taskPath":"\\","ownerMarker":"dev.helsincy.modmanager/save-backup","userSid":"S-1-5-21-1","actionCount":1,"actionExecute":"C:\\HMM\\hmm-save-backup-worker.exe","actionArguments":"--once","actionWorkingDirectory":"","logonTriggerCount":1,"timeTriggerCount":1,"logonTriggerUserSid":"S-1-5-21-1","logonTriggerEnabled":true,"timeTriggerEnabled":true,"logonDelay":"PT1M","periodicInterval":"PT15M","periodicDuration":"","logonType":"Interactive","runLevel":"Limited","multipleInstances":"IgnoreNew","startWhenAvailable":true,"allowStartOnBatteries":true,"dontStopOnBatteries":true,"wakeToRun":false,"runOnlyIfNetworkAvailable":false,"executionTimeLimit":"PT1H","enabled":true}}"#;
    let parsed = parse_script_output(output).expect("valid output");
    assert!(matches!(parsed, ScheduledTaskCommandOutcome::Found(_)));
}
#[test]
fn rejects_unknown_schema_oversized_output_and_unknown_status() {
    assert!(parse_script_output(br#"{"schemaVersion":2,"status":"completed"}"#).is_err());
    assert!(parse_script_output(br#"{"schemaVersion":1,"status":"surprise"}"#).is_err());
    assert!(parse_script_output(&vec![b'x'; 65_537]).is_err());
}
```
再断言 command builder 的 executable 是 `system_powershell_runtime()` 返回的 absolute path，
basename 固定为 `powershell.exe`，且 args 只有
`-NoLogo -NoProfile -NonInteractive -Command SCRIPT`（`SCRIPT` 是 `include_str!` 的固定内容），没有
`ExecutionPolicy`、用户脚本、task XML 或 shell 拼接。
Windows test 还要断言 executable/module 都是 absolute existing file，file name 分别为
`powershell.exe` / `ScheduledTasks.psd1`；测试只读系统目录，不执行 ScheduledTasks command。

脚本 source test 固定关键 fail-closed 约束：
```rust
let script = include_str!("scheduled_task.ps1");
assert!(script.contains("-TaskPath \"\\\""));
assert!(script.contains("CategoryInfo.Category"));
assert!(script.contains("CmdletizationQuery_NotFound"));
assert!(script.contains("HMM_SCHEDULED_TASKS_MODULE"));
assert!(script.contains("Import-Module -Name $modulePath"));
assert!(script.contains("$Value.schemaVersion = 1"));
assert!(!script.contains("NativeErrorCode"));
assert!(!script.contains("Get-Module -ListAvailable"));
assert!(!script.contains("ExecutionPolicy"));
assert!(!script.contains("Invoke-Expression"));
assert!(!script.lines().any(|line| line.contains("Register-ScheduledTask") && line.contains("-Force")));
```
- [ ] **Step 2: 运行 RED 测试**

Run:
```powershell
cargo test -p hmm-infra save_backup_background_registry::tests
```
Expected: FAIL，错误包含 `parse_script_output` / runner types 不存在。

- [ ] **Step 3: 添加 Windows-only timeout 依赖与 runner contract**

workspace dependencies 加入：
```toml
wait-timeout = "0.2.1"
windows-sys = "0.61.2"
```
`hmm-infra` 已有的 Windows dependencies 保留 `winreg`（Steam discovery 正在使用），只加入：
```toml
[target.'cfg(windows)'.dependencies]
wait-timeout.workspace = true
windows-sys = { workspace = true, features = ["Win32_System_SystemInformation"] }
```
`mod.rs` 中定义 operation/outcome/runner trait，并仅在 Windows 编译 real runner：
```rust
#[derive(Debug, Clone, PartialEq, Eq)]
enum ScheduledTaskCommand {
    Identity,
    Inspect { task_name: String, owner_marker: String },
    Register(ScheduledTaskSpec),
    Unregister { task_name: String, owner_marker: String },
}
#[derive(Debug, Clone, PartialEq, Eq)]
enum ScheduledTaskCommandOutcome {
    Identity(String),
    Missing,
    Found(ScheduledTaskReadback),
    Completed,
    PermissionRequired,
    ModuleUnavailable,
    OwnershipConflict,
}
trait ScheduledTaskCommandRunner: Send + Sync {
    fn run(
        &self,
        command: ScheduledTaskCommand,
    ) -> SaveBackupBackgroundRegistryResult<ScheduledTaskCommandOutcome>;
}
```
`powershell.rs` 把 `operation_failed` 转成 typed `OperationFailed` error；timeout/invalid output
分别转成 `CommandTimeout` / `CommandInvalidOutput`，不创建 `Failed(String)` 之类携带原文的
outcome。

- [ ] **Step 4: 创建固定 ScheduledTasks 脚本**

`scheduled_task.ps1` 使用以下完整流程；所有动态值来自 Rust 设置的固定环境键：
```powershell
Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"
$ProgressPreference = "SilentlyContinue"
$WarningPreference = "SilentlyContinue"
$VerbosePreference = "SilentlyContinue"
$DebugPreference = "SilentlyContinue"
$InformationPreference = "SilentlyContinue"
[Console]::OutputEncoding = New-Object System.Text.UTF8Encoding($false)
function Write-Result([hashtable]$Value) {
    $Value.schemaVersion = 1
    [Console]::Out.WriteLine(($Value | ConvertTo-Json -Compress -Depth 6))
    exit 0
}
function Get-TaskOrStatus([string]$TaskName) {
    try {
        return Get-ScheduledTask -TaskPath "\" -TaskName $TaskName -ErrorAction Stop
    } catch {
        Write-TaskLookupFailure $_
    }
}
function Test-PermissionFailure($ErrorRecord) {
    $category = [string]$ErrorRecord.CategoryInfo.Category
    return $category -eq "PermissionDenied" -or $category -eq "SecurityError"
}
function Write-TaskLookupFailure($ErrorRecord) {
    $category = [string]$ErrorRecord.CategoryInfo.Category
    $errorId = [string]$ErrorRecord.FullyQualifiedErrorId
    if ($category -eq "ObjectNotFound" -and $errorId.StartsWith("CmdletizationQuery_NotFound")) {
        Write-Result @{ status = "not_found" }
    }
    if (Test-PermissionFailure $ErrorRecord) {
        Write-Result @{ status = "permission_required" }
    }
    Write-Result @{ status = "operation_failed" }
}
function Write-OperationFailure($ErrorRecord) {
    if (Test-PermissionFailure $ErrorRecord) {
        Write-Result @{ status = "permission_required" }
    }
    Write-Result @{ status = "operation_failed" }
}
function Resolve-Sid([string]$Identity) {
    if ([string]::IsNullOrWhiteSpace($Identity)) { return "" }
    if ($Identity -match "^S-[0-9-]+$") { return $Identity }
    return (New-Object System.Security.Principal.NTAccount($Identity)).Translate(
        [System.Security.Principal.SecurityIdentifier]
    ).Value
}
try {
    $operation = $env:HMM_OPERATION
    if ($operation -eq "identity") {
        $sid = [System.Security.Principal.WindowsIdentity]::GetCurrent().User.Value
        Write-Result @{ status = "identity"; currentUserSid = $sid }
    }
    $modulePath = $env:HMM_SCHEDULED_TASKS_MODULE
    if ([string]::IsNullOrWhiteSpace($modulePath) -or -not (Test-Path -LiteralPath $modulePath -PathType Leaf)) {
        Write-Result @{ status = "module_unavailable" }
    }
    Import-Module -Name $modulePath -Force -ErrorAction Stop
    $taskName = $env:HMM_TASK_NAME
    $ownerMarker = $env:HMM_OWNER_MARKER
    if ([string]::IsNullOrWhiteSpace($taskName) -or [string]::IsNullOrWhiteSpace($ownerMarker)) {
        Write-Result @{ status = "operation_failed" }
    }
    if ($operation -eq "inspect") {
        $task = Get-TaskOrStatus $taskName
        $actions = @($task.Actions)
        $triggers = @($task.Triggers)
        $logon = @($triggers | Where-Object { $_.CimClass.CimClassName -eq "MSFT_TaskLogonTrigger" })
        $time = @($triggers | Where-Object { $_.CimClass.CimClassName -eq "MSFT_TaskTimeTrigger" })
        $action = if ($actions.Count -eq 1) { $actions[0] } else { $null }
        Write-Result @{ status = "found"; task = @{
            taskPath = [string]$task.TaskPath
            ownerMarker = [string]$task.Description
            userSid = Resolve-Sid ([string]$task.Principal.UserId)
            actionCount = $actions.Count
            actionExecute = if ($null -eq $action) { "" } else { [string]$action.Execute }
            actionArguments = if ($null -eq $action) { "" } else { [string]$action.Arguments }
            actionWorkingDirectory = if ($null -eq $action) { "" } else { [string]$action.WorkingDirectory }
            logonTriggerCount = $logon.Count
            timeTriggerCount = $time.Count
            logonTriggerUserSid = if ($logon.Count -eq 1) { Resolve-Sid ([string]$logon[0].UserId) } else { "" }
            logonTriggerEnabled = if ($logon.Count -eq 1) { [bool]$logon[0].Enabled } else { $false }
            timeTriggerEnabled = if ($time.Count -eq 1) { [bool]$time[0].Enabled } else { $false }
            logonDelay = if ($logon.Count -eq 1) { [string]$logon[0].Delay } else { "" }
            periodicInterval = if ($time.Count -eq 1) { [string]$time[0].Repetition.Interval } else { "" }
            periodicDuration = if ($time.Count -eq 1) { [string]$time[0].Repetition.Duration } else { "" }
            logonType = [string]$task.Principal.LogonType
            runLevel = [string]$task.Principal.RunLevel
            multipleInstances = [string]$task.Settings.MultipleInstances
            startWhenAvailable = [bool]$task.Settings.StartWhenAvailable
            allowStartOnBatteries = -not [bool]$task.Settings.DisallowStartIfOnBatteries
            dontStopOnBatteries = -not [bool]$task.Settings.StopIfGoingOnBatteries
            wakeToRun = [bool]$task.Settings.WakeToRun
            runOnlyIfNetworkAvailable = [bool]$task.Settings.RunOnlyIfNetworkAvailable
            executionTimeLimit = [string]$task.Settings.ExecutionTimeLimit
            enabled = [string]$task.State -ne "Disabled"
        }}
    }
    if ($operation -eq "register") {
        $workerPath = $env:HMM_WORKER_PATH
        $userSid = $env:HMM_USER_SID
        if ([string]::IsNullOrWhiteSpace($workerPath) -or [string]::IsNullOrWhiteSpace($userSid)) {
            Write-Result @{ status = "operation_failed" }
        }
        $action = New-ScheduledTaskAction -Execute $workerPath -Argument "--once"
        $logon = New-ScheduledTaskTrigger -AtLogOn -User $userSid
        $logon.Delay = "PT1M"
        $periodic = New-ScheduledTaskTrigger -Once -At (Get-Date).AddMinutes(1) -RepetitionInterval (New-TimeSpan -Minutes 15)
        $principal = New-ScheduledTaskPrincipal -UserId $userSid -LogonType Interactive -RunLevel Limited
        $settings = New-ScheduledTaskSettingsSet -StartWhenAvailable -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries -ExecutionTimeLimit (New-TimeSpan -Hours 1) -MultipleInstances IgnoreNew
        $current = $null
        try { $current = Get-ScheduledTask -TaskPath "\" -TaskName $taskName -ErrorAction Stop }
        catch {
            $category = [string]$_.CategoryInfo.Category
            $errorId = [string]$_.FullyQualifiedErrorId
            $missing = $category -eq "ObjectNotFound" -and $errorId.StartsWith("CmdletizationQuery_NotFound")
            if (-not $missing) { Write-OperationFailure $_ }
        }
        if ($null -ne $current -and [string]$current.Description -ne $ownerMarker) {
            Write-Result @{ status = "ownership_conflict" }
        }
        if ($null -eq $current) {
            Register-ScheduledTask -TaskPath "\" -TaskName $taskName -Action $action -Trigger @($logon, $periodic) -Settings $settings -Principal $principal -Description $ownerMarker | Out-Null
        } else {
            $updated = Set-ScheduledTask -TaskPath "\" -TaskName $taskName -Action $action -Trigger @($logon, $periodic) -Settings $settings -Principal $principal
            Enable-ScheduledTask -InputObject $updated | Out-Null
        }
        Write-Result @{ status = "completed" }
    }
    if ($operation -eq "unregister") {
        $current = Get-TaskOrStatus $taskName
        if ([string]$current.Description -ne $ownerMarker) {
            Write-Result @{ status = "ownership_conflict" }
        }
        Unregister-ScheduledTask -InputObject $current -Confirm:$false -ErrorAction Stop
        Write-Result @{ status = "completed" }
    }
    Write-Result @{ status = "operation_failed" }
} catch {
    Write-OperationFailure $_
}
```
`Get-ScheduledTask` 的 missing classification 不使用 `CimException.NativeErrorCode`：PowerShell
5.1 实际返回的可能是 `CimJobException` 且 `NativeErrorCode = null`。只使用非本地化
`ErrorCategory::ObjectNotFound` + stable `FullyQualifiedErrorId` 前缀
`CmdletizationQuery_NotFound`；permission 只接受
`PermissionDenied` / `SecurityError` category，其他异常全部 fail closed 为 operation_failed。
register branch 在构造完整 spec 后立即重新读取 owner：missing 使用无 `-Force` 的 create，owned
使用 `Set-ScheduledTask` + `Enable-ScheduledTask`，foreign 返回 conflict；unregister 通过复核后的
`InputObject` 删除。仍需在 final review 记录同一用户恶意进程可在 read/write 间竞争的残余风险。

- [ ] **Step 5: 实现 Rust subprocess 与 JSON 白名单 parser**

parser DTO 使用 `deny_unknown_fields`，避免 PowerShell 输出悄悄扩展为未 review 的 contract：
```rust
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ScriptEnvelope {
    schema_version: u32,
    status: String,
    current_user_sid: Option<String>,
    task: Option<ScheduledTaskReadback>,
}
pub(super) fn parse_script_output(
    output: &[u8],
) -> SaveBackupBackgroundRegistryResult<ScheduledTaskCommandOutcome> {
    if output.len() > MAX_OUTPUT_BYTES {
        return Err(SaveBackupBackgroundRegistryError::CommandInvalidOutput);
    }
    let envelope: ScriptEnvelope = serde_json::from_slice(output)
        .map_err(|_| SaveBackupBackgroundRegistryError::CommandInvalidOutput)?;
    if envelope.schema_version != SAVE_BACKUP_BACKGROUND_REGISTRY_SCHEMA_VERSION {
        return Err(SaveBackupBackgroundRegistryError::CommandInvalidOutput);
    }
    match (envelope.status.as_str(), envelope.current_user_sid, envelope.task) {
        ("identity", Some(sid), None) if !sid.is_empty() => Ok(ScheduledTaskCommandOutcome::Identity(sid)),
        ("not_found", None, None) => Ok(ScheduledTaskCommandOutcome::Missing),
        ("found", None, Some(task)) => Ok(ScheduledTaskCommandOutcome::Found(task)),
        ("completed", None, None) => Ok(ScheduledTaskCommandOutcome::Completed),
        ("permission_required", None, None) => Ok(ScheduledTaskCommandOutcome::PermissionRequired),
        ("module_unavailable", None, None) => Ok(ScheduledTaskCommandOutcome::ModuleUnavailable),
        ("ownership_conflict", None, None) => Ok(ScheduledTaskCommandOutcome::OwnershipConflict),
        ("operation_failed", None, None) => Err(SaveBackupBackgroundRegistryError::OperationFailed),
        _ => Err(SaveBackupBackgroundRegistryError::CommandInvalidOutput),
    }
}
```
`ScheduledTaskReadback` 自身的 `deny_unknown_fields` 保证 task payload 不解析或保留其他 JSON。
parser tests 再覆盖 unknown field、found 缺 task、identity 混入 task 和 operation_failed ->
typed error。

runner 固定：15 秒 timeout、stdout 64 KiB、stderr 丢弃、Windows hidden window。为避免
子进程输出填满 pipe 后与 `wait_timeout` 互相等待，reader thread 必须在 child 运行期间持续
drain stdout，只保留前 `MAX_OUTPUT_BYTES + 1` bytes。`run` 的输入参数命名为
`request: ScheduledTaskCommand`，下面的 `command` local 专指 `std::process::Command`：
```rust
use super::{
    task_spec::ScheduledTaskReadback, ScheduledTaskCommand, ScheduledTaskCommandOutcome,
    ScheduledTaskCommandRunner,
};
use hmm_ports::{
    SaveBackupBackgroundRegistryError, SaveBackupBackgroundRegistryResult,
    SAVE_BACKUP_BACKGROUND_REGISTRY_SCHEMA_VERSION,
};
use serde::Deserialize;
use std::ffi::OsString;
use std::io::Read;
use std::os::windows::ffi::OsStringExt;
use std::os::windows::process::CommandExt;
use std::process::{ChildStdout, Command, Stdio};
use std::path::PathBuf;
use std::time::Duration;
use wait_timeout::ChildExt;
use windows_sys::Win32::System::SystemInformation::GetSystemDirectoryW;
const COMMAND_TIMEOUT: Duration = Duration::from_secs(15);
const MAX_OUTPUT_BYTES: usize = 64 * 1024;
const SCRIPT: &str = include_str!("scheduled_task.ps1");
pub(super) struct SystemPowerShellRuntime {
    pub(super) executable: PathBuf,
    pub(super) scheduled_tasks_module: PathBuf,
}
pub(super) fn system_powershell_runtime() -> SaveBackupBackgroundRegistryResult<SystemPowerShellRuntime> {
    let mut buffer = vec![0_u16; 32_768];
    let length = unsafe { GetSystemDirectoryW(buffer.as_mut_ptr(), buffer.len() as u32) };
    if length == 0 || length as usize >= buffer.len() {
        return Err(SaveBackupBackgroundRegistryError::OperationFailed);
    }
    buffer.truncate(length as usize);
    let powershell_root = PathBuf::from(OsString::from_wide(&buffer))
        .join("WindowsPowerShell")
        .join("v1.0");
    let executable = powershell_root.join("powershell.exe");
    let scheduled_tasks_module = powershell_root
        .join("Modules")
        .join("ScheduledTasks")
        .join("ScheduledTasks.psd1");
    if !executable.is_absolute() || !executable.is_file() || !scheduled_tasks_module.is_absolute() {
        return Err(SaveBackupBackgroundRegistryError::OperationFailed);
    }
    Ok(SystemPowerShellRuntime { executable, scheduled_tasks_module })
}
pub(super) fn build_command(request: &ScheduledTaskCommand) -> SaveBackupBackgroundRegistryResult<Command> {
let runtime = system_powershell_runtime()?;
let mut command = Command::new(&runtime.executable);
command.args(["-NoLogo", "-NoProfile", "-NonInteractive", "-Command", SCRIPT]);
match request {
    ScheduledTaskCommand::Identity => { command.env("HMM_OPERATION", "identity"); }
    ScheduledTaskCommand::Inspect { task_name, owner_marker } => {
        command.env("HMM_OPERATION", "inspect").env("HMM_SCHEDULED_TASKS_MODULE", &runtime.scheduled_tasks_module)
            .env("HMM_TASK_NAME", task_name).env("HMM_OWNER_MARKER", owner_marker);
    }
    ScheduledTaskCommand::Register(spec) => {
        command.env("HMM_OPERATION", "register").env("HMM_SCHEDULED_TASKS_MODULE", &runtime.scheduled_tasks_module)
            .env("HMM_TASK_NAME", &spec.task_name).env("HMM_OWNER_MARKER", &spec.owner_marker)
            .env("HMM_WORKER_PATH", &spec.worker_path).env("HMM_USER_SID", &spec.user_sid);
    }
    ScheduledTaskCommand::Unregister { task_name, owner_marker } => {
        command.env("HMM_OPERATION", "unregister").env("HMM_SCHEDULED_TASKS_MODULE", &runtime.scheduled_tasks_module)
            .env("HMM_TASK_NAME", task_name).env("HMM_OWNER_MARKER", owner_marker);
    }
}
command.stdout(Stdio::piped()).stderr(Stdio::null());
command.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
Ok(command)
}
fn drain_stdout(mut stdout: ChildStdout) -> std::io::Result<Vec<u8>> {
    let mut retained = Vec::with_capacity(MAX_OUTPUT_BYTES + 1);
    let mut buffer = [0_u8; 8 * 1024];
    loop {
        let count = stdout.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        let remaining = (MAX_OUTPUT_BYTES + 1).saturating_sub(retained.len());
        retained.extend_from_slice(&buffer[..count.min(remaining)]);
    }
    Ok(retained)
}
#[derive(Debug, Clone, Copy, Default)]
pub(super) struct PowerShellScheduledTaskCommandRunner;
impl ScheduledTaskCommandRunner for PowerShellScheduledTaskCommandRunner {
fn run(&self, request: ScheduledTaskCommand) -> SaveBackupBackgroundRegistryResult<ScheduledTaskCommandOutcome> {
let mut command = build_command(&request)?;
let mut child = command
    .spawn()
    .map_err(|_| SaveBackupBackgroundRegistryError::OperationFailed)?;
let stdout = child
    .stdout
    .take()
    .ok_or(SaveBackupBackgroundRegistryError::CommandInvalidOutput)?;
let reader = std::thread::spawn(move || drain_stdout(stdout));
let status = match child
    .wait_timeout(COMMAND_TIMEOUT)
    .map_err(|_| SaveBackupBackgroundRegistryError::OperationFailed)?
{
    Some(status) => status,
    None => {
        let _ = child.kill();
        let _ = child.wait();
        let _ = reader.join();
        return Err(SaveBackupBackgroundRegistryError::CommandTimeout);
    }
};
let output = reader
    .join()
    .map_err(|_| SaveBackupBackgroundRegistryError::CommandInvalidOutput)?
    .map_err(|_| SaveBackupBackgroundRegistryError::CommandInvalidOutput)?;
if !status.success() {
    return Err(SaveBackupBackgroundRegistryError::OperationFailed);
}
parse_script_output(&output)
}
}
```
只设置 `HMM_OPERATION`、`HMM_SCHEDULED_TASKS_MODULE`、`HMM_TASK_NAME`、
`HMM_OWNER_MARKER`、`HMM_WORKER_PATH`、`HMM_USER_SID` 六个内部环境键；未使用的键不设置。
module path 只为非 Identity command 设置，值只能来自 `system_powershell_runtime()`。timeout 时
kill + wait，返回内部
`CommandTimeout`；invalid/oversized JSON 返回 `InvalidOutput`。这两个错误只由 Task 3 映射
稳定 registration status/error code，原始 bytes 不进入 error display。

command -> env 映射固定为：Identity 只设置 `HMM_OPERATION=identity`；Inspect 设置
`inspect + system module path + task_name + owner_marker`；Register 设置
`register + system module path + task_name + owner_marker + canonical worker_path + user_sid`；
Unregister 设置 `unregister + system module path + task_name + owner_marker`。runner API 不接受 executable、script、task path、
arguments 或 XML；task path `\` 和 `--once` 都在编译期脚本内固定。

- [ ] **Step 6: 运行 GREEN、脚本静态边界和 crate 检查**

Run:
```powershell
cargo test -p hmm-infra save_backup_background_registry::tests
cargo check -p hmm-infra
cargo clippy -p hmm-infra --all-targets -- -D warnings
$forbidden = rg -n "ExecutionPolicy|Invoke-Expression|iex|schtasks|task_xml|FromBase64String|NativeErrorCode|SystemRoot|Get-Module -ListAvailable|Command::new\(\"powershell.exe\"\)|Register-ScheduledTask.*-Force" src-tauri/crates/hmm-infra/src/save_backup_background_registry/powershell.rs src-tauri/crates/hmm-infra/src/save_backup_background_registry/scheduled_task.ps1 2>&1
if ($LASTEXITCODE -eq 0) { $forbidden; throw 'forbidden Scheduled Task runner pattern found' }
if ($LASTEXITCODE -ne 1) { throw 'runner boundary search failed' }
```
Expected: tests/check/clippy PASS；边界搜索无命中。自动测试不得调用 real runner。

- [ ] **Step 7: Commit**
```powershell
git add Cargo.toml Cargo.lock src-tauri/crates/hmm-infra/Cargo.toml src-tauri/crates/hmm-infra/src/save_backup_background_registry/mod.rs src-tauri/crates/hmm-infra/src/save_backup_background_registry/powershell.rs src-tauri/crates/hmm-infra/src/save_backup_background_registry/scheduled_task.ps1 src-tauri/crates/hmm-infra/src/save_backup_background_registry/tests.rs
git commit -m "feat: add controlled scheduled task runner"
```
---

### Task 3: 实现 Windows Registry Inspect/Register/Update/Unregister

**Files:**
- Create: `src-tauri/crates/hmm-infra/src/save_backup_background_registry/registry.rs`
- Create: `src-tauri/crates/hmm-infra/src/save_backup_background_registry/windows.rs`
- Modify: `src-tauri/crates/hmm-infra/src/save_backup_background_registry/mod.rs`
- Modify: `src-tauri/crates/hmm-infra/src/save_backup_background_registry/tests.rs`
- Modify: `src-tauri/crates/hmm-infra/src/lib.rs`
- Modify: `src-tauri/crates/hmm-infra/tests/save_backup_background_registry.rs`

**Interfaces:**
- Produces: infallible public Windows-only `WindowsScheduledTaskRegistry::from_current_exe()`。
- Produces: `registry.rs` private generic `ScheduledTaskRegistry<R>`，使 fake lifecycle tests 在所有测试平台运行。
- Consumes: Tasks 1/2 的 task spec、runner operation/outcome。
- Preserves: `SaveBackupBackgroundRegistry` 无输入 contract。

- [ ] **Step 1: 写完整 fake lifecycle RED 测试**

fake runner 必须可排队 outcomes 并记录 operations。先实现完整 harness：
```rust
#[derive(Clone, Default)]
struct FakeRunner {
    outcomes: Arc<Mutex<VecDeque<SaveBackupBackgroundRegistryResult<ScheduledTaskCommandOutcome>>>>,
    commands: Arc<Mutex<Vec<ScheduledTaskCommand>>>,
}
impl FakeRunner {
    fn with_outcomes(outcomes: Vec<SaveBackupBackgroundRegistryResult<ScheduledTaskCommandOutcome>>) -> Self {
        Self {
            outcomes: Arc::new(Mutex::new(outcomes.into())),
            commands: Arc::new(Mutex::new(Vec::new())),
        }
    }
    fn commands(&self) -> Vec<ScheduledTaskCommand> {
        self.commands.lock().expect("commands lock").clone()
    }
}
impl ScheduledTaskCommandRunner for FakeRunner {
    fn run(&self, command: ScheduledTaskCommand) -> SaveBackupBackgroundRegistryResult<ScheduledTaskCommandOutcome> {
        self.commands.lock().expect("commands lock").push(command);
        self.outcomes.lock().expect("outcomes lock").pop_front().expect("queued outcome")
    }
}
struct RegistryFixture {
    _temp: tempfile::TempDir,
    sid: String,
    worker_path: PathBuf,
    exact_readback: ScheduledTaskReadback,
}
impl RegistryFixture {
    fn new() -> Self {
        Self::build(true)
    }
    fn new_without_worker_file() -> Self {
        Self::build(false)
    }
    fn build(create_worker: bool) -> Self {
        let temp = tempfile::tempdir().expect("temp dir");
        let worker_path = temp.path().join("hmm-save-backup-worker.exe");
        if create_worker {
            std::fs::write(&worker_path, b"fixture").expect("write worker fixture");
        }
        let sid = "S-1-5-21-100-200-300-400".to_owned();
        let spec_path = if create_worker {
            std::fs::canonicalize(&worker_path).expect("canonical worker fixture")
        } else {
            worker_path.clone()
        };
        let spec = ScheduledTaskSpec::new(&sid, spec_path).expect("fixture spec");
        let exact_readback = exact_readback(&spec);
        Self { _temp: temp, sid, worker_path, exact_readback }
    }
}
#[test]
fn register_creates_missing_task_then_requires_exact_readback() {
    let fixture = RegistryFixture::new();
    let runner = FakeRunner::with_outcomes(vec![
        Ok(ScheduledTaskCommandOutcome::Identity(fixture.sid.clone())),
        Ok(ScheduledTaskCommandOutcome::Missing),
        Ok(ScheduledTaskCommandOutcome::Completed),
        Ok(ScheduledTaskCommandOutcome::Found(fixture.exact_readback.clone())),
    ]);
    let registry = ScheduledTaskRegistry::new(runner.clone(), fixture.worker_path.clone());
    assert_eq!(registry.register().expect("register"), SaveBackupBackgroundRegistrationStatus::Registered);
    assert!(matches!(runner.commands().as_slice(), [ScheduledTaskCommand::Identity, ScheduledTaskCommand::Inspect { .. }, ScheduledTaskCommand::Register(_), ScheduledTaskCommand::Inspect { .. }]));
}
#[test]
fn register_repairs_owned_drift_but_never_overwrites_foreign_owner() {
    let fixture = RegistryFixture::new();
    let mut drift = fixture.exact_readback.clone();
    drift.action_arguments = "--once --profile default".to_owned();
    let repair = FakeRunner::with_outcomes(vec![
        Ok(ScheduledTaskCommandOutcome::Identity(fixture.sid.clone())),
        Ok(ScheduledTaskCommandOutcome::Found(drift)),
        Ok(ScheduledTaskCommandOutcome::Completed),
        Ok(ScheduledTaskCommandOutcome::Found(fixture.exact_readback.clone())),
    ]);
    let registry = ScheduledTaskRegistry::new(repair.clone(), fixture.worker_path.clone());
    assert_eq!(registry.register().expect("repair"), SaveBackupBackgroundRegistrationStatus::Registered);
    assert!(repair.commands().iter().any(|command| matches!(command, ScheduledTaskCommand::Register(_))));
    let mut foreign = fixture.exact_readback.clone();
    foreign.owner_marker = "another.application/task/v1".to_owned();
    let conflict = FakeRunner::with_outcomes(vec![
        Ok(ScheduledTaskCommandOutcome::Identity(fixture.sid.clone())),
        Ok(ScheduledTaskCommandOutcome::Found(foreign)),
    ]);
    let registry = ScheduledTaskRegistry::new(conflict.clone(), fixture.worker_path.clone());
    assert_eq!(registry.register().expect_err("foreign owner blocked"), SaveBackupBackgroundRegistryError::TaskOwnershipConflict);
    assert!(!conflict.commands().iter().any(|command| matches!(command, ScheduledTaskCommand::Register(_))));
}
#[test]
fn unregister_is_idempotent_rechecks_and_does_not_require_worker_file() {
    let fixture = RegistryFixture::new_without_worker_file();
    let runner = FakeRunner::with_outcomes(vec![
        Ok(ScheduledTaskCommandOutcome::Identity(fixture.sid.clone())),
        Ok(ScheduledTaskCommandOutcome::Found(fixture.exact_readback.clone())),
        Ok(ScheduledTaskCommandOutcome::Completed),
        Ok(ScheduledTaskCommandOutcome::Missing),
    ]);
    let registry = ScheduledTaskRegistry::new(runner.clone(), fixture.worker_path);
    assert_eq!(registry.unregister().expect("unregister"), SaveBackupBackgroundRegistrationStatus::NotRegistered);
    assert!(runner.commands().iter().any(|command| matches!(command, ScheduledTaskCommand::Unregister { .. })));
}
#[test]
fn runner_errors_remain_typed_and_fail_closed() {
    for expected in [
        SaveBackupBackgroundRegistryError::CommandTimeout,
        SaveBackupBackgroundRegistryError::CommandInvalidOutput,
        SaveBackupBackgroundRegistryError::OperationFailed,
    ] {
        let fixture = RegistryFixture::new();
        let runner = FakeRunner::with_outcomes(vec![Err(expected)]);
        let registry = ScheduledTaskRegistry::new(runner, fixture.worker_path);
        assert_eq!(registry.inspect().expect_err("typed failure"), expected);
    }
}
```
`RegistryFixture` 在 temp dir 创建/缺省 worker，使用固定 SID 构造 exact read-back；另补
canonical worker 的 missing/non-file/symlink 拒绝、exact register no-op、post-write 仍 drift、
permission/module unavailable 和 foreign unregister tests，不增加 real runner 调用。

- [ ] **Step 2: 运行 RED 测试**

Run:
```powershell
cargo test -p hmm-infra save_backup_background_registry::tests
```
Expected: FAIL，错误包含 `ScheduledTaskRegistry` / `WindowsScheduledTaskRegistry` 未定义。

- [ ] **Step 3: 实现 expected spec 和 inspect 映射**

`ScheduledTaskRegistry<R>` 保存 `runner: R` 和可注入 `worker_path: Option<PathBuf>`；tests 的
`new(runner, path)` 包装为 `Some(path)`，production locator 可传 None。health inspect/register：

1. register/inspect 时用 `canonical_worker_path` canonicalize sibling parent 与 worker；
   missing/non-file/symlink 或 canonical worker parent 不等于 canonical sibling parent -> typed
   `WorkerBinaryUnavailable`。
2. `Identity` 获取 current SID。
3. 用 SID + canonical path 构造 `ScheduledTaskSpec`。
4. `Inspect` outcome 映射：
```rust
match outcome {
    ScheduledTaskCommandOutcome::Missing => SaveBackupBackgroundRegistrationStatus::NotRegistered,
    ScheduledTaskCommandOutcome::PermissionRequired => SaveBackupBackgroundRegistrationStatus::PermissionRequired,
    ScheduledTaskCommandOutcome::ModuleUnavailable => SaveBackupBackgroundRegistrationStatus::UnsupportedPlatform,
    ScheduledTaskCommandOutcome::OwnershipConflict => return Err(SaveBackupBackgroundRegistryError::TaskOwnershipConflict),
    ScheduledTaskCommandOutcome::Found(actual) => match spec.compare(&actual) {
        ScheduledTaskSpecMatch::Exact => SaveBackupBackgroundRegistrationStatus::Registered,
        ScheduledTaskSpecMatch::OwnedDrift => SaveBackupBackgroundRegistrationStatus::ConfigurationDrift,
        ScheduledTaskSpecMatch::OwnershipConflict => return Err(SaveBackupBackgroundRegistryError::TaskOwnershipConflict),
    },
    ScheduledTaskCommandOutcome::Identity(_) | ScheduledTaskCommandOutcome::Completed => {
        return Err(SaveBackupBackgroundRegistryError::CommandInvalidOutput);
    }
}
```
read-back compare 不覆盖原始 `action_execute`：先要求它精确等于 spec canonical worker path，再用
`symlink_metadata` 拒绝 symlink，并 canonicalize 验证结果仍等于 spec。alias、路径不存在或
canonicalize 失败都视为 owned drift，不把 path 写进 error。helper 固定为：
```rust
fn canonical_worker_path(worker_path: &Path) -> SaveBackupBackgroundRegistryResult<PathBuf> {
    let parent = worker_path
        .parent()
        .ok_or(SaveBackupBackgroundRegistryError::WorkerBinaryUnavailable)?;
    let canonical_parent = std::fs::canonicalize(parent)
        .map_err(|_| SaveBackupBackgroundRegistryError::WorkerBinaryUnavailable)?;
    let metadata = std::fs::symlink_metadata(worker_path)
        .map_err(|_| SaveBackupBackgroundRegistryError::WorkerBinaryUnavailable)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(SaveBackupBackgroundRegistryError::WorkerBinaryUnavailable);
    }
    let canonical_worker = std::fs::canonicalize(worker_path)
        .map_err(|_| SaveBackupBackgroundRegistryError::WorkerBinaryUnavailable)?;
    if canonical_worker.parent() != Some(canonical_parent.as_path()) {
        return Err(SaveBackupBackgroundRegistryError::WorkerBinaryUnavailable);
    }
    Ok(canonical_worker)
}
```
private helper 的签名固定为：
```rust
struct RegistryInspection {
    status: SaveBackupBackgroundRegistrationStatus,
    spec: ScheduledTaskSpec,
}
pub(super) struct ScheduledTaskRegistry<R> {
    runner: R,
    worker_path: Option<PathBuf>,
}
impl<R: ScheduledTaskCommandRunner> ScheduledTaskRegistry<R> {
    pub(super) fn new(runner: R, worker_path: PathBuf) -> Self {
        Self::with_worker_path(runner, Some(worker_path))
    }
    pub(super) fn with_worker_path(runner: R, worker_path: Option<PathBuf>) -> Self {
        Self { runner, worker_path }
    }
    fn inspect_internal(&self) -> SaveBackupBackgroundRegistryResult<RegistryInspection>;
    fn inspect_expected(
        &self,
        spec: &ScheduledTaskSpec,
    ) -> SaveBackupBackgroundRegistryResult<SaveBackupBackgroundRegistrationStatus>;
    fn inspect_owned_raw(
        &self,
    ) -> SaveBackupBackgroundRegistryResult<(String, ScheduledTaskCommandOutcome)>;
}
```
public `inspect()` 只返回 `inspect_internal()?.status`；`register()` 使用完整
`RegistryInspection` 取得 write 所需 spec；`unregister()` 只使用 `inspect_owned_raw()`，因此
不会间接依赖 worker 文件。

- [ ] **Step 4: 实现幂等 register/update/unregister**

register algorithm：

```rust
let before = self.inspect_internal()?;
match before.status {
    Registered => return Ok(Registered),
    NotRegistered | ConfigurationDrift => {}
    PermissionRequired | UnsupportedPlatform | RegistrationFailed => return Ok(before.status),
}
match self.runner.run(ScheduledTaskCommand::Register(before.spec.clone()))? {
    ScheduledTaskCommandOutcome::Completed => {}
    ScheduledTaskCommandOutcome::PermissionRequired => return Ok(PermissionRequired),
    ScheduledTaskCommandOutcome::ModuleUnavailable => return Ok(UnsupportedPlatform),
    ScheduledTaskCommandOutcome::OwnershipConflict => {
        return Err(SaveBackupBackgroundRegistryError::TaskOwnershipConflict);
    }
    _ => return Err(SaveBackupBackgroundRegistryError::CommandInvalidOutput),
}
Ok(self.inspect_expected(&before.spec)?)
```
unregister 先执行 identity + raw inspect，只比较 task name/ownership marker，不 canonicalize 或
要求 worker 文件存在；`Missing` 直接成功，owned task 才允许 runner delete，随后使用
`inspect_owned_raw()` 返回的已派生 task name 直接执行一次 `Inspect`，read-back 必须 `Missing`，
不再次请求 Identity。post-delete `Missing` -> `NotRegistered`，permission/module -> 对应状态，
owned `Found` -> `RegistrationFailed`，foreign `Found` -> typed ownership conflict；Identity/
Completed/OwnershipConflict outcome -> `CommandInvalidOutput`。delete 只接受 `Completed`；其余
pre-delete failure 不调用 delete。fake tests 覆盖 unexpected outcome，不把损坏输出误报成功。

`windows.rs` 的 public wrapper 只负责 sibling locator 和 trait delegation；generic lifecycle 留在
`registry.rs`：

```rust
pub struct WindowsScheduledTaskRegistry {
    inner: ScheduledTaskRegistry<PowerShellScheduledTaskCommandRunner>,
}
impl WindowsScheduledTaskRegistry {
    pub fn from_current_exe() -> Self {
        let worker = std::env::current_exe()
            .ok()
            .and_then(|current_exe| current_exe.parent().map(|parent| parent.to_path_buf()))
            .map(|parent| parent.join("hmm-save-backup-worker.exe"));
        Self {
            inner: ScheduledTaskRegistry::with_worker_path(
                PowerShellScheduledTaskCommandRunner,
                worker,
            ),
        }
    }
}
impl SaveBackupBackgroundRegistry for WindowsScheduledTaskRegistry {
    fn inspect(&self) -> SaveBackupBackgroundRegistryResult<SaveBackupBackgroundRegistrationStatus> {
        self.inner.inspect()
    }
    fn register(&self) -> SaveBackupBackgroundRegistryResult<SaveBackupBackgroundRegistrationStatus> {
        self.inner.register()
    }
    fn unregister(&self) -> SaveBackupBackgroundRegistryResult<SaveBackupBackgroundRegistrationStatus> {
        self.inner.unregister()
    }
}
```
locator 为 None 时 inspect/register 返回 typed `WorkerBinaryUnavailable`，不得执行
PowerShell inspect/register；unregister 仍执行 identity + raw ownership inspect。fake lifecycle
补测 None locator 下 inspect/register fail closed、unregister owned/missing 仍成功。

- [ ] **Step 5: 添加默认不运行的 Windows smoke harness**

在 private module tests 中添加：

```rust
#[cfg(target_os = "windows")]
#[test]
#[ignore = "creates a real user Scheduled Task; disposable Windows account/VM only"]
fn windows_scheduled_task_registry_smoke() {
    assert_eq!(
        std::env::var("HMM_RUN_WINDOWS_SCHEDULED_TASK_SMOKE").as_deref(),
        Ok("1"),
        "explicit smoke authorization is required",
    );
    let worker_path = std::env::var_os("HMM_WINDOWS_SMOKE_WORKER_PATH")
        .map(PathBuf::from)
        .expect("test-only worker path is required");
    let registry = ScheduledTaskRegistry::new(PowerShellScheduledTaskCommandRunner, worker_path);
    assert_eq!(
        registry.inspect().expect("initial inspect"),
        SaveBackupBackgroundRegistrationStatus::NotRegistered,
        "smoke refuses to overwrite a pre-existing task",
    );
    let body = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        assert_eq!(registry.register().expect("register"), SaveBackupBackgroundRegistrationStatus::Registered);
        assert_eq!(registry.inspect().expect("inspect"), SaveBackupBackgroundRegistrationStatus::Registered);
        assert_eq!(registry.register().expect("idempotent register"), SaveBackupBackgroundRegistrationStatus::Registered);
        if std::env::var("HMM_WINDOWS_SMOKE_WAIT_FOR_TRIGGER").as_deref() == Ok("1") {
            println!("Run the registered task in Task Scheduler, verify the heartbeat in the second terminal, then press Enter.");
            let mut acknowledgement = String::new();
            std::io::stdin().read_line(&mut acknowledgement).expect("read smoke acknowledgement");
            assert_eq!(registry.inspect().expect("post-trigger inspect"), SaveBackupBackgroundRegistrationStatus::Registered);
        }
    }));
    let first_cleanup = registry.unregister();
    let second_cleanup = registry.unregister();
    let final_inspect = registry.inspect();
    assert_eq!(first_cleanup.expect("first cleanup"), SaveBackupBackgroundRegistrationStatus::NotRegistered);
    assert_eq!(second_cleanup.expect("idempotent cleanup"), SaveBackupBackgroundRegistrationStatus::NotRegistered);
    assert_eq!(final_inspect.expect("cleanup read-back"), SaveBackupBackgroundRegistrationStatus::NotRegistered);
    if let Err(payload) = body {
        std::panic::resume_unwind(payload);
    }
}
#[cfg(target_os = "windows")]
#[test]
#[ignore = "cleanup for an explicitly authorized disposable Scheduled Task smoke"]
fn windows_scheduled_task_registry_cleanup_smoke() {
    assert_eq!(
        std::env::var("HMM_RUN_WINDOWS_SCHEDULED_TASK_SMOKE").as_deref(),
        Ok("1"),
        "explicit smoke authorization is required",
    );
    let worker_path = std::env::var_os("HMM_WINDOWS_SMOKE_WORKER_PATH")
        .map(PathBuf::from)
        .expect("test-only worker path is required");
    let registry = ScheduledTaskRegistry::new(PowerShellScheduledTaskCommandRunner, worker_path);
    assert_eq!(
        registry.unregister().expect("first cleanup"),
        SaveBackupBackgroundRegistrationStatus::NotRegistered,
    );
    assert_eq!(
        registry.unregister().expect("idempotent cleanup"),
        SaveBackupBackgroundRegistrationStatus::NotRegistered,
    );
}
```
这些 env 值只存在于 `#[cfg(test)]` ignored harness，不进入 production registry/CLI/DTO。
初始状态不是 not_registered 时立即停止且不删除，避免碰触已有任务。`catch_unwind` 保证普通
assert/panic 路径也执行两次幂等 unregister 和最终 read-back；进程被强制终止仍可能跳过
Rust cleanup，因此人工 smoke 文档必须保留独立的最终 Task Scheduler 检查。
如果 test process 被强制终止并留下 owned task，只能运行上述显式授权的 cleanup-only test；
它不接受 task name，内部 raw inspect 会再次验证 ownership marker，且 unregister 不要求 worker
文件存在。ownership conflict 时必须停止并交由人工调查，不能调用 `schtasks /Delete`。

- [ ] **Step 6: 运行 GREEN、fallback 和公开 API 检查**

Run:
```powershell
cargo test -p hmm-infra save_backup_background_registry::tests
cargo test -p hmm-infra --test save_backup_background_registry
cargo check -p hmm-infra
cargo clippy -p hmm-infra --all-targets -- -D warnings
```
Expected: 全部 PASS；fake tests 在不创建真实 Scheduled Task 的情况下覆盖完整生命周期。

- [ ] **Step 7: Commit**

```powershell
git add src-tauri/crates/hmm-infra/src/save_backup_background_registry/registry.rs src-tauri/crates/hmm-infra/src/save_backup_background_registry/windows.rs src-tauri/crates/hmm-infra/src/save_backup_background_registry/mod.rs src-tauri/crates/hmm-infra/src/save_backup_background_registry/tests.rs src-tauri/crates/hmm-infra/src/lib.rs src-tauri/crates/hmm-infra/tests/save_backup_background_registry.rs
git commit -m "feat: register Windows save backup task"
```
---

### Task 4: 拆分 Worker Heartbeat 事实并迁移 SQLite

**Files:**
- Modify: `src-tauri/crates/hmm-core/src/save_backup.rs`
- Modify: `src-tauri/crates/hmm-app/src/save_backup_background_worker.rs`
- Modify: `src-tauri/crates/hmm-app/src/save_backup_scheduler.rs`
- Modify: `src-tauri/crates/hmm-app/tests/save_backup_background_worker.rs`
- Modify: `src-tauri/crates/hmm-app/tests/save_backup_scheduler.rs`
- Modify: `src-tauri/crates/hmm-app/tests/save_backup_task.rs`
- Modify: `src-tauri/crates/hmm-infra/src/sqlite/migrations.rs`
- Create: `src-tauri/crates/hmm-infra/src/sqlite/migrations/007_save_backup_worker_heartbeat.sql`
- Modify: `src-tauri/crates/hmm-infra/src/sqlite/save_backup_scheduler_repository.rs`
- Modify: `src-tauri/crates/hmm-infra/tests/save_backup_scheduler_repository.rs`
- Modify: `src-tauri/src/save_backup_dto.rs`

**Interfaces:**
- Produces: `SaveBackupSchedulerState::worker_heartbeat_at: Option<u128>`。
- Produces: `SaveBackupWorkerHeartbeat { game_id, profile_id, worker_instance_id, heartbeat_at }`，不再包含 protection status。
- Preserves: `last_checked_at` 只表示 scheduler check；lease fields 的 owner-scoped 更新规则不变。

- [ ] **Step 1: 写 RED 测试，锁住 heartbeat 隔离**

将 infra heartbeat 测试改成以下断言；修改实现前应因字段不存在/旧字段仍必需而编译失败：

```rust
#[test]
fn worker_heartbeat_updates_only_worker_health_fields() {
    let (_temp, repo) = scheduler_repo();
    repo.upsert_state(&sample_state()).expect("seed state");
    repo.record_worker_heartbeat(SaveBackupWorkerHeartbeat {
        game_id: GameId::mhw(),
        profile_id: ProfileId::new("default"),
        worker_instance_id: "worker-b".to_owned(),
        heartbeat_at: 1_234,
    })
    .expect("heartbeat can be saved");
    let loaded = repo
        .get_state(&GameId::mhw(), &ProfileId::new("default"))
        .expect("load state")
        .expect("state exists");
    assert_eq!(loaded.worker_instance_id.as_deref(), Some("worker-b"));
    assert_eq!(loaded.worker_heartbeat_at, Some(1_234));
    assert_eq!(loaded.last_checked_at, Some(10));
    assert_eq!(loaded.background_status, SaveBackupBackgroundProtectionStatus::TrayOnly);
    assert_eq!(loaded.lease_owner, None);
}
```
在 `migrations.rs` 的 unit tests 中加入真实 006 -> 007 升级断言，不只检查列名：

```rust
#[test]
fn scheduler_migration_adds_nullable_worker_heartbeat() {
    let mut conn = rusqlite::Connection::open_in_memory().expect("open in-memory db");
    let migrations = migrations();
    migrations.to_version(&mut conn, 6).expect("migrate through 006");
    conn.execute(
        "INSERT INTO save_backup_scheduler_state (
            game_id, profile_id, enabled, background_protection_enabled,
            background_status, last_checked_at, worker_instance_id, updated_at
         ) VALUES (?1, ?2, 1, 1, 'tray_only', ?3, ?4, ?3)",
        rusqlite::params!["mhw", "legacy-profile", 1_234_i64, "legacy-worker"],
    )
    .expect("insert legacy scheduler row");
    migrations.to_latest(&mut conn).expect("migrate through 007");
    let heartbeat: Option<i64> = conn
        .query_row(
            "SELECT worker_heartbeat_at
             FROM save_backup_scheduler_state
             WHERE game_id = 'mhw' AND profile_id = 'legacy-profile'",
            [],
            |row| row.get(0),
        )
        .expect("read migrated heartbeat");
    assert_eq!(heartbeat, None, "migration must not forge heartbeat from last_checked_at");
}
```
同一 integration test 文件增加一个默认 ignored、严格只读的人工 heartbeat probe；它不调用
`open_database`（避免隐式 migration），只在一次性环境的第二终端读取 operator 显式提供的
synthetic DB/profile：

```rust
#[cfg(target_os = "windows")]
#[test]
#[ignore = "reads disposable smoke AppData after a real Scheduled Task trigger"]
fn windows_smoke_probe_sees_fresh_worker_heartbeat() {
    assert_eq!(
        std::env::var("HMM_RUN_WINDOWS_SCHEDULED_TASK_SMOKE").as_deref(),
        Ok("1"),
        "explicit smoke authorization is required",
    );
    let database_path = PathBuf::from(
        std::env::var_os("HMM_WINDOWS_SMOKE_DATABASE_PATH")
            .expect("disposable smoke database path is required"),
    );
    let profile_id = std::env::var("HMM_WINDOWS_SMOKE_PROFILE_ID")
        .expect("synthetic smoke profile id is required");
    let conn = rusqlite::Connection::open_with_flags(
        database_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .expect("open disposable database read-only");
    let heartbeat: Option<i64> = conn
        .query_row(
            "SELECT worker_heartbeat_at FROM save_backup_scheduler_state
             WHERE game_id = 'mhw' AND profile_id = ?1",
            [&profile_id],
            |row| row.get(0),
        )
        .expect("synthetic scheduler state exists");
    let heartbeat = heartbeat.expect("worker heartbeat exists") as u128;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock is after epoch")
        .as_millis();
    assert!(heartbeat <= now);
    assert!(now - heartbeat <= 45 * 60_000, "worker heartbeat is stale");
}
```
probe 不输出 DB path、profile id 或 heartbeat 原值。future/stale 分支仍只用 Task 5 fixed clock
自动化测试验证，不修改测试机时钟或数据库。

- [ ] **Step 2: 运行 RED 测试**

Run:
```powershell
cargo test -p hmm-infra --test save_backup_scheduler_repository worker_heartbeat
cargo test -p hmm-infra --test save_backup_scheduler_repository scheduler_migration
```
Expected: 至少一个命令 FAIL，错误包含 `heartbeat_at` 或 `worker_heartbeat_at` 未定义；不能是 fixture/真实路径错误。

- [ ] **Step 3: 实现领域字段和 migration**

将 heartbeat/state 更新为：

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaveBackupWorkerHeartbeat {
    pub game_id: GameId,
    pub profile_id: ProfileId,
    pub worker_instance_id: String,
    pub heartbeat_at: u128,
}
```
在 `SaveBackupSchedulerState` 的 `worker_instance_id` 后加入：

```rust
pub worker_heartbeat_at: Option<u128>,
```
创建 migration：

```sql
ALTER TABLE save_backup_scheduler_state
ADD COLUMN worker_heartbeat_at INTEGER;
```
在 `migrations.rs` 的 `006` 后注册 `007_save_backup_worker_heartbeat.sql`。旧行保持 null，禁止迁移时填充 `last_checked_at`。

- [ ] **Step 4: 更新 repository SQL 和所有构造点**

repository 的 heartbeat 写入必须严格为：

```rust
fn record_worker_heartbeat(&self, heartbeat: SaveBackupWorkerHeartbeat) -> Result<()> {
    let conn = self.lock_db()?;
    conn.execute(
        "UPDATE save_backup_scheduler_state
         SET worker_instance_id = ?3,
             worker_heartbeat_at = ?4,
             updated_at = ?4
         WHERE game_id = ?1 AND profile_id = ?2",
        params![
            heartbeat.game_id.as_str(),
            heartbeat.profile_id.as_str(),
            heartbeat.worker_instance_id,
            to_i64(heartbeat.heartbeat_at),
        ],
    )
    .context("failed to record save backup worker heartbeat")?;
    Ok(())
}
```
同步 INSERT/SELECT/transaction UPDATE 的列与索引。所有 `SaveBackupSchedulerState` literal
显式加入 `worker_heartbeat_at`；scheduler check 复制 existing 值：

- INSERT/UPSERT column order 在 `worker_instance_id` 后插入 `worker_heartbeat_at`，总参数从
  `?1..?15` 变为 `?1..?16`；conflict update 同时写
  `worker_heartbeat_at = excluded.worker_heartbeat_at`。
- SELECT / `row_to_state` 的 zero-based index 固定为：`worker_instance_id=11`、
  `worker_heartbeat_at=12`、`lease_owner=13`、`lease_expires_at=14`、`updated_at=15`。
- owner-scoped transaction UPDATE 在 `worker_instance_id = ?12` 后加入
  `worker_heartbeat_at = ?13`，lease/updated 参数顺延到 `?14/?15/?16`；WHERE game/profile 不变。
- `worker_heartbeat_at` 使用 `optional_i64_to_u128` 读取，`Option<u128>::map(to_i64)` 写入。

```rust
worker_heartbeat_at: existing.as_ref().and_then(|state| state.worker_heartbeat_at),
```
worker 写入改为：

```rust
SaveBackupWorkerHeartbeat {
    game_id: game_id.clone(),
    profile_id: profile_id.clone(),
    worker_instance_id: worker_instance_id.to_owned(),
    heartbeat_at: check.checked_at,
}
```
- [ ] **Step 5: 运行 GREEN 与回归**

Run:
```powershell
cargo test -p hmm-infra --test save_backup_scheduler_repository
cargo test -p hmm-app --test save_backup_background_worker
cargo test -p hmm-app --test save_backup_scheduler
cargo test -p hmm-app --test save_backup_task
cargo test -p hmm-tauri save_backup
```
Expected: 全部 PASS；heartbeat 测试证明 `last_checked_at`、`background_status` 和 lease 未被覆盖。

- [ ] **Step 6: Commit**

```powershell
git add src-tauri/crates/hmm-core/src/save_backup.rs src-tauri/crates/hmm-app/src/save_backup_background_worker.rs src-tauri/crates/hmm-app/src/save_backup_scheduler.rs src-tauri/crates/hmm-app/tests/save_backup_background_worker.rs src-tauri/crates/hmm-app/tests/save_backup_scheduler.rs src-tauri/crates/hmm-app/tests/save_backup_task.rs src-tauri/crates/hmm-infra/src/sqlite/migrations.rs src-tauri/crates/hmm-infra/src/sqlite/migrations/007_save_backup_worker_heartbeat.sql src-tauri/crates/hmm-infra/src/sqlite/save_backup_scheduler_repository.rs src-tauri/crates/hmm-infra/tests/save_backup_scheduler_repository.rs src-tauri/src/save_backup_dto.rs
git commit -m "refactor: split save backup worker heartbeat"
```
---

### Task 5: 新增后台注册生命周期与健康派生服务

**Files:**
- Create: `src-tauri/crates/hmm-app/src/save_backup_background.rs`
- Create: `src-tauri/crates/hmm-app/tests/save_backup_background.rs`
- Modify: `src-tauri/crates/hmm-app/src/lib.rs`

**Interfaces:**
- Produces: `SAVE_BACKUP_BACKGROUND_HEARTBEAT_TTL_MILLIS: u128 = 2_700_000`。
- Produces: `SaveBackupBackgroundStatus { scheduler_state, status, last_error_code }`。
- Produces: `SaveBackupBackgroundRegistrationResult { status, error_code }`。
- Produces: `SaveBackupBackgroundService::{status, register, unregister}`。
- Consumes: Task 1 的 `ConfigurationDrift` / registry schema constant 和 Task 4 的 `worker_heartbeat_at`。

- [ ] **Step 1: 写 fail-closed 状态矩阵 RED 测试**

使用一个 table-driven 测试覆盖全部优先级：

```rust
#[test]
fn protection_status_requires_enabled_exact_registration_and_fresh_heartbeat() {
    let now = 3_000_000_u128;
    let missing = Harness::new(now, SaveBackupBackgroundRegistrationStatus::Registered, None);
    assert_eq!(missing.service.status(&GameId::mhw(), &ProfileId::new("default")).expect("status").status, SaveBackupBackgroundProtectionStatus::NotEnabled);
    let cases = [
        (false, false, SaveBackupBackgroundRegistrationStatus::Registered, Some(now), SaveBackupBackgroundProtectionStatus::NotEnabled, None),
        (true, false, SaveBackupBackgroundRegistrationStatus::Registered, Some(now), SaveBackupBackgroundProtectionStatus::TrayOnly, None),
        (true, true, SaveBackupBackgroundRegistrationStatus::UnsupportedPlatform, Some(now), SaveBackupBackgroundProtectionStatus::UnsupportedPlatform, Some("save_backup_background_unsupported_platform")),
        (true, true, SaveBackupBackgroundRegistrationStatus::PermissionRequired, Some(now), SaveBackupBackgroundProtectionStatus::PermissionRequired, Some("save_backup_background_permission_required")),
        (true, true, SaveBackupBackgroundRegistrationStatus::NotRegistered, Some(now), SaveBackupBackgroundProtectionStatus::RegistrationFailed, Some("save_backup_background_not_registered")),
        (true, true, SaveBackupBackgroundRegistrationStatus::ConfigurationDrift, Some(now), SaveBackupBackgroundProtectionStatus::RegistrationFailed, Some("save_backup_background_configuration_drift")),
        (true, true, SaveBackupBackgroundRegistrationStatus::RegistrationFailed, Some(now), SaveBackupBackgroundProtectionStatus::RegistrationFailed, Some("save_backup_background_registration_failed")),
        (true, true, SaveBackupBackgroundRegistrationStatus::Registered, None, SaveBackupBackgroundProtectionStatus::WorkerUnhealthy, Some("save_backup_background_worker_unhealthy")),
        (true, true, SaveBackupBackgroundRegistrationStatus::Registered, Some(now + 1), SaveBackupBackgroundProtectionStatus::WorkerUnhealthy, Some("save_backup_background_worker_unhealthy")),
        (true, true, SaveBackupBackgroundRegistrationStatus::Registered, Some(now - 2_700_001), SaveBackupBackgroundProtectionStatus::WorkerUnhealthy, Some("save_backup_background_worker_unhealthy")),
        (true, true, SaveBackupBackgroundRegistrationStatus::Registered, Some(now - 2_700_000), SaveBackupBackgroundProtectionStatus::Protected, None),
    ];
    for (enabled, protection_enabled, registration, heartbeat, expected, expected_error) in cases {
        let harness = Harness::new(now, registration, Some(sample_state(enabled, protection_enabled, heartbeat)));
        let status = harness.service.status(&GameId::mhw(), &ProfileId::new("default")).expect("status");
        assert_eq!(status.status, expected);
        assert_eq!(status.last_error_code.as_deref(), expected_error);
    }
}
struct Harness {
    service: SaveBackupBackgroundService,
}
impl Harness {
    fn new(
        now: u128,
        registration: SaveBackupBackgroundRegistrationStatus,
        state: Option<SaveBackupSchedulerState>,
    ) -> Self {
        Self {
            service: SaveBackupBackgroundService::new(
                Arc::new(FakeRegistry::for_inspect(Ok(registration))),
                Arc::new(FakeSchedulerRepository(Mutex::new(state))),
                Arc::new(RecordingAuditLog::default()),
                Arc::new(FixedClock(now)),
            ),
        }
    }
}
fn sample_state(
    enabled: bool,
    background_protection_enabled: bool,
    worker_heartbeat_at: Option<u128>,
) -> SaveBackupSchedulerState {
    SaveBackupSchedulerState {
        game_id: GameId::mhw(),
        profile_id: ProfileId::new("default"),
        enabled,
        background_protection_enabled,
        background_status: SaveBackupBackgroundProtectionStatus::TrayOnly,
        last_checked_at: Some(1_000),
        last_attempt_at: None,
        last_success_at: None,
        next_due_at: Some(4_000_000),
        pending_reason: None,
        last_error_code: None,
        worker_instance_id: worker_heartbeat_at.map(|_| "worker-a".to_owned()),
        worker_heartbeat_at,
        lease_owner: None,
        lease_expires_at: None,
        updated_at: 1_000,
    }
}
struct FixedClock(u128);
impl AppClock for FixedClock {
    fn now_unix_millis(&self) -> anyhow::Result<u128> { Ok(self.0) }
}
struct FakeRegistry {
    inspect_results: Mutex<VecDeque<SaveBackupBackgroundRegistryResult<SaveBackupBackgroundRegistrationStatus>>>,
    register_results: Mutex<VecDeque<SaveBackupBackgroundRegistryResult<SaveBackupBackgroundRegistrationStatus>>>,
    unregister_results: Mutex<VecDeque<SaveBackupBackgroundRegistryResult<SaveBackupBackgroundRegistrationStatus>>>,
}
impl FakeRegistry {
    fn for_inspect(result: SaveBackupBackgroundRegistryResult<SaveBackupBackgroundRegistrationStatus>) -> Self {
        Self {
            inspect_results: Mutex::new(VecDeque::from([result])),
            register_results: Mutex::new(VecDeque::new()),
            unregister_results: Mutex::new(VecDeque::new()),
        }
    }
}
impl SaveBackupBackgroundRegistry for FakeRegistry {
    fn inspect(&self) -> SaveBackupBackgroundRegistryResult<SaveBackupBackgroundRegistrationStatus> {
        self.inspect_results.lock().expect("inspect lock").pop_front().expect("inspect result")
    }
    fn register(&self) -> SaveBackupBackgroundRegistryResult<SaveBackupBackgroundRegistrationStatus> {
        self.register_results.lock().expect("register lock").pop_front().expect("register result")
    }
    fn unregister(&self) -> SaveBackupBackgroundRegistryResult<SaveBackupBackgroundRegistrationStatus> {
        self.unregister_results.lock().expect("unregister lock").pop_front().expect("unregister result")
    }
}
#[derive(Default)]
struct RecordingAuditLog(Mutex<Vec<AuditLogEvent>>);
impl AuditLogWriter for RecordingAuditLog {
    fn record(&self, event: AuditLogEvent) -> anyhow::Result<()> {
        self.0.lock().expect("audit lock").push(event);
        Ok(())
    }
}
struct FakeSchedulerRepository(Mutex<Option<SaveBackupSchedulerState>>);
impl SaveBackupSchedulerStateRepository for FakeSchedulerRepository {
    fn get_state(&self, _: &GameId, _: &ProfileId) -> anyhow::Result<Option<SaveBackupSchedulerState>> {
        Ok(self.0.lock().expect("state lock").clone())
    }
    fn upsert_state(&self, _: &SaveBackupSchedulerState) -> anyhow::Result<()> { panic!("unused") }
    fn acquire_due_lease(&self, _: SaveBackupSchedulerLeaseRequest) -> anyhow::Result<Option<SaveBackupSchedulerState>> { panic!("unused") }
    fn release_lease(&self, _: &GameId, _: &ProfileId, _: &str) -> anyhow::Result<()> { panic!("unused") }
    fn record_worker_heartbeat(&self, _: SaveBackupWorkerHeartbeat) -> anyhow::Result<()> { panic!("unused") }
}
```
扩展 `FakeRegistry` 记录调用顺序，并为 lifecycle case 排队 register/unregister 结果。
另写 lifecycle tests：register 返回 registered 后必须再 inspect；unregister 返回
not_registered 后必须再 inspect；ownership/configuration/permission 只写稳定 error code；
Audit Log fields 不包含 `path`、`sid`、`task_name`、`command`、`xml`。
再加入两条 error retention 断言：`save_backup_background_configuration_drift` 这类陈旧平台错误
在 `tray_only` / `protected` 输出中被清空，而 `save_backup_auto_skipped_game_running` 这类
scheduler/auto-backup 错误仍被保留。

- [ ] **Step 2: 运行 RED 测试**

Run:
```powershell
cargo test -p hmm-app --test save_backup_background
```
Expected: FAIL，错误包含 module/type `save_backup_background` 不存在。

- [ ] **Step 3: 实现 service 类型和状态派生**

新增以下 public shape：

```rust
pub const SAVE_BACKUP_BACKGROUND_HEARTBEAT_TTL_MILLIS: u128 = 45 * 60_000;
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaveBackupBackgroundStatus {
    pub scheduler_state: Option<SaveBackupSchedulerState>,
    pub status: SaveBackupBackgroundProtectionStatus,
    pub last_error_code: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaveBackupBackgroundRegistrationResult {
    pub status: SaveBackupBackgroundRegistrationStatus,
    pub error_code: Option<String>,
}
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum SaveBackupBackgroundServiceError {
    #[error("save backup scheduler state is unavailable")]
    SchedulerStateUnavailable,
    #[error("app clock is unavailable")]
    ClockUnavailable,
    #[error("audit log is unavailable")]
    AuditUnavailable,
}
impl SaveBackupBackgroundServiceError {
    pub fn code(self) -> &'static str {
        match self {
            Self::SchedulerStateUnavailable => "save_backup_scheduler_unavailable",
            Self::ClockUnavailable => "save_backup_clock_unavailable",
            Self::AuditUnavailable => "save_backup_background_audit_unavailable",
        }
    }
}
pub struct SaveBackupBackgroundService {
    registry: Arc<dyn SaveBackupBackgroundRegistry>,
    scheduler_state_repository: Arc<dyn SaveBackupSchedulerStateRepository>,
    audit_log: Arc<dyn AuditLogWriter>,
    clock: Arc<dyn AppClock>,
}
impl SaveBackupBackgroundService {
    pub fn new(
        registry: Arc<dyn SaveBackupBackgroundRegistry>,
        scheduler_state_repository: Arc<dyn SaveBackupSchedulerStateRepository>,
        audit_log: Arc<dyn AuditLogWriter>,
        clock: Arc<dyn AppClock>,
    ) -> Self {
        Self { registry, scheduler_state_repository, audit_log, clock }
    }
}
```
`status` 的实现顺序必须固定：

```rust
pub fn status(
    &self,
    game_id: &GameId,
    profile_id: &ProfileId,
) -> Result<SaveBackupBackgroundStatus, SaveBackupBackgroundServiceError> {
    let state = self.scheduler_state_repository
        .get_state(game_id, profile_id)
        .map_err(|_| SaveBackupBackgroundServiceError::SchedulerStateUnavailable)?;
    let Some(state) = state else {
        return Ok(status_result(None, SaveBackupBackgroundProtectionStatus::NotEnabled, None));
    };
    if !state.enabled {
        return Ok(status_result(Some(state), SaveBackupBackgroundProtectionStatus::NotEnabled, None));
    }
    if !state.background_protection_enabled {
        let error = retained_scheduler_error(state.last_error_code.clone());
        return Ok(status_result(Some(state), SaveBackupBackgroundProtectionStatus::TrayOnly, error));
    }
    let registration = match self.registry.inspect() {
        Ok(status) => status,
        Err(error) => {
            return Ok(status_result(
                Some(state),
                SaveBackupBackgroundProtectionStatus::RegistrationFailed,
                Some(error.code().to_owned()),
            ));
        }
    };
    if registration != SaveBackupBackgroundRegistrationStatus::Registered {
        let (status, code) = registration_failure(registration);
        return Ok(status_result(Some(state), status, Some(code.to_owned())));
    }
    let now = self.clock.now_unix_millis()
        .map_err(|_| SaveBackupBackgroundServiceError::ClockUnavailable)?;
    let fresh = state.worker_heartbeat_at
        .is_some_and(|heartbeat| heartbeat <= now && now - heartbeat <= SAVE_BACKUP_BACKGROUND_HEARTBEAT_TTL_MILLIS);
    if !fresh {
        return Ok(status_result(
            Some(state),
            SaveBackupBackgroundProtectionStatus::WorkerUnhealthy,
            Some("save_backup_background_worker_unhealthy".to_owned()),
        ));
    }
    let error = retained_scheduler_error(state.last_error_code.clone());
    Ok(status_result(Some(state), SaveBackupBackgroundProtectionStatus::Protected, error))
}
fn status_result(
    scheduler_state: Option<SaveBackupSchedulerState>,
    status: SaveBackupBackgroundProtectionStatus,
    last_error_code: Option<String>,
) -> SaveBackupBackgroundStatus {
    SaveBackupBackgroundStatus { scheduler_state, status, last_error_code }
}
fn retained_scheduler_error(error_code: Option<String>) -> Option<String> {
    error_code.filter(|code| !code.starts_with("save_backup_background_"))
}
fn registration_failure(
    status: SaveBackupBackgroundRegistrationStatus,
) -> (SaveBackupBackgroundProtectionStatus, &'static str) {
    match status {
        SaveBackupBackgroundRegistrationStatus::NotRegistered => (
            SaveBackupBackgroundProtectionStatus::RegistrationFailed,
            "save_backup_background_not_registered",
        ),
        SaveBackupBackgroundRegistrationStatus::ConfigurationDrift => (
            SaveBackupBackgroundProtectionStatus::RegistrationFailed,
            "save_backup_background_configuration_drift",
        ),
        SaveBackupBackgroundRegistrationStatus::RegistrationFailed => (
            SaveBackupBackgroundProtectionStatus::RegistrationFailed,
            "save_backup_background_registration_failed",
        ),
        SaveBackupBackgroundRegistrationStatus::PermissionRequired => (
            SaveBackupBackgroundProtectionStatus::PermissionRequired,
            "save_backup_background_permission_required",
        ),
        SaveBackupBackgroundRegistrationStatus::UnsupportedPlatform => (
            SaveBackupBackgroundProtectionStatus::UnsupportedPlatform,
            "save_backup_background_unsupported_platform",
        ),
        SaveBackupBackgroundRegistrationStatus::Registered => unreachable!("registered handled before failure mapping"),
    }
}
```
`registration_failure` 必须完整映射五个非 registered statuses；`NotRegistered`、
`ConfigurationDrift`、`RegistrationFailed`、`PermissionRequired`、`UnsupportedPlatform`
分别映射规格中的稳定 code。`Registered` 分支使用 `unreachable!`，因为调用方已排除。

- [ ] **Step 4: 实现 register/unregister read-back 和白名单审计**

register 只有在 `registry.register()` 和随后 `registry.inspect()` 都返回 `Registered`
时才成功；unregister 只有在 operation 和 read-back 都返回 `NotRegistered` 时成功。
registry typed error 映射为 public `RegistrationFailed`，并原样保留其稳定 `code()`；禁止把
PowerShell/CIM message、stdout/stderr 或 path 放入结果/审计。补测
`TaskOwnershipConflict`、`WorkerBinaryUnavailable`、`CommandTimeout`、
`CommandInvalidOutput`、`OperationFailed` 的 `last_error_code`。

public signatures 如下；private helper 精确签名为 `fn change_registration(&self, operation: RegistrationOperation) -> Result<SaveBackupBackgroundRegistrationResult, SaveBackupBackgroundServiceError>`：

```rust
pub fn register(
    &self,
) -> Result<SaveBackupBackgroundRegistrationResult, SaveBackupBackgroundServiceError> {
    self.change_registration(RegistrationOperation::Register)
}
pub fn unregister(
    &self,
) -> Result<SaveBackupBackgroundRegistrationResult, SaveBackupBackgroundServiceError> {
    self.change_registration(RegistrationOperation::Unregister)
}
#[derive(Debug, Clone, Copy)]
enum RegistrationOperation {
    Register,
    Unregister,
}
impl RegistrationOperation {
    fn expected_status(self) -> SaveBackupBackgroundRegistrationStatus {
        match self {
            Self::Register => SaveBackupBackgroundRegistrationStatus::Registered,
            Self::Unregister => SaveBackupBackgroundRegistrationStatus::NotRegistered,
        }
    }
}
```
`change_registration` 必须先读取 clock；clock 失败时不修改系统任务。随后执行对应 registry
operation。只有 operation status 等于 `expected_status()` 时才执行第二次 `inspect()`；read-back
仍等于 expected 才返回无 error 的 success result。其余 registration status 使用
`registration_failure` 映射；typed error 返回：

```rust
SaveBackupBackgroundRegistrationResult {
    status: SaveBackupBackgroundRegistrationStatus::RegistrationFailed,
    error_code: Some(error.code().to_owned()),
}
```
register 的 post-write read-back 若为 `ConfigurationDrift` / `PermissionRequired` /
`UnsupportedPlatform`，保留该 status 与对应稳定 error code；unregister 的 post-write read-back
只要不是 `NotRegistered`，统一返回 `RegistrationFailed` +
`save_backup_background_registration_failed`。无论 success/failure 都用预先取得的 timestamp 写
一次白名单 Audit Log；audit writer 失败返回 `AuditUnavailable`，调用方必须通过只读 status
重新确认实际系统状态，不能假设 operation 回滚。

Audit Log 固定为：

```rust
AuditLogEvent {
    timestamp_unix_millis,
    category: "save_backup".to_owned(),
    operation: "background_registration".to_owned(),
    result: if success { "success" } else { "failure" }.to_owned(),
    fields: BTreeMap::from([
        ("registration_status".to_owned(), status.as_str().to_owned()),
        (
            "task_schema_version".to_owned(),
            SAVE_BACKUP_BACKGROUND_REGISTRY_SCHEMA_VERSION.to_string(),
        ),
        ("error_code".to_owned(), error_code.unwrap_or_default()),
    ]),
}
```
测试用 fake registry 记录调用顺序，fake audit writer 保存 event；不要在 production
service 中加入 task name/path/SID 参数。

- [ ] **Step 5: 运行 GREEN、clippy 和边界搜索**

Run:
```powershell
cargo test -p hmm-app --test save_backup_background
cargo clippy -p hmm-app --all-targets -- -D warnings
$forbidden = rg -n "raw_path|task_name|worker_path|raw_sid|task_xml|command_line" src-tauri/crates/hmm-app/src/save_backup_background.rs 2>&1
if ($LASTEXITCODE -eq 0) { $forbidden; throw 'background service exposes platform detail' }
if ($LASTEXITCODE -ne 1) { throw 'background service boundary search failed' }
```
Expected: tests/clippy PASS；production `rg` 无输出；integration test 自身断言 Audit Log field
白名单和 forbidden key 不泄漏。

- [ ] **Step 6: Commit**

```powershell
git add src-tauri/crates/hmm-app/src/save_backup_background.rs src-tauri/crates/hmm-app/tests/save_backup_background.rs src-tauri/crates/hmm-app/src/lib.rs
git commit -m "feat: derive save backup background health"
```
---

### Task 6: 在 Tauri Composition Root 接入健康服务并保持 DTO 白名单

**Files:**
- Modify: `src-tauri/src/state.rs`
- Modify: `src-tauri/src/save_backup_commands.rs`
- Modify: `src-tauri/src/save_backup_dto.rs`
- Modify: `src-tauri/crates/hmm-app/src/save_backup_scheduler.rs`
- Modify: `src-tauri/crates/hmm-app/tests/save_backup_scheduler.rs`

**Interfaces:**
- Consumes: Task 5 的 `SaveBackupBackgroundService` / `SaveBackupBackgroundStatus`。
- Consumes: Task 3 的 Windows registry；non-Windows 使用 fallback。
- Produces: `AppState::save_backup_background: Arc<SaveBackupBackgroundService>`。
- Preserves: `get_save_backup_background_status({ gameId, profileId })` JSON shape。
- Produces: async command + `tauri::async_runtime::spawn_blocking`；15 秒 PowerShell timeout 不直接阻塞 command runtime。
- Removes: `SaveBackupAutoSchedulerService::background_status` passthrough，避免两个 status owner。

- [ ] **Step 1: 写 DTO 派生状态和敏感字段 RED 测试**

```rust
#[test]
fn background_status_dto_uses_derived_health_without_internal_fields() {
    let dto = SaveBackupBackgroundStatusDto::from_status(
        &GameId::mhw(),
        &ProfileId::new("default"),
        SaveBackupBackgroundStatus {
            scheduler_state: Some(SaveBackupSchedulerState {
                game_id: GameId::mhw(),
                profile_id: ProfileId::new("default"),
                enabled: true,
                background_protection_enabled: true,
                background_status: SaveBackupBackgroundProtectionStatus::TrayOnly,
                last_checked_at: Some(900),
                last_attempt_at: None,
                last_success_at: None,
                next_due_at: Some(2_000),
                pending_reason: None,
                last_error_code: None,
                worker_instance_id: Some("worker-private".to_owned()),
                worker_heartbeat_at: Some(1_000),
                lease_owner: Some("lease-private".to_owned()),
                lease_expires_at: Some(2_000),
                updated_at: 1_000,
            }),
            status: SaveBackupBackgroundProtectionStatus::Protected,
            last_error_code: None,
        },
    );
    let value = serde_json::to_value(dto).expect("serialize dto");
    assert_eq!(value["status"], "protected");
    let serialized = value.to_string();
    for forbidden in ["workerInstanceId", "workerHeartbeatAt", "leaseOwner", "leaseExpiresAt", "taskName", "workerPath", "sid", "xml"] {
        assert!(!serialized.contains(forbidden), "leaked {forbidden}");
    }
}
```
command 的 source boundary 不写成 self-reading unit test（forbidden string 会被测试字面量自身命中）；
Step 5 使用定向 `rg` 检查 `save_backup_commands.rs` 不含 `hmm_infra`、`PowerShell`、
`worker_path` 或 `task_name`。

- [ ] **Step 2: 运行 RED 测试**

Run:
```powershell
cargo test -p hmm-tauri save_backup_dto
cargo test -p hmm-tauri save_backup_commands
```
Expected: FAIL，错误包含 `from_status` 或 `save_backup_background` 不存在。

- [ ] **Step 3: 装配 platform registry 和 app service**

`state.rs` 按平台导入：

```rust
#[cfg(target_os = "windows")]
use hmm_infra::WindowsScheduledTaskRegistry;
#[cfg(not(target_os = "windows"))]
use hmm_infra::UnsupportedSaveBackupBackgroundRegistry;
```
在 repository/audit/clock 已创建后组装：

```rust
let save_backup_background_registry: Arc<dyn SaveBackupBackgroundRegistry> = {
    #[cfg(target_os = "windows")]
    {
        Arc::new(
            WindowsScheduledTaskRegistry::from_current_exe(),
        )
    }
    #[cfg(not(target_os = "windows"))]
    {
        Arc::new(UnsupportedSaveBackupBackgroundRegistry)
    }
};
let save_backup_background = Arc::new(SaveBackupBackgroundService::new(
    save_backup_background_registry,
    Arc::clone(&save_backup_scheduler_state_repository),
    Arc::clone(&audit_log_writer),
    Arc::new(SystemClock),
));
```
把 service 加入 `AppState`。构造 registry 不执行 PowerShell；只有显式
status/register/unregister 才调用 runner，因此 headless worker 启动不会查询系统任务。

- [ ] **Step 4: 切换 command 和 DTO mapper**

command 改为 async，并在取得 owned ids/service 后把同步平台查询移入 blocking pool：

```rust
#[tauri::command]
pub async fn get_save_backup_background_status(
    request: GetSaveBackupBackgroundStatusRequestDto,
    state: State<'_, AppState>,
) -> Result<SaveBackupBackgroundStatusDto, CommandErrorDto> {
    let game_id = parse_game_id(request.game_id)?;
    let profile_id = parse_profile_id(request.profile_id)?;
    let service = Arc::clone(&state.save_backup_background);
    let query_game_id = game_id.clone();
    let query_profile_id = profile_id.clone();
    let background = tauri::async_runtime::spawn_blocking(move || {
        service.status(&query_game_id, &query_profile_id)
    })
    .await
    .map_err(|_| CommandErrorDto {
        code: "save_backup_background_status_unavailable".to_owned(),
        message: "save backup background status is unavailable".to_owned(),
    })?
    .map_err(save_backup_background_error_to_command_error)?;
    Ok(SaveBackupBackgroundStatusDto::from_status(
        &game_id,
        &profile_id,
        background,
    ))
}
```
command error mapper 只用 stable code 和固定消息：

```rust
fn save_backup_background_error_to_command_error(
    error: SaveBackupBackgroundServiceError,
) -> CommandErrorDto {
    CommandErrorDto {
        code: error.code().to_owned(),
        message: "save backup background status is unavailable".to_owned(),
    }
}
```
补 unit test 覆盖 scheduler/clock 两个 status-query 错误码；`AuditUnavailable` 当前只由未暴露给
前端的 lifecycle method 返回，P7.2b 暴露 register/unregister command 时再加入对应 command
mapping，不提前新增 Tauri surface。

DTO mapper 用 `background.status` / `background.last_error_code`，其余时间/调度字段从
`background.scheduler_state` 白名单复制。state 为 None 时仍输出 not_enabled 的空摘要。

删除 `SaveBackupAutoSchedulerService::background_status` 及其三个 passthrough tests；scheduler
只负责 due/state/lease，不再拥有平台保护 health。

- [ ] **Step 5: 运行 GREEN 和 bridge 边界验证**

Run:
```powershell
cargo test -p hmm-tauri save_backup_dto
cargo test -p hmm-tauri save_backup_commands
cargo test -p hmm-app --test save_backup_scheduler
cargo check -p hmm-tauri
cargo clippy --workspace --all-targets -- -D warnings
$forbidden = rg -n "hmm_infra|PowerShell|worker_path|task_name" src-tauri/src/save_backup_commands.rs 2>&1
if ($LASTEXITCODE -eq 0) { $forbidden; throw 'Tauri command contains platform details' }
if ($LASTEXITCODE -ne 1) { throw 'Tauri command boundary search failed' }
```
Expected: tests/check/clippy 全部 PASS；`rg` 无输出；测试不得调用真实 ScheduledTasks module。

- [ ] **Step 6: Commit**

```powershell
git add src-tauri/src/state.rs src-tauri/src/save_backup_commands.rs src-tauri/src/save_backup_dto.rs src-tauri/crates/hmm-app/src/save_backup_scheduler.rs src-tauri/crates/hmm-app/tests/save_backup_scheduler.rs
git commit -m "feat: wire save backup background health"
```
---

### Task 7: 将 Headless Worker 作为 Tauri Sidecar 打包

**Files:**
- Create: `scripts/prepare-save-backup-worker-sidecar.mjs`
- Create: `scripts/prepare-save-backup-worker-sidecar.test.mjs`
- Modify: `package.json`
- Create: `src-tauri/tauri.windows.conf.json`
- Modify: `.gitignore`

**Interfaces:**
- Produces: `sidecarFileName(targetTriple)`、`hostTripleFromRustc(output)`、
  `targetDirectoryFromCargoMetadata(output)`、`buildProfile(args)` 和
  `resolveTargetTriple(explicitTarget, hostTarget, tauriArch)` 纯函数。
- Produces: pnpm scripts `prepare:save-backup-worker-sidecar` / `prepare:save-backup-worker-sidecar:dev`。
- Produces: Tauri external binary `binaries/hmm-save-backup-worker`。
- Preserves: base `tauri.conf.json` 的跨平台 hooks；externalBin 只在 Windows config 合并。
- Preserves: generated target-triple binaries 不进入 Git。

- [ ] **Step 1: 写 sidecar 命名 RED 测试**

```javascript
import test from "node:test";
import assert from "node:assert/strict";
import path from "node:path";
import {
  buildProfile,
  hostTripleFromRustc,
  resolveTargetTriple,
  sidecarFileName,
  targetDirectoryFromCargoMetadata,
} from "./prepare-save-backup-worker-sidecar.mjs";
test("parses rustc host triple", () => {
  assert.equal(
    hostTripleFromRustc("rustc 1.95.0\nhost: x86_64-pc-windows-msvc\n"),
    "x86_64-pc-windows-msvc",
  );
  assert.throws(() => hostTripleFromRustc("rustc 1.95.0\n"), /host triple/);
});
test("uses Tauri target-triple sidecar naming", () => {
  assert.equal(
    sidecarFileName("x86_64-pc-windows-msvc"),
    "hmm-save-backup-worker-x86_64-pc-windows-msvc.exe",
  );
  assert.equal(
    sidecarFileName("x86_64-unknown-linux-gnu"),
    "hmm-save-backup-worker-x86_64-unknown-linux-gnu",
  );
});
test("uses cargo metadata target directory and explicit profiles", () => {
  assert.equal(
    targetDirectoryFromCargoMetadata('{"target_directory":"D:/cargo-target"}'),
    path.normalize("D:/cargo-target"),
  );
  assert.equal(buildProfile([]), "release");
  assert.equal(buildProfile(["--debug"]), "debug");
  assert.throws(() => buildProfile(["--unknown"]), /unknown sidecar argument/);
});
test("rejects unsafe or conflicting target triple input", () => {
  assert.throws(() => sidecarFileName("../windows"), /invalid Rust target triple/);
  assert.throws(
    () => resolveTargetTriple("aarch64-pc-windows-msvc", "x86_64-pc-windows-msvc", "x86_64"),
    /does not match TAURI_ENV_ARCH/,
  );
});
```
- [ ] **Step 2: 运行 RED 测试**

Run:
```powershell
node --test scripts/prepare-save-backup-worker-sidecar.test.mjs
```
Expected: FAIL with `ERR_MODULE_NOT_FOUND`。

- [ ] **Step 3: 实现 deterministic sidecar helper**

```javascript
import { spawnSync } from "node:child_process";
import { copyFileSync, mkdirSync, statSync } from "node:fs";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
export function hostTripleFromRustc(output) {
  const match = /^host:\s+([^\s]+)$/m.exec(output);
  if (!match) throw new Error("rustc host triple is unavailable");
  return match[1];
}
export function targetDirectoryFromCargoMetadata(output) {
  const metadata = JSON.parse(output);
  if (typeof metadata.target_directory !== "string" || metadata.target_directory.length === 0) {
    throw new Error("Cargo target directory is unavailable");
  }
  return path.normalize(metadata.target_directory);
}
export function buildProfile(args) {
  if (args.length === 0) return "release";
  if (args.length === 1 && args[0] === "--debug") return "debug";
  throw new Error("unknown sidecar argument");
}
export function sidecarFileName(targetTriple) {
  if (!/^[A-Za-z0-9_.-]+$/.test(targetTriple)) {
    throw new Error("invalid Rust target triple");
  }
  const extension = targetTriple.includes("windows") ? ".exe" : "";
  return `hmm-save-backup-worker-${targetTriple}${extension}`;
}
export function resolveTargetTriple(explicitTarget, hostTarget, tauriArch) {
  const target = explicitTarget ?? hostTarget;
  sidecarFileName(target);
  if (tauriArch && !target.startsWith(`${tauriArch}-`)) {
    throw new Error("sidecar target does not match TAURI_ENV_ARCH");
  }
  return target;
}
function run(command, args, options = {}) {
  const result = spawnSync(command, args, {
    cwd: repoRoot,
    stdio: "inherit",
    ...options,
  });
  if (result.error) throw result.error;
  if (result.status !== 0) throw new Error(`${command} exited with ${result.status}`);
}
function capture(command, args) {
  const result = spawnSync(command, args, {
    cwd: repoRoot,
    encoding: "utf8",
  });
  if (result.error || result.status !== 0) {
    throw result.error ?? new Error(`${command} exited with ${result.status}`);
  }
  return result.stdout ?? "";
}
export function prepareSidecar(args = []) {
  const profile = buildProfile(args);
  const hostTarget = hostTripleFromRustc(capture("rustc", ["-vV"]));
  const explicitTarget = process.env.HMM_SIDECAR_TARGET_TRIPLE
    ?? process.env.CARGO_BUILD_TARGET;
  const targetTriple = resolveTargetTriple(
    explicitTarget,
    hostTarget,
    process.env.TAURI_ENV_ARCH,
  );
  const cargoArgs = [
    "build", "-p", "hmm-tauri", "--bin", "hmm-save-backup-worker",
    "--target", targetTriple,
  ];
  if (profile === "release") cargoArgs.push("--release");
  run("cargo", cargoArgs);
  const targetDirectory = targetDirectoryFromCargoMetadata(
    capture("cargo", ["metadata", "--format-version", "1", "--no-deps"]),
  );
  const extension = targetTriple.includes("windows") ? ".exe" : "";
  const source = path.join(
    targetDirectory,
    targetTriple,
    profile,
    `hmm-save-backup-worker${extension}`,
  );
  if (!statSync(source).isFile()) {
    throw new Error("worker sidecar build output is missing");
  }
  const destinationDir = path.join(repoRoot, "src-tauri", "binaries");
  mkdirSync(destinationDir, { recursive: true });
  copyFileSync(source, path.join(destinationDir, sidecarFileName(targetTriple)));
}
if (
  process.argv[1]
  && import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href
) {
  prepareSidecar(process.argv.slice(2));
}
```
- [ ] **Step 4: 接入 pnpm/Tauri 并忽略生成物**

`package.json` scripts 加入：

```json
"prepare:save-backup-worker-sidecar": "node scripts/prepare-save-backup-worker-sidecar.mjs",
"prepare:save-backup-worker-sidecar:dev": "node scripts/prepare-save-backup-worker-sidecar.mjs --debug"
```
新建 `tauri.windows.conf.json`，只在 Windows 合并 sidecar 和 hooks；base `tauri.conf.json` 不改：

```json
{
  "build": {
    "beforeDevCommand": "corepack pnpm run prepare:save-backup-worker-sidecar:dev && corepack pnpm dev",
    "beforeBuildCommand": "corepack pnpm build && corepack pnpm run prepare:save-backup-worker-sidecar"
  },
  "bundle": {
    "externalBin": ["binaries/hmm-save-backup-worker"]
  }
}
```
native build 默认使用 `rustc -vV` host triple。cross-target packaging 必须同时设置受控构建变量
（例如 `$env:HMM_SIDECAR_TARGET_TRIPLE = "aarch64-pc-windows-msvc"`，并向
`tauri build --target` 传入同一字符串）；helper 会校验它与
`TAURI_ENV_ARCH` 一致。该变量只决定 build artifact name/target，不进入 runtime registry、CLI、
DTO 或 Scheduled Task action。

`.gitignore` 加入：

```text
src-tauri/binaries/
```
- [ ] **Step 5: 运行 GREEN、构建 helper 与 ignore 检查**

Run:
```powershell
node --test scripts/prepare-save-backup-worker-sidecar.test.mjs
cmd /c corepack pnpm run prepare:save-backup-worker-sidecar:dev
cmd /c corepack pnpm run prepare:save-backup-worker-sidecar
cargo check -p hmm-tauri --bin hmm-save-backup-worker
$hostTriple = ((rustc -vV | Select-String '^host:').Line -replace '^host:\s*', '')
$extension = if ($hostTriple -like '*windows*') { '.exe' } else { '' }
$sidecar = "src-tauri/binaries/hmm-save-backup-worker-$hostTriple$extension"
git check-ignore $sidecar
if (-not (Test-Path -LiteralPath $sidecar -PathType Leaf)) { throw 'prepared sidecar is missing' }
git diff --exit-code -- src-tauri/tauri.conf.json
```
Expected: tests/build/check PASS；`git check-ignore` 输出 sidecar 路径；`git status --short`
不显示生成 binary；base config 无 diff。debug/release 两次准备都从 `cargo metadata`
报告的 target directory 读取产物。

- [ ] **Step 6: 验证实际 bundle 包含 sidecar**

Run:
```powershell
cmd /c corepack pnpm run tauri:build
```
Expected: Tauri build 成功；Windows 安装产物的应用目录包含
`hmm-save-backup-worker.exe`。仅看到例如
`src-tauri/binaries/hmm-save-backup-worker-x86_64-pc-windows-msvc.exe` 不算 bundle 通过；必须在
一次性 Windows 测试账户/VM 安装生成的 NSIS/MSI 之一，并确认主程序 sibling 是无 triple 后缀的
worker。另一个 installer 格式和自动卸载清理仍属于 release packaging gate。如果本机缺少
WiX/NSIS 工具或下载条件，记录未执行原因，但在完成 P7.2a Windows runtime acceptance 前必须
在 Windows packaging 环境补跑。

- [ ] **Step 7: Commit**

```powershell
git add .gitignore package.json src-tauri/tauri.windows.conf.json scripts/prepare-save-backup-worker-sidecar.mjs scripts/prepare-save-backup-worker-sidecar.test.mjs
git commit -m "build: bundle save backup worker sidecar"
```
---

### Task 8: 同步契约、测试文档并执行 Windows Smoke Gate

**Files:**
- Create: `docs/testing/windows-save-backup-scheduled-task-smoke.md`
- Modify: `docs/SAVE_BACKUP_BACKGROUND_AUTOMATION_DESIGN.md`
- Modify: `docs/SAVE_BACKUP_BACKGROUND_SCHEDULER_CORE_PLAN.md`
- Modify: `docs/FRONTEND_BACKEND_CONTRACT.md`
- Modify: `docs/TESTING.md`
- Modify: `docs/LOGGING.md`
- Modify: `docs/release/发布与产物说明.md`
- Modify: `docs/release/构建发布与脚本说明.md`
- Modify: `TODO.md`
- Modify: `docs/superpowers/specs/2026-07-10-save-backup-windows-scheduled-task-core-design.md`
- Modify: `docs/superpowers/plans/2026-07-10-save-backup-windows-scheduled-task-core-implementation.md`

**Interfaces:**
- Documents: public status/error mapping、read-only health、15m/45m policy、sidecar 和人工 smoke。
- Preserves: P7.2b UI/exit work 和 installer cleanup 仍未完成。
- Verifies: P7.1 worker/scheduler/task safety regression and full repository gate。

- [ ] **Step 1: 先写可执行的 Windows smoke 文档**

文档必须包含以下边界与顺序：

```text
环境：一次性 Windows 本地测试账户或 VM；干净 AppData；人工 Profile；临时 save/backup fixture。
前置：安装包含 sidecar 的测试 bundle；创建 synthetic automatic-backup Profile；确认没有真实玩家 save/game 路径。
顺序：inspect missing -> register -> inspect exact -> register idempotent -> Task Scheduler 人工 Run ->
只读 probe 确认 fresh worker_heartbeat_at -> unregister -> unregister idempotent -> inspect missing。
cleanup：最后 inspect 必须 missing；删除 synthetic fixture；不保留 task XML/原始输出截图。
停止条件：ownership conflict、permission error、worker path 非 bundle sibling、任何真实玩家路径出现时立即停止。
```
真实任务操作只能由人工在该环境执行。自动化 agent/test 不得执行 smoke 命令。配置漂移逐字段
识别/修复由 fake runner 自动化覆盖，不在真实系统任务上主动制造危险 drift；P7.2a real smoke
也不修改 DB 中的 `background_protection_enabled` 来伪造 UI `protected`。

- [ ] **Step 2: 同步文档中的已实现与未实现边界**

逐份写明以下事实，不使用“完整后台保障已完成”这类扩大表述：

- P7.2a Windows registry/health/sidecar 已实现并通过 fake tests。
- `protected = exact registration + fresh heartbeat`，TTL 45 分钟。
- `get_save_backup_background_status` 只读，不注册、不修复、不启动任务。
- DTO/Audit Log 不含 task/SID/path/PowerShell/XML。
- P7.2a real smoke 只验收 exact registration、安装态 worker 真实触发、fresh heartbeat 与 cleanup；
  exact + fresh -> protected/future/stale 状态矩阵由 fixed-clock tests 覆盖。
- P7.2b Profile/Settings enable、退出提示仍未实现。
- P7.2b 才完成真实启用后的 UI/退出端到端 protected acceptance。
- NSIS/WiX uninstall hook 仍是 release packaging gate。
- release docs 记录 Windows bundle 必须包含 sibling worker、target-triple 源产物不提交，且
  “bundle 包含 sidecar”与“installer 自动 cleanup”是两个独立 gate。
- 只有人工 smoke 实际执行并清理成功后，才能记录 Windows runtime acceptance。

更新 `TODO.md` 时，不得把整个 T8 或 P7.2b 标为完成。

- [ ] **Step 3: 运行聚焦 Rust/Node 回归**

Run:
```powershell
cargo test -p hmm-core background_registration_statuses_have_stable_codes
cargo test -p hmm-infra --test save_backup_scheduler_repository
cargo test -p hmm-infra save_backup_background_registry::tests
cargo test -p hmm-app --test save_backup_background
cargo test -p hmm-app --test save_backup_background_worker
cargo test -p hmm-app --test save_backup_scheduler
cargo test -p hmm-app --test save_backup_task
cargo test -p hmm-tauri save_backup
cargo check -p hmm-tauri --bin hmm-save-backup-worker
node --test scripts/prepare-save-backup-worker-sidecar.test.mjs
```
Expected: 全部 PASS；过程中不出现 real Scheduled Task create/update/delete。

- [ ] **Step 4: 运行完整 repository gate**

Run:
```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify.ps1
git diff --check
git status --short --branch
```
Expected: `verify.ps1` exit 0；diff check 无输出；status 只包含本 task 预期文档文件。

- [ ] **Step 5: 执行或明确推迟 Windows 人工 smoke**

仅当当前环境明确是一次性 Windows 测试账户/VM 且用户授权真实 Scheduled Task 操作时，
按 smoke 文档执行。Terminal A 使用安装后的 sibling worker 启动 lifecycle harness；它会在任务
保持 registered 时等待人工确认，并在正常/普通 panic 路径无条件 cleanup：

```powershell
$installDirectory = (Read-Host 'Absolute install directory in the disposable test account/VM').Trim()
$workerCandidate = Join-Path $installDirectory 'hmm-save-backup-worker.exe'
if (-not (Test-Path -LiteralPath $workerCandidate -PathType Leaf)) { throw 'installed sibling worker is missing' }
$env:HMM_RUN_WINDOWS_SCHEDULED_TASK_SMOKE = "1"
$env:HMM_WINDOWS_SMOKE_WORKER_PATH = (Resolve-Path -LiteralPath $workerCandidate).Path
$env:HMM_WINDOWS_SMOKE_WAIT_FOR_TRIGGER = "1"
try {
    cargo test -p hmm-infra windows_scheduled_task_registry_smoke -- --ignored --nocapture
    if ($LASTEXITCODE -ne 0) { throw 'scheduled task lifecycle smoke failed' }
} finally {
    Remove-Item Env:\HMM_RUN_WINDOWS_SCHEDULED_TASK_SMOKE -ErrorAction SilentlyContinue
    Remove-Item Env:\HMM_WINDOWS_SMOKE_WORKER_PATH -ErrorAction SilentlyContinue
    Remove-Item Env:\HMM_WINDOWS_SMOKE_WAIT_FOR_TRIGGER -ErrorAction SilentlyContinue
}
```
Terminal A 等待时，在 Task Scheduler UI 只对本 harness 新建且 marker 正确的任务点击 Run。等待
worker 退出后，在 Terminal B 运行只读 heartbeat probe；路径和 profile 必须属于 disposable
fixture，probe 不输出它们：

```powershell
$databaseCandidate = (Read-Host 'Absolute SQLite path in disposable smoke AppData').Trim()
if (-not (Test-Path -LiteralPath $databaseCandidate -PathType Leaf)) { throw 'disposable smoke database is missing' }
$env:HMM_RUN_WINDOWS_SCHEDULED_TASK_SMOKE = "1"
$env:HMM_WINDOWS_SMOKE_DATABASE_PATH = (Resolve-Path -LiteralPath $databaseCandidate).Path
$env:HMM_WINDOWS_SMOKE_PROFILE_ID = 'scheduled-task-smoke'
try {
    cargo test -p hmm-infra --test save_backup_scheduler_repository windows_smoke_probe_sees_fresh_worker_heartbeat -- --ignored --exact --nocapture
    if ($LASTEXITCODE -ne 0) { throw 'worker heartbeat smoke probe failed' }
} finally {
    Remove-Item Env:\HMM_RUN_WINDOWS_SCHEDULED_TASK_SMOKE -ErrorAction SilentlyContinue
    Remove-Item Env:\HMM_WINDOWS_SMOKE_DATABASE_PATH -ErrorAction SilentlyContinue
    Remove-Item Env:\HMM_WINDOWS_SMOKE_PROFILE_ID -ErrorAction SilentlyContinue
}
```
probe PASS 后回到 Terminal A 按 Enter，让 harness 执行两次 unregister 和最终 inspect。无论命令
结果如何，都必须再执行 Task Scheduler UI 只读检查，确认没有残留应用任务；若残留，停止验收
并在同一 disposable 环境重新设置 authorization/固定 sibling worker env 后运行：

```powershell
$installDirectory = (Read-Host 'Absolute install directory in the disposable test account/VM').Trim()
$workerCandidate = Join-Path $installDirectory 'hmm-save-backup-worker.exe'
if (-not (Test-Path -LiteralPath $workerCandidate -PathType Leaf)) { throw 'installed sibling worker is missing' }
$env:HMM_RUN_WINDOWS_SCHEDULED_TASK_SMOKE = "1"
$env:HMM_WINDOWS_SMOKE_WORKER_PATH = (Resolve-Path -LiteralPath $workerCandidate).Path
try {
    cargo test -p hmm-infra windows_scheduled_task_registry_cleanup_smoke -- --ignored --nocapture
    if ($LASTEXITCODE -ne 0) { throw 'scheduled task cleanup smoke failed' }
} finally {
    Remove-Item Env:\HMM_RUN_WINDOWS_SCHEDULED_TASK_SMOKE -ErrorAction SilentlyContinue
    Remove-Item Env:\HMM_WINDOWS_SMOKE_WORKER_PATH -ErrorAction SilentlyContinue
}
```
cleanup-only test 内部重新派生 task identity 并校验 ownership marker；不得直接用 task name、
`schtasks /Delete` 或 Task Scheduler UI 宽泛删除。cleanup 后再次用 UI 只读确认。
否则记录：

```text
未执行 Windows Scheduled Task 人工 smoke：当前环境不是已授权的一次性测试账户/VM。
因此尚不宣称 Windows runtime acceptance；fake/临时自动化验证结果单独记录。
```
禁止为了完成 checklist 在用户日常 Windows 账户中创建测试任务。

- [ ] **Step 6: 更新计划状态并 Commit**

仅在聚焦/完整自动化通过后勾选自动化 checklist；只有真实 smoke 完成并 cleanup 后才能勾选
Windows smoke 项。然后提交：

```powershell
git add docs/SAVE_BACKUP_BACKGROUND_AUTOMATION_DESIGN.md docs/SAVE_BACKUP_BACKGROUND_SCHEDULER_CORE_PLAN.md docs/FRONTEND_BACKEND_CONTRACT.md docs/TESTING.md docs/LOGGING.md docs/release/发布与产物说明.md docs/release/构建发布与脚本说明.md TODO.md docs/testing/windows-save-backup-scheduled-task-smoke.md docs/superpowers/specs/2026-07-10-save-backup-windows-scheduled-task-core-design.md docs/superpowers/plans/2026-07-10-save-backup-windows-scheduled-task-core-implementation.md
git commit -m "docs: record Windows background protection gate"
```
---

## Final Review Checklist

- [ ] Current diff contains only P7.2a files and no `.planning/`, sidecar binaries, logs, backups or fixtures.
- [ ] Core/app/infra/Tauri dependency direction matches the approved design.
- [ ] No automated test or verification command created/updated/deleted a real Scheduled Task.
- [ ] No frontend/CLI input can provide command, script, task name, SID, worker path, arguments or XML.
- [ ] Ownership conflict never overwrites or deletes another task.
- [ ] Register/update/unregister all perform read-back and fail closed.
- [ ] Heartbeat does not mutate scheduler `last_checked_at`, `background_status` or lease.
- [ ] `protected` requires exact registration and heartbeat in `[now-45m, now]`.
- [ ] DTO/log/audit scans show no task name, SID, worker path, raw command output or task XML.
- [ ] Sidecar artifact is ignored by Git and present in a successful Windows bundle.
- [ ] `scripts/` and `tauri.windows.conf.json` changes received explicit human governance/packaging review.
- [ ] P7.1 worker/scheduler/task tests and full `verify.ps1` pass after the final change.
- [ ] Windows smoke status is honest: either executed in disposable environment with cleanup proof, or explicitly not executed with no runtime acceptance claim.
- [ ] P7.2b UI/exit behavior and NSIS/WiX uninstall cleanup remain explicitly incomplete.
