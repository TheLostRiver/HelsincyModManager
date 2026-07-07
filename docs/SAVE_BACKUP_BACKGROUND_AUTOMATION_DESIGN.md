# 自动备份后台保障设计

本文档定义存档自动备份在主客户端关闭后的保障语义、后台执行架构、调度规则、UI 提示、安全边界和分阶段落地计划。它补充 [存档备份系统设计](SAVE_BACKUP_DESIGN.md)，不替代手动备份、manifest、历史记录、恢复流程或存档目录自动发现设计。

## 背景

自动备份如果只依赖主窗口打开期间的前端定时器或 Tauri 主进程内 tick，就无法覆盖用户真正退出客户端后的时间段。对存档管理工具来说，“用户以为开启了自动备份，但退出客户端后实际没有备份”是不可接受的产品风险。

因此，正式自动备份必须把“关闭窗口”“退出主客户端”“系统未运行”区分清楚，并提供可解释、可验证、可审计的后台保障。

## 目标

- 用户关闭主窗口后，自动备份仍应继续生效。
- 用户真正退出主客户端后，自动备份仍应由本机后台守护或系统计划任务接管。
- 后台触发的备份必须复用现有 `SaveBackupService` 执行链路。
- 后台备份必须写入同一份 manifest、SQLite 备份历史、Audit Log 和保留策略结果。
- 主客户端再次打开时，能展示后台备份的最近结果、失败原因和是否仍受后台保障。
- 当后台保障未启用、失效或权限不足时，UI 必须明确提醒用户，不得让用户误以为自动备份仍受保护。

## 非目标

首个后台保障切片不实现：

- 云端备份或 Steam Cloud 同步。
- 跨设备调度。
- 系统级 Windows Service。
- 备份恢复写入。
- 游戏运行期间强制复制存档。
- 前端直接调度文件系统备份。

Windows Service 通常需要额外权限、安装/卸载治理和更复杂的升级策略，不作为第一阶段默认方案。首选用户级后台守护和用户级计划任务。

## 产品语义

### 关闭窗口

点击窗口关闭按钮默认只隐藏主窗口并保留托盘常驻。此时主客户端进程仍在，自动备份可以由客户端内调度器继续执行。

UI 必须提供清晰入口说明当前状态是“后台运行中”，而不是“程序已退出”。托盘菜单至少应包含：

- 打开 Helsincy。
- 暂停自动备份。
- 立即检查备份计划。
- 退出程序。

### 退出程序

用户选择“退出程序”表示主客户端进程结束。若后台保障已启用，自动备份应由用户级后台守护继续生效；若后台保障未启用，退出前必须提示：

```text
退出主客户端后，自动备份将不再受后台保障。
```

提示应提供：

- 启用后台保障并退出。
- 仅退出本次。
- 取消。

### 系统未运行

电脑关机、系统未登录、用户会话不存在或磁盘不可用时，程序不能执行备份。此时设计目标是“追赶”，不是虚假承诺。

下一次用户登录或后台守护恢复时，应检查错过的计划：

- 若存在 overdue 自动备份，且源/目标仍有效，则排队执行一次追赶备份。
- 若游戏正在运行，则记录 pending，等待游戏退出或下次调度窗口。
- 若源目录失效、目标不可写或设置不完整，则写入失败历史和轻量告警。

## 推荐架构

```text
主客户端 Tauri App
  UI、托盘、设置、历史展示、手动备份入口
  可运行客户端内调度器

后台备份守护进程
  无 UI 或极小托盘状态
  按用户会话启动
  读取同一份本地配置和数据库
  复用应用层备份服务

系统计划入口
  Windows: 用户级 Scheduled Task
  Linux: systemd user timer 或 desktop autostart
  Steam Deck: 后续实验性适配

共享后端能力
  ProfileSaveSettingsRepository
  SaveBackupService
  SaveBackupRepository
  SaveBackupWriter
  AuditLogWriter
  Game running detector
```

关键原则：

- 不能有两套备份实现。
- 不能让前端定时器成为自动备份事实来源。
- 不能让后台守护接收前端传入的真实路径。
- 不能绕过备份执行前的源目录、目标目录、包含关系、symlink/junction、大小上限和保留策略校验。

## 进程模型

### 主客户端

主客户端负责：

- 展示自动备份设置。
- 展示后台保障状态。
- 提供启用/停用后台保障入口。
- 触发手动备份。
- 展示后台备份历史和失败提醒。
- 在托盘常驻时执行轻量调度。

主客户端不负责在进程退出后继续备份。

### 后台守护

后台守护负责：

- 在用户登录后启动。
- 周期性检查已启用自动备份的 profile。
- 对 overdue 或 due 的计划创建备份任务。
- 在游戏运行时延后，而不是强行备份。
- 将结果写入同一份 SQLite 历史和 Audit Log。
- 记录自身健康状态，供主客户端下次读取。

