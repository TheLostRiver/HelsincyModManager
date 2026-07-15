# Core Mod Lifecycle CL3 真正重装实施计划

- 状态：Task 0-10 已完成，CL3 于 2026-07-15 标记为 `implemented`；CL4 / Gate A 尚未认证

> **执行说明：** 本计划消费
> [CL3 真正重装设计](../specs/2026-07-14-core-mod-lifecycle-cl3-true-reinstall-design.md) 与
> [CL0/CL1/CL2/CL3 验收基线](../../CORE_MOD_LIFECYCLE_CL0_ACCEPTANCE.md)。每个 Task 先写失败测试、
> 再做最小实现并独立提交。不得把多个 Task 合并成一个大型实现 PR。

**Goal:** 使用稳定 logical Mod id、不可变 package revision、四类 ReinstallPlan、preflight、独立
recovery transaction 和 entry-set replacement，完成真实 `v1 -> v2 -> restart -> uninstall -> baseline`。

**Architecture:** 纯分类与状态不变量放 `hmm-core`；repository/filesystem 能力经 `hmm-ports`；
preview/prepare/commit/recovery/task 编排放 `hmm-app`；JSON/FS adapter 放 `hmm-infra`；Tauri 保持薄；
前端只消费受控 id、聚合 preview 和 task 状态。install/uninstall/reinstall/recovery 共享现有
`GameProfileWriteLockRegistry`。

**Safety:** 自动化只使用人工 v1/v2、fake ports、`tempfile::TempDir`、temp AppData 和 temp game root。
不读取或修改真实 MHW:I、Steam userdata、玩家存档、第三方 Mod、日常 AppData 或真实 backup。

**Scope stop:** 本计划不实现 CL4 认证、ARMOR_RETARGET、P7.2c、分页、批量迁移、批量操作、任务队列、
revision GC 或无关 UI 重构。CL3 完成后必须停下并单独进入 CL4 review。

---

## 执行纪律

每个 Task 都必须：

1. 从最新 main 建独立 `hy/` 分支/worktree，先确认用户 worktree 未被触碰。
2. 先运行指定 RED，记录真实失败原因；不能写完实现后补一个恒绿测试。
3. 只修改列出的职责边界；若发现跨 Task 阻断，先更新计划/评审，不顺手扩张。
4. 运行 Task 聚焦命令、相关 crate tests/check，再创建独立提交。
5. 不提交 `.planning/`、`.plan-attestation`、真实 archive/save、日志、backup、cache 或构建产物。
6. command/DTO/phase/error 真正落地时同步 `docs/FRONTEND_BACKEND_CONTRACT.md`，不能提前声称存在。
7. 高风险 file/recovery 变更使用 HMM install safety、task/concurrency 和 review gate。

推荐提交顺序与 Task 顺序一致；某 Task 暴露安全缺口时允许增加一个紧邻的修复提交，但必须有对应
失败测试，且不能进入后续产品范围。

## Task 0：基线与 contract 锁定

**Files:**

- Read: `docs/superpowers/specs/2026-07-14-core-mod-lifecycle-cl3-true-reinstall-design.md`
- Read: `docs/CORE_MOD_LIFECYCLE_CL0_ACCEPTANCE.md`
- Read: `docs/FRONTEND_BACKEND_CONTRACT.md`
- Read: `docs/TESTING.md`
- Read: `docs/LOGGING.md`
- No production file changes

### Preflight

1. 确认 main 上 CL0、CL1、CL2 证据仍通过，CL3/Gate A 未被误标完成。
2. 确认当前 importer、manifest schema、recovery record、write lock、Tauri command 和前端 reinstall
   事实仍与设计文档一致；若 main 已演进，先修订 spec/plan。
3. 全新 worktree 先按 frozen lockfile 安装前端依赖，并生成 ignored/untracked 的 debug worker sidecar；
   不从其他 worktree 复制产物，也不提交 `node_modules/`、`target/`、`dist/` 或 `src-tauri/binaries/`。
4. 保存基线命令结果：

```powershell
cmd /c corepack pnpm install --frozen-lockfile
cmd /c corepack pnpm run prepare:save-backup-worker-sidecar:dev
cargo test -p hmm-tauri state::core_mod_lifecycle_tests
cargo test -p hmm-app install
cargo check --workspace
cmd /c corepack pnpm run typecheck
git status --short
```

