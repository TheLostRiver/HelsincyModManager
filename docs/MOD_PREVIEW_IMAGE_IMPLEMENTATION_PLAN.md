# Mod 预览图安全处理实现计划

本文记录 Mod 预览图安全处理功能的当前落地状态、剩余缺口和后续实施顺序。长期设计约束以 [Mod 预览图安全处理设计](MOD_PREVIEW_IMAGE_PIPELINE_DESIGN.md)、[前后端通信契约设计](FRONTEND_BACKEND_CONTRACT.md)、[安全策略](../SECURITY.md) 和 [测试指南](TESTING.md) 为准。

本文不是逐行代码脚手架。后续实现应以当前代码为准，避免照搬历史计划中的示例代码。

## 目标

预览图处理属于 Mod 导入分析 prepare 阶段：

- 原始 Mod 包和包内原始图片始终是不可信输入。
- 前端只消费后端生成的 `PreviewImageDto`，不能提交本地路径、缓存路径或压缩包内部路径要求后端读取。
- 后端只返回受控 `thumbnailUrl` 或 fallback 状态，不暴露真实磁盘路径。
- 图片处理失败降级为 fallback，不阻断 Mod 导入主流程；包级路径穿越、压缩炸弹等安全问题仍由导入流水线阻断。
- 缩略图缓存是可删除、可重建的派生数据，不作为安装、卸载或回滚事实来源。

## 当前状态

