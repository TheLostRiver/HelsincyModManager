# 前后端通信契约设计

本文档定义 Helsincy Mod Manager 中 React 前端、Tauri command 和 Rust 应用层之间的通信契约。目标是让 UI 能稳定调用后端能力，同时不把文件系统规则、游戏适配规则、安装安全策略或长任务并发细节泄漏到前端。

本文是长期架构契约，不是单次功能实现计划。具体功能仍应在对应设计文档或实现计划中说明。

## 目标

- 统一 Tauri command 的命名、参数、返回值和错误结构。
- 明确 DTO、领域模型、应用服务和前端 view model 的边界。
- 为长任务、进度事件、取消和最终结果查询提供一致模式。
- 防止前端承担路径拼接、retarget 改写、安装计划、备份、回滚、冲突检测等后端职责。
- 为后续 Mod 导入、安装、替换目标映射和存档备份提供可扩展通信基线。

## 非目标

- 不在本文实现完整 RPC 框架或代码生成系统。
- 不要求一次性重写现有 `game_setup` command。
- 不定义具体 UI 布局或交互细节。
- 不绕过 `InstallPlan`、manifest、backup、rollback 或游戏 adapter 设计。
- 不让前端直接获得真实缓存路径、真实游戏路径或宽泛文件系统能力。

## 分层边界

```text
React UI / hooks / view model
  只处理展示、交互和局部状态

feature typed API
  封装 invoke，提供 TypeScript 输入/输出类型

Tauri commands
  薄边界：参数校验、DTO 转换、调用 AppState 暴露的 runtime 服务

hmm-runtime
  Tauri-free composition root：装配 app、ports、infra 和 game adapters

hmm-app
  用例编排：依赖 ports，不依赖具体文件系统或平台实现

hmm-ports
  应用层依赖的 traits/interfaces，不包含具体 infra

hmm-core / hmm-infra / hmm-games-*
  领域模型、真实 I/O 和游戏适配规则
```

前端可以展示 `pathLabel`、`displayName`、`internalId` 等后端提供的字段，但不能据此拼接写入路径或推断安装行为。

## Command 命名

Tauri command 使用 `snake_case`，以动词或查询动作开头：

- 查询状态：`get_game_setup_status`
- 查询应用健康：`app_health`
- 校验输入：`validate_game_directory`
- 保存配置：`save_game_directory`
- 扫描候选：`scan_game_candidates`
- 启动自检并自动保存有效发现：`auto_detect_game_directory`
- 查询前置依赖状态：`get_game_prerequisite_status`
- 预览计划：`preview_install_plan`、`preview_retarget_plan`
- 启动长任务：`start_import_mod_task`
- 查询导入结果：`get_mod_library`、`get_mod_detail`、`get_mod_dependency_graph`、`get_mod_detail_preview_image`
- 分类管理：`create_category`、`update_category`、`delete_category`、`list_categories`、`set_mod_categories`、`get_mod_categories`
- Mod 展示元数据：`update_mod_metadata`、`delete_mod_metadata`
- Profile 管理：`list_profiles`、`get_active_profile`、`create_profile`、`update_profile`、`delete_profile`、`set_active_profile`
- Profile 存档备份与恢复：`start_save_backup_task`、`list_save_backups`、`check_auto_save_backup`、`get_save_backup_background_status`、`preview_save_restore`、`start_save_restore_task`
- 全局存档后台保护：`get_save_backup_background_control_status`、`enable_save_backup_background_protection`、`disable_save_backup_background_protection`
- Profile 存档目录发现：`discover_profile_save_directories`、`confirm_profile_save_directory_candidate`
- 窗口生命周期：`hide_main_window_to_tray`、`get_app_exit_guard`、`exit_app`
- 游戏启动：`launch_game(gameId)`
- 查询安装恢复摘要：`scan_install_recovery`
- 查询安装恢复动作预览：`preview_recovery_action`
- 启动安装恢复动作任务：`start_recovery_action_task`
- 查询诊断摘要：`get_preview_image_diagnostics`
- 导出诊断包：`export_preview_image_diagnostics`
- 导出审计日志诊断包：`export_audit_log_diagnostics`
- 导出完整支持诊断包：`export_support_diagnostics`
- 手动后端维护：`maintain_thumbnail_cache`
- 读取和写入受控设置：`get_thumbnail_cache_settings`、`set_thumbnail_cache_settings`、`get_log_storage_settings`、`set_log_storage_settings`、`get_debug_log_settings`、`set_debug_log_settings`
- 取消长任务：`cancel_task`
- T17 批量迁移：`select_external_import_source`、`start_external_import_scan`、`get_external_import_preview`、`create_external_import_selection`、`update_external_import_selection`、`select_all_external_import_candidates`、`start_external_import_batch`、`retry_external_import_batch`、`get_external_import_batch_result`
- ARMOR 替换目标：`list_replacement_targets`、`analyze_imported_mod_replacement`、`preview_initial_retarget_install`、`start_retarget_install_task`、`preview_retarget_reinstall`、`start_retarget_reinstall_task`

命名应表达用例，而不是底层文件操作。禁止新增类似 `copy_file`、`delete_path`、`read_any_file` 这类宽泛文件系统 command。

## 应用健康与 App Log

L3 新增无参数 `get_diagnostics_page_snapshot`。后端固定每类最多返回 100 条已校验内容，返回平台安全摘要、
App/Debug/Task 安全日志行、已校验 Audit 事件及稳定健康状态。契约不接受或返回日志路径、任意文件名、原始错误；
单类读取失败只以稳定 `*_read_failed` 状态降级。前端只允许复制事件中已校验的 `error_code` / `task_id`。

`evidenceHealth` 是写入链路的聚合健康对象：`debugLogStatus` / `taskLogStatus` / `auditLogStatus` /
`logStorageStatus` 是稳定状态码；`debugLogEventRejectedCount` / `debugLogWriteFailureCount` /
`debugLogRetentionFailureCount` 分别累计 Debug 事件拒绝、写入失败和保留清理失败；
`taskLogWriteFailureCount` / `auditWriteFailureCount` 分别累计 Task/Audit 写入失败；
`taskLogRetentionFailureCount` / `auditLogRetentionFailureCount` 分别累计 Task/Audit 保留清理失败；
`auditWriteFailureAfterCommitCount` 累计玩家文件与 manifest 已提交后发生的审计写入失败。此时
`auditLogStatus` 为 `audit_write_failed_after_commit`，页面必须显示诊断退化，但不得为补写审计再次改动玩家文件。
`logStorageFailureCount`、`logStorageUnsatisfiedCount` 和 `logStorageSettingsFailureCount` 分别累计预算维护失败、
受保护文件导致无法收敛和 settings 读取/校验失败；`logStorageStatus` 稳定值为
`ok | log_storage_settings_unavailable | log_storage_budget_unsatisfied | log_storage_budget_failed`，严重度按该顺序递增。
Task 状态为 `ok | task_log_retention_failed | task_log_write_failed`；Audit 状态为
`ok | audit_log_retention_failed | audit_write_failed | audit_write_failed_after_commit`。write 与
post-commit failure 的严重度高于 retention failure，后续清理失败不能覆盖或降低更严重状态。

Debug 设置 DTO 固定为 `{ enabled: boolean }`。`get_debug_log_settings` 无参数；
`set_debug_log_settings` 只接受顶层 `enabled`，不接受路径、文件名、类别、过滤器或自由文本。
设置持久化成功后立即更新当前进程的共享开关，保存失败保持旧状态。前端设置页必须提供
loading/saving/error/retry 状态，并且不能把该持久化开关混入仅当前会话有效的预览设置 dirty state。

`app_health()` 不接收参数，只返回下面的稳定字符串之一：

| 值 | 含义 |
| --- | --- |
| `ok` | App Log 初始化后尚未检测到受控事件拒绝、保留或写入失败 |
| `app_log_event_rejected` | 专用 sink 拒绝了未知、重复、敏感或不合规字段 |
| `app_log_retention_failed` | App Log 过期文件清理失败 |
| `app_log_write_failed` | App Log 时钟、目录、序列化或文件写入失败 |
| `app_log_initialization_failed` | 日志目录准备或全局 subscriber 初始化失败 |

前端 `AppHealth` 必须穷举这些值并对未知值 fail closed。该状态只表示诊断链路健康，不是安装、
manifest、backup、rollback、recovery 或 Task Log 的事实来源；退化不会新增玩家目录写入。command 不返回
日志目录、日志正文、原始平台错误、用户名、Steam ID、credential、存档或第三方 Mod 内容。
退化状态在当前进程生命周期内保持已观测到的最严重值，应用重启时重新初始化；前端不能把它解释为
仅代表最近一次事件，也不能据此推断安装或恢复状态。

L1 App Log 只消费后端专用安全事件 envelope。任务注册只记录 `taskId/kind/status/phase` 的 queued 摘要；
游戏发现只记录 `gameId/outcome/candidateCount` 或稳定错误码。Task/Audit writer、reader、写入失败策略与
retention 健康现由 `get_diagnostics_page_snapshot` 的 `evidenceHealth` 投影；`app_health` 仍只表示 App Log，
不得混入 Task/Audit 状态或据此推断玩家文件事实。

## DTO 边界

Rust DTO 放在 `src-tauri/src/` 下，负责跨 Tauri 边界的序列化形状。DTO 不应成为领域模型的替代品。

要求：

- Rust DTO 使用 `#[serde(rename_all = "camelCase")]`。
- 前端类型使用 camelCase，与实际 JSON 形状一致。
- DTO 中的 enum 以稳定字符串表达，优先使用 `snake_case` 值。
- 后端可以返回展示标签，例如 `pathLabel`，但不应把完整敏感路径作为默认展示字段。
- `metadata` 可用于透传游戏专属展示信息，但前端不能基于 metadata 拼接安装路径或改写资源编号。

示例：

```rust
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandErrorDto {
    pub code: String,
    pub message: String,
    pub category: String,
    pub retryable: bool,
    pub task_id: Option<String>,
}
```

当前 `CommandErrorDto` 只有 `code` 和 `message`。后续新增高风险或长任务 command 时，应扩展为上面的通用形态，旧 command 可以分阶段迁移。

### T17 批量迁移后端契约与 Slice 4C 结果/重试前端边界

已完成的 Slice 2 提供 `hunting_box_directory_v1` 的 Windows + MHW:I 只读来源选择、扫描和分页预览。已完成的
Slice 3 在此基础上增加后端 selection snapshot、task-scoped 安全物化、批量导入编排和结果分页；原生目录选择器只在
Rust 内返回路径，路径立即登记到短生命周期的 source registry。前端永远不接收、提交或记录该路径。

Slice 4A 的 React 工作流消费来源选择、scan task、取消和基础分页 preview。PR #198 的 Slice 4B 在同一入口内增加
后端 selection snapshot、selection-aware preview、候选 mutation、服务端全选、已有分类映射、显式冲突决定、
sealed batch start、取消和严格按 `taskId` 的 import progress。PR #199 的 Slice 4C 消费权威分页 result、partial
success 与 retry；它仍不计算路径、物化包、安装、启用、获取 game/profile 写锁或写入游戏目录。

| command | 输入 | 返回 |
| --- | --- | --- |
| `select_external_import_source` | 无 | `ExternalImportSourceDto \| null`；取消原生选择器时返回 `null` |
| `start_external_import_scan` | `sourceId` | `{ task: TaskStartedDto, batchId }` |
| `get_external_import_preview` | `batchId`、可选 `selectionId`、可选 `cursor`、可选 `limit` | 脱敏的 `ExternalImportPreviewPageDto`；绑定 selection 时返回 summary 与当前页选择事实 |
| `create_external_import_selection` | `batchId` | `ExternalImportSelectionDto` |
| `update_external_import_selection` | `selectionId`、`expectedRevision`、`entries` | `ExternalImportSelectionMutationResultDto` |
| `select_all_external_import_candidates` | `selectionId`、`expectedRevision` | `ExternalImportSelectionMutationResultDto` |
| `start_external_import_batch` | `batchId`、`selectionId`、`expectedRevision` | `{ task: TaskStartedDto, batchId }` |
| `retry_external_import_batch` | `batchId`、已 sealed 的 `selectionId` | `{ task: TaskStartedDto, batchId }` |
| `get_external_import_batch_result` | `batchId`、可选 `cursor`、可选 `limit` | 脱敏的 `ExternalImportBatchResultPageDto` |

`sourceId`、`batchId` 与 `selectionId` 会先去除两端空白，再按受限字符集校验为 opaque ID；去除后为空、包含路径/URI/内部空白或超长的值整体拒绝。preview cursor 是
后端解释的十进制 offset token，前端应只复用响应的下一 cursor，省略时从 `0` 开始；`limit` 默认 `50`、最大
`100`，不在 `1..=100` 的值整体拒绝。响应只包含 batch/source adapter 的稳定 ID、scan/import status、
可选 selection summary、candidate ID、受限 metadata hint、文件数、总字节数、preview/conflict/reason code、
当前页候选 `selected` 与可选 `{ conflictResolution, categoryId }` decision、总数和下一 cursor；不得包含
selection 的 batch/entries/updatedAt、source fingerprint、source item key hash、content fingerprint、路径、
XML、archive/sandbox/cache ref 或第三方文件内容。

省略 `selectionId` 时响应 `selection = null`，所有当前页候选必须为 `selected = false` 且 decision 为 `null`。
提供 `selectionId` 时 selection 必须存在并绑定同一 batch；不存在或跨 batch 均返回
`external_import_selection_unavailable`，不能静默降级成无 selection，也不能泄露跨 batch selection 是否存在。
仍处于 `editing` 但已达到 `expiresAtUnixMillis` 的 selection 可以在只读响应中派生为 `expired`，该查询不得产生
repository 写入。

`update_external_import_selection.entries` 必须包含 `1..=200` 个 mutation；每项只包含同一 batch 的 opaque
`candidateId`、`selected` 和可选的稳定冲突/分类决定。重复、未知、跨 batch、blocked、空 mutation、过期或
sealed selection 全部整体拒绝，不截断、不部分应用。`select_all_external_import_candidates` 使用固定的后端
“所有 ready 候选”谓词，不接受前端展开的 candidate ID 数组，并继续受 10,000 项和资源预算限制。
`start_external_import_batch` 只消费 batch/selection/revision 三元组，并通过短 SQLite 事务封存 selection；
retry 只可重放 sealed snapshot 中仍可重试的项。结果 cursor 与 preview cursor 一样是十进制 offset token，
默认 `limit = 50`、最大 `100`，响应只返回 candidate ID、状态、稳定 reason code、可选内部 `modId`、
`retryable`、总数和下一 cursor。

前端在同一 import task 进入 `completed`、`failed` 或 `cancelled` 后，必须从 cursor `0` 查询结果；progress
event 的聚合计数不得解释为 partial success。result page 必须按 exact DTO key、同一 batch、稳定终态 status/
reason、opaque ID、页大小和重复 candidate fail closed；后续页只复用 `nextCursor` 并按 candidate ID 去重。
`retry_external_import_batch` 只接收 `batchId + sealed selectionId`，前端不得提交候选 ID 或重建 retryable
谓词；成功返回的新 taskId 必须复用同一 taskId-scoped listener、early-event buffer、取消和终态状态机。
每个 terminal taskId 只有在首屏权威结果验证成功后才可触发一次 best-effort Mod 库刷新；刷新失败不能改写
batch/result 事实或伪造导入失败。

稳定错误至少包括 `external_import_source_picker_unavailable`、`external_import_source_unavailable`、
`external_import_source_id_invalid`、`external_import_batch_id_invalid`、
`external_import_selection_id_invalid`、`external_import_candidate_id_invalid`、
`external_import_category_id_invalid`、
`external_import_preview_cursor_invalid`、`external_import_preview_limit_invalid`、
`external_import_task_unavailable`、`external_import_batch_unavailable`、`external_import_scan_failed`、
`external_import_clock_unavailable`、`external_import_result_cursor_invalid`、
`external_import_result_limit_invalid`、`external_import_selection_unavailable`、
`external_import_batch_not_startable`、`external_import_catalog_unavailable`、
`external_import_category_unavailable`、`external_import_result_request_invalid`、
`selection_revision_conflict`、`selection_empty`、
`selection_mutation_empty`、`selection_mutation_limit_exceeded`、`selection_total_limit_exceeded`、
`selection_resource_limit_exceeded`、`selection_candidate_invalid`、`selection_expired` 和 `selection_closed`。
错误 message 不能回显路径或底层 I/O 文本。

### T13 批量生命周期规划契约

本节登记 [批量 Mod 生命周期领域设计](BATCH_MOD_LIFECYCLE_DESIGN.md) 的 transport 形状，用于约束
T13-01 至 T13-08。**T13-06 已实现下列 command、DTO、AppState service 和 typed API；T13-07 前端
工作流已接入它们，但当前 GUI 必须在 Sandbox 模式（`HMM_SANDBOX_DATA_DIR`）下才可用：**