后台守护可以是同一 Tauri/Rust workspace 打包出的单独二进制，也可以是主二进制的 headless 子命令。首选单独二进制或明确的 headless mode，避免无 UI 运行时仍初始化完整 WebView。

### 系统计划任务

Windows 第一阶段建议使用用户级 Scheduled Task：

- 在用户登录时启动后台守护。
- 可选择每隔固定时间唤醒一次守护执行单次检查。
- 不要求管理员权限。
- 安装、更新、卸载时由受控后端流程维护。

计划任务只负责拉起守护，不负责备份逻辑。

## 数据模型补充

现有 `ProfileSaveSettings.schedule` 表达备份节奏。后台保障需要额外持久化调度状态，避免只靠内存判断。

建议新增受控表或等价 repository：

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
  updated_at INTEGER NOT NULL
  PRIMARY KEY (game_id, profile_id)
```

字段约束：

- 不保存完整本地路径。
- 不保存 Steam ID。
- 不保存存档内容或 manifest 正文。
- `pending_reason` 和 `last_error_code` 使用稳定短码。
- `worker_instance_id` 是本机内部短 id，只用于诊断同一时间是否有多个 worker 竞争。

## 调度规则

### due 判断

调度器只基于后端持久化设置判断：

- profile 是否存在。
- profile 是否启用自动备份。
- schedule 是否为 daily / weekly 等自动节奏。
- `next_due_at` 是否已到。
- 上次成功时间是否已满足间隔。

前端展示用 view model 可以格式化这些信息，但不能计算最终 due 事实。

### 追赶策略

如果错过多个计划窗口，不应一次性补多个备份。默认只执行一次追赶备份，然后根据当前 schedule 计算下一次 `next_due_at`。

原因：

- 避免刚开机时连续写多个重复备份。
- 避免保留策略被短时间大量触发。
- 存档文件只有当前状态，多次补跑通常没有价值。

### 游戏运行冲突

自动备份默认不在游戏运行时执行，避免复制游戏正在写入的存档。

调度结果：

- 游戏未运行：正常执行。
- 游戏运行中：设置 `pending_reason = game_running`，延后。
- 游戏退出后：执行一次 pending 备份。
- 无法判断游戏是否运行：保守延后，并记录 `game_running_unknown`。

手动备份可以在 UI 中要求用户确认风险；自动备份不应在无人值守场景下要求确认。

### 并发与锁

- 同一 `gameId/profileId` 的备份任务必须串行。
- 主客户端调度器和后台守护可能同时存在，必须通过数据库锁、任务锁或调度租约避免重复执行。
- 获得调度租约不等于获得文件写入许可；备份执行前仍由 `SaveBackupService` 重新校验所有目录与安全上限。
- 长时间扫描、压缩、hash 不应持有不必要的全局锁。

建议租约字段：

```text
lease_owner
lease_expires_at
```

租约过期后其他 worker 可以接管；接管前必须重新读取状态并再次判断 due。

## 后台保障状态

主客户端应展示一个稳定状态：

```text
protected
tray_only
not_enabled
registration_failed
worker_unhealthy
permission_required
unsupported_platform
```

含义：

- `protected`：后台守护已注册且最近健康检查正常。
- `tray_only`：主客户端托盘常驻可执行自动备份，但真正退出后不受保护。
- `not_enabled`：用户未启用后台保障。
- `registration_failed`：计划任务或自启动注册失败。
- `worker_unhealthy`：已注册但最近没有心跳或连续失败。
- `permission_required`：当前环境需要额外权限或系统设置。
- `unsupported_platform`：当前平台尚未实现后台保障。

状态进入 `registration_failed`、`worker_unhealthy`、`permission_required` 或 `unsupported_platform` 时，Profile 页面和设置页必须有清晰提示。主界面打开时可以出现居中偏上的悬浮 UI，但不能只依赖悬浮提示；页面内状态也必须可见。

## 用户界面

### 设置页

设置页应提供：

- 自动备份总开关。
- 后台保障开关。
- 当前保障状态。
- 上次后台检查时间。
- 上次成功备份时间。
- 最近失败原因。
- “立即检查”按钮。

当用户启用自动备份但未启用后台保障时，应显示明确提示：

```text
当前仅在客户端运行时自动备份。启用后台保障后，退出主客户端也会继续检查备份计划。
```

### Profile 页

Profile 页应展示与当前 profile 相关的：

- schedule 摘要。
- 下一次计划时间。
- 是否有 pending 备份。
- 最近一次自动备份结果。
- 后台保障是否覆盖该 profile。

### 悬浮 UI

悬浮 UI 用于短时提醒：

- 后台保障启用成功。
- 后台守护不可用。
- 自动备份失败。
- 启动后发现错过备份并已追赶。

位置延续当前项目约定：界面正中上方一点。正常信息几秒后自动消失；高风险失败可以保留页面内状态，而不是让悬浮 UI 长期停留。

## 错误码

建议新增或复用稳定错误码：

```text
save_backup_scheduler_unavailable
save_backup_background_not_enabled
save_backup_background_registration_failed
save_backup_background_worker_unhealthy
save_backup_background_permission_required
save_backup_background_unsupported_platform
save_backup_auto_skipped_game_running
save_backup_auto_skipped_game_running_unknown
save_backup_auto_source_invalid
save_backup_auto_destination_unavailable
save_backup_auto_task_conflict
```

错误 message 不包含完整本地路径、Steam ID、Windows 用户名、存档内容、manifest 正文或备份根目录。

## 审计与日志

以下事件应进入 Audit Log：

- 启用或停用后台保障。
- 注册或移除用户级计划任务。
- 自动备份开始。
- 自动备份成功。
- 自动备份失败。
- 自动备份因游戏运行延后。
- 后台守护健康状态变更。

允许字段：

- `game_id`
- `profile_id`
- `task_id`
- `backup_id`
- `trigger = auto`
- `result`
- `error_code`
- `file_count`
- `archive_size_bytes`
- `scheduler_state`

禁止字段：

- 完整存档目录。
- 完整备份目录。
- Steam ID。
- Windows 用户名。
- 存档内容。
- manifest 正文。
- hash 列表。

## 安全边界

后台守护和主客户端都必须遵守同一安全边界：

- 不从前端接收备份源路径或目标路径。
- 不直接读取任意文件系统路径。
- 不绕过 profile save settings repository。
- 不绕过 game adapter 的存档目录规则。
- 不在游戏运行中无人值守备份。
- 不把备份写入游戏目录内部。
- 不跟随 symlink/junction。
- 不记录敏感路径或账号信息。
- 不把失败的保留策略清理反向判定为本次备份失败，但必须记录 warning/audit。

## 平台策略

### Windows

MVP 平台。推荐：

- 主客户端支持托盘常驻。
- 后台守护通过用户级 Scheduled Task 注册。
- 计划任务在用户登录时启动，必要时按固定间隔唤醒。
- 不默认要求管理员权限。

### Linux / Steam Deck

后续实验性支持。候选方案：

- systemd user service + timer。
- XDG autostart。
- Steam Deck Desktop Mode 下先提供托盘/客户端内调度，再逐步接入 user service。

跨平台实现必须通过 port/trait 抽象注册、卸载、健康检查和状态查询，不把 Windows 计划任务细节泄漏到应用层。

## 测试要求

实现后台保障时至少覆盖：

- 后台保障开启/关闭会更新调度状态。
- 注册失败返回稳定错误码。
- due profile 会触发一次 `SaveBackupTrigger::Auto`。
- 错过多个窗口只追赶一次。
- 游戏运行中自动备份被延后。
- 游戏退出后 pending 备份被执行。
- 主客户端调度器和后台守护并发时不会重复备份同一 profile。
- 后台触发仍会执行源目录、目标目录、包含关系、symlink/junction、大小上限校验。
- 自动备份历史 DTO 不返回完整路径、Steam ID、manifest 正文或 hash 列表。
- Audit Log 不包含完整本地路径、Steam ID、用户名或存档内容。

测试必须使用临时目录、fake game running detector、fake scheduler registry 和 fake clock。不得依赖真实 MHW:I 安装目录、真实 Steam userdata、真实玩家存档或真实系统计划任务。

## 分阶段落地

### 切片 1：设计与契约

- 新增本文档。
- 在存档备份总设计中明确自动备份后台保障要求。
- 后续实现前同步契约、测试和日志文档。

### 切片 2：客户端内自动备份

- 复用手动备份服务。
- 接入 schedule due 判断。
- 主客户端运行期间执行自动备份。
- 关闭窗口进入托盘仍可执行。
- UI 明确显示“仅客户端运行时受保护”。

### 切片 3：后台守护 MVP

- 新增 headless worker 或独立守护二进制。
- 新增 scheduler state repository。
- 新增 worker 心跳和健康状态。
- 新增 fake registry 测试。
- Windows 使用用户级 Scheduled Task 注册。

### 切片 4：主客户端状态与提示

- 设置页展示后台保障状态。
- Profile 页展示 profile 级自动备份状态。
- 启动时读取后台结果并展示页面内状态和短时悬浮 UI。
- 退出主客户端时根据后台保障状态提示用户。

### 切片 5：跨平台扩展

- 抽象 scheduler registry port。
- Linux / Steam Deck 接入 user service 或 autostart。
- 增加平台差异文档和手动 smoke test 记录。

## 验收标准

正式宣称“自动备份可用”前必须满足：

- 主窗口关闭后仍会按计划备份。
- 主客户端退出后，后台保障启用时仍会按计划备份。
- 后台保障不可用时，UI 明确提示自动备份不会在退出后继续。
- 后台自动备份结果能在主客户端重新打开后看到。
- 所有自动备份结果都有 manifest、历史记录和 Audit Log。
- 所有高风险路径都有临时目录或 fake 依赖测试覆盖。
- 文档、契约、测试说明和日志说明同步更新。
