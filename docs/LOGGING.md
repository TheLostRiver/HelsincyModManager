# 日志与审计设计

本文档定义 Helsincy Mod Manager 的日志、审计、诊断导出和脱敏规则。日志系统的目标不是尽可能多地记录本机信息，而是在不泄露玩家隐私的前提下，为安装、卸载、回滚、备份和故障诊断提供足够证据。

## 目标

- 诊断 Mod 导入、安装、卸载、回滚、存档备份、游戏启动等流程。
- 审计高风险文件操作，尤其是写入、覆盖、删除、备份、恢复和 manifest 变更。
- 为前端任务进度、错误提示和日志查看器提供结构化事件。
- 支持导出已脱敏的诊断包，便于用户反馈问题。
- 默认保护玩家隐私，不记录完整本地路径、Steam ID、token、cookie、真实存档内容或第三方 Mod 内容。

## 非目标

- 默认不做远程遥测。
- 不把日志当作安装清单、备份清单或回滚状态的替代品。
- 不把日志写入游戏目录、Mod 目录、存档目录或仓库目录。
- 不通过日志保存可恢复玩家文件的唯一信息。

## 日志类型

### App Log

记录应用生命周期、配置加载、数据库初始化、游戏发现摘要、任务启动和普通错误。App Log 用于开发者和用户定位常见问题。

### Task Log

记录单个后台任务的阶段、进度、耗时和失败原因。每个 Task Log 必须关联 `task_id`，避免 UI 或排障时把多个任务混在一起。

### Audit Log

记录高风险操作的安全证据。Audit Log 不追求详细内容，而追求可追踪的操作边界，例如“哪个 profile 对哪个 game instance 执行了哪些受控动作，结果是什么”。

### Debug Log

用于开发构建或用户主动开启的临时诊断。Debug Log 默认关闭，只有用户通过设置页的持久化开关明确开启后，
运行时才会写入 `logs/debug/debug-YYYY-MM-DD.log`。开关通过
`get_debug_log_settings` / `set_debug_log_settings({ enabled })` 读取和更新；保存失败时保持旧的运行态。
事件只允许稳定 `event`、受控 ID、`component`/`operation`/`result`/`error_code`、`task_id` 和数值聚合字段，
拒绝自由文本、原始路径、原始错误、Manifest、Hash、存档或第三方 Mod 内容。Debug Log 复用 managed-log
的 capability-relative no-follow writer/reader，保留含当天在内最近 7 个 UTC 日；非法日期、未知文件名、
非普通文件和 link/junction/reparse entry 保留，Debug 清理失败不阻断其他日志类别。

## 推荐技术栈

Rust 后端优先使用：

- `tracing`：结构化事件和 span。
- `tracing-subscriber`：格式化、过滤、分层输出。
- `tracing-appender`：文件输出和非阻塞写入。

前端不直接写核心日志文件。前端负责展示任务进度、用户可读错误、日志查看器和诊断导出入口；需要记录的操作通过 Tauri command 或事件边界交给后端统一处理。

## 存放位置

日志目录必须位于应用数据或应用状态目录下：

```text
Windows:
  %APPDATA%/HelsincyModManager/logs/

Linux:
  $XDG_STATE_HOME/HelsincyModManager/logs/
  ~/.local/state/HelsincyModManager/logs/
```

禁止位置：

- 游戏安装目录。
- Mod 导入目录。
- 存档目录。
- 仓库目录。
- 系统临时目录中没有应用命名空间隔离的位置。

建议目录结构：

```text
logs/
  app/app-YYYY-MM-DD.log
  debug/debug-YYYY-MM-DD.log
  tasks/task-<task_id>.log
  audit/audit-YYYY-MM-DD.log
  diagnostics/exported-<timestamp>.zip
```

## 保留策略

默认保留策略：

- App Log：14 天。
- Task Log：30 天。
- Audit Log：90 天。
- Debug Log：7 天。

