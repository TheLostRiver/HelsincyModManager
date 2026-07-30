# 批量 Mod 生命周期领域设计

> 状态：T13-00 设计冻结；产品代码尚未实现
>
> 日期：2026-07-30
>
> 范围：Windows + MHW:I 的批量安装、批量卸载和批量真正重装

## 背景

Helsincy Mod Manager 已有经过 Gate A 认证的单项安装、卸载和真正重装链路：

```text
InstallPlan
  -> preflight
  -> backup / snapshot
  -> commit
  -> manifest
  -> rollback / recovery
```

这些链路已经使用 TaskManager、同一 game/profile 写锁、最小 Audit Log、安装 manifest 和 durable
recovery 事实。批量能力必须编排这些单项事务，不能建立第二套文件写入器，也不能由前端、Tauri
command 或 CLI 循环调用单项 command 来冒充服务端批量用例。

T17 第三方管理器批量迁移已经具备 selection snapshot、partial result 和 retry，但其语义是把外部来源
安全物化为 HMM 内部导入记录，默认不安装、不启用，也不写游戏目录。T13 可以借鉴其 sealed selection
形态，不能复用其写入、失败或恢复假设。

本文冻结 T13-00 的领域语义。后续 T13-01 至 T13-08 必须实现并验证本文，不能在 transport 或 UI
层重新解释。

## 目标

- 为批量安装、卸载和真正重装定义同一套 sealed batch identity。
- 在任何写入前发现跨 Mod 最终 target 冲突、资源超限、恢复未收敛和计划过期。
- 保证每个 Mod 是独立事务，不宣称整个批次全局原子。
- 默认在首次阻断或失败后停止，显式 continue 只越过可隔离的 item failure。
- 保留已成功项的真实成功事实，不因后项失败、取消或日志故障伪造回滚。
- 让取消只停止启动新项；已经进入不可抢占 commit 的单项完成一致性收尾。
- 让 retry 只消费原 sealed batch 中服务端判定为 retryable 的项，成功项不重放。
- 为未来 app、CLI、Tauri、前端、Task Log 和 Audit Log 提供稳定状态和 code。
- 所有自动化使用 temp/fake/人工 fixture，不读取真实游戏、Steam userdata、Mod 或存档。

## 非目标

- T13-00 不新增 Rust 类型、repository、command、DTO、CLI 参数或前端页面。
- 不提供跨 Mod 全局 rollback，也不把整个批次描述成数据库式原子事务。
- 不允许 last-wins、执行顺序覆盖或全局 `force` 解决最终 target 冲突。
- 不改变单项 manifest、backup、rollback 或 recovery 的事实来源。
- 不把 batch journal、Task Log、Audit Log 或 UI state 当成玩家文件事实。
- 不开放 Production CLI 写能力；跨进程 admission 未完成前仍保持 parser 不可达。
- 不支持任意 game root、target path、backup ref、manifest path 或 recovery ref 输入。
- 不在本轮加入 Linux / Steam Deck、多游戏适配或真实 Windows 安装态验收。

## 术语

| 术语 | 含义 |
| --- | --- |
| Batch request | 用户仍可编辑的有界批量意图，不具备写权限 |
| Preview token | 绑定一次只读 preview snapshot 的短期 opaque 校验值 |
| Sealed batch | 已规范化、不可修改并持久化的批量意图 |
| Batch plan | 对 sealed batch 在某个受控状态快照上生成的只读计划 |
| Batch digest | Batch plan 规范序列化后的确定性身份摘要 |
| Plan token | 绑定 batch、digest、attempt、环境和有效期的 opaque 执行授权 |
| Batch attempt | 对同一 sealed batch 的一次初始执行或服务端 retry |
| Item | 一个 Mod 的一次 install、uninstall 或 reinstall 操作 |
| Item write truth | 单项 commit、manifest、rollback 和 recovery 共同证明的真实终态 |
| Batch control state | 调度、取消、停止和汇总状态，不替代 item write truth |
| Global blocker | 即使使用 continue 策略也必须停止整个批次的安全问题 |
| Item blocker | 只影响一个 item，且能够证明不影响其他 item 的前置问题 |

