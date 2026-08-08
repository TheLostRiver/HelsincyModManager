# Windows 存档后台任务人工 Smoke

本文档只用于一次性 Windows 本地测试账户或虚拟机中的人工验收。普通自动化、coding agent、CI 和开发者日常账户不得执行这里的真实 Scheduled Task 操作。

## 验收范围

本 smoke 只验证：

- 安装目录中存在主程序 sibling `hmm-save-backup-worker.exe`。
- 初始 inspect 为 missing。
- register 后 read-back 为 exact，重复 register 保持幂等。
- Task Scheduler 人工 Run 能启动固定 `--once` worker。
- 只读 probe 能看到新鲜的 `worker_heartbeat_at`。
- unregister 和重复 unregister 均安全，最终 inspect 为 missing。

配置漂移的逐字段识别和修复由 fake runner 自动化测试覆盖。不要在真实任务上主动制造 drift，也不要修改数据库中的 `background_protection_enabled` 来伪造 UI `protected`。

## 环境要求

- 一次性 Windows 本地测试账户或 VM，当前用户明确授权本次真实任务操作。
- 干净的应用 AppData。
- 已安装包含 sidecar 的测试 bundle。
- 人工创建的 synthetic automatic-backup Profile。
- 临时 save/backup fixture，不使用真实游戏目录、Steam userdata 或玩家存档。
- 可运行本仓库测试 harness 的源码与 Rust 工具链。

P7.2a 安装态 runtime acceptance 已于 2026-08-07 在一次性 Windows Sandbox 完成。验收覆盖安装目录
sibling worker、真实 user Scheduled Task 的 register/exact read-back、幂等 register、Task Scheduler
人工 Run、新鲜 heartbeat、幂等 unregister 和最终 missing inspect。首次生命周期 harness 的控制台
stdin 未能接收 acknowledgement，因此最后的 unregister leg 使用同一 disposable 环境中的
ownership-checked cleanup smoke 完成，并在 Task Scheduler UI 刷新后只读确认无残留任务；该偏差须保留在
结果记录中，不得伪装成原 harness 完全无偏差通过。

## 停止条件

出现以下任一情况立即停止，不继续注册或触发：

- ownership conflict。
- permission error。
- worker 不是安装目录中的主程序 sibling。
- 任意真实玩家路径、真实 Steam 标识或真实游戏目录进入 fixture、输出或截图。
- read-back 不是 exact，或 cleanup 后任务仍存在。

不要保留 task XML、PowerShell 原始输出或包含本地路径的截图。

## 执行顺序

### 1. 启动生命周期 Harness

在 Terminal A 中运行：

```powershell
$installDirectory = (Read-Host 'Absolute install directory in the disposable test account/VM').Trim()
$workerCandidate = Join-Path $installDirectory 'hmm-save-backup-worker.exe'
if (-not (Test-Path -LiteralPath $workerCandidate -PathType Leaf)) { throw 'installed sibling worker is missing' }
$env:HMM_RUN_WINDOWS_SCHEDULED_TASK_SMOKE = "1"
$env:HMM_WINDOWS_SMOKE_WORKER_PATH = (Resolve-Path -LiteralPath $workerCandidate).Path
$env:HMM_WINDOWS_SMOKE_WAIT_FOR_TRIGGER = "1"
try {
    cargo test -p hmm-infra save_backup_background_registry::tests::windows_scheduled_task_registry_smoke -- --ignored --exact --nocapture
    if ($LASTEXITCODE -ne 0) { throw 'scheduled task lifecycle smoke failed' }
} finally {
    Remove-Item Env:\HMM_RUN_WINDOWS_SCHEDULED_TASK_SMOKE -ErrorAction SilentlyContinue
    Remove-Item Env:\HMM_WINDOWS_SMOKE_WORKER_PATH -ErrorAction SilentlyContinue
    Remove-Item Env:\HMM_WINDOWS_SMOKE_WAIT_FOR_TRIGGER -ErrorAction SilentlyContinue
}
```

Harness 必须依次确认 missing、register exact 和重复 register exact，然后等待人工触发。它在正常返回和普通 panic 路径中都执行 unregister cleanup。

### 2. 人工触发固定任务

Terminal A 等待时，在 Task Scheduler UI 中只对本 harness 创建且 ownership marker 正确的任务点击 Run。不要编辑 action、trigger、principal 或 settings。

### 3. 只读检查 Heartbeat

worker 退出后，在 Terminal B 中对 disposable SQLite fixture 运行：