LOG-01 已落地 Task/Audit 的按年龄保留：`HmmRuntime::from_builder` 在完整写侧 runtime 启动时
best-effort 执行一次，因此 Tauri、Sandbox lifecycle CLI 与固定 `--once` worker 复用同一策略；
独立只读 automation 不创建日志目录，也不执行清理。Task Log 只识别 writer 可拥有的
`task-<task_id>.log`，以 capability 句柄读取到的 mtime 保留含当天在内最近 30 个 UTC 日；Audit Log
只识别真实合法日历日期的 `audit-YYYY-MM-DD.log`，按文件名日期保留含当天在内最近 90 个 UTC 日。
未知文件、非法日期、非普通文件以及 link/junction/reparse entry 一律保留。

Task/Audit writer、reader 与 retention 统一通过 `managed_log` 的 capability-relative no-follow
目录/文件句柄访问固定 `logs/tasks`、`logs/audit`。任一类别枚举、复验或删除失败只记录对应
`task_log_retention_failed` / `audit_log_retention_failed` 与独立累计计数；另一类别仍继续清理，runtime
继续启动，且不会改变 InstallPlan、manifest、backup、rollback、recovery 或玩家文件事务结果。

LOG-02 已落地统一总空间预算。现有 `config/settings.json` schema v1 使用可选
`logStorageMaxBytes`；缺失或 `null` 使用默认 128 MiB，显式值不得小于 1 MiB。Tauri 只提供窄
`get_log_storage_settings` / `set_log_storage_settings` 契约，读取和更新 `{ maxBytes }`，不接受日志路径、
文件名、类别或删除策略；写设置不会立即执行清理。共享完整 runtime 启动时读取同一设置并执行维护，
只读 automation 仍不创建日志目录、不读取 settings 进行维护，也不产生清理副作用。

预算只统计固定 `logs/app`、`logs/tasks`、`logs/audit`、`logs/debug` 下可由 HMM writer 严格证明拥有的
普通日志文件。未知文件、非法日期、目录、非普通文件、symlink/junction/reparse entry 不计入 owned
预算且不删除。清理优先级为：Debug 与 Task 同层按最旧排序，其次 App，最后只处理超过 30 天硬下限的
Audit。当前 UTC 日的 App/Debug 和最近 30 个 UTC 日的 Audit 永不作为预算候选；清理目标额外预留
16 KiB 给本次最小维护 Audit。受保护文件本身已超过预算时返回 `log_storage_budget_unsatisfied`，不得
突破 Audit 或当前日日志保护边界。

枚举、复验或删除失败按类别隔离，其他类别继续尝试；删除使用 capability-relative no-follow 句柄，
并在打开前、打开后和删除前复验文件身份及目录 containment。健康状态稳定投影为
`ok | log_storage_settings_unavailable | log_storage_budget_unsatisfied | log_storage_budget_failed`，并
分别累计 settings、unsatisfied 和维护失败计数。只有发生删除、失败、无法收敛或 settings degraded 时，
runtime 才在维护完成后写一条 `log_storage_budget_maintenance` Audit；写完不再次调用预算维护，避免
递归日志风暴。任何预算退化都不改变 InstallPlan、manifest、backup、rollback、recovery 或玩家文件事实。

## 结构化字段

日志事件应优先使用结构化字段，而不是拼接长字符串。

推荐字段：

```text
timestamp
level
target
event
task_id
game_id
profile_id
mod_id
operation
result
duration_ms
error_code
safe_path
file_hash
file_size
```

禁止字段：

```text
raw_path
raw_steam_id
raw_token
raw_cookie
raw_save_content
raw_mod_content
```

如果必须定位文件，应记录脱敏后的 `safe_path`、相对逻辑路径、路径尾部、稳定 hash 或文件大小，而不是完整本地路径。

## 脱敏规则

日志写入前必须经过统一脱敏层。禁止各模块自己临时拼接“看起来没问题”的字符串。

必须脱敏或拒绝记录：

- Windows 用户名和 home 路径。
- Linux 用户名和 home 路径。
- 完整游戏安装路径。
- 完整存档路径。
- Steam ID。
- token、API key、cookie、会话信息。
- 真实存档内容。
- 第三方 Mod 包内容、readme 全文或资源文件内容。

允许记录：