## 硬性不变量

1. 一个 batch 只包含一种 operation、一个 game instance 和一个 profile。
2. 同一 batch 中一个 Mod 最多出现一次；重复项整体拒绝，不静默去重。
3. 每个 item 必须绑定确切 revision 或确切 installed manifest 事实，不能使用“最新版本”。
4. 输入顺序由后端按稳定 item key 规范化；展示名、选择顺序和 locale 不参与语义。
5. 跨 item 的最终 target claim 必须在计划阶段求并集；同一规范 target 被多个 item 写入时整体阻断。
6. 任何真实写入仍走已有单项安全链，batch runner 不直接复制、覆盖、删除或恢复文件。
7. 已成功 item 不因后项失败、取消、batch journal 或 Audit 写入失败而回滚。
8. `continue_on_item_failure` 不能越过 global blocker、recovery required 或不确定写入事实。
9. 取消不能抢占 commit、rollback、manifest 收敛或 recovery；只在安全点生效。
10. retry 的 item 集合由后端从 sealed batch 和已有 terminal results 计算，调用方不能提交任意 ID。
11. result、event、log 和 DTO 不返回完整路径、backup/snapshot ref、manifest 正文、hash 列表或原始错误。
12. preview 完全只读，不写 game、manifest、backup、recovery、Audit 或 query projection。

## 领域模型

### Batch plan request

逻辑模型：

```text
BatchPlanRequest
  schema_version
  operation
  game_id
  profile_id
  execution_policy
  items[]
```

`operation` 的稳定值：

```text
install
uninstall
reinstall
```

`execution_policy` 的稳定值：

```text
stop_on_failure              # 默认
continue_on_item_failure     # 必须由用户显式选择
```

Batch request 是有界输入值，不是持久化写入事实。前端可以编辑本地选择，但后端只认可完整、已验证的
request；request 没有 batch ID、digest、plan token 或写入权限。

### Operation-specific item input

安装：

```text
InstallBatchItemInput
  mod_id
  revision_id
  layer
  replacement_binding_snapshot?
```

卸载：

```text
UninstallBatchItemInput
  mod_id
  expected_installed_revision_id
```

真正重装：

```text
ReinstallBatchItemInput
  mod_id
  installed_revision_id
  candidate_revision_id
  layer
  replacement_binding_snapshot?
```

输入不包含路径、package file id、backup ref、manifest generation、plan token 或 hash。这些事实由后端读取
并封存在 BatchPlan 中。

同一 Mod 的普通 reinstall 和同 revision replacement target switch 都使用 reinstall operation。后者只有
在现有真正重装领域规则证明 binding snapshot 属于同一 Mod/profile/revision 时才允许，通用 batch core
不解析 MHW slot/path。

### Stable item key 与顺序

后端先验证 ID，再按以下逻辑 key 的 UTF-8 字节序规范排序：

```text
operation + "\0" + mod_id
```

一个 batch 不允许同一 `mod_id` 出现两次，因此 candidate revision 或 target choice 不用于制造第二个同
Mod item。`item_id` 是后端签发的短 opaque ID；`ordinal` 是规范排序后的 0-based 序号。

相同语义输入不受前端选择顺序影响，结果页始终按 `ordinal` 稳定排序。

### Preview 与 sealed batch

Preview 是纯只读步骤：

1. 校验 request 的 item 数量、ID 和 operation-specific input。
2. 规范排序并拒绝重复 Mod。
3. 构建 BatchPlan、blocking/warning summary 和内部 batch digest。
4. 若 ready，返回绑定 digest、environment、schema 和短有效期的 opaque preview token。
5. 不创建 batch journal、query projection、Audit 或任何 temp artifact。

用户确认后调用独立 seal 用例，并重新提交同一有界 request 与 preview token。Seal 用例重新读取当前
facts、重建 digest 并验证 token；只有完全一致时，才在短事务中：

