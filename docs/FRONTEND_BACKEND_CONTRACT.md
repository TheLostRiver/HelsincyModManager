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
- 预览计划：`preview_install_plan`、`preview_retarget_plan`
- 启动长任务：`start_import_mod_task`
- 查询导入结果：`get_mod_library`、`get_mod_detail`、`get_mod_detail_preview_image`
- 查询诊断摘要：`get_preview_image_diagnostics`
- 手动后端维护：`maintain_thumbnail_cache`
- 写入受控设置：`set_thumbnail_cache_settings`
- 取消长任务：`cancel_task`

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

新增 task kind 时必须在此表登记对应 phase code，避免前端硬编码未登记值。

规则：

- 每个进度事件必须携带 `taskId`。
- 前端不能靠“当前页面只有一个任务”来匹配事件。
- 取消使用 `cancel_task(taskId)`；当前实现支持取消 `queued` 和 `running` 任务。running prepare 不会强制杀线程；zip 解压、预览图候选扫描和预览图 processor 会在后端 cancellation token 检查点协作式停止。图片库单次解码/编码调用本身仍不是抢占式中断；runner 会在取消后停止后续持久化、完成事件和失败覆盖事件。
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

### 2. `replacement / retarget`

首批 command：

```text
list_replacement_targets(gameId)
analyze_replacement_assets({ gameId, packageId })
preview_retarget_plan({ gameId, packageId, binding })
```

边界：

- 前端只提交 `packageId` 和 `targetId`。
- 后端通过 repository 解析 package，不接受 cache path。
- MHW adapter 负责 slot 解析、catalog 归一化和路径级 plan。
- 返回 preview 时可展示最终相对路径摘要，但前端不能自行生成路径。

### 3. 安装计划预览

首批 command：

```text
preview_install_plan(input)
start_install_task(input)
cancel_task(taskId)
```

边界：

- `preview_install_plan` 不写真实游戏目录。
- `start_install_task` 必须基于已经生成或可重建的 plan。
- 真实 commit 过程必须写 manifest，并能回滚或恢复。

### 4. Mod 预览图

Mod 预览图属于导入分析结果，不属于前端文件读取能力。具体安全策略见 [Mod 预览图安全处理设计](MOD_PREVIEW_IMAGE_PIPELINE_DESIGN.md)。

首批 command / 结果形态：

```text
start_import_mod_task(input)
get_mod_library(query)
get_mod_detail(modId)
get_mod_detail_preview_image(modId)
get_preview_image_candidates(modId)
select_preview_image_candidate(modId, candidateIndex)
get_preview_image_diagnostics()
maintain_thumbnail_cache()
set_thumbnail_cache_settings({ thumbnailCacheMaxBytes, thumbnailCacheMaxAgeDays })
```

边界：

