# Windows 安装器 Owned Task 卸载清理（P7.2c）实施计划

> **执行说明：** 本计划当前只定义后续工作，不代表 helper、NSIS hook、WiX custom action 或
> runtime gate 已实现。执行时按 Task 顺序小步提交；每个 RED 必须先观察预期失败，再写 GREEN。

**目标：** 为 NSIS/WiX 真正产品卸载提供同一个无参数 cleanup helper，只清理当前用户的 HMM
owned Scheduled Task，保留 foreign task，阻断正在运行或无法确认的 cleanup，并完成两个
installer 的独立 disposable VM gate。

**设计规格：**
[Windows 安装器 Owned Task 卸载清理设计](../specs/2026-07-12-save-backup-installer-cleanup-design.md)

**基线：** P7.2a registry/runner/sidecar 与 P7.2b 用户流程已经存在；P7.2a 安装态 runtime
acceptance 尚未完成。P7.2c 不等待该验收才开始实现，但两个 gate 必须分别记录，不能互相替代。

## 执行护栏

- 自动化不得创建、更新、启动、停止或删除真实 Scheduled Task。
- 不读取真实游戏目录、Steam userdata、玩家存档或真实备份目录。
- 不修改 worker 固定 `--once` contract，不调用 Settings `disable()`。
- 不向 helper/installer 暴露 task name、SID、owner marker、path、PowerShell、XML 或 timeout 参数。
- foreign task 必须保留且允许卸载继续；owned drift 可清理。
- running/queued task 不得强杀；真正卸载必须阻断。
- 升级、repair、modify 必须跳过 cleanup。
- NSIS 与 WiX 都必须先完成生成模板 spike，再写 lifecycle glue。
- `.planning/`、bundle、target-triple sidecar、installer 输出和 VM fixture 不提交。

---

### Task 1: 实现 Rust installer cleanup contract 与窄 helper

**Files:**

- Modify: `src-tauri/crates/hmm-infra/src/save_backup_background_registry/mod.rs`
- Modify: `src-tauri/crates/hmm-infra/src/save_backup_background_registry/registry.rs`
- Modify: `src-tauri/crates/hmm-infra/src/save_backup_background_registry/task_spec.rs`
- Modify: `src-tauri/crates/hmm-infra/src/save_backup_background_registry/powershell.rs`
- Modify: `src-tauri/crates/hmm-infra/src/save_backup_background_registry/windows.rs`
- Modify: `src-tauri/crates/hmm-infra/src/save_backup_background_registry/tests.rs`
- Create: `src-tauri/src/installer_cleanup.rs`
- Create: `src-tauri/src/bin/hmm-save-backup-installer-cleanup.rs`
- Modify: `src-tauri/src/lib.rs`

**Interfaces:**

- Produces: `InstallerCleanupOutcome` with removed/absent/foreign/busy/unverified variants.
- Produces: one no-argument helper binary and stable exit mapping `0/20/21/22/23/64`.
- Reuses: current identity、task name derivation、`TASK_OWNER_MARKER`、controlled runner、owned delete
  and post-delete read-back.
- Preserves: ordinary `SaveBackupBackgroundRegistry::unregister()` behavior and worker `--once` CLI.
- Does not depend on: Tauri runtime、AppState、SQLite、Audit Log、AppData、worker file or save paths.

- [ ] **Step 1: 写 fake runner cleanup RED 测试**

在 infra tests 中先覆盖：

```text
missing -> AlreadyAbsent, no mutation
owned exact Ready -> Removed, delete then missing read-back
owned drift Disabled -> Removed
foreign -> ForeignPreserved, no mutation
Running / Queued -> OwnedTaskRunning, no delete
Ready first inspect -> Running mutation precheck -> OwnedTaskRunning, no delete
permission/module/timeout/invalid output/unknown state -> fail closed
delete failure/post-delete owned/post-delete foreign -> stable unverified outcome
```

测试必须检查完整 command sequence，证明 foreign/busy/unverified 分支没有 unregister mutation。

- [ ] **Step 2: 写 helper exit mapping RED 测试**

在 `installer_cleanup.rs` 中锁定：