| 区域 | 状态 | 说明 |
| --- | --- | --- |
| `hmm-core` 领域模型 | 已落地 | 已有 `PreviewImagePolicy`、输出格式、状态、fallback reason 和策略校验测试。 |
| `hmm-ports` 接口 | 已落地 | 已有 `PackagePreviewScanner`、`PreviewImageProcessor`、`ThumbnailStore`、`ModImportPackagePreparer` 和相关值对象。 |
| `hmm-infra` magic bytes 与候选扫描 | MVP 已落地 | 已能按扩展名发现候选并稳定保留 top N；扩展名仍只作为候选发现，不作为格式信任依据。默认策略不因候选总数超限直接返回 `TooManyCandidates`。 |
| `hmm-infra` 图片处理器 | 已落地 | 已有大小、magic bytes、header 尺寸、像素数、解码、缩放、编码和缓存写入流程，并覆盖 PNG/JPEG/WebP、损坏图、像素超限等验收项。 |
| `hmm-infra` 缩略图缓存 | MVP 已落地 | 已有原子写入、opaque URL 返回、package 登记校验、symlink 拒绝、引用保留清理和 infra-local 空间上限 / LRU 清理策略；`ThumbnailStore` 已支持由调用方传入 `preview-<max_edge_px>` variant，默认仍是 `preview-768`；清理只作用于应用数据目录下的 `thumbnails` 缓存根。 |
| `hmm-infra` 导入沙盒准备器 | 最小实现已落地 | 已有 `ZipModImportPackagePreparer`，能把 zip 解压到 task-scoped sandbox，并拒绝父级穿越、绝对路径、symlink entry、大小写不敏感路径碰撞、entry 数超限、单文件解压后大小超限和总解压大小超限；解压失败或取消会清理本次 task sandbox。当前只覆盖 zip，并已由 AppState 装配到后台 prepare runner。 |
| `hmm-infra` 包元数据分析 | MVP 已落地 | 已有 `SandboxModPackageMetadataAnalyzer`，只从安全 sandbox 中有限读取 manifest JSON 和 README，推断展示名，并解析版本、作者、分类、标签和依赖的通用字段。 |
| `hmm-app` 预览图服务 | 已落地到 MVP | 已有 `PreviewImageService`、`ModImportAnalysisService` 和 `PreviewImageCandidateSelectionService`，且 URL 解析职责已收敛到应用层边界；prepare runner 成功后会保存导入分析结果。候选选择写回会基于已登记 `modId` 重新定位受控 sandbox、按后端候选序号复用 scanner / policy / processor 流水线，并把新的 `previewImage` 写回已导入 Mod 记录。 |
| `hmm-app` 导入 prepare 服务 | 已落地到 MVP | 已有 `ModImportPrepareService` 和 `ModImportTaskRunner`，能通过 `ModImportPackagePreparer` 取得 sandbox package、调用导入分析服务、更新 task 状态、生成 `unpack.*` / `preview_image.*` / `prepare.completed` 进度事件，并将分析结果写入结果仓储；`mod_import.preview_image.fallback` 事件会在 `error` 字段携带稳定 fallback reason，不使用 message 文本承载分支逻辑。 |
| Tauri DTO | 已落地 | 已有 `PreviewImageDto`、fallback reason DTO、`ImportPreviewImage -> PreviewImageDto` 映射测试，以及 library/detail DTO 序列化测试。 |
| Custom protocol | 部分落地 | 已注册 `thumbnail` protocol，并支持 `thumbnail://...` 以及 Windows WebView 兼容的 `http://thumbnail.localhost/...` 形态；已补 symlink/package registry 等安全测试。缓存清理由 `hmm-infra` 的 store 生命周期 API 处理，不通过 protocol 或前端触发。 |
| `start_import_mod_task` | prepare runner 与结果保存已接线 | 当前校验 archive 路径、登记 queued 的 `mod_import` task 并发送 `mod_import.queued`；随后后台 runner 执行 zip 沙盒解包和预览图处理，发送受控进度事件，并保存导入分析结果。running prepare 被取消后，runner 会在检查点停止保存结果和完成事件，并 best-effort 触发一次缩略图缓存维护。 |
| `get_mod_library` / `get_mod_detail` | MVP 已落地 | 查询 app data 下的导入分析结果仓储，返回包含 `previewImage` 的 library/detail DTO；展示名优先来自后端包元数据分析，缺失时回退 `packageId`；library DTO 暴露 `author`、`versionLabel`、`categoryLabels`，detail DTO 暴露通用 metadata 摘要。 |
| `get_mod_detail_preview_image` | 后端入口已落地 | 详情页可按后端 `modId` 请求更大派生预览图。后端固定使用 `preview-1024` 策略重扫受控 sandbox 并处理首个可用候选，返回既有 `PreviewImageDto`；该命令对导入记录只读，不写回导入记录，处理过程中只会写入可丢弃的 thumbnail cache，不创建 task，不发送 progress event，也不新增显式 variant 字段、sandbox/cache/archive-internal 路径、本地路径或图片字节。 |
| `get_preview_image_diagnostics` | 后端入口已落地 | 基于已持久化导入结果聚合预览图诊断摘要，返回总导入数、缩略图数、fallback 数、fallback reason 计数和导出前类别确认清单；当前只把预览图聚合摘要标记为可包含，并明确排除缩略图文件、`thumbnailUrl` 和原始 Mod 包内容。不导出第三方图片内容、缓存路径、sandbox 路径或本地路径。 |
| `export_preview_image_diagnostics` | 后端入口已落地 | 写入受控预览图诊断 zip，不接受前端输出路径；后端固定写入 app data 下的 `logs/diagnostics/`，返回 `exportId`、`fileName`、`sizeBytes` 和本次导出的诊断摘要。当前 zip 只包含脱敏的 `preview-image-diagnostics.json`，不包含缩略图文件、`thumbnailUrl`、`contentHash`、sandbox/cache/local 路径、README 全文、原始 Mod 包内容或原始日志。导出成功后会写入最小 Audit Log 事件，只记录 `operation`、`category`、`result`、`export_id`、`file_name`、`size_bytes` 和聚合计数；诊断 zip 写入失败时会写入失败 Audit Log 事件，只记录 `file_name`、稳定 `error_code` 和聚合计数，不记录原始错误文本或路径；Audit Log 写入失败时命令不报告成功。 |
| `export_audit_log_diagnostics` | 后端入口已落地 | 通过 `AuditLogReader` 读取最近 N 条已校验审计事件，写入受控 `audit-log-diagnostics.json` 诊断包，并为该导出动作写入最小 Audit Log 事件；单次导出最多包含 200 条审计事件，避免诊断包无界膨胀。该命令不接受输出路径、日志路径或事件数量参数，不改变当前预览图诊断 zip，也不在 DTO 中返回审计事件正文、原始日志、未脱敏路径、第三方 Mod 内容或缩略图 URL。 |
| App/Task 文本日志读取基础 | 后端 ports/infra 基础已落地 | `TextLogReader` / `FileSystemTextLogReader` 可从 app data 下的 `logs/app/` 与 `logs/tasks/` 读取最近 N 行已校验文本，只接受白名单文件名并跳过敏感或不合规行，返回值只包含安全文件名和文本行。该能力已通过 `export_support_diagnostics` 的 app service/command 链路受控使用，但不纳入当前预览图诊断 zip 或审计日志诊断 zip。 |
| 平台诊断摘要基础 | 后端 ports/infra 基础已落地 | `DiagnosticsEnvironmentProvider` / `SystemDiagnosticsEnvironmentProvider` 可生成应用版本、平台 OS、CPU 架构和受控 game adapter id 列表摘要，不读取本地路径或玩家数据。该能力已通过 `export_support_diagnostics` 的 app service/command 链路受控使用，但不纳入当前预览图诊断 zip 或审计日志诊断 zip。 |
| `export_support_diagnostics` | 后端入口已落地 | 通过 `SupportDiagnosticsExportService` 组合平台摘要、已校验 App Log 文本行、已校验 Task Log 文本行和已校验 Audit Log 事件，写入受控完整诊断 zip，并为导出动作写入最小 Audit Log 事件；若平台摘要、App Log、Task Log、Audit Log 读取或诊断 zip 写入失败，也会写入只含稳定错误分类和聚合计数的失败 Audit Log 事件。该命令不接受输出路径、日志路径、类别选择、行数或事件数量参数；DTO 只返回 `exportId`、`fileName`、`sizeBytes`、`appLogLineCount`、`taskLogLineCount` 和 `auditEventCount`，不返回日志正文、审计事件正文、路径、原始错误、第三方 Mod 内容或缩略图 URL；不改变当前预览图诊断 zip 或审计日志诊断 zip 的契约。 |
| `maintain_thumbnail_cache` | 后端入口已落地 | 手动触发同一条 best-effort 缓存维护链路；支持引用保留、可选按时间保留、settings 空间上限和 LRU 清理；不创建前端 task、不发送 progress event、不返回清理报告或真实缓存路径。 |
| `set_thumbnail_cache_settings` | 后端入口已落地 | 写入 `thumbnailCacheMaxBytes` 和 `thumbnailCacheMaxAgeDays` 后端设置；`null`/缺省表示回退默认语义，`0` 会被拒绝；不暴露 settings 文件路径。 |
| 前端类型与卡片展示 | MVP 已落地 | 已有 `PreviewImage` union、卡片 `<img>` 懒加载、加载失败 fallback 和静态测试；库页面会优先加载真实 DTO，后端不可用或结果为空时保留 mock fallback。 |
| 并发限制和事件 | 部分落地 | prepare runner 已发送 task progress 且事件携带 `taskId`；图片解码并发 limiter 已通过 app 层 `LimitedPreviewImageProcessor` 以默认并发 2 接入；running prepare cancellation 已下传到 zip 解压 entry/chunk、preview scanner 遍历和 processor 读文件/解码前后/缩略图写入前后检查点。图片库自身的单次解码/编码调用仍不是抢占式中断。 |

