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

当前 P7.2a 交付未在开发者日常账户执行本 smoke，因此尚不构成 Windows runtime acceptance。

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
    cargo test -p hmm-infra windows_scheduled_task_registry_smoke -- --ignored --nocapture
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

probe 只确认 heartbeat 新鲜，不输出数据库路径、Profile 路径或玩家数据。

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
    cargo test -p hmm-infra windows_scheduled_task_registry_cleanup_smoke -- --ignored --nocapture
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
