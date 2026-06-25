# Mod 预览图安全处理设计

本文档定义 Helsincy Mod Manager 导入第三方 Mod 包时，如何安全、稳定地提取、校验、转换和展示 Mod 预览图。目标是避免超大图片、损坏图片、伪装图片或图片炸弹影响应用稳定性，同时保证 Mod 卡片 Full Bleed 封面排版可控。

## 设计原则

- 安全和稳定性优先于图片保真度。
- 第三方 Mod 包中的原始图片不能直接进入前端 UI。
- 原始导入包保持只读；缩略图是可删除、可重建的派生缓存。
- 图片处理属于导入分析流水线，不属于前端展示规则。
- 前端只展示后端生成的受控缩略图或默认占位图。
- 图片处理失败不应阻止 Mod 导入，除非同时触发更高层级的包安全风险。
- 真实缓存路径、原始包路径和本地用户路径不能暴露给前端或日志。

## 非目标

- 不在 MVP 中支持用户手动裁剪预览图。
- 不保存第三方原始预览图作为 UI 资源。
- 不把预览图缓存作为安装、卸载或回滚事实来源。
- 不允许前端根据本地文件路径、游戏路径或压缩包内部路径拼接图片地址。
- 不为每个游戏单独实现一套预览图展示组件。

## 风险模型

导入包中的预览图是不可信输入。必须防御：

- 扩展名伪装，例如 `.png` 实际不是 PNG。
- 损坏图片导致解码器错误或崩溃。
- 超大压缩态文件导致 I/O 和内存压力。
- 超大像素图片导致解码后内存暴涨。
- 大量候选图片拖慢包分析。
- 极端长宽比破坏卡片布局。
- 前端直接加载本地原图导致 WebView 卡顿或 OOM。

## 模块边界

### `hmm-core`

定义纯领域模型和策略值，不读取真实文件系统：

```text
PreviewImagePolicy
  max_input_bytes
  max_decoded_pixels
  max_candidates_per_package
  output_max_edge_px
  output_quality
  preferred_output_format

PreviewImageStatus
  Thumbnail
  Fallback

PreviewImageRejectionReason
  Missing
  TooLarge
  TooManyCandidates
  UnsupportedFormat
  DecodeFailed
  PixelLimitExceeded
  CacheWriteFailed
```

`CacheWriteFailed` 用于缩略图缓存写入或 `thumbnailUrl` 解析失败的降级。它属于缓存层失败，不阻断 Mod 导入主流程，只影响展示。

`hmm-core` 只表达规则和结果，不依赖图片库、缓存目录、Tauri 或平台 API。

### `hmm-ports`

定义应用层依赖的接口：

```text
PackagePreviewScanner
  从已安全解压的 sandbox/cache 中返回预览图候选

PreviewImageProcessor
  校验、解码、缩放并生成缩略图

ThumbnailStore
  保存缩略图缓存，并把后端 opaque 引用解析为受控 URL

ModPackageMetadataAnalyzer
  从已安全解压的 sandbox/cache 中读取有限包元数据，用于展示名等非安装事实
```

接口参数应使用包内逻辑路径、内部 ID、hash 或后端 source ref。不要要求前端传入本地缓存路径。

缓存清理由后端基础设施层或后续专门维护服务触发，不能让前端直接删除缓存目录，也不能把真实缓存路径放入 DTO。当前 MVP 在 `FileSystemThumbnailStore` 提供 infra-local prune API，不扩展 `ThumbnailStore` port，避免 app 层依赖缓存布局细节。

### `hmm-app`

负责编排导入流程：

```text
安全解压完成
-> 包结构分析
-> 发现预览图候选
-> 应用 PreviewImagePolicy
-> 调用 PreviewImageProcessor
-> 保存 ThumbnailStore
-> 调用 ModPackageMetadataAnalyzer
-> 写入 Mod 元数据中的 display_name 和 preview_image 字段
```

图片处理应作为导入任务的一部分携带 `task_id`。失败时记录结构化原因，并返回 fallback 结果。当前 prepare 阶段通过 `mod_import.preview_image.fallback` task event 的 `error` 字段携带稳定 fallback reason，不把路径、原始错误文本或图片内容放进事件。

### `hmm-infra`

负责真实 I/O 和图片处理：