## 已知差异与决策点

### 候选数量超限

当前策略已定稿为 **保留稳定排序后的 top N 并继续处理**：

- `SandboxPackagePreviewScanner` 在遍历过程中维护最多 `max_candidates_per_package` 个候选。
- `PreviewImageService` 也会防御性地只处理前 `max_candidates_per_package` 个候选，避免其他 scanner 实现返回过量候选时造成过多解码。
- `TooManyCandidates` 仍保留为 domain/DTO reason，供未来产品策略改为“候选超量直接降级”或诊断记录时使用；当前默认路径不发出该 fallback。

### Thumbnail URL 解析职责

当前 `PreviewImageService` 只编排 scanner / processor 并返回 `PreviewImageProcessingResult`；`ModImportAnalysisService` 持有 `ThumbnailStore` 并调用 `resolve_url`。职责边界为：

- `PreviewImageService` 只返回 `PreviewImageProcessingResult` 和 `ThumbnailRef`。
- `ModImportAnalysisService` 或更靠近 DTO 的应用服务负责把 `ThumbnailRef` 解析为 `thumbnailUrl`。

这样可以避免不同服务注入不同 store 时产生不一致。缩略图 URL 解析失败仍由 `ModImportAnalysisService` 降级为 `CacheWriteFailed` fallback。

