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
  薄边界：参数校验、DTO 转换、调用 AppState 中的应用服务

hmm-app
  用例编排：依赖 ports，不依赖具体文件系统或平台实现

hmm-core / hmm-ports / hmm-infra / hmm-games-*
  领域模型、traits、真实 I/O 和游戏适配规则
```

前端可以展示 `pathLabel`、`displayName`、`internalId` 等后端提供的字段，但不能据此拼接写入路径或推断安装行为。

## Command 命名

Tauri command 使用 `snake_case`，以动词或查询动作开头：

- 查询状态：`get_game_setup_status`
- 校验输入：`validate_game_directory`
- 保存配置：`save_game_directory`
- 扫描候选：`scan_game_candidates`
- 启动自检并自动保存有效发现：`auto_detect_game_directory`
- 查询前置依赖状态：`get_game_prerequisite_status`
- 预览计划：`preview_install_plan`、`preview_retarget_plan`
- 启动长任务：`start_import_mod_task`
- 查询导入结果：`get_mod_library`、`get_mod_detail`、`get_mod_dependency_graph`、`get_mod_detail_preview_image`
- Profile 管理：`list_profiles`、`get_active_profile`、`create_profile`、`update_profile`、`delete_profile`、`set_active_profile`
- Profile 存档备份：`start_save_backup_task`、`list_save_backups`、`check_auto_save_backup`、`get_save_backup_background_status`
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
- 读取和写入受控设置：`get_thumbnail_cache_settings`、`set_thumbnail_cache_settings`
- 取消长任务：`cancel_task`
- ARMOR 替换目标：`list_replacement_targets`、`analyze_imported_mod_replacement`、`preview_initial_retarget_install`、`start_retarget_install_task`、`preview_retarget_reinstall`、`start_retarget_reinstall_task`

命名应表达用例，而不是底层文件操作。禁止新增类似 `copy_file`、`delete_path`、`read_any_file` 这类宽泛文件系统 command。

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

`AppState` 只保存应用服务和共享基础设施句柄，不保存前端 view 状态。

新增服务时应遵循：

1. 在 `hmm-app` 中定义用例服务。
2. 依赖 `hmm-ports` traits。
3. 在 `src-tauri/src/state.rs` 中组合具体 infra 和 adapter。
4. 在 command 中通过 `State<'_, AppState>` 调用服务。

如果服务需要内部可变状态，优先让服务内部用清晰的锁或队列表达，而不是在 command 中临时拼装全局状态。

## ARMOR_RETARGET AR4/AR5 契约

AR4 的入口固定在 `Mod 管理 -> Mod 详情统一面板 -> 替换目标 Tab`。右键“MOD 文件修改”只负责用
replacement Tab 打开同一个详情面板，不新增孤立页面。`/replacements` 仍保留给后续全局 binding、
占用和冲突总览。

六个 command 的请求只使用稳定身份：

| command | 请求 | 返回 |
| --- | --- | --- |
| `list_replacement_targets` | `gameId`、可选 `query` | catalog target 列表 |
| `analyze_imported_mod_replacement` | `gameId`、可选 `profileId`、`modId` | source、匹配文件数、warning、`retargetable` 与可选 `installedTargetId` |
| `preview_initial_retarget_install` | `gameId`、`profileId`、`modId`、`targetId`、layer | retarget action、warning 与 InstallPlan 冲突摘要 |
| `start_retarget_install_task` | 与 preview 相同 | `TaskStartedDto` |
| `preview_retarget_reinstall` | `gameId`、`profileId`、`modId`、`targetId`、layer | `ReinstallPlanPreviewDto` 与 plan token |
| `start_retarget_reinstall_task` | 与 preview 相同，另加 `planToken` | `TaskStartedDto` |