- 当前 `start_import_mod_task` 会做 archive 路径基础校验，通过后端 `TaskManager` 登记 `mod_import` queued 任务，返回 `TaskStartedDto { taskId, kind, status }`，并发送 `hmm://task-progress` 的 `mod_import.queued` 事件；随后后台 prepare runner 会执行受控 zip 沙盒解包和预览图处理，并发送 `unpack.*`、`preview_image.*` 与 `prepare.completed` 事件。受控 zip 解包会拒绝父级穿越、绝对路径、symlink entry、大小写不敏感路径碰撞、entry 数超限、单文件解压后大小超限和总解压大小超限。prepare 成功后，导入分析结果会保存到 app data 下的受控结果仓储；进度事件仍不承载巨大结果。若任务在 running prepare 期间被取消，zip 解压会在 entry/chunk 检查点协作式停止并清理本次 sandbox；预览图扫描/处理会在 scanner 遍历和 processor 读文件、解码前后、缩略图写入前后的检查点停止。runner 会停止保存结果，不再发送 `prepare.completed`，也不会用 failed 覆盖 cancelled 状态，并会 best-effort 触发一次缩略图缓存维护。
- `get_mod_library()` 返回已持久化的导入分析结果列表，条目包含 `id`、`name`、`author`、`versionLabel`、`status`、`sizeLabel`、`categoryLabels` 和 `previewImage`。当前 MVP 使用 `packageId` 作为稳定 `id`；`name` 优先来自后端包元数据分析，缺失时回退 `packageId`；`author` 和 `versionLabel` 只来自后端解析的短文本 metadata；`categoryLabels` 来自后端解析的通用 category 和 tags，不由前端从路径推断。后端会从受控 sandbox 的 manifest/readme 候选解析通用元数据，多个 manifest 候选只用于补齐缺失字段，不作为安装事实来源。
- `get_mod_detail(modId)` 通过后端受控 `modId` 查询单个导入结果，返回 `null` 或包含 `previewImage` 与 `metadata { version, author, category, tags, dependencies }` 摘要的详情 DTO；这些 metadata 字段只用于展示和后续诊断输入，不表示依赖已安装、冲突已验证或安装计划事实。前端不传递 sandbox/cache/archive-internal 路径。
- `get_mod_detail_preview_image(modId)` 通过后端受控 `modId` 查询并生成详情页更大派生预览图，返回 `PreviewImageDto` 或 `null`。该命令固定使用后端 `preview-1024` 策略，不接受前端传入尺寸、variant、路径或图片字节；后端会基于已持久化导入记录定位受控 sandbox，重扫受限候选并处理首个可用候选。该命令对导入记录和原始 Mod 包只读，不写回导入记录，处理过程中只会写入可丢弃的 thumbnail cache，不创建长任务，不发送 progress event；DTO 不包含显式 variant、logical path、sandbox/cache/archive-internal 路径、本地路径或图片字节。
- `get_preview_image_candidates(modId)` 基于已持久化导入记录返回受限候选列表，返回 `null` 表示该 `modId` 未登记。该命令只接受后端 `modId`，不接受 sandbox/cache/archive-internal 路径；后端通过受控 sandbox locator 重扫候选，并应用 `max_candidates_per_package` 上限。DTO 只包含 `candidateIndex`、`fileName` 和 `compressedSizeBytes`，不返回 logical path、`thumbnailUrl`、缓存路径、本地路径或图片字节。该命令不写导入结果，不创建长任务，不发送 progress event。
- `select_preview_image_candidate(modId, candidateIndex)` 基于已持久化导入记录写回用户选择的预览图，返回更新后的 `PreviewImageDto`，返回 `null` 表示该 `modId` 未登记。该命令只接受后端 `modId` 和非负 `candidateIndex`，不接受 logical path、sandbox/cache/archive-internal 路径、压缩包内部路径、本地图片路径或图片字节；后端会重新定位受控 sandbox、重扫受限候选并按候选序号处理单个候选。该命令不创建长任务，不发送 progress event；处理失败或 URL 解析失败会返回并持久化 `fallback`。
- `get_preview_image_diagnostics()` 基于已持久化导入结果返回预览图处理摘要：`totalImportedMods`、`thumbnailCount`、`fallbackCount` 和按 `reason` 聚合的 `fallbackReasons`。该命令不创建长任务、不发送 progress event、不读取或导出第三方图片内容、不返回 `thumbnailUrl`、缓存路径、sandbox 路径或本地路径；它只为后续诊断导出提供已脱敏统计输入，完整诊断包导出仍需单独设计。
- `maintain_thumbnail_cache()` 手动触发后端缩略图缓存维护，复用当前导入结果引用保留、settings 空间上限 / LRU 清理和可选按时间保留逻辑。该命令不创建长任务、不发送 progress event、不返回清理报告或真实缓存路径；清理失败按 best-effort 处理，不改变导入、安装、卸载或回滚事实。
- `set_thumbnail_cache_settings({ thumbnailCacheMaxBytes, thumbnailCacheMaxAgeDays })` 写入受控后端设置并返回当前设置 DTO。`thumbnailCacheMaxBytes` 可为正整数或 `null`，`null` 表示回退默认空间上限；`0` 会返回稳定错误码 `thumbnail_cache_max_bytes_invalid`。`thumbnailCacheMaxAgeDays` 可为正整数天数或 `null`，`null` 表示不启用按时间保留延迟、沿用当前未引用缩略图维护语义；`0` 会返回稳定错误码 `thumbnail_cache_max_age_days_invalid`。该命令不接收或返回 settings 文件路径、缓存路径、sandbox 路径或任意文件系统路径。
- 前端只能接收后端生成的 `previewImage` 结构。
- 前端不能提交真实缓存路径、压缩包内部路径或本地图片路径让后端读取。
- 预览图处理失败返回 `fallback` 状态，不应阻断 Mod 导入主流程。
- 失败原因使用稳定 `snake_case` 字符串；已注册的 `reason` 值见下文 DTO 定义（例如 `too_large`、`decode_failed`、`pixel_limit_exceeded`、`cache_write_failed`）。
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
};
```

该 DTO 不包含 `thumbnailUrl`、`contentHash`、缓存路径、本地路径或图片字节，不能作为图片导出或缓存读取入口。

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