### 停止条件

- 基线失败且与 CL3 无关：先建独立阻断修复，不在 Task 1 混入。
- fixture、真实数据边界或 installed 状态来源不明确：停止，不靠手改 JSON 继续。
- spec 中任何身份/recovery/commit point 决策仍有两种实现含义：先 review 文档。

本 Task 不需要提交；若 contract 必须修订，单独 `docs:` 提交并重新 review 后再开始 Task 1。

## Task 1：Core identity、分类与 entry-set replacement

**Files:**

- Create: `src-tauri/crates/hmm-core/src/reinstall.rs`
- Modify: `src-tauri/crates/hmm-core/src/install.rs`
- Modify: `src-tauri/crates/hmm-core/src/lib.rs`
- Test: `src-tauri/crates/hmm-core/src/reinstall.rs`

### RED

先定义测试名称和最小输入 builder，至少覆盖：

- `classifies_fixture_as_one_retained_two_replaced_one_added_one_stale`
- `revision_change_alone_does_not_turn_identical_provider_bytes_into_replaced`
- `provider_or_layer_change_is_replaced_even_when_final_bytes_match`
- `classification_rejects_duplicate_or_unclassified_target_facts`
- `classification_rejects_cross_mod_target_ownership`
- `replace_entries_for_mod_removes_only_requested_mod_and_all_stale_entries`
- `replace_entries_for_mod_rejects_mixed_revision_or_other_owner`
- `legacy_entry_set_requires_one_provenance_resolved_revision`

测试必须使用纯值、相对 `InstallTargetPath` 和人工 SHA/size，不访问文件系统。

运行并观察 RED：

```powershell
cargo test -p hmm-core reinstall -- --nocapture
```

### GREEN

实现最小 domain contract：

- `ModRevisionId` 或等价强类型；不与 `ModId` / task id 混用。
- target group/provider stack signature。
- `ReinstallTargetClass::{Retained, Replaced, Added, Stale}`。
- complete/unique classification validation。
- `replace_entries_for_mod` 纯 manifest transformation。
- manifest entry 的 backward-compatible optional revision field；legacy/new/mixed invariant。

不要把 source bytes、repository、clock、Tauri DTO、MHW path 或 backup I/O 放入 core。

### 验收

```powershell
cargo test -p hmm-core reinstall
cargo test -p hmm-core install
cargo check -p hmm-core
cargo clippy -p hmm-core --all-targets -- -D warnings
```

**Commit:** `feat(core): 定义真正重装分类契约`

## Task 2：Revision catalog v2 与 candidate import app contract

**Files:**

- Modify: `src-tauri/crates/hmm-ports/src/mod_import.rs`
- Modify: `src-tauri/crates/hmm-ports/src/lib.rs`
- Modify: `src-tauri/crates/hmm-infra/src/mod_import.rs`
- Modify: `src-tauri/crates/hmm-app/src/mod_import.rs`
- Modify: `src-tauri/crates/hmm-app/src/mod_import_task.rs`
- Modify: `src-tauri/crates/hmm-app/src/mod_library.rs`
- Modify: `src-tauri/crates/hmm-app/src/lib.rs`
- Test: existing mod import repository/app/task/library tests

最终文件名可按现有模块拆分；若 `mod_import.rs` 接近 file-size gate，新增
`mod_revision_catalog.rs`，不要继续堆大。

### RED

Repository tests：

- schema v1 一条 record 原子迁移为一个 logical Mod + 一个 revision；id/metadata 保持。
- schema v2 对同一 mod 追加 v2 后保留 v1，不按 `mod_id` 覆盖。
- migrated `origin_revision_id` 在追加 v2 后仍唯一解析 legacy installed v1，不能回退到 latest/display。
- revision id 跨 Mod 重复或 owner mismatch 被拒绝。
- migration/temp write/rename failure 不破坏原 v1 文件。
- reload 后 logical Mod 列表仍只有一张卡，revision list 有 v1/v2。

App/task tests：