```powershell
$databaseCandidate = (Read-Host 'Absolute SQLite path in disposable smoke AppData').Trim()
if (-not (Test-Path -LiteralPath $databaseCandidate -PathType Leaf)) { throw 'disposable smoke database is missing' }
$env:HMM_RUN_WINDOWS_SCHEDULED_TASK_SMOKE = "1"
$env:HMM_WINDOWS_SMOKE_DATABASE_PATH = (Resolve-Path -LiteralPath $databaseCandidate).Path
$env:HMM_WINDOWS_SMOKE_PROFILE_NAME = 'scheduled-task-smoke'
try {
    cargo test -p hmm-infra --test save_backup_scheduler_repository windows_smoke_probe_sees_fresh_worker_heartbeat_by_profile_name -- --ignored --exact --nocapture
    if ($LASTEXITCODE -ne 0) { throw 'worker heartbeat smoke probe failed' }
} finally {
    Remove-Item Env:\HMM_RUN_WINDOWS_SCHEDULED_TASK_SMOKE -ErrorAction SilentlyContinue
    Remove-Item Env:\HMM_WINDOWS_SMOKE_DATABASE_PATH -ErrorAction SilentlyContinue
    Remove-Item Env:\HMM_WINDOWS_SMOKE_PROFILE_NAME -ErrorAction SilentlyContinue
}
```

GUI 创建的 Profile 使用随机内部 ID，因此 probe 通过唯一的 synthetic Profile 名称在数据库内
只读解析 ID；名称不唯一时 fail closed。probe 只确认 heartbeat 新鲜，不输出数据库路径、内部
Profile ID、Profile 路径或玩家数据。

### 4. 完成 Cleanup

probe 通过后回到 Terminal A 按 Enter。Harness 必须执行两次 unregister，并最终 inspect missing。随后在 Task Scheduler UI 做一次只读检查，确认没有残留应用任务，再删除 synthetic Profile 和临时 fixture。

若任务仍残留，停止验收，仅在同一 disposable 环境运行 ownership-checked cleanup：

```powershell
$installDirectory = (Read-Host 'Absolute install directory in the disposable test account/VM').Trim()
$workerCandidate = Join-Path $installDirectory 'hmm-save-backup-worker.exe'
if (-not (Test-Path -LiteralPath $workerCandidate -PathType Leaf)) { throw 'installed sibling worker is missing' }
$env:HMM_RUN_WINDOWS_SCHEDULED_TASK_SMOKE = "1"
$env:HMM_WINDOWS_SMOKE_WORKER_PATH = (Resolve-Path -LiteralPath $workerCandidate).Path
try {
    cargo test -p hmm-infra save_backup_background_registry::tests::windows_scheduled_task_registry_cleanup_smoke -- --ignored --exact --nocapture
    if ($LASTEXITCODE -ne 0) { throw 'scheduled task cleanup smoke failed' }
} finally {
    Remove-Item Env:\HMM_RUN_WINDOWS_SCHEDULED_TASK_SMOKE -ErrorAction SilentlyContinue
    Remove-Item Env:\HMM_WINDOWS_SMOKE_WORKER_PATH -ErrorAction SilentlyContinue
}
```

cleanup harness 会重新派生 task identity 并验证 ownership marker。不要使用 task name、`schtasks /Delete` 或 Task Scheduler UI 做宽泛删除。cleanup 后再次用 UI 只读确认。

## 结果记录

只有以下顺序全部通过且 cleanup 成功，才能记录 Windows runtime acceptance：

```text
inspect missing -> register -> inspect exact -> register idempotent ->
manual Run -> fresh worker_heartbeat_at -> unregister ->
unregister idempotent -> inspect missing
```

记录只包含通过/失败状态、应用版本、Windows 版本和 CPU 架构。不要记录 task name、SID、worker 路径、task XML、原始 PowerShell 输出、Profile 路径、数据库路径或截图。

### 2026-08-07 acceptance record

- 状态：`PASS`（安装态 runtime acceptance）。
- 应用：`0.1.0-alpha.0`；Windows 10 Enterprise `10.0.19041` build `19041`；架构 `AMD64`。
- 受控链路：`inspect missing -> register -> exact read-back -> idempotent register -> manual Run
  -> fresh heartbeat -> unregister -> idempotent unregister -> inspect missing`。
- 有效 worker 运行还完成了一个 synthetic automatic backup；safe evidence 仅保留在仓库外的
  `hmm-save-02-evidence/logs/{app,tasks,audit}` 目录。
- 清理：synthetic Profile 在 Sandbox 关闭前删除；Task Scheduler UI 刷新后无 owned task；宿主机
  `hmm-save-02-fixtures` 已移入回收站。原始 task XML、SID、完整路径和 PowerShell transcript 未保留。
- 偏差：Terminal A 未响应 stdin acknowledgement，故 cleanup harness 承担最终 unregister 与
  幂等清理；cleanup harness 和最终 UI 检查均通过。
