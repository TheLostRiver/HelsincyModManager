# Core Mod Lifecycle CL0 验收基线

- 日期：2026-07-12
- 状态：CL0、CL1、CL2 已完成；Gate A 尚未认证
- 上游决策：[核心 Mod 生命周期优先级计划](CORE_MOD_LIFECYCLE_PRIORITY_PLAN.md)
- CL1 实施记录：[安装/卸载纵向闭环实施计划](superpowers/plans/2026-07-12-core-mod-lifecycle-cl1-implementation.md)
- CL3 设计：[真正重装 contract/spec](superpowers/specs/2026-07-14-core-mod-lifecycle-cl3-true-reinstall-design.md)
- CL3 实施入口：[真正重装实施计划](superpowers/plans/2026-07-14-core-mod-lifecycle-cl3-true-reinstall-implementation.md)
- 下一切片：CL3 真正重装 contract 与实现

## 1. 目的与边界

本文固定 Core Mod Lifecycle 的人工 fixture、证据层级、可执行验收矩阵、当前 composition 缺口和
Tauri 桌面 smoke 安全步骤。它回答“现在哪条链能真实复用、下一步缺什么”，不把已有单元测试
误报为完整玩家闭环。

CL0 已新增 test-only AppState composition harness，覆盖：

```text
synthetic zip
  -> ModImportTaskService / ModImportTaskRunner
  -> controlled sandbox + persisted import analysis
  -> MHW:I adapter allowed roots
  -> InstallPlanningService
  -> InstallPlan
  -> drop AppState
  -> recreate AppState from the same temp AppData
  -> rebuild the same plan from persisted facts
```

CL0 不执行 install/uninstall game writes，不实现 package revision 更新、真正重装、ARMOR_RETARGET、
新 command、前端行为或通用测试框架。CL0 完成不代表 Gate A `certified`。

## 2. 事实基线

| 能力 | 当前证据 | 结论 |
| --- | --- | --- |
| Headless composition | `AppState::from_app_data_dir(temp)` | 可复用真实 SQLite、JSON repositories、infra adapters 和 task services，且不启动 GUI maintenance thread |
| 游戏目录配置 | `GameSetupService::save_game_directory` + real probe + MHW adapter | temp game root 只需人工 `MonsterHunterWorld.exe`；允许安装根为 `nativePC` |
| Mod 导入 | synthetic zip -> real importer -> sandbox -> JSON analysis | 可用；`mod_id == package_id == import task_id` |
| 安装计划 | persisted analysis + sandbox scanner + MHW adapter -> `InstallPlan` | 可用；只消费 `nativePC` 下普通相对文件 |
| 重启事实恢复 | drop/recreate AppState | game config、import library 和 plan 可恢复；TaskManager 状态按设计不持久化 |
| 安装/卸载服务 | L2 AppState composition 与 L3 disposable desktop smoke 已通过 | CL1/CL2 已完成；Gate A 仍等待 CL3/CL4 |
| 真正重装 | UI `reinstall` 复用普通 install；manifest merge 保留未触达旧 entry | 未实现，归 CL3 |
| package revision | 每次 import 生成新 task/package/mod id | 未实现“同一稳定 Mod 身份绑定新 package revision”，归 CL3 |
| 桌面隔离 | Windows Sandbox + 人工 TEMP game/archive + disposable AppData | CL2 已执行并清理；日常账户/真实游戏仍不在验收范围 |

## 3. 固定 Fixture Contract

所有内容均为 ASCII 人工字节，不包含第三方 Mod、真实游戏资源、真实 hash 或玩家数据。

### 3.1 环境布局

```text
<temp>/
  app-data/                  # SQLite、config、import、install facts、logs
  game/
    MonsterHunterWorld.exe  # 人工占位普通文件，仅用于目录验证
    nativePC/lifecycle/
      overwritten.bin       # 安装前已有基线文件
  lifecycle-v1.zip
  lifecycle-v2.zip
```