```text
get_batch_mod_lifecycle_capability
preview_batch_mod_lifecycle
seal_batch_mod_lifecycle
start_batch_mod_lifecycle
get_batch_mod_lifecycle_result
retry_batch_mod_lifecycle
```

取消继续复用受控 `cancel_task(taskId)`，但只有 `start`/`retry` 返回真实 `TaskStartedDto` 后才有可取消
task；当前 start/retry 同步执行完整批次，返回的 task 已是终态（取消返回
`task_cannot_be_cancelled`）。运行中取消与中间 progress 事件不属于当前已认证契约；如需开放，必须
作为独立异步化任务设计和验证。
前端不能在本地循环调用单项 install/uninstall/reinstall command 来构造批次。

preview/seal request 使用同一完整输入；`items` 元素是带 `operation` tag 的 discriminated union：

```text
BatchModLifecycleRequestDto
  schemaVersion             必须为整数 1
  operation                install | uninstall | reinstall
  gameId
  profileId
  executionPolicy          stop_on_failure | continue_on_item_failure
  items[]                  每个元素必须携带与 request.operation 一致的 operation tag
  replacementTargets[]     （可选，仅 reinstall）same-revision target switch 的 modId -> targetId

InstallBatchItemDto（operation: "install"）
  modId
  revisionId
  layer                     { name, priority }

UninstallBatchItemDto（operation: "uninstall"）
  modId
  expectedInstalledRevisionId

ReinstallBatchItemDto（operation: "reinstall"）
  modId
  installedRevisionId
  candidateRevisionId
  layer
```

输入不包含路径、package file id、manifest generation、backup/snapshot ref、hash、digest、item ID、
plan token 或 replacement binding 快照。same-revision reinstall（`installedRevisionId ==
candidateRevisionId`）通过 `replacementTargets` 表达目标选择，binding 由后端在 seal 时从受控
`targetId` 解析；前端不得构造或回传 `replacementBindingSnapshot`。一个 request 只允许一种
operation、一个 game/profile，最多 100 项；同一 `modId` 重复时整体拒绝。后端按稳定 item key 规范
排序，前端选择顺序不定义执行顺序。

| command | 输入 | 返回 |
| --- | --- | --- |
| `get_batch_mod_lifecycle_capability` | 无 | `BatchModLifecycleCapabilityDto`；只包含 `previewAvailable`、`writeAvailable` 和可选稳定 `unavailableReasonCode`。Production 未接入 Sandbox 时两者为 `false` 且 reason 为 `sandbox_batch_production_forbidden`；该 DTO 只是交互门禁，每个 preview/write command 仍必须逐次重验 Sandbox 环境 |
| `preview_batch_mod_lifecycle` | `request` | 纯只读 `BatchModLifecyclePreviewDto`；包含 status、operation、policy、item/global reason 聚合、action/retained/replaced/added/stale 聚合、ready/blocked 数量和可选 opaque `previewToken` |
| `seal_batch_mod_lifecycle` | 完整 `request`、`previewToken` | `BatchModLifecycleSealDto`；只包含 `batchId`、status、operation、policy、`expiresAtUnixMillis` 和 opaque `planToken` |
| `start_batch_mod_lifecycle` | `batchId`、`planToken` | `{ task: TaskStartedDto, batchId, attemptNumber }`；同步执行 attempt 0 后在返回前发出唯一 terminal event |
| `get_batch_mod_lifecycle_result` | `batchId`、`attemptNumber`、可选 `cursor`、可选 `limit` | `BatchModLifecycleResultPageDto`；cursor 只属于该 attempt |
| `retry_batch_mod_lifecycle` | `batchId`、`expectedAttemptNumber` | `{ task: TaskStartedDto, batchId, attemptNumber }`；retry item set 完全由后端从 sealed batch 和已有终态计算 |

`preview` 必须零写入：不创建 batch journal、projection、Audit、manifest、backup、recovery 或 temp
artifact。`seal` 会重读当前事实并重建 digest；request/token/fact 任一不一致时返回
`batch_plan_stale`，不持久化部分 snapshot。`start` 只消费 `batchId + planToken`，token 默认 30 分钟
过期；digest 是内部确定性身份，不是公开写权限，也不得进入 DTO、日志或诊断。

`start`/`retry` 当前为同步执行模型：command 在完成完整批次后才返回，返回的 `task` 已是终态，
并恰好发出一个 terminal event（`install.batch.<operation>.<terminal>`）。`task.kind` 统一为
`install`；`task.status` 与 phase 的映射为：`completed`/`completed_with_errors` -> `completed`
（phase 分别为 `.completed` / `.completed_with_errors`），`cancelled` -> `cancelled`，
`blocked`/`recovery_required`/`interrupted`/`failed` -> `failed`（phase 分别为 `.failed` /
`.recovery_required`）；权威 batch 状态始终以 result query 的 `status` 为准。当前契约不发出 queued/
planning/preflight/processing/stopping 等中间 phase，`cancel_task` 对 batch task 返回
`task_cannot_be_cancelled`；未来异步化不得在未更新本契约与 Gate 证据时静默改变这些语义。

`previewToken` 和 `planToken` 是唯一允许 token 的两个直接 response 字段。前端只在当前确认流程的
内存中持有，不写 local storage、状态持久化、日志或 diagnostics；调用 `seal`/`start` 后立即丢弃。
后端也不持久化原始 token，只能保存单向 verifier/metadata 或使用经过审计的 keyed token。

Preview `status` 只有 `ready` 和 `blocked`。默认 `stop_on_failure` 只有零 item blocker 时为
`ready`；显式 `continue_on_item_failure` 在没有 global blocker 且至少一个 item ready 时为 `ready`，
此时 `blockedItemCount` 可以大于零。其他情况均为 `blocked` 且 `previewToken = null`。

首版资源上限固定为 100 items、50,000 target actions 和 16 MiB canonical plan。任一上限超出时整体
返回 `batch_resource_limit_exceeded`，不截断、不部分 seal。跨 item 最终 target、卸载 remove/restore
集合或 backup ownership 冲突是 `batch_global_target_conflict`，`continue_on_item_failure` 不能绕过。

Result page 默认 `limit = 50`、最大 `100`；cursor 是后端解释的 opaque 分页值，前端只复用
`nextCursor`。非法 cursor/limit 整体拒绝（`batch_input_invalid`）。页面按 `ordinal` 稳定排序，
summary 携带 `itemCount` 与各状态计数；页面还返回当前 `attemptNumber`；返回 item 仅包含：

```text
itemId
ordinal
modId
status
reasonCode?
retryable
```

（规划草案中的 item `actionSummary` 未落地：CLI 与 journal 均不持久化 per-item action summary，
T13-06 契约以实际实现为准。）

单项终态稳定值为 `succeeded`、`blocked`、`failed`、`recovery_required`、`cancelled`、`skipped`。
Batch 终态稳定值为 `completed`、`completed_with_errors`、`blocked`、`cancelled`、
`recovery_required`、`failed`。`retryable` 是独立布尔事实；成功项和 `recovery_required` 项不能重放。
运行中 commit 收到取消但安全提交成功时，item 仍是 `succeeded`，取消只阻止启动后续项。
同一 attempt 的 start 使用后端 CAS 只登记一个 task；重复调用幂等返回同一 task。Retry 必须匹配最近
terminal `expectedAttemptNumber`，并发 retry 最多一个创建下一 attempt，另一个返回
`batch_attempt_stale`。Result query 和 cursor 必须绑定确切 attempt；新 attempt 不改变旧 attempt 的
分页身份。

batch task phase family 已注册为：

```text
install.batch.<operation>.queued
install.batch.<operation>.planning
install.batch.<operation>.preflight
install.batch.<operation>.processing
install.batch.<operation>.stopping
install.batch.<operation>.completed
install.batch.<operation>.completed_with_errors
install.batch.<operation>.cancelled
install.batch.<operation>.recovery_required
install.batch.<operation>.failed
```

`<operation>` 只能是 `install`、`uninstall` 或 `reinstall`。T13-06 当前只发出 terminal 子集
（`.completed` / `.completed_with_errors` / `.cancelled` / `.recovery_required` / `.failed`）。
每个 attempt 对外只有一个 taskId 和恰好一个 terminal event；progress 只提供聚合计数，大型 item
结果必须通过分页 query 读取。

Batch phase 映射到共享 `TaskProgressEventDto` 时还必须满足：

- `message` 只能是有界脱敏文案，前端不得按文案分支。
- 当前共享 `error` 字段存在时只允许登记过的稳定 code，不得透传原始异常、路径或内部文本；若未来
  改为结构化错误，也只允许白名单 `code` 和 `category`，不能携带原始异常。
- `resultRef` 只能是公开的 opaque `batchId`，不能是路径、token、digest、cursor、manifest、
  backup/snapshot ref 或内部 storage ref。

规划中的 batch-level stable code 至少包括：

```text
batch_input_invalid
batch_duplicate_item
batch_resource_limit_exceeded
batch_global_target_conflict
batch_plan_blocked
batch_plan_stale
batch_plan_expired
batch_token_invalid
batch_recovery_pending
batch_journal_unavailable
batch_write_admission_unavailable
batch_evidence_unavailable
batch_retry_unavailable
batch_attempt_stale
batch_result_unavailable
batch_cancelled
batch_internal_error
```

Item reason 优先复用既有单项 code；批量调度新增
`stopped_after_item_failure`、`cancelled_before_start`、`batch_item_plan_stale`、
`source_revision_changed`、`manifest_changed`、`target_changed`、`rollback_succeeded` 和
`recovery_required`。前端按稳定 code 映射本地化文案，不能根据 message 分支。

Tauri 侧 batch command 只在 Sandbox 模式可用：GUI 启动时读取 `HMM_SANDBOX_DATA_DIR` 环境变量，
指向一个绝对路径的 disposable Sandbox 数据根；未设置、为空或非法时 batch command 返回稳定错误
`sandbox_batch_production_forbidden`（message 为固定脱敏文案），不开放 Production 写入。该目录的
语义与 CLI `--sandbox-data-dir` 一致（游戏配置、mod 目录、manifest 与 batch journal 全部位于
Sandbox 根内）。错误映射：`SandboxBatchAutomationError` 的稳定 `code` 原样透传为
`CommandErrorDto.code`，`message` 为按 code 映射的固定脱敏文案，禁止透传原始异常或路径。

除 preview/seal 对应的直接 response 返回各自 opaque token 外，result、progress/event、其他 DTO、
CLI stdout/JSON/JSONL、Task/Audit Log 和诊断都不得公开完整路径、Windows 用户名、Steam ID、
token、digest、target/hash 列表、backup/snapshot ref、manifest/source/package 正文或原始错误。
公开错误只含稳定 code、category、retryable 与脱敏 message。

CLI-4 只能映射相同 app use case；在 T13-00、CLI-2A/2B/2C、CORE-PREF-01 与 T13-01 至 T13-04
完成前，Sandbox batch parser 也保持不可达。Production batch 写入还必须等待独立跨进程 admission。

## 错误契约

错误分为四类：

| category | 含义 | 前端行为 |
| --- | --- | --- |
| `validation` | 用户输入或请求参数不合法 | 展示可操作提示 |
| `recoverable` | 后端可恢复或用户可重试 | 展示重试/重新扫描/重新选择入口 |
| `conflict` | 安装、依赖、目标路径或 profile 冲突 | 展示冲突详情和解决动作 |
| `internal` | 未预期内部失败 | 展示通用错误，保留诊断入口 |

错误 `code` 必须稳定，适合前端分支和测试，例如：

- `unsupported_game`
- `directory_not_absolute`
- `package_not_found`
- `unsafe_archive_path`
- `ambiguous_replacement_source`
- `target_catalog_missing`
- `install_conflict`
- `task_cancelled`
- `internal_error`

`message` 是用户可读文本，可以随文案调整，不应被前端当作逻辑判断依据。

## 前端 typed API

每个 feature 优先拥有本地 API 文件：

```text
src/features/<feature>/<feature>Types.ts
src/features/<feature>/<feature>Api.ts
```

`src/shared/api/tauri.ts` 只放通用 helper 和稳定 re-export，不应堆积所有 feature 的具体请求。

建议新增统一 helper：

```ts
export function invokeCommand<TResponse>(
  command: string,
  args?: Record<string, unknown>,
): Promise<TResponse>;
```

feature API 负责把 UI 输入整理成 DTO 输入，但不做业务规则推断：

```ts
export function previewRetargetPlan(input: PreviewRetargetPlanInput) {
  return invokeCommand<RetargetPlanPreview>("preview_retarget_plan", input);
}
```

## AppState 和服务装配

`AppState` 是 `HmmRuntime` 的薄包装，只负责解析 Tauri app data、触发 GUI-only 生命周期行为并
向 command 暴露 runtime 服务；它不保存前端 view 状态，也不重新装配业务依赖。

新增服务时应遵循：

1. 在 `hmm-app` 中定义用例服务。
2. 依赖 `hmm-ports` traits。
3. 在 `hmm-runtime` 中组合具体 infra 和 adapter；只有 GUI-only 生命周期留在
   `src-tauri/src/state.rs`。
4. 在 command 中通过 `State<'_, AppState>` 调用服务。

如果服务需要内部可变状态，优先让服务内部用清晰的锁或队列表达，而不是在 command 中临时拼装全局状态。

## Equipment Replacement AR4/AR5/WR-04 契约

AR4 的入口固定在 `Mod 管理 -> Mod 详情统一面板 -> 替换目标 Tab`。右键“MOD 文件修改”只负责用
replacement Tab 打开同一个详情面板，不新增孤立页面。`/replacements` 仍保留给后续全局 binding、
占用和冲突总览。

六个 command 的请求只使用稳定身份：

| command | 请求 | 返回 |
| --- | --- | --- |
| `list_replacement_targets` | `gameId`、`modId`、可选 `query` | 与该 Mod source type/path-family 兼容的 catalog target 列表 |
| `analyze_imported_mod_replacement` | `gameId`、可选 `profileId`、`modId` | source、匹配文件数、warning、`retargetable` 与可选 `installedTargetId` |
| `preview_initial_retarget_install` | `gameId`、`profileId`、`modId`、`targetId`、layer | retarget action、warning 与 InstallPlan 冲突摘要 |
| `start_retarget_install_task` | 与 preview 相同 | `TaskStartedDto` |
| `preview_retarget_reinstall` | `gameId`、`profileId`、`modId`、`targetId`、layer | `ReinstallPlanPreviewDto` 与 plan token |
| `start_retarget_reinstall_task` | 与 preview 相同，另加 `planToken` | `TaskStartedDto` |

前端不得提交 `packageId`、revision package id、source path、sandbox/cache/staging/game root、
`sourceId`、`bindingId`、`internalId` 或最终 target path。前四个 AR4 command 从当前 display revision
重建包事实，重新扫描并分析唯一受支持 source，按 `targetId` 查询 catalog，生成 binding、
`RetargetPlan`、staging 和 `InstallPlan`。WR-04 Weapon preview 会由受限 content reader 从同一受控
revision sandbox 读取 MOD3/MRL3 bytes 并生成 sealed transform invocation；前端不接触 bytes、digest、
transformer 参数或路径。两个 AR5/WR-04 target-switch command 的 revision 来源见下文，不得复用
display revision。target DTO 只返回展示名、alias、稳定 id/internal id、target type 与 `catalogScope`，
不返回原始 catalog metadata。source/action DTO 只投影稳定 type/id/internal id、support 与动作事实，
不返回 source/target relative path 或 path-family；UI preview 只显示 resource type、internal id、动作数、
冲突与 prerequisite。

`preview_initial_retarget_install` 与 `start_retarget_install_task` 只允许目标 Mod 在当前 profile 的恢复
状态严格为 `not_installed`。`installed`、`committed_cleanup_pending`、`cleanup_pending`、
`rollback_required`、`repair_required`、`unknown` 以及状态查询失败全部 fail closed。已安装 Mod 的
target switch 属于 AR5，AR4 不得退化为普通 install 覆盖。

`preview_retarget_reinstall` 与 `start_retarget_reinstall_task` 只用于 recovery status 严格为 `installed`
的同 revision target switch。后端从 manifest 解析 installed revision，再由 repository 和 adapter 重建
package/source/candidate binding/staging/InstallPlan；它不读取当前 display revision 来决定 candidate，
因此导入新 revision 后切换 target 也不会隐式升级 Mod。前端不得提交 revision、package、source、
binding、staging、game root 或最终路径。