### 手动候选选择

当前已落地两个后端基础入口：

- `get_preview_image_candidates(modId)` 是只读候选列表 command。它只接受后端已登记的 `modId`，通过导入结果仓储确认记录存在，再由后端 sandbox locator 解析受控 sandbox 根；返回 DTO 只包含 `candidateIndex`、`fileName` 和 `compressedSizeBytes`，不包含 logical path、sandbox/cache 路径、压缩包内部路径、`thumbnailUrl` 或图片字节。
- `PreviewImageService::process_selected_package_preview` 会重新通过 scanner 获取受 `max_candidates_per_package` 限制的候选列表，并按后端候选序号只处理一个候选。该入口仍调用同一个 `PreviewImageProcessor`，因此文件大小、magic bytes、像素数、缩放、转码、缓存写入和取消检查都不会绕过。
- `select_preview_image_candidate(modId, candidateIndex)` 是写回已导入 Mod 记录的后端 command。它只接受 `modId` 和非负 `candidateIndex`，不会接受 logical path、sandbox/cache/archive-internal 路径、本地图片路径或图片字节；命令返回更新后的 `PreviewImageDto`，未知 `modId` 返回 `null`。
- 候选选择失败或缩略图 URL 解析失败时，写回 `fallback` 结果而不是阻断导入主流程；返回值和持久化记录保持一致。

尚未落地的部分：

- 前端可见的候选缩略图选择 UI。

后续补 UI 时，前端只能提交后端生成的候选标识或序号，不能提交 sandbox/cache/archive-internal 路径要求后端读取。

### Custom protocol 形态

长期契约仍以 `thumbnail://<package_id>/<variant>/<content_hash>` 作为 opaque URL 语义。当前实现还接受 `http://thumbnail.localhost/<package_id>/<variant>/<content_hash>`，这是 Tauri/WebView 兼容形态，不应被前端手工拼接，也不应暴露真实缓存路径。

### 缓存读取安全

协议 handler 需要满足：

- 只接受 package id、variant、content hash 三段安全 segment。
- 拒绝路径穿越、绝对路径和未登记 package。
- 拒绝最终文件是 symlink 或指向缓存根之外。
- 只从应用数据目录下的 thumbnails 缓存读取。
- 返回正确 `Content-Type` 和缓存头。

当前实现已有 segment 白名单、containment、content type、未登记 package 拒绝和 symlink 拒绝。缓存生命周期的最小清理 API 已在 infra store 中提供，protocol handler 不承担删除职责。

### 缩略图缓存清理策略

当前已在 `FileSystemThumbnailStore` 落地最小清理 API：调用方传入仍被导入结果或库记录引用的 `ThumbnailRef` 集合，store 只删除未引用的缩略图文件和清空后的 package 目录。另有 infra-local `prune_to_size_limit(max_bytes, retained)`，可在保留当前引用缩略图的前提下，按访问时间优先、修改时间兜底的 LRU 顺序删除未引用缩略图，直到普通缩略图文件总大小不超过上限。

边界约束：