前端不得提交 `packageId`、revision package id、source path、sandbox/cache/staging/game root、
`sourceId`、`bindingId`、`internalId` 或最终 target path。前四个 AR4 command 从当前 display revision
重建包事实，重新扫描并分析唯一受支持 source，按 `targetId` 查询 catalog，生成 binding、
`RetargetPlan`、staging 和 `InstallPlan`。两个 AR5 target-switch command 的 revision 来源见下文，
不得复用 display revision。`internalId` 与最终相对路径只能作为后端返回的只读预览信息。

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

`start_retarget_install_task` 继续使用 `TaskKind::Install`、`hmm://task-progress` 和既有
game/profile 写锁。新增 phase 为 `install.retarget.queued`、`install.retarget.plan.building`、
`install.retarget.commit.processing`、`install.retarget.completed`、`install.retarget.failed`；失败事件的
`error` 使用 `install_retarget_failed:<phase>`。commit 必须继续经过 Audit Log、backup、manifest、
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
| `install` | `install.reinstall.cancelled` | 任务在 queued/prepare 安全点取消；commit 开始后不抢占式中断 |
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

新增 task kind 时必须在此表登记对应 phase code，避免前端硬编码未登记值。

规则：

- 每个进度事件必须携带 `taskId`。
- 前端不能靠“当前页面只有一个任务”来匹配事件。
- 取消使用 `cancel_task(taskId)`；当前实现支持取消 `queued` 和 `running` 任务。running prepare 不会强制杀线程；zip 解压、预览图候选扫描和预览图 processor 会在后端 cancellation token 检查点协作式停止。图片库单次解码/编码调用本身仍不是抢占式中断；install commit 已开始后不做抢占式中断，必须依赖 backup / rollback / manifest 链路保持可恢复状态。
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

`get_game_prerequisite_status(gameId)` 是只读前置依赖诊断入口。前端只提交稳定 `gameId`；后端先读取已保存的游戏目录配置，再在当前已配置游戏目录内检查受控规则，不接受测试目录、任意本地路径、archive 路径或前端拼接的文件名。当前第一版只覆盖 `Stracker's Loader` 和 `CRCBypass`，`loader-config.json` 只校验 `enablePluginLoader = true`，不做自动安装、自动修复或 preflight 阻断。

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
```

边界：

- `not_configured` 表示当前游戏尚未保存有效目录；前端只做空状态提示。
- `game_directory_invalid` 表示已保存目录重新校验失败；前端可展示稳定 `errorCode` 和用户可读 `message`，但不能把它解释为前置缺失。
- `rules_unavailable` 表示本地前置规则文件不可读或已损坏；前端只能做只读告警，不得降级为“已验证通过”。
- `ready` 表示规则已加载并完成检查；`summaryStatus` 只用于展示聚合诊断，逐项判断应基于 `items[].status` 和 `issues[].code`。
- `installed_unverified` 表示检测到文件存在但签名未命中当前已知规则集；这是 warning，不是阻断或自动修复信号。
- `issues[].path` 只能返回脱敏后的相对路径片段，例如 `dinput8.dll`、`loader-config.json`、`nativePC/plugins/QuestLoader.dll`；DTO、错误消息和日志都不能暴露绝对盘符、用户名或真实游戏目录。

### 2. `replacement / retarget`

首批 command：

```text
list_replacement_targets({ gameId, query? })
analyze_imported_mod_replacement({ gameId, profileId?, modId })
preview_initial_retarget_install({ gameId, profileId, modId, targetId, layerName, layerPriority })
start_retarget_install_task({ gameId, profileId, modId, targetId, layerName, layerPriority })
preview_retarget_reinstall({ gameId, profileId, modId, targetId, layerName, layerPriority })
start_retarget_reinstall_task({ gameId, profileId, modId, targetId, layerName, layerPriority, planToken })
```

边界：

- 前端只提交稳定的 game/Mod/profile/target/layer identity，不提交 package/revision/source/binding/path。
- 分析响应只可附带可选稳定 `installedTargetId`；它是展示和同目标阻断事实，不是路径或 binding DTO。
- 首次安装由 repository 解析当前 display revision；已安装 target switch 从 manifest 解析 installed revision，
  不接受 cache、sandbox 或 staging path，也不隐式升级。
- MHW adapter 负责 slot 解析、catalog 归一化和路径级 plan。
- 返回 preview 时可展示最终相对路径摘要，但前端不能自行生成路径。
- initial preview/start 只允许 recovery status 严格为 `not_installed`；retarget reinstall preview/start 只允许
  `installed`，并复用真正重装的 plan token、锁、backup、manifest、rollback/recovery 与 task phases。

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
};

type ProfileSaveSettingsDto = {
  profileId: string;
  saveDirectory: ProfileDirectorySelectionDto;
  backupDirectory: ProfileDirectorySelectionDto;
  schedule: ProfileBackupScheduleDto;
  retention: ProfileBackupRetentionDto;
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
check_auto_save_backup({ request: { gameId, profileId } })
get_save_backup_background_status({ request: { gameId, profileId } })
```