1. 签发 `batch_id` 和 item IDs。
2. 保存不可修改的 normalized input 与 BatchPlan snapshot。
3. 创建 attempt 0 journal。
4. 签发 apply plan token。

Sealed batch 不因 preview token 过期、运行、取消或 retry 自动消失。它按 batch journal 保留策略保存，用于结果
查询、恢复和审计对账。后续变更 revision、target choice 或 policy 必须创建新 batch。

### BatchPlan

逻辑模型：

```text
BatchPlan
  plan_schema_version
  operation
  game_id
  profile_id
  execution_policy
  items[]
  global_target_claims_digest
  prerequisite_rules_version
  resource_limits_version
  batch_digest
```

每个 item plan 至少封存：

```text
BatchItemPlan
  item_id
  ordinal
  input_snapshot
  source_revision_fact
  installed_manifest_fact?
  replacement_binding_fact?
  prerequisite_decision
  single_plan_digest
  target_claims[]              # 仅后端内部
  backup_requirements_digest?
  blocking_reasons[]
  warning_codes[]
```

`target_claims` 使用已验证的逻辑 `InstallTargetPath` 和 operation-specific write kind，只存在于领域/应用
内部。公开 projection 只返回聚合数量和稳定 reason code。

## Digest 与 plan token

### Canonical digest

`batch_digest` 使用版本化 canonical bytes 的 SHA-256。Canonical representation 必须：

- 固定字段顺序、enum code 和整数编码。
- items 使用规范 `ordinal` 排序。
- target claims 使用 Windows 目标规范化后的稳定顺序。
- blocking/warning codes 排序并去重。
- 包含 operation、game/profile、execution policy、精确 revision、binding、单项计划摘要、preflight
  决策、跨 item target claim 和资源限制版本。
- 排除 `batch_id`、`item_id`、时间戳、TTL、display text、绝对路径、原始 error 和随机 token。

同一 snapshot 必须生成同一 digest。Batch digest 是身份和缓存键，不是秘密，也不是写 capability。

### Opaque execution token

真正 apply 使用独立 `plan_token`。Token 必须绑定：

```text
batch_id
batch_digest
attempt_number
environment
schema_version
issued_at
expires_at
```

Token 使用后端保存的 opaque random record 或经过审计的 keyed token，不能把裸 digest 当授权。
Production 和 Sandbox token 不能互用。

Preview token 和 plan token 默认 30 分钟过期。Preview token 过期后必须重新执行纯只读 preview；
plan token 过期只使该 execution plan 不可执行，不删除 sealed batch 或历史结果。Retry 会基于同一
sealed input 生成新的 attempt 和 token；调用方不能延长旧 token。

## 计划构建

### 两阶段读取

计划构建在 game/profile 写锁外执行：

1. 读取 exact revision、binding、manifest/recovery 和 prerequisite facts。
2. 为每个 item 调用已有单项 preview/planner。
3. 把所有 operation-specific write target 规范化并求并集。
4. 计算 global blocker、item blocker、warnings 和资源预算。
5. 生成只读 preview、digest 和可选 token。

Apply 获取写 admission 和既有 game/profile 写锁后，只做有界 revalidation，不在锁内重新执行长时间
scan/hash/extract/analyze。

### Cross-item target conflict

跨 item target key 使用 Windows 语义规范化：

- 路径分隔符统一。
- 大小写不敏感比较。
- 拒绝 `.`、`..`、绝对路径、盘符、控制字符和不安全保留名。
- replacement target 必须先由 game adapter 转换为合法最终 `InstallTargetPath`。

如果两个 item 的 write set 包含同一 key，返回 `batch_global_target_conflict`。首版不考虑 provider
priority、执行顺序或“最后一个覆盖”；整个 batch 没有 token，continue 策略也不能绕过。

卸载的 remove/restore target 和重装的 retained/replaced/added/stale write set 同样参加求并集。无法
证明 target 不重叠时 fail closed。

### Preflight 分类

Global blocker 包括：