- 清理范围固定为 `root_dir/thumbnails/`，其中 `root_dir` 是后端组合根传入的应用数据目录；不读取或删除原始 Mod 包、sandbox、staging、游戏目录或存档目录。
- 删除前检查 `thumbnails` 根、package entry 和缩略图 entry 的 symlink metadata；遇到 symlink、非预期目录或非普通文件时跳过，不跟随。
- 对实际删除目标执行 canonical containment 校验，确认仍位于 canonical `thumbnails` 根下。
- 当前只做非递归清理：删除 stale 文件和空 package 目录，不递归清理未知子目录。
- 清理失败不改变安装、卸载、回滚或导入结果事实；缩略图仍是可删除、可重建的派生缓存。

当前已落地的缓存生命周期能力：

- prepare runner 在成功保存导入分析结果后，会从结果仓储读取当前仍被 library/detail 记录引用的缩略图集合，并 best-effort 触发一次缓存维护。
- 导入结果会持久化缩略图 `variant`，缓存维护按 `package_id` / `variant` / `content_hash` 精确保留引用文件；旧导入记录缺少 `variant` 时默认兼容为 `preview-768`。
- 缓存维护会先执行引用保留清理；当后端 settings 配置了 `thumbnailCacheMaxAgeDays` 时，只删除超过该天数的未引用缩略图，未配置时沿用当前立即清理未引用缩略图的语义；随后使用 `thumbnailCacheMaxBytes` 执行空间上限 / LRU 清理，未配置或读取失败时回退默认 `512 MiB`。仍被当前导入结果引用的缩略图不会因按时间保留或空间上限被删除。
- `FileSystemThumbnailStore::prune_to_size_limit` 已提供后端空间上限 / LRU 清理能力；该 API 不暴露缓存路径给 app 或前端，也不通过 protocol handler 触发。
- `FileSystemThumbnailStore::prune_unreferenced_thumbnails_older_than` 已提供后端按时间保留能力；该 API 仍只作用于应用数据目录下的 `thumbnails` 缓存根，并保留当前引用缩略图。
- prune 失败不改变导入 task 的 completed 状态，也不发送用户可见失败事件；缓存仍是可删除、可重建的派生数据。
- 后端当前读写 `app_data/config/settings.json`：

```json
{
  "version": 1,
  "thumbnailCacheMaxBytes": 536870912,
  "thumbnailCacheMaxAgeDays": 30
}
```
- AppState 会尝试启动后端定时维护线程，默认每 6 小时执行一次同一条 best-effort 缓存维护链路；该线程不创建前端 task、不发送 progress event，也不暴露缓存路径，启动或清理失败只会降级为跳过本轮缓存治理。

尚未落地的缓存生命周期能力：

- UI 设置入口和更完整的保留策略展示。
- 详情页请求更大派生图的后端只读用例已落地为 `get_mod_detail_preview_image(modId)`，固定生成/解析 `preview-1024` 派生图；前端详情页展示入口仍未落地。该能力仍不能直接展示原图。

### 图片处理并发

设计要求图片解码并发限制为 `1-2`。当前实现已在 `hmm-app` 中提供 `LimitedPreviewImageProcessor`，作为 `PreviewImageProcessor` trait object 的装饰器包住真实 infra processor；`AppState` 默认使用 `DEFAULT_PREVIEW_IMAGE_PROCESSING_CONCURRENCY = 2`。

边界约束：

- limiter 控制进入图片处理器的并发数量，不限制候选扫描、zip 沙盒解包或其他 prepare 阶段工作。
- `PreviewImageService` 仍只依赖 `PackagePreviewScanner` 和 `PreviewImageProcessor` trait，不依赖 infra concrete type。
- infra 的 `ImageCratePreviewImageProcessor` 不持有全局 task 状态，也不感知 app 层并发策略。
- 该 limiter 不承担 task cancellation；运行中取消由 `TaskManager` 和 prepare runner 检查点处理。

### 运行中取消

当前 `cancel_task(taskId)` 可以取消 `queued` 和 `running` 的 `mod_import` task。对于 running prepare，取消语义是协作式的：