- 普通 import 创建新 logical Mod。
- revision import 要求已存在 `modId`，成功附加 unique revision/package。
- 不存在 mod、cancelled、prepare failure 不写 revision record并清理本次 sandbox。
- `stored_analysis_from_result` 不再把 task/package id 强制覆盖 logical mod id。
- metadata/category overlay 仍绑定 logical mod，不随 display revision 丢失。

先运行定向测试并确认当前覆盖式 repository 失败：

```powershell
cargo test -p hmm-infra mod_import
cargo test -p hmm-app mod_import
cargo test -p hmm-app mod_library
```

### GREEN

1. 在同一个原子 catalog 文件中保存 `mods[] + revisions[]`。
2. 分离 `save_new_mod`、`append_revision`、`get_revision`、`list_revisions` 等窄 port；保留必要的
   compatibility query，但不继续以 `save_analysis(mod_id)` 表达所有语义。
3. logical Mod 保存不可变 `origin_revision_id`/migration provenance；candidate import 不改变它。
4. installed revision 不写 catalog current pointer；library query 后续与 manifest join。
5. `display_revision_id` 只影响卡片展示，不表示已安装。
6. old revisions/sandboxes 全部保留；CL3 不做 GC。

本 Task 只落 Rust app/repository contract，不新增 Tauri command 或前端。

### 验收

```powershell
cargo test -p hmm-infra mod_import
cargo test -p hmm-app mod_import
cargo test -p hmm-app mod_library
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

**Commit:** `feat(import): 持久化稳定 Mod revision catalog`

## Task 3：Manifest revision 与 durable reinstall recovery transaction

**Files:**

- Modify: `src-tauri/crates/hmm-core/src/install.rs`
- Modify: `src-tauri/crates/hmm-core/src/reinstall.rs`
- Modify: `src-tauri/crates/hmm-ports/src/install.rs`
- Create or Modify: `src-tauri/crates/hmm-ports/src/reinstall.rs`
- Create: `src-tauri/crates/hmm-infra/src/reinstall.rs`
- Modify: `src-tauri/crates/hmm-infra/src/install_commit.rs`
- Modify: crate `lib.rs` exports
- Test: focused core/ports/infra serialization and repository tests

### RED

至少覆盖：

- schema v1 manifest 无 revision 仍可读取，但 mixed revision set 被拒绝。
- candidate manifest entries 序列化 camel/snake field contract 后 reload revision 不丢失。
- recovery transaction 保存完整 old entry set、四类 target、pre/post summary 和 snapshot ownership。
- transaction status 只允许 planned -> committing -> completed/rollback_required/repair_required 等受控迁移。
- manifest/recovery temp write、sync、rename failure 不产生半 JSON。
- snapshot ref containment、absolute/traversal/symlink rejection 复用现有 backup safety。
- original backup 与 transaction snapshot cleanup policy 不混淆。

```powershell
cargo test -p hmm-core reinstall
cargo test -p hmm-infra reinstall
cargo test -p hmm-infra install_commit
```

### GREEN

- 新增独立 `ReinstallRecoveryTransaction` 与 repository port，不把 pre-reinstall facts 塞进普通
  `InstallRecoveryRecordEntry`。
- 可以物理复用 `FileSystemInstallBackupStore`，但 app/domain 必须显式标记 snapshot purpose、promotion
  和 cleanup ownership。
- repository 继续使用受控 root、containment、regular-file/symlink 校验和 atomic write helper。
- manifest loader 支持 legacy；成功重装 entry set 必须全带 candidate revision。
- 不新增逐 entry manifest save API；profile manifest 仍一次整体原子保存。

### 验收

```powershell
cargo test -p hmm-core install
cargo test -p hmm-core reinstall
cargo test -p hmm-infra reinstall
cargo test -p hmm-infra install_commit
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

**Commit:** `feat(install): 记录重装 revision 与恢复事务`

## Task 4：App preview、preflight 与 stale-preview token

**Files:**

- Create: `src-tauri/crates/hmm-app/src/reinstall.rs`
- Modify: `src-tauri/crates/hmm-app/src/lib.rs`
- Test: `src-tauri/crates/hmm-app/src/reinstall_tests.rs` 或 sibling module

### RED