- batch token/digest/schema/environment 不匹配。
- duplicate item、资源超限或 canonicalization 失败。
- 跨 item target conflict。
- game/profile 配置或 prerequisite rule catalog 不可用。
- game 正在运行或运行状态未知，且单项写侧要求阻断。
- game/profile 存在 active install/reinstall recovery 或 recovery repository 不可用。
- batch journal 在任何写入前不可持久化。
- 写 admission 不可用。
- apply 前 batch plan stale。

Item blocker 包括能够证明只影响一个 Mod 的问题，例如：

- exact source/candidate revision 不存在或未 ready。
- install item 已安全安装，或 uninstall item 未安装。
- 该 item 的 manifest/target/backup 事实不满足单项操作要求。
- 该 item 缺少 required prerequisite。
- 与 batch 外现有 Mod 的 target ownership 冲突。

warning 不会自动变成 success，也不被 frontend 文案解释。它使用稳定 code，并随 plan token 一起绑定。

## 资源上限

T13-01 的首版 hard limits：

| 资源 | 上限 |
| --- | ---: |
| 单个 batch item 数 | 100 |
| 全部 item 的最终 target action 总数 | 50,000 |
| canonical internal plan 大小 | 16 MiB |
| 结果分页默认值 | 50 |
| 结果分页最大值 | 100 |
| preview token 有效期 | 30 分钟 |
| plan token 有效期 | 30 分钟 |

上限集中在版本化 `BatchResourceLimits`，不能散落在 frontend、Tauri 或 CLI。超过任一 hard limit 整体返回
`batch_resource_limit_exceeded`，不截断、不部分 seal，也不生成 token。

## 执行策略

### `stop_on_failure`

这是默认策略：

- Preview 中存在任何 global blocker 或 item blocker 时，整个 plan 为 blocked，任何写入前停止。
- Apply 会在第一项写入前重新验证所有 batch-global facts 和全部 item 的可执行性。
- 任一 item 在运行时 blocked、failed 或 recovery required 后，不再启动新 item。
- 已成功 item 保留；尚未启动 item 记录为 `skipped`，reason 为 `stopped_after_item_failure`。
- 不对已成功 item 做补偿性全局 rollback。

“首次阻断停止”不表示只检查到第一个问题。Preview 可以返回有界聚合 reason，执行判断仍然是零写入。

### `continue_on_item_failure`

该策略必须由用户显式选择并进入 digest/Audit：

- 仍然先通过全部 global blocker 检查。
- Preview 中的 isolated item blocker 记录为 `blocked`，其他 ready item 可以执行。
- Item 在写入前失败，或写入失败且单项 rollback 已证明成功时，可以继续下一项。
- 每项开始前重新验证其 source/manifest/target/preflight facts。
- 后续 item 不得依赖失败 item 的成功结果；跨 target overlap 已在计划阶段整体阻断。

以下情况即使选择 continue 也必须停止：

- `rollback_required`、`repair_required`、`unknown` 或任何不确定玩家文件事实。
- manifest/recovery 持久化结果不确定。
- batch journal 在 commit 后无法记录结果。
- Audit evidence 在 commit 后降级为不可用。
- task observer/result channel 无法继续提供可追踪 identity。
- 新出现的 global conflict、admission failure、game state blocker 或取消请求。

Continue 是失败策略，不是安全 override。

## Item 与 batch 状态

### Item terminal status

| Status | 含义 |
| --- | --- |
| `succeeded` | 单项 commit、manifest 和必要 cleanup 已证明完成 |
| `blocked` | 写入前的 item-specific 前置不满足 |
| `failed` | 单项未写入，或写入失败且 rollback 已证明恢复 |
| `recovery_required` | 写入/manifest/recovery 事实未安全收敛 |
| `cancelled` | 在 commit 前安全取消，该 item 没有写入 |
| `skipped` | 因 stop policy 或 batch-global stop 未启动 |

`retryable` 是独立布尔事实，不是 status：