当 `analyze_imported_mod_replacement` 携带 `profileId` 时，后端可从可信 manifest 返回唯一、稳定的
`installedTargetId`，仅用于重启后标记“当前已安装”和阻止把当前 target 当成切换候选。manifest 缺失、
Mod 未安装或状态不可信时省略该字段；binding 歧义时返回稳定的安装状态不可用错误。该字段不包含
binding identity、revision、internal id、相对/绝对路径、staging 或 manifest 内容。

同 revision 只有 persisted/candidate binding 证明同一 Mod/profile/source/path-family lineage，且新
`targetId` 与已安装 target 不同时才允许进入真正重装。当前 target、缺失 binding、不安全 recovery
状态、blocking conflict 或 preview token 过期均 fail closed。start 继续使用既有
`install.reinstall.*` phase、game/profile 写锁和 cancellation barrier；前端严格按 `taskId` 匹配事件，
取消入口只在 queued/plan/preflight 安全阶段可见。

稳定错误码至少包括：

- `replacement_unsupported_game`
- `replacement_mod_not_found`
- `replacement_package_unavailable`
- `replacement_analysis_unavailable`
- `replacement_source_not_retargetable`
- `replacement_target_catalog_unavailable`
- `replacement_target_not_found`
- `replacement_install_state_unavailable`
- `replacement_initial_install_blocked`
- `replacement_preview_unavailable`
- `weapon_developer_seed_unavailable`
- `weapon_source_content_unavailable`
- `weapon_cross_family_target`

完整 Weapon catalog 仍受 WR-02B provenance/licensing 门禁。`catalogScope=developer_sandbox` 的人工
weapon target 只有在 GUI runtime 从显式 `HMM_SANDBOX_DATA_DIR` 构造有效 Sandbox environment 时才会
注册；同一 environment 同时启用生命周期 root admission。Production composition 保持 Armor-only，
不能仅通过前端输入或普通 feature flag 打开人工 weapon target 或 Production 写入。

`start_retarget_install_task` 继续使用 `TaskKind::Install`、`hmm://task-progress` 和既有
game/profile 写锁。新增 phase 为 `install.retarget.queued`、`install.retarget.plan.building`、
`install.retarget.commit.processing`、`install.retarget.completed`、`install.retarget.failed`；失败事件的
`error` 使用 `install_retarget_failed:<phase>`，其中 `prerequisite` 表示前置被阻断或 preview 后
decision 漂移。commit 必须继续经过 Audit Log、backup、manifest、
rollback/recovery 链路。原始导入包只读，retarget staging 是可清理、可重建的临时输入，不是事实来源。

## 长任务契约

扫描、解压、hash、分析、安装、备份、恢复、retarget materialize 等耗时操作必须以任务形式表达。

启动 command 返回：

```text
TaskStartedDto
  taskId
  kind
  status
```

进度事件使用统一事件名：

```text
hmm://task-progress
```

事件 payload：

```text
TaskProgressEventDto
  taskId
  kind
  status          // queued / running / completed / failed / cancelled
  phase
  current
  total
  message
  error
  resultRef
```

`phase` 是稳定的点状命名空间字符串，格式为 `<task_kind>.<stage>.<sub>`，前端可据此展示阶段文案，但不能用 `message` 文本做分支。当前已定义的 phase code：

| task kind | phase | 含义 |
| --- | --- | --- |
| `mod_import` | `mod_import.queued` | 导入任务已登记，等待后续执行 |
| `mod_import` | `mod_import.cancelled` | 导入任务被取消；running prepare 会在后端检查点协作式停止 |
| `mod_import` | `mod_import.unpack.started` / `.completed` / `.failed` | 安全解压阶段 |
| `mod_import` | `mod_import.preview_image.processing` | 预览图候选扫描和处理 |
| `mod_import` | `mod_import.preview_image.fallback` | 预览图降级为 fallback，导入继续 |
| `mod_import` | `mod_import.prepare.completed` | prepare 阶段已完成，后续结果通过查询或持久化链路获取 |
| `mod_import` | `mod_import.analyze.processing` | 包结构和依赖分析 |
| `mod_import` | `mod_import.commit.processing` | 写入游戏实例前的 plan 落地 |
| `mod_import` | `external_import.scan.queued` | 第三方来源只读扫描已登记，`resultRef` 为 opaque batch ID |
| `mod_import` | `external_import.scan.discovering` | 只读枚举来源根目录的直接候选，不获取 game/profile 写锁 |
| `mod_import` | `external_import.scan.fingerprinting` | 受限 XML 解析、资源计数和内容指纹阶段；事件不包含候选明细 |
| `mod_import` | `external_import.scan.completed` | 脱敏 preview 已在短 SQLite 事务中持久化，明细通过分页 query 获取 |
| `mod_import` | `external_import.scan.failed` | 扫描失败；`error` 只携带稳定 external-import code |
| `mod_import` | `external_import.scan.cancelled` | 扫描在目录/XML/hash 安全点取消；不会执行物化或游戏写入 |
| `mod_import` | `external_import.import.queued` | 已封存 selection 的批量导入已登记，`resultRef` 为 opaque batch ID |
| `mod_import` | `external_import.import.materializing` | 后端正在重新校验来源并在 app-private task scope 物化内部包；不获取 game/profile 写锁 |
| `mod_import` | `external_import.import.preparing` | 内部包正在复用既有 sandbox 分析链路；事件不包含候选明细或路径 |
| `mod_import` | `external_import.import.persisting` | JSON authority catalog 正在按有界 chunk 持久化；SQLite projection 仍是可重建读模型 |
| `mod_import` | `external_import.import.completed` | 批量导入终态已持久化；partial success 通过结果分页查询表达 |
| `mod_import` | `external_import.import.failed` | 批次级故障停止后续调度；已持久化成功项与结果页不被回滚 |
| `mod_import` | `external_import.import.cancelled` | 导入在安全检查点取消；已成功项保留，未开始项以可重试取消结果持久化 |
| `install` | `install.queued` | 安装任务已登记，等待后续执行 |
| `install` | `install.plan.building` | 后端正在从已导入 Mod 和游戏 adapter 重建 `InstallPlan` |
| `install` | `install.commit.processing` | 后端正在执行受写锁保护的 backup / commit / manifest 流程 |
| `install` | `install.completed` | 安装提交已完成，manifest 已写入 |
| `install` | `install.failed` | 安装提交失败；后端会 best-effort 走回滚或保留可恢复状态 |
| `install` | `install.cancelled` | 安装任务被取消；已进入 commit 阶段后不保证抢占式中断 |
| `install` | `install.uninstall.queued` | 卸载任务已登记，等待后续执行 |
| `install` | `install.uninstall.processing` | 后端正在执行受写锁保护的 manifest 驱动卸载 |
| `install` | `install.uninstall.completed` | 卸载完成，manifest 已移除对应 Mod 的托管条目 |
| `install` | `install.uninstall.failed` | 卸载失败；后端会 best-effort 回滚已应用的删除或恢复 |
| `install` | `install.reinstall.queued` | 真正重装任务已登记，等待后续执行 |
| `install` | `install.reinstall.plan.building` | 后端正在写锁外重建 candidate revision 的 plan/source facts |
| `install` | `install.reinstall.preflight.processing` | 后端正在完成四类 target 聚合与进入 commit 前的预检 |
| `install` | `install.reinstall.commit.processing` | 后端正在写锁内执行 snapshot / mutation / manifest entry-set replacement |
| `install` | `install.reinstall.rollback.processing` | 同步失败后正在恢复 pre-reinstall revision |
| `install` | `install.reinstall.completed` | candidate manifest entry set 已固化并完成受控收尾 |
| `install` | `install.reinstall.failed` | 重装失败，或已越过 commit point 但仍需受控 reconciliation |
| `install` | `install.recovery.queued` | 恢复动作任务已登记，等待后续执行 |
| `install` | `install.recovery.planning` | 后端正在等待写锁并准备受控恢复动作 |
| `install` | `install.recovery.processing` | 后端正在执行受写锁保护的恢复动作 |
| `install` | `install.recovery.completed` | 恢复动作已完成，durable recovery record 已更新 |
| `install` | `install.recovery.failed` | 恢复动作失败；后端会 best-effort 回滚已应用的删除或恢复 |
| `save_backup` | `save_backup.queued` | 存档备份任务已登记，等待后续执行 |
| `save_backup` | `save_backup.scanning` | 后端正在校验并扫描受控存档源目录 |
| `save_backup` | `save_backup.archiving` | 后端正在写入受控 zip 备份 |
| `save_backup` | `save_backup.manifest_writing` | 后端正在写入 sidecar manifest 和 SQLite 历史摘要 |
| `save_backup` | `save_backup.retention_pruning` | 后端正在按保留策略清理旧备份 |
| `save_backup` | `save_backup.completed` | 存档备份已完成 |
| `save_backup` | `save_backup.failed` | 存档备份失败；事件只携带稳定错误 code，不携带完整路径 |
| `save_backup` | `save_backup.cancelled` | 存档备份任务被取消；已进入一致性收尾阶段时以后端状态为准 |
| `save_restore` | `save_restore.queued` | 玩家存档恢复任务已登记，等待 listener 就绪后的后台执行 |
| `save_restore` | `save_restore.preparing` | 锁外重新校验来源、物化受控 staging 并记录目标摘要 |
| `save_restore` | `save_restore.pre_restore_backup` | 锁外创建默认开启的独立 pre-restore 安全备份 |
| `save_restore` | `save_restore.revalidating` | 获取共享 game/profile 写锁后复核短事实、token 和目标/staging 摘要 |
| `save_restore` | `save_restore.committing` | cancellation barrier 内执行目录交换、回滚或 recovery 收尾 |
| `save_restore` | `save_restore.completed` | 恢复事务已 durable completed；`error` 可仅为 `save_restore_evidence_degraded` |
| `save_restore` | `save_restore.failed` | 恢复未提交或已证明回滚；`error` 只携带稳定 code |
| `save_restore` | `save_restore.recovery_required` | 无法证明原状态，事务与受控 recovery evidence 已保留 |
| `save_restore` | `save_restore.cancelled` | transport 已接受 commit barrier 前的协作式取消 |

新增 task kind 时必须在此表登记对应 phase code，避免前端硬编码未登记值。

规则：

- 每个进度事件必须携带 `taskId`。
- 前端不能靠“当前页面只有一个任务”来匹配事件。
- T17 import listener 必须同时匹配 `kind = mod_import`、精确 `taskId` 和登记的
  `external_import.import.*` phase。通用 `mod_import.cancelled` 只表示正在取消，不是 external-import
  专用终态；前端必须等待同一 task 的 `external_import.import.cancelled`。failed/cancelled 的聚合计数不能用于
  推断 partial success。
- 取消使用 `cancel_task(taskId)`；当前实现支持取消 `queued` 和 `running` 任务。running prepare 不会强制杀线程；zip 解压、预览图候选扫描和预览图 processor 会在后端 cancellation token 检查点协作式停止。图片库单次解码/编码调用本身仍不是抢占式中断；install commit 已开始后不做抢占式中断，必须依赖 backup / rollback / manifest 链路保持可恢复状态。T17 scan 在目录/XML/hash 阶段可取消；Slice 3 import 在物化、sandbox 分析和 chunk 间安全点可取消，已成功项保留、未开始项以分页结果表达。写入 preview 或 batch terminal state 的短 SQLite 事务进入取消屏障后以后端终态为准。通用 `mod_import.cancelled` 事件可能先由 `cancel_task` 发出，runner 随后会发送带同一 `taskId` 的 `external_import.scan.cancelled` 或 `external_import.import.cancelled` 终态。
- install/uninstall/reinstall/recovery/retarget 共用 `install.cancelled` 作为 transport 发出的取消 terminal。runner
  在观察到 `TaskManager` 的取消事实后只停止后续安全阶段，不再发送第二个 cancelled；commit 取消屏障
  生效后以后端完成或失败终态为准。
- save restore listener 必须同时匹配 `kind = save_restore`、精确 `taskId` 和已登记的
  `save_restore.*` phase。`preparing`、pre-restore、等待锁和 `revalidating` 阶段可取消；进入
  `committing` barrier 后 `cancel_task` 返回稳定不可取消错误，UI 必须等待 completed/failed/
  recovery_required 终态，不能用本地取消状态覆盖后端事实。transport 或 command response 的 cancelled
  可以先用于即时反馈，但 runner 若因取消终态持久化失败发送 `recovery_required`，后者必须覆盖 cancelled。
- 长任务最终结果应通过 `resultRef` 或查询 command 获取，避免把巨大结果塞进进度事件。
- 写入同一游戏实例的 commit 阶段必须串行。

## 安全约束

前端请求中禁止出现：

- 真实导入缓存路径。
- 任意本地文件读写路径。
- 由前端拼接出的安装目标路径。
- 由前端改写后的 `nativePC` 路径或 MHW slot 路径。
- 真实存档内容、第三方 Mod 内容或未脱敏诊断日志。

允许前端发送：

- 后端生成的稳定 id，例如 `gameId`、`profileId`、`modId`、`packageId`、`taskId`、`targetId`。
- 用户通过系统文件选择器选择的目录或文件路径，但 command 必须再次校验。
- 纯展示/筛选输入，例如 query、filter、sort、view mode。

所有真实文件写入必须由后端基于受控 id 解析，并经过安全校验、计划、备份、manifest 和回滚流程。

预览图缩略图的 `thumbnailUrl` 一律走 custom protocol（见上文「Mod 预览图」），前端不直接持有缓存路径，也不通过 asset protocol 或 `convertFileSrc` 自行解析。

## 首批迁移对象

### 1. `game_setup`

现有 command 可以保留，但应逐步补齐：

- 统一 `CommandErrorDto`。
- 前端 API 改用 `invokeCommand` helper。
- 错误 code 与 TypeScript union 对齐。
- 对真实目录只返回必要 `pathLabel`，完整路径只在明确需要时返回。
- `auto_detect_game_directory(gameId)` 只接收稳定 `gameId`，由后端复用 Steam discovery、adapter 校验与 `save_game_directory` 持久化有效候选；返回稳定 `outcome`、状态摘要、错误码和候选数量，不返回自动保存过程中使用的真实目录。

`get_game_prerequisite_status(gameId)` 是只读前置依赖诊断入口。前端只提交稳定 `gameId`；后端先读取已保存的游戏目录配置，再在当前已配置游戏目录内检查受控规则，不接受测试目录、任意本地路径、archive 路径或前端拼接的文件名。当前第一版只覆盖 `Stracker's Loader` 和 `CRCBypass`，`loader-config.json` 只校验 `enablePluginLoader = true`，不做自动安装或自动修复。单项 install/reinstall lifecycle 不再自行解释这份展示 DTO，而是与诊断入口复用同一个 app-level prerequisite decision provider。

返回 DTO 形状：

```ts
type GamePrerequisiteReportState =
  | "not_configured"
  | "game_directory_invalid"
  | "rules_unavailable"
  | "ready";

type GamePrerequisiteSummaryStatus = "verified" | "warning" | "error";

type GamePrerequisiteItemStatus =
  | "missing"
  | "misconfigured"
  | "installed_verified"
  | "installed_unverified";

type GamePrerequisiteIssueCode =
  | "missing_required_file"
  | "signature_unverified"
  | "config_read_failed"
  | "config_invalid_json"
  | "config_field_mismatch"
  | "rules_unavailable"
  | "rules_corrupted";

type GamePrerequisiteIssueDto = {
  code: GamePrerequisiteIssueCode;
  path: string;
};

type GamePrerequisiteItemDto = {
  id: string;
  displayName: string;
  status: GamePrerequisiteItemStatus;
  issues: GamePrerequisiteIssueDto[];
};

type GamePrerequisiteReportDto = {
  gameId: string;
  state: GamePrerequisiteReportState;
  summaryStatus: GamePrerequisiteSummaryStatus | null;
  items: GamePrerequisiteItemDto[];
  errorCode: GameSetupErrorCode | null;
  message: string | null;
};

type GamePrerequisiteDecisionStatus = "ready" | "warning" | "blocked";

type GamePrerequisiteDecisionCode =
  | "game_not_configured"
  | "game_directory_invalid"
  | "rules_unavailable"
  | "rules_corrupted"
  | "storage_unavailable"
  | "storage_corrupted"
  | "unsupported_game"
  | "missing_required_file"
  | "signature_unverified"
  | "config_read_failed"
  | "config_invalid_json"
  | "config_field_mismatch"
  | "prerequisite_decision_invalid";

type GamePrerequisiteDecisionDto = {
  status: GamePrerequisiteDecisionStatus;
  rulesVersion: number | null;
  codes: GamePrerequisiteDecisionCode[];
};
```

边界：

