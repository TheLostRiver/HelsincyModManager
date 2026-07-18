# Mod 库分页设计

> 状态：Slice 1（PR #186）和 Slice 2（PR #187）已完成；Slice 3 已完成实现、本地验证和独立复审、待 PR/merge，Slice 4 尚未开始，T18 整体仍在进行中。
>
> Slice 3 已接入数字分页 footer、250ms 搜索 debounce/latest-request gate、loading/error/empty、page-local selection 和当前页 durable status overlay；本地统一验证及四视图/四窗口视觉 smoke 已通过。

## 背景与现状审计

设计启动时，Mod 管理页面没有分页。后端 `get_mod_library()` 一次返回完整 `Vec<ModLibraryItemDto>`，前端把整个列表保存在 state，完成搜索和分类/状态筛选后，一次渲染全部结果。

设计启动时的数据流为：

```text
JsonModImportResultRepository.list_analysis()
  -> 合并 SQLite metadata overlay / category
  -> get_mod_library() 返回完整列表
  -> 前端查询全部 Mod 的 install manifest / recovery 状态
  -> 前端按搜索词和 filter 过滤
  -> 一次渲染全部可见卡片
```

该旧路径会同时产生四类问题：

- 列表很长时，定位、滚动和选择操作困难。
- bridge 返回完整卡片 DTO，预览图和标签会放大序列化与前端内存成本。
- install manifest / recovery 状态刷新会携带全部 Mod ID。
- 搜索、状态、分类和总数都在不同步骤处理，加入分页后容易出现“页码基于一套结果、卡片基于另一套结果”的错误。

设计启动时，搜索框提示支持“名称、作者或标签”，实际实现只匹配名称。Slice 1/2 已把三类字段统一到后端查询契约，Slice 3 的页面消费者改为使用该查询结果。

当前 Slice 3 工作分支已把 Mod 管理页面迁移到 `query_mod_library()`：前端只持有当前页 DTO，并只为当前页合并 manifest/recovery durable 状态；无参 `get_mod_library()` 仅作为尚未迁移调用方的兼容入口保留。

仓库已有的分页规划只服务第三方迁移批次的候选预览和结果查询，不覆盖主 Mod 库，因此需要独立任务和设计。

## 目标

- Mod 库始终只渲染当前页，降低长列表的操作和视觉负担。
- 搜索、分类/状态筛选、稳定排序、总数和分页由同一个查询快照决定。
- page query 只返回当前页条目，前端不再为列表页加载全部 Mod DTO。
- 当前页条目在返回前完成 profile-aware install/recovery 状态合并。
- 页码、每页数量、刷新、空状态、错误状态和快速切换行为可预测。
- “选择本页”和未来“选择全部匹配结果”保持清晰边界，避免隐藏的跨页误操作。
- 分页在经典、网格、列表和机能视图中共用同一查询状态，不按视图复制页面。
- 与批量迁移的持久化扩展协调，最终避免每次翻页都完整解析和合并整个 JSON 库。

## 非目标

首版不做：

- 无限滚动或自动加载下一页。
- 前端自行切片作为最终架构。
- 跨页“全选全部匹配结果”或跨页批量安装/卸载；这属于 T13。
- 拖拽跨页排序或自定义手工顺序。
- 为不同视图模式设置不同 page size。
- 把分页查询建模为 TaskManager 长任务或发送 progress event。
- 在 URL、日志或诊断包中持久化玩家搜索词。
- 为分页读取任意文件路径、游戏目录或第三方 Mod 内容。
- 在 Slice 3 提前实现 Slice 4 的可查询 read model、migration、大库性能门禁，或 T13/T17 的功能。

## 核心决策

### 后端查询分页是目标架构

纯前端 `visibleItems.slice(...)` 可以快速得到页码，但仍会把完整 Mod 库、全部预览 DTO 和全部状态请求送到前端，无法解决规模问题。Slice 1/2 已新增后端权威的 page query，Slice 3 负责迁移页面消费者。