```text
Removed / AlreadyAbsent / ForeignPreserved -> 0
OwnedTaskRunning -> 20
OwnershipUnverified -> 21
RemovalUnverified -> 22
PlatformUnavailable -> 23
any CLI argument -> 64
```

同时断言 helper 不把内部 error/debug 文本写入返回 contract。

- [ ] **Step 3: 运行 RED**

Run:

```powershell
cargo test -p hmm-infra save_backup_installer_cleanup
cargo test -p hmm-tauri installer_cleanup
```

Expected: FAIL，原因是 outcome、runner command、state 字段和 helper module 尚不存在；失败不能来自
真实 PowerShell 调用。

- [ ] **Step 4: 增加 installer-specific typed cleanup**

实施最小变化：

1. read-back 白名单增加 typed task state；只接受已知 state。该 state 只供 installer quiescence
   判断，不进入普通 exact-registration spec 比较，避免 running task 被误报为 configuration drift。
2. 新增 installer-specific command/flow，在 mutation 前再次比较固定 marker 与 state。
3. `Running`/`Queued` 返回 busy，不调用 `Stop-ScheduledTask`。
4. marker 不匹配返回 `ForeignPreserved`，不复用普通 unregister 的 error-to-block mapping。
5. owned exact/drift 共用同一 ownership-checked delete。
6. post-delete 必须 missing；出现 foreign 时保留且 fail closed，不做第二次删除。
7. 保留 runner 的系统 PowerShell 绝对路径、module manifest、15 秒 timeout、64 KiB stdout 上限和
   stderr 丢弃规则。

如果无法在受控 PowerShell mutation 内完成“marker + state 复核后删除”，停止 Task 1 并回到设计
review，不得用 Rust 的早期 inspect 代替 mutation 前复核。

- [ ] **Step 5: 实现窄 binary**

binary 只读取 `std::env::args_os()` 是否存在额外参数，调用 infra cleanup，并映射退出码。它不
初始化 GUI/Tauri、不读取环境中的 task/path override、不输出原始错误。

- [ ] **Step 6: 运行 GREEN 与回归**

Run:

```powershell
cargo fmt --all -- --check
cargo test -p hmm-infra save_backup_installer_cleanup
cargo test -p hmm-infra save_backup_background_registry::tests
cargo test -p hmm-tauri installer_cleanup
cargo check -p hmm-tauri --bin hmm-save-backup-installer-cleanup
cargo check -p hmm-tauri --bin hmm-save-backup-worker
```

Expected: PASS；普通测试没有真实 Scheduled Task mutation；worker parser/registry 回归保持通过。

- [ ] **Step 7: Review diff 并提交**

```powershell
git diff --check
git status --short
git add src-tauri/crates/hmm-infra/src/save_backup_background_registry src-tauri/src/installer_cleanup.rs src-tauri/src/bin/hmm-save-backup-installer-cleanup.rs src-tauri/src/lib.rs
git commit -m "feat: add owned task installer cleanup helper"
```

---

### Task 2: 泛化 Windows sidecar 构建与 bundle 配置

**Files:**

- Create: `scripts/prepare-windows-sidecars.mjs`
- Create: `scripts/prepare-windows-sidecars.test.mjs`
- Delete: `scripts/prepare-save-backup-worker-sidecar.mjs`
- Delete: `scripts/prepare-save-backup-worker-sidecar.test.mjs`
- Modify: `package.json`
- Modify: `src-tauri/tauri.windows.conf.json`

**Interfaces:**

- Produces: worker 与 cleanup helper 两个 target-triple source binaries。
- Preserves: `cargo metadata` target directory、host/cross-target consistency、debug/release profile、
  GUI `default-run` 和 ignored generated directory。
- Preserves: inner Cargo build 使用局部 `TAURI_CONFIG` 清空全部 `externalBin`，避免自举递归。

- [ ] **Step 1: 写双 binary RED 测试**

把纯函数从单个 worker name 泛化为固定 allowlist，例如：

```text
hmm-save-backup-worker
hmm-save-backup-installer-cleanup
```