- `not_configured` 表示当前游戏尚未保存有效目录；前端只做空状态提示。
- `game_directory_invalid` 表示已保存目录重新校验失败；前端可展示稳定 `errorCode` 和用户可读 `message`，但不能把它解释为前置缺失。
- `rules_unavailable` 表示本地前置规则文件不可读或已损坏；诊断页显示只读告警，install/reinstall
  lifecycle decision 必须 fail closed，不得降级为“已验证通过”。
- `ready` 表示规则已加载并完成检查；`summaryStatus` 只用于展示聚合诊断，逐项判断应基于 `items[].status` 和 `issues[].code`。
- `installed_unverified` 表示检测到文件存在但签名未命中当前已知规则集；lifecycle decision 必须显式
  返回 `warning`，可以继续但不能在 UI 中显示为“预检通过”。
- `issues[].path` 只能返回脱敏后的相对路径片段，例如 `dinput8.dll`、`loader-config.json`、`nativePC/plugins/QuestLoader.dll`；DTO、错误消息和日志都不能暴露绝对盘符、用户名或真实游戏目录。
- `GamePrerequisiteDecisionDto` 只用于 lifecycle preview/confirm。它不包含 `items`、issue path、
  display name、message 或配置正文；前端只按稳定 status/code 展示，不重算 adapter 规则。

### 2. `replacement / retarget`

首批 command：

```text
list_replacement_targets({ gameId, modId, query? })
analyze_imported_mod_replacement({ gameId, profileId?, modId })
preview_initial_retarget_install({ gameId, profileId, modId, targetId, layerName, layerPriority })
start_retarget_install_task({ gameId, profileId, modId, targetId, layerName, layerPriority })
preview_retarget_reinstall({ gameId, profileId, modId, targetId, layerName, layerPriority })
start_retarget_reinstall_task({ gameId, profileId, modId, targetId, layerName, layerPriority, planToken })
```

边界：

- 前端只提交稳定的 game/Mod/profile/target/layer identity，不提交 package/revision/source/binding/path。
- `list_replacement_targets` 的目标 DTO 自 I18N-08 起以 `displayNames`（locale -> 展示名的完整映射）
  携带全语言名称，不再按固定 locale 投影单一 `displayName`/`secondaryName`。映射键集即该游戏的
  名称 locale 能力声明；展示投影与 fallback 链（当前 locale -> fallback 链 -> en -> 任一可用）由前端
  `resolveReplacementTargetNames` 执行，语言切换不重拉目标列表。检索为跨语言语义：任一语言的
  展示名与全部 alias 都参与匹配，不随界面语言变化。
- 分析响应只可附带可选稳定 `installedTargetId`；它是展示和同目标阻断事实，不是路径或 binding DTO。
- 首次安装由 repository 解析当前 display revision；已安装 target switch 从 manifest 解析 installed revision，
  不接受 cache、sandbox 或 staging path，也不隐式升级。
- MHW adapter 负责 slot 解析、catalog 归一化和路径级 plan。
- 返回 preview 时前端只展示稳定类型、internal id、动作数、冲突和 prerequisite；不显示或自行生成
  source/target relative path 与 path-family。
- initial preview/start 只允许 recovery status 严格为 `not_installed`；retarget reinstall preview/start 只允许
  `installed`，并复用真正重装的 plan token、锁、backup、manifest、rollback/recovery 与 task phases。
- initial preview 顶层返回与普通 install/reinstall 同源的 `prerequisiteDecision`，nested
  `installPlan` 仍是纯计划 DTO，不伪造前置事实。`blocked` 禁止确认，`warning` 可显式继续；
  start runner 在 materialize staging 前读取 decision，并在获取 game/profile 写锁前完成最终 provider
  重验和 status、stable codes、rules version 比较。写锁内不读取 prerequisite 规则或做文件 hash；
  blocked 或漂移必须在 commit/manifest/game write 前拒绝。

### 3. Profile 管理

首批 command：

```text
list_profiles()
get_active_profile()
create_profile({ name, description? })
update_profile({ profileId, name?, description? })
delete_profile(profileId)
set_active_profile(profileId)
```

边界：

- Profile 是 SQLite 中的用户可编辑关系数据；安装 manifest / recovery record 仍保留当前 JSON per profile 存储，不在本切片迁移。
- 首次迁移会创建 `default` profile，并将其标记为 active，保证既有硬编码 `"default"` 流程有兼容承载点。
- `create_profile` 只接收展示名和可选描述，不接收路径、game root、manifest root、backup root 或任意文件系统参数。
- `update_profile` 只允许改展示名和描述；清空描述通过 `description: null` 表达，缺省表示不修改。
- `delete_profile` 必须阻止删除 `default` profile 和当前 active profile，避免让现有安装/恢复链路失去安全默认 profile。
- `set_active_profile` 只切换后端记录的 active profile，不自动改写现有安装 manifest，也不启动安装、卸载、恢复或批量切换任务。
- 当前安装、卸载和恢复 command 仍显式接收 `profileId`；前端后续可以先读取 `get_active_profile`，再把 active id 传给这些既有 command。

Profile DTO 形状：

```ts
type ProfileDto = {
  id: string;
  name: string;
  description: string | null;
  isActive: boolean;
  createdAt: number;
  updatedAt: number;
};
```

Profile 存档设置命令：

```text
get_profile_save_settings({ gameId, profileId })
validate_profile_save_directory({ gameId, profileId, directory })
validate_profile_backup_directory({ gameId, profileId, directory })
set_profile_save_settings(input)
discover_profile_save_directories({ gameId, profileId })
confirm_profile_save_directory_candidate({ discoveryId, candidateId })
```

边界：

- 这些命令用于配置指定 Profile 的存档备份设置；它们不执行备份、恢复、保留清理、安装、卸载、manifest 写入或回滚。
- 前端可以传递用户通过系统目录选择器选中的目录，但每个命令都必须在后端重新验证。
- `get_profile_save_settings` 使用 `gameId` 解析游戏相关默认值，并使用 `profileId` 读取对应 Profile 的配置。
- 响应 DTO 只暴露 `pathLabel`、状态、计划值和稳定校验 code；不暴露 `manifestPath`、`backupRoot`、`backupRef`、sandbox/cache 路径、原始存档内容或第三方 Mod 内容。
- `validate_profile_save_directory` 按游戏/应用规则校验存档源目录，并返回可安全展示的标签。
- `validate_profile_backup_directory` 校验备份目标目录；当后端能判断目录关系时，必须拒绝位于当前游戏安装目录内的位置。
- `set_profile_save_settings` 只在 app-service 校验通过后存储配置；后续为该设置域接入 audit 支持后，自动备份设置变更应写入 Audit Log 事件。
- `retention.maxCount` 范围为 0..=999，其中 `0` 表示不按数量限制。`retention.maxAgeDays` 与
  `retention.maxTotalBytes` 的 `null` 表示不限制；为支持数字输入 UI，Tauri DTO 边界也接受数值 `0`
  并归一化为 `null`/领域层 `None`。非零年龄范围仍为 1..=3650 天，非零空间范围仍为 16 MiB..=1 TiB。
  新配置档默认三项均不限制；已有配置档的持久化值不迁移、不覆盖。
- `preRestoreBackupEnabled` 是 Profile 级持久安全设置，缺省请求与 migration 012 都使用 `true`。前端可以
  修改该开关，但单次恢复请求不能临时关闭它；后端提交时必须重新读取当前持久值。
- `discover_profile_save_directories` 由后端基于已保存游戏配置、Steam root、MHW:I 存档规则和 Profile 设置执行存档源目录发现；前端只提交 `gameId` 和 `profileId`，不提交 Steam userdata 路径、account id、SteamID64、profile URL 或 XML。
- `confirm_profile_save_directory_candidate` 只接收后端生成的 `discoveryId` 和 `candidateId`；后端从短期候选缓存恢复真实目录并重新验证后，才写入对应 Profile 的存档设置。确认成功会消费该 pending discovery，同一组 opaque id 不能重复确认。
- 存档目录发现命令的错误使用稳定 `save_directory_discovery_*` code，`message` 固定为泛化文案，不包含完整本地路径、Steam ID、account id、profile URL、XML 原文或存档文件内容。

DTO 形状：

```ts
type BackupCadence = "manual" | "daily" | "weekly";

type ProfileDirectoryStatusDto =
  | "unset"
  | "valid"
  | "invalid"
  | "defaulted";

type ProfileDirectorySelectionDto = {
  mode: "unset" | "custom" | "default";
  status: ProfileDirectoryStatusDto;
  pathLabel: string | null;
  messages: string[];
};

type ProfileBackupScheduleDto = {
  cadence: BackupCadence;
  hour: number | null;
  minute: number | null;
  weekdays: number[];
};

type ProfileBackupRetentionDto = {
  maxCount: number;
  maxAgeDays: number | null;
  maxTotalBytes: number | null;
};

type SteamAccountDisplaySummaryDto = {
  accountName: string | null;
  avatarUrl: string | null;
  accountLabel: string;
};

type ProfileSaveSettingsDto = {
  profileId: string;
  saveDirectory: ProfileDirectorySelectionDto;
  backupDirectory: ProfileDirectorySelectionDto;
  schedule: ProfileBackupScheduleDto;
  retention: ProfileBackupRetentionDto;
  steamAccount: SteamAccountDisplaySummaryDto | null;
  preRestoreBackupEnabled: boolean;
  updatedAt: number;
};

type SaveDirectoryDiscoveryOutcome =
  | "auto_saved"
  | "confirmation_required"
  | "not_found"
  | "existing_valid"
  | "existing_invalid"
  | "scan_failed";

type SaveDirectoryCandidateDto = {
  candidateId: string;
  source: "steam_userdata";
  confidence: "high" | "medium" | "low";
  recommended: boolean;
  accountName: string | null;
  avatarUrl: string | null;
  accountLabel: string;
  pathLabel: string;
  lastModifiedAt: number | null;
  evidence: string[];
};

type SaveDirectoryDiscoveryDto = {
  discoveryId: string;
  gameId: string;
  profileId: string;
  outcome: SaveDirectoryDiscoveryOutcome;
  recommendedCandidateId: string | null;
  candidates: SaveDirectoryCandidateDto[];
  savedSettings?: ProfileDirectorySelectionDto | null;
  errorCode?: string | null;
};
```

`SaveDirectoryDiscoveryDto` 只承载 opaque id、账号展示摘要、后端校验过的头像 URL、`pathLabel`、`accountLabel`、`lastModifiedAt`、`evidence`、`outcome` 和可选的已保存目录选择摘要。它不得包含完整本地路径、account id、SteamID64、Steam profile URL、XML 原文、真实存档文件名列表或存档内容。

Profile 存档备份命令：

```text
start_save_backup_task({ request: { gameId, profileId, note? } })
list_save_backups({ request: { gameId, profileId, limit? } })
query_save_backup_center({ request: { gameId, profileId?, trigger?, status?, search?, offset?, limit? } })
update_save_backup_note({ request: { gameId, profileId, backupId, note? } })
run_save_backup_retention({ request: { gameId, profileId } })
check_auto_save_backup({ request: { gameId, profileId } })
get_save_backup_background_status({ request: { gameId, profileId } })
```

边界：

- `start_save_backup_task` 是手动存档备份的长任务入口，返回 `TaskStartedDto`；前端按 `taskId` 监听 `save_backup.*` phase。
- `list_save_backups` 只查询后端持久化的备份历史摘要，用于 Profile 页面或后续备份中心刷新历史。
- `query_save_backup_center` 是跨 Profile 的后端权威分页与聚合入口；前端不得先列 Profile 再 N+1 拼装历史、
  空间或状态事实。`limit` 最大 100，`offset` 必须位于后端支持的 signed integer 范围，搜索只匹配 Profile
  名称和备注且最长 100 字符。
- `update_save_backup_note` 只接收短 identity 和最长 200 字符的可选备注；不得传 manifest、文件名或路径。
- `run_save_backup_retention` 与同一 game/profile 的备份任务共用 scope，返回结构化报告。删除进入持久化
  intent 后不可取消，必须收敛为 completed/partial 或保留 pending 供重试。前端调用前必须显示二次确认，
  不能把“立即整理”做成单击即永久删除。
- 备份中心稳定错误至少包括 `save_backup_center_query_invalid`、`save_backup_center_unavailable`、
  `save_backup_center_profile_missing`、`save_backup_center_backup_missing`、`save_backup_note_invalid`、
  `save_backup_task_conflict` 和 `save_backup_retention_failed`；前端不得用 message 文本分支。
- `check_auto_save_backup` 是客户端运行期/启动时的自动备份检查入口；它根据后端持久化的 Profile 存档设置和备份历史判断当前计划是否到期。若到期，后端会以 `trigger = "auto"` 复用存档备份任务链路并返回 `startedTask`。
- 计划到期时后端先做游戏运行检测：游戏运行中或无法判断时保守延后，不获取调度租约、不启动任务，并在 `pendingReason` 返回 `game_running` / `game_running_unknown`；游戏退出后的下一次检查自动补跑。运行检测由后端 `GameRunningDetector` port 决定，前端不参与判断。
- 前端只能传递 `gameId`、`profileId`、可选 `note` 和可选 `limit`；不得传入存档源路径、备份根目录、文件名、manifest 正文、文件列表、hash、sandbox/cache 路径或 backup ref。
- Tauri command 只做 DTO 映射和 app service 转发；目录解析、默认备份目录、自选根目录子目录、压缩、manifest、SQLite 历史、保留策略和审计均由后端服务处理。
- 同一 `gameId + profileId` 同时只允许一个存档备份任务处于 queued/running 范围；重复启动会返回稳定错误码 `task_scope_busy`，避免自动检查和手动按钮并发写同一份存档。
- 当前 `check_auto_save_backup` 返回 `clientRuntimeOnly: true`，它仍只表示本次 due check 发生在主客户端/Tauri 运行期间，不代表已经启用退出后保护。
- `get_save_backup_background_status` 是只读查询：后端读取 scheduler state、inspect Windows registry 并按固定 clock/45 分钟 TTL 派生健康状态；它不注册、不修复、不启动 worker、不获取租约。没有持久化状态或未启用后台保护时返回 `status: "not_enabled"`。
- `protected` 必须同时满足 `backgroundProtectionEnabled = true`、Scheduled Task read-back 完全匹配，以及 `worker_heartbeat_at` 位于 `[now - 45m, now]`；未来时间、过期心跳、配置漂移和检查不确定性都 fail closed 为非保护状态。
- `SaveBackupBackgroundStatusDto` 只包含白名单字段；不得包含 `lease_owner`、`lease_expires_at`、`worker_instance_id`、task name、SID、worker path、PowerShell、task XML、原始命令输出、完整路径、Steam ID、manifest 正文或 hash 列表。
- P7.2b 增加三个无参数的全局控制命令。Settings 是唯一调用方；Profile 继续只读调用 per-profile status，不得启用、停用或修复全局注册。

全局后台保护控制命令：

```text
get_save_backup_background_control_status()
enable_save_backup_background_protection()
disable_save_backup_background_protection()
```

- 三个命令都不接受 `gameId`、`profileId`、路径、task name、SID、worker 参数或平台命令。
- 全局 SQLite 设置保存用户意图、当前启用时间和 worker heartbeat；这些字段只由 app service/repository/worker 修改，前端不能提交。
- 启用先持久化 intent，再执行受控 register/read-back；成功返回 `starting`，不能直接返回 `protected`。停用只有在 owned task 已确认移除后才清除 intent/heartbeat；部分失败保持可重试事实。
- 同一 AppState 内的 register/unregister/enable/disable 会由 app service 串行执行完整转换，包括设置写入、平台注册/read-back、审计和返回状态构造，避免并发命令留下 intent 与平台注册不一致。
- 停用会阻止后续 Scheduled Task invocation，但不会取消已经启动的 `--once` worker cycle；已启动 cycle 仍沿用 scheduler lease 与存档备份安全链完成。停用完成后新启动的 worker 必须读取 disabled intent 并立即 no-op。
- `starting` 使用 5 分钟启动宽限；`protected` 要求当前启用周期的 heartbeat 位于 `[now - 45m, now]`。未来、过期、早于 `enabledAt` 的 heartbeat 都 fail closed。

```ts
type SaveBackupBackgroundControlStatusDto = {
  desiredEnabled: boolean;
  status:
    | "not_enabled"
    | "starting"
    | "protected"
    | "registration_failed"
    | "worker_unhealthy"
    | "permission_required"
    | "unsupported_platform";
  enabledAt: number | null;
  lastHeartbeatAt: number | null;
  lastErrorCode: string | null;
};
```

DTO 形状：

