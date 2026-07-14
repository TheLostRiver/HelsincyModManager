# Core Mod Lifecycle CL3 真正重装设计

- 日期：2026-07-14
- 状态：`planned`；本文只固定未来实现契约，不表示 command、migration 或 UI 已落地
- 上游：[核心 Mod 生命周期优先级计划](../../CORE_MOD_LIFECYCLE_PRIORITY_PLAN.md)
- 验收基线：[Core Mod Lifecycle CL0/CL1/CL2 验收基线](../../CORE_MOD_LIFECYCLE_CL0_ACCEPTANCE.md)
- 实施计划：[CL3 真正重装实施计划](../plans/2026-07-14-core-mod-lifecycle-cl3-true-reinstall-implementation.md)

## 1. 背景

当前安装、卸载、backup、manifest、rollback/recovery、共享写锁和任务事件已经存在，但 UI 的
“重装”仍直接调用普通 `start_install_task`。当前 importer 还把 import task id 同时用作 package id
和 mod id；第二次导入会生成另一张 Mod 卡，无法表达“同一个 Mod 的新 package revision”。

普通 install 的 manifest merge 按本次写入 target 替换条目，不会删除新计划中消失的旧 target，
也没有保存“重装前版本”的 transaction recovery facts。因此它不能安全完成 `v1 -> v2`：

- `retained` target 需要保留磁盘内容但切换安装 revision 事实；
- `replaced` target 需要写入 v2，同时保留未来卸载所需的游戏原始长期 backup；
- `added` target 需要按普通安装规则新增或覆盖；
- `stale` target 需要删除工具新增文件，或恢复安装 v1 前的游戏原始文件；
- 任一步失败都必须回到重装前的 v1，而不是直接回到游戏原始基线。

CL3 先固定身份、计划、恢复、持久化、任务和 UI contract，再按独立小提交实现。本文不进入 CL4
Gate A 认证，也不提前实现 ARMOR_RETARGET、P7.2c、分页、批量迁移或批量操作。

## 2. 目标

1. 为一张稳定 logical Mod 卡保存多个不可变 package revision。
2. 从“旧 manifest entry set + candidate InstallPlan + 当前目标摘要 + backup”构建唯一 ReinstallPlan。
3. 在任何游戏目录 mutation 前完成全量 source、manifest、target、backup 和 ownership preflight。
4. 在同一 `gameId/profileId` 写锁内短时 revalidate、backup、commit、manifest 和 rollback。
5. 成功时一次原子替换指定 Mod 的 manifest entry set；失败时恢复重装前版本。
6. 让重启后的已安装 revision、恢复状态和 UI 动作都来自持久化事实，而不是 task 内存。
7. 只向前端、task event 和 Audit Log 暴露受控 id、计数、phase/result/error code。
8. 使用人工 v1/v2、temp AppData/game root 与 fake ports 覆盖 L1/L2/L3，不接触真实玩家数据。

## 3. 非目标

- 不在 CL3 中删除旧 revision、旧 sandbox 或设计通用 GC。
- 不按展示名、作者、版本文本、metadata 或 archive 文件名自动合并 Mod。
- 不支持跨 Mod 的同 target layered 重写；CL3 对其他 Mod ownership fail closed。
- 不把 repair、依赖平台、通用 transformer 或 ARMOR target switch 塞进重装用例。
- 不允许前端提交 game root、source/sandbox path、target/delete list、backup ref 或 manifest 正文。
- 不把 TaskManager、Task Log 或 staging 当成安装事实来源。
- 不在本文档提交中实现 Rust、Tauri、TypeScript、migration、fixture 或依赖变更。

## 4. 术语与权威事实

| 术语 | 含义 | 权威来源 |
| --- | --- | --- |
| logical Mod | 玩家库中的稳定 Mod 身份；metadata/category 等用户关系绑定该 id | revision catalog 的 logical Mod record |
| package revision | 一次不可变导入结果；拥有独立 revision id、package id、source/sandbox ref 和分析结果 | revision catalog |
| installed revision | 当前 profile 中真正写入游戏目录的 revision | completed manifest entry set |
| display revision | Mod 卡当前展示的 revision，可与 installed revision 不同 | revision catalog |
| candidate revision | 用户本次选择用于 preview/reinstall 的 ready revision | 请求 id + revision catalog 校验 |
| original backup | 安装任一 revision 前的游戏原始文件，用于最终 uninstall | manifest `backup_ref` |
| transaction snapshot | 本次重装前的 v1 target bytes，只用于失败回到 v1 | reinstall recovery transaction |
| prepared reinstall | 锁外完成 source preload、分类和初步 preflight 的内部 app 对象 | 进程内 app service；不是持久化事实 |
| plan token | 绑定一次 preview 输入事实的短 opaque token | 后端生成；只能做 stale-preview guard |