- 游戏 ID，例如 `mhw`.
- profile ID、mod ID、task ID 等内部 ID。
- 文件大小。
- 文件 hash。
- 逻辑目标路径，例如 `nativePC/...`，但必须先确认不包含本机用户名或账号信息。
- 错误分类和错误码。

## 任务事件与 UI

任务进度事件不是日志文件的简单镜像。TaskManager 负责创建 `task_id`，并把进度事件发送给前端。

任务事件至少包含：

```text
task_id
task_type
phase
progress_current
progress_total
message_code
result
```

面向用户的 `message_code` 由前端映射为本地化文本。后端不应把敏感路径拼进用户可见消息。

## 审计要求

以下操作必须写 Audit Log：

- 写入游戏目录。
- 覆盖已有文件。
- 删除游戏目录文件。
- 创建或恢复备份。
- 自动备份因游戏运行或状态未知被延后（`result = deferred`，只在 pending 原因翻转时记录一次，避免重复检查刷屏）。
- 写入安装清单。
- 回滚成功或失败。
- 修改替换目标绑定。
- 变更自动备份设置。
- 导出诊断包。

Audit Log 必须记录操作结果。如果操作失败，应记录错误分类和是否已回滚，但不能记录敏感原文路径。

## 错误分类

日志和前端错误展示应共用稳定分类：

- `UserActionRequired`：需要玩家手动选择目录、安装前置或确认风险。
- `Recoverable`：操作失败但状态未损坏，可重试。
- `RollbackSucceeded`：操作失败但回滚完成。
- `RollbackFailed`：操作失败且回滚未完全完成，需要人工处理。
- `DataSafetyRisk`：可能影响玩家存档或游戏目录完整性。
- `InternalBug`：程序内部不变量被破坏。

涉及 `RollbackFailed` 或 `DataSafetyRisk` 的事件必须进入 Audit Log。

## 诊断导出

L3 的 `/diagnostics` 页面通过无参数 `get_diagnostics_page_snapshot` 读取固定上限安全快照。App/Task 文本
继续由 `TextLogReader` 校验，Audit 事件继续由 `AuditLogReader` 校验；前端不接触日志目录或文件路径。
任一类别不可读时只返回稳定状态，其他安全类别仍可展示。受控导出继续使用
`export_support_diagnostics`，成功/失败沿用既有最小 Audit Log 策略。

CLI-1B 的 `hmm diagnostics snapshot` 复用从上述流程抽出的 reader-only
`DiagnosticsPageSnapshotService`。CLI 只投影 bounded platform summary、App/Task/Audit 分类状态和
聚合计数；不返回日志正文、来源文件名、Audit fields、evidence health 进程内计数、完整本机信息或
export path，也不构造 diagnostic exporter 或写 Audit Log。Sandbox 日志目录必须位于显式 data root
内，自动测试只使用人工日志 fixture。

诊断包用于用户主动反馈问题，必须默认脱敏。

可包含：

- 已脱敏的 App Log。
- 已脱敏的 Debug Log。
- 已脱敏的 Task Log。
- 已脱敏的 Audit Log。
- 应用版本、平台、游戏适配器 ID。
- 配置摘要，例如是否启用自动备份、保留策略数值。

禁止包含：

- 真实存档文件。
- 第三方 Mod 包。
- 完整本地路径。
- Steam ID。
- token、cookie、API key。
- 数据库中可还原玩家本地隐私的信息。

导出前应给用户展示将包含的类别，而不是展示敏感原文。

## 模块边界

- `TaskManager`：创建 `task_id`，维护任务状态，发送进度事件。
- `InstallExecutor`：对文件写入、覆盖、删除、manifest、回滚写 Audit Log。
- `SaveBackupService`：对备份、恢复、保留策略清理写 Audit Log。
- `GameDiscoveryService`：只输出脱敏路径摘要，不记录完整本地路径。
- `Tauri Commands`：记录调用边界和错误分类，不记录未脱敏参数。
- `Frontend`：展示用户可读信息，不直接拼接底层文件系统日志。

当前已落地的最小 App Log 能力：

