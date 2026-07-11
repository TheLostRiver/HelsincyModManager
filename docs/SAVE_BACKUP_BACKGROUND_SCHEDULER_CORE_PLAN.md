# 后台自动备份调度内核实现计划

本文档定义 `SAVE_BACKUP_BACKGROUND_AUTOMATION_DESIGN.md` 中的后台调度内核，以及 P7.1 worker 基础和 P7.2a Windows 平台核心。P7.2a 已落地用户级 Scheduled Task 注册/read-back/移除、独立 heartbeat 健康派生和 worker sidecar 基础；P7.2b 用户启用与退出流程仍未实现。

## 当前实施状态

P7.1 已完成 scheduler state、lease 和单次 worker。P7.2a 进一步完成：

- Windows 用户级 registry adapter、固定 Scheduled Task spec、幂等写入与 ownership-checked read-back/unregister。
- 独立 `worker_heartbeat_at` 和 45 分钟 TTL；heartbeat 不覆盖 scheduler check、background status 或 lease。
- 只读 `get_save_backup_background_status` 健康派生；只有 enabled + exact registration + fresh heartbeat 才返回 `protected`。
- dev/release worker sidecar 准备与 Windows `externalBin` 配置。
- fake registry/runner、固定 clock、临时 SQLite/目录覆盖的自动化测试；不操作真实系统任务。

P7.2a 没有 Profile/Settings 真实启用开关或退出前提示，安装态人工 smoke 也尚未在一次性账户/VM 完成。因此普通产品流程仍保持 `tray_only`，不得表述为已通过 Windows runtime acceptance 或完整退出后保障。

## 目标

- 为自动备份增加持久化调度状态，避免只依赖主客户端内存或前端定时器。
- 通过持久化租约去重，避免主客户端运行期调度器和后台 worker 同时为同一 `gameId/profileId` 启动重复备份。
- 记录 worker 心跳、最近检查、最近尝试、最近成功、下次计划、pending 原因和最近错误码。
- 后台触发仍复用现有 `SaveBackupTaskRunner -> SaveBackupService -> SaveBackupWriter/Repository/AuditLog` 链路。
- 所有自动化测试只使用 fake repository / fake clock / 临时 SQLite，不依赖真实系统计划任务、真实 MHW 安装、真实 Steam userdata 或真实玩家存档。

## 非目标

- 自动化测试不注册、更新、启动或删除真实 Windows Scheduled Task。
- 本切片不新增 Windows Service。
- P7.2a 不新增前端 register/unregister surface、Profile/Settings 启用入口或退出提示。
- P7.2a 不实现 NSIS/WiX 自动卸载 cleanup；这是独立 release packaging gate。
- 本切片不实现存档恢复。
- 本切片不新增第二套备份文件写入逻辑。
- 本切片不让前端传入存档路径、备份路径、manifest、hash 或 scheduler lease 字段。

## 分层边界

### `hmm-core`

定义后台保护状态和调度状态中的稳定枚举：

- `SaveBackupBackgroundProtectionStatus`
- `SaveBackupSchedulerPendingReason`
- `SaveBackupSchedulerState`

这些类型只包含短 ID、枚举、时间戳和布尔值，不包含真实路径、Steam ID、manifest 正文、hash 列表或存档内容。

### `hmm-ports`

新增调度状态 repository port：

```rust
pub trait SaveBackupSchedulerStateRepository: Send + Sync {
    fn get_state(&self, game_id: &GameId, profile_id: &ProfileId) -> PortResult<Option<SaveBackupSchedulerState>>;
    fn upsert_state(&self, state: &SaveBackupSchedulerState) -> PortResult<()>;
    fn acquire_due_lease(&self, request: SaveBackupSchedulerLeaseRequest) -> PortResult<Option<SaveBackupSchedulerState>>;
    fn release_lease(&self, game_id: &GameId, profile_id: &ProfileId, lease_owner: &str) -> PortResult<()>;
    fn record_worker_heartbeat(&self, heartbeat: SaveBackupWorkerHeartbeat) -> PortResult<()>;
}
```

租约获取必须是单个数据库事务，保证并发 worker 只有一个能拿到同一 `gameId/profileId` 的到期任务。

### `hmm-app`

新增或扩展调度服务：

- 读取 profile 和 save settings。
- 对启用自动备份的 profile 计算 due/next due。
- 对 due profile 尝试获取 lease。
- 获取 lease 后返回 `StartSaveBackupTaskRequest { trigger: Auto }`，由现有任务服务启动备份。
- 如果游戏运行检测尚未接入，本切片先定义 pending reason 和 fake port 测试，不把“游戏运行中仍备份”做成默认行为。
- 后台 worker 检查失败只记录稳定错误码，不阻塞应用启动。

### `hmm-infra`

新增 SQLite migration 和实现：

```text
save_backup_scheduler_state
  game_id TEXT NOT NULL
  profile_id TEXT NOT NULL
  enabled INTEGER NOT NULL
  background_protection_enabled INTEGER NOT NULL
  last_checked_at INTEGER
  last_attempt_at INTEGER
  last_success_at INTEGER
  next_due_at INTEGER
  pending_reason TEXT
  last_error_code TEXT
  worker_instance_id TEXT
  lease_owner TEXT
  lease_expires_at INTEGER
  updated_at INTEGER NOT NULL
  PRIMARY KEY (game_id, profile_id)
```

字段约束：