安装成功状态必须能由“manifest + revision catalog”重建。两者不是跨文件原子事务：catalog 在
reinstall 前已经保存不可变 candidate；reinstall 的 commit point 只有 manifest 原子替换。成功后
不需要再写一个 catalog `currentRevisionId` 才能成立。

## 5. 不变量

以下条件是实现和 review 的硬门禁：

1. 原始导入包只读；source/sandbox 不因 reinstall 被改写。
2. candidate 必须存在、属于请求的 logical Mod、状态为 ready，且与 installed revision 不同。
3. 指定 Mod 的旧 manifest entry set 必须非空、状态可信、revision 可唯一解析。
4. ReinstallPlan 必须完整分类旧/新 target union；不能存在“未分类但继续写入”的 target。
5. 任一 source、manifest、target、backup、ownership 或 plan conflict 不确定时，游戏目录写入为零，
   manifest 保持不变。
6. 同一 target 不能被其他 logical Mod 共同拥有；发现时返回稳定阻断原因，不猜测 layer winner。
7. 进入第一次 target mutation 前，全部必需的 mutating-target transaction snapshot 与 durable
   recovery record 已落盘。
8. `retained` 不写游戏目录；`replaced` 继承 original backup；`stale` 按 original backup 语义清理。
9. manifest 只在全部 target mutation 成功后保存一次；只替换指定 Mod 的 entry set。
10. 同步失败 rollback 的目标是 pre-reinstall v1；旧 manifest 在成功 commit 前一直有效。
11. rollback 未完整成功时不能删除仍需使用的 transaction snapshot，不能报告 completed。
12. install、uninstall、reinstall 和 controlled recovery 共享同一 `gameId/profileId` 写锁 registry。
13. prepare/source preload 不持有游戏写锁；commit 开始后不做抢占式 cancellation。
14. task event、DTO、App/Task/Audit Log 不包含真实 path、backup ref、manifest/source 正文或 hash 列表。

## 6. 稳定 Mod 身份与 revision catalog

### 6.1 Catalog v2

当前 JSON schema v1 是 `{ version, records[] }`，并按 `mod_id` 覆盖记录。CL3 将同一个原子 JSON
文件演进为概念上的 schema v2：

```text
ModRevisionCatalogV2
  version = 2
  mods[]
    mod_id
    origin_revision_id
    display_revision_id
  revisions[]
    revision_id
    mod_id
    package_id
    import_task_id (provenance only)
    source/sandbox opaque facts
    immutable analysis + metadata summary
```

具体 JSON 字段由 Task 2 的 serialization tests 固定，但必须满足：

- `mod_id`、`revision_id`、`package_id` 和 `task_id` 是不同语义类型，代码不得依赖文本相等。
- 一个 revision 只属于一个 logical Mod；保存后不可改绑到另一个 Mod。
- `origin_revision_id` 在 logical Mod 创建/迁移时固定，后续 candidate import 不改变；它只提供明确的
  创建 provenance 和 legacy manifest 解析锚点，不表示当前 installed/display revision。
- 普通 import 创建 logical Mod 与首个 revision。
- revision import 必须显式提交已有 `modId`；后端先验证 Mod 存在，再附加新 revision。
- metadata/category/dependency overlay 继续绑定 logical `mod_id`，不随 revision 漂移。
- library list 每个 logical Mod 只返回一张卡；revision list 通过独立 query 提供。
- installed revision 从 manifest 查询，不能由 `display_revision_id` 或“最新导入”推断。
- v1 迁移为“一条旧 record -> 一个 logical Mod + 一个 origin revision”；迁移保存旧
  `mod_id/package_id -> origin_revision_id` 的明确 provenance。兼容迁移可暂时复用旧 package id 作为
  revision id 的文本值，但实现仍使用不同类型。
- migration 在同一受控文件中 temp write、sync、rename；失败时旧 v1 文件仍可读，不能产生半迁移。
- CL3 不删除旧 revision/sandbox。即使 UI 标为 superseded，其 source availability 仍由后端验证。