使用 fake catalog/planner/source/game/backup/manifest ports，至少覆盖：

- fixture preview 返回 retained=1、replaced=2、added=1、stale=1。
- preview 只读：game write/remove、manifest save、recovery save、backup store 全为 0。
- candidate missing/unready/owner mismatch/already installed 返回稳定 blocking reason；
  `candidate_not_found` 的 public preview 返回 null candidate/token。
- unsafe manifest status、legacy revision 无法唯一解析、mixed set 阻断。
- source read、target missing/changed/read error、backup missing/read error 阻断。
- candidate plan blocking conflict、其他 Mod provider/target ownership 阻断。
- token 对 manifest entry set、candidate revision/source summary、layer 变化敏感。
- public preview 只含短 revision id、四类计数、reason code、token，不含 path/ref/hash/content；ready 必须
  同时有 installed/candidate revision 与 token，blocked 不生成 token。

```powershell
cargo test -p hmm-app reinstall::tests::preview -- --nocapture
```

### GREEN

- `ReinstallPreviewService` 只接收 game/profile/mod/candidate/layer 等受控 domain ids。
- planner 从 revision catalog 解析 candidate package/sandbox；调用方不能直接传 source root。
- 全量 source preload 和初步 preflight 在锁外完成。
- `planToken` 是后端 canonical facts 的 opaque stale guard；start/commit 仍会重建，不以 token 授权。
- error 内部可保留诊断 source，但公开 mapping 只使用稳定 code。

不要在本 Task 执行游戏写入、接 TaskManager、加 Tauri command 或修改前端。

### 验收

```powershell
cargo test -p hmm-app reinstall
cargo test -p hmm-app install
cargo check -p hmm-app
cargo clippy -p hmm-app --all-targets -- -D warnings
```

**Commit:** `feat(app): 构建真正重装预览与预检`

## Task 5：App commit、rollback 与 entry-set replacement

**Files:**

- Modify: `src-tauri/crates/hmm-app/src/reinstall.rs`
- Create if needed: `src-tauri/crates/hmm-app/src/reinstall_commit.rs`
- Modify: `src-tauri/crates/hmm-app/src/lib.rs`
- Test: focused app reinstall commit/fault tests

### RED：happy path

人工 fake bytes 覆盖：

1. 全部 source 在第一次 target mutation 前已读取。
2. 全部 mutating target 的必需 pre-state snapshot 与 planned/committing recovery facts 在第一次
   mutation 前已保存；retained 不创建多余 snapshot。
3. retained 不写；replaced 写 v2；added 写 v2；stale 删除或恢复 original。
4. `overwritten.bin` 的 original backup ref 从 v1 entry 继承，不被 v1 snapshot 替换。
5. manifest 只 save 一次，只替换请求 Mod entry set；其他 Mod entries 不变。
6. candidate entries 全带 v2 revision；stale/old v1 entries 消失。
7. transaction completed 后只清理非 promoted snapshots 和无人引用 stale original backup。

### RED：failure matrix

对每个阶段注入失败并断言 game/manifest/recovery/backup：

| 故障 | 必须证明 |
| --- | --- |
| second source read | 0 backup、0 game mutation、0 manifest save |
| transaction backup store | 0 game mutation、0 manifest save，已建 snapshot cleanup |
| recovery planned/committing save | 0 game mutation，facts 可收敛 |
| first/middle/last write/remove/restore | rollback 回 v1；old manifest 有效 |
| manifest save before replace | rollback 回 v1；old manifest 有效 |
| manifest error but candidate visible | 原子恢复 old manifest snapshot，再 rollback v1；恢复失败进入 repair_required，绝不完成 |
| rollback one target fails | 只保留未恢复 target 与所需 snapshot；已恢复 target 的 snapshot 以 cleanup_pending/cleaned checkpoint，状态 rollback_required |
| recovery update fails during rollback | snapshot 不被提前删除，状态 repair_required |
| transaction completed save after successful manifest save | v2 manifest 保持权威，不 rollback；transaction/snapshot 保持 committing，task 返回 `install_reinstall_failed:post_commit` 与 committed_cleanup_pending |
| post-commit cleanup fails | v2 manifest 仍 completed；逐 snapshot/backup ref checkpoint 已完成进度并保留 transaction resume point，不回滚已提交 v2 |
| rollback/completed transaction remove fails | 保留 RolledBack/Completed durable transaction；rollback result 显式携带 cleanup pending，且不得丢弃 ownership 或重复恢复已收敛 target |

