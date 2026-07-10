# 后台自动备份调度内核实现计划

本文档定义 `SAVE_BACKUP_BACKGROUND_AUTOMATION_DESIGN.md` 中后台自动备份的调度内核，以及已完成的 P7.1 worker 基础能力。P7.1 已落地可测试的调度状态、租约去重、worker heartbeat 和单次 headless worker；真实 Windows 用户级 Scheduled Task 注册仍属于后续 P7.2。

## 当前实施状态

P7.1 已完成：

- 稳定的后台 registry contract 与 `UnsupportedPlatform` fallback，未注册平台不会误报为已注册。
- SQLite 持久化 scheduler state、lease 与 worker heartbeat。
- 只接受 `--once` 的 headless worker binary；它复用既有 scheduler、task runner、备份历史、manifest 和 Audit Log 链路，并且不初始化 WebView。
- fake ports、固定 clock、临时 SQLite/目录覆盖的聚焦测试。

P7.1 没有注册或移除真实 Windows 用户级 Scheduled Task，也没有平台注册健康检查、Settings/Profile 的真实启用开关或退出前“启用并退出”提示。因此当前产品语义仍是 `tray_only`：headless binary 是平台注册前的基础能力，不是已注册的退出后自动运行机制；不得标为 `protected` 或完整后台保障。

## 目标

- 为自动备份增加持久化调度状态，避免只依赖主客户端内存或前端定时器。
- 通过持久化租约去重，避免主客户端运行期调度器和后台 worker 同时为同一 `gameId/profileId` 启动重复备份。
- 记录 worker 心跳、最近检查、最近尝试、最近成功、下次计划、pending 原因和最近错误码。
- 后台触发仍复用现有 `SaveBackupTaskRunner -> SaveBackupService -> SaveBackupWriter/Repository/AuditLog` 链路。
- 所有自动化测试只使用 fake repository / fake clock / 临时 SQLite，不依赖真实系统计划任务、真实 MHW 安装、真实 Steam userdata 或真实玩家存档。

## 非目标

- 本切片不注册真实 Windows Scheduled Task。
- 本切片不新增 Windows Service。
- P7.1 的 `--once` headless worker 不等于独立 guardian 或已注册的系统调度机制。
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

P7.1 不新增 Tauri command、DTO 或 UI。后续 P7.2 如需前端展示，可在平台注册和健康检查完成后受控地新增查询入口：

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

DTO 不返回 lease owner、worker id、真实路径或任何备份文件系统细节。

### 前端

P7.1 不实现前端开关或状态页。P7.2 的 Profile/Settings UI 才可展示后台保护状态摘要：

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

P7.1 没有新增 DTO/command/UI。P7.2 若新增这些内容：

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

1. P7.2a（下一任务）：完成 Windows 用户级 Scheduled Task 注册/更新/移除、健康检查和 `protected` 判定的设计与实施计划，再落地受控平台注册核心。
2. P7.2a：用 fake registry/command runner 覆盖生命周期和状态映射，并定义不接触真实玩家数据的 Windows 人工 smoke 策略。
3. P7.2b：在平台注册和 heartbeat 健康确认通过后，接入 Profile/Settings 后台保护真实启用开关。
4. P7.2b：接入退出主客户端时的“启用并退出”提示，未受保护时保持明确警示。
5. Linux / Steam Deck user service 或 autostart 实验支持。