### 6.2 Candidate import

未来实现新增窄入口 `start_import_mod_revision_task` 或等价 discriminated request。它复用现有 archive
picker、sandbox extraction、path safety、analysis、task id 和 cancellation 规则，只改变成功保存语义：

```text
archive + existing modId
  -> safe import prepare
  -> new immutable revision/package
  -> atomic append to that logical Mod
```

普通 `start_import_mod_task` 继续创建新 Mod。后端绝不因 display name、metadata version 或文件名相似
自动合并。revision import 失败或取消时，不写 catalog revision，并按现有规则清理本次 sandbox。

## 7. Manifest v2 installed revision 事实

`InstallManifestEntry` 需要新增受控 `revision_id`。为兼容已有 schema v1：

- loader 必须能读取没有 `revision_id` 的 legacy entry；
- 同一 Mod 的 entry set 要么全部是可唯一解析的 legacy set，要么全部带同一个 revision id；mixed set
  直接进入 unknown/repair gate；
- v1 entry set 只能通过 catalog 中不可变 `origin_revision_id` 与迁移 provenance 唯一解析；candidate
  import 即使已追加第二条 revision也不能改变该结果。缺失、冲突或仅能按“最新 revision”猜测时阻断
  `installed_revision_unknown`；
- CL3 成功后，被替换 Mod 的所有新 entries 都带 candidate revision id；
- 其他 Mod 的 legacy entries 原样保留，不因本次重装被猜测迁移；
- 第一次成功保存 CL3 manifest 时 document schema 升为 v2；若仍有其他 Mod 的 legacy set，
  `schema_migration` 必须明确标记兼容状态，不能把它们误报为已迁移；
- `installed_file`、`package_file_id`、layer 和 `backup_ref` 继续作为 target/provider/卸载事实；
- 同一 target group 中只能出现零个或一个不同的 original `backup_ref`，冲突 ref 必须阻断。

manifest 与 catalog 不做原子双写。candidate 在 preview 前已经是 catalog durable fact；成功 manifest
原子保存后，它即成为 installed revision。若重启时 manifest 指向 catalog 中不存在或不可解析的
revision，状态必须是 unknown/repair_required，任何破坏性动作 fail closed。

## 8. ReinstallPlan 分类

### 8.1 分类单位

分类单位是规范化后的最终 `InstallTargetPath` group，而不是单条 entry。分类输入：

- 指定 logical Mod 的旧 manifest entries，按 target 分组；
- candidate InstallPlan 中属于同一 logical Mod 的 actions，按 target 分组；
- candidate source 全量 bytes/summary；
- 当前 target bytes/summary；
- original backup 可读性与其他 Mod ownership。

每个 group 都构造稳定 provider stack signature：`package_file_id + layer name/priority + canonical order`。
revision id 本身不参与“是否需要磁盘写入”的判断，否则每次 revision 更新都会把 retained 误判为
replaced；但 candidate manifest entries 仍必须写入新的 revision id。

### 8.2 四类语义

| 分类 | 判定 | 游戏目录动作 | manifest / backup 动作 |
| --- | --- | --- | --- |
| retained | 新旧都有；provider stack 相同；candidate 最终 bytes 与旧 installed winner 相同 | 无 | 用 candidate revision entries 替换事实；继承 original backup |
| replaced | 新旧都有；provider stack 或最终 bytes 任一改变 | 写 candidate 最终内容 | 继承 original backup；另建 v1 transaction snapshot |
| added | 只在 candidate plan 中出现 | target 不存在则新增；存在且无其他 owner 时先备份再覆盖 | 现有文件 backup 成为 original backup；无文件则无 original backup |
| stale | 只在旧 manifest 中出现 | 无 original backup 则删除；有 original backup 则恢复游戏原始内容 | 成功 manifest 不再含该 entry；commit 后才清理无人引用的 original backup |

`retained` 仍必须校验当前 target 与旧 installed summary 一致，且 original backup（若有）可读。
“bytes 相同但 package file/layer stack 改变”属于 replaced，以免丢失 provider/layer 事实。

### 8.3 Fixture 预期

固定人工 fixture 的 `v1 -> v2` 预期为：