同时覆盖 lock-time revalidation：preview 后 manifest/source/target/backup/ownership 任一变化，第一次 game
mutation 前返回 `preview_stale` 或具体 preflight code。

```powershell
cargo test -p hmm-app reinstall::tests::commit -- --nocapture
cargo test -p hmm-app reinstall::tests::fault -- --nocapture
```

### GREEN

严格实现设计文档第 10 节顺序。不要复用当前 `merge_install_manifest` 的按 target 删除语义；调用
Task 1 的 `replace_entries_for_mod`。commit 内只消费锁外 prepared source bytes，并做短时 revalidation。

同步错误 rollback 到 pre-reinstall；崩溃/未知状态留 durable transaction。任何 pre-commit failure
path 都要先收敛 recovery facts，再清理 snapshot。manifest 成功返回后已越过 commit point；此后的
completed bookkeeping failure 只能进入 post_commit reconciliation，不能复用普通 rollback。

### 验收

```powershell
cargo test -p hmm-app reinstall
cargo test -p hmm-app install
cargo test -p hmm-app install_recovery
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

**Commit:** `feat(app): 安全提交并回滚真正重装`

## Task 6：Reinstall task、共享写锁、恢复扫描与 Audit

**Files:**

- Create: `src-tauri/crates/hmm-app/src/reinstall_task.rs`
- Modify: `src-tauri/crates/hmm-app/src/install_task.rs`
- Modify: `src-tauri/crates/hmm-app/src/install_recovery.rs`
- Modify: `src-tauri/crates/hmm-app/src/install_manifest_query.rs`
- Modify: `src-tauri/crates/hmm-app/src/lib.rs`
- Test: sibling task/concurrency/recovery/Audit tests

### RED

- start 创建 `TaskKind::Install` queued task，不泄漏 request。
- events 使用完整 `install.reinstall.*` phase 集并始终携带同一 taskId。
- failure error 为 `install_reinstall_failed:<phase>`，包含独立 `post_commit` phase，message 不参与分支。
- prepare 可与另一个 game/profile commit 并行，且不持有 write lock。
- 同一 game/profile 的 install、uninstall、reinstall、controlled recovery 两两串行。
- queued/prepare cancellation 不产生 game write；committing 后 cancellation 不截断 commit/rollback。
- crash transaction 被 recovery scan 映射为 rollback_required/repair_required；manifest candidate 已固化时
  映射为 committed_cleanup_pending，并由受控 reconciliation 持久化 completed 后 cleanup。
- 注入 completed bookkeeping 持久化失败时，task 返回 `install_reinstall_failed:post_commit`，v2
  manifest/target 不回滚、snapshot 不提前清理；Audit 顶层 result 为 failure，稳定 rollback result 为
  not_attempted_post_commit。
- manifest/recovery unsafe 状态阻断新的 install/uninstall/reinstall。
- committed_cleanup_pending/cleanup_pending 是 scan 派生分类而非 transaction status，并在受控
  reconciliation 完成前阻断同 game/profile 的新写入。
- `reinstall_mod` Audit 只含白名单 id/count/result/error/rollback fields。

并发测试使用 channels/barriers，不靠长 `sleep` 判断锁顺序。

```powershell
cargo test -p hmm-app reinstall_task -- --nocapture
cargo test -p hmm-app install_task
cargo test -p hmm-app install_recovery
```

### GREEN

- 新 runner 注入 AppState 已共享的 `Arc<GameProfileWriteLockRegistry>`，不创建第四套 registry。
- phases、failure mapping、Audit helper 保持独立 module，避免继续膨胀 `install_task.rs`。
- 进入 committing 后设置明确 cancellation barrier；最终 task 状态以 commit/rollback 结果为准。
- recovery scan 只读；任何写入型收敛/rollback 继续走受控 action 与同一写锁。
- post_commit reconciliation 重新验证 candidate manifest/target summaries；无法证明时进入
  repair_required，不自动回滚已越过 commit point 的 v2。

### 验收

```powershell
cargo test -p hmm-app reinstall_task
cargo test -p hmm-app install_task
cargo test -p hmm-app install_recovery
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