| Item 终态 | 默认 retryable |
| --- | --- |
| `succeeded` | false |
| `blocked` | 仅稳定 reason 明确允许时为 true |
| `failed` | 仅未写入或 rollback succeeded 且错误可重试时为 true |
| `recovery_required` | false，必须先完成受控 recovery |
| `cancelled` | true |
| `skipped` | 原 sealed input 仍有效时为 true |

### Commit 期间取消

现有单项任务允许 commit 期间记录取消请求，但 commit 继续完成。Batch 层必须按文件事实分类：

- commit/manifest 成功：item 是 `succeeded`，不是 `cancelled`。
- commit 失败且 rollback 成功：item 是 `failed`。
- commit 或 rollback 未收敛：item 是 `recovery_required`。
- 当前 item 收尾后不再启动新 item。

TaskManager 的 Cancelled 状态不能覆盖成功 manifest/Audit 事实。

### Batch terminal status

| Status | 含义 |
| --- | --- |
| `completed` | 全部 item succeeded |
| `completed_with_errors` | 存在安全终态的 blocked/failed/skipped，但没有 recovery required |
| `blocked` | 任何 item 写入前被 global/default policy 阻断 |
| `cancelled` | 取消阻止批次完成；可能同时存在已成功 item |
| `recovery_required` | 至少一个 item 的玩家文件事实未收敛 |
| `failed` | batch orchestration/journal/admission 失败且不能给出正常完成状态 |

`recovery_required` 优先于其他汇总状态。Batch status 始终伴随各 item count，不能把 `cancelled` 或
`completed_with_errors` 解释为“没有文件被修改”。

## 执行顺序

每个 attempt 按规范 `ordinal` 确定性执行：

```text
validate token and sealed batch
  -> acquire batch journal intent
  -> revalidate all global facts before first write
  -> for item in canonical order
       -> check cancellation
       -> prepare/revalidate outside write lock where possible
       -> acquire existing game/profile write lock
       -> revalidate compact item facts
       -> run existing single-item transaction
       -> release write lock
       -> persist terminal item result
       -> apply stop/continue policy
  -> persist batch terminal summary
```

不能持有 game/profile 写锁执行整个 batch。单项之间释放不需要的 lock、DB transaction、file handle 和
staging resource。不同 game/profile 的其他任务可以按现有资源预算并行；同一 game/profile 的实际写入
仍由同一 coordination 串行。

如果另一受控任务在两个 item 之间改变状态，下一 item revalidation 必须返回 stale/blocked，而不是依赖
旧 preview 继续写入。Batch 不承诺跨 item 隔离。

## Plan stale 与 expected changes

Batch apply 在第一项写入前检查：

- game/profile identity 与配置摘要。
- exact source/candidate revisions 与 binding snapshots。
- 相关 manifest entry sets 和 install/reinstall recovery。
- 相关 target size/hash summaries 与 backup availability。
- prerequisite decision/rule version。
- batch digest、policy、limits 和 environment。

前一 item 的合法成功会改变 manifest，不能让全部后项仅因全局 generation 增长而过期。因此：

- 每个 item digest 只绑定自己的相关 manifest/source/target/preflight facts。
- Batch digest 绑定跨 item target claims 和规范顺序。
- 后项重新读取自己的相关事实；非重叠前项的预期变化不算 stale。
- 如果前项变化实际影响后项 target/manifest/preflight，后项返回 `batch_item_plan_stale`。

第一项写入前发现 stale：整个 batch `blocked`，零写入。中途发现 item stale：按 stop/continue 处理；
已经成功项不回滚。

## Retry

Retry 不修改原 sealed batch，而是创建新的 `BatchAttempt`：

```text
BatchAttempt
  batch_id
  attempt_number
  parent_attempt?
  eligible_item_ids[]       # 仅后端计算
  attempt_plan_digest
  plan_token
```

规则：

1. 只选择最近 terminal result 中 `retryable = true` 的 item。
2. `succeeded` 永不重放。
3. 调用方只提交 `batch_id` 和必要的 optimistic revision，不提交 item IDs。
4. Retry 继续使用原 exact revision、policy、binding 和 operation input。
5. 如果用户希望换 revision、target 或 policy，必须创建新 batch。
6. Retry 前重新生成计划和短期 token；过期或 stale 不自动写入。
7. 原失败 item 若已进入 recovery required，必须先由受控 recovery 收敛，不能直接 retry。
8. `skipped` 和 commit 前 `cancelled` 可以 retry，但仍要通过当前 preflight。