```ts
type TaskStartedDto = {
  taskId: string;
  kind: "save_backup";
  status: "queued";
};

type SaveBackupSummaryDto = {
  backupId: string;
  gameId: string;
  profileId: string;
  trigger: "manual" | "auto" | "pre_install" | "pre_restore";
  status:
    | "completed"
    | "retention_pending"
    | "retention_partial"
    | "deleted_by_retention"
    | "missing"
    | "invalid";
  fileName: string;
  createdAt: number;
  sizeBytes: number;
  fileCount: number;
  sourcePathLabel: string | null;
  notes: string | null;
};

type SaveBackupCenterProfileSummaryDto = {
  profileId: string;
  profileName: string;
  isActive: boolean;
  steamAccount: SteamAccountDisplaySummaryDto | null;
  retention: ProfileBackupRetentionDto;
  backupCount: number;
  archiveBytes: number;
  protectedCount: number;
  attentionCount: number;
  budgetSatisfied: boolean;
};

type SaveBackupCenterItemDto = {
  profileName: string;
  backup: SaveBackupSummaryDto;
};

type SaveBackupCenterSummaryDto = {
  backupCount: number;
  archiveBytes: number;
  protectedCount: number;
  attentionCount: number;
};

type SaveBackupCenterPageDto = {
  items: SaveBackupCenterItemDto[];
  profiles: SaveBackupCenterProfileSummaryDto[];
  offset: number;
  limit: number;
  totalCount: number;
  summary: SaveBackupCenterSummaryDto;
};

type SaveBackupRetentionReportDto = {
  outcome: "within_policy" | "completed" | "partial" | "blocked" | "failed";
  evidenceDegraded: boolean;
  scannedCount: number;
  protectedCount: number;
  problemCount: number;
  candidateCount: number;
  deletedCount: number;
  partialCount: number;
  blockedCount: number;
  archiveBytesBefore: number;
  archiveBytesAfter: number;
  releasedBytes: number;
  maxTotalBytes: number | null;
  budgetSatisfied: boolean;
};

`evidenceDegraded` 只表示 retention Audit 在文件清理完成后未能确认。显式维护仍返回原业务
`outcome`；自动备份的 `save_backup.completed` event 以稳定 `save_backup_evidence_degraded` code
投影同一情况，不得改写为业务失败。

type ProfileAutoSaveBackupCheckDto = {
  gameId: string;
  profileId: string;
  clientRuntimeOnly: true;
  status: "manual_only" | "not_due" | "due";
  checkedAt: number;
  lastDueAt: number | null;
  nextDueAt: number | null;
  lastAutoBackupAt: number | null;
  pendingReason:
    | "game_running"
    | "game_running_unknown"
    | "source_invalid"
    | "destination_unavailable"
    | "task_conflict"
    | null;
  startedTask: TaskStartedDto | null;
};

type SaveBackupBackgroundStatusDto = {
  gameId: string;
  profileId: string;
  status:
    | "protected"
    | "tray_only"
    | "not_enabled"
    | "starting"
    | "registration_failed"
    | "worker_unhealthy"
    | "permission_required"
    | "unsupported_platform";
  backgroundProtectionEnabled: boolean;
  lastCheckedAt: number | null;
  lastAttemptAt: number | null;
  lastSuccessAt: number | null;
  nextDueAt: number | null;
  pendingReason:
    | "game_running"
    | "game_running_unknown"
    | "source_invalid"
    | "destination_unavailable"
    | "task_conflict"
    | null;
  lastErrorCode: string | null;
};
```

`SaveBackupSummaryDto`、备份中心 DTO、`ProfileAutoSaveBackupCheckDto` 和 `SaveBackupBackgroundStatusDto` 不返回
完整本地路径、备份根目录、存档源目录、Steam ID、manifest 正文、文件 hash 列表、调度租约字段、worker
实例 id 或真实存档内容。账号昵称、白名单头像 URL 和掩码 label 只能来自后端确认后持久化的展示 snapshot；
它们不参与 restore、retention ownership 或目录校验。备份中心渲染持久化头像前仍须再次校验 HTTPS 和受信
Steam hostname，损坏或本机篡改的 snapshot 必须回退为文字头像。

Profile 玩家存档恢复命令：

```text
preview_save_restore({ request: { gameId, profileId, backupId } })
start_save_restore_task({ request: {
  gameId,
  profileId,
  backupId,
  previewToken,
  confirmed,
  confirmedWithoutPreRestore
} })
```

`backupId` 是后端备份 writer 生成的 opaque identity。当前 canonical 值为
`<gameId>:<profileId>:<UTC timestamp>:<trigger>`，同秒文件名冲突时可带第五段 `sequence`；每段只允许
ASCII 字母、数字、`-`、`_`、`.`。为兼容旧数据，后端可接受受控的单段 legacy ID。前端不得拆解、拼接、
改写或把 `backupId` 当作文件名/路径；应从列表 DTO 原样回传。Tauri 边界必须拒绝空段、任意冒号形状、
盘符、斜杠、UNC 和其他 path-shaped 值。

请求只接收短的 game/profile/backup identity、后端签发的 opaque preview token 和确认位；不接收 archive、
manifest、目标目录、文件列表、hash、备份根目录或任意本地路径。`preview_save_restore` 必须零玩家写入，
在签发 token 前完成 backup summary identity、manifest/schema、archive SHA-256、逐文件 path/size/hash、
大小上限、containment、目标目录和游戏运行状态校验。Preview DTO 形状为：

```ts
type SaveRestorePreviewDto = {
  backup: SaveBackupSummaryDto;
  fileCount: number;
  totalUncompressedBytes: number;
  preRestoreBackupEnabled: boolean;
  requiresAdditionalConfirmation: boolean;
  warningCodes: string[];
  previewToken: string;
  expiresAt: number;
};

type SaveRestoreTaskStartedDto = {
  taskId: string;
  kind: "save_restore";
  status: "queued";
};
```

`previewToken` 默认 5 分钟有效，并绑定 game/profile/backup、Profile 设置、source facts 和目标摘要。
任务启动后在锁外重新校验并物化 staging；默认开启时先创建 `trigger = "pre_restore"` 的独立安全备份，
备份失败不得进入 commit。获取共享 game/profile 写锁后只做短事实复核和目录交换；成功提交后刷新备份
历史。关闭安全备份时 preview 必须返回 warning，start 同时要求 `confirmed = true` 与
`confirmedWithoutPreRestore = true`。

取消屏障：`preparing`、`pre_restore_backup`、等待锁和 `revalidating` 可协作取消；进入 `committing` 后
禁止重分类为 cancelled。终态为 `completed`、`failed`、`recovery_required` 或 `cancelled`，且必须按
同一 `taskId` 保留在恢复 Modal 中。`completed` 事件若 Task/Audit evidence 写入失败，只携带稳定
`save_restore_evidence_degraded`，不能变成业务失败。协作取消必须先持久化
`Failed + save_restore_cancelled` 再清理 staging；持久化失败时保留现场并发送
`recovery_required + save_restore_transaction_unavailable`。`failed` 事件以 `error` 为主错误码；`message`
仅可携带受控二级 warning code，例如 rolled-back 后的 `save_restore_recovery_cleanup_failed`，前端必须同时
展示主结果与 warning，不能把 warning 当作原始文本。

恢复错误使用稳定 code；至少包括 `save_restore_profile_missing`、`save_restore_backup_missing`、
`save_restore_backup_unavailable`、`save_restore_target_unset`、`save_restore_target_invalid`、
`save_restore_game_running`、`save_restore_game_running_unknown`、`save_restore_backup_directory_unavailable`、
`save_restore_archive_unavailable`、`save_restore_manifest_unavailable`、`save_restore_manifest_invalid`、
`save_restore_archive_invalid`、`save_restore_hash_mismatch`、`save_restore_path_unsafe`、
`save_restore_size_limit_exceeded`、`save_restore_staging_unavailable`、`save_restore_clock_unavailable`、
`save_restore_token_issue_failed`、`save_restore_token_invalid`、
`save_restore_token_expired`、`save_restore_token_stale`、`save_restore_confirmation_required`、
`save_restore_high_risk_confirmation_required`、`save_restore_pre_restore_backup_invalid`、
`save_restore_facts_changed`、`save_restore_lock_unavailable`、`save_restore_prepared_missing`、
`save_restore_target_unavailable`、`save_restore_target_unsafe`、`save_restore_target_changed`、
`save_restore_commit_failed`、`save_restore_rolled_back`、`save_restore_recovery_required`、
`save_restore_transaction_unavailable`、
`save_restore_recovery_evidence_unsafe`、`save_restore_recovery_cleanup_failed`、
`save_restore_evidence_degraded`。前端按 code 映射文案，不能按 message 分支，也不得把底层路径或错误原文
显示给用户。

### 4. 游戏启动

首批 command：

```text
launch_game(gameId)
```

边界：

- `launch_game` 是短命令，不创建 long-running task，不发送 `hmm://task-progress` 事件。
- 前端只提交稳定 `gameId`，不提交 exe 路径、游戏目录、Steam URI、shell 命令或任意本地路径。
- 后端通过已保存的 `GameInstance` 判断是否已配置；没有配置或配置状态不可用时返回 `game_not_configured`。
- 游戏专属启动方式留在对应 game adapter；MHW:I 当前优先使用 Steam protocol。
- 平台打开 URI 的细节留在 infra runner；自动化测试必须使用 fake runner，不能启动真实 Steam 或真实游戏。
- 返回 DTO 只包含 `gameId` 和 `method`，不包含完整本地路径、Steam URI、Steam ID、启动器进程信息或游戏目录。

DTO 形状：

```ts
type GameLaunchMethod = "steam_protocol" | "direct_executable";

type GameLaunchReceiptDto = {
  gameId: string;
  method: GameLaunchMethod;
};
```

稳定错误码：

```text
unsupported_game
game_not_configured
storage_corrupted
storage_failed
launcher_unavailable
launch_failed
```

### 5. 安装计划预览

首批 command：

```text
preview_install_plan(input)
preview_imported_mod_install_plan(input)
start_install_task(input)
start_uninstall_task(input)
get_install_manifest_status(input)
scan_install_recovery(input)
preview_recovery_action(input)
start_recovery_action_task(input)
start_import_mod_revision_task(input)
get_mod_revisions(modId)
preview_reinstall_plan(input)
start_reinstall_task(input)
cancel_task(taskId)
```

边界：

- `preview_install_plan` 不写真实游戏目录。
- `start_install_task` 必须基于已经生成或可重建的 plan。
- `start_uninstall_task` 必须基于已有 manifest、`installed_file` 摘要和 backup 记录，不根据当前 Mod 包内容猜测。
- `start_recovery_action_task` 必须基于 durable recovery record、`installed_file` 摘要和 backup 记录，不根据当前 Mod 包内容猜测。
- `start_import_mod_revision_task` 只把 picker 提供的 archive 附加到显式指定的既有 logical Mod，不按名称、作者、版本或文件名自动合并。
- `preview_reinstall_plan` 只读构建 candidate plan 和四类 target 聚合；`start_reinstall_task` 只接收受控 id、layer 和 opaque preview token。
- 真实 commit 过程必须写 manifest，并能回滚或恢复。
- 当前 `preview_install_plan` 只暴露只读计划预览壳，用于验证 Tauri DTO 与 `hmm-app` 计划服务边界；它返回相对目标路径摘要、来源 id、层级信息和阻断冲突，不创建目录、不复制文件、不删除文件、不写 manifest。
- `preview_install_plan` 的 `allowedTargetRoots` 和 `files[].targetPath` 必须来自后端分析/adapter 结果或测试夹具；正式前端 UI 不得根据游戏名、Mod 内容或用户输入自行拼接最终安装路径。后续 package analyzer / game adapter 接入后，应优先让前端只提交后端生成的 `modId`、`packageId`、`profileId` 或 `targetId`。
- `preview_imported_mod_install_plan` 是正式前端优先使用的后端驱动预览入口。前端只提交 `gameId`、`modId` 和 layer 摘要；后端通过已持久化导入记录定位受控 sandbox，只读枚举包内普通文件，并使用对应 game adapter 声明的允许安装根生成 `InstallPlan` 输入。
- `preview_imported_mod_install_plan` 不接受 `targetPath`、`allowedTargetRoots`、sandbox/cache 路径、导入包路径或游戏目录路径；DTO 和错误 message 不应包含完整本地路径或第三方 Mod 内容。
- `preview_imported_mod_install_plan` 返回 flattened `InstallPlanPreviewDto` 加
  `prerequisiteDecision`。required missing、规则不可用/损坏、目录/存储不可用或 decision 无法证明时
  为 `blocked`；`signature_unverified` 为 `warning`。通用 `preview_install_plan` 没有游戏上下文，
  保持只返回纯计划，不伪造 prerequisite decision。
- `start_install_task` 是后端驱动的安装提交入口。前端只提交 `gameId`、`modId`、`profileId` 和 layer 摘要；后端从已持久化导入记录和受控 sandbox 重建 `InstallPlan`，再在同一 `gameId/profileId` 写锁下执行 `InstallPlan -> backup -> commit -> manifest`。该 command 不接受 `targetPath`、`allowedTargetRoots`、sandbox/cache 路径、导入包路径、游戏目录路径或备份/manifest 路径。
- `start_install_task` 在锁外构建 plan 和 prerequisite decision，并在获取写锁前立即重读同一个
  provider。blocked 或 status/codes/rulesVersion 漂移时必须在 commit、manifest 和游戏目录写入前
  fail closed；锁内只重验已封存 plan/token、write admission 和当前写入状态。
- `start_install_task` 返回 `TaskStartedDto { taskId, kind: "install", status: "queued" }`，并发送 `hmm://task-progress` 的 `install.queued` 事件；后台 runner 会发送 `install.plan.building`、`install.commit.processing`、`install.completed` 或 `install.failed`。失败事件的 `error` 使用稳定前缀 `install_failed:<phase>`，当前 phase 可为 `planning`、`lock`、`commit`、`complete`、`recovery_pending` 或 `recovery_unavailable`；后两者表示在 commit 前分别因存在待收敛重装恢复事务或恢复仓储不可用而 fail-closed。事件 payload 不承载目标路径、完整本地路径、manifest 内容或第三方 Mod 内容。
- `start_install_task` 会写最小 Audit Log 事件，字段只包含 `task_id`、`game_id`、`mod_id`、`profile_id` 和 `action_count` 等短 id/计数；失败事件可额外包含与 task event 一致的稳定 `error_code`。事件不记录完整本地路径、用户名、Steam ID、sandbox/cache 路径或第三方 Mod 内容。
- `start_uninstall_task` 是后端驱动的最小安全卸载入口。前端只提交 `gameId`、`modId` 和 `profileId`；后端在同一 `gameId/profileId` 写锁下读取受控 manifest，且只处理该 Mod 的 manifest entries。该 command 不接受 `targetPath`、game root、backup root/ref、manifest root/path、sandbox/cache 路径、导入包路径或游戏目录路径。
- `start_uninstall_task` 只会对存在 `installed_file` 摘要且当前目标文件 size/SHA-256 与 manifest 匹配的 entries 执行破坏性动作：无 `backup_ref` 的本工具新增文件会删除；有 `backup_ref` 的覆盖文件会从受控 backup 恢复。缺少摘要、目标摘要不匹配、目标缺失、backup 缺失或 backup 读取失败都会阻断自动卸载。
- `start_uninstall_task` 返回 `TaskStartedDto { taskId, kind: "install", status: "queued" }`，并发送 `hmm://task-progress` 的 `install.uninstall.queued` 事件；后台 runner 会发送 `install.uninstall.processing`、`install.uninstall.completed` 或 `install.uninstall.failed`。失败事件的 `error` 使用稳定前缀 `install_uninstall_failed:<phase>`，当前 phase 可为 `lock`、`uninstall`、`complete`、`recovery_pending` 或 `recovery_unavailable`；后两者表示在卸载前分别因存在待收敛重装恢复事务或恢复仓储不可用而 fail-closed。事件 payload 不承载目标路径、完整本地路径、manifest 内容、backup ref 或第三方 Mod 内容。
- 正式前端卸载 UI 只能在 `get_install_manifest_status` 摘要显示 `installed` 时提供单选卸载入口；typed API 只能调用 `start_uninstall_task` 并传入 `gameId`、`modId`、`profileId`。若摘要返回 `committed_cleanup_pending`、`cleanup_pending`、`rollback_required`、`repair_required` 或 `unknown`，必须阻断安装/重装和自动卸载入口。前端按 `taskId` 和 `install.uninstall.*` phase 展示任务状态，完成后重新查询 manifest 摘要；失败时不根据 Mod 包内容、展示标签或页面内存态推断修复动作。
- 若前端额外调用 `scan_install_recovery` 摘要，只能用于展示 issue code、计数和恢复中心所需的聚合详情；不能用它推断未文档化修复动作，也不能根据 Mod 包内容、展示标签或页面内存态推断修复动作。
- `start_uninstall_task` 会写最小 Audit Log 事件，字段只包含 `task_id`、`game_id`、`mod_id`、`profile_id`、`removed_file_count` 和 `restored_file_count` 等短 id/计数；失败事件可额外包含与 task event 一致的稳定 `error_code`。事件不记录完整本地路径、用户名、Steam ID、sandbox/cache 路径、backup 路径、manifest 正文或第三方 Mod 内容。
- `start_import_mod_revision_task` 接收 `archivePath` 和既有 `modId`，复用普通导入的安全解压、取消和持久化链路，并返回 `TaskStartedDto { taskId, kind: "mod_import", status: "queued" }`。archive path 只允许出现在这个 picker 驱动的导入入口；`get_mod_revisions`、`preview_reinstall_plan` 和 `start_reinstall_task` 均不接受 archive/source/sandbox/game-root/target/backup/manifest path。
- `get_mod_revisions` 返回一张 logical Mod 的 `originRevisionId`、`displayRevisionId` 和全部受其所有的 revision ids。origin/display revision 的权威来源是 revision catalog；installed revision 的权威来源始终是当前 profile 的 completed manifest entry set，不能从 display revision、导入顺序、任务内存或“最新版本”推断。
- `preview_reinstall_plan` 是只读入口。后端按请求的 `candidateRevisionId` 校验 owner/readiness，并使用该 revision 自己的 `packageId` 定位受控 sandbox，而不是使用当前 display revision projection；随后从 manifest、当前 target 摘要、original backup 和 durable reinstall transaction 构建 strict preview union。该命令不写 game/manifest/recovery，不返回 path、backup/snapshot ref、hash、manifest/source content 或第三方 Mod 内容。
- `ReinstallPlanPreviewDto.status` 是判别字段：两个分支都必须返回 `prerequisiteDecision`。`ready`
  必须同时返回非空 installed/candidate revision、非空 `planToken`、四类计数和空 reasons；
  `blocked` 必须返回 `planToken: null`，revision 可以为 null。`candidate_not_found` 必须返回
  `candidateRevision: null`，前端不得伪造 summary 或提交 token。