测试两个 Windows target-triple 文件名、非 Windows extension、非法 triple、重复/未知 binary、
missing build output、metadata target dir、debug/release 与 `TAURI_ENV_ARCH` 冲突。

- [ ] **Step 2: 运行 RED**

```powershell
node --test scripts/prepare-windows-sidecars.test.mjs
```

Expected: FAIL with `ERR_MODULE_NOT_FOUND`。

- [ ] **Step 3: 实现一次构建两个受控 bin**

脚本只能迭代内部固定清单，不能接受任意 binary/path 参数。每个 Cargo build 都使用同一 target、
profile 与局部 `externalBin: []`。复制前逐一确认 source 是普通文件，destination 仍在
`src-tauri/binaries/`。

- [ ] **Step 4: 更新 pnpm/Tauri 配置**

新增：

```text
prepare:windows-sidecars
prepare:windows-sidecars:dev
```

Windows `beforeDevCommand` / `beforeBuildCommand` 使用新命令，`bundle.externalBin` 精确包含两个
base names。删除旧单 worker scripts 后，仓库文档和命令不能继续引用旧文件名。

- [ ] **Step 5: 运行 GREEN 与产物检查**

```powershell
node --test scripts/prepare-windows-sidecars.test.mjs
cmd /c corepack pnpm run prepare:windows-sidecars:dev
cmd /c corepack pnpm run prepare:windows-sidecars
$hostTriple = ((rustc -vV | Select-String '^host:').Line -replace '^host:\s*', '')
git check-ignore "src-tauri/binaries/hmm-save-backup-worker-$hostTriple.exe"
git check-ignore "src-tauri/binaries/hmm-save-backup-installer-cleanup-$hostTriple.exe"
git status --short
```

Expected: 两个 source artifacts 均存在且 ignored；`git status` 不包含 binary。

- [ ] **Step 6: Review diff 并提交**

```powershell
git add package.json src-tauri/tauri.windows.conf.json scripts/prepare-windows-sidecars.mjs scripts/prepare-windows-sidecars.test.mjs scripts/prepare-save-backup-worker-sidecar.mjs scripts/prepare-save-backup-worker-sidecar.test.mjs
git commit -m "build: bundle installer cleanup sidecar"
```

---

### Task 3: 生成 NSIS 基线并接入 PREUNINSTALL hook

**Files:**

- Create: `src-tauri/windows/nsis-installer-hooks.nsh`
- Create: `scripts/windows-installer-cleanup-config.test.mjs`
- Modify: `src-tauri/tauri.windows.conf.json`

**Interfaces:**

- Uses: Tauri 2.11.2 `bundle.windows.nsis.installerHooks`。
- Runs: installed sibling helper inside `NSIS_HOOK_PREUNINSTALL`。
- Preserves: upgrade/repair/modify task；silent uninstall fail-closed。

- [ ] **Step 1: 用锁定 CLI 生成 NSIS 基线模板**

```powershell
cmd /c corepack pnpm tauri build --bundles nsis --debug
```

从 CLI 输出/生成目录找到实际 `installer.nsi`，记录以下证据：

- `NSIS_HOOK_PREUNINSTALL` 的确在文件、注册表、快捷方式删除之前。
- 升级/旧版本移除的真实控制流和可用变量。
- silent uninstall 的真实判定方式。
- 安装目录/helper path 的既有引用方式。

生成模板和 bundle 是本地证据，不提交。如果工具下载/生成失败，记录原因并停止 Task 3；不得
猜测升级变量后继续写 hook。

- [ ] **Step 2: 写 NSIS 静态 RED 测试**

测试读取 `.nsh` 与 Windows config，断言：

- 只定义 `NSIS_HOOK_PREUNINSTALL`，路径由 config 指向受控仓库文件。
- helper 名固定，无 task/SID/path 参数。
- 真实卸载条件来自 Step 1 证据；升级路径明确跳过。
- exit `0` 继续，`20/21/22/23/64` 阻断。
- silent 模式不弹 UI，也不忽略非零码。
- 文件不含 `schtasks`、`Stop-ScheduledTask`、owner marker、PowerShell 或 XML。

- [ ] **Step 3: 实现最小 hook**