边界：

- `start_save_backup_task` 是手动存档备份的长任务入口，返回 `TaskStartedDto`；前端按 `taskId` 监听 `save_backup.*` phase。
- `list_save_backups` 只查询后端持久化的备份历史摘要，用于 Profile 页面或后续备份中心刷新历史。
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
  trigger: "manual" | "auto" | "pre_install";
  status: "completed" | "deleted_by_retention" | "missing" | "invalid";
  fileName: string;
  createdAt: number;
  sizeBytes: number;
  fileCount: number;
  sourcePathLabel: string | null;
  notes: string | null;
};

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

`SaveBackupSummaryDto`、`ProfileAutoSaveBackupCheckDto` 和 `SaveBackupBackgroundStatusDto` 不返回完整本地路径、备份根目录、存档源目录、Steam ID、manifest 正文、文件 hash 列表、调度租约字段、worker 实例 id 或真实存档内容。

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
- `start_install_task` 是后端驱动的安装提交入口。前端只提交 `gameId`、`modId`、`profileId` 和 layer 摘要；后端从已持久化导入记录和受控 sandbox 重建 `InstallPlan`，再在同一 `gameId/profileId` 写锁下执行 `InstallPlan -> backup -> commit -> manifest`。该 command 不接受 `targetPath`、`allowedTargetRoots`、sandbox/cache 路径、导入包路径、游戏目录路径或备份/manifest 路径。
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
- `ReinstallPlanPreviewDto.status` 是判别字段：`ready` 必须同时返回非空 installed/candidate revision、非空 `planToken`、四类计数和空 reasons；`blocked` 必须返回 `planToken: null`，revision 可以为 null。`candidate_not_found` 必须返回 `candidateRevision: null`，前端不得伪造 summary 或提交 token。
- `start_reinstall_task` 返回 `TaskStartedDto { taskId, kind: "install", status: "queued" }`，先发送 `install.reinstall.queued`，后台 runner 再以同一 taskId 发送 `install.reinstall.*`。prepare/source/plan 在 write lock 外执行；commit 与 install/uninstall/controlled recovery 共享同一 `gameId/profileId` 写锁、reinstall recovery admission 和 Audit writer。
- `start_reinstall_task` 的 token 对前端是不透明值。后端在写锁内重新校验 game instance 未在 prepare 后改变、token、candidate ownership/source、旧 manifest、target/backup facts，并确认该 profile 没有任一 active reinstall transaction；不匹配时 fail closed。失败 event 使用 `install_reinstall_failed:<phase>`，phase 为 `planning`、`preflight`、`lock`、`backup`、`commit`、`manifest`、`post_commit`、`rollback` 或 `complete`。`post_commit` 表示 candidate manifest 已越过 commit point，不能伪装为 rolled back；后续由受控 reconciliation 收敛。
- 重装 preview blocking reason 的稳定值为 `not_installed`、`candidate_not_found`、`candidate_not_ready`、`candidate_owner_mismatch`、`candidate_already_installed`、`manifest_state_unsafe`、`installed_revision_unknown`、`source_unavailable`、`target_missing`、`target_changed`、`target_read_failed`、`backup_missing`、`backup_read_failed`、`plan_conflict`、`cross_mod_target_conflict` 和预留的 `preview_stale`。command error 负责输入/用例不可用，reason 只负责可展示预检结论。
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
      planToken: string;
      installedRevision: ModRevisionSummaryDto;
      candidateRevision: ModRevisionSummaryDto;
      counts: ReinstallTargetCountsDto;
      blockingReasons: [];
    }
  | {
      status: "blocked";
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
```

### 6. Mod 预览图

Mod 预览图属于导入分析结果，不属于前端文件读取能力。具体安全策略见 [Mod 预览图安全处理设计](MOD_PREVIEW_IMAGE_PIPELINE_DESIGN.md)。

首批 command / 结果形态：

```text
start_import_mod_task(input)
get_mod_library()
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
```

边界：

- 当前 `start_import_mod_task` 会做 archive 路径基础校验，通过后端 `TaskManager` 登记 `mod_import` queued 任务，返回 `TaskStartedDto { taskId, kind, status }`，并发送 `hmm://task-progress` 的 `mod_import.queued` 事件；随后后台 prepare runner 会执行受控 zip 沙盒解包和预览图处理，并发送 `unpack.*`、`preview_image.*` 与 `prepare.completed` 事件。受控 zip 解包会拒绝父级穿越、绝对路径、symlink entry、大小写不敏感路径碰撞、entry 数超限、单文件解压后大小超限和总解压大小超限。prepare 成功后，导入分析结果会保存到 app data 下的受控结果仓储；进度事件仍不承载巨大结果。若任务在 running prepare 期间被取消，zip 解压会在 entry/chunk 检查点协作式停止并清理本次 sandbox；预览图扫描/处理会在 scanner 遍历和 processor 读文件、解码前后、缩略图写入前后的检查点停止。runner 会停止保存结果，不再发送 `prepare.completed`，也不会用 failed 覆盖 cancelled 状态，并会 best-effort 触发一次缩略图缓存维护。
- `get_mod_library()` 返回已持久化的导入分析结果列表，条目包含 `id`、`name`、`author`、`versionLabel`、`status`、`sizeLabel`、`categoryLabels` 和 `previewImage`。当前 MVP 使用 `packageId` 作为稳定 `id`；`name` 优先来自后端包元数据分析，缺失时回退 `packageId`；`author` 和 `versionLabel` 只来自后端解析的短文本 metadata；`categoryLabels` 来自后端解析的通用 category 和 tags，不由前端从路径推断。后端会从受控 sandbox 的 manifest/readme 候选解析通用元数据，多个 manifest 候选只用于补齐缺失字段，不作为安装事实来源。
- `get_mod_detail(modId)` 通过后端受控 `modId` 查询单个导入结果，返回 `null` 或包含 `previewImage` 与 `metadata { version, author, category, tags, dependencies }` 摘要的详情 DTO；这些 metadata 字段只用于展示和后续诊断输入，不表示依赖已安装、冲突已验证或安装计划事实。前端不传递 sandbox/cache/archive-internal 路径。
- `get_mod_dependency_graph()` 基于已持久化导入结果返回只读声明图。节点只包含后端 `modId` 和展示名；边只包含声明来源 `sourceModId`、包内短文本 `dependency`，以及当该声明与另一个已导入 `modId` 规范化后精确匹配时才返回的 `matchedImportedModId`。该命令不接受路径、不创建长任务、不发送 progress event、不判断依赖是否已安装、不返回 install/profile/manifest 状态，也不把展示名或 metadata 摘要升级成安装事实。
- `get_mod_detail_preview_image(modId)` 通过后端受控 `modId` 查询并生成详情页更大派生预览图，返回 `PreviewImageDto` 或 `null`。该命令固定使用后端 `preview-1024` 策略，不接受前端传入尺寸、variant、路径或图片字节；后端会基于已持久化导入记录定位受控 sandbox，重扫受限候选并处理首个可用候选。该命令对导入记录和原始 Mod 包只读，不写回导入记录，处理过程中只会写入可丢弃的 thumbnail cache，不创建长任务，不发送 progress event；DTO 不包含显式 variant、logical path、sandbox/cache/archive-internal 路径、本地路径或图片字节。
- `get_preview_image_candidates(modId)` 基于已持久化导入记录返回受限候选列表，返回 `null` 表示该 `modId` 未登记。该命令只接受后端 `modId`，不接受 sandbox/cache/archive-internal 路径；后端通过受控 sandbox locator 重扫候选，并应用 `max_candidates_per_package` 上限。DTO 只包含 `candidateIndex`、`fileName` 和 `compressedSizeBytes`，不返回 logical path、`thumbnailUrl`、缓存路径、本地路径或图片字节。该命令不写导入结果，不创建长任务，不发送 progress event。
- `select_preview_image_candidate(modId, candidateIndex)` 基于已持久化导入记录写回用户选择的预览图，返回更新后的 `PreviewImageDto`，返回 `null` 表示该 `modId` 未登记。该命令只接受后端 `modId` 和非负 `candidateIndex`，不接受 logical path、sandbox/cache/archive-internal 路径、压缩包内部路径、本地图片路径或图片字节；后端会重新定位受控 sandbox、重扫受限候选并按候选序号处理单个候选。该命令不创建长任务，不发送 progress event；处理失败或 URL 解析失败会返回并持久化 `fallback`。
- `get_preview_image_diagnostics()` 基于已持久化导入结果返回预览图处理摘要：`totalImportedMods`、`thumbnailCount`、`fallbackCount`、按 `reason` 聚合的 `fallbackReasons`，以及用于导出前确认的 `exportCategories`。当前 `exportCategories` 声明预览图聚合摘要可纳入诊断包，并明确排除缩略图文件、`thumbnailUrl` 资源引用和原始第三方 Mod 包内容。该命令不创建长任务、不发送 progress event、不读取或导出第三方图片内容、不返回 `thumbnailUrl`、缓存路径、sandbox 路径或本地路径。
- `export_preview_image_diagnostics()` 基于同一份已脱敏摘要写入受控诊断 zip。该命令不接受输出路径参数；后端固定写入 app data 下的 `logs/diagnostics/`，返回 `exportId`、`fileName`、`sizeBytes` 和本次导出的 `diagnostics` 摘要。当前 zip 只包含 `preview-image-diagnostics.json`，不包含缩略图文件、`thumbnailUrl`、`contentHash`、sandbox/cache/local 路径、README 全文、原始第三方 Mod 包内容或原始日志。导出成功后后端会写入最小 Audit Log 事件；若诊断 zip 写入失败，会先写入只含稳定错误分类和聚合计数的失败审计事件；若审计写入失败，命令不返回成功。该命令不创建长任务、不发送 progress event；更通用的日志/audit 诊断包导出仍需后续治理能力补齐。
- `export_audit_log_diagnostics()` 导出已脱敏审计日志诊断包。该命令不接受输出路径、日志路径或事件数量参数；后端固定读取 app data 下已校验的最近审计事件，单次最多 200 条，并固定写入 app data 下的 `logs/diagnostics/`。返回 DTO 只包含 `exportId`、`fileName`、`sizeBytes` 和 `auditEventCount`，不返回审计事件正文、审计日志路径、本地路径、原始错误文本、第三方 Mod 内容、缩略图 URL 或缓存/sandbox 路径。该命令不创建长任务、不发送 progress event；当前只覆盖 Audit Log 子集。
- `export_support_diagnostics()` 导出完整支持诊断包。该命令不接受输出路径、日志路径、类别选择、行数或事件数量参数；后端固定从 app data 下读取已校验 App Log / Task Log 文本行、已校验 Audit Log 事件和平台摘要，并固定写入 app data 下的 `logs/diagnostics/`。返回 DTO 只包含 `exportId`、`fileName`、`sizeBytes`、`appLogLineCount`、`taskLogLineCount` 和 `auditEventCount`，不返回日志正文、审计事件正文、诊断包路径、本地路径、原始错误文本、第三方 Mod 内容、缩略图 URL、`contentHash` 或缓存/sandbox 路径。该命令不创建长任务、不发送 progress event；用户可见入口仍应在前端展示类别确认，而不是展示敏感原文。
- `maintain_thumbnail_cache()` 手动触发后端缩略图缓存维护，复用当前导入结果引用保留、settings 空间上限 / LRU 清理和可选按时间保留逻辑。该命令不创建长任务、不发送 progress event、不返回清理报告或真实缓存路径；清理失败按 best-effort 处理，不改变导入、安装、卸载或回滚事实。
- `get_thumbnail_cache_settings()` 读取当前受控后端设置并返回 `AppSettingsDto`。该命令不接受参数、不写入 settings 文件、不触发缓存维护，也不返回 settings 文件路径、缓存路径、sandbox 路径或任意文件系统路径。
- `set_thumbnail_cache_settings({ thumbnailCacheMaxBytes, thumbnailCacheMaxAgeDays })` 写入受控后端设置并返回当前设置 DTO。`thumbnailCacheMaxBytes` 可为正整数或 `null`，`null` 表示回退默认空间上限；`0` 会返回稳定错误码 `thumbnail_cache_max_bytes_invalid`。`thumbnailCacheMaxAgeDays` 可为正整数天数或 `null`，`null` 表示不启用按时间保留延迟、沿用当前未引用缩略图维护语义；`0` 会返回稳定错误码 `thumbnail_cache_max_age_days_invalid`。该命令不接收或返回 settings 文件路径、缓存路径、sandbox 路径或任意文件系统路径。
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
  taskLogLineCount: number;
  auditEventCount: number;
};
```

`fileName` 只是文件名，不是完整本地路径；前端不能传入或拼接导出路径。当前导出包可包含已脱敏平台摘要、已校验 App Log 文本行、已校验 Task Log 文本行和最多 200 条已校验 Audit Log 事件，但命令 DTO 本身不返回日志正文、事件正文、诊断包路径、本地路径、原始 Mod 包内容、缩略图 URL、`contentHash`、缓存/sandbox 路径、原始日志或未脱敏错误文本。

## 窗口关闭与托盘生命周期

- `hmm://window-close-requested` 由 Tauri 后端在主窗口收到关闭请求时发出；后端会先阻止默认关闭，前端必须显示关闭选择或按已保存偏好调用窄命令。
- `hide_main_window_to_tray` 只隐藏当前主窗口，不执行备份、不修改 Profile、不读取路径。
- `exit_app` 只退出当前 Tauri 主客户端进程，不声明后台守护已接管。
- `get_app_exit_guard()` 是只读结构化决策；所有真正退出入口，包括主窗口关闭、remembered exit 和托盘“退出程序”，都必须经过同一流程。
- `exit_app({ request: { overrideUnprotected } })` 要求显式布尔值。普通退出只能传 `false`；只有危险退出对话框的当次明确确认可以传 `true`。后端在真正退出前始终重新计算 guard，不信任前端缓存。
- `exit_app({ request: { overrideUnprotected: false } })` 若在查询后因状态竞态变为不安全，会返回稳定 code `exit_confirmation_required`；前端必须重新读取 `get_app_exit_guard`，不得解析 `CommandErrorDto.message` 猜测原因。
- 危险退出默认操作和初始焦点为留在托盘，不显示 remember；Escape、overlay 和关闭按钮都只取消。`starting` override 不 unregister、不清除 `desiredEnabled`。

```ts
type AppExitGuardReason =
  | "background_starting"
  | "background_not_enabled"
  | "registration_failed"
  | "worker_unhealthy"
  | "permission_required"
  | "unsupported_platform"
  | "status_unavailable";

type AppExitGuardDto =
  | { decision: "safe"; reason: null }
  | { decision: "confirmation_required"; reason: AppExitGuardReason };

type ExitAppRequestDto = {
  overrideUnprotected: boolean;
};
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