安装前 `nativePC/lifecycle/overwritten.bin` 的精确字节为：

```text
game-baseline-original\n
```

### 3.2 v1 / v2 文件

| Target | 游戏基线 | v1 bytes | v2 bytes | `v1 -> v2` 分类 |
| --- | --- | --- | --- | --- |
| `nativePC/lifecycle/retained.bin` | 不存在 | `fixture-retained\n` | `fixture-retained\n` | retained |
| `nativePC/lifecycle/replaced.bin` | 不存在 | `fixture-replaced-v1\n` | `fixture-replaced-v2\n` | replaced |
| `nativePC/lifecycle/overwritten.bin` | `game-baseline-original\n` | `fixture-overwrite-v1\n` | `fixture-overwrite-v2\n` | replaced，且未来卸载必须恢复游戏基线 |
| `nativePC/lifecycle/stale.bin` | 不存在 | `fixture-stale-v1\n` | 不存在 | stale |
| `nativePC/lifecycle/added-v2.bin` | 不存在 | 不存在 | `fixture-added-v2\n` | added |

Fixture contract label 固定为 `core-lifecycle-fixture`，它不是当前运行时的 `mod_id`。当前 importer
不能把 v1/v2 映射到同一稳定 `mod_id`；自动化必须捕获真实 import task id。CL3 在实现 package
revision contract 前，不得用手改 `results.json` 的方式假装真正重装已可用。

## 4. 证据层级

| 层级 | 说明 | 是否可证明玩家闭环 |
| --- | --- | --- |
| L0 Contract | fixture 集合、状态词汇、矩阵和安全停止条件 | 否 |
| L1 Focused | core/app/infra/DTO/frontend 的 fake/temp 聚焦测试 | 只能证明局部规则 |
| L2 Composition | temp AppData + temp game root + real AppState/infra adapters | 可证明自动化纵向链，但不证明窗口/文件选择器/event bridge |
| L3 Desktop | disposable account/VM 中运行实际 Tauri 应用 | 可证明桌面工作流；仍不等于真实玩家日常目录验收 |

Gate A 必须同时消费 L1、L2 和实际执行的 L3 证据。单独一层不能标记 `certified`。

## 5. 自动化验收矩阵

