# 第三方 Mod 管理器批量迁移设计（狩技盒子兼容）

> 状态：Slice 1–4C 已完成(Slice 1、Slice 2“只读来源扫描与分页预览”、Slice 3“安全物化与批量导入编排”、Slice 4A“外部来源与只读预览”、PR #198 的 Slice 4B“选择、决定与批量启动”、PR #199 的 Slice 4C“结果、重试与最终加固”);Slice 5“跨批次导入记录与保留期”进行中(见同名章节)。
>
> 本文定义产品、架构、安全和验收边界并记录分 Slice 状态；实际可用性仍以对应 PR 与验证证据为准。

## 背景与规划审计结论

Helsincy Mod Manager 已有单个压缩包的安全导入链路，也规划了 Mod 批量安装/卸载，但当前没有“从第三方 Mod 管理器一次性迁移整个 Mod 库”的等价规划。

现有批量操作和本设计解决的是不同问题：

- 批量安装/卸载处理 HMM 已导入 Mod 的游戏目录写入。
- 第三方管理器批量迁移处理外部来源的只读扫描、预览和导入。
- 迁移完成后，安装或启用仍是玩家后续发起的独立动作。

首个兼容来源采用狩技盒子常见的目录布局。已核对的参考行为是：玩家选择一个根目录，工具扫描数字命名的直接子目录，每个候选包含 `files/` 和 `info.xml`，再尽量保留名称、作者、类型和版本。HMM 只兼容这一可观察的数据契约，不复制参考工具的直接目录复制、覆盖或安装策略。

本设计只借鉴可独立观察的产品流程与输入格式。后续实现必须独立完成，不复制任何第三方源码、素材、UI、文案或项目内部标识。为准确描述互操作范围，正式文档、来源 adapter、提交说明和产品界面可以使用玩家识别兼容来源所必需的公开来源名称；不得出现仅用于研究参考的第三方实现项目名称，也不得暗示授权、隶属、背书或代码来源关系。

本文所说的“无缝衔接”是一次性迁移体验：一次选择、统一预览、批量导入、结果汇总和失败重试。它不表示两个管理器之间实时同步，也不表示继承外部管理器的启用、安装或冲突事实。

批次候选预览和结果分页只服务本迁移工作流，不等于主 Mod 库分页。批量迁移完整 UI 对外完成前，应先落地 [Mod 库分页设计](MOD_LIBRARY_PAGINATION_DESIGN.md)，避免导入大量 Mod 后主列表难以操作。

## 目标

- 玩家只选择一次来源目录，即可发现全部兼容候选。
- 在任何写入前展示候选、阻断原因、重复项、同名冲突、元数据和预计工作量。
- 玩家可以全选、排除候选，并为可处理的冲突做显式决定。
- 每个候选仍复用 HMM 的安全导入、sandbox、分析和持久化能力。
- 批次支持取消、部分成功、失败重试和幂等重放。
- 尽量保留外部元数据，同时不让外部元数据覆盖 HMM 的用户编辑层或安装事实。
- 后续可以增加其他第三方来源 adapter，而不把来源格式写进前端或通用导入核心。

## 非目标

首版不做：

- 自动安装、自动启用或自动写入游戏目录。
- 导入外部管理器的启用状态、优先级、文件覆盖栈或安装清单。
- 读取外部管理器数据库、注册表、网络账号或下载凭据。
- 监视来源目录、实时同步或双向同步。
- 删除、移动、重命名或修复外部来源文件。
- 以外部状态替代 HMM 的 `InstallPlan`、`InstallManifest`、backup 或 rollback。
- 静默覆盖 HMM 中已有的导入记录。
- 同时兼容任意未知目录布局；其他来源必须另加 adapter 和 fixtures。
- 在本设计阶段新增源码、依赖、Tauri command 或前端 UI。

## 首个来源契约

第一版 adapter id 建议使用稳定、版本化的 `hunting_box_directory_v1`。名称是实现建议，最终公开 code 一旦发布就不得改变语义。

只扫描玩家所选根目录的直接子目录：

```text
selected-root/
  1001/
    files/
      nativePC/
      ...
    info.xml
  1002/
    files/
      ...
    info.xml
```

候选规则：

- 目录名必须只包含 ASCII 数字；不递归寻找更深层的序号目录。
- `files` 必须是普通目录，`info.xml` 必须是普通文件。
- 非数字目录、结构缺失项和不支持的链接项要进入预览结果，不能静默消失。
- 序号只作为来源内的候选标识，不作为 HMM `modId`。
- adapter 不假设 `files/` 中一定存在 `nativePC`；内容语义仍交给现有包分析器和游戏 adapter。

Slice 5 起的兼容超集(不 bump adapter id,契约只放宽不收紧):

- **注册层有效库根解析**:玩家常选狩技盒子安装根而不是 `Mods_582010`。注册时一次性、
  确定性地解析:所选目录已有直接数字子目录 → 以所选目录为准;没有数字子目录但存在常规的
  `Mods_582010` 子目录(ASCII 忽略大小写,链接/重解析点不下潜)→ 下潜一层;其余情况
  (含探测超出有界枚举上限)原样注册。fingerprint 按有效根计算——选安装根与直接选
  `Mods_582010` 得到同一 identity,scanner/materializer 契约与 `numeric-directory:{name}`
  候选身份零改动。`582010`(MHW Steam AppId)常量归 adapter 所有。
- **缺载荷防御**:编号目录只有 `info.xml`、`files/` 确实不存在时(狩技盒子「无操作」安装
  方式的残留),候选归为专用 `payload_missing`(不可选择),仍解析 `info.xml` 让预览带上
  mod 名;`files/` 是链接/重解析点/非目录时仍按 `structure_invalid` 拒绝,不得降级。

### 元数据映射

`info.xml` 只提供不可信的元数据提示。第一版映射如下：

| HMM 候选字段 | 外部字段 | 回退与规则 |
|---|---|---|
| 建议名称 | `moduleName`，其次 `name` | 两者都无效时显示脱敏的候选占位名，不把序号当最终身份 |
| 建议作者 | `author` | 空值忽略 |
| 建议版本 | `version` | 空值忽略，只作为显示文本 |
| 来源类型 | `modType` | 进入待映射来源标签，不自动创建 HMM 分类 |

解析规则：

- 禁止 DTD、外部实体和网络读取。
- XML 文件大小、节点深度、文本长度和字段数量必须有集中配置的上限。
- 所有文本 trim 后再校验；控制字符、超长文本和无效编码返回稳定 reason code。
- XML 损坏时，候选可以显示为 `metadata_invalid`，但只有文件内容本身通过安全扫描后，玩家才能选择忽略元数据继续导入。
- 外部元数据只补齐新导入快照中的缺失字段，不覆盖包分析器得到的更可信信息，也不覆盖已有 Mod 的用户 metadata overlay。
- `modType` 必须在预览中映射到已有 HMM 分类或选择“不分配”；首版不静默创建分类。

## 用户工作流

1. 玩家在 Mod 管理页选择“从第三方管理器导入”。
2. 玩家选择“狩技盒子目录”来源，由原生目录选择器选择根目录。
3. 后端登记一个短生命周期、不可猜测的 `sourceId`；前端不接收完整本地路径。
4. 玩家启动扫描任务，立即得到 `taskId` 和 `batchId`。
5. 后端只读发现候选、解析受限元数据、计算大小和内容指纹，并持续发送聚合进度。
6. 前端按页查询批次预览，展示可导入、重复、同名、阻断和结构不完整项。
7. 前端创建后端 selection snapshot；分页选择通过每次不超过 200 项的 mutation 更新，“选择全部”由后端按当前查询执行，不把全部候选 ID 展开到前端。
8. 玩家完成候选、分类映射和允许的冲突决定后，前端用 `selectionId + expectedRevision` 启动批量导入；后端原子校验并封存该选择快照。
9. 后端逐项重新校验来源，将快照中的候选物化为受控的内部包，再复用现有单包 prepare/analyze/persist 流程。
10. 前端按页查询结果，展示成功、已存在、跳过、阻断、失败和取消项，并只重试可重试项。
11. 导入完成后刷新 Mod 库；安装和启用仍由现有工作流单独触发。

扫描和导入之间如果来源发生变化，该候选必须返回 `source_changed` 并要求重新扫描，不得继续使用过期预览做决定。

## 架构边界

下面的类型名用于表达职责，实施时可以按仓库现状调整命名；分层边界和安全约束是强制的。

| 模块 | 规划职责 | 禁止职责 |
|---|---|---|
| `hmm-core` | 无路径的批次、候选、状态、冲突决定、provenance 和稳定 reason code | 文件系统、XML、狩技盒子路径规则 |
| `hmm-ports` | 来源扫描、候选物化、批次仓储等 trait | 具体目录遍历或数据库实现 |
| `hmm-app` | 扫描预览、选择校验、批量编排、取消、幂等和结果汇总 | 直接调用真实文件系统或解析第三方 XML |
| `hmm-infra` | 狩技盒子目录 adapter、受限 XML 解析、内容指纹、内部包物化和批次仓储 | 游戏安装规则或 UI 决策 |
| `hmm-games-mhw` | 对物化后的包执行既有 MHW:I 内容分析 | 识别第三方管理器目录或数据库 |
| `hmm-tauri` | 参数校验、DTO 映射、启动/查询用例 | 目录扫描、去重、分类映射或导入规则 |
| React 前端 | 来源选择、预览、选择、进度和结果展示 | 拼接路径、解析 XML、计算 hash 或决定安全阻断 |

外部管理器是 import source adapter，不是 game adapter，也不是安装 backend。

## 概念模型

```text
ExternalImportSource
  source_id                 # 不可猜测、短生命周期
  adapter_id                # 例如 hunting_box_directory_v1
  display_label             # 不含完整路径
  expires_at

ExternalImportBatch
  batch_id
  adapter_id
  source_fingerprint        # 本机密钥化摘要，不可直接枚举路径字典
  scan_status
  import_status
  created_at

ExternalImportSelection
  selection_id              # 对前端公开的 opaque id
  batch_id
  revision                  # optimistic concurrency / stale-write guard
  status                    # editing | sealed | expired
  selected_count
  expires_at

ExternalImportSelectionEntry
  selection_id
  candidate_id              # 存在即表示选中，只引用同 batch 的后端候选
  decision?                 # 稳定 enum + 已有 category id
  updated_at

ExternalImportCandidate
  candidate_id              # 对前端公开的 opaque id
  source_item_key_hash      # 不保存原始路径
  content_fingerprint
  metadata_hint
  file_count
  total_bytes
  preview_status
  conflict_kind

ExternalImportItemResult
  candidate_id
  status
  reason_code
  imported_mod_id?
  retryable

ImportProvenance
  adapter_id
  batch_id
  source_item_key_hash
  content_fingerprint
  imported_at
```

不得在对前端 DTO、progress event、Task Log 或 Audit Log 中放入来源根路径、候选相对路径、sandbox/cache 路径、XML 原文或第三方 Mod 内容。

`source_fingerprint` 不能是本地路径的普通 SHA-256。它应使用仅存于本机 app data 的随机 secret key，对 adapter id 与规范化来源 identity 做 HMAC 或经审计的 keyed digest；该值只用于玩家重新选择来源后的批次对账，不向前端、日志或诊断包公开。密钥丢失时要求重新扫描，不允许为了恢复匹配而降级持久化明文路径。

## Adapter 与安全物化

来源 adapter 分成两个端口，避免扫描预览和实际导入隐式耦合：

- scanner：只读枚举候选、解析受限 metadata、统计资源并计算规范化内容指纹。
- materializer：仅对玩家选中的候选重新校验，并在 app data 的 task-scoped 目录生成内部规范化包。

规范化内容指纹至少覆盖：

- 规范化后的相对路径序列。
- 每个普通文件的字节长度和内容 hash。
- 大小写不敏感路径键，避免 Windows 碰撞。

指纹不包含绝对路径、mtime 或来源根目录，因此移动整个来源目录不会制造不同内容身份。mtime 和文件 identity 可以作为扫描到执行之间的快速变化探针，但不能替代内容校验。

materializer 必须：

- 使用 `symlink_metadata` 等不跟随链接的检查，拒绝 symlink、junction、reparse point 和其他特殊文件。
- 对每个路径段做相对路径、保留名、空段、尾随点/空格、Unicode 和大小写碰撞校验。
- 使用与单包导入一致或更严格的文件数量、单文件大小、总大小和目录深度预算。
- 在读取过程中检查取消，并在写入前后复核大小与内容指纹。
- 只写 task-scoped 临时目录；失败或取消后清理未完成物化结果。
- 生成完成后把内部包交给现有 archive inspect、sandbox extract、analyze 和 result persistence 链路。

不得把 `files/` 直接复制到 HMM Mod 库或游戏目录。内部规范化包只是导入输入，不是安装事实。

## 预览状态与冲突策略

预览状态使用稳定 code，用户可读文案由前端映射。建议至少覆盖：

- `ready`
- `already_imported`
- `duplicate_in_batch`
- `name_collision`
- `structure_invalid`
- `metadata_invalid`
- `unsupported_entry`
- `resource_limit_exceeded`
- `source_unreadable`

默认策略：

| 情况 | 默认行为 | 允许的显式决定 |
|---|---|---|
| 与 HMM 已有条目内容指纹相同 | 跳过并关联已有 `modId` | 跳过；首版不重复导入 |
| 同批次候选内容相同 | 只保留第一项 | 选择其中一项 |
| 名称相同、内容不同 | 阻止静默覆盖 | `keep_both` 或跳过 |
| 来源类型无法映射 | 不分配分类 | 映射到一个已有分类 |
| XML 损坏、文件内容安全 | 默认不选 | 明确忽略 metadata 后继续 |
| 路径、链接或资源预算不安全 | 阻断 | 不允许 override |

`keep_both` 的新 `modId` 由后端分配，不能由前端通过改名或路径拼接生成。替换已有只读导入快照不属于首版；未来若需要，必须单独设计对现有安装、Profile 和 metadata overlay 的影响。

## 批次、幂等与恢复

- `batchId + candidateId` 是批次条目的幂等键。
- 内容指纹是跨批次去重依据；显示名称不是身份。
- 批次重试由后端从同一 sealed selection 中只调度 `retryable = true` 的条目；前端不重新选择或提交 candidate ID。
- 成功、已存在和显式跳过项不会因重试重复创建。
- 如果导入结果已持久化、批次日志尚未来得及标记成功，恢复时按 provenance/content fingerprint 对账并补记结果。
- 应用启动时只把前一进程遗留的 `running` batch 收敛为 `failed`，保留 sealed selection 与已有结果；不会自动恢复来源 I/O 或重新导入。玩家重新选择匹配来源后再显式 retry，既有 catalog/provenance 继续参与幂等对账。
- 取消后保留已成功项；未开始项标为 cancelled，运行中项在安全检查点停止并清理临时目录。
- 单项失败不回滚其他已成功的只读导入；批次最终状态可以是 `completed_with_errors`。
- 批次级仓储故障、来源整体失效或无法保证结果持久化时，应停止调度新项。
- source root 默认不持久化；应用重启后要继续重试，玩家必须重新选择来源，由后端用 source fingerprint 和批次记录完成对账。

批量选择必须存为后端权威 snapshot，不能把分页结果重新展开成一次无上限的 Tauri 请求：

- `selectionId` 由后端签发并绑定唯一 `batchId`；selection entry 只保存后端候选 ID 和已校验决定，不保存来源路径。
- 单次 selection mutation 最多包含 `200` 个候选变更；超过上限整体拒绝，不截断、不部分应用。
- 单个批次最多选择 `10,000` 个候选；同时继续受集中配置的批次总文件数、总字节数和物化预算约束。mutation、服务端全选和启动都会校验这些上限；任一上限超出都整体拒绝且保持旧 snapshot/revision。
- “选择全部”由后端针对 batch preview 的稳定 query/filter 执行，不接受前端展开的全量 ID；blocked 项和仍缺少显式冲突决定的项不会被静默选中。
- 每次 mutation 使用 `expectedRevision` 做 compare-and-swap，并返回新 revision、选中计数和聚合预算；revision 不匹配返回稳定冲突错误。
- 启动导入时后端在短事务中复核 batch/source revision、selection revision、总量上限和全部决定，再把 snapshot 从 `editing` 原子切换到 `sealed`。sealed snapshot 不再修改，重试继续引用同一不可变选择事实。
- selection snapshot 与 batch journal 一起通过短事务持久化；应用重启后仍能恢复 revision 和 sealed 事实，但来源失效时仍须玩家重新选择并完成 source fingerprint 对账。
- `expiresAt` 只限制仍处于 `editing` 的选择会话；snapshot 一旦 sealed，就按 batch journal 生命周期保留，不能在运行、取消或重试期间因编辑 TTL 到期而消失。

当前 `ModImportResultRepository` 仍是 JSON 单文件。T18 Slice 4A 已决定保留 JSON revision catalog 作为 logical Mod/revision/import provenance 的权威来源，同时在既有 SQLite 中建立可重建 query projection。T17 不再另选一套竞争的 Mod read model：

- 为权威 JSON 仓储增加有界、分块 `upsert_many` 和原子替换，每个 chunk 最多完整写一次，禁止每个候选都重写全文件。
- T18/T17 共用 SQLite query projection；projection 只保存可重建查询列、规范化 key、分类关系和 profile status 派生值，不成为导入或安装事实。
- JSON chunk 成功而 projection 更新失败时标记 projection dirty，由 rebuild 对账；不能回滚已确认的导入事实，也不能返回 stale projection。

批次 journal/selection 继续使用 SQLite 短事务；它们只记录编排、selection 和 provenance 引用，不能取代 revision catalog、query projection generation 或安装 manifest。

## 任务与并发

扫描和导入都必须是带 `taskId` 的长任务，并复用 `cancel_task(taskId)`：

- 一个用户动作只创建一个可见批次任务，不为每个候选向 UI 暴露无上限的子任务。
- 内部 scan/hash/materialize/analyze 使用有界 worker 数和统一 IO/CPU 预算。
- progress event 只携带批次聚合计数、阶段和 `taskId`；候选明细通过分页 query 获取。
- scan、hash、inspect、extract 和 analyze 可以受控并行。
- 仓储写入使用短事务或分块原子写，不能让长任务持有数据库写事务。
- 整个流程不获取 game/profile 写锁，因为它不写游戏目录也不改变启用状态。
- scanner 必须在保留 root 直接子项前应用 `max_total_candidates` 预算，默认最多 `10,000` 项；超过时停止
  枚举，并只持久化一个无路径的 `resource_limit_exceeded` 预览项，不能把大量空目录收集进内存或 SQLite。
- queued task 的进度事件若无法发射，command 必须把刚创建的 task 与 batch 收敛为失败，不能留下前端无法取得
  identity 的 `pending` 记录。
- 若未来提供“导入后安装”，必须作为独立任务进入现有 game/profile 串行写队列，不能塞进本批次。

建议 phase code：

```text
external_import.scan.discovering
external_import.scan.fingerprinting
external_import.scan.completed
external_import.scan.failed
external_import.import.materializing
external_import.import.preparing
external_import.import.persisting
external_import.import.completed
external_import.import.failed
```

phase code 一经写入前后端契约就必须保持稳定。部分成功的明细由结果 query 表达，不用 failed event 覆盖已完成事实。

## Tauri 与前端契约规划

当前实现的窄命令如下：

```text
select_external_import_source()
start_external_import_scan(sourceId)
get_external_import_preview(batchId, selectionId?, cursor?, limit?)
create_external_import_selection(batchId)
update_external_import_selection(selectionId, expectedRevision, entries)
select_all_external_import_candidates(selectionId, expectedRevision)
start_external_import_batch(batchId, selectionId, expectedRevision)
retry_external_import_batch(batchId, selectionId)
get_external_import_batch_result(batchId, cursor?, limit?)
list_external_import_batches(cursor?, limit?)
cancel_task(taskId)
```

已完成的 Slice 2 bridge 包含 `select_external_import_source()`、`start_external_import_scan(sourceId)` 和
基础 preview query；首个 command 固定登记唯一的
`hunting_box_directory_v1` 来源，不接受 adapter/path 参数；原生选择器中的路径只保留在 Rust registry。
已完成的 Slice 3 在保留上述只读契约的前提下增加 selection create/update、服务端全选、sealed batch start/retry 和
分页 result query。Slice 4A 消费 source picker、scan task 和基础 preview；PR #198 的 Slice 4B 将 preview 扩展为
`get_external_import_preview(batchId, selectionId?, cursor?, limit?)`，并消费 selection/decision/start/progress。
当前 Slice 4C 消费脱敏 result page 与 sealed selection retry；后端契约和 Rust projection 无需扩张。

约束：

- 来源选择命令返回 opaque `sourceId` 和不含完整路径的显示标签。
- scan start 返回小型 `{ task, batchId }`；不得把候选数组塞进启动响应或完成事件。
- preview/result query 必须分页；默认 `limit = 50`，最大 `limit = 100`，非法值整体拒绝。
- selection create 返回小型 selection summary；selection-aware preview 只返回同 batch selection summary，以及当前页每个候选的 `selected` 和可选 decision，不得返回全部 selected IDs。selection 不存在或跨 batch 时统一返回 `external_import_selection_unavailable`；只读 preview 可以派生 editing selection 的 `expired` 状态，但不得写回 repository。
- selection update 的 `entries` 形如 `{ candidateId, selected, decision? }`，每次必须包含 `1..=200` 项；只接受同一 batch 的后端 candidate id、稳定 enum 和已有 category id，重复、未知或跨 batch candidate 整体拒绝。`selected = true` 拒绝 blocked 候选；`selected = false` 可以移除同 batch 的已有 entry，且不保留孤立 decision。
- Slice 3 的 `select_all_external_import_candidates` 使用固定的“所有 ready 候选”后端谓词，不接受候选 ID 数组；未来筛选扩展只能追加稳定 query/filter，仍不得把 ID 数组展开到前端。后端在 `10,000` 项总上限和资源预算内更新 selection snapshot。
- `start_external_import_batch` 只接受 `batchId + selectionId + expectedRevision`；不接受 candidate ID 或 decision 数组，并在启动前封存选择快照。
- selection 相关稳定错误至少包括 `selection_revision_conflict`、`selection_empty`、`selection_mutation_empty`、`selection_mutation_limit_exceeded`、`selection_total_limit_exceeded`、`selection_candidate_invalid`、`selection_expired` 和 `selection_closed`。
- DTO 不接受或返回 root/path/archive/sandbox/cache/hash 原文。
- 前端将入口放在 `features/mods/` 的导入工作流内，不新建营销式页面，也不在浏览器侧读取目录。
- Slice 4B 前端覆盖 selection loading/empty/editing/error、分页、CAS 冲突刷新、sealed start、running/completed/failed/cancelled；partial-success 明细、result 和 retry 状态仍由 Slice 4C 覆盖。
- import listener 与 task state 由入口组件持有的持久 workflow hook 管理，Dialog 内 panel 只渲染；关闭 Dialog 不得丢失 `taskId`、early-event buffer 或运行中进度。事件必须同时匹配 `kind = mod_import`、精确 `taskId` 与登记的 `external_import.import.*` phase。

## 安全与隐私

### 文件系统

- 选择来源不等于信任来源；所有条目仍按敌对输入处理。
- 扫描和物化都必须拒绝链接、特殊文件、路径逃逸和大小写碰撞。
- 读取前后做来源变化校验，避免扫描到执行之间的 TOCTOU。
- 来源始终只读；测试必须断言原 fixture 内容、名称和 mtime 未被业务逻辑修改，不把读取可能更新的 atime 当成可保证字段。
- 临时内部包和 sandbox 必须位于 app data 的受控 task scope，并沿用清理策略。

### XML 与文本

- 使用结构化 XML parser，不用正则或字符串截取解析。
- 禁止外部实体、DOCTYPE 和网络解析。
- 元数据进入 UI 前限制长度并拒绝控制字符；渲染时按普通文本处理。

### 日志与诊断

- Task Log 记录 `task_id`、`batch_id`、adapter id、聚合计数、耗时、结果和 reason code。
- 不记录完整本地路径、来源目录名、XML 原文、Mod 文件名列表或第三方内容。
- Audit Log 不把只读扫描伪装成游戏写入审计；实际导入结果持久化可记录内部 `mod_id` 和 content fingerprint 的不可逆摘要。
- 诊断包只允许导出聚合状态和稳定错误分类，不包含内部包、来源 metadata 原文或 preview 内容。

## 分阶段实施计划

每个切片应独立 review 和提交；在前一切片通过门禁前不进入下一切片。

### Slice 1：领域契约、批量写入与人工 fixtures

- 定义无路径的 batch/candidate/selection/provenance/status/reason 模型，以及 200 项 mutation、10,000 项批次选择和资源预算策略。
- 定义 scanner、materializer 和 batch repository ports。
- 对 `ModImportResultRepository` 增加有界 JSON `upsert_many` 基准与原子写入，并复用 T18 SQLite query projection；不迁移或替代 JSON provenance。
- 建立完全人工构造的目录/XML fixtures 和安全拒绝矩阵。
- 不接入 Tauri，不提供 UI，不扫描真实第三方目录。

### Slice 2：只读来源扫描与分页预览

- 实现 `hunting_box_directory_v1` scanner、受限 XML parser 和内容指纹。
- 实现 ephemeral source registry、scan task、取消和 durable batch preview。
- 增加 Tauri scan/query DTO，但保持路径不出后端。
- 不新增 React 迁移页面；以 native picker、窄 Tauri contract 和 bridge tests 完成可验证预览工作流。

### Slice 3：安全物化与批量导入编排

- 实现 candidate revalidation 和 task-scoped 内部包 materializer。
- 逐项复用既有 archive inspect、sandbox extract、analyze 和 persist service。
- 实现去重、`keep_both`、partial success、取消、重试和崩溃对账。
- 证明不会安装、启用或写入游戏目录。

### Slice 4A：外部来源与只读预览（已完成，PR #196；PR #197 补齐 review 遗漏）

- 在既有 Mod 管理导入工作流中完成来源选择、scan task 状态、取消、空/失败/过期来源状态与只读分页预览。
- 前端只消费受控 source/scan/preview DTO，不接收路径、XML、fingerprint、archive/sandbox/cache 引用，也不创建 selection 或启动批量导入。
- 完成 feature-local typed API、taskId scoped listener、browser smoke 与状态文档同步。

### Slice 4B：选择、决定与批量启动（已完成，PR #198）

- 完成候选选择、服务端全选、分类映射、冲突决定、sealed selection 和 batch start/import progress。
- 每次 selection mutation 保持 200 项上限，跨页全选继续由后端谓词执行。
- selection-aware preview 返回服务端权威的 selection summary 与当前页选择/决定；CAS 冲突和服务端全选后重新读取权威首屏，不在浏览器展开候选 ID。
- import listener 严格匹配 `kind + taskId + phase`；通用 `mod_import.cancelled` 只进入非终态 cancelling，等待 external-import 专用 cancelled 终态。failed/cancelled 的聚合计数不用于推断 partial success。

### Slice 4C：结果、重试与最终加固（PR #199）

- task 进入 completed/failed/cancelled 终态后，从 cursor `0` 查询服务端权威 result page；partial success
  只由 result item status 汇总，不能从 progress event 的聚合计数推断。
- 首屏默认 `50` 项、单页最多 `100` 项；后续页只复用 opaque `nextCursor` 并按 `candidateId` 去重，
  总量可到 `10,000`，前端不把全量 ID 展开进 DOM 或 retry 请求。
- result DTO 使用 exact-key allowlist 和稳定 status/reason code；未知字段、路径式 identity、未知 code、
  重复候选或超页 payload 全部 fail closed，不显示底层错误或敏感事实。
- retry 只提交 `batchId + sealed selectionId`，由后端选择全部 retryable 项；返回的新 taskId 复用同一
  listener、early-event buffer、terminal stickiness、cancel 与 race gate。
- 每个 terminal taskId 只有在首屏权威结果通过验证后才 best-effort 刷新一次 Mod 库；刷新失败不改变
  已持久化导入/result 事实，只显示稳定可恢复反馈。
- task progress concern 已从 selection workflow 拆到独立 hook；大批次自动化使用 10,000 条人工脱敏
  result、5 次 warmup、40 次 sample，本机 p95=`3.937 ms`，低于固定 `250 ms` 同机预算。
- 默认继续 import-only，不安装、启用、获取 game/profile 写锁或写游戏目录。

### Slice 5：跨批次导入记录与保留期（进行中）

Slice 1–4C 的批次事实只服务当次工作流:关闭 Dialog 后 batchId 丢失,玩家无法回看「导入了哪些、
成功/失败了哪些」。Slice 5 把已持久化的 SQLite 批次事实开放为跨批次只读查询,并补齐配套体验。
这是对「批次候选预览和结果分页只服务本迁移工作流」既有口径的**显式扩张**,边界如下:

- **只读历史查询** `list_external_import_batches(cursor?, limit?)`:按创建时间倒序(同毫秒按
  `batchId` 升序)分页,默认 `20`、最大 `50`。响应字段是显式白名单——`batchId`、`adapterId`、
  `scanStatus`、`importStatus`、`createdAtUnixMillis`、`candidateCount` 与逐状态结果计数;
  `sourceFingerprint`、`sourceId`、`selectionId`、`sourceItemKeyHash`、`contentFingerprint`
  和任何路径不得出现。DTO 刻意不复用 preview/result 共享的 batch 形状,避免击穿前端
  exact-key 守卫。
- **计数与明细同源**:聚合计数来自 `external_import_item_results` 的派生 `status` 列
  (migration 015,`created_at`/`status` 均为可空派生列,权威事实仍是 JSON,可随时回填重建;
  残留 NULL 走 `result_json` 兜底归类,无法映射的状态整体报错,不静默漏计)。
- **历史只读,不提供从历史重试**:来源注册是短生命周期(30 分钟 TTL),历史批次的来源多半已
  失效;重试必须回到当次工作流重新选择来源目录。历史 UI 用文案说明这一点。
- **保留期**:已执行过导入的批次(`completed`/`completed_with_errors`/`failed`/`cancelled`)
  按数量保留最近 `50` 个,不按时间删除——它们是可追溯事实;只扫描未导入的批次
  (`import_status = pending`)保留最近 `10` 个且不超过 `7` 天——追溯价值最低、体量最大;
  `running` 永不清理。清理在启动期 `recover_interrupted_batches()` 之后执行一次,尽力而为、
  失败不阻断启动;删除只作用于批次行,候选/selection/结果由外键级联清理。历史 UI 必须明示
  保留上限,前端不得假设 `batchId` 长期有效。
- **审计口径不扩张**:历史数据源就是既有批次表,不新增 audit category,只读查询不写审计。
- **中断批次口径**:沿用启动恢复把遗留 `running` 收敛为 `failed` 的既有事实,不新增
  `interrupted` 状态;文案用中性表述。
- **配套体验**(按 PR 切片交付):result 明细补候选 `displayName`(后端 JOIN 候选表,只出
  display name,digest 不流经;前后端 exact-key 守卫同 PR 原子更新);历史视图放在既有导入
  Dialog 内页签 + 工具栏 icon 直达(记录模式打开不拉起原生选择器),不新建页面;入口按钮
  对齐工具栏治理口径(禁用原因 tooltip、状态警示点);Mod 详情面板展示脱敏来源行
  (`adapter` 显示名 + 导入时间,容忍批次已被保留期清理);来源易用性——注册层把「无直接
  数字子目录但含 `Mods_582010` 子目录」的所选目录规范化到该子目录(同一 fingerprint
  identity,materializer 契约不变),缺 `files/` 载荷的编号目录仍解析 `info.xml` 并在预览中
  以明确原因列出。

后续新增其他第三方来源时，只增加 adapter、来源契约文档和 fixtures；不得在现有 adapter 里加入来源名称分支。

## 测试策略

所有自动化只使用临时目录和人工构造的最小 fixture，不提交或读取真实第三方 Mod、真实游戏目录或玩家数据。

### Adapter 与安全测试

- 合法数字目录、多个候选和空来源。
- 非数字目录、嵌套数字目录、缺少 `files`、缺少 `info.xml`。
- `files`/`info.xml` 是 symlink、junction、reparse point 或特殊文件。
- 父级穿越、绝对路径、保留名、尾随点/空格、无效 UTF-8 和大小写碰撞。
- 文件数、单文件大小、总大小、深度和 XML 限制。
- root 直接候选数量超过集中 `max_total_candidates` 预算时，scanner 在有界读取后停止，并返回脱敏资源拒绝项。
- XML 损坏、DOCTYPE、外部实体、超深节点、超长字段和控制字符。
- `moduleName`/`name` 回退及 author/modType/version 映射。
- 扫描后修改、替换或删除来源项时返回 `source_changed`。
- 扫描、失败和取消后来源目录字节不变且无新增文件。

### 批次与持久化测试

- 同内容跨批次去重、批内去重和同名不同内容 `keep_both`。
- 重复提交同一 batch/candidate 不产生重复 Mod。
- 单项失败不影响其他成功项，结果分页计数一致。
- 取消时已成功项保留、运行项清理、未开始项可重试。
- 在导入结果保存后、batch journal 更新前模拟崩溃并完成对账。
- 仓储故障后停止调度新项，不丢失已确认成功事实。
- 大批次不会为每项重写完整 JSON，也不会开启长 SQLite 写事务。

### 契约与前端测试

- 所有 task event 都携带正确 `taskId`，且不承载候选列表。
- DTO 和日志不包含 path/root/archive/sandbox/cache/XML/hash 原文。
- preview/result query 的 cursor 和 limit 校验。
- selection mutation 覆盖 0/1/199/200/201 项、累计 9,999/10,000/10,001 项、重复/未知/跨 batch candidate、blocked 项的选择与移除、stale revision、expired 和 sealed snapshot。
- 服务端“选择全部”不接收 ID 数组，不包含 blocked 或缺少决定的项；超过 10,000 项或资源预算时整体拒绝且 selection 不变。
- start command 只消费 `batchId + selectionId + expectedRevision`，selection 为空、revision 不一致或决定不完整时不创建 task。
- 未知 status/reason code 前端 fail closed，并显示通用可恢复文案。
- 默认冲突策略不会覆盖已有导入。
- 取消、部分成功、过期来源、重试和刷新 Mod 库流程。
- result page exact-key/status/reason/identity/页大小校验、cursor append 去重与 stale request/batch/task
  generation 丢弃。
- retry 请求不包含 candidate ID，返回新 taskId 后复用 progress listener；每个 terminal taskId 的
  Mod 库刷新至多一次，刷新失败不覆盖权威结果。
- 10,000 条人工、无路径 result 固定执行 5 次 warmup 与 40 次 sample，每次只验证最多 100 项的页；
  输出 p95 并保持 `250 ms` 同机预算，不把该 wall-clock 门禁描述为跨机器 SLA。

### Slice 5 历史查询与保留期测试

- migration 015 回填:legacy 行迁移后 `created_at`/`status` 逐行来自 JSON 事实,行数不变、旧列不丢。
- 历史页按创建时间倒序、同毫秒按 `batchId` 升序,分页不重不漏。
- 派生列聚合计数与 `result_json` 权威事实的 Rust 逐行重算完全一致(锁住 SQL 对 serde 字段名的依赖);
  派生列被置 NULL 时计数经兜底路径不变;无法映射的状态整体报错。
- 重试 upsert 把结果在状态桶之间迁移后计数正确。
- 保留期:`running` 永不删除;imported 上限内不删除;超限与过期 scan-only 删除后候选/selection/结果
  无孤儿行。
- history cursor/limit 边界(0/1/20/50/51、非数字、路径样式)在触达仓储前整体拒绝。
- 历史 DTO 序列化断言不含 `sourceFingerprint`/`sourceId`/`selectionId`/digest/路径。

### 验证入口

实施期间按改动切片运行聚焦 Rust、Tauri 和前端测试；每个 PR 最终运行：

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify.ps1
```

真实狩技盒子目录只能作为用户主动执行的脱敏人工 smoke，不进入自动化、仓库 fixture、日志或诊断包。

## 验收标准

- 一次选择可以得到完整、分页、可解释的候选预览。
- 分页选择由后端 snapshot 承载；单次 mutation 和批次总选择均有硬上限，启动请求不携带 O(N) candidate IDs/decisions。
- 非数字和结构异常项可见且带稳定 reason code，不被静默忽略。
- 默认只导入，整个任务不获取 game/profile 写锁，也不触碰游戏目录。
- 外部来源在成功、失败和取消路径都保持只读。
- 每个选中项进入现有安全导入链路，没有直接复制到 Mod 库的旁路。
- 内容重复默认不创建副本；同名不同内容绝不静默覆盖。
- 任务可取消，部分成功可查询、可重试且重放幂等。
- 元数据按映射规则保留，不覆盖用户 overlay，不把外部状态当安装事实。
- DTO、事件、日志和诊断包不泄露完整路径或第三方内容。
- 自动化只使用人工 fixture，统一验证脚本通过。

## 实施前确认项

下面问题必须在 Slice 1 用人工、脱敏 fixture 和基准数据定稿，不能靠读取真实玩家目录临时猜测：

- 兼容范围是否只接受根级数字目录，还是存在必须支持的合法层级变体。
- 集中资源上限应复用单包导入默认值，还是为整个批次增加独立总预算。
- `ModImportResultRepository` 的 JSON `upsert_many` chunk 大小、失败重试和 projection dirty/rebuild 对账预算。
- 外部 `modType` 到 HMM 分类的默认映射表是否需要随 adapter 版本发布。
- 在最多 10,000 个选中候选的硬上限内，定稿首版性能验收的候选数量、总文件数和总字节预算；基准只能收紧上限，放宽必须另行 review。

### Slice 1 定稿（2026-07-22）

- 兼容范围保持根级数字直接子目录；本切片的人工 fixture 不推断更多真实目录变体，也不实现 scanner。
- selection 使用独立集中预算：最多 `10,000` 个候选、`1,000,000` 个文件、`64 GiB` 源字节和 `64 GiB`
  物化字节。单项物化预算为最多 `16,384` 个文件、`1 GiB` 单文件、`4 GiB` 总字节和 `64` 层目录深度；
  放宽任何值必须单独 review。
- Slice 2 的只读 scanner 复用该 `10,000` 项上限作为 `max_total_candidates`：它限制来源根目录的直接项枚举，
  不是前端分页限制。超限时不保留任意未完整扫描的部分列表，只返回一个不可选择的资源拒绝预览项。
- 权威 JSON `upsert_many` 固定为最多 `10,000` 项、每 `200` 项一个原子替换。前块成功、后块失败时前块保持
  durable，调用方做 exact retry；既有 projection tracking 在权威写入前标记 dirty，写后标记失败则使查询
  fail closed，rebuild 从 JSON 权威事实对账。
- 首版基准使用 `10,000` 条完全人工、无路径的候选/导入记录和 `50` 个 chunk；记录 warmup/sample 的同机
  基线，但不把 wall-clock 作为跨机器 SLA。
- 外部 `modType` 只保留为不可信 metadata hint，不在 Slice 1 创建默认分类映射；映射和来源 UI 留到后续
  adapter/UI Slice。
- Slice 1 仅交付领域契约、ports、JSON provenance/有界写入协调和临时 fixture 矩阵；不建立 SQLite batch
  journal、source registry、真实目录扫描、XML parser、materializer、Tauri command 或游戏目录写入。

这些确认项只影响实现细节，不改变本文的只读来源、显式预览、复用安全导入链路和默认不安装边界。
