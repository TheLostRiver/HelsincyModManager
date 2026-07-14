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

用于开发构建或用户主动开启的临时诊断。Debug Log 仍必须经过脱敏，不允许因为是调试日志就输出敏感原文。

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

最大总占用空间必须可配置。达到空间限制时，优先删除最旧的 Debug Log 和 Task Log；Audit Log 删除必须遵守审计保留策略。

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

诊断包用于用户主动反馈问题，必须默认脱敏。

可包含：

- 已脱敏的 App Log。
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

当前已落地的最小 Audit Log 能力：
- 安装、卸载和后端受控回滚/重装收敛任务会写入最小安装审计事件。普通安装和卸载失败事件在既有短 id/计数字段外只增加与 task event 一致的稳定 `error_code`，不会记录原始 repository 或文件系统错误。
- `rollback_install` 与 `reconcile_reinstall` 事件只记录 `task_id`、`game_id`、`mod_id`、`profile_id`、`remove_file_count`、`restore_file_count` 和 `backup_count` 等短 id/计数；`reconcile_reinstall` 的计数既可表示 post-commit cleanup，也可表示受控恢复到 pre-reinstall 状态的 remove/restore/snapshot cleanup 数量。事件不记录完整本地路径、backup/snapshot ref 或 root、manifest 正文、sandbox/cache 路径或第三方 Mod 内容。
- `reinstall_mod` 事件只允许 `task_id`、`game_id`、`profile_id`、`mod_id`、`previous_revision_id`、`candidate_revision_id`、四类 target 聚合计数，以及失败时的稳定 `error_code` / `rollback_result`；不记录 target 列表、完整路径、backup/snapshot ref、manifest/source 正文、hash 列表或第三方 Mod 内容。
- 手动存档备份任务会写入最小存档备份审计事件。成功事件只记录 `task_id`、`game_id`、`profile_id`、`backup_id`、`trigger`、`file_count` 和 `archive_size_bytes` 等短 id/计数；失败事件只记录稳定 `error_code`，不记录完整存档目录、备份目录、Steam ID、manifest 正文、存档内容或 hash 列表。
- P7.2a 后台平台注册生命周期会写入 category `save_backup`、operation `background_registration` 的最小审计事件。除顶层 result 外，fields 只允许 `registration_status`、`task_schema_version` 和稳定 `error_code`；不得记录 task name、SID、worker path、PowerShell executable/script、task XML、原始 stdout/stderr、CIM exception、Profile/save/backup 路径或用户名。
- 后台状态查询本身只读且不写重复审计；register/update/unregister 只有在受控 app use case 中才记录结果。ownership conflict、permission、invalid output 和 timeout 只映射稳定 code，不持久化原始平台错误。
- 用户在 fail-closed 退出提示中明确选择当次仍然退出时，后端写入 category `save_backup`、operation `background_exit_override`、result `success` 的最小审计事件。fields 只允许 `protection_status` 和稳定 `error_code`；`starting` 的 error code 为空字符串，不得为补齐字段而记录原始错误。
- `background_exit_override` 不记录 Profile/game id、task name、SID、worker id/path、PowerShell/XML、lease、完整本地路径、存档/备份内容或前端文案。override command 会在后端重新计算 guard；若审计不可用，只写脱敏 warning 并允许这次已经明确确认的退出，不能永久困住用户。
- `export_preview_image_diagnostics` 成功写入受控预览图诊断 zip 后，会在 app data 下的 `logs/audit/audit-YYYY-MM-DD.log` 写入 JSONL 审计事件，日期来自事件时间戳；若诊断 zip 写入失败，也会先写入失败审计事件。
- 该事件只记录操作名、类别、结果、导出文件名/ID、大小、稳定错误分类和聚合计数，不记录完整本地路径、原始错误文本、`thumbnailUrl`、`contentHash`、sandbox/cache 路径、README 全文、原始 Mod 包内容或原始日志。
- `hmm-ports` 已提供最小 `TextLogReader` port，`hmm-infra` 可从 app data 下的 `logs/app/app-YYYY-MM-DD.log` 与 `logs/tasks/task-<task_id>.log` 读取最近 N 行已校验文本；读取时会跳过不符合白名单文件名的日志、空行、包含控制字符或敏感片段的行，只返回安全文件名和文本行。该读取能力已通过 `export_support_diagnostics` 的 app service/command 链路受控使用。
- `hmm-ports` 已提供最小 `DiagnosticsEnvironmentProvider` port，`hmm-infra` 可生成应用版本、平台 OS、CPU 架构和受控 game adapter id 列表的诊断摘要；该摘要不读取或返回本地路径、Steam ID、token/cookie/API key，并已通过 `export_support_diagnostics` 的 app service/command 链路受控使用。
- `hmm-ports` 已提供最小 `AuditLogReader` port，`hmm-infra` 可从 app data 下的审计 JSONL 中读取最近 N 条已校验事件，作为后续完整日志/审计诊断包的基础；读取时会跳过损坏 JSONL 行或未通过脱敏校验的事件，只返回已校验事件。该读取能力已通过 `export_audit_log_diagnostics` 的 app service/command 链路受控使用，但仍未纳入当前预览图诊断 zip。
- `export_audit_log_diagnostics` 已提供最小后端命令：通过 `AuditLogReader` 读取最近 N 条已校验审计事件并写入受控 `audit-log-diagnostics.json` 诊断包，同时为该导出动作写入最小 Audit Log 事件；单次导出最多包含 200 条审计事件，避免诊断包无界膨胀；命令 DTO 只返回文件名、大小和事件计数，不返回审计事件正文或路径。
- `export_support_diagnostics` 已提供最小后端命令：通过 `SupportDiagnosticsExportService` 把平台摘要、已校验 App Log 文本行、已校验 Task Log 文本行和已校验 Audit Log 事件组合写入受控 `support-diagnostics.json`、`app-log-diagnostics.json`、`task-log-diagnostics.json` 和 `audit-log-diagnostics.json` 诊断 zip，并为该导出动作写入最小 Audit Log 事件；若平台摘要、App Log、Task Log、Audit Log 读取或诊断 zip 写入失败，也会先写入只含稳定 `error_code` 和聚合计数的失败 Audit Log 事件，不记录原始错误文本或路径；命令不接受输出路径、日志路径、类别选择、行数或事件数量参数，DTO 只返回文件名、大小和聚合计数，不返回日志正文、审计事件正文或路径。
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