使用生成模板的正式宏/变量，不新增可由用户覆盖的命令。交互失败提示只显示泛化 reason；silent
路径设置非零 installer result 并终止真正卸载。

- [ ] **Step 4: 运行 config/NSIS build GREEN**

```powershell
node --test scripts/windows-installer-cleanup-config.test.mjs
cmd /c corepack pnpm tauri build --bundles nsis --debug
git status --short
```

Expected: 静态测试和 NSIS bundle PASS；生成物 ignored/untracked 范围符合 policy。不得安装或运行
bundle，也不得创建真实 task。

- [ ] **Step 5: Review diff 并提交**

```powershell
git add src-tauri/windows/nsis-installer-hooks.nsh src-tauri/tauri.windows.conf.json scripts/windows-installer-cleanup-config.test.mjs
git commit -m "build: clean owned task during NSIS uninstall"
```

---

### Task 4: 生成 WiX 基线并接入 user-context custom action

**Files:**

- Create: `src-tauri/windows/wix/main.wxs`
- Create as required by generated baseline: `src-tauri/windows/wix/*.wxs`
- Modify: `src-tauri/tauri.windows.conf.json`
- Modify: `scripts/windows-installer-cleanup-config.test.mjs`

**Interfaces:**

- Uses: locked Tauri WiX custom template/fragment/ref support。
- Runs: installed helper before `RemoveFiles` in the initiating interactive user's context。
- Condition: semantics exactly `REMOVE="ALL" AND NOT UPGRADINGPRODUCTCODE`。
- Return: checked; helper exit `0` proceeds, every nonzero aborts true uninstall。

- [ ] **Step 1: 解决仅用于生成基线的 MSI version 前置**

使用锁定 CLI 支持的临时 config merge 提供 MSI-compatible numeric version，且不修改发布版本事实。
临时 config 放在系统 temp 目录，不提交。先验证 merge 后的 effective config，再继续：

```powershell
$msiSpikeConfig = Join-Path $env:TEMP 'hmm-p72c-wix-spike.json'
$msiSpikeJson = '{"bundle":{"windows":{"wix":{"version":"0.1.0"}}}}'
$utf8NoBom = New-Object System.Text.UTF8Encoding($false)
[System.IO.File]::WriteAllText($msiSpikeConfig, $msiSpikeJson, $utf8NoBom)
Get-Content -Raw -LiteralPath $msiSpikeConfig | ConvertFrom-Json | Out-Null
```

该 override 只解决 WiX numeric version gate；不能覆盖 identifier、product/upgrade code、install mode
或其他 lifecycle 配置。Task 4/5 完成后删除临时文件。

- [ ] **Step 2: 生成并审阅 WiX 基线**

```powershell
cmd /c corepack pnpm tauri build --bundles msi --debug --config "$msiSpikeConfig"
```

确认 Tauri 实际 WiX schema/version、主 template 结构、helper `File` id、
`InstallExecuteSequence`、`RemoveFiles`、upgrade properties 和 custom action 可插入点。生成失败时停止
Task 4；不得从网上示例猜 WiX v3/v4 syntax。

- [ ] **Step 3: 扩展静态 RED 测试**

用 XML parser 或结构化检查覆盖：

- Tauri config 指向受控 custom template/fragment。
- custom action 固定运行已安装 helper，无外部参数。
- `Return="check"` 或锁定 schema 的等价 checked-return。
- impersonation/user context 明确，不以 SYSTEM/错误 SID 运行。
- condition 只匹配 full remove 且排除 major upgrade。
- sequence 严格位于 `RemoveFiles` 前。
- repair/modify/upgrade 不匹配。

- [ ] **Step 4: 实现 custom template/action**

从生成基线做最小差异，不改无关 installer UI、目录、component 或 upgrade policy。若 helper file key、
sequence 或 impersonation 无法由生成模板和 XML 证明，停止实施并回到设计 review。

- [ ] **Step 5: 运行 WiX GREEN**

```powershell
node --test scripts/windows-installer-cleanup-config.test.mjs
cmd /c corepack pnpm tauri build --bundles msi --debug --config "$msiSpikeConfig"
git diff --check
git status --short
```