为了兼容当前 JSON 仓储，首个后端切片允许在 `hmm-app` 内先执行 `list_analysis -> merge -> filter -> sort -> page`。这可以先收窄 Tauri payload 和前端渲染范围，但仍是 O(n) 的兼容阶段，不能作为大型 Mod 库性能工作的最终完成标准。

### 使用数字页，不使用 cursor 作为首版 UI

用户需要知道当前位置并能直接跳到某页，因此首版采用 1-based 数字页和 offset 语义。响应必须带当前实际页和匹配总数；前端不从当前条目数量猜总页数。

如果未来数据量达到 offset 查询不可接受的程度，可以在保持 UI 页码语义的前提下增加后端 page token/cache。本设计不提前暴露 cursor 给前端。

### 固定 page size 选项

首版 page size：

```text
12 / 24 / 48 / 96
```

- 默认值为 `24`。
- 最大值为 `96`，后端拒绝不在 allowlist 中的值。
- page size 不随窗口宽度或视图模式自动变化，避免切换视图时页码和选择范围跳动。
- 前端只持久化 page size 偏好，不持久化当前页、搜索词或选择状态。

### 稳定排序先于分页

分页前必须完成稳定排序。首版固定使用：

```text
name_asc
```

排序键使用展示名称的规范化形式，建议为 Unicode NFKC、trim 和 locale-independent lowercase；相同排序键始终以 `modId` 升序作为 tie-breaker。原始显示名称不被改写。

首版不增加排序菜单。未来加入 `imported_desc` 等排序前，必须先提供 durable `importedAt` 或等价序号，不能依赖 JSON 当前数组顺序或文件 mtime。

## 查询语义

查询必须严格按以下顺序执行：

```text
读取导入快照
  -> 合并 metadata overlay
  -> 合并用户分类
  -> 合并当前 profile 的 install/recovery 状态
  -> 规范化搜索
  -> 应用搜索和 filter
  -> 稳定排序
  -> 计算 matchingTotal
  -> page clamp
  -> 截取当前页
```

顺序是契约的一部分。尤其不能先分页再做状态或分类过滤，否则页面数量、页内数量和筛选结果会不一致。

兼容阶段必须先把各 repository 的结果组装成一份不可变的内存列表，再从该列表计算 `libraryTotal`、`matchingTotal` 和当前页，不能分别读取数据计算 count 与 items。由于 JSON 与 SQLite 之间没有跨存储原子事务，该阶段只能保证单次响应内部一致；最终可查询 read model 应在同一 SQLite 短 read transaction/snapshot 中完成 count 与 page query。若兼容读取期间检测到来源 revision 变化，应重试或返回稳定 unavailable error，不能拼接两个 revision 的结果。

### 搜索

- trim 首尾空白并折叠连续空白。
- 使用与排序一致的 Unicode 规范化和大小写规则。
- 最长 `128` 个 Unicode scalar；超过上限返回稳定 validation error。
- 匹配展示名称、作者和 category labels；category labels 包括导入 metadata tags 和用户分类展示名。
- 空搜索词等价于不搜索。
- 前端输入 debounce 建议为 `250ms`，但 Enter 可以立即查询。
- 前端必须用 request sequence 只接收最新响应，避免慢请求覆盖新搜索结果。

### Filter

首版 filter 与现有页面一致：

```text
all
status(statusCode)
category(categoryId)
```

- category filter 使用稳定 `categoryId`，不能继续用分类名称比较。
- status filter 基于已经合并的当前 profile 状态。
- 无 active profile 时仍可查询 Mod 库；profile-dependent 状态按既有 fail-closed 规则呈现，不能由前端猜测已安装。
- 未知 filter kind、status code 或不存在的 category id 返回稳定错误，不能回退到 `all` 后悄悄展示错误结果。

## 概念契约

`query_mod_library` 已在 Slice 2 作为正式 typed contract 落地，并同步 `docs/FRONTEND_BACKEND_CONTRACT.md`；下面是当前契约摘要。

无参 `get_mod_library()` 继续作为兼容 contract 保留，不能把它静默改造成带 query 的同名命令。Mod 管理页面在 Slice 3 使用独立分页命令，避免旧调用方在迁移期间产生输入/输出歧义。