- `hmm-infra` 提供专用 `hmm.safe_app_log` layer，只接受命名 envelope 的固定字段；普通 tracing、
  `message`、未知/重复字段、`Debug` 和不合规值不会写入 App Log。
- 受控事件以单行 JSONL 写入 app data/state 下的 `logs/app/app-YYYY-MM-DD.log`，按 UTC 日期轮转，
  默认保留含当天在内的最近 14 天；不写入游戏、Mod、存档、仓库或无命名空间临时目录。
- writer 从已验证的 app-data 目录句柄逐级打开 `logs/app`，后续创建、打开、枚举和保留删除都相对该
  capability handle 执行；祖先 symlink/junction/reparse point 不能把操作重定向到 app-data 根外。
  Unix 上 `logs`/`logs/app` 收紧为 `0700`、日文件收紧为 `0600`；Windows 依赖 handle/reparse
  containment，不把 POSIX mode 声称为 Windows ACL。
- 写入前统一校验稳定 event/code、短 ID、聚合计数和逻辑相对 `safe_path`。完整 home/game/save 路径、
  Windows/Linux 用户名、Steam ID、token/cookie/API key、控制字符和第三方内容被拒绝或脱敏。
- 最小事件覆盖应用启动、configuration/database state 初始化、游戏发现聚合结果、queued task 注册、
  窗口/后台任务等普通稳定错误；不会记录 task message/error/result ref、候选目录或原始平台错误。
- `app_health` 只返回 `ok`、`app_log_event_rejected`、`app_log_retention_failed`、
  `app_log_write_failed` 或 `app_log_initialization_failed`。初始化/运行时失败不 panic、不写玩家目录，
  也不改变 InstallPlan、manifest、backup、rollback、recovery 或 Audit Log 事实链。
- 既有 `FileSystemTextLogReader` 和 `export_support_diagnostics` 可消费这些受控 JSONL 行；默认不远程传输，
  L1 不实现 per-task Task Log、Audit 降级策略、日志页面或前端通知。

当前已落地的最小 Audit Log 能力：
- 安装、卸载和后端受控回滚/重装收敛任务会写入最小安装审计事件。普通安装和卸载失败事件在既有短 id/计数字段外只增加与 task event 一致的稳定 `error_code`，不会记录原始 repository 或文件系统错误。
- T13-02 批量安装、T13-03 批量卸载和 T13-04 批量真正重装的 batch-level Audit 只使用设计白名单字段；
  T13-05 Sandbox runtime/CLI 复用同一 writer 和脱敏投影，不建立第二套审计格式。字段仅包含短
  `task_id`/`batch_id`、稳定 `operation`、`execution_policy`、`attempt_number`、item/result 聚合计数和
  稳定 `error_code`；不记录 plan token、digest、路径或 item target 列表。每个 item 继续复用已有单项
  安装/卸载/真正重装 Audit。真正重装 post-commit、cleanup 或 Audit 故障保留 committed item 事实并
  标记 evidence degraded，不伪造 rollback。若 retry 在 scope admission 竞争失败后
  无法安全回收未执行 attempt，失败事件只以 `batch_retry_cleanup_ineligible` 或
  `batch_retry_cleanup_failed` 区分 guard 拒绝与 repository 故障，不记录原始数据库错误。
- 若 batch Audit 已写入而 journal 终结失败，runner 会追加 `result=interrupted`、
  `error_code=batch_journal_interrupted` 的纠正事件；对应 attempt 标记 evidence degraded 并禁止
  retry，不把已提交的玩家文件伪造成失败或回滚。