- `cancel_task` 立即把 task 状态标记为 `cancelled`，并发送 `mod_import.cancelled` 事件。
- prepare runner 在 prepare 返回后的检查点读取 task 状态；如果已经取消，则不保存导入分析结果，不发送 `mod_import.prepare.completed`，也不发送 failed 覆盖事件，并按当前已持久化导入结果的引用 best-effort 触发一次缩略图缓存维护。
- `ModImportPackagePreparer` 现在接收后端 cancellation token；`ZipModImportPackagePreparer` 在 zip entry 循环和文件 chunk 复制前检查取消，检测到取消后返回 cancelled error，并沿用失败清理路径删除本次 task sandbox。
- zip 解包当前默认限制最多 `16384` 个 entry、单个普通文件解压后最大 `1 GiB`、单个包总解压后普通文件大小最大 `4 GiB`；超过上限属于包级安全拒绝，失败后清理本次 task sandbox。
- `PreviewImageService` 会把同一个 cancellation token 下传到 `PackagePreviewScanner` 和 `PreviewImageProcessor`；scanner 在目录遍历期间检查取消，processor 在路径校验、文件读取、图片尺寸读取、完整解码前后、缩略图编码后和缓存写入后检查取消。
- 当前实现不强制抢占 `image` crate 的单次解码/编码调用；取消会在这些调用前后的检查点生效。若取消发生在缩略图缓存写入后、导入结果保存前，runner 仍不会保存导入结果，并会触发同一条 best-effort 缓存维护链路，使未引用的派生缩略图更早具备清理机会。

### 包元数据分析

当前 library/detail 的 `name` / `displayName` 已不再只能使用 `packageId`。prepare 阶段会通过 `ModPackageMetadataAnalyzer` port 分析安全 sandbox 中的有限元数据：

- manifest JSON 候选：`manifest.json`、`mod.json`、`metadata.json`、`info.json`。
- 支持字段：`displayName`、`display_name`、`name`、`title`。
- 通用 schema 字段：`version` / `modVersion`、`author` / `authors`、`category` / `type`、`tags`、`dependencies` / `depends` / `requires`。
- 多个 manifest 候选会按缺失字段补齐；`authors`、`tags` 和 `dependencies` 当前支持字符串或字符串数组，作者数组会合并为短文本。
- `get_mod_library` 的 `author` / `versionLabel` 和 `categoryLabels` 由后端解析到的通用 metadata 生成，不由前端从路径或文件名推断。
- `get_mod_detail` 返回 `metadata { version, author, category, tags, dependencies }` 摘要；这些字段只表示包内短文本声明，不表示依赖安装状态、冲突检测结果或安装计划事实。
- README 候选：`README.md`、`README.txt`、`README`，使用第一个 Markdown 标题或非空文本行。
- 单个元数据文件读取上限为 64 KiB，扫描深度限制为 2 层，symlink 和异常 entry 跳过。
- 元数据缺失、损坏或不可读时回退 `packageId`，不阻断导入主流程。

该能力仍是 MVP：尚未做游戏专属 manifest schema、依赖是否安装的语义校验或跨 Mod 依赖图构建。

## 下一批实施顺序

### 1. 补齐安全测试与小修

优先补不依赖真实导入流水线的测试：

- `thumbnail_protocol`：覆盖 symlink 拒绝、未登记 package 拒绝、Windows localhost 兼容形态、content type。
- `processor`：覆盖 JPEG 正常路径、WebP 正常路径、损坏图片 `DecodeFailed`、像素超限 `PixelLimitExceeded`。
- `scanner`：明确候选超量行为，补对应测试。

最小验证：

```powershell
cargo test -p hmm-infra preview_image
cargo test -p hmm-tauri thumbnail_protocol
```

### 2. 收敛 App 层职责（已完成）

`PreviewImageService` 与 `ModImportAnalysisService` 的职责已经整理为：

- `PreviewImageService` 编排 scanner 和 processor，返回 `ThumbnailRef` 或 fallback。
- `ModImportAnalysisService` 负责把处理结果映射为 `ImportPreviewImage`。
- URL 解析只保留在 `ModImportAnalysisService` 这个应用层边界，避免重复解析。

最小验证：

```powershell
cargo test -p hmm-app preview_image
cargo test -p hmm-app mod_import
```