- `start_reinstall_task` 返回 `TaskStartedDto { taskId, kind: "install", status: "queued" }`，先发送 `install.reinstall.queued`，后台 runner 再以同一 taskId 发送 `install.reinstall.*`。prepare/source/plan 在 write lock 外执行；commit 与 install/uninstall/controlled recovery 共享同一 `gameId/profileId` 写锁、reinstall recovery admission 和 Audit writer。
- `start_reinstall_task` 的 token 对前端是不透明值。完整 prerequisite provider 重验在获取写锁前完成；
  后端在写锁内重新校验 game instance 未在 prepare 后改变、token、candidate ownership/source、旧
  manifest、target/backup facts，并确认该 profile 没有任一 active reinstall transaction；不匹配时
  fail closed。失败 event 使用 `install_reinstall_failed:<phase>`，phase 为 `planning`、`preflight`、
  `lock`、`backup`、`commit`、`manifest`、`post_commit`、`rollback` 或 `complete`。`post_commit`
  表示 candidate manifest 已越过 commit point，不能伪装为 rolled back；后续由受控 reconciliation
  收敛。
- 重装 preview blocking reason 的稳定值为 `prerequisites_blocked`、`not_installed`、
  `candidate_not_found`、`candidate_not_ready`、`candidate_owner_mismatch`、
  `candidate_already_installed`、`manifest_state_unsafe`、`installed_revision_unknown`、
  `source_unavailable`、`target_missing`、`target_changed`、`target_read_failed`、`backup_missing`、
  `backup_read_failed`、`plan_conflict`、`cross_mod_target_conflict` 和预留的 `preview_stale`。
  command error 负责输入/用例不可用，reason 只负责可展示预检结论。
- Task 7 落地上述 Rust/Tauri contract 与 AppState composition；Task 8 已接入 feature-local TypeScript wrapper、显式 logical Mod revision 导入、revision 选择、strict preview/confirm 和按 taskId 监听的真正重装 UI。
- 安装提交写入 manifest entry 时会记录后端内部使用的 `installed_file` 摘要（写入内容 size + SHA-256）。该摘要不进入当前前端 DTO，不暴露目标路径、backup ref、manifest path、sandbox/cache path 或文件内容；后续卸载/恢复扫描可用它判断目标文件是否仍与受控安装事实一致。
- `get_install_manifest_status` 是只读安装状态摘要入口。前端提交 `profileId`、`modIds`，并可以提交 `gameId`。传入 `gameId` 时，后端复用只读 recovery scan，在同一 `gameId/profileId` 写锁下读取受控 manifest、目标文件摘要和 backup 是否存在，并按 `modId` 返回 `status`、`managedFileCount` 和 `backupCount`；未传 `gameId` 时，后端保留旧的 manifest-only 查询，只从受控 manifest 仓储读取对应 profile 的 manifest。该 command 不接受 `targetPath`、manifest root/path、backup root/ref、sandbox/cache 路径、导入包路径或游戏目录路径。
- `get_install_manifest_status` 的返回状态为 `not_installed`、`installed`、`committed_cleanup_pending`、`cleanup_pending`、`rollback_required`、`repair_required` 或 `unknown`。传入 `gameId` 时，后端把 recovery scan 的 `completed` 映射为 `installed`，并保留两个重装 cleanup-pending 状态及其他不安全状态；未传 `gameId` 时，旧 manifest-only 路径仍只根据匹配到的 manifest entries 派生 `installed`，缺失 manifest 或无匹配 entry 返回 `not_installed`。
- `get_install_manifest_status` 不返回 target path、game root、backup ref/root、manifest root/path、sandbox/cache 路径、manifest 正文、目标文件 hash 或第三方 Mod 内容。缺失 manifest 不是错误，不应让前端回退为 mock 安装事实或从任务内存态推断已安装状态。manifest-only 路径读取失败使用稳定错误码 `install_manifest_unavailable`；传入 `gameId` 后读取游戏配置或恢复扫描失败时沿用 `scan_install_recovery` 的稳定错误码 `game_instance_unavailable` / `install_recovery_unavailable`。
- `scan_install_recovery` 是只读恢复扫描摘要入口。前端只提交 `gameId`、`profileId` 和 `modIds`；`modIds` 可为空，表示扫描该 profile manifest 内全部已知托管 Mod，便于启动级恢复检查或独立恢复中心先获得全局健康摘要。后端通过受控游戏配置解析 game root，并复用同一 `gameId/profileId` 的安装/卸载写锁后读取受控 manifest、目标文件摘要和 backup 是否存在。该 command 不接受或返回 target path、game root、backup ref/root、manifest root/path、sandbox/cache 路径、导入包路径、游戏目录路径或 manifest 正文。
- `scan_install_recovery` 返回每个 mod 的 `status`、托管文件计数、backup 计数、聚合 issue 计数和稳定 issue code。`completed` 表示 manifest entries、当前目标摘要和需要的 backup 均一致；`committed_cleanup_pending` 表示 candidate manifest/targets 已证明，但 completed bookkeeping 尚待受控收敛；`cleanup_pending` 表示 transaction 已 completed 但 snapshot/record cleanup 尚未结束；`rollback_required` 表示重装前/普通安装写入窗口未确认完成；`repair_required` 表示无法安全自动收敛的一致性问题；`unknown` 表示读取失败等无法判断状态。缺失 manifest、无匹配 entry 且没有 recovery record 时返回 `not_installed`。当前命令只读，不自动删除、恢复、回滚或写 manifest。
- `preview_recovery_action` 是只读恢复动作预览入口。前端只提交 `gameId`、`profileId`、`modId` 和 `actionKind`；action kind contract 为 `rollback_install` 或 `reconcile_reinstall`。现有详细 availability/count preview 针对 `rollback_install`；`reconcile_reinstall` 在没有专用 preview 证明时返回 blocked，不能据此绕过后端 task revalidation。该 command 不接受或返回 target path、game root、backup ref/root、manifest root/path、sandbox/cache 路径、目标文件 hash 明文、导入包路径、游戏目录路径、manifest 正文或第三方 Mod 内容，也不执行写入或写 Audit Log。
- `preview_recovery_action` 只有在 durable recovery record 为 `committing` 或 `rollback_required`、每个目标仍匹配 `installed_file` 摘要，且覆盖文件所需 backup 均存在并可读时才返回 `available`。无 recovery record、状态不在可回滚窗口、缺少 `installed_file`、目标缺失、目标摘要变化、目标读取失败、backup 缺失或 backup 读取失败都会返回 `blocked`，并使用 `rollback_state_missing`、`missing_installed_file_summary`、`target_missing`、`target_changed`、`target_read_failed`、`backup_missing` 或 `backup_read_failed` 等稳定 reason code。
- `start_recovery_action_task` 是后端驱动的受控恢复动作任务入口。`rollback_install` 根据普通 install recovery record 回到安装前状态；`reconcile_reinstall` 根据 durable reinstall transaction 重新验证 candidate/pre-reinstall manifest 与 target/snapshot facts，再完成 post-commit cleanup 或受控回到 pre-reinstall。无法证明时进入 `repair_required` 并 fail closed。该 command 不接受 `targetPath`、game root、backup/snapshot ref/root、manifest root/path、sandbox/cache 路径、导入包路径或游戏目录路径。
- `start_recovery_action_task` 返回 `TaskStartedDto { taskId, kind: "install", status: "queued" }`，并发送 `hmm://task-progress` 的 `install.recovery.queued` 事件；后台 runner 会发送 `install.recovery.planning`、`install.recovery.processing`、`install.recovery.completed` 或 `install.recovery.failed`。失败事件的 `error` 使用稳定前缀 `install_recovery_failed:<phase>`，当前 phase 可为 `lock`、`planning`、`processing` 或 `complete`。事件 payload 不承载目标路径、完整本地路径、backup ref、manifest 内容、目标 hash、sandbox/cache 路径或第三方 Mod 内容。
- `start_recovery_action_task` 会写最小 Audit Log 事件，`operation` 为 `rollback_install` 或 `reconcile_reinstall`，字段只包含 `task_id`、`game_id`、`mod_id`、`profile_id`、`remove_file_count`、`restore_file_count` 和 `backup_count` 等短 id/计数，不记录完整本地路径、用户名、Steam ID、backup/snapshot ref/root、manifest 正文、sandbox/cache 路径或第三方 Mod 内容。
- Mod 库前端应在 `get_install_manifest_status` 中传入 `gameId`，让状态摘要直接反映只读 recovery scan。前端应把 `committed_cleanup_pending` / `cleanup_pending` / `rollback_required` / `repair_required` / `unknown` 都作为不安全状态展示并阻断新的安装、卸载或重装；扫描失败时应降级为 `unknown`，不回退为 mock 安装事实或任务内存态。
- Dashboard / App Frame / 独立恢复中心可以在游戏目录配置完成后调用 `scan_install_recovery`，传入空 `modIds` 获取当前 profile 的全量托管安装健康摘要。Dashboard 等入口级摘要只能展示扫描 Mod 数、需处理数、未知数、托管文件数、backup 计数、issue 总数和 `issues[].issue/count` 等聚合信息；App Frame 全局告警只能在需要处理、状态未知或扫描不可用时展示轻量摘要和恢复中心导航；独立恢复中心可以额外展示每个托管 Mod 的短 id、状态、托管文件计数、backup 计数、issue 计数、稳定 issue 分类，以及由前端 view model 基于稳定 issue code 派生的 rich repair summary、风险等级、阻断原因和人工处理建议。扫描失败必须展示状态未知，不能解释为健康或自动触发恢复。
- 独立恢复中心可以提供用户主动触发的 `export_support_diagnostics` 入口。该入口必须通过 feature-local typed API 调用无参数 command；前端导出前先展示将包含的已脱敏类别确认，导出后只展示 `exportId`、`fileName`、`sizeBytes`、`appLogLineCount`、`taskLogLineCount` 和 `auditEventCount`，不能传入或展示输出路径、日志路径、诊断包完整路径、日志正文、审计事件正文、manifest/backup/root、sandbox/cache 路径或第三方 Mod 内容。诊断导出成功不改变安装、卸载、恢复扫描或 manifest 状态。
- 独立恢复中心的人工处理决策面板只能把 `retry_scan` 映射为重新触发只读 `scan_install_recovery`，把 `export_diagnostics` 映射为上述诊断导出确认流程，把 `controlled_recovery` 映射为滚动到逐 Mod 状态列表；真正的写入型按钮只在单个 `rollback_required` Mod 行上出现。用户点击逐 Mod 回滚按钮时，前端必须先调用 `preview_recovery_action`；只有后端返回 `available` 才展示确认动作，确认后才调用 `start_recovery_action_task`。前端必须按返回的 `taskId` 匹配 `install.recovery.*` 事件，完成后重新触发 `scan_install_recovery` 刷新恢复中心、Dashboard 摘要和 App Frame 全局告警；不得根据 Mod 包内容、展示标签或页面内存态推断修复动作，也不得调用 `start_install_task`、`start_uninstall_task` 或任何未文档化的恢复、删除、回滚、manifest 写入 command。
- `scan_install_recovery` 的前端展示只允许使用 `managedFileCount`、`backupCount`、`issueCount` 和 `issues[].issue/count` 等聚合摘要。UI 不得展示或提交 target path、game root、backup ref/root、manifest root/path、sandbox/cache 路径、manifest 正文、目标文件 hash 或第三方 Mod 内容。
- `scan_install_recovery` 读取失败使用稳定错误码：未配置或无法读取 game instance 返回 `game_instance_unavailable`；manifest 仓储不可用返回 `install_recovery_unavailable`。错误 message 不应包含完整本地路径、backup ref、manifest 路径或第三方 Mod 内容。
- `preview_recovery_action` 读取失败使用稳定错误码：未配置或无法读取 game instance 返回 `game_instance_unavailable`；动作预览不可用返回 `install_recovery_action_preview_unavailable`。错误 message 不应包含完整本地路径、backup ref、manifest 路径、sandbox/cache 路径或第三方 Mod 内容。
- `preview_install_plan` 的错误使用稳定 code，例如 `install_target_path_empty`、`install_target_path_absolute`、`install_target_path_parent_traversal`、`install_target_path_windows_drive_prefix`、`install_target_path_invalid_segment` 和 `install_target_root_not_allowed`；错误 message 不应包含完整本地路径或第三方 Mod 内容。
- `preview_imported_mod_install_plan` 的错误使用稳定 code，例如 `game_id_invalid`、`install_planning_sources_unavailable`、`install_planning_game_adapter_not_found`、`install_planning_imported_mod_not_found`、`install_planning_imported_mod_analysis_unavailable`、`install_planning_imported_mod_sandbox_unavailable`、`install_planning_imported_mod_file_scan_unavailable`，以及复用的 `install_target_*` / `install_target_root_not_allowed` 路径校验错误；错误 message 不应包含完整本地路径、sandbox/cache 路径或第三方 Mod 内容。
- revision/reinstall commands 的输入错误使用 `mod_id_empty`、`profile_id_empty`、`candidate_revision_id_empty`、`layer_name_empty`、`game_id_invalid` 或 `plan_token_invalid`；query/用例不可用使用 `mod_revisions_not_found`、`mod_revisions_unavailable`、`reinstall_catalog_unavailable`、`reinstall_manifest_unavailable`、`reinstall_recovery_unavailable`、`reinstall_candidate_plan_unavailable`、`reinstall_preview_unavailable` 或 `reinstall_start_unavailable`。这些错误的 message 不能包含 archive/source/sandbox/game-root/target/backup/manifest path、ref、hash 或第三方 Mod 内容。

当前安装与恢复 DTO 形状：