| Target | 分类 |
| --- | --- |
| `retained.bin` | retained = 1 |
| `replaced.bin`、`overwritten.bin` | replaced = 2 |
| `added-v2.bin` | added = 1 |
| `stale.bin` | stale = 1 |

最终 uninstall 必须删除 retained/replaced/added 中由工具新增的文件，并用同一 original backup 将
`overwritten.bin` 恢复为 `game-baseline-original\n`。

## 9. Preview、preflight 与 plan token

### 9.1 锁外 prepare

`preview_reinstall_plan` 是只读用例；`start_reinstall_task` 的 runner 也会在锁外重新 prepare，而不是
信任前端 preview：

1. 校验 game/profile/mod/revision/layer 等受控 id。
2. 读取 logical Mod、candidate revision 和 installed manifest entry set。
3. 从 candidate revision 重新构建 InstallPlan，拒绝 blocking conflicts 和非请求 Mod provider。
4. 全量 preload candidate source bytes，计算内部 summary。
5. 初步读取当前 target 与所需 original backup，构建四类 target 和 blocking reasons。
6. 生成只绑定后端 canonical facts 的短 opaque `planToken`。

preview 返回聚合计数、revision 摘要、blocking reason code 和 token，不返回 target path、source path、
backup ref、manifest 正文、raw hash 或 bytes。token 不是授权凭据，也不是可执行计划；start 必须重建。

### 9.2 锁内 revalidate

runner 获取共享写锁后必须重新读取并比较：

- game/profile 仍可用；
- manifest id/schema/status/指定 Mod entry set 未变化；
- candidate revision ownership、availability 和 source summary 未变化；
- target 当前 summary 仍匹配 pre-reinstall v1；
- original backup 仍存在且可读；
- 没有新增其他 Mod ownership/conflict；
- 重新计算的 plan token 与用户确认 token 一致。

任一变化返回 stale/preflight failure，在创建 game mutation 前停止。不要在锁内重新解压 archive、遍历
大型 sandbox 或做长 hash；source bytes 已在锁外 preload，锁内只做受限 revalidation。

## 10. Commit、rollback 与 recovery transaction

### 10.1 Durable transaction model

普通 `InstallRecoveryRecord` 只描述“本次 install 如何回到游戏原始基线”，不足以表达 v1 -> v2。
CL3 使用独立 `ReinstallRecoveryTransaction` 领域模型和 repository；它可以物理复用现有 backup root
和 JSON 原子写 helper，但必须区分 ownership/lifecycle。

transaction 至少保存内部事实：

- profile/mod、old/candidate revision、plan token/hash、状态；
- 完整 pre-reinstall manifest entry set；
- 每个 target 的分类、pre-state summary、candidate summary；
- mutating target 的 transaction snapshot ref 或“pre-state 不存在”；retained 只保存可验证摘要，
  不为无磁盘 mutation 创建多余 snapshot；
- transaction snapshot 使用 `stored -> cleanup_pending -> cleaned` 记录 durable cleanup progress：
  `stored` 仍可用于恢复，`cleanup_pending` 表示 target 已恢复或已确认无需恢复、snapshot 只待删除，
  `cleaned` 表示删除结果已 checkpoint；任一状态转换都先后通过原子 transaction save 固化；
- original backup ref（若有）及其 promotion/cleanup 语义；
- 足以判断 target 处于 pre-state、candidate-state 还是 unknown 的摘要。

这些 path/ref 只存在受控内部 repository，绝不进入 DTO、task event 或日志。

### 10.2 成功顺序

```text
prepare outside lock
  -> acquire shared game/profile write lock
  -> revalidate manifest/catalog/source/target/backup/token
  -> create every required mutating-target transaction snapshot (no game mutation yet)
  -> atomically save planned recovery transaction
  -> atomically mark transaction committing
  -> apply retained/replaced/added/stale in deterministic target order
  -> build candidate manifest by replacing only this Mod entry set
  -> atomically save manifest once (commit point)
  -> persist transaction completed (post-commit bookkeeping)
  -> cleanup non-promoted transaction snapshots and unreferenced stale original backups,
     checkpointing each deleted ref while retaining complete target summaries
  -> remove completed transaction best-effort
  -> complete task
```

对 added 且已有未托管文件的 target，pre-state snapshot 可以在成功时晋升为 original backup；对
replaced/stale，v1 transaction snapshot 永远不能覆盖原有 original backup 语义。