```text
query_mod_library(input) -> ModLibraryPageDto
```

当前输入：

```text
ModLibraryQueryDto
  profileContext?:
    gameId
    profileId
  search
  filter
    kind: all | status | category
    status?: stable status code
    categoryId?: stable category id
  sort: name_asc
  page                 # 1-based
  pageSize             # 12 | 24 | 48 | 96
```

当前响应：

```text
ModLibraryPageDto
  items: ModLibraryItemDto[]
  page                 # 后端 clamp 后的实际页
  pageSize
  libraryTotal         # 未应用 search/filter 的库总数
  matchingTotal        # 应用 search/filter 后的总数
```

约束：

- `items.len() <= pageSize`。
- `libraryTotal == 0` 表示 Mod 库为空；`libraryTotal > 0 && matchingTotal == 0` 表示没有匹配结果。
- `matchingTotal == 0` 时响应 `page = 1`、`items = []`。
- 请求页超过最后一页时，后端返回最后一个有效页，而不是返回无法解释的空页。
- `hasPrevious`、`hasNext` 和总页数由前端基于实际 `page/pageSize/matchingTotal` 计算，不在 DTO 重复存储。
- DTO 不新增 archive、sandbox、cache、manifest、game path 或其他文件系统字段。
- command 是短只读查询，不创建 task、不发送 event、不获取 game/profile 写锁。
- 错误使用稳定 code；用户可见 message 不参与前端逻辑。

`get_mod_library()` 在迁移期可以保留给尚未迁移的调用方，但 Mod 管理页面切换后不得同时调用新旧列表 API。所有正式消费者迁移完成后，再单独评估移除旧 command，不能在同一切片中无审计删除。

## UI 与交互

### 布局

分页器属于 Mod 库工具表面，不放入卡片，也不做浮动卡片：

```text
Mod 库
  固定工具栏 / 快捷操作
  可滚动的当前页卡片区
  独立分页 footer

悬浮反馈层（不占分页网格行）
  安装计划 Detail Sheet / 任务 Notice / 终态 Toast
```

Slice 3 已为 `.mod-library` 增加独立 pagination row。常规桌面窗口中，分页 footer 位于
`.mod-library__content` 之外并保持在页面网格内，翻页时不随卡片内层滚动；它不得遮挡自绘滚动条、
返回顶部按钮或卡片内容。

`max-width: 1280px` 且 `max-height: 720px` 的短高桌面窗口是明确例外：为了避免搜索、筛选和安全
操作区把卡片轨道压缩成不可用的细条，Mod 页面启用外层纵向滚动并为卡片区保留至少 `460px`
内容轨道。分页 footer 仍位于卡片内层滚动区之外，但需要通过页面外层滚动到达，不做 viewport
固定或覆盖式悬浮。该取舍优先保证卡片可检查、无重叠和无横向溢出。

分页 footer 包含：

- 当前范围，例如 `25-48 / 286`。
- page size 使用项目自绘 listbox，不使用原生 `<select>`。
- 首页、上一页、数字页、下一页、末页按钮。
- 当前页使用 `aria-current="page"`。

首页/上一页/下一页/末页使用 lucide 的标准 chevron 图标、accessible name 和项目自绘 tooltip。数字页使用紧凑数字按钮；页数较多时最多展示 7 个页码位置，并用不可点击省略号表示间隔。

### 状态变化规则

| 操作 | 页码 | 选择 | 滚动位置 |
|---|---|---|---|
| 修改搜索词 | 重置到 1 | 清空 | 回到顶部 |
| 修改 filter | 重置到 1 | 清空 | 回到顶部 |
| 修改 page size | 重置到 1 | 清空 | 回到顶部 |
| 切换视图模式 | 保持 | 保持 | 保持 |
| 点击其他页 | 切换目标页 | 清空 | 回到顶部 |
| 刷新当前页 | 后端 clamp | 重新校验并默认清空 | 保持或在 clamp 时回顶部 |
| 安装/卸载完成 | 重新查询当前条件 | 清空 | 统一回到顶部 |