- 从 sandbox/cache 读取候选文件。
- 先检查文件大小和 magic bytes。
- 读取图片 header 获取尺寸和格式。
- 解码前检查像素数上限。
- 生成固定规格缩略图。
- 写入应用数据目录下的 thumbnails 缓存。
- 返回后端受控的缩略图引用。
- 从安全 sandbox 中有限读取 manifest JSON 和 README，用于推断展示名。

图片处理不能在持有游戏写锁时执行。

### Tauri command

Tauri command 只做窄用例入口和 DTO 映射：

- 可以返回 `previewImage` 的结构化状态。
- 不能暴露任意本地文件读取能力。
- 不能让前端提交本地图片路径要求后端读取。
- 不能把真实缓存路径直接返回给前端。

`thumbnailUrl` 的字节流由 **custom protocol handler** 提供，不属于 Tauri command。handler 必须满足：

- 只接受 opaque `thumbnailRef` 形态的请求（语义上为 `thumbnail://<package_id>/<variant>/<content_hash>`），不接受任意路径。
- 可以为了 Tauri/WebView 平台兼容接受等价的受控 origin，例如 `http://thumbnail.localhost/<package_id>/<variant>/<content_hash>`；前端仍只能消费后端 DTO 返回的 `thumbnailUrl`，不能自行拼接。
- 解析后定位到受控缓存目录，校验 `thumbnails` 根、package 目录和最终文件都不是 symlink / junction，最终路径不穿越且不指向缓存目录之外，并拒绝未登记 package 的访问。
- 设置正确的 `Content-Type`（如 `image/jpeg`）和可缓存响应头。
- 文件缺失或解析失败时返回合适的 HTTP 状态（如 404），由前端 `<img onError>` 降级到 fallback 占位。

具体 command 命名、错误 DTO、长任务事件和 typed API 边界必须遵循 [前后端通信契约设计](FRONTEND_BACKEND_CONTRACT.md)。本文只定义 Mod 预览图 feature 的输入输出形状和安全规则，不另起一套跨边界协议。

### 前端

前端只负责展示：

- 有缩略图时渲染受控 `thumbnailUrl`。
- 无缩略图或加载失败时展示当前渐变剪影占位。
- 用固定比例和 `object-fit: cover` 保证排版稳定。
- 不基于失败原因执行安全判断。

## 默认策略

MVP 默认值建议：

| 项目 | 默认值 | 说明 |
| --- | ---: | --- |
| 单张候选图压缩态大小 | `20 MiB` | 超过后直接 fallback |
| 解码后像素数 | `16 MP` | 防止超大位图占用内存 |
| 每个包候选图片数 | `8` | 防止大量图片拖慢分析 |
| 输出缩略图最长边 | `768 px` | 兼顾卡片清晰度和体积 |
| 输出格式 | `JPEG` | 当前 MVP 使用可控质量 JPEG；WebP 保留为后续可选优化 |
| 输出质量 | `80` | 保持体积可控 |
| 图片处理并发 | `1-2` | 避免导入多包时内存峰值过高；当前默认通过 app 层 limiter 限制为 2 |

这些值应来自数据驱动配置或应用默认策略，不应写死在前端组件中。后续可以提供高级设置，但默认值必须偏保守。

## 候选图发现

预览图候选只来自安全解压后的 sandbox/cache。候选文件选择规则：

- 优先匹配常见名称，例如 `preview`、`cover`、`poster`、`thumbnail`、`image`。
- 支持常见扩展名，例如 `.png`、`.jpg`、`.jpeg`、`.webp`。
- 不信任扩展名，最终以 magic bytes 和解码结果为准。
- 只处理有限数量候选，超过数量后忽略低优先级候选；如果未来产品策略要求候选超量直接降级，才返回 `too_many_candidates`。
- 候选排序应稳定，避免同一个包重复导入时得到不同封面。

候选扫描不应读取真实游戏目录，也不应读取原始压缩包外的路径。

当前默认策略是“保留稳定排序后的前 N 个候选并继续处理”，因为它能在保证资源上限的同时给用户保留预览图。scanner 会维护 top N，app 层服务也会防御性地只处理前 N 个候选。`too_many_candidates` 保留为可观测 fallback reason，但当前默认路径不发出该 reason。

