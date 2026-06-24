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
| `hmm-infra` 缩略图缓存 | MVP 已落地 | 已有原子写入、opaque URL 返回、package 登记校验、symlink 拒绝和 infra-local 清理策略；清理只作用于应用数据目录下的 `thumbnails` 缓存根。 |
| `hmm-infra` 导入沙盒准备器 | 最小实现已落地 | 已有 `ZipModImportPackagePreparer`，能把 zip 解压到 task-scoped sandbox，并拒绝父级穿越、绝对路径、symlink entry、大小写不敏感路径碰撞；解压失败会清理本次 task sandbox。当前只覆盖 zip，并已由 AppState 装配到后台 prepare runner。 |
| `hmm-infra` 包元数据分析 | MVP 已落地 | 已有 `SandboxModPackageMetadataAnalyzer`，只从安全 sandbox 中有限读取 manifest JSON 和 README，推断 `displayName` / `display_name` / `name` / `title` 或 README 标题。 |
| `hmm-app` 预览图服务 | 已落地到 MVP | 已有 `PreviewImageService` 和 `ModImportAnalysisService`，且 URL 解析职责已收敛到导入分析边界；prepare runner 成功后会保存导入分析结果。 |
| `hmm-app` 导入 prepare 服务 | 已落地到 MVP | 已有 `ModImportPrepareService` 和 `ModImportTaskRunner`，能通过 `ModImportPackagePreparer` 取得 sandbox package、调用导入分析服务、更新 task 状态、生成 `unpack.*` / `preview_image.*` / `prepare.completed` 进度事件，并将分析结果写入结果仓储。 |
| Tauri DTO | 已落地 | 已有 `PreviewImageDto`、fallback reason DTO、`ImportPreviewImage -> PreviewImageDto` 映射测试，以及 library/detail DTO 序列化测试。 |
| Custom protocol | 部分落地 | 已注册 `thumbnail` protocol，并支持 `thumbnail://...` 以及 Windows WebView 兼容的 `http://thumbnail.localhost/...` 形态；已补 symlink/package registry 等安全测试。缓存清理由 `hmm-infra` 的 store 生命周期 API 处理，不通过 protocol 或前端触发。 |
| `start_import_mod_task` | prepare runner 与结果保存已接线 | 当前校验 archive 路径、登记 queued 的 `mod_import` task 并发送 `mod_import.queued`；随后后台 runner 执行 zip 沙盒解包和预览图处理，发送受控进度事件，并保存导入分析结果。running prepare 被取消后，runner 会在检查点停止保存结果和完成事件。 |
| `get_mod_library` / `get_mod_detail` | MVP 已落地 | 查询 app data 下的导入分析结果仓储，返回包含 `previewImage` 的 library/detail DTO；展示名优先来自后端包元数据分析，缺失时回退 `packageId`。 |
| 前端类型与卡片展示 | MVP 已落地 | 已有 `PreviewImage` union、卡片 `<img>` 懒加载、加载失败 fallback 和静态测试；库页面会优先加载真实 DTO，后端不可用或结果为空时保留 mock fallback。 |
| 并发限制和事件 | 部分落地 | prepare runner 已发送 task progress 且事件携带 `taskId`；图片解码并发 limiter 已通过 app 层 `LimitedPreviewImageProcessor` 以默认并发 2 接入；running prepare cancellation 已有 runner 检查点保护，但尚未深入 zip 解压循环内部中断。 |

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

当前已在 `FileSystemThumbnailStore` 落地最小清理 API：调用方传入仍被导入结果或库记录引用的 `ThumbnailRef` 集合，store 只删除未引用的缩略图文件和清空后的 package 目录。

边界约束：

- 清理范围固定为 `root_dir/thumbnails/`，其中 `root_dir` 是后端组合根传入的应用数据目录；不读取或删除原始 Mod 包、sandbox、staging、游戏目录或存档目录。
- 删除前检查 `thumbnails` 根、package entry 和缩略图 entry 的 symlink metadata；遇到 symlink、非预期目录或非普通文件时跳过，不跟随。
- 对实际删除目标执行 canonical containment 校验，确认仍位于 canonical `thumbnails` 根下。
- 当前只做非递归清理：删除 stale 文件和空 package 目录，不递归清理未知子目录。
- 清理失败不改变安装、卸载、回滚或导入结果事实；缩略图仍是可删除、可重建的派生缓存。

尚未落地的缓存生命周期能力：

- 定时后台维护任务。
- 空间上限、LRU 或按时间保留策略。
- 与导入结果仓储的自动联动触发。

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
- prepare runner 在 prepare 返回后的检查点读取 task 状态；如果已经取消，则不保存导入分析结果，不发送 `mod_import.prepare.completed`，也不发送 failed 覆盖事件。
- 当前实现不强制中断正在进行的 zip 解压或图片处理线程；如需更细粒度中断，后续要把 cancellation token 继续下传到 infra 解压/扫描/处理循环。

### 包元数据展示名

当前 library/detail 的 `name` / `displayName` 已不再只能使用 `packageId`。prepare 阶段会通过 `ModPackageMetadataAnalyzer` port 分析安全 sandbox 中的有限元数据：

- manifest JSON 候选：`manifest.json`、`mod.json`、`metadata.json`、`info.json`。
- 支持字段：`displayName`、`display_name`、`name`、`title`。
- README 候选：`README.md`、`README.txt`、`README`，使用第一个 Markdown 标题或非空文本行。
- 单个元数据文件读取上限为 64 KiB，扫描深度限制为 2 层，symlink 和异常 entry 跳过。
- 元数据缺失、损坏或不可读时回退 `packageId`，不阻断导入主流程。

该能力仍是 MVP：尚未做游戏专属 manifest schema、版本号、作者、依赖、分类或标签解析。

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
- zip 解包只写入 app-controlled task sandbox，不写游戏目录；包级路径穿越、绝对路径、symlink entry 和大小写不敏感路径碰撞必须阻断导入。
- 预览图阶段使用已登记的 `mod_import.preview_image.processing` 和 `mod_import.preview_image.fallback` phase code。
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
- 导入沙盒准备器拒绝 `../`、绝对路径、symlink entry 和大小写不敏感路径碰撞；失败时不保留部分解压出的 task sandbox。
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