翻页回到 `.mod-library__content` 顶部时使用即时滚动，避免先播放跨整页平滑动画。视图切换的既有 transition 可以保留，但翻页不应对 24-96 张卡片启动长时间 stagger 动画。

### Loading、empty 和 error

- 初次加载显示当前视图对应的有限 skeleton，不渲染完整 mock 库。
- 搜索/翻页刷新时保留上一个成功页面，设置 `aria-busy` 并显示轻量 loading 状态，避免内容闪空。
- 请求失败时保留上一个成功页面，显示稳定错误文案和重试入口。
- `libraryTotal == 0` 显示“尚未导入 Mod”。
- `libraryTotal > 0 && matchingTotal == 0` 显示“没有匹配的 Mod”，并保留清除搜索/filter 的入口。
- stale response 不能覆盖较新的查询状态。

### 响应式与可访问性

- 分页 footer 使用稳定 grid/flex 约束；空间不足时范围摘要和 page size 换行，页码控件不得溢出。
- 至少覆盖 `1440x900`、`1366x768`、`1280x800` 和项目最小 `960x640` 窗口。
- `max-width: 1280px` 且 `max-height: 720px` 的短高窗口允许页面外层滚动，但必须保留至少
  `460px` 的可用卡片轨道、首屏内容提示和 footer 可达性，且不能出现横向溢出或内容覆盖。
- 所有 icon-only 按钮有 `aria-label` 和 tooltip。
- disabled 首页/上一页/下一页/末页仍保留稳定尺寸，不引起 footer 位移。
- Tab 顺序按 page size、首页、上一页、页码、下一页、末页排列。
- 页面切换完成后把焦点留在触发按钮或分页导航，不强制跳到卡片首项；使用状态区域播报新范围。

## 选择语义

T18 首版选择严格限定为当前页：

- “全选”改为“选择本页”。
- “反选”改为“反选本页”。
- 翻页、搜索、filter 或 page size 改变时清空选择。
- 当前页选择计数与 `matchingTotal` 分开展示，不能写成含糊的“已选 N / 共 M”让玩家误以为选中了所有页。
- install/uninstall 仍遵守当前单项动作约束。

跨页累积选择、“选择全部 N 个匹配结果”和跨页批量执行属于 T13。届时应设计显式 selected ids 或后端 selection token，并在执行前重新验证；不能复用 T18 的 page-local Set 假装支持全库批量操作。

## 架构边界

| 模块 | 规划职责 | 禁止职责 |
|---|---|---|
| `hmm-app` | `ModLibraryQuery`、查询顺序、overlay/category/status 合并、filter/sort/page、响应计数 | Tauri DTO、React 状态、具体 SQLite/JSON API |
| `hmm-ports` | 仅在性能切片需要时定义可查询 read repository | UI 页码组件或具体 SQL |
| `hmm-infra` | 当前 JSON 兼容读取；后续可查询 SQLite read model/索引实现 | profile 状态和 UI 选择语义 |
| `hmm-tauri` | query DTO 校验、app service 调用、DTO 映射、稳定错误 | 搜索、过滤、排序或 page clamp 规则 |
| React `features/mods` | query state、debounce、stale-response 防护、分页 footer、loading/error、page-local selection | 重新实现后端搜索/状态/分类规则 |

查询模型是 application read use case，不需要为了分页把 UI 概念放进 `hmm-core`。只有多个用例真正共享稳定领域语义时，才评估向 core 下沉。

Slice 3 按 feature-local 边界拆出查询、分页、反馈和 durable overlay 模块，包括：

```text
useModLibraryQuery.ts
modLibraryPaginationModel.ts
ModLibraryPagination.tsx
ModLibraryPagination.css
ModLibraryQueryFeedback.tsx
modLibraryRecoveryRefresh.ts
```

不要继续把分页、请求竞态和选择重置逻辑全部堆入已经较大的 `ModLibraryPage.tsx` / `ModLibraryPage.css`。

