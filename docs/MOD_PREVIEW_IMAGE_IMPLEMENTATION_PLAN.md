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
| `hmm-ports` 接口 | 已落地 | 已有 `PackagePreviewScanner`、`PreviewImageProcessor`、`ThumbnailStore` 和相关值对象。 |
| `hmm-infra` magic bytes 与候选扫描 | 部分落地 | 已能按扩展名发现候选并稳定保留 top N；扩展名仍只作为候选发现，不作为格式信任依据。当前实现没有把候选超量显式上报为 `TooManyCandidates`。 |
| `hmm-infra` 图片处理器 | 已落地 | 已有大小、magic bytes、header 尺寸、像素数、解码、缩放、编码和缓存写入流程，并覆盖 PNG/JPEG/WebP、损坏图、像素超限等验收项。 |
| `hmm-infra` 缩略图缓存 | 部分落地 | 已有原子写入、opaque URL 返回、package 登记校验和 symlink 拒绝。后续仍需要补清理策略。 |
| `hmm-app` 预览图服务 | 部分落地 | 已有 `PreviewImageService` 和 `ModImportAnalysisService`，且 URL 解析职责已收敛到导入分析边界；真实导入任务尚未接线。 |
| Tauri DTO | 已落地 | 已有 `PreviewImageDto`、fallback reason DTO 和 `ImportPreviewImage -> PreviewImageDto` 映射测试。 |
| Custom protocol | 部分落地 | 已注册 `thumbnail` protocol，并支持 `thumbnail://...` 以及 Windows WebView 兼容的 `http://thumbnail.localhost/...` 形态；已补 symlink/package registry 等安全测试。后续仍需清理策略。 |
| `start_import_mod_task` | 最小入口已落地 | 当前只校验 archive 路径、登记 queued 的 `mod_import` task 并发送 `mod_import.queued`。尚未解压、分析、持久化或调用预览图处理。 |
| `get_mod_library` / `get_mod_detail` | 未落地 | 真实库查询和详情查询尚未返回 `previewImage`。 |
| 前端类型与卡片展示 | 部分落地 | 已有 `PreviewImage` union、卡片 `<img>` 懒加载、加载失败 fallback 和静态测试；当前仍主要消费 mock 数据。 |
| 并发限制和事件 | 未完全落地 | 文档要求图片解码并发受限，并在预览图阶段发送 task progress；当前真实执行链路尚未接入。 |

## 已知差异与决策点

### 候选数量超限

当前扫描器在遍历过程中保留排序后的前 `max_candidates_per_package` 个候选，但不会返回 `TooManyCandidates`。后续需要二选一并保持文档、测试和实现一致：

- **推荐：** 扫描器返回 top N，同时在诊断/日志中记录候选被截断；最终仍尝试处理 top N。`TooManyCandidates` 仅在策略要求“候选超量直接降级”时使用。
- **替代：** 发现候选超过限制后直接返回 `Fallback(TooManyCandidates)`，不处理任何图片。这个方案更保守，但用户体验较差。

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

当前实现已有 segment 白名单、containment、content type、未登记 package 拒绝和 symlink 拒绝。后续可补充缓存生命周期与清理策略。

### 图片处理并发

设计要求图片解码并发限制为 `1-2`。在真实导入任务接线前可以暂不落地；一旦支持后台多包导入或并行分析，必须引入单独的图片处理 limiter，不能依赖全局 task 数量。

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

### 3. 接入真实导入分析任务

在安全解压和包结构分析骨架可用后，将预览图服务接入 `mod_import` prepare 阶段：

```text
start_import_mod_task
-> queued task
-> safe unpack into sandbox
-> package structure analyze
-> preview image processing
-> persisted import result
-> get_mod_library / get_mod_detail exposes previewImage
```

要求：

- 所有 progress event 携带 `taskId`。
- 预览图阶段使用已登记的 `mod_import.preview_image.processing` 和 `mod_import.preview_image.fallback` phase code。
- 不把 sandbox 路径、缓存目录、包内路径或真实本地路径放入 DTO、event message 或日志。
- 图片处理不在 game write lock 内执行。

### 4. 接入库查询与前端真实数据

实现 `get_mod_library` 和 `get_mod_detail` 后，前端卡片从真实 DTO 消费 `previewImage`：

- `thumbnail`：渲染 `thumbnailUrl`。
- `fallback` 或图片加载失败：保留当前渐变剪影占位。
- 前端不使用 `convertFileSrc`、asset protocol、base64 data URL 或本地路径拼接。

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