| ID | 场景 | 当前状态 | 证据/下一负责人 |
| --- | --- | --- | --- |
| CL0-F1 | v1/v2 retained/replaced/added/stale 分类固定 | 通过 | `fixture_contract_covers_reinstall_target_classes` |
| CL0-C1 | synthetic v1 zip 经 real importer 持久化 | 通过 | CL0 AppState harness |
| CL0-C2 | persisted import + MHW adapter 重建 4-action InstallPlan | 通过 | CL0 AppState harness |
| CL0-C3 | drop/recreate AppState 后 game config/library/plan 一致 | 通过 | CL0 AppState harness |
| CL0-C4 | CL0 plan-only harness 不修改 temp game root | 通过 | 覆盖基线字节与新增 target 不存在断言 |
| CL1-C1 | v1 install：3 个新增文件 + 1 个覆盖文件 | 通过 | `headless_composition_installs_restarts_uninstalls_and_restores_baseline` |
| CL1-C2 | 安装后 restart：manifest/recovery 状态为 installed | 通过 | 同一 L2 composition；4 managed files、1 backup、0 issues |
| CL1-C3 | uninstall：新增文件删除、覆盖文件恢复、再重启为 not installed | 通过 | 同一 L2 composition；manifest 驱动卸载 |
| CL1-C4 | 最终 game root 与安装前基线逐字节一致 | 通过 | 同一 L2 composition；相对文件路径与精确人工 bytes 快照 |
| CL1-F1 | source read failure 零误报/可恢复 | 通过 | `commit_plan_aborts_without_writes_when_source_read_fails`；零 game writes / manifest saves |
| CL1-F2 | backup store failure 在任何 target write 前阻断 | 通过 | `commit_plan_aborts_before_target_write_when_backup_store_fails`；前缀 pending backup 清理 |
| CL1-F3 | game write / manifest save / rollback failure | 已有聚焦证据 | CL1 复用并建立证据映射 |
| CL1-S1 | drift、missing summary、missing backup fail closed | 已有聚焦证据 | CL1 复用并建立证据映射 |
| CL1-A1 | install/uninstall success Audit Log 与公开证据脱敏 | 通过 | 同一 L2 composition；只含 task/game/mod/profile 与动作/删除/恢复计数 |
| CL2-D1 | 文件选择器 -> import -> preview -> install -> restart -> uninstall | 通过 | Windows Sandbox 实际 Tauri smoke；4 actions / 0 blocking conflicts / baseline restored |
| CL3-I1 | schema v1 导入记录迁移为稳定 logical Mod + revision catalog | 阻断 | CL3 Task 2；规划见真正重装 spec/plan |
| CL3-P1 | 四类 ReinstallPlan、全量 preflight 与零写入阻断 | 阻断 | CL3 Task 1/4；预期 1 retained / 2 replaced / 1 added / 1 stale |
| CL3-R1 | v1 -> v2 -> restart -> uninstall -> baseline | 阻断 | CL3 Task 5/9；必须保留 original backup 并移除 old/stale entries |
| CL3-F1 | source/backup/write/delete/manifest/rollback failure 回到 v1 | 阻断 | CL3 Task 3/5；部分恢复只保留未恢复 durable facts |
| CL3-T1 | shared lock、task phase/cancellation barrier 与 Audit 脱敏 | 阻断 | CL3 Task 6/7；install/uninstall/reinstall/recovery 同锁 |
| CL3-D1 | Windows Sandbox revision import -> reinstall -> restart -> uninstall | 阻断 | CL3 Task 10；只用人工 TEMP fixture 与 disposable AppData |

聚焦执行入口：

```powershell
cargo test -p hmm-tauri state::core_mod_lifecycle_tests
```

CL3 各行仍是预期证据，不是当前已通过测试。正式 contract 与测试/清理矩阵见
[CL3 真正重装设计](superpowers/specs/2026-07-14-core-mod-lifecycle-cl3-true-reinstall-design.md)，
逐提交执行顺序见
[CL3 真正重装实施计划](superpowers/plans/2026-07-14-core-mod-lifecycle-cl3-true-reinstall-implementation.md)。

## 6. 当前缺口清单

### G1：AppState 安装/卸载纵向 acceptance（已解决）

CL1 Task 1 已扩展同一 CL0 harness，使用真实 filesystem/JSON/SQLite composition 完成 v1 install
-> restart -> uninstall -> restart -> baseline。成功 commit 会先持久化 `completed` 关闭崩溃窗口，
再 best-effort 清理 active recovery record；重启状态来自 manifest、目标摘要和 backup 事实，不依赖
旧 TaskManager 状态或另造安装链路。

### G2：两个直接 fault tests（已解决）

CL1 已在 app service recording fakes 中直接注入第二个 source read 与第二次 `store_backup` failure，
证明完整 source preload 和 backup prepare 成功前不会发生任何 target mutation 或 manifest save；已创建的
pending backup 和 planned recovery facts 会安全清理。Fault injection 没有进入生产 AppState。

### G3：稳定 Mod 身份与 package revision 未建模

当前 import task id 同时充当 package id 与 mod id，v2 import 会成为另一个 Mod。CL3 必须先定义：

- 稳定 logical Mod id；
- 当前/候选 package revision 绑定；
- 新 revision 的 sandbox/source 生命周期；
- 成功、失败或取消后 revision 指针如何提交/回滚。

没有该 contract，`start_reinstall_task` 即使存在也无法表达“同一 Mod 从 v1 更新到 v2”。

### G4：真正重装 use case 未实现

当前 UI 的“安装 / 重装”调用普通 `start_install_task`，manifest merge 会保留新 plan 未触达的旧
entry。retained/replaced/added/stale 分类、preflight、reinstall recovery facts 和 entry-set replacement
均归 CL3。