```ts
type PreviewImportedModInstallPlanRequestDto = {
  gameId: string;
  modId: string;
  layerName: string;
  layerPriority: number;
};

type PreviewInstallPlanRequestDto = {
  allowedTargetRoots: string[];
  files: Array<{
    modId: string;
    packageFileId: string;
    targetPath: string;
    layerName: string;
    layerPriority: number;
  }>;
};

type StartInstallTaskRequestDto = {
  gameId: string;
  modId: string;
  profileId: string;
  layerName: string;
  layerPriority: number;
};

type StartUninstallTaskRequestDto = {
  gameId: string;
  modId: string;
  profileId: string;
};

type StartImportModRevisionTaskRequestDto = {
  archivePath: string;
  modId: string;
};

type ReinstallFileLayerDto = {
  name: string;
  priority: number;
};

type PreviewReinstallPlanRequestDto = {
  gameId: string;
  profileId: string;
  modId: string;
  candidateRevisionId: string;
  layer: ReinstallFileLayerDto;
};

type StartReinstallTaskRequestDto = PreviewReinstallPlanRequestDto & {
  planToken: string;
};

type ModRevisionSummaryDto = {
  revisionId: string;
};

type ModRevisionListDto = {
  modId: string;
  originRevisionId: string;
  displayRevisionId: string;
  revisions: ModRevisionSummaryDto[];
};

type ReinstallTargetCountsDto = {
  retained: number;
  replaced: number;
  added: number;
  stale: number;
};

type ReinstallBlockingReasonDto =
  | "prerequisites_blocked"
  | "not_installed"
  | "candidate_not_found"
  | "candidate_not_ready"
  | "candidate_owner_mismatch"
  | "candidate_already_installed"
  | "manifest_state_unsafe"
  | "installed_revision_unknown"
  | "source_unavailable"
  | "target_missing"
  | "target_changed"
  | "target_read_failed"
  | "backup_missing"
  | "backup_read_failed"
  | "plan_conflict"
  | "cross_mod_target_conflict"
  | "preview_stale";

type ReinstallPlanPreviewDto =
  | {
      status: "ready";
      prerequisiteDecision: GamePrerequisiteDecisionDto;
      planToken: string;
      installedRevision: ModRevisionSummaryDto;
      candidateRevision: ModRevisionSummaryDto;
      counts: ReinstallTargetCountsDto;
      blockingReasons: [];
    }
  | {
      status: "blocked";
      prerequisiteDecision: GamePrerequisiteDecisionDto;
      planToken: null;
      installedRevision: ModRevisionSummaryDto | null;
      candidateRevision: ModRevisionSummaryDto | null;
      counts: ReinstallTargetCountsDto;
      blockingReasons: Array<{
        code: ReinstallBlockingReasonDto;
        count: number;
      }>;
    };

type InstallManifestStatusRequestDto = {
  gameId?: string;
  profileId: string;
  modIds: string[];
};

type InstallRecoveryScanRequestDto = {
  gameId: string;
  profileId: string;
  modIds: string[];
};

type InstallRecoveryActionKindDto = "rollback_install" | "reconcile_reinstall";

type InstallRecoveryActionPreviewRequestDto = {
  gameId: string;
  profileId: string;
  modId: string;
  actionKind: InstallRecoveryActionKindDto;
};

type StartRecoveryActionTaskRequestDto = {
  gameId: string;
  profileId: string;
  modId: string;
  actionKind: InstallRecoveryActionKindDto;
};

type InstallManifestStatusDto =
  | "not_installed"
  | "installed"
  | "committed_cleanup_pending"
  | "cleanup_pending"
  | "rollback_required"
  | "repair_required"
  | "unknown";

type InstallManifestStatusSummaryDto = {
  profileId: string;
  modId: string;
  status: InstallManifestStatusDto;
  managedFileCount: number;
  backupCount: number;
};

type InstallRecoveryStatusDto =
  | "not_installed"
  | "completed"
  | "committed_cleanup_pending"
  | "cleanup_pending"
  | "rollback_required"
  | "repair_required"
  | "unknown";

type InstallRecoveryIssueDto =
  | "missing_installed_file_summary"
  | "target_missing"
  | "target_changed"
  | "target_read_failed"
  | "backup_missing"
  | "backup_read_failed";

type InstallRecoverySummaryDto = {
  profileId: string;
  modId: string;
  status: InstallRecoveryStatusDto;
  managedFileCount: number;
  backupCount: number;
  issueCount: number;
  issues: Array<{
    issue: InstallRecoveryIssueDto;
    count: number;
  }>;
};

type InstallRecoveryActionAvailabilityDto = "available" | "blocked";

type InstallRecoveryActionBlockReasonDto =
  | "rollback_state_missing"
  | "missing_installed_file_summary"
  | "target_missing"
  | "target_changed"
  | "target_read_failed"
  | "backup_missing"
  | "backup_read_failed";

type InstallRecoveryActionPreviewDto = {
  profileId: string;
  modId: string;
  actionKind: InstallRecoveryActionKindDto;
  availability: InstallRecoveryActionAvailabilityDto;
  removeFileCount: number;
  restoreFileCount: number;
  backupCount: number;
  blockingIssueCount: number;
  blockingReasons: Array<{
    reason: InstallRecoveryActionBlockReasonDto;
    count: number;
  }>;
};

type InstallPlanPreviewDto = {
  hasBlockingConflicts: boolean;
  actions: Array<{
    targetPath: string;
    modId: string;
    packageFileId: string;
    layerName: string;
    layerPriority: number;
  }>;
  conflicts: Array<{
    targetPath: string;
    providers: Array<{
      modId: string;
      packageFileId: string;
      layerName: string;
      layerPriority: number;
    }>;
  }>;
};

type ImportedModInstallPreflightDto = InstallPlanPreviewDto & {
  prerequisiteDecision: GamePrerequisiteDecisionDto;
};
```

### 6. Mod 预览图

Mod 预览图属于导入分析结果，不属于前端文件读取能力。具体安全策略见 [Mod 预览图安全处理设计](MOD_PREVIEW_IMAGE_PIPELINE_DESIGN.md)。

首批 command / 结果形态：

```text
start_import_mod_task(input)
get_mod_library()
query_mod_library({ request })
get_mod_detail(modId)
get_mod_dependency_graph()
get_mod_detail_preview_image(modId)
get_preview_image_candidates(modId)
select_preview_image_candidate(modId, candidateIndex)
get_preview_image_diagnostics()
export_preview_image_diagnostics()
export_audit_log_diagnostics()
export_support_diagnostics()
maintain_thumbnail_cache()
get_thumbnail_cache_settings()
set_thumbnail_cache_settings({ thumbnailCacheMaxBytes, thumbnailCacheMaxAgeDays })
get_log_storage_settings()
set_log_storage_settings({ maxBytes })
get_debug_log_settings()
set_debug_log_settings({ enabled })
```

边界：

- 当前 `start_import_mod_task` 会做 archive 路径基础校验，通过后端 `TaskManager` 登记 `mod_import` queued 任务，返回 `TaskStartedDto { taskId, kind, status }`，并发送 `hmm://task-progress` 的 `mod_import.queued` 事件；随后后台 prepare runner 会执行受控 zip 沙盒解包和预览图处理，并发送 `unpack.*`、`preview_image.*` 与 `prepare.completed` 事件。受控 zip 解包会拒绝父级穿越、绝对路径、symlink entry、大小写不敏感路径碰撞、entry 数超限、单文件解压后大小超限和总解压大小超限。prepare 成功后，导入分析结果会保存到 app data 下的受控结果仓储；进度事件仍不承载巨大结果。若任务在 running prepare 期间被取消，zip 解压会在 entry/chunk 检查点协作式停止并清理本次 sandbox；预览图扫描/处理会在 scanner 遍历和 processor 读文件、解码前后、缩略图写入前后的检查点停止。runner 会停止保存结果，不再发送 `prepare.completed`，也不会用 failed 覆盖 cancelled 状态，并会 best-effort 触发一次缩略图缓存维护。
- `get_mod_library()` 返回已持久化的导入分析结果列表，条目包含 `id`、`name`、`author`、`versionLabel`、`status`、`sizeLabel`、`categoryLabels` 和 `previewImage`。当前 MVP 使用 `packageId` 作为稳定 `id`；`name` 优先来自后端包元数据分析，缺失时回退 `packageId`；`author` 和 `versionLabel` 只来自后端解析的短文本 metadata；`categoryLabels` 来自后端解析的通用 category 和 tags，不由前端从路径推断。后端会从受控 sandbox 的 manifest/readme 候选解析通用元数据，多个 manifest 候选只用于补齐缺失字段，不作为安装事实来源。
- `query_mod_library({ request })` 是与旧无参列表并存的短只读分页查询。`request` 使用 camelCase，包含可选 `profileContext { gameId, profileId }`、`search`、`filter`、固定 `sort: "name_asc"`、1-based `page` 和 `pageSize: 12 | 24 | 48 | 96`。`filter` 只能是 `{ kind: "all" }`、`{ kind: "status", status }` 或 `{ kind: "category", categoryId }`；status 稳定值为 `not_installed`、`installed`、`committed_cleanup_pending`、`cleanup_pending`、`rollback_required`、`repair_required`、`unknown`。status filter 必须携带 profile context；无 active profile 时仍可使用 all/category 查询，但后端不会猜测安装状态。响应 `ModLibraryPageDto { items, page, pageSize, libraryTotal, matchingTotal }` 只返回 clamp 后的当前页，`items.length <= pageSize`；每个 profile-aware item 的 `status` 与后端 filter 事实一致，并可带 `installSummary { status, managedFileCount, backupCount }`。无 profile context 的条目保持旧导入状态且省略 `installSummary`。稳定错误码为 `game_id_invalid`、`profile_id_empty`、`mod_library_filter_invalid`、`mod_library_sort_invalid`、`mod_library_page_invalid`、`mod_library_page_size_unsupported`、`mod_library_search_too_long`、`mod_library_category_not_found`、`mod_library_profile_context_required`、`mod_library_unavailable` 和 `mod_library_status_unavailable`；前端不得按 message 分支。该命令不创建 task、不发送 event、不获取写锁，也不接受或返回 archive、sandbox/cache、manifest、game path、backup ref、第三方 Mod 内容或其他文件系统字段。旧 `get_mod_library()` 和现有页面消费者在迁移 Slice 3 前保持不变。
- `get_mod_detail(modId)` 通过后端受控 `modId` 查询单个导入结果，返回 `null` 或包含 `previewImage` 与 `metadata { version, author, category, tags, dependencies }` 摘要的详情 DTO；这些 metadata 字段只用于展示和后续诊断输入，不表示依赖已安装、冲突已验证或安装计划事实。前端不传递 sandbox/cache/archive-internal 路径。
- `get_mod_dependency_graph()` 基于已持久化导入结果返回只读声明图。节点只包含后端 `modId` 和展示名；边只包含声明来源 `sourceModId`、包内短文本 `dependency`，以及当该声明与另一个已导入 `modId` 规范化后精确匹配时才返回的 `matchedImportedModId`。该命令不接受路径、不创建长任务、不发送 progress event、不判断依赖是否已安装、不返回 install/profile/manifest 状态，也不把展示名或 metadata 摘要升级成安装事实。
- `get_mod_detail_preview_image(modId)` 通过后端受控 `modId` 查询并生成详情页更大派生预览图，返回 `PreviewImageDto` 或 `null`。该命令固定使用后端 `preview-1024` 策略，不接受前端传入尺寸、variant、路径或图片字节；后端会基于已持久化导入记录定位受控 sandbox，重扫受限候选并处理首个可用候选。该命令对导入记录和原始 Mod 包只读，不写回导入记录，处理过程中只会写入可丢弃的 thumbnail cache，不创建长任务，不发送 progress event；DTO 不包含显式 variant、logical path、sandbox/cache/archive-internal 路径、本地路径或图片字节。
- `get_preview_image_candidates(modId)` 基于已持久化导入记录返回受限候选列表，返回 `null` 表示该 `modId` 未登记。该命令只接受后端 `modId`，不接受 sandbox/cache/archive-internal 路径；后端通过受控 sandbox locator 重扫候选，并应用 `max_candidates_per_package` 上限。DTO 只包含 `candidateIndex`、`fileName` 和 `compressedSizeBytes`，不返回 logical path、`thumbnailUrl`、缓存路径、本地路径或图片字节。该命令不写导入结果，不创建长任务，不发送 progress event。
- `select_preview_image_candidate(modId, candidateIndex)` 基于已持久化导入记录写回用户选择的预览图，返回更新后的 `PreviewImageDto`，返回 `null` 表示该 `modId` 未登记。该命令只接受后端 `modId` 和非负 `candidateIndex`，不接受 logical path、sandbox/cache/archive-internal 路径、压缩包内部路径、本地图片路径或图片字节；后端会重新定位受控 sandbox、重扫受限候选并按候选序号处理单个候选。该命令不创建长任务，不发送 progress event；处理失败或 URL 解析失败会返回并持久化 `fallback`。
- `get_preview_image_diagnostics()` 基于已持久化导入结果返回预览图处理摘要：`totalImportedMods`、`thumbnailCount`、`fallbackCount`、按 `reason` 聚合的 `fallbackReasons`，以及用于导出前确认的 `exportCategories`。当前 `exportCategories` 声明预览图聚合摘要可纳入诊断包，并明确排除缩略图文件、`thumbnailUrl` 资源引用和原始第三方 Mod 包内容。该命令不创建长任务、不发送 progress event、不读取或导出第三方图片内容、不返回 `thumbnailUrl`、缓存路径、sandbox 路径或本地路径。
- `export_preview_image_diagnostics()` 基于同一份已脱敏摘要写入受控诊断 zip。该命令不接受输出路径参数；后端固定写入 app data 下的 `logs/diagnostics/`，返回 `exportId`、`fileName`、`sizeBytes` 和本次导出的 `diagnostics` 摘要。当前 zip 只包含 `preview-image-diagnostics.json`，不包含缩略图文件、`thumbnailUrl`、`contentHash`、sandbox/cache/local 路径、README 全文、原始第三方 Mod 包内容或原始日志。导出成功后后端会写入最小 Audit Log 事件；若诊断 zip 写入失败，会先写入只含稳定错误分类和聚合计数的失败审计事件；若审计写入失败，命令不返回成功。该命令不创建长任务、不发送 progress event；更通用的日志/audit 诊断包导出仍需后续治理能力补齐。
- `export_audit_log_diagnostics()` 导出已脱敏审计日志诊断包。该命令不接受输出路径、日志路径或事件数量参数；后端固定读取 app data 下已校验的最近审计事件，单次最多 200 条，并固定写入 app data 下的 `logs/diagnostics/`。返回 DTO 只包含 `exportId`、`fileName`、`sizeBytes` 和 `auditEventCount`，不返回审计事件正文、审计日志路径、本地路径、原始错误文本、第三方 Mod 内容、缩略图 URL 或缓存/sandbox 路径。该命令不创建长任务、不发送 progress event；当前只覆盖 Audit Log 子集。
- `export_support_diagnostics()` 导出完整支持诊断包。该命令不接受输出路径、日志路径、类别选择、行数或事件数量参数；后端固定从 app data 下读取已校验 App Log / Debug Log / Task Log 文本行、已校验 Audit Log 事件和平台摘要，并固定写入 app data 下的 `logs/diagnostics/`。返回 DTO 只包含 `exportId`、`fileName`、`sizeBytes`、四类计数，以及稳定、无路径的 `debugLogStatus`、`taskLogStatus`、`auditLogStatus`、`logStorageStatus`、`debugLogEventRejectedCount`、`debugLogWriteFailureCount`、`debugLogRetentionFailureCount`、`taskLogWriteFailureCount`、`taskLogRetentionFailureCount`、`auditWriteFailureCount`、`auditWriteFailureAfterCommitCount`、`auditLogRetentionFailureCount`、`logStorageFailureCount`、`logStorageUnsatisfiedCount`、`logStorageSettingsFailureCount`。Debug、Task、Audit 和日志空间状态只使用本节登记的稳定 code；命令不返回日志正文、审计事件正文、诊断包路径、本地路径、原始错误文本、第三方 Mod 内容、缩略图 URL、`contentHash` 或缓存/sandbox 路径。该命令不创建长任务、不发送 progress event；用户可见入口仍应在前端展示类别确认，而不是展示敏感原文。
- `maintain_thumbnail_cache()` 手动触发后端缩略图缓存维护，复用当前导入结果引用保留、settings 空间上限 / LRU 清理和可选按时间保留逻辑。该命令不创建长任务、不发送 progress event、不返回清理报告或真实缓存路径；清理失败按 best-effort 处理，不改变导入、安装、卸载或回滚事实。
- `get_thumbnail_cache_settings()` 读取当前受控后端设置并返回 `AppSettingsDto`。该命令不接受参数、不写入 settings 文件、不触发缓存维护，也不返回 settings 文件路径、缓存路径、sandbox 路径或任意文件系统路径。
- `set_thumbnail_cache_settings({ thumbnailCacheMaxBytes, thumbnailCacheMaxAgeDays })` 写入受控后端设置并返回当前设置 DTO。`thumbnailCacheMaxBytes` 可为正整数或 `null`，`null` 表示回退默认空间上限；`0` 会返回稳定错误码 `thumbnail_cache_max_bytes_invalid`。`thumbnailCacheMaxAgeDays` 可为正整数天数或 `null`，`null` 表示不启用按时间保留延迟、沿用当前未引用缩略图维护语义；`0` 会返回稳定错误码 `thumbnail_cache_max_age_days_invalid`。该命令不接收或返回 settings 文件路径、缓存路径、sandbox 路径或任意文件系统路径。
- `get_log_storage_settings()` 读取当前日志总空间预算并返回窄 `LogStorageSettingsDto { maxBytes }`。`maxBytes` 为正整数或 `null`；`null` 表示使用后端默认 128 MiB。该命令不接受参数、不写 settings、不执行预算维护，也不返回日志目录、文件名、清理候选或任意文件系统路径。
- `set_log_storage_settings({ maxBytes })` 只更新日志总空间预算并返回当前 `LogStorageSettingsDto`。`maxBytes` 为不小于 1 MiB 的整数或 `null`；`null` 表示回退默认 128 MiB，小于 1 MiB 返回稳定错误码 `log_storage_max_bytes_invalid`，settings 读取或保存失败返回 `app_settings_unavailable`。该命令不会立即删除日志，不接受类别、文件名、路径或清理优先级参数；启动维护仍由共享 runtime 按后端固定策略执行。
- 前端只能接收后端生成的 `previewImage` 结构。
- 前端不能提交真实缓存路径、压缩包内部路径或本地图片路径让后端读取。
- 预览图处理失败返回 `fallback` 状态，不应阻断 Mod 导入主流程。
- 失败原因使用稳定 `snake_case` 字符串；已注册的 `reason` 值见下文 DTO 定义（例如 `too_large`、`decode_failed`、`pixel_limit_exceeded`、`cache_write_failed`）。当 prepare 阶段发送 `mod_import.preview_image.fallback` 事件时，事件 payload 的 `error` 字段携带同一组稳定 fallback reason，`message` 不作为前端分支依据。
- 图片处理任务和导入任务事件必须携带 `taskId`。
- 预览图阶段事件使用 `mod_import.preview_image.processing` 和 `mod_import.preview_image.fallback` 两个 phase code（见上文「长任务契约」）。