**Commit:** `feat(task): 串行化真正重装与恢复状态`

## Task 7：Tauri DTO、commands、AppState composition 与 contract

**Files:**

- Create: `src-tauri/src/reinstall_dto.rs`
- Create: `src-tauri/src/reinstall_commands.rs`
- Modify: `src-tauri/src/mod_import_commands.rs`
- Modify: `src-tauri/src/state.rs`
- Modify: `src-tauri/src/task_events.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `docs/FRONTEND_BACKEND_CONTRACT.md`
- Test: DTO serialization、command parser/error、AppState wiring tests

若现有 `dto.rs` 只需共享小类型则 re-export；CL3 DTO 保持 feature-local 文件，避免继续膨胀单文件。

### RED

DTO serialization tests 锁定：

- `PreviewReinstallPlanRequestDto` / `StartReinstallTaskRequestDto` 为 camelCase。
- revision/preview/count/reason DTO 不含 path、backup ref、hash、manifest/source content。
- preview DTO 按 `status` 序列化为 discriminated union：ready 的 installed/candidate/token 均非空且
  reasons 为空；blocked 的 token 为 null，revision 可空，`candidate_not_found` 的 candidate 为 null。
- start response 为 `{ taskId, kind: "install", status: "queued" }`。
- queued event 为 `install.reinstall.queued`；runner events 保持 task identity。
- invalid IDs、preview unavailable、start unavailable 使用稳定 code 和 sanitized message。
- task error serialization 覆盖 `install_reinstall_failed:post_commit`，且不把它映射为 rolled_back。

Command tests 锁定：

- `preview_reinstall_plan` 与 `start_reinstall_task` 只 parse -> app -> map。
- start 发送 queued 后 spawn runner；command body 不做分类/FS/backup/manifest。
- `start_import_mod_revision_task` 只允许 picker archive + existing mod id，不自动合并。
- `get_mod_revisions` 只返回受控 revision summaries。
- AppState 注入真实 `ConfiguredReinstallExecutor` 与现有共享 write lock registry。

```powershell
cargo test -p hmm-tauri reinstall -- --nocapture
cargo test -p hmm-tauri dto
cargo test -p hmm-tauri install_commands
```

### GREEN

按 spec 第 14 节实现四个窄 command/query，并同步：

- command names、request/result DTO；
- `install.reinstall.*` phase table；
- stable error/blocking reason；
- committed_cleanup_pending/cleanup_pending 派生状态与 `post_commit` error/Audit 映射；
- archive path 仅属于受控 revision import，reinstall command 不接受任何 path；
- installed/display/candidate revision 的权威来源说明。

真实 composition 可以新增 `ConfiguredReinstallExecutor`，在内部解析 game root、catalog/source、
manifest/backup/recovery adapters；不要把 concrete infra 或 path 暴露给 app/Tauri DTO。

### 验收

```powershell
cargo test -p hmm-tauri reinstall
cargo test -p hmm-tauri dto
cargo test -p hmm-tauri state
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\check-doc-links.ps1
```

**Commit:** `feat(tauri): 暴露真正重装窄契约`

## Task 8：Frontend candidate revision、preview、confirm 与 task UI

**Files:**

- Modify: `src/features/mods/modImportApi.ts`
- Modify: `src/features/mods/modImportTypes.ts`
- Modify: `src/features/mods/modLibraryApi.ts`
- Modify: `src/features/mods/modLibraryTypes.ts`
- Create: `src/features/mods/modReinstallApi.ts`
- Create: `src/features/mods/modReinstallTypes.ts`
- Create: `src/features/mods/modReinstallTaskState.ts`
- Create: `src/features/mods/ReinstallPlanPreviewPanel.tsx`
- Create: `src/features/mods/ReinstallPlanPreviewPanel.css`
- Modify: `src/features/mods/CompactActionPanel.tsx`
- Modify: `src/features/mods/ModLibraryPage.tsx`
- Modify: relevant `.test.mjs` files or create focused reinstall tests

### RED

先用 source-contract/helper tests 锁定：

- revision import 调 `start_import_mod_revision_task` 并传已有 `modId`。
- reinstall wrapper 只传 ids/layer/token，不传 target/delete/path/ref/manifest。
- `ready` preview 显示 retained/replaced/added/stale 四类聚合计数。
- blocked preview 按稳定 reason code 映射状态，不用 message 分支；`candidate_not_found` 可安全渲染 null
  candidate。
- frontend 先按 `status` narrowing；只有 ready branch 可读取 candidate/token 并启用确认，禁止对 blocked
  branch 使用非空断言。
- 只有 installed + candidate ready 才可 preview/confirm。
- rollback_required/repair_required/unknown/unavailable/preview stale 全部禁用确认。
- reinstall 不再调用普通 `startInstallTask`。
- listener 只匹配返回 taskId 与 `install.reinstall.*`。
- completed/failed/cancelled 都重新获取 revisions 与 manifest/recovery status。
- post_commit failure refetch 后显示 candidate 已提交但 cleanup pending，保持写入动作禁用，且不提供
  rollback v1 快捷动作。

```powershell
cmd /c corepack pnpm run test
cmd /c corepack pnpm run typecheck
```

### GREEN

- 保持一张 logical Mod 卡，清楚区分 installed/display/candidate revision。
- revision import 使用现有 picker 与 import task UI；不要从 metadata/name 猜 owner。
- 新建清晰的 reinstall discriminated state，不把它塞回 `install | uninstall` union。
- 使用独立 `ReinstallPlanPreviewPanel`；不让普通 `InstallPlanPreviewPanel` 承担两套语义。
- icon、focus trap、disabled/tooltip、modal keyboard 行为复用现有组件模式。
- 不做分页、主题、卡片布局或详情页无关重构，不增加依赖/lockfile 变更。

### 验收

```powershell
cmd /c corepack pnpm run test
cmd /c corepack pnpm run typecheck
cmd /c corepack pnpm run lint
cmd /c corepack pnpm run build
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\check-frontend-boundaries.ps1
```

用浏览器或实际 Tauri 在至少现有桌面基线宽度检查 ready、blocked、running、failed、completed 状态；
记录实际检查的 viewport/state，不能只凭 typecheck 声称视觉通过。

**Commit:** `feat(frontend): 接入真正重装版本工作流`

## Task 9：L2 AppState v1 -> v2 -> restart -> uninstall -> baseline

**Files:**

- Modify: `src-tauri/src/state_core_mod_lifecycle_tests.rs`
- Modify: `docs/CORE_MOD_LIFECYCLE_CL0_ACCEPTANCE.md`（只记录已执行证据）
- Modify: `docs/TESTING.md`（把 planned 聚焦命令改为实际命令）

### RED

扩展固定人工 fixture，不复制生产逻辑：

1. import v1 创建 logical Mod，install 并 restart 为 installed v1。
2. revision import v2 到同一 `modId`；library 仍一张卡、revision list 为 2。
3. preview 为 1 retained / 2 replaced / 1 added / 1 stale。
4. run reinstall，断言精确 v2 bytes、stale 消失、original backup 保留、old entries 消失。
5. restart 后 installed revision 从 manifest+catalog 恢复为 v2，TaskManager 为空不影响状态。
6. uninstall 后新增文件消失、`overwritten.bin` 恢复游戏基线。
7. 再次 restart 为 not installed；game root 与 pre-v1 snapshot 逐字节一致。

新增第二个 L2 场景：manifest save failure -> 自动 rollback v1 -> restart 仍显示 installed v1，且 active
transaction/recovery 摘要不误报 completed。故障注入只留 test composition，不进入生产 AppState 开关。

```powershell
cargo test -p hmm-tauri state::core_mod_lifecycle_tests::headless_composition_reinstalls_v1_to_v2_and_restores_baseline -- --nocapture
```

### GREEN

只修复真实 composition/wiring 缺口。不得用手改 catalog/manifest JSON、direct-copy helper、测试专用
production command 或 TaskManager 状态伪造 installed revision。

### 验收

```powershell
cargo test -p hmm-tauri state::core_mod_lifecycle_tests
cargo test -p hmm-core reinstall
cargo test -p hmm-app reinstall
cargo test -p hmm-app reinstall_task
cargo test -p hmm-infra reinstall
cargo check --workspace
```

Task 9 收尾时，只有自动化真实通过后，才能把对应 CL3 L1/L2 matrix 行改为“通过”；L3 尚待
Task 10，Gate A 尚待 CL4。

**Commit:** `test(lifecycle): 验收 v1 到 v2 真正重装闭环`

## Task 10：Windows Sandbox L3、文档收敛与 CL3 closeout

**Files:**

- Modify: `docs/CORE_MOD_LIFECYCLE_CL0_ACCEPTANCE.md`
- Modify: `docs/CORE_MOD_LIFECYCLE_PRIORITY_PLAN.md`
- Modify: `docs/ROADMAP.md`（仅在实际证据需要同步时）
- Modify: `docs/TESTING.md`
- Modify: `TODO.md`（仅在用户 worktree 无冲突且证据完成时）
- Modify: `docs/FRONTEND_BACKEND_CONTRACT.md`（核对实际 contract）

### L3 前置

- L1/L2、frontend checks、workspace check/clippy 和完整 verify 已通过。
- 只在 Windows Sandbox/disposable account；人工 v1/v2 和 game root 位于唯一 TEMP root。
- AppData 必须属于 disposable 环境；不选择真实游戏、Mod、Steam userdata 或存档。
- 操作前记录 baseline bytes；任何路径归属不确定立即停止。

### L3 工作流

1. 配置人工 MHW:I-like game root。
2. 普通导入 v1、preview/install、关闭并重开，确认 installed v1。
3. 对同一 Mod 导入 v2 revision，确认仍是一张卡且可选 candidate。
4. preview 确认四类计数 1/2/1/1；执行真正重装。
5. 关闭并重开，确认 installed v2 与动作可用性来自持久化状态。
6. 卸载，确认 game root 回到 pre-v1 baseline；再次重开为 not installed。
7. 导出支持诊断，检查只含受控 id/count/phase/result/error，无 path/ref/content。
8. 退出应用，执行 TEMP containment 校验后清理；Sandbox AppData 随环境销毁。

### 缺口处理

若 L3 暴露问题：

- 先写能复现的 L1/L2 测试；
- 创建独立最小修复提交；
- 重跑相关聚焦、完整 verify 和完整 L3；
- 不在 closeout 文档提交中夹带产品修复。

### 完整验证

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify.ps1
git diff --check
git status --short
```