如果 retry item set 为空，返回 `batch_retry_unavailable`，不创建空 task。

## 取消

| 取消时机 | 行为 |
| --- | --- |
| queued 前/后、尚未开始 | 全部未开始 item cancelled，零写入 |
| plan/prepare | 协作式停止，清理可丢弃 staging，零写入 |
| item 之间 | 不启动下一项，保留已成功结果 |
| commit/manifest | 记录请求，不抢占；当前 item 安全收尾后停止 |
| rollback/recovery | 不抢占安全收敛，完成后停止 |

一个 batch 对外只有一个可见 `task_id`。内部 item 不是无上限 UI 子任务；详细结果通过有界分页 query
读取。CLI-2A 落地后，batch task 必须恰好一个 terminal event，sequence 单调递增。

## 崩溃与恢复

Batch journal 只记录编排，不取代 manifest/recovery：

- 每项开始前先持久化 `running` intent。
- 单项终态后再持久化 item result。
- Commit 后 journal 写失败不能回滚成功文件；必须停止调度新项。
- 应用重启不会自动继续破坏性写入。

启动恢复流程：

1. 把遗留 `running` attempt 标记为 `interrupted`。
2. 只对当时的 running item 读取 operation-specific manifest/recovery 事实。
3. 如果 completed manifest 和 cleanup 都能精确证明，收敛为 `succeeded`。
4. 如果存在 `committed_cleanup_pending`、`cleanup_pending`、`rollback_required`、`repair_required` 或
   `unknown`，收敛为 `recovery_required`。
5. 如果能证明未进入写入且没有 recovery，收敛为 retryable `failed`。
6. 不能证明时 fail closed，不自动删除、恢复或重试。
7. 其余未开始 item 保持 `skipped`，由用户显式 retry。

完成受控 recovery 后，用户从原 sealed batch 发起新的 retry attempt。不会自动恢复整个 batch，也不会
回滚已经成功的其他 Mod。

## Operation-specific 语义

### Batch install

输入绑定 exact imported revision、layer 和可选 replacement binding snapshot。

Preview 必须证明：

- revision 属于该 Mod 且 source ready。
- 当前 Mod 不处于 installed/unsafe recovery 状态。
- 单项 InstallPlan 无 blocking conflict。
- batch 内外最终 target ownership 安全。
- required prerequisite satisfied；warning 保留。

执行复用单项 install transaction。成功以 completed manifest 中 exact revision、binding 和 installed file
summary 为准。

### Batch uninstall

输入只指定 Mod 和 expected installed revision；删除/恢复集合完全来自受控 manifest 和 backup 事实，
不读取当前 package 猜测。

Preview 必须证明：

- exact installed revision 与 manifest entry set 一致。
- 每个 target 的 installed file summary 仍匹配。
- 需要的 original backup 存在且可读。
- 没有 active install/reinstall recovery。
- 多个 uninstall item 的 remove/restore target 不重叠。

目标缺失、摘要变化或 backup 不可用只会阻断，不提供 force delete/ignore hash。

### Batch true reinstall

输入绑定 installed revision、candidate revision、layer 和可选 binding snapshot。

Preview 复用现有真正重装 retained/replaced/added/stale 分类和 token facts，并额外检查跨 item target
union。执行继续使用 durable reinstall transaction、original backup 继承、candidate manifest commit 和
post-commit reconciliation。

同 revision retarget 只有在现有 `is_same_revision_replacement_target_switch` 及 binding ownership 规则
证明安全时可进入计划。通用 batch 只消费 snapshot，不解析 armor/weapon path 或 target catalog。

## Journal、manifest 与 projection

建议持久化职责：