- 不保存完整本地路径。
- 不保存 Steam ID。
- 不保存存档内容。
- 不保存 manifest 正文或 hash 列表。
- `pending_reason` 和 `last_error_code` 使用稳定短码。
- `lease_owner` 和 `worker_instance_id` 只用于本机诊断和去重。

### `src-tauri`

P7.2a 复用既有查询入口，并把 owner 切换为应用层健康服务：

```text
get_save_backup_background_status({ gameId, profileId })
```

返回 DTO 只包含：

- `gameId`
- `profileId`
- `status`
- `backgroundProtectionEnabled`
- `lastCheckedAt`
- `lastAttemptAt`
- `lastSuccessAt`
- `nextDueAt`
- `pendingReason`
- `lastErrorCode`

该查询只读，不注册、不修复、不启动 worker、不获取 lease。DTO 不返回 lease owner、worker id、task name、SID、worker path、PowerShell、XML、原始命令输出或任何备份文件系统细节。

### 前端

P7.2a 不新增前端开关。P7.2b 的 Profile/Settings UI 才可消费真实启用流程并展示后台保护状态摘要：

- 已受后台保护
- 仅客户端运行期保护
- 未启用后台保护
- 后台 worker 异常
- 当前平台暂不支持

高风险失败可用居中偏上的悬浮 UI 短时提示，但页面内状态必须可见，不能只靠悬浮 UI。

## 调度与租约规则

1. 调度器读取 profile/save settings 和 backup history。
2. 未启用自动备份时写入 `enabled = 0`，不启动任务。
3. 到期时先尝试获取持久化 lease。
4. 同一 `gameId/profileId` 存在任意未过期 lease 时，本次检查都视为 busy 并跳过，包括 lease owner 与当前请求相同的情况。
5. lease 已过期时可以接管，但必须重新读取状态并重新判断 due。
6. 获取 lease 不等于获得文件写入许可；备份前仍由 `SaveBackupService` 重验源目录、目标目录、包含关系、symlink/junction、大小上限和 retention。
7. 单次 catch-up 只启动一次备份，不为错过的多个窗口连续补多个备份。
8. 任务完成后更新 `last_success_at`、`last_attempt_at`、`next_due_at` 并释放 lease。
9. 任务失败后更新 `last_attempt_at`、`last_error_code` 并释放 lease。

### P7.1 已实现的额外保护

- `upsert_state` 刷新检查字段时不会覆盖已有的 `lease_owner` 或 `lease_expires_at`；lease 只能由过期接管或按 owner 的 `release_lease` 清理。
- worker 已取得 due lease 后，若 heartbeat 写入失败或任务 reservation/start 失败，必须立即释放该 lease，避免后续检查被虚假的在途状态阻塞。

## 错误码

新增或复用以下稳定短码：

```text
save_backup_scheduler_unavailable
save_backup_background_not_enabled
save_backup_background_worker_unhealthy
save_backup_auto_skipped_game_running
save_backup_auto_skipped_game_running_unknown
save_backup_auto_source_invalid
save_backup_auto_destination_unavailable
save_backup_auto_task_conflict
```

错误消息和 audit fields 不得包含完整路径、Steam ID、Windows 用户名、存档内容、manifest 正文或原始错误文本。

## 测试计划

### Rust app 层

- due profile 会生成一次 `SaveBackupTrigger::Auto` 任务请求。
- 错过多个窗口只 catch-up 一次。
- 同一 `gameId/profileId` 的任意未过期 lease（包括相同 owner）都会阻止重复启动任务。
- 过期 lease 会被接管，并重新判断 due。
- heartbeat 或 task reservation/start 失败会立即释放已取得 lease。
- 手动 schedule 不会启动自动备份。
- settings/history/clock/repository 失败会返回稳定错误码。

### Rust infra 层

- SQLite migration 创建 `save_backup_scheduler_state`。
- `upsert_state` 不保存路径字段，且不会覆盖已有 lease。
- `acquire_due_lease` 在事务内只允许一个 owner 获得 lease。
- `release_lease` 只释放当前 owner 的 lease。
- 过期 lease 可被新 owner 接管。

### Tauri/前端

P7.2a 保持既有 DTO shape 和 command name，只把只读查询切换到派生健康服务；P7.2b 新增 UI/启用流程时继续满足：

- DTO 序列化为 camelCase。
- command 参数只接受 `gameId/profileId`，不接受路径或 lease 字段。
- typed API wrapper 使用 feature-local 文件。
- Profile UI 显示后台保护状态，不再把 client-runtime 状态说成完整后台保护。

## 验证命令

按实际改动范围执行：

```powershell
cargo test -p hmm-app --test save_backup_scheduler
cargo test -p hmm-infra save_backup_scheduler
cargo test -p hmm-tauri save_backup
cargo check --workspace
cmd /c corepack pnpm run typecheck
cmd /c corepack pnpm run lint
cmd /c corepack pnpm run build
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify.ps1
```

如果本切片没有前端改动，可记录未运行前端视觉 smoke 的原因。

## 后续切片

1. 在已授权的一次性 Windows 账户/VM 按 smoke 文档完成安装态 sibling worker、真实触发、fresh heartbeat 与 cleanup 验收。
2. P7.2b：接入 Profile/Settings 后台保护真实启用开关。
3. P7.2b：接入退出主客户端时的“启用并退出”提示和真实启用后的端到端 `protected` 验收。
4. 发布 gate：补 NSIS/WiX 自动卸载 cleanup，并分别验证安装器包含 sidecar 和卸载无残留任务。
5. Linux / Steam Deck user service 或 autostart 实验支持。