## 持久化与性能演进

当前导入结果位于 JSON 单文件，metadata overlay 和用户分类位于 SQLite。首个查询服务可以读取三者并在内存聚合，但每次翻页仍会解析完整 JSON 和加载完整 overlay/category 映射。

T17 批量迁移和 T18 分页应共享同一份持久化基准与迁移决策：

- 不为两个任务分别建立互相竞争的 Mod read model。
- 优先评估把只读导入快照迁入 SQLite，或建立可重建的 SQLite query projection。
- projection 不是安装事实；导入快照/provenance 和现有 manifest 边界保持不变。
- SQLite count 与 page items 查询应在同一短 read transaction/snapshot 内完成。
- 为规范化名称、分类关系和常用 filter 建立经基准证明必要的索引。
- 不在 read transaction 中执行文件扫描、hash、预览图处理或 install recovery 写操作。

在以下条件满足前，T18 不能宣称“大型 Mod 库性能完成”：

- page response 不返回超过 page size 的条目。
- 前端不加载完整 Mod DTO 列表。
- install/recovery 状态查询不再由前端提交全库 Mod ID。
- 使用 1,000 和 10,000 条人工记录报告查询、序列化和翻页基准。
- 若仍保留每页完整 JSON 解析，必须有明确上限、基准和后续 migration，不得把 bridge payload 变小等同于后端已扩展。

## 与 T13 / T17 的关系

- T18 不依赖 T17，现有 Mod 库也可以先实现分页。
- T17 的候选预览/结果分页是批次工作流，不能替代 T18 的主库分页。
- T18 应在 T17 Slice 4 完整迁移 UI 对外完成前落地，避免批量导入立即制造难以操作的主列表。
- T18 首版只做 page-local selection；T13 才负责跨页批量选择和写任务。
- T13 实施时消费 T18 的 query/filter contract，但不能让前端把“全部匹配结果”展开成无上限 ID 列表后再提交。

## 分阶段实施计划

### Slice 1：查询语义与 app service

**实施状态：已完成（PR #186）。**

- 定义 app-level query/filter/sort/page/result 类型和稳定 validation error。
- 使用现有 repositories 实现兼容版聚合查询，严格遵守 merge/filter/sort/page 顺序。
- 把 profile install/recovery 状态合并移入查询 use case。
- 用 fake repositories 覆盖搜索、分类 ID、状态、稳定排序、页边界和 clamp。
- 不修改前端，不移除 `get_mod_library`。

### Slice 2：Tauri DTO 与 typed API

**实施状态：已完成（PR #187）。**

- 新增窄 `query_mod_library` command 和 camelCase DTO。
- 校验 search 长度、filter、page 和 page size allowlist。
- 新增 feature-local typed API wrapper。
- 更新 `docs/FRONTEND_BACKEND_CONTRACT.md`，注册 command、DTO 和稳定错误。
- source/serialization 测试确认不暴露路径、sandbox/cache、manifest 或第三方内容。

### Slice 3：分页 UI 与 page-local selection

**实施状态：功能实现、本地统一验证、完整视觉 smoke 及独立复审已完成，待 PR/merge。**

- 增加独立数字 Pagination 组件、helper 和局部 CSS。
- 接入 250ms debounce、latest-request gate、loading/error/empty 和 page clamp。
- page footer 放在内层滚动区之外，并与自绘滚动条/返回顶部按钮兼容。
- 把“全选/反选”改成明确的本页语义。
- 当前页只加载 manifest/recovery durable overlay；安装/卸载等终态动作后只对单 Mod durable 状态做受控探测，再重查当前 query。
- 经典、增强网格、列表、机能四种视图和 `1440x900`、`1366x768`、`1280x800`、`960x640` 四个窗口均已完成视觉验收；分页 footer 无重叠、无横向溢出，短高窗口无顶部状态栏穿透。
- 不实现跨页批量选择。

### Slice 4：可查询 read model 与性能门禁

**实施状态：尚未开始。**