cleanup 是 commit 后的非破坏性收敛：失败可以保留受控 orphan backup/完成记录等待后续维护，但不能
把已经固化的 v2 报成 rollback_required。任何 original backup 只有在 manifest 已不再引用且对应
target 已成功恢复后才能删除。transaction 只能在所有 snapshot/backup ref 已清理并 checkpoint 后移除；
删除或 transaction removal 失败时保留最后一个 durable resume point。

recovery transaction save 返回错误时也必须按 profile/mod read-back 判定 durable ownership：只有确认
transaction 不存在时，才允许 best-effort 删除尚未归属的 snapshot；读取失败、记录不匹配或落盘结果不明时
保留 snapshot 并 fail-closed。original backup cleanup 使用 at-least-once 顺序：保留 ref，先执行幂等删除，
删除成功后再清空 ref 并保存 checkpoint；禁止在删除前清空唯一 durable ref，以免删除失败后形成 orphan。

`save_manifest` 成功返回是唯一 commit point。其后的 completed 状态持久化属于 post-commit
bookkeeping；若该持久化返回错误：

1. v2 manifest 继续作为权威事实，绝不能进入普通 rollback 或报告 `rolled_back`；
2. 保留最后一个 durable `committing` transaction 和全部待清理 snapshot，不提前 cleanup；
3. task 以稳定错误 `install_reinstall_failed:post_commit` 结束，公开恢复分类为
   `committed_cleanup_pending`，不能伪报 task completed；
4. 后续受控 reconciliation 在同一 game/profile 写锁下重新验证 candidate manifest 与 target 摘要，
   再持久化 completed bookkeeping 并执行 cleanup；若现场已无法证明 candidate-state，则进入
   `repair_required`，但仍不得自动回滚已经越过 commit point 的 v2。

completed 已持久化后的纯 cleanup failure 可以保留 completed transaction/orphan backup 并让后续维护
继续清理；它与上述 `post_commit` bookkeeping failure 是两个独立 fault point。

### 10.3 同步失败

若 manifest commit point 之前任一步失败：

1. 按反向顺序用 transaction snapshot 恢复所有可能已 mutation 的 target；pre-state 不存在则删除。
2. 每个 target 恢复后先将 snapshot 持久化为 `cleanup_pending`，才允许删除；删除后再 checkpoint
   `cleaned` 或移除已收敛 target fact。cleanup 失败时不得重新把已恢复 target 当作未恢复 target。
3. 旧 manifest 未被修改，继续作为 v1 权威事实。
4. rollback 全部成功：task 为 failed，公开结果为 `rolled_back`，删除或收敛 transaction。
   transaction removal 失败时仍保持 `rolled_back`，但内部 commit result 必须标记 cleanup pending，
   并保留 durable `rolled_back` transaction 供后续收敛。
5. rollback 部分失败：只保留未恢复 target facts，状态为 `rollback_required` 或 `repair_required`。

manifest save 返回错误时不能假设 rename 未发生。实现必须重新读取 manifest：

- 仍是旧 entry set：rollback 磁盘到 v1；
- 已是完整 candidate entry set：先用 recovery transaction 中的 pre-reinstall snapshot 原子恢复旧
  manifest，再 rollback 磁盘到 v1；
- 结果不确定或旧 manifest 恢复失败：保留 recovery transaction 并进入 repair_required，绝不 completed。

只要 `save_manifest` 向本次 runner 返回错误，该 task 就不能把 candidate 当作成功提交。与之不同，
进程在成功 save 后、mark-completed 前崩溃时没有“save 返回错误”事实；该现场由重启 reconciliation
按完整 candidate manifest + target summaries 单独判断。若 save 已成功返回、但 mark-completed 向当前
runner 返回错误，则按 10.2 的 `post_commit` 规则结束当前 task，再由同一 reconciliation 收敛。

聚焦 fault tests 必须证明“可控制的 manifest save failure”最终恢复 v1 bytes 与旧 manifest。无法保证
存储结果的真实异常必须被持久化为受控恢复状态，而不是伪造成功。

### 10.4 崩溃恢复

重启扫描按 recovery transaction 状态与 manifest/target 摘要判断：