手动选择候选图也必须建立在同一组受限候选之上。当前后端已有只读候选列表 command `get_preview_image_candidates(modId)`：它只接受后端已登记的 `modId`，通过导入结果仓储确认记录存在，再由后端 sandbox locator 解析受控 sandbox 根，并应用同一份 `PreviewImagePolicy` / `max_candidates_per_package` 上限。候选列表 DTO 只返回 `candidateIndex`、`fileName` 和 `compressedSizeBytes`，不返回 logical path、sandbox/cache 路径、压缩包内部路径、`thumbnailUrl` 或图片字节。

当前后端也已有写回 command `select_preview_image_candidate(modId, candidateIndex)`。该命令只接受后端 `modId` 和非负 `candidateIndex`，通过导入结果仓储和 sandbox locator 重新定位受控 sandbox，再复用 `PreviewImageService::process_selected_package_preview` 重新扫描安全 sandbox 中的受限候选并处理单个候选。处理结果会解析为 `PreviewImageDto` 并写回已导入 Mod 记录；未知 `modId` 返回 `null`。该入口不接受本地路径、缓存路径、sandbox/cache/archive-internal 路径、压缩包内部路径或图片字节作为输入。

详情页更大预览图使用独立 command `get_mod_detail_preview_image(modId)`。该命令同样只接受后端已登记的 `modId`，通过导入结果仓储和 sandbox locator 定位受控 sandbox，然后使用后端固定的 `preview-1024` 策略复用同一条 scanner / processor / thumbnail store 流水线。返回值仍是既有 `PreviewImageDto`；未知 `modId` 返回 `null`；处理失败或 URL 解析失败返回 fallback。该命令对导入记录和原始 Mod 包只读，不写回导入记录；处理过程中只会写入可丢弃的 thumbnail cache，不创建 task，不发送 progress event，也不接受或返回 logical path、sandbox/cache/archive-internal 路径、压缩包内部路径、本地图片路径、显式 variant 字段或图片字节。

## 缩略图生成

处理顺序必须先便宜、后昂贵：

```text
候选 source ref
-> 文件大小检查
-> magic bytes 检查
-> header 尺寸读取
-> 像素数检查
-> 受控解码
-> 等比缩放
-> 转码为标准格式
-> 原子写入缩略图缓存
```

输出缩略图不要求裁成 3:4。后端只负责控制尺寸和格式；前端卡片用 `object-fit: cover` 进行展示裁切。这样可以避免在后端过早丢失画面信息，也让详情页未来可以复用同一缩略图。

缓存文件名应基于稳定 hash，而不是原始文件名：

```text
thumbnails/
  <package_id>/
    preview-<max_edge_px>-<content_hash>.<ext>
```

文件名采用 `<variant>-<content_hash>.<ext>` 顺序，其中 `variant` 由后端图片处理策略的 `output_max_edge_px` 派生，形如 `preview-768` 或 `preview-1024`。当前默认策略仍生成 `preview-768`，但 `ThumbnailStore` port 和 `FileSystemThumbnailStore` 已不再硬编码该值，可为详情页更大派生图复用同一条受控后端流水线。`<ext>` 由后端根据 `preferred_output_format` 决定，当前 MVP 默认 `.jpg`（对应 JPEG 输出）。扩展名不进入前端 DTO，前端只看到 `thumbnailUrl`。如果后续把默认格式切到 WebP，文件名和缓存布局变化由后端内部吸收，DTO 不变。

实际目录由 infra 决定，不进入前端 DTO。

## 包元数据分析

导入结果中的展示名和展示用元数据属于后端包分析结果，不应由前端从文件名、路径或压缩包内部路径推断。当前 MVP 的元数据分析用于生成安全、短小、可显示的 `display_name`，并解析有限的通用 manifest schema：

```text
安全解压 sandbox
-> 有限扫描 manifest/readme 候选
-> 跳过 symlink、目录、过大文件和异常 entry
-> 读取 displayName / display_name / name / title
-> 读取 version / author / category / tags / dependencies 等通用字段
-> 或读取 README 第一条标题/非空文本
-> 清洗控制字符、折叠空白、限制长度
-> 缺失或损坏时回退 package_id
```

边界约束：