- 与 T17 Slice 1 共用持久化基准和 migration 决策。
- 消除或明确限制每次翻页的完整 JSON 解析和全量 merge。
- 增加人工大库 fixture、查询/序列化基准和必要索引。
- 建立大库查询、序列化、翻页和渲染性能门禁，并在 read model 迁移后复跑交互回归。
- 只有本切片通过，TODO 才能把 T18 标记为完成。

## 测试策略

所有自动化使用人工生成的 Mod metadata、临时 JSON/SQLite 和 fake status services，不读取真实 Mod、游戏目录、玩家存档或本地路径。

### 查询与契约

- 0、1、11、12、13、23、24、25、95、96、97 条边界。
- page 为 0、负数/反序列化失败、超出末页和极大整数。
- allowlist page size 与非法值。
- search trim/空白折叠、Unicode normalization、名称/作者/标签匹配和 128 字符上限。
- category 使用 id 而不是重名 label。
- profile status 合并后再过滤和分页。
- 同名条目按 `modId` tie-breaker 稳定分页，无重复或漏项。
- libraryTotal、matchingTotal、page 和 items 始终一致。
- DTO/错误不包含路径、cache、sandbox、manifest 或第三方内容。

### 前端状态

- search/filter/page size 变化重置 page 和 selection。
- view mode 变化保持 page、selection 和 page size。
- 翻页清空选择并把内层滚动区复位。
- 较早请求晚返回时不会覆盖最新结果。
- 初始加载、刷新中、空库、无匹配、错误保留和重试。
- 页码窗口、省略号、首页/末页和 disabled 状态。
- `aria-current`、accessible names、状态播报和键盘导航。
- install/uninstall 后 filter membership 变化时重新查询并 clamp。

### 性能与视觉

- 1,000 / 10,000 条人工记录的 app query 与 Tauri serialization benchmark。
- 当前页最多渲染 96 个卡片节点。
- `1440x900`、`1366x768`、`1280x800`、`960x640` 下 footer 不重叠、不横向溢出。
- 四种 view mode 使用同一分页状态，切换时 footer 尺寸稳定。
- custom scrollbar、back-to-top 和 pagination footer 互不遮挡。

### 验证入口

实现切片按边界运行聚焦前端、Rust/Tauri 和 contract 测试；每个 PR 最终运行：

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify.ps1
```

Slice 3 运行时代码和 UI 已通过本地统一验证、四视图/四窗口的完整浏览器视觉 smoke 及独立复审；当前仍待 PR/merge，Slice 4 和 T18 整体不得提前标记完成。

## 验收标准

- Mod 管理页有明确、可访问、响应式的数字分页 footer。
- 默认每页 24，可选 12/24/48/96，非法 page size 被后端拒绝。
- 搜索覆盖名称、作者和标签，分类按 id、状态按当前 profile 事实过滤。
- 查询先 merge/filter/sort，再分页；总数和当前页来自同一快照。
- 稳定排序和 `modId` tie-breaker 保证翻页无重复、无漏项。
- 前端只持有当前页 DTO，不请求全库安装/恢复状态。
- 翻页、搜索、filter 和 page size 的选择/滚动规则符合本文矩阵。
- 首版“选择本页/反选本页”无歧义，不暗示跨页批量能力。
- 空库、无匹配、loading、stale response、错误和刷新状态完整。
- 大库自动化只用人工数据；统一验证通过。
- T17 完整迁移 UI 发布时，主 Mod 库分页已经可用。

## Slice 4 实施前确认项

- 用人工大库 benchmark 确认兼容版 in-memory query 可接受的临时上限。
- 与 T17 共用的导入快照 SQLite migration 或 query projection 路线。
- 当前页 install/recovery durable overlay 如何迁入最终 read model，避免在 Tauri command 或前端重写规则。
- `name_asc` 规范化算法在 Rust 与测试 fixture 中的精确定义。
- Slice 3 保留的 plain-browser 开发 mock 如何继续与生产 Tauri 错误路径隔离；生产初始失败不得展示成真实 Mod。

这些确认项不改变后端权威分页、page-local selection 和默认不实现跨页批量操作的核心决策。