| 现场 | 结论 |
| --- | --- |
| planned，全部 target 仍是 pre-state | 无 game mutation；可安全清理 transaction snapshots |
| committing，manifest 仍是旧 set，target 为 pre/candidate 混合 | 提供受控 rollback 到 v1 |
| committing，manifest 已是完整 candidate set，全部 target 为 candidate-state | 判定为 `committed_cleanup_pending`；后续受控 reconciliation 收敛 |
| completed，manifest/target 为 candidate-state | 判定为 `cleanup_pending`；后续受控 reconciliation 清理 |
| 任一 target、backup 或 manifest 无法判断 | repair_required；所有 install/uninstall/reinstall 阻断 |

`committed_cleanup_pending` 与 `cleanup_pending` 是 recovery scan 的派生分类，不是新增的 durable
transaction status；底层 transaction 分别仍为 `committing` 与 `completed`。两者在 reconciliation 完成
前都阻断同一 game/profile 的新 install、uninstall 或 reinstall，避免新的写入与旧 snapshot cleanup 竞态。

恢复动作与 reconciliation 仍使用同一个写锁 registry。扫描只读，只报告稳定分类，不直接改游戏
文件、manifest、transaction 或 backup；任何写入型恢复必须走受控 app use case，涉及游戏目录时还要
先 preview/确认并使用窄 command。

## 11. Manifest entry-set replacement

CL3 新增纯粹的 `replace_entries_for_mod` 语义，不能直接复用当前按 target 删除的
`merge_install_manifest`：

1. clone 完整 profile manifest；
2. 验证旧 set 仅属于请求 mod，candidate entries 全部属于同一 mod/revision；
3. 验证 candidate/old targets 没有任何其他 Mod owner；
4. 仅移除 `entry.mod_id == requested_mod_id` 的旧 entries；
5. 加入 retained/replaced/added 对应 candidate entries；stale 不加入；
6. 其他 Mod entries、document metadata 和未触及事实保持不变；
7. 更新 schema/status/timestamp/plan hash 后只调用一次 `save_manifest`。

成功后不能残留 old revision entry；失败时不能出现 old/new mixed set。对跨 Mod 共用 target 的未来
layered reinstall 必须另行设计，CL3 只返回 `cross_mod_target_conflict`。

## 12. Task、并发与 cancellation

为减少无关共享 contract 变更，CL3 继续使用 `TaskKind::Install`，以专用 phase 区分：

| phase | 含义 |
| --- | --- |
| `install.reinstall.queued` | task 已登记 |
| `install.reinstall.plan.building` | 锁外重建 candidate plan/source |
| `install.reinstall.preflight.processing` | 聚合分类并准备进入 commit |
| `install.reinstall.commit.processing` | 写锁内 backup/mutation/manifest |
| `install.reinstall.rollback.processing` | 同步失败后回到 pre-reinstall |
| `install.reinstall.completed` | manifest 已固化 candidate revision |
| `install.reinstall.failed` | 失败或需要受控恢复 |
| `install.reinstall.cancelled` | 仅在 queued/prepare 安全点取消 |

失败 event 的 `error` 使用 `install_reinstall_failed:<phase>`，phase 至少覆盖 `planning`、`preflight`、
`lock`、`backup`、`commit`、`manifest`、`post_commit`、`rollback` 和 `complete`。event 始终携带
task id，前端不能按“当前唯一任务”匹配。`post_commit` 明确表示 candidate manifest 已越过 commit
point、但 completed bookkeeping 尚待 reconciliation；它绝不等价于 `rolled_back`。

一旦 durable transaction 进入 committing，cancellation 变为 deferred/rejected：runner 必须完成
commit 或 rollback，再落最终 task 状态。不能因 UI 已显示 cancelled 而留下已提交 manifest。

## 13. 模块边界

建议文件布局，最终以实现时 file-size gate 和现有模块为准：

| 边界 | 职责 | 禁止 |
| --- | --- | --- |
| `hmm-core/reinstall.rs` | revision id、target signature、四类纯分类、entry-set replacement invariant、transaction 状态 | FS、Tauri、MHW path、repository |
| `hmm-ports/mod_import.rs` / `reinstall.rs` | revision catalog、source、manifest、recovery transaction ports | concrete JSON/FS、DTO |
| `hmm-app/reinstall.rs` | preview、preflight、plan token、prepare/commit 编排 | 直接依赖 infra concrete type |
| `hmm-app/reinstall_task.rs` | TaskManager、phase、cancellation barrier、shared lock、Audit | Tauri event emitter、真实 path |
| `hmm-infra` | catalog v2 migration、atomic JSON、filesystem backup/manifest/recovery adapters | 领域分类、UI 决策 |
| `src-tauri/src` | request parse、DTO/error mapping、AppState composition、queued event、spawn runner | 分类、path 构建、文件写入 |
| `src/features/mods` | revision 选择、聚合 preview、确认、taskId listener、完成后 refetch | target/backup/rollback 规则 |