- 只读安全 sandbox，不读取原始压缩包外的路径。
- 元数据分析失败不阻断导入主流程，也不影响预览图 fallback 语义。
- 展示名、分类、标签、版本、作者和依赖文本不是安装、卸载、回滚或冲突检测事实来源。
- 当前 `get_mod_library` 会暴露由后端生成的 `author`、`versionLabel` 和 `categoryLabels`；`get_mod_detail` 会暴露 `metadata { version, author, category, tags, dependencies }` 摘要。这些字段只用于展示和后续诊断输入，不表示依赖安装状态。
- 当前 `get_mod_dependency_graph` 会基于已持久化导入结果生成只读依赖声明图；边只表示某个导入记录声明了依赖文本，并在该文本与另一个已导入 `modId` 规范化后精确匹配时返回 `matchedImportedModId`。它不表示依赖已安装、已启用或已通过安装计划校验。
- 多个 manifest 候选会按缺失字段补齐；`authors`、`tags` 和 `dependencies` 当前支持字符串或字符串数组，作者数组会合并为短文本。
- 更复杂的游戏专属 manifest schema、依赖是否安装的语义校验和安装计划级依赖图构建属于后续包分析 / 安装计划能力；它们需要游戏 adapter、已安装 Mod 事实或 profile/install manifest 作为依据，不能仅凭预览图导入阶段读取到的短文本 metadata 推断。

## 缩略图缓存清理

缩略图缓存是派生数据，可以删除并在后续导入或重新处理时重建。当前 MVP 的清理策略是 **引用保留 + stale 删除**，并已补充 infra-local **空间上限 + LRU 删除** 能力：

```text
当前仍被导入结果或库记录引用的 ThumbnailRef 集合
-> sanitize package_id / variant / content_hash
-> 遍历 app-data thumbnails 根
-> 保留匹配 <variant>-<content_hash>.* 的文件
-> 删除未引用的普通文件
-> 删除清空后的 package 目录
```

当调用方需要控制缓存占用时，可以使用 `FileSystemThumbnailStore::prune_to_size_limit(max_bytes, retained)`：

```text
当前仍被导入结果或库记录引用的 ThumbnailRef 集合
-> sanitize package_id / variant / content_hash
-> 遍历 app-data thumbnails 根内普通缩略图文件
-> 统计普通文件总大小
-> 保留当前引用文件
-> 对未引用文件按 accessed 时间优先、modified 时间兜底排序
-> 从最旧文件开始删除，直到总大小不超过 max_bytes
-> 删除清空后的 package 目录
```

安全边界：

- 清理只作用于应用数据目录下的 `thumbnails/` 缓存根。
- 不删除原始 Mod 包、安全解压 sandbox、retarget staging、游戏目录或存档目录。
- 不跟随 symlink / junction；遇到 symlink、非预期目录或非普通文件时跳过。
- 删除前必须确认 canonical 删除目标仍位于 canonical `thumbnails/` 根下。
- 当前清理不递归删除未知子目录，避免把异常目录结构当成正常缓存处理。
- 清理失败不影响安装、卸载、回滚或导入结果事实，只影响派生封面缓存占用。

当前 MVP 已接入导入结果仓储联动触发：`mod_import` prepare runner 在成功保存导入分析结果后，会收集结果仓储中仍被 library/detail 记录引用的 `ThumbnailRef` 集合，并调用后端缓存维护 port 执行 best-effort maintenance。该维护先执行引用保留清理；如果后端 settings 配置了 `thumbnailCacheMaxAgeDays`，则只删除超过该天数的未引用缩略图，未配置时沿用立即删除未引用缩略图的既有语义。随后维护会使用后端 settings 的 `thumbnailCacheMaxBytes` 执行空间上限 / LRU 清理；未配置或读取失败时回退默认 `512 MiB`。仍被当前导入结果引用的缩略图不会因按时间保留或空间上限被删除。该触发不扩展前端命令，不把缓存路径暴露给 DTO，也不会因为清理失败把成功导入改成失败。

导入结果中的缩略图记录会持久化 `variant`，用于后续缓存维护精确保留非默认尺寸派生图。旧记录缺少 `variant` 字段时按 `preview-768` 兼容处理，避免升级后误删既有默认缩略图。

当前后端 settings 仓储会读写 `app_data/config/settings.json`：

```json
{
  "version": 1,
  "thumbnailCacheMaxBytes": 536870912,
  "thumbnailCacheMaxAgeDays": 30
}
```

AppState 会尝试启动后端定时维护线程，默认每 6 小时执行一次同一条 best-effort 缓存维护链路。该线程不创建前端 task、不发送 progress event、不暴露缓存路径；启动或清理失败只影响派生缓存占用，不影响导入、安装、卸载或回滚事实。