然后使用 `hmm-review-gate` findings-first 自审：身份/ownership、source preload、write ordering、manifest
commit point、backup lifetime、rollback/crash recovery、shared lock、cancellation barrier、DTO/Audit redaction、
fixture containment、artifact hygiene 和文档状态。

只有 L3 真实执行、清理与本地 review 全部完成后，才能把 CL3 标为 `implemented`。Gate A 仍保持
未认证，并创建下一项 CL4 独立 review/certification 任务；不得在本 Task 顺手认证。

**Commit:** `docs(lifecycle): 记录 CL3 真正重装验收证据`

## 完成定义

- [x] Task 0 基线清洁，contract 无歧义。
- [x] Task 1 pure classifier/entry-set replacement 通过。
- [x] Task 2 catalog v2 migration 与 revision import 通过。
- [x] Task 3 manifest revision/recovery transaction repository 通过。
- [x] Task 4 preview/preflight/token 零写入测试通过。
- [x] Task 5 happy path、failure matrix、rollback/cleanup 测试通过。
- [x] Task 6 task/shared lock/cancellation/recovery/Audit 测试通过。
- [x] Task 7 Tauri DTO/commands/AppState/contract 通过。
- [x] Task 8 frontend revision/reinstall workflow 与视觉状态通过。
- [x] Task 9 L2 `v1 -> v2 -> restart -> uninstall -> baseline` 通过。
- [x] Task 10 L3 Windows Sandbox、diagnostics、cleanup 与完整 verify 通过。
- [x] CL3 标记 implemented，CL4/Gate A 仍未提前认证。
- [x] 没有进入 ARMOR_RETARGET、P7.2c、分页、批量迁移或 revision GC。