### 3. 接入真实导入分析任务（MVP 已完成）

当前已有 `ModImportPrepareService` 和 `ModImportTaskRunner` 作为 app 层编排骨架。它们只依赖 ports，不依赖 infra concrete type。infra 侧已提供 zip 的最小安全沙盒准备器和 JSON 导入结果仓储，并已通过 `AppState` 装配进后台 prepare runner。

当前 `mod_import` prepare 阶段 MVP 链路：

```text
start_import_mod_task
-> queued task
-> safe unpack into sandbox via ModImportPackagePreparer
-> package structure analyze
-> preview image processing
-> persisted import result
-> get_mod_library / get_mod_detail exposes previewImage
```

要求：

- 所有 progress event 携带 `taskId`。
- zip 解包只写入 app-controlled task sandbox，不写游戏目录；包级路径穿越、绝对路径、symlink entry、大小写不敏感路径碰撞、entry 数超限、单文件解压后大小超限和总解压大小超限必须阻断导入。
- 预览图阶段使用已登记的 `mod_import.preview_image.processing` 和 `mod_import.preview_image.fallback` phase code；fallback 事件的 `error` 字段携带稳定 reason（如 `decode_failed` / `unsupported_format`），不承载路径、原始错误文本或图片内容。
- 不把 sandbox 路径、缓存目录、包内路径或真实本地路径放入 DTO、event message 或日志。
- 图片处理不在 game write lock 内执行。

### 4. 接入库查询与前端真实数据（MVP 已完成）

`get_mod_library` 和 `get_mod_detail` 已返回包含 `previewImage` 的真实 DTO，前端卡片按以下规则消费：

- `thumbnail`：渲染 `thumbnailUrl`。
- `fallback` 或图片加载失败：保留当前渐变剪影占位。
- 前端不使用 `convertFileSrc`、asset protocol、base64 data URL 或本地路径拼接。
- 当后端结果为空或查询失败时，当前页面保留现有 mock fallback，避免脚手架阶段出现空白库。

最小验证：

```powershell
cmd /c corepack pnpm run typecheck
cmd /c corepack pnpm run lint
cmd /c corepack pnpm run test
```

### 5. 收尾验证与文档同步

当真实导入链路、library/detail 查询和前端展示都接通后，执行完整检查：

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify.ps1
```

如果无法执行完整验证，最终回复或 PR 描述必须说明：

- 已执行哪些检查。
- 未执行哪些检查。
- 未执行原因。

## 验收标准

功能完成后必须满足：

- 正常 PNG/JPEG/WebP 候选图可以生成受控缩略图。
- 超大压缩态文件不进入解码阶段。
- magic bytes 不匹配的伪装图片返回 fallback。
- 损坏图片返回 fallback，不 panic。
- 解码后像素数超过限制返回 fallback。
- 候选图超量行为与文档一致，并有测试覆盖。
- 缩略图缓存写入或 URL 解析失败时导入主流程仍返回 fallback。
- protocol handler 不暴露真实缓存路径，并拒绝 traversal、absolute、symlink 和未登记 package。
- 导入沙盒准备器拒绝 `../`、绝对路径、symlink entry、大小写不敏感路径碰撞、entry 数超限、单文件解压后大小超限和总解压大小超限；失败时不保留部分解压出的 task sandbox。
- 前端卡片在 thumbnail、fallback、图片加载失败三种状态下尺寸不跳动。
- `PreviewImageDto` 字段与 TypeScript 类型一致。
- 任务进度事件携带 `taskId`。
- 日志不包含完整本地路径、第三方图片内容或敏感信息。
- 缩略图缓存可删除、可重建，不影响安装、卸载或回滚事实。

## 文档维护规则

改动以下内容时需要同步检查本文和设计文档：

- `PreviewImageDto` 字段或 fallback reason。
- `thumbnailUrl` 协议形态或缓存布局。
- 默认输出格式、大小限制、像素限制或候选数量策略。
- `start_import_mod_task`、`get_mod_library`、`get_mod_detail` 的契约。
- 图片处理并发、progress phase 或日志字段。