| 数据 | 事实边界 |
| --- | --- |
| Revision catalog | 可安装 source/revision 事实 |
| Install manifest | 已提交游戏目录事实 |
| Install/reinstall recovery | 未收敛写入事实 |
| Batch journal | sealed intent、attempt、item result 和调度状态 |
| Query projection | 可重建分页查询，不是写入事实 |
| Task/Audit Log | 过程和审计证据，不是 manifest |

Batch journal 使用短事务。Result projection 更新失败时标记 dirty 并重建，不能回滚 manifest 或把 stale
projection 当权威结果。

## Task、Audit 与隐私

未来 batch task 的稳定 phase family：

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

`<operation>` 只能是 `install`、`uninstall` 或 `reinstall`，让 progress consumer 不依赖页面内存推断
动作；phase 仍不包含 Mod ID、路径或自由文本。

Batch-level Audit 只允许：

```text
task_id
batch_id
operation
execution_policy
attempt_number
item_count
succeeded_count
blocked_count
failed_count
cancelled_count
skipped_count
recovery_required_count
result
error_code?
```

Per-item 继续写已有单项 Audit，并可追加 `batch_id`、`batch_item_id` 和 `attempt_number`。不得记录 item
target list、plan token、digest、backup/snapshot ref、manifest/source 正文、完整路径、Steam ID 或第三方
Mod 内容。

Audit 在 commit 后失败不能改变 item success，但 batch 必须记录证据健康降级并停止启动新破坏性 item。
Task observer/result channel 失败同样不能伪造 rollback；当前 item 收尾后停止。

## Planned transport contract

T13-06 前以下只是规划，不是可调用 command：

```text
preview_batch_mod_lifecycle(request)
seal_batch_mod_lifecycle(request, previewToken)
start_batch_mod_lifecycle(batchId, planToken)
get_batch_mod_lifecycle_result(batchId, cursor?, limit?)
retry_batch_mod_lifecycle(batchId)
cancel_task(taskId)
```

Preview request 只包含 operation、gameId、profileId、executionPolicy 和有界 item inputs。Response 只返回：

- status、operation 和 policy。
- item/global reason 聚合计数。
- action/retained/replaced/added/stale 等聚合计数。
- ready/blocked item 数量。
- opaque previewToken 或 `null`。

Seal response 只返回 batchId、status、operation、policy、expiresAt 和 opaque planToken；seal 前后任一
fact 变化都返回 `batch_plan_stale`，不会持久化部分 snapshot。

`previewToken` 和 `planToken` 只允许出现在各自的直接 command response 中，调用方仅在内存持有，
不得持久化、记录或转发到 result/progress/event/log/Audit/diagnostics。CLI adapter 应在单次受控流程
内部消费 token，不得写入 stdout、JSON/JSONL 或 shell history。

Result query 分页返回 itemId、ordinal、短 modId、status、reasonCode、retryable 和聚合计数。它不返回
target path、hash、backup/ref、manifest、source/package 或原始 error。

CLI-4 只能映射相同 app use case。Sandbox 写能力和单项 lifecycle CLI E2E 未通过前，batch parser
保持不可达；Production 还必须等待独立跨进程 admission。

## Stable code 规划