通用 CL3 contract 不包含 MHW `nativePC`、slot、catalog 或 ARMOR path 规则。未来 ARMOR target switch 只
生成新的 candidate InstallPlan，再复用本重装链路。

## 14. 未来 Tauri / DTO contract

以下是 Task 7 必须落地并同步到 `docs/FRONTEND_BACKEND_CONTRACT.md` 的目标，不是当前已实现 command：

```ts
type PreviewReinstallPlanRequestDto = {
  gameId: string;
  profileId: string;
  modId: string;
  candidateRevisionId: string;
  layer: FileLayerDto;
};

type StartReinstallTaskRequestDto = PreviewReinstallPlanRequestDto & {
  planToken: string;
};

type ReinstallTargetCountsDto = {
  retained: number;
  replaced: number;
  added: number;
  stale: number;
};

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
      blockingReasons: Array<{ code: string; count: number }>;
    };
```

`status` 是 DTO 的判别字段：`ready` 必须同时提供非空 candidate、installed revision 与 token，且没有
blocking reason；`blocked` 不生成 token，并允许缺失尚未解析出的 revision。特别是
`candidate_not_found` 必须返回 `candidateRevision: null`，不能伪造 revision summary。前端必须先按
`status` narrowing，再读取 candidate 或提交 token。

窄 command 集合：

- `start_import_mod_revision_task`
- `get_mod_revisions`
- `preview_reinstall_plan`
- `start_reinstall_task`

`start_reinstall_task` 返回既有 `TaskStartedDto { taskId, kind: "install", status: "queued" }`。command
body 保持 `parse -> app service -> DTO/error -> queued event -> spawn runner`。start 只接收受控 id、layer
与 preview token；不接收 archive/source/sandbox/target/delete/backup/manifest/game-root path。

preview blocking reason 至少稳定覆盖：

- `not_installed`
- `candidate_not_found`
- `candidate_not_ready`
- `candidate_owner_mismatch`
- `candidate_already_installed`
- `installed_revision_unknown`
- `manifest_state_unsafe`
- `source_unavailable`
- `target_missing` / `target_changed` / `target_read_failed`
- `backup_missing` / `backup_read_failed`
- `plan_conflict`
- `cross_mod_target_conflict`
- `preview_stale`

command error 负责“用例不可用/输入无效”，blocking reason 负责可展示的预检结论。message 不能作为
前端分支，也不能包含 path、ref、hash 或第三方内容。

## 15. 前端工作流

1. Mod 库仍以 logical `modId` 展示一张卡，分别展示 installed revision 与可用 revision 摘要。
2. 用户通过“导入新版本”把 archive 附加到当前 Mod，不能靠展示名自动合并。
3. 只有 manifest/recovery 摘要为 `installed` 且 candidate 为 ready 时启用重装 preview。
4. `committed_cleanup_pending`、`cleanup_pending`、`rollback_required`、`repair_required`、`unknown`、
   candidate unavailable 一律禁用确认。
5. preview 使用独立 `ReinstallPlanPreviewPanel` 或明确 discriminated state，展示四类聚合计数与阻断。
6. 确认时提交同一 candidate id 与 `planToken`；后端仍重新 prepare/revalidate。
7. task listener 按返回的 `taskId` 匹配 `install.reinstall.*`，不复用“普通 install 即重装”的分支。
8. completed 后重新查询 Mod revisions、manifest/recovery 状态和动作可用性；不从 task 内存推断 v2。
9. failed/cancelled 后同样 refetch；如果后端返回 rollback/repair/cleanup-pending 状态，进入受控恢复或
   reconciliation 入口。`post_commit` 必须展示“candidate 已提交、等待收敛”，不能提供回滚到 v1 的快捷动作。

现有 `CompactActionPanel` 的 reinstall 分支、`modInstallTaskState.ts` 的 `install | uninstall` union 和普通
`InstallPlanPreviewPanel` 都需要在 Task 8 中校正，但不进行无关页面重构、主题或分页工作。