当前已有 `maintain_thumbnail_cache` 后端命令可手动触发同一条 best-effort 缓存维护链路；该命令不创建前端 task、不发送 progress event、不返回清理报告或真实缓存路径。`set_thumbnail_cache_settings` 可写入 `thumbnailCacheMaxBytes` 和 `thumbnailCacheMaxAgeDays`，二者可为正整数或 `null`，`null` 表示回退默认语义，`0` 会被拒绝；该命令不暴露 settings 文件路径。尚未定义 UI 设置入口；这属于后续缓存生命周期治理。已有的 `prune_unreferenced_thumbnails_older_than`、`prune_to_size_limit`、settings 读写、默认导入后维护、定时后端维护和手动后端触发只属于后端生命周期能力，不改变缩略图 URL 契约。

## 诊断摘要

当前后端已有 `get_preview_image_diagnostics` 命令，用于为诊断导出提供已脱敏的预览图处理摘要。该摘要只基于已持久化导入结果聚合：

- `totalImportedMods`
- `thumbnailCount`
- `fallbackCount`
- `fallbackReasons[]`，其中每项包含稳定 `snake_case` 的 `reason` 和 `count`
- `exportCategories[]`，用于导出前类别确认；当前只把 `preview_image_summary` 标记为 `included`，并把 `thumbnail_files`、`thumbnail_urls` 和 `raw_package_content` 标记为 `excluded`

该命令不读取缩略图文件、不导出第三方图片内容、不返回 `thumbnailUrl`、`contentHash`、缓存路径、sandbox 路径、原始 Mod 包路径或本地路径。它也不创建长任务、不发送 progress event。

当前后端还提供 `export_preview_image_diagnostics` 命令，用于写入一个受控的预览图诊断 zip。该命令不接受输出路径参数；后端固定写入 app data 下的 `logs/diagnostics/`，返回文件名和导出大小摘要。当前 zip 只包含 `preview-image-diagnostics.json`，内容来自上述脱敏聚合摘要和 `exportCategories`，不包含缩略图文件、`thumbnailUrl`、`contentHash`、缓存路径、sandbox 路径、本地路径、README 全文、原始第三方 Mod 包内容或原始日志。导出成功后会写入最小 Audit Log 事件，审计字段仅包含操作名、类别、结果、导出文件名/ID、大小和聚合计数；若诊断 zip 写入失败，会写入失败 Audit Log 事件，审计字段仅包含操作名、类别、失败结果、导出文件名、稳定错误分类和聚合计数，不包含原始错误文本或底层路径；若审计写入失败，命令不返回成功，避免出现无审计证据的成功导出。

这只是预览图摘要诊断包基础，不等同于完整日志/审计诊断包系统。当前 Audit Log 只记录该导出动作的安全摘要，不把 Audit Log 本身纳入该预览图 zip。后端已有最小审计日志读取 port、infra 实现、app 层 `AuditLogDiagnosticsExportService` 和 Tauri 命令 `export_audit_log_diagnostics`，可以读取最近 N 条已校验审计事件并写入受控 `audit-log-diagnostics.json` 诊断包，同时为该导出动作写入最小 Audit Log 事件；单次导出最多包含 200 条审计事件，避免诊断包无界膨胀。该命令不接受输出路径、日志路径或事件数量参数，也未改变当前预览图诊断 zip。后端还已有 App Log / Task Log 文本读取基础：`TextLogReader` 和 `FileSystemTextLogReader` 只从 app data 下的 `logs/app/`、`logs/tasks/` 读取白名单日志文件，跳过敏感或不合规文本行，并只返回安全文件名和文本行；同时已有 `DiagnosticsEnvironmentProvider` / `SystemDiagnosticsEnvironmentProvider` 平台摘要基础，可生成应用版本、OS、CPU 架构和受控 game adapter id 列表。`export_support_diagnostics` 已通过 `SupportDiagnosticsExportService` 组合平台摘要、App Log、Task Log 和 Audit Log，写入受控完整诊断 zip 并记录最小 Audit Log 事件；若平台摘要、App Log、Task Log、Audit Log 读取或诊断 zip 写入失败，也会写入只含稳定错误分类和聚合计数的失败 Audit Log 事件；该命令不接受输出路径、日志路径、类别选择、行数或事件数量参数，DTO 只返回文件名、大小和聚合计数，不返回日志正文、审计事件正文或路径，也未改变当前预览图或审计日志诊断 zip。后续若要提供用户可触发的前端完整导出入口，仍必须继续遵守 [日志与审计设计](LOGGING.md) 的统一脱敏、类别确认和用户主动导出规则。