### G5：桌面 AppData 没有测试隔离入口

GUI `AppState::new` 固定使用 Tauri OS AppData。CL2 在新增受控 dev/test override 前，不得在维护者
日常账户执行会写 app data 的 lifecycle smoke；当前可执行路径是 disposable Windows account/VM。

### G6：TaskManager restart 后为空是预期行为

Task 状态是页面/进程内短期事实。重启后的安装状态必须来自 manifest/recovery query，不得要求
TaskManager 持久化，也不得用 Task Log 代替 manifest。

## 7. Tauri 桌面 Smoke（CL2 执行）

### 7.1 硬前置

- 只在一次性 Windows 本地账户或 disposable VM 执行。
- 不选择真实 MHW:I 目录、真实 Mod、Steam userdata 或玩家存档。
- smoke 前完整验证通过；本地应用可用 `cmd /c corepack pnpm tauri dev` 启动。
- 任何步骤出现非临时路径、无法确认 AppData 所属账户、异常后台任务或无法清理状态，立即停止。

CL2 已于 2026-07-13 在 Windows Sandbox 中执行；使用人工 TEMP game/archive 和 disposable AppData，
未接触真实 MHW:I、真实 Mod、Steam userdata 或玩家存档。

### 7.2 创建人工输入

在 disposable 环境的 PowerShell 中创建唯一 temp root。以下内容只生成 v1；v2 留给 CL3：

```powershell
$smokeRoot = Join-Path $env:TEMP ("hmm-core-lifecycle-smoke-" + [guid]::NewGuid())
$gameRoot = Join-Path $smokeRoot "game"
$fixtureRoot = Join-Path $smokeRoot "fixture-v1"
$archivePath = Join-Path $smokeRoot "lifecycle-v1.zip"

New-Item -ItemType Directory -Force -Path (Join-Path $gameRoot "nativePC\lifecycle") | Out-Null
New-Item -ItemType Directory -Force -Path (Join-Path $fixtureRoot "nativePC\lifecycle") | Out-Null

[IO.File]::WriteAllBytes((Join-Path $gameRoot "MonsterHunterWorld.exe"), [Text.Encoding]::ASCII.GetBytes("fixture executable`n"))
[IO.File]::WriteAllBytes((Join-Path $gameRoot "nativePC\lifecycle\overwritten.bin"), [Text.Encoding]::ASCII.GetBytes("game-baseline-original`n"))
[IO.File]::WriteAllBytes((Join-Path $fixtureRoot "nativePC\lifecycle\overwritten.bin"), [Text.Encoding]::ASCII.GetBytes("fixture-overwrite-v1`n"))
[IO.File]::WriteAllBytes((Join-Path $fixtureRoot "nativePC\lifecycle\replaced.bin"), [Text.Encoding]::ASCII.GetBytes("fixture-replaced-v1`n"))
[IO.File]::WriteAllBytes((Join-Path $fixtureRoot "nativePC\lifecycle\retained.bin"), [Text.Encoding]::ASCII.GetBytes("fixture-retained`n"))
[IO.File]::WriteAllBytes((Join-Path $fixtureRoot "nativePC\lifecycle\stale.bin"), [Text.Encoding]::ASCII.GetBytes("fixture-stale-v1`n"))