Batch-level：

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
batch_result_unavailable
batch_cancelled
batch_internal_error
```

Item-level 优先复用单项现有 blocking/error code。Batch 新增的调度 reason：

```text
stopped_after_item_failure
cancelled_before_start
batch_item_plan_stale
source_revision_changed
manifest_changed
target_changed
rollback_succeeded
recovery_required
```

对外 error 不序列化内部 Rust `Display`/`Debug`。前端按 code 映射本地化文案。

## 测试矩阵

### T13-01 BatchPlan

- 不同 UI 选择顺序生成相同规范顺序和 digest。
- operation/policy/revision/binding/target/preflight 任一变化会改变 digest。
- batchId、时间戳、display text 不影响 digest。
- duplicate Mod、101 items、50,001 actions 和超过 16 MiB plan 整体拒绝且零写入。
- Windows 大小写/分隔符等价 target 跨 item conflict 整体阻断。
- preview 不写 game、manifest、backup、recovery、Audit、DB projection 或 temp artifact。
- previewToken/planToken 过期、环境不匹配和 digest 不匹配 fail closed。

### T13-02 Batch install

- 全成功按 ordinal 提交，每项 manifest 和 Audit 正确。
- 默认策略任一预检 blocker 时整个 batch 零写入。
- 首项成功、第二项失败时保留首项，后项 skipped。
- Continue 只越过 pre-write/rollback-succeeded failure。
- Manifest save、rollback、journal 和 Audit 故障分别覆盖 before/after commit。
- 外部 sentinel 在所有成功/失败/cancel 场景保持不变。

### T13-03 Batch uninstall

- 只按 manifest/installed summary/backup 执行，不读取 package 猜测。
- target missing/changed、backup missing/read failure 阻断。
- 多 item remove/restore target overlap 整体阻断。
- 中途失败保留已完成卸载事实，不恢复其他已成功 item。
- Crash/restart 能区分 succeeded、retryable failed 和 recovery required。

### T13-04 Batch reinstall

- retained/replaced/added/stale 聚合与单项计划一致。
- candidate/installed revision、binding、target 或 original backup 变化使 item stale。
- Manifest failure -> rollback previous revision；rollback 不完整 -> recovery required 并停止。
- 同 revision retarget 复用既有 snapshot/recovery，不泄漏 MHW path 到通用 core。

### Cancellation、retry 与 concurrency

- queued、prepare、item 间、commit 中和 rollback/recovery 中取消。
- Commit 中取消且 commit 成功时 item=succeeded，后项不启动。
- Retry 只选择 retryable item；成功项和 recovery-required 项不重放。
- 改 revision/target/policy 不能通过 retry，必须新 batch。
- 同 game/profile 写入严格串行；plan/scan 在写锁外；item 间释放写锁。
- 不同 game/profile 在资源预算允许时可并行。
- 一个 batch task 恰好一个 terminal event，result page 不依赖 event item 明细。

### Privacy 与 contract

- 除 preview/seal 对应的直接 response 返回各自 opaque token 外，result/progress/event/其他 DTO、
  CLI JSON/JSONL、Task/Audit/diagnostics 不含完整路径、Steam ID、token、digest、backup/snapshot
  ref、manifest/source 正文、hash 列表或原始 error；token 不落盘、不写日志。
- Result pagination 默认 50、最大 100，非法 cursor/limit 整体拒绝。
- Stable status/code serialization 有快照/contract tests。
- Production CLI 写命令在 admission 前 parser 不可达。

## 实施依赖

```text
T13-00 领域语义
  -> CLI-2A 流式 task observer
  -> CLI-2B Sandbox write capability / containment
  -> CLI-2C 单项 lifecycle CLI E2E
  -> CORE-PREF-01 单项 preflight 一致化
  -> T13-01 Sealed BatchPlan
  -> T13-02 Batch install
  -> T13-03 Batch uninstall
  -> T13-04 Batch true reinstall / retarget
  -> T13-05 CLI contract
  -> T13-06 Tauri / typed API
  -> T13-07 UI
  -> T13-08 Windows Sandbox Gate C
```

任何后续切片发现本文与已认证单项事实冲突时，必须先更新设计并独立 review，不能在实现中静默偏离。

## T13-00 完成定义

- 本文明确 sealed snapshot、digest、token expiry、cross-item conflict 和资源上限。
- 本文把纯只读 preview 与持久化 seal 分离，避免 plan 预览产生隐式写入。
- 本文明确 stop/continue、partial、cancel、retry 和 crash recovery。
- 安装、卸载和真正重装分别定义 exact input、preflight、write truth 和 recovery。
- 公开 contract 只预留稳定短 ID、状态、code 和聚合计数，不描述未实现 command 为可用。
- `docs/FRONTEND_BACKEND_CONTRACT.md`、`docs/TESTING.md`、CLI 设计、TODO 和路线图同步。
- 文档检查、完整统一验证和 findings-first 自审通过后，T13-00 才可进入 PR 合并门禁。