Expected: XML/static tests 与 MSI build PASS；不安装 bundle、不触碰真实 task。

- [ ] **Step 6: Review diff 并提交**

```powershell
git add src-tauri/windows/wix src-tauri/tauri.windows.conf.json scripts/windows-installer-cleanup-config.test.mjs
git commit -m "build: clean owned task during WiX uninstall"
```

---

### Task 5: 加固 static/config/packaging 自动化

**Files:**

- Modify: `scripts/prepare-windows-sidecars.test.mjs`
- Modify: `scripts/windows-installer-cleanup-config.test.mjs`
- Modify as needed: `policy/project-policy.json` only if a real generated-path rule is missing

**Interfaces:**

- Verifies: sidecar inventory、Tauri merge、NSIS/WiX lifecycle、forbidden strings 和 artifact hygiene。
- Does not perform: installer execution、real Scheduled Task mutation 或真实玩家路径访问。

- [ ] **Step 1: 增加跨文件一致性 RED**

覆盖 helper name/exit codes 在 Rust、NSIS、WiX 和 config 间一致；两个 installers 都引用同一 sibling
helper；worker CLI 仍只有 `--once`；helper 无参数。

- [ ] **Step 2: 增加 forbidden-output/command 扫描**

扫描新增 Rust/NSIS/WiX/scripts，拒绝：

```text
schtasks /Delete
Stop-ScheduledTask
task name/SID/XML/raw stdout/raw stderr 输出
save/profile/backup/game/Steam path 参数
```

扫描必须聚焦本 feature，不能误把既有内部 PowerShell ownership 核心当作新复制实现。

- [ ] **Step 3: 运行聚焦自动化**

```powershell
cargo test -p hmm-infra save_backup_installer_cleanup
cargo test -p hmm-infra save_backup_background_registry::tests
cargo test -p hmm-tauri installer_cleanup
cargo check -p hmm-tauri --bin hmm-save-backup-installer-cleanup
cargo check -p hmm-tauri --bin hmm-save-backup-worker
node --test scripts/prepare-windows-sidecars.test.mjs scripts/windows-installer-cleanup-config.test.mjs
```

- [ ] **Step 4: 构建两个 installer artifact**

```powershell
cmd /c corepack pnpm tauri build --bundles nsis
cmd /c corepack pnpm tauri build --bundles msi --config "$msiSpikeConfig"
```

检查安装器 payload 确实包含无 triple 后缀的 worker/helper sibling。只看到
`src-tauri/binaries/*-<target-triple>.exe` 不算 bundle gate 通过。

```powershell
Remove-Item -LiteralPath $msiSpikeConfig -ErrorAction SilentlyContinue
```

- [ ] **Step 5: 完整本地验证**

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify.ps1
git diff --check
git status --short --branch
```

- [ ] **Step 6: 必要时提交测试加固**

仅有独立测试/policy 改动时单独提交。若必须修改 `policy/project-policy.json`，触发治理 review，
说明没有降低任何现有检查。

```powershell
git add scripts/prepare-windows-sidecars.test.mjs scripts/windows-installer-cleanup-config.test.mjs policy/project-policy.json
git commit -m "test: harden installer cleanup packaging gates"
```

---

### Task 6: 同步文档、执行 review gate 与 disposable VM 验收

**Files:**

- Modify: `docs/SAVE_BACKUP_BACKGROUND_AUTOMATION_DESIGN.md`
- Modify: `docs/TESTING.md`
- Modify: `docs/LOGGING.md`
- Modify: `docs/release/发布与产物说明.md`
- Modify: `docs/release/构建发布与脚本说明.md`
- Modify: `TODO.md`
- Modify: `docs/superpowers/specs/2026-07-12-save-backup-installer-cleanup-design.md`
- Modify: `docs/superpowers/plans/2026-07-12-save-backup-installer-cleanup-implementation.md`
- Create: `docs/testing/windows-save-backup-installer-cleanup-smoke.md`

**Interfaces:**

- Documents: helper contract、exit mapping、upgrade/repair/true uninstall、NSIS/WiX gates、已执行证据。
- Preserves: P7.2a runtime acceptance 与 P7.2c cleanup acceptance 是独立状态。

- [ ] **Step 1: 先写 disposable VM smoke 文档**

文档固定一次性账户/VM、synthetic task/profile/save/backup fixture、授权边界、每个 case 的 cleanup
和停止条件。禁止在日常账户执行；禁止记录 task name、SID、路径、XML 或原始命令输出。

- [ ] **Step 2: 同步正式 contract 与命令**

只有真实脚本/命令落地后才把它们写进 `docs/TESTING.md`。日志文档明确 installer 只记录稳定
exit/reason，不使用 App Audit Log。发布文档分别记录 build、P7.2a runtime、P7.2c NSIS、
P7.2c WiX 四类证据。

- [ ] **Step 3: 本地 review gate**

按 `hmm-review-gate` findings-first 复审：

- dependency direction 与 helper 单一职责。
- foreign/busy/unverified/delete-readback matrix。
- upgrade/repair/modify/silent behavior。
- installer sequencing 与当前用户 identity。
- no `.planning/`、bundle、sidecar、logs、fixtures、private paths 或 secrets。
- docs 不提前声称 VM gate 已通过。

- [ ] **Step 4: 运行完整本地验证**

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify.ps1
git diff --check
git status --short --branch
```