- `rollback_install` 与 `reconcile_reinstall` 事件只记录 `task_id`、`game_id`、`mod_id`、`profile_id`、`remove_file_count`、`restore_file_count` 和 `backup_count` 等短 id/计数；`reconcile_reinstall` 的计数既可表示 post-commit cleanup，也可表示受控恢复到 pre-reinstall 状态的 remove/restore/snapshot cleanup 数量。事件不记录完整本地路径、backup/snapshot ref 或 root、manifest 正文、sandbox/cache 路径或第三方 Mod 内容。
- `reinstall_mod` 事件只允许 `task_id`、`game_id`、`profile_id`、`mod_id`、`previous_revision_id`、`candidate_revision_id`、四类 target 聚合计数，以及失败时的稳定 `error_code` / `rollback_result`。ARMOR 同 revision target switch 在这份既有白名单上唯一新增可选 `target_id`；不记录 target 列表、binding、staging、plan token、完整路径、backup/snapshot ref、manifest/source 正文、hash 列表或第三方 Mod 内容。
- 手动存档备份任务会写入最小存档备份审计事件。成功事件只记录 `task_id`、`game_id`、`profile_id`、`backup_id`、`trigger`、`file_count` 和 `archive_size_bytes` 等短 id/计数；失败事件只记录稳定 `error_code`，不记录完整存档目录、备份目录、Steam ID、manifest 正文、存档内容或 hash 列表。
- P7.2a 后台平台注册生命周期会写入 category `save_backup`、operation `background_registration` 的最小审计事件。除顶层 result 外，fields 只允许 `registration_status`、`task_schema_version` 和稳定 `error_code`；不得记录 task name、SID、worker path、PowerShell executable/script、task XML、原始 stdout/stderr、CIM exception、Profile/save/backup 路径或用户名。
- 后台状态查询本身只读且不写重复审计；register/update/unregister 只有在受控 app use case 中才记录结果。ownership conflict、permission、invalid output 和 timeout 只映射稳定 code，不持久化原始平台错误。
- 用户在 fail-closed 退出提示中明确选择当次仍然退出时，后端写入 category `save_backup`、operation `background_exit_override`、result `success` 的最小审计事件。fields 只允许 `protection_status` 和稳定 `error_code`；`starting` 的 error code 为空字符串，不得为补齐字段而记录原始错误。
- `background_exit_override` 不记录 Profile/game id、task name、SID、worker id/path、PowerShell/XML、lease、完整本地路径、存档/备份内容或前端文案。override command 会在后端重新计算 guard；若审计不可用，只写脱敏 warning 并允许这次已经明确确认的退出，不能永久困住用户。
- `export_preview_image_diagnostics` 成功写入受控预览图诊断 zip 后，会在 app data 下的 `logs/audit/audit-YYYY-MM-DD.log` 写入 JSONL 审计事件，日期来自事件时间戳；若诊断 zip 写入失败，也会先写入失败审计事件。
- 该事件只记录操作名、类别、结果、导出文件名/ID、大小、稳定错误分类和聚合计数，不记录完整本地路径、原始错误文本、`thumbnailUrl`、`contentHash`、sandbox/cache 路径、README 全文、原始 Mod 包内容或原始日志。
- `hmm-ports` 已提供最小 `TextLogReader` port，`hmm-infra` 可从 app data 下的 `logs/app`、`logs/debug` 与 `logs/tasks` 读取最近 N 行已校验文本；读取时会跳过不符合白名单文件名的日志、空行、包含控制字符或敏感片段的行，只返回安全文件名和文本行。Debug 类别只有在有内容时才创建目录。该读取能力已通过 `get_diagnostics_page_snapshot` 与 `export_support_diagnostics` 的 app service/command 链路受控使用。
- `hmm-ports` 已提供最小 `DiagnosticsEnvironmentProvider` port，`hmm-infra` 可生成应用版本、平台 OS、CPU 架构和受控 game adapter id 列表的诊断摘要；该摘要不读取或返回本地路径、Steam ID、token/cookie/API key，并已通过 `export_support_diagnostics` 的 app service/command 链路受控使用。
- `hmm-ports` 已提供最小 `AuditLogReader` port，`hmm-infra` 可从 app data 下的审计 JSONL 中读取最近 N 条已校验事件，作为后续完整日志/审计诊断包的基础；读取时会跳过损坏 JSONL 行或未通过脱敏校验的事件，只返回已校验事件。该读取能力已通过 `export_audit_log_diagnostics` 的 app service/command 链路受控使用，但仍未纳入当前预览图诊断 zip。
- `export_audit_log_diagnostics` 已提供最小后端命令：通过 `AuditLogReader` 读取最近 N 条已校验审计事件并写入受控 `audit-log-diagnostics.json` 诊断包，同时为该导出动作写入最小 Audit Log 事件；单次导出最多包含 200 条审计事件，避免诊断包无界膨胀；命令 DTO 只返回文件名、大小和事件计数，不返回审计事件正文或路径。
- `export_support_diagnostics` 已提供最小后端命令：通过 `SupportDiagnosticsExportService` 把平台摘要、已校验 App Log/Debug Log/Task Log 文本行和已校验 Audit Log 事件组合写入受控 `support-diagnostics.json`、`app-log-diagnostics.json`、`debug-log-diagnostics.json`、`task-log-diagnostics.json` 和 `audit-log-diagnostics.json` 诊断 zip，并为该导出动作写入最小 Audit Log 事件；若平台摘要、任一日志类别、Audit Log 读取或诊断 zip 写入失败，也会先写入只含稳定 `error_code` 和聚合计数的失败 Audit Log 事件，不记录原始错误文本或路径；命令不接受输出路径、日志路径、类别选择、行数或事件数量参数，DTO 只返回文件名、大小和聚合计数，不返回日志正文、审计事件正文或路径。
- L2 已新增 per-task Task Log writer：统一消费与 `hmm://task-progress` 相同的 `taskId/kind/status/phase/current/total`，按 `logs/tasks/task-<task_id>.log` 隔离写入 JSONL，并由同一 task 的首个事件计算 `durationMs`。writer 不记录自由文本 `message`、原始 `error`、`resultRef`、本地路径或第三方 Mod 内容；只有通过稳定 code 校验的错误码可以进入 Task Log。
- L2 为 Audit 写入增加 `best_effort` 与 `report_after_commit` 显式策略。安装、卸载、重装、retarget、recovery 和存档备份的成功事实在玩家文件或 manifest 已提交后若写 Audit 失败，只更新证据健康为 `audit_write_failed_after_commit`，不得再次修改玩家文件或伪造业务回滚；调度 deferred、后台 worker 错误等非提交事实使用 best-effort，但失败仍累计为 `audit_write_failed`，不再静默消失。
- Task/Audit writer 与 retention 共享只增不减的进程内证据健康快照。`export_support_diagnostics` 返回并在 `support-diagnostics.json` 中写入稳定状态与聚合计数：`taskLogStatus`、`auditLogStatus`、`taskLogWriteFailureCount`、`taskLogRetentionFailureCount`、`auditWriteFailureCount`、`auditWriteFailureAfterCommitCount`、`auditLogRetentionFailureCount`。Task 状态为 `ok | task_log_retention_failed | task_log_write_failed`；Audit 状态为 `ok | audit_log_retention_failed | audit_write_failed | audit_write_failed_after_commit`，且 write/post-commit 严重度不会被后续 retention failure 降低。该摘要不包含路径、正文或原始平台错误；`app_health` 仍只表示 App Log 健康，不混入 Task/Audit 语义。
- 若审计写入失败，命令不报告导出成功；当前预览图 zip 仍只包含脱敏聚合摘要，不等同于完整日志/审计诊断包导出。

## MVP 落地要求

在实现 Mod 安装前，至少应先落地：

- Rust logging / telemetry 模块。
- `tracing` 初始化。
- `task_id` 生成与任务事件类型。
- 路径、Steam ID、token 的脱敏 helper。
- 日志目录定位。
- Audit Log 写入接口。
- 针对脱敏和审计事件的单元测试。

没有这些基础能力前，不应开始实现真实游戏目录写入。

## 测试要求

至少覆盖：

- home 路径脱敏。
- 游戏目录路径脱敏。
- Steam ID 脱敏。
- token、API key、cookie 脱敏。
- `task_id` 在任务日志和进度事件中一致传播。
- 文件写入、覆盖、删除、备份、回滚都会产生 Audit Log。
- 后台 register/update/unregister 审计字段满足白名单，且不包含 task/SID/path/PowerShell/XML/raw output。
- 诊断包不包含真实存档、第三方 Mod 包、完整本地路径或明显敏感信息。

测试必须使用人工构造的路径、临时目录和最小样本，不能依赖真实游戏安装目录或真实玩家存档。