## 前端 DTO

前端 DTO 是 [前后端通信契约设计](FRONTEND_BACKEND_CONTRACT.md) 的 feature-specific 扩展。字段使用 camelCase，enum 值使用稳定 `snake_case` 字符串。前端建议消费以下结构：

```ts
type PreviewImage =
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

`thumbnailUrl` 必须由后端解析为受控资源 URL。本项目采用 custom protocol 方案（见 [前后端通信契约设计](FRONTEND_BACKEND_CONTRACT.md) 「Mod 预览图」一节）：后端注册 `thumbnail://` scheme，由 protocol handler 根据 opaque `thumbnailRef`（`package_id` / `variant` / `content_hash`）从应用缓存目录读字节返回，前端拿不到真实磁盘路径。asset protocol、`convertFileSrc` 和 base64 data URL 不作为正式契约方案。

在 Windows WebView 或 Tauri 内部实现需要时，后端可以把同一 opaque 引用解析为 `http://thumbnail.localhost/...` 这类受控 origin。它只是传输兼容形态，不改变安全边界：前端不得拼接该 URL，也不得从中推断缓存布局。

`reason` 只用于展示和测试分支，不应被前端用来推断底层文件系统状态。需要重新处理图片时，前端只能发送 `packageId`、`modId`、`taskId` 等后端生成的稳定 id。

## 前端展示规则

Mod 卡片保持固定比例：

```css
.mod-card {
  position: relative;
  aspect-ratio: 3 / 4;
  overflow: hidden;
}

.mod-card__poster-img {
  position: absolute;
  inset: 0;
  width: 100%;
  height: 100%;
  object-fit: cover;
  object-position: center top;
}
```

图片标签应使用懒加载和异步解码：

```tsx
<img
  className="mod-card__poster-img"
  src={previewImage.thumbnailUrl}
  loading="lazy"
  decoding="async"
  alt=""
/>
```

图片加载失败时，卡片应退回当前渐变剪影占位。列表中不要显示低层解码错误文本，避免污染浏览体验。

## 错误处理

图片处理失败返回 fallback，而不是中断导入：

| reason | 行为 |
| --- | --- |
| `missing` | 使用默认封面 |
| `too_large` | 使用默认封面，导入结果可提示“预览图过大已忽略” |
| `too_many_candidates` | 当前默认路径不发出；若未来改为候选超量直接降级，则使用默认封面 |
| `unsupported_format` | magic bytes 不匹配或格式不支持时使用默认封面，并通过稳定 fallback reason 进入 task event / 诊断聚合；若后续升格为安全审计事件，也只能记录稳定分类，不记录路径或原始错误文本 |
| `decode_failed` | 使用默认封面 |
| `pixel_limit_exceeded` | 使用默认封面 |
| `cache_write_failed` | 使用默认封面，导入仍可继续；同 `thumbnailRef` 的 URL 下次访问由 protocol handler 重试或返回 404 触发前端回退 |

如果同一个包同时触发路径穿越、压缩炸弹或其他包级安全问题，应由导入流水线按包安全规则阻断，而不是由预览图模块单独决定。

## 日志与隐私

日志只记录结构化字段：

```text
task_id
mod_id
package_id
candidate_count
input_size
decoded_width
decoded_height
format
result
reason
duration_ms
```

禁止记录：

- 原始图片内容。
- 第三方 Mod 包内容。
- 完整本地路径。
- Windows/Linux 用户名。
- Steam ID。
- token、cookie、API key。

用户可见消息使用稳定 `message_code`，不拼接本地路径。

## 并发与性能

