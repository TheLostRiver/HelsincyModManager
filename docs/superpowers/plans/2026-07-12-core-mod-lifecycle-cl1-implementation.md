# Core Mod Lifecycle CL1 安装/卸载纵向闭环实施计划

> **执行说明：** 本计划消费
> [CL0 验收基线](../../CORE_MOD_LIFECYCLE_CL0_ACCEPTANCE.md) 的 test-only AppState harness。
> CL1 只认证 v1 install/restart/uninstall/baseline；不得实现 v2 package revision、真正重装或
> ARMOR_RETARGET。

**Goal:** 使用人工 v1 zip、temp AppData 和 temp MHW:I-like game root，让真实 AppState composition
完成 import -> InstallPlan -> install -> restart -> manifest/recovery status -> uninstall -> baseline，
并补齐 source read / backup store failure 的直接聚焦测试。

**Architecture:** L2 happy path 继续放在 `hmm-tauri` state sibling test module，复用真实 importer、
SQLite/JSON repositories、MHW adapter、filesystem install adapters、task runners 和 write locks。L1
fault injection 留在 `hmm-app` service tests，不向生产 AppState 注入故障开关。Tauri window/event/file
picker 属 CL2，不在本计划中模拟。

**Safety:** 所有写入只发生在 `tempfile::TempDir` 下；不使用真实游戏、真实 Mod、Steam userdata、
玩家存档或第三方内容。卸载必须从 manifest + installed summary + backup 事实执行。

---

## Task 1：扩展 AppState happy-path lifecycle acceptance

**Files:**

- Modify: `src-tauri/src/state_core_mod_lifecycle_tests.rs`
- Modify: `docs/CORE_MOD_LIFECYCLE_CL0_ACCEPTANCE.md`

### RED

新增测试 `headless_composition_installs_restarts_uninstalls_and_restores_baseline`，先复用 CL0 helper
完成 temp game setup 与 v1 import，然后断言尚未实现的完整步骤：

1. `install_tasks.start_install_task` 返回 queued install task。
2. `install_task_runner.run_install_task` 返回 building -> processing -> completed。
3. game root 中 3 个新增 target 为 v1 bytes，`overwritten.bin` 为 v1 bytes。
4. `<AppData>/install/manifests/default.json` 存在 4 entries；覆盖 entry 有 backup ref；recovery record
   已清理。
5. drop/recreate AppState 后，用 recovery scan/manifest query 得到 installed，而不是读取旧 task state。
6. `uninstall_tasks.start_uninstall_task` + runner 完成后，3 个新增 target 消失，覆盖 target 恢复
   `game-baseline-original\n`。
7. 再次 restart 后为 not installed，game root 与安装前 baseline snapshot 逐字节一致。

运行并观察 RED：

```powershell
cargo test -p hmm-tauri state::core_mod_lifecycle_tests::headless_composition_installs_restarts_uninstalls_and_restores_baseline -- --nocapture
```

### GREEN

只修复阻断既有 composition 的真实缺口。允许修改范围按优先级：

1. test harness/helper；
2. 当前 slice 必需的窄 app/infra composition bug；
3. 只有 contract 真实变化时才同步 DTO/docs。

不得引入 direct-copy、测试专用生产 command、宽泛 filesystem API 或全局 mutable test mode。

### 验收

- 所有 progress event 携带同一 task id。
- import/plan 不持有 game/profile write lock；commit/uninstall 复用同一 write lock registry。
- 安装前后只比较相对路径、精确人工 bytes、size/SHA-256 与状态，不记录 temp absolute path。
- TaskManager restart 后为空是预期；manifest/recovery 是安装事实。

---

## Task 2：补 source read 与 backup store failure

**Files:**

- Modify: `src-tauri/crates/hmm-app/src/install_tests.rs`

### RED

新增两个聚焦测试：

1. `commit_plan_aborts_without_writes_when_source_read_fails`
2. `commit_plan_aborts_before_target_write_when_backup_store_fails`

扩展既有 recording fakes：

- source reader 可按 `PackageFileId` 注入 read failure；
- backup store 可在 `store_backup` 注入 failure；
- 暴露只读计数/记录，证明失败顺序。

预期：

| 故障 | error phase | game writes | manifest save | recovery result |
| --- | --- | --- | --- | --- |
| source read | `SourceRead` | 0 | 0 | 不产生误导 completed |
| backup store | `Backup` | 0 | 0 | 已建 planned facts 必须安全清理或进入明确状态 |

运行并观察 RED：

```powershell
cargo test -p hmm-app commit_plan_aborts_without_writes_when_source_read_fails
cargo test -p hmm-app commit_plan_aborts_before_target_write_when_backup_store_fails
```

### GREEN

优先只扩展 test doubles 和断言。若测试暴露生产 service 顺序不满足“先完整 source read / backup，后
target mutation”，才做最小实现修复，并补 recovery state 断言。

---

## Task 3：锁定状态、审计与脱敏证据

**Files:**

- Modify: `src-tauri/src/state_core_mod_lifecycle_tests.rs`
- Modify: `docs/CORE_MOD_LIFECYCLE_CL0_ACCEPTANCE.md`

在 Task 1 happy path 上补断言：

- install success Audit Log 只含 task/game/mod/profile/action count；
- uninstall success Audit Log 只含 removed/restored counts；
- serialized public evidence 不含 game root、AppData、sandbox、backup ref 或 manifest 正文；
- restart recovery summary 的 managed file/backup counts 与 fixture 一致；
- uninstall 后 recovery summary 为 not installed 且无 stale recovery record。

不要把读取原始 Audit Log 的能力暴露给前端；测试可使用 infra reader或读取受控 temp 日志，最终
只断言白名单字段与禁入字符串。

---

## Task 4：同步 CL1 状态并完成验证

**Files:**

- Modify: `TODO.md`
- Modify: `docs/CORE_MOD_LIFECYCLE_PRIORITY_PLAN.md`
- Modify: `docs/CORE_MOD_LIFECYCLE_CL0_ACCEPTANCE.md`
- Modify: `docs/INSTALL_PLAN_STATUS.md`
- Modify: `docs/INSTALL_PLAN_MVP_TODO.md`
- Modify: `docs/TESTING.md`

只有 Task 1-3 全部通过后，才能把 CL1 标为 complete。CL2 桌面 smoke、CL3 真正重装和 Gate A
仍保持未完成。

聚焦验证：

```powershell
cargo test -p hmm-tauri state::core_mod_lifecycle_tests
cargo test -p hmm-app install
cargo test -p hmm-app install_task
cargo test -p hmm-infra install_commit
cargo check --workspace
```

完整验证与 review gate：

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify.ps1
```

Review 必须检查：temp containment、fixture 人工性、write ordering、manifest/backup/recovery 事实、
Audit Log 脱敏、task id、锁边界、restart 语义和 repository hygiene。

## 完成定义

- [ ] v1 L2 install/restart/uninstall/baseline acceptance 通过。
- [ ] source read 与 backup store fault tests 通过。
- [ ] 既有 write/manifest/rollback/drift tests 映射到 CL1 矩阵。
- [ ] Audit/status evidence 不含敏感路径或内容。
- [ ] 完整 `verify.ps1` 与本地 review gate 通过。
- [ ] CL2/CL3 仍未被误报为完成。