Compress-Archive -LiteralPath (Join-Path $fixtureRoot "nativePC") -DestinationPath $archivePath
```

### 7.3 工作流

1. 启动实际 Tauri 应用，配置 `$gameRoot`；确认 UI 识别为 MHW:I 测试目录。
2. 通过系统文件选择器导入 `$archivePath`；记录公开 task id、phase 和最终 library item，不记录绝对路径。
3. 打开安装计划预览；预期 4 个 actions、0 blocking conflicts，target 均为 `nativePC/lifecycle/*`。
4. 确认安装；预期任务依次进入 queued/building/processing/completed。
5. 关闭并重启应用；预期 library item 从 manifest/recovery 恢复为 installed，不依赖旧 task state。
6. 确认卸载；预期 3 个新增文件删除、`overwritten.bin` 恢复基线字节。
7. 再次重启；预期状态为 not installed，恢复扫描无不安全 issue。
8. 检查 UI/日志证据只包含短 id、phase、计数和稳定错误码，不显示 root、backup ref 或 manifest 正文。

当前“安装 / 重装”按钮不得用于 v1 -> v2 验收；真正重装留给 CL3。

### 7.4 清理

先确认应用已退出、卸载已完成、游戏基线已恢复。只删除受控 temp root：

```powershell
$resolvedSmokeRoot = [IO.Path]::GetFullPath($smokeRoot)
$resolvedTempRoot = [IO.Path]::GetFullPath($env:TEMP)
$tempPrefix = $resolvedTempRoot.TrimEnd([IO.Path]::DirectorySeparatorChar, [IO.Path]::AltDirectorySeparatorChar) + [IO.Path]::DirectorySeparatorChar
$smokeLeaf = [IO.Path]::GetFileName($resolvedSmokeRoot)
if (-not $resolvedSmokeRoot.StartsWith($tempPrefix, [StringComparison]::OrdinalIgnoreCase) -or
    -not $smokeLeaf.StartsWith("hmm-core-lifecycle-smoke-", [StringComparison]::Ordinal)) {
    throw "refusing to remove a smoke root outside TEMP"
}
Remove-Item -LiteralPath $resolvedSmokeRoot -Recurse -Force
```

AppData 清理由 disposable account/VM 销毁完成；不要为了 smoke 手工递归删除日常账户 AppData。

### 7.5 执行结果（2026-07-13）

- 实际 Tauri 应用通过系统文件选择器导入四文件 v1 人工 ZIP；计划预览为 4 actions、0 blocking conflicts。
- CL2 首轮暴露两个阻断：前端缺少 archive picker/import task 入口；Windows mapped-folder rename 已生效后父目录 sync 报错时，app rollback 遗漏当前 change 并过早删除 pending backup。
- Review follow-up 覆盖混合回滚结果：成功恢复的项先从 durable recovery record 移除，再清理对应 pending backup；`rollback_required` 只保留真正未恢复的项，避免后续受控恢复被已恢复目标阻断。
- 最小修复后安装完成；重启从 manifest/recovery 恢复 installed，卸载删除 3 个新增文件并恢复 1 个覆盖文件，再次重启为 not installed 且无 rollback/repair/unknown 状态。
- 主机在安装态和卸载态分别校验人工文件长度/hash；最终 game root 与安装前两文件 baseline 逐字节一致。
- 支持诊断包只包含四个固定 JSON 条目；App/Task 日志为 0 行，两条 install Audit 仅含短 id 和动作/删除/恢复计数，敏感路径模式命中 0。
- 应用和 Sandbox 退出后，旧失败现场与新通过现场的唯一 TEMP roots 均在 containment 校验后删除；Sandbox AppData 随 VM 销毁。
- 本次没有执行真实游戏 smoke，也没有把现有“安装 / 重装”按钮当作 v1 -> v2 证明；真正重装仍归 CL3。

## 8. CL0 完成定义

- [x] v1/v2 fixture contract 固定并由 test 锁定分类。
- [x] test-only AppState import/plan/restart harness 通过。
- [x] 当前 composition、证据层级和缺口已记录。
- [x] 桌面 smoke 有安全前置、步骤、停止条件和清理边界。
- [x] CL1 最小实施计划已创建。
- [x] CL1 install/uninstall acceptance 通过。
- [x] CL1 source read / backup store fault tests 通过。
- [x] CL1 manifest/recovery/Audit Log/task phase 与脱敏证据通过。
- [x] CL2 桌面 smoke 实际执行。
- [ ] CL3 真正重装实现并通过。

CL2 桌面 smoke 已完成；CL3 真正重装和 CL4 最终复审仍未完成，Gate A 必须保持未认证。