- 图片处理属于 prepare 阶段，可以和包分析、hash 等任务并行。
- 不要在持有游戏写锁时处理图片。
- 图片解码应有单独并发限制，避免多个大图同时解码；当前实现由 `hmm-app` 的 `LimitedPreviewImageProcessor` 包裹 `PreviewImageProcessor` trait object，默认并发为 2。
- 并发 limiter 只控制图片处理器入口，不改变候选扫描、沙盒解包、进度事件或 thumbnail URL 契约。
- running prepare cancellation 使用后端 cancellation token 协作式下传；当前 zip 解压会在 entry 循环和文件 chunk 复制前检查取消，取消后清理本次 task sandbox，不保存导入结果，也不发送完成事件，并由 runner best-effort 触发一次缩略图缓存维护。
- zip 解包只写入 task-scoped sandbox，并在解包前/解包中拒绝父级穿越、绝对路径、symlink entry、大小写不敏感路径碰撞、entry 数超限、单文件解压后大小超限和总解压大小超限。当前默认上限为 `16384` 个 entry、单文件 `1 GiB`、总解压普通文件大小 `4 GiB`。
- 预览图 scanner / processor 也使用同一个 cancellation token：scanner 在候选目录遍历中检查取消，processor 在路径校验、文件读取、图片尺寸读取、完整解码前后、缩略图编码后和缓存写入后检查取消。
- `image` crate 的单次解码/编码调用本身仍不是抢占式中断；取消会在调用前后的安全检查点生效。若取消发生在缩略图缓存写入后、导入结果保存前，缩略图仍只是可删除、可重建的派生缓存；runner 会在取消返回前复用当前导入结果引用触发 best-effort 维护，清理失败不改变取消语义。
- 任务进度事件必须携带 `task_id`。
- 缩略图缓存可以由后端维护任务异步清理；当前已有 infra-local prune API，并已在导入结果保存成功、prepare 返回后观察到取消、或 prepare 因取消中止时进行 best-effort 仓储联动 prune，清理失败不影响安装状态、取消状态或导入结果持久化语义。

## 测试要求

测试必须使用人工构造的最小图片和临时目录，不能提交真实第三方 Mod 包。

至少覆盖：

- 正常 PNG 生成缩略图。
- 正常 JPEG 生成缩略图。
- 正常 WebP 生成缩略图。
- 没有预览图时返回 fallback。
- 扩展名为图片但 magic bytes 不匹配。
- 损坏图片解码失败。
- 文件大小超过限制。
- 像素数超过限制。
- 候选图片数量超过限制时保留 top N 继续处理，scanner 和 app 层服务都不能处理超过策略上限的候选。
- 缩略图缓存写入失败时导入仍返回 fallback。
- 缩略图缓存清理只删除未引用普通文件，保留当前引用，跳过 symlink 或异常 entry，不越过缓存根。
- 包元数据展示名优先来自 sandbox manifest/readme，缺失或损坏时回退 package id，且不读取 sandbox 外路径。
- zip 沙盒准备器拒绝路径穿越、绝对路径、symlink entry、大小写不敏感路径碰撞、entry 数超限、单文件解压后大小超限和总解压大小超限，且失败时清理本次 task sandbox。
- protocol handler 拒绝 traversal、absolute path、symlinked `thumbnails` 根、package / 文件 symlink、未登记 package，并返回正确 content type。
- 前端卡片在有图、无图、图片加载失败时比例不变。
- 日志不包含完整路径或第三方图片内容。

## 后续扩展

- 前端详情页尚未接入更大派生图展示；后端只读入口 `get_mod_detail_preview_image(modId)` 已可按固定 `preview-1024` 策略生成/解析详情预览图，但仍不能直接展示原图。
- 支持用户手动选择候选图的 UI；只读候选列表 command/DTO、后端按候选序号选择并复用同一条处理流水线的基础入口、以及选择结果写回已导入 Mod 记录的 command 已落地。
- 支持游戏专属包元数据 schema、依赖安装状态校验和安装计划级依赖图；该能力应接入游戏 adapter 与安装事实来源，不应把预览图 metadata 摘要或当前声明图升级成依赖真相来源。
- 继续细化图片处理取消治理，例如更细粒度的解码超时、worker 隔离或取消后 stale 缩略图维护策略；这些属于后续鲁棒性增强，当前后端 MVP 已通过并发 limiter、task event 和协作式 cancellation 检查点落地。
- 支持 UI 设置入口和更完整的保留策略展示。
- 支持按主题或分类生成更丰富的默认封面，但默认封面仍属于前端展示层。
- 扩展完整日志/审计诊断包的前端用户确认入口；当前已落地的 `export_preview_image_diagnostics` 只导出预览图聚合摘要 JSON，`export_audit_log_diagnostics` 只导出最多 200 条已校验 Audit Log 事件，`export_support_diagnostics` 可导出平台摘要、已校验 App Log、已校验 Task Log 和已校验 Audit Log 的完整支持诊断 zip。这些能力都不能导出第三方图片内容、缩略图 URL、原始 Mod 包内容、原始日志或未脱敏路径；对应 command DTO 也只能返回摘要。