- [ ] **Step 5: 在 disposable Windows VM 分别验收 NSIS 与 WiX**

每种 installer 执行：missing、owned exact、owned drift、foreign、running、upgrade、repair/modify、
helper/permission failure，以及交互/静默真正卸载。running case 必须证明 worker 未被强杀；foreign
case必须证明 task 保留但产品成功卸载。

如果没有可访问 VM、可安装 bundle 或明确授权，记录“未执行”并保持 P7.2c TODO 未完成。不得在
开发者日常账户补跑真实 task 操作。

- [ ] **Step 6: 更新状态并提交文档**

只有 NSIS 和 WiX runtime gate 都通过，才能勾选 P7.2c 实现完成。否则文档应精确列出已完成的
代码/static/build gate 与仍未执行的 runtime gate。

```powershell
git add docs/SAVE_BACKUP_BACKGROUND_AUTOMATION_DESIGN.md docs/TESTING.md docs/LOGGING.md docs/release/发布与产物说明.md docs/release/构建发布与脚本说明.md TODO.md docs/testing/windows-save-backup-installer-cleanup-smoke.md docs/superpowers/specs/2026-07-12-save-backup-installer-cleanup-design.md docs/superpowers/plans/2026-07-12-save-backup-installer-cleanup-implementation.md
git commit -m "docs: record installer cleanup release gates"
```

---

## Final Review Checklist

- [ ] Diff 只包含 P7.2c 相关文件，没有 `.planning/`、sidecar、installer、logs 或 fixture。
- [ ] helper 无参数，不依赖 AppData/SQLite/Audit Log/worker/save/backup/game/network。
- [ ] task name、SID、marker、PowerShell 与 delete/read-back 只有 infra ownership 核心一份。
- [ ] missing/owned exact/owned drift/foreign 均可幂等继续；foreign 从未 mutation。
- [ ] running/queued 不 mutation、不强杀；unknown state fail closed。
- [ ] mutation 前 ownership/state 复核与 post-delete missing read-back 有测试。
- [ ] Settings `disable()` 和 worker `--once` contract 未改变。
- [ ] upgrade/repair/modify 跳过，true interactive/silent uninstall 执行同一 fail-closed helper。
- [ ] NSIS PREUNINSTALL 与 WiX pre-RemoveFiles sequencing 均由生成模板证据和静态测试证明。
- [ ] WiX custom action 在发起卸载的当前交互用户上下文运行。
- [ ] 两个 installer payload 都包含无 triple 后缀的 worker/helper sibling。
- [ ] 自动化没有真实 Scheduled Task 或玩家数据操作。
- [ ] `verify.ps1` 在最后一次变更后通过；未执行检查有明确原因。
- [ ] NSIS/WiX disposable VM gate 分别记录；任一未通过时不宣称 P7.2c 完成。