#### thumbnailUrl 解析方案

`thumbnailUrl` 必须由后端解析为受控资源 URL，前端拿不到真实磁盘路径。本项目采用 **custom protocol** 方案：

- 后端注册自定义协议 scheme（如 `thumbnail://`），由 protocol handler 根据 opaque `thumbnailRef`（`package_id` / `variant` / `content_hash`）从应用缓存目录读字节返回，并设置正确 `Content-Type` 和缓存头。
- 前端 `<img src={thumbnailUrl}>` 直接消费，不做任何路径拼接或 `convertFileSrc` 转换。
- protocol handler 必须校验请求路径落在受控缓存目录内，拒绝穿越、绝对路径、符号链接和未登记 package 的访问。
- 真实缓存路径不进入任何 DTO、日志或前端代码。
- 缓存目录可整体删除重建，删除后同 `thumbnailRef` 的 URL 仍可命中（handler 重新生成或返回 fallback）。

custom protocol 是契约层唯一允许的 `thumbnailUrl` 形态；asset protocol、`convertFileSrc` 和 base64 data URL 不作为正式契约方案，详情页若未来需要内联可另行扩展契约，不得在现有 DTO 上隐式承载。

#### 输出格式与缓存布局

- MVP 默认输出 **JPEG**（质量 80，最长边 768px）；WebP 保留为后续可选优化。
- 缩略图文件名实际扩展名（`.jpg` / `.webp`）由后端根据 `preferred_output_format` 决定，**不进入前端 DTO**。
- 缓存目录布局由 infra 决定，示例 `thumbnails/<package_id>/preview-768-<content_hash>.<ext>`，不进入前端契约。
- 若后续把默认格式切到 WebP，DTO 字段不变，前端无感知。

推荐 DTO 形状：

```ts
type PreviewImageDto =
  | {
      kind: "thumbnail";
      thumbnailUrl: string;
      width: number;
      height: number;
      contentHash: string;
    }
  | {
      kind: "fallback";
      reason:
        | "missing"
        | "too_large"
        | "too_many_candidates"
        | "unsupported_format"
        | "decode_failed"
        | "pixel_limit_exceeded"
        | "cache_write_failed";
    };
```

候选列表 DTO 只承载可展示的候选摘要和后端候选序号：

```ts
type PreviewImageCandidateListDto = {
  modId: string;
  candidates: Array<{
    candidateIndex: number;
    fileName: string;
    compressedSizeBytes: number;
  }>;
};
```

该 DTO 不包含 logical path、`thumbnailUrl`、`contentHash`、sandbox/cache 路径、本地路径或图片字节。选择候选图时，前端只能向 `select_preview_image_candidate` 提交后端生成的 `candidateIndex` 或等价 opaque id，不能提交路径。

依赖声明图 DTO 只承载已导入结果之间的保守声明关系：

```ts
type ModDependencyGraphDto = {
  nodes: Array<{
    modId: string;
    name: string;
  }>;
  edges: Array<{
    sourceModId: string;
    dependency: string;
    matchedImportedModId?: string | null;
  }>;
};
```

该 DTO 不包含路径、安装状态、profile 状态、冲突结果或 install manifest 事实。`matchedImportedModId` 只表示声明文本与某个已导入 `modId` 的规范化精确匹配，不表示依赖已启用或已安装。

诊断摘要 DTO 只承载统计信息：

```ts
type PreviewImageDiagnosticsDto = {
  totalImportedMods: number;
  thumbnailCount: number;
  fallbackCount: number;
  fallbackReasons: Array<{
    reason:
      | "missing"
      | "too_large"
      | "too_many_candidates"
      | "unsupported_format"
      | "decode_failed"
      | "pixel_limit_exceeded"
      | "cache_write_failed";
    count: number;
  }>;
  exportCategories: Array<{
    category:
      | "preview_image_summary"
      | "thumbnail_files"
      | "thumbnail_urls"
      | "raw_package_content";
    status: "included" | "excluded";
    reason?:
      | "derived_image_content"
      | "opaque_resource_reference"
      | "third_party_mod_content";
  }>;
};
```

该 DTO 不包含 `thumbnailUrl`、`contentHash`、缓存路径、本地路径或图片字节，不能作为图片导出或缓存读取入口。`exportCategories` 是结构化确认清单，不代表真实诊断包已经写入磁盘。

诊断包导出 DTO 只返回后端受控写入结果摘要：

```ts
type PreviewImageDiagnosticsExportDto = {
  exportId: string;
  fileName: string;
  sizeBytes: number;
  diagnostics: PreviewImageDiagnosticsDto;
};
```

`fileName` 只是文件名，不是完整本地路径；前端不能传入或拼接导出路径。当前导出包只包含已脱敏的 `preview-image-diagnostics.json`，不包含缩略图文件、`thumbnailUrl`、`contentHash`、缓存路径、sandbox 路径、本地路径、原始 Mod 包内容或原始日志。Audit Log 事件由后端内部写入，不进入 DTO，也不暴露审计日志路径；失败审计事件同样不能包含原始错误文本、完整本地路径或缓存/sandbox 路径。

审计日志诊断包导出 DTO 只返回后端受控写入结果摘要：

```ts
type AuditLogDiagnosticsExportDto = {
  exportId: string;
  fileName: string;
  sizeBytes: number;
  auditEventCount: number;
};
```

`fileName` 只是文件名，不是完整本地路径；前端不能传入或拼接导出路径。当前导出包只包含最多 200 条已校验审计事件的 `audit-log-diagnostics.json`，命令 DTO 本身不返回事件正文，也不暴露审计日志路径、诊断包路径、缓存路径、sandbox 路径、本地路径、原始 Mod 包内容、原始日志或未脱敏错误文本。

完整支持诊断包导出 DTO 只返回后端受控写入结果摘要：

```ts
type SupportDiagnosticsExportDto = {
  exportId: string;
  fileName: string;
  sizeBytes: number;
  appLogLineCount: number;
  debugLogLineCount: number;
  taskLogLineCount: number;
  auditEventCount: number;
  debugLogStatus: "ok" | "debug_log_retention_failed" | "debug_log_event_rejected" | "debug_log_write_failed";
  taskLogStatus: "ok" | "task_log_retention_failed" | "task_log_write_failed";
  auditLogStatus: "ok" | "audit_log_retention_failed" | "audit_write_failed" | "audit_write_failed_after_commit";
  logStorageStatus: "ok" | "log_storage_settings_unavailable" | "log_storage_budget_unsatisfied" | "log_storage_budget_failed";
  debugLogEventRejectedCount: number;
  debugLogWriteFailureCount: number;
  debugLogRetentionFailureCount: number;
  taskLogWriteFailureCount: number;
  taskLogRetentionFailureCount: number;
  auditWriteFailureCount: number;
  auditWriteFailureAfterCommitCount: number;
  auditLogRetentionFailureCount: number;
  logStorageFailureCount: number;
  logStorageUnsatisfiedCount: number;
  logStorageSettingsFailureCount: number;
};
```

`fileName` 只是文件名，不是完整本地路径；前端不能传入或拼接导出路径。当前导出包可包含已脱敏平台摘要、已校验 App/Debug/Task Log 文本行和最多 200 条已校验 Audit Log 事件，但命令 DTO 本身不返回日志正文、事件正文、诊断包路径、本地路径、原始 Mod 包内容、缩略图 URL、`contentHash`、缓存/sandbox 路径、原始日志或未脱敏错误文本。

### 7. 分类与 Mod 展示元数据

分类 command 管理用户定义的分类及 Mod 与分类的关联；Mod 展示元数据 command 管理用户对已导入 Mod 的展示信息覆盖。两者都是短任务，不创建 `TaskManager` 任务，也不发送进度事件。

| command | 输入 | 返回 |
| --- | --- | --- |
| `create_category` | `{ name, color?, sortOrder? }` | 新分类的 `string` id |
| `update_category` | `{ categoryId, name?, color?, sortOrder? }` | `void` |
| `delete_category` | `{ categoryId }` | `void` |
| `list_categories` | 无 | `CategoryWithCountDto[]` |
| `set_mod_categories` | `{ modId, categoryIds }` | `void` |
| `get_mod_categories` | `{ modId }` | `CategoryDto[]` |
| `update_mod_metadata` | `{ modId, displayName?, author?, version?, description?, nexusModId? }` | `void` |
| `delete_mod_metadata` | `{ modId }` | `void` |

分类 DTO 的前端可见形状固定为：

```ts
type CategoryDto = {
  id: string;
  name: string;
  color: string | null;
  sortOrder: number;
};

type CategoryWithCountDto = CategoryDto & {
  modCount: number;
};
```

分类契约边界：

- `create_category` 会去除 `name` 和可选 `color` 两端空白；空名称被拒绝，空颜色按未设置处理，省略 `sortOrder` 时使用后端默认顺序 `0`。
- `update_category` 是字段级更新：省略 `name`、`color` 或 `sortOrder` 表示保留原值；`color: null` 明确清空颜色，字符串表示设置去除两端空白后的颜色。空名称被拒绝。
- `set_mod_categories` 以本次 `categoryIds` 替换指定 Mod 的分类关联；后端会去重 id。前端只提交后端分类查询返回的 id，不把分类名称、颜色或展示顺序解释为安装事实。
- 六个分类 command 的应用层失败统一映射为稳定错误码 `category_error`。前端不得解析 `message` 区分验证、未找到或存储失败。

Mod 展示元数据契约边界：

- `update_mod_metadata` 会先去除 `modId` 两端空白并拒绝空值，然后保存或替换该 logical Mod 的整份用户 overlay。`displayName`、`author`、`version` 和 `description` 会去除两端空白，空字符串归一化为未设置；省略的可选字段也以未设置写入本次 overlay。`nexusModId` 仅为可选展示字段。
- `delete_mod_metadata` 删除指定 Mod 的用户 overlay；后续展示回退到原始导入分析元数据。删除 overlay 不删除 Mod、安装记录、manifest、备份或玩家文件。
- 两个命令对空 `modId` 返回稳定错误码 `mod_id_empty`；时钟、仓储或服务失败统一返回 `mod_metadata_unavailable`。错误响应不暴露底层路径或存储错误。
- 展示元数据和分类都是 library overlay，不是安装、依赖、冲突、路径、manifest、profile 或 game adapter 的事实来源。前端不得据此生成 `InstallPlan`、拼接写入路径或绕过后端安装流程。

## 窗口关闭与托盘生命周期

- `hmm://window-close-requested` 由 Tauri 后端在主窗口收到关闭请求时发出；后端会先阻止默认关闭，前端必须显示关闭选择或按已保存偏好调用窄命令。
- `hide_main_window_to_tray` 只隐藏当前主窗口，不执行备份、不修改 Profile、不读取路径。
- `exit_app` 只退出当前 Tauri 主客户端进程，不声明后台守护已接管。
- `get_app_exit_guard()` 是只读结构化决策；只有 `confirmation_required` 会同时返回后端签发的短时、一次性 `exitAuthorization`，`safe` 与 `blocked` 都不签发授权。
- `exit_app({ request: { overrideUnprotected, exitAuthorization? } })` 要求显式布尔值。普通退出只能传 `false`；命令先隐藏主窗口，再执行一次权威 guard。安全时返回 `outcome: "exiting"` 并退出；后台保护不安全时恢复窗口，返回 `outcome: "confirmation_required"`、稳定原因和一次性授权。
- `TaskKind::SaveRestore` 处于 queued/running 时，或 restore scope 状态不可读时，`get_app_exit_guard` 与 `exit_app` 必须 fail closed 返回 `blocked`。`blocked` 不是可 override 的后台保护警告：前端只能提供“返回应用”或“收起至系统托盘”，不得传递授权、渲染“仍然退出”或尝试绕过该状态。最终 exit admission 在后端原子关闭 restore 新任务登记，避免 guard 查询和实际退出之间的竞争窗口。
- Windows guard 每次读取当前 Task Scheduler definition/status 并结合 fresh heartbeat 判定；不得用长 TTL 或会话缓存替代本次精确读回。只读 inspect 使用 Task Scheduler COM，注册、启动、停用和 installer cleanup 仍沿用受控 PowerShell mutation。
- 隐藏窗口后的所有非退出路径（确认返回、restore `blocked`、授权存储/guard 错误或其他 command 失败）都必须恢复主窗口；只有明确返回 `outcome: "exiting"` 才允许保持隐藏。授权 mutex 错误必须直接 fail closed，不能回退为无授权的安全退出。
- 只有危险退出对话框的当次明确确认可以传 `overrideUnprotected: true`，并必须透传该对话框持有的授权。授权缺失、过期、错配或已消费时，后端回退到完整 guard；若仍不安全则返回新的原因和授权，前端必须重置执行态并刷新当前危险确认，不能直接 override 或停留在“正在退出”。
- 危险退出默认操作和初始焦点为留在托盘，不显示 remember；Escape、overlay 和关闭按钮都只取消。`starting` override 不 unregister、不清除 `desiredEnabled`。

```ts
type SaveBackupExitGuardReason =
  | "background_starting"
  | "background_not_enabled"
  | "registration_failed"
  | "worker_unhealthy"
  | "permission_required"
  | "unsupported_platform"
  | "status_unavailable";

type AppExitBlockReason =
  | "save_restore_in_progress"
  | "save_restore_status_unavailable";

type AppExitGuardDto =
  | { decision: "safe"; reason: null; exitAuthorization: null }
  | {
      decision: "confirmation_required";
      reason: SaveBackupExitGuardReason;
      exitAuthorization: string;
    }
  | { decision: "blocked"; reason: AppExitBlockReason; exitAuthorization: null };

type ExitAppRequestDto = {
  overrideUnprotected: boolean;
  exitAuthorization?: string;
};

type ExitAppResultDto =
  | { outcome: "exiting"; reason: null; exitAuthorization: null }
  | {
      outcome: "confirmation_required";
      reason: SaveBackupExitGuardReason;
      exitAuthorization: string;
    }
  | { outcome: "blocked"; reason: AppExitBlockReason; exitAuthorization: null };
```

- Settings 全局控制与 exit guard 均只消费稳定 snake_case status/reason/code；UI 不展示 raw backend message。
- P7.2b 实现了应用级启停、状态和退出警告，但安装态 sibling worker/真实触发/fresh heartbeat 验收仍未完成，不能把自动化结果描述为 Windows runtime acceptance。
- 前端不得通过宽泛 window/filesystem API 重建生命周期逻辑；只调用本节列出的窄命令。

## 测试要求

Tauri / Rust 桥接改动至少运行：

```powershell
cargo test --workspace
cargo check --workspace
```

前端 typed API 改动至少运行：

```powershell
cmd /c corepack pnpm run typecheck
```

涉及 UI 工作流时补充：

```powershell
cmd /c corepack pnpm run lint
cmd /c corepack pnpm run build
```

涉及长任务和高风险文件操作时，还需要覆盖：

- command 参数校验。
- 错误 DTO 是否可展示。
- 事件是否携带 `taskId`。
- 取消后状态是否一致。
- 不同游戏实例/同 profile 的并发边界。
- 临时目录中的 install/backup/rollback 行为。

## 文档维护

当新增或修改 Tauri command、DTO、长任务事件、错误 code、前端 typed API 或 app service 装配方式时，应同步检查本文档。

如果某个 feature 需要特殊通信形态，应在对应 feature 文档中说明原因，并保持本文的通用安全边界不变。