## 16. Audit、日志与隐私

Audit `operation` 固定为 `reinstall_mod`。允许字段白名单：

- `task_id`、`game_id`、`profile_id`、`mod_id`
- `previous_revision_id`、`candidate_revision_id`（短受控 id）
- `retained_count`、`replaced_count`、`added_count`、`stale_count`
- `result`、稳定 `error_code`、`rollback_result`

禁止 target 列表、完整 path、用户名、Steam ID、backup ref/root、manifest/source 正文、raw hash、
sandbox/cache path 和第三方 Mod 内容。Task Log 同样只记录 task id、phase、结果和稳定错误分类。
completed bookkeeping 失败时 Audit 使用顶层 `result: "failure"`，fields 使用稳定
`error_code: "install_reinstall_failed:post_commit"` 与 `rollback_result: "not_attempted_post_commit"`；不能
记录 `rolled_back` 或泄漏 transaction/snapshot ref。

## 17. 验收矩阵

### L1 聚焦测试

| 证据 | 必须证明 |
| --- | --- |
| Core classifier | 四类完整且互斥；provider/layer 改变不误判 retained；跨 Mod ownership 阻断 |
| Catalog v1 -> v2 | 一张 logical Mod 卡、一个 migrated revision；追加 v2 不覆盖 v1；migration failure 保留 v1 |
| Manifest compatibility | legacy v1 可唯一解析；mixed/unknown revision 阻断；candidate entries 全带 revision id |
| Preview/preflight | read-only；四类计数正确；`candidate_not_found` 返回 null candidate/token；ready 必有 candidate/token；source/target/backup/manifest 任一失败时零 game write |
| Backup ownership | replaced 保留 original backup；transaction snapshot 成功后清理；added backup 可晋升 |
| Commit/rollback | 每个 pre-commit write/remove/manifest failure 回到 v1；部分 rollback 只保留未恢复 facts；completed bookkeeping failure 保留 v2 并进入 post_commit reconciliation |
| Entry-set replacement | 只替换指定 Mod；stale 消失；其他 Mod facts 不变；冲突 fail closed |
| Task/lock | taskId/phases/error 稳定；prepare 在锁外；四种写入用例共享 registry；commit 不被取消中断；post_commit 收敛仍使用同一锁 |
| DTO/Audit/frontend | status union 与 camelCase/snake_case shape 稳定；无 path/ref/content；post_commit 使用 failure/error/rollback 白名单并在最终状态后 refetch |

### L2 AppState composition

使用固定人工 zip、temp AppData、temp MHW:I-like game root 与真实 AppState composition：

```text
import v1 as logical Mod
  -> install v1
  -> restart and observe installed v1
  -> import v2 revision into the same modId
  -> preview 1 retained / 2 replaced / 1 added / 1 stale
  -> reinstall v2
  -> restart and observe installed v2 from manifest + catalog
  -> uninstall
  -> restart and compare game root byte-for-byte with pre-v1 baseline
```

同时断言 old v1 manifest entry set 不再存在、`overwritten.bin` original backup 跨重装保留、active
recovery transaction 已收敛、task/Audit 证据脱敏。至少再覆盖一次 manifest save failure -> rollback v1
-> restart 仍显示 installed v1。

### L3 Windows Sandbox

在 disposable Windows Sandbox 中运行实际 Tauri 应用，只使用人工 v1/v2、TEMP game root 和 disposable
AppData，执行 revision import、preview、confirm、restart、uninstall、diagnostics 和 cleanup。不得选择
真实 MHW:I、Steam userdata、第三方 Mod 或日常账户目录。

## 18. 完成定义

CL3 只有同时满足以下条件才可标为 implemented：

- catalog v2、manifest revision、ReinstallPlan、recovery transaction、task/Tauri/frontend 全部落地；
- L1 fault/serialization/concurrency/privacy tests 通过；
- L2 `v1 -> v2 -> restart -> uninstall -> baseline` 通过；
- L3 Windows Sandbox 实际桌面工作流执行并记录清理证据；
- 完整 `scripts/verify.ps1` 与 HMM review gate 通过；
- 当前 contract 文档、前后端 contract、测试指南和验收基线同步；
- CL4 仍未提前标记 Gate A `certified`。

CL3 完成后下一项只能是 CL4 独立复审和认证。ARMOR_RETARGET 仍等待 Gate A。
