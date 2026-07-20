# 测试指南

本文档定义 Helsincy Mod Manager 的测试与验证基线。项目当前处于规划和脚手架基线阶段，测试命令会随着核心功能落地继续完善。

## 目标

- 让协作者知道不同改动至少要验证什么。
- 避免所有改动都被迫全量验证。
- 对 Mod 安装、存档备份、文件写入、并发任务等高风险路径建立固定检查入口。
- 明确记录哪些验证已经执行，哪些因为环境限制没有执行。

## 基础环境

当前使用：

- Node.js 24 或更新的 LTS 版本。
- pnpm 通过 `packageManager` 锁定，并由 Corepack 启用。
- Rust stable。
- Tauri 2 对应平台依赖。
- Windows 开发环境建议安装 PowerShell 7+。

当前前端依赖由 `package.json` 和 `pnpm-lock.yaml` 锁定。Windows PowerShell 5.1 下建议使用 `cmd /c corepack pnpm ...`，避免直接调用 `pnpm.ps1` 时被执行策略拦截。

## 文档改动

适用范围：

- `README.md`
- `CHANGELOG.md`
- `CONTRIBUTING.md`
- `SECURITY.md`
- `AGENTS.md`
- `docs/GOVERNANCE.md`
- `docs/LOGGING.md`
- `docs/`
- `docs/release/`

最小验证：

- 检查链接路径是否有效。
- 检查文档职责是否重复。
- 检查文档是否与当前架构阶段一致。

当前可执行命令：

```powershell
git status --short --branch
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\check-whitespace.ps1
./scripts/verify.ps1
```

Linux / Steam Deck 开发环境可以使用：

```bash
bash scripts/verify.sh
```

如果 Windows PowerShell 执行策略阻止脚本运行，可以使用：

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify.ps1
```

安装本地 Git hooks：

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\install-hooks.ps1
```

## 前端改动

适用范围：

- `src/`
- 前端组件、页面、状态管理、API 调用封装。

脚手架完成后的最小验证：

```powershell
cmd /c corepack pnpm install --frozen-lockfile
cmd /c corepack pnpm run typecheck
cmd /c corepack pnpm run lint
cmd /c corepack pnpm run build
```

涉及 UI 工作流时，建议补充：

```powershell
cmd /c corepack pnpm run test
```

涉及 App Shell、侧边栏模式、Dashboard 页面拆分时，还必须确认：

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\check-frontend-boundaries.ps1
```

该脚本会阻止 Dashboard 页面读取 `sidebarMode` / `useSidebarMode`、阻止按侧边栏模式复制 Dashboard 页面、确认导航定义只有一份，并避免 Dashboard 样式通过 `[data-sidebar-mode]` 按侧边栏模式分叉。

涉及 UI Shell、侧边栏模式或 Dashboard v2 视觉基线时，建议补充浏览器 smoke test：

- 桌面宽屏 `1440x900`：验证普通侧边栏和悬浮侧边栏下，顶部状态栏、主卡片、模块预览和右侧状态面板均正常显示，切换侧边栏后文案不变。
- 常见窗口 `1366x768`：验证普通侧边栏和悬浮侧边栏均可用，顶部状态栏文字不重叠，主操作按钮完整可读，切换侧边栏不会让页面滚动到错误位置。
- Steam Deck 近似窗口 `1280x800`：验证触控目标尺寸可用，悬浮侧边栏不遮挡主操作按钮和右侧状态面板，空间不足时按响应式策略由内部内容区滚动。

涉及真实桌面交互、窗口、文件选择器或 Tauri command 调用时，需要启动本地应用进行手动 smoke test。

窗口关闭与托盘生命周期切片至少运行：

- `node --test src/app/window-lifecycle/windowLifecycleUi.test.mjs src/app/window-lifecycle/windowClosePreference.test.mjs`
- `cmd /c corepack pnpm run typecheck`
- `cmd /c corepack pnpm run lint`
- `cmd /c corepack pnpm run build`
- `cargo test -p hmm-tauri window_lifecycle`
- `cargo check -p hmm-tauri`

可视化检查需要覆盖：normal close dialog、`starting` 与 `worker_unhealthy` unsafe dialog、收起至托盘后从托盘恢复、完全退出、记住选择、设置页改回每次询问。unsafe 必须默认聚焦留在托盘、不显示 remember，并在最小 `960x640` 窗口无文字重叠；只有后端状态为 `protected` 才能描述退出后受保护。

### T19 Feedback UI U1

共享反馈基元和首个游戏目录 Dialog 至少运行：

```powershell
node --test src/shared/feedback/feedbackPrimitives.test.mjs src/features/game-setup/gameSetupStartupDetection.test.mjs src/features/mods/modDetailDialog.test.mjs src/features/mods/modReinstallApi.test.mjs
cmd /c corepack pnpm run test
cmd /c corepack pnpm run typecheck
cmd /c corepack pnpm run lint
cmd /c corepack pnpm run build
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\check-frontend-boundaries.ps1
```

浏览器 smoke 覆盖 `1440x900`、`1366x768`、`960x640` 和窄屏 `390x844`：确认只存在一个
body-level feedback host，游戏目录决策以 `aria-modal` Dialog 呈现，初始焦点和正反向 Tab wrap 正确，
空闲态 Escape/背景点击可关闭，busy 态关闭入口全部阻断，关闭后焦点返回，长文本和按钮不溢出。
同时确认 z-index 200 的窗口关闭保护高于共享 Dialog 180，只有视觉最顶层模态处理 Tab/Escape。

### T19 Feedback UI U2

核心 Mod 生命周期反馈迁移至少运行：

```powershell
node --test src/features/mods/modInstallTaskState.test.mjs src/features/mods/modLifecycleFeedbackState.test.mjs src/features/mods/modLifecycleFeedbackUi.test.mjs src/features/mods/modInstallPlanApi.test.mjs src/features/mods/modReinstallTaskState.test.mjs
cmd /c corepack pnpm run test
cmd /c corepack pnpm run typecheck
cmd /c corepack pnpm run lint
cmd /c corepack pnpm run build
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\check-frontend-boundaries.ps1
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\check-file-size.ps1
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify-core-mod-lifecycle.ps1
```

状态测试必须覆盖 ready、blocking conflict、running、completed、ordinary failed、cancelled 和
recovery-required。Task Notice 只允许消费匹配 `taskId` 的 install/uninstall phase；terminal Toast 必须在
manifest/recovery 刷新 verified 后生成，`committed_cleanup_pending`、`cleanup_pending`、
`rollback_required`、`repair_required`、`unknown` 以及刷新不可用都保留持久阻断并抑制普通 Toast。

浏览器 smoke 覆盖 `1440x900`、`1366x768`、`960x640` 和 `390x844`：安装计划/冲突以可滚动
Detail Sheet 呈现且关闭不改变任务；卸载以 `alertdialog` 呈现，背景点击不关闭，初始焦点为取消，
Escape 只取消；运行态 Task Notice 和 terminal Toast 不挤压 Mod 列表、长 Mod 名不溢出。重装与 retarget
仍使用既有 DTO、phase、preview token 和 durable refresh，不因 U2 改变工作流。

### T19 Feedback UI U3

跨 feature 短时反馈迁移至少运行：

```powershell
node --test src/shared/feedback/feedbackToastState.test.mjs src/shared/feedback/feedbackPrimitives.test.mjs src/features/mods/modImportAction.test.mjs src/features/profiles/profileFrontendIntegration.test.mjs src/features/install-recovery/recoveryCenterRoute.test.mjs
cmd /c corepack pnpm run test
cmd /c corepack pnpm run typecheck
cmd /c corepack pnpm run lint
cmd /c corepack pnpm run build
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\check-frontend-boundaries.ps1
```

状态测试必须覆盖 stable event key 合并、不同事件不按文案误合并、队列上限与最旧项淘汰、taskId 保留和定向关闭。逐 feature 迁移时，字段错误、页面加载错误、恢复中心决策面板和全局安全告警继续保持持久语义；长任务进度继续按 taskId 显示，终态成功必须在对应刷新完成后发布。

浏览器 smoke 覆盖 `1440x900`、`1366x768`、`960x640` 和 `390x844`：确认只有一个 body-level feedback host，Toast 队列不挤压页面且不产生横向溢出，长文本可换行，最多一个可选动作，hover/focus 暂停自动关闭，Escape 关闭最新通知，并验证多个任务的通知仍带稳定来源 key 与 taskId。

### T19 Logging / Diagnostics L3

```powershell
node --test src/features/diagnostics/diagnosticsPage.test.mjs
cargo test -p hmm-app page_snapshot_keeps_safe_sections_available_when_one_reader_fails
cargo test -p hmm-tauri serializes_diagnostics_page_snapshot_without_path_fields
cmd /c corepack pnpm run typecheck
cmd /c corepack pnpm run lint
cmd /c corepack pnpm run build
cargo check --workspace
```

必须覆盖 reader 部分失败、空结果、稳定健康码、DTO/typed API 无路径字段、受控导出确认与失败不误报成功。
诊断 ZIP 继续执行既有敏感片段扫描；浏览器 smoke 覆盖 `1440x900`、`960x640` 和 `390x844`。

## Tauri / Rust 桥接改动


适用范围：

- `src-tauri/`
- Tauri commands
- Tauri state
- 前后端 DTO
- 事件推送

最小验证：

```powershell
cargo test --workspace
cargo check --workspace
```

建议补充：

```powershell
cmd /c corepack pnpm run tauri:dev
```

验证重点：

- command 参数校验。
- 错误返回是否可被前端展示。
- 长任务是否通过事件返回进度。
- 是否暴露了过宽的文件系统能力。

## Rust 核心逻辑改动

适用范围：

- `src-tauri/crates/hmm-core/`
- `src-tauri/crates/hmm-ports/`
- `src-tauri/crates/hmm-app/`
- `src-tauri/crates/hmm-infra/`
- `src-tauri/crates/hmm-games-mhw/`

最小验证：

```powershell
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

验证重点：

- 领域层是否仍然不依赖基础设施。
- 应用层是否依赖 trait，而不是具体实现。
- 游戏适配规则是否封装在 adapter 内。
- 错误类型是否能表达可恢复失败和不可恢复失败。

### T18 Mod 库 read-model 基准

Slice 4A 提供显式、release-only、默认 ignored 的确定性 harness：

```powershell
# 新 worktree 首次运行 hmm-tauri 测试时先准备 ignored development sidecar。
cmd /c corepack pnpm run prepare:save-backup-worker-sidecar:dev
cargo test -p hmm-tauri --release mod_library_read_model_baseline -- --ignored --nocapture
```

要求：

- 固定生成 1,000 / 10,000 条人工 Mod/revision、metadata overlay、category pair 和 profile manifest fixture。
- 每个测量阶段固定 5 次 warmup 和 40 次 sample；后续切片不得降低样本数来改善表面 p95。
- 只使用内存和 temp JSON；不读取真实 Mod、游戏目录、manifest、玩家存档或用户 AppData。
- 报告 JSON catalog read/project、snapshot overlay/category merge、profile status query、兼容 query、status-filter query 和 exact page DTO serialization 的 median/p95/min/max。
- debug build 必须拒绝运行，避免把未优化结果写成基线；普通 `cargo test --workspace` 不执行 ignored benchmark。
- wall-clock 结果只在固定 runner 或相同机器/工具链上比较，不在普通单元测试中加入跨机器绝对时延断言。
- 4B/4C 必须复用相同 fixture schema、warmup/sample 和 JSON 输出格式，不能通过减少数据字段或跳过 status filter 制造虚假提升。

### T18 Slice 4B projection writer/rebuild

Slice 4B 的自动化只使用人工 projection records、临时 JSON 和临时 SQLite，不读取真实 Mod、游戏目录、manifest、存档或用户 AppData。聚焦入口：

```powershell
cargo test -p hmm-infra mod_library_projection -- --nocapture
cargo test -p hmm-infra mod_import_catalog_upsert_many -- --nocapture
cargo test -p hmm-ports mod_library_projection -- --nocapture
cargo test -p hmm-app mod_library_query -- --nocapture
```

必须覆盖：

- migration 的 projection 表、`BINARY` 索引、query-key version 和 profile status 复合外键。
- rebuild 的 dirty -> complete generation、旧行清理、重复 Mod/revision/package 拒绝，以及失败后不发布部分 rows。
- profile 首次发布/替换、generation completeness、未知 Mod 外键失败和 dirty fail-closed 语义。
- `upsert_many` 的 10,000 总上限、200 分块、同一 Mod 多 revision 的 exact retry 幂等且不重复写入，以及后块失败时前块保持已提交。

4B 不执行生产 SQLite query/count/page、Tauri/前端 smoke 或性能门禁；这些属于 Slice 4C。完整交付仍需运行 workspace tests/check/clippy 和 `scripts/verify.ps1`。

## Mod 导入与压缩包处理

适用范围：

- archive inspect
- sandbox extract
- package analyzer
- preview extractor

必须覆盖：

- 正常 zip / 7z 包。
- 包含 `nativePC` 的 Mod。
- 包含根目录 DLL 的 Mod。
- 包含预览图的 Mod。
- 没有预览图的 Mod。
- 路径穿越样本。
- 绝对路径样本。
- 大小写冲突样本。
- 伪装图片样本。

测试要求：

- 只能使用人工构造的最小测试包。
- 不提交真实第三方 Mod 包。
- 解压目标必须是临时目录。

## 安装、卸载与回滚

适用范围：

- InstallPlan
- InstallExecutor
- manifest
- backup
- rollback

必须覆盖：

- 新文件安装。
- 覆盖已有文件并备份。
- 安装中途失败并回滚。
- 卸载已安装 Mod。
- 基于 manifest 卸载。
- 两个 Mod 写入同一路径的冲突检测。
- 切换替换目标后的重新安装。

测试要求：

- 使用临时目录模拟游戏目录。
- 不直接操作真实 MHW:I 安装目录。
- 每个测试结束后校验临时目录状态。

### Core Mod Lifecycle Gate A

核心 Mod 生命周期的正式自动验收入口为：

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify-core-mod-lifecycle.ps1
```

PowerShell 7 或非 Windows 环境使用同一脚本：

```powershell
pwsh -NoProfile -File ./scripts/verify-core-mod-lifecycle.ps1
```

脚本先从 `hmm-tauri` 测试列表发现 `headless_composition_*` 场景，要求发现数不少于固定基线 6，
并逐项确认 T19 计划记录的 6 个固定场景仍然存在；发现数为 0、少于基线、缺少固定场景或场景被
标记为 ignored 时均失败。发现成功后，脚本通过同一个公共前缀一次执行全部场景，并保留 cargo 的
非零退出码。Windows 新 worktree 会在发现测试前通过仓库既有 helper 准备 debug sidecar；生成的
`target/`、`node_modules/` 和 `src-tauri/binaries/` 内容仍为 ignored 运行时产物，不得提交。

GitHub `Verify` workflow 调用同一脚本，不在 YAML 中复制测试清单。场景继续只使用人工 zip、temp
AppData、temp MHW:I-like game root、fake port 和受控 staging/backup/manifest roots；该入口不得读取
真实 MHW:I、Steam userdata、玩家存档或第三方 Mod 包。

CL0/CL1 test-only composition harness 使用人工 zip、temp AppData 与 temp MHW:I-like game root。
CL0 覆盖 fixture 分类、真实 importer、持久化 import/game config、MHW adapter InstallPlan 和
AppState restart；CL1 在同一 harness 上覆盖 install -> restart -> uninstall -> baseline、manifest/
recovery counts、task identity/phase、Audit Log 字段白名单和公开证据脱敏：

```powershell
cargo test -p hmm-tauri state::core_mod_lifecycle_tests
cargo test -p hmm-app install
```

`hmm-app install` 必须包含 source read 与 backup store failure 的双 action 注入，证明完整 source/
backup prepare 成功前 game writes 和 manifest saves 都为零，并覆盖 pending backup/recovery cleanup。

CL0/CL1 的 fixture、证据矩阵、缺口和桌面 smoke 见
[Core Mod Lifecycle CL0 验收基线](CORE_MOD_LIFECYCLE_CL0_ACCEPTANCE.md)。CL2 桌面 smoke 只允许在
disposable account/VM 执行，不得使用维护者日常 AppData 或真实游戏目录；CL3 才验证 v1 -> v2
真正重装。CL1 自动化通过不代表 Gate A `certified`。

CL3 的测试矩阵见
[真正重装设计](superpowers/specs/2026-07-14-core-mod-lifecycle-cl3-true-reinstall-design.md) 与
[逐任务实施计划](superpowers/plans/2026-07-14-core-mod-lifecycle-cl3-true-reinstall-implementation.md)。
L1/L2 已有实际聚焦入口：

```powershell
cargo test -p hmm-tauri state::core_mod_lifecycle_tests
cargo test -p hmm-tauri state::core_mod_lifecycle_tests::headless_composition_reinstalls_v1_to_v2_and_restores_baseline -- --nocapture
cargo test -p hmm-tauri state::core_mod_lifecycle_tests::headless_composition_rolls_back_v1_when_reinstall_manifest_save_fails -- --nocapture
cargo test -p hmm-app mod_import::revision_tests
cargo test -p hmm-core reinstall
cargo test -p hmm-app reinstall
cargo test -p hmm-app reinstall_task
cargo test -p hmm-infra mod_revision_catalog
cargo test -p hmm-infra reinstall
cargo check --workspace
```

这些命令覆盖四类 target/entry-set replacement、catalog migration/revision import、manifest/recovery
原子持久化、preview/preflight 零写入、commit/rollback fault matrix、共享写锁/cancellation barrier、
DTO/task/Audit 契约，以及 L2 `v1 -> v2 -> restart -> uninstall -> baseline` 和 manifest failure ->
rollback v1 -> restart。所有 AppData、archive 和 game root 均为 TEMP/artificial fixture。

CL3 Task 10 已于 2026-07-15 在 Windows Sandbox 中使用人工 v1/v2 ZIP、唯一 TEMP game root 和
disposable AppData 实际执行 L3。证据覆盖同一 logical Mod 单卡 revision import、v1 安装与重启、
1 retained / 2 replaced / 1 added / 1 stale 真正重装、v2 重启、manifest 卸载、逐字节 baseline
恢复、not-installed 重启、恢复中心零残留；支持诊断白名单包含四个固定 JSON 条目，且受控 TEMP
cleanup 已完成。未使用真实
MHW:I、第三方 Mod、Steam userdata、玩家存档或维护者日常 AppData。

CL3 自动化与桌面证据全部通过并标记为 `implemented`。CL4 于 2026-07-15 重新执行上述聚焦矩阵、
全部前端测试、完整 `scripts/verify.ps1` 和 `cargo clippy --workspace --all-targets -- -D warnings`，并完成
独立安全/边界复审；Gate A 已标记为 `certified`。Gate B / AR1 的 replacement model/catalog、AR2
parser/analyzer/纯 `RetargetPlan`、AR3 staging/InstallPlan/binding snapshot 与 AR4 Tauri typed
contract/最小受控 UI 测试已落地。AR5 同 revision 真正重装 target switch、重启恢复、manifest 卸载
baseline 与受控 UI 自动化也已落地；最终 artifact 的 disposable Windows Sandbox 纵向复验通过后，
Gate B 已标记为 `certified`。

### ARMOR_RETARGET AR1

AR1 只覆盖纯领域模型、只读 port 与静态 MHW:I catalog，不读取真实 Mod 或游戏目录：

```powershell
cargo test -p hmm-core --test replacement
cargo test -p hmm-ports --test replacement_catalog
cargo test -p hmm-games-mhw --test armor_catalog
```

这些测试分别锁定 stable target/binding/source/catalog identity 与 serde 不变量、catalog list/find/search
trait contract、`mhw-armor-v1` seed、MHW internal id/metadata schema，以及 NFC/中点/NFKC 搜索规范化和
Fatalis/Alatreon 精确隔离。

### ARMOR_RETARGET AR2

AR2 只使用人工 package file identity 和相对路径字符串，不读取真实 Mod、游戏目录或玩家数据：

```powershell
cargo test -p hmm-core --test replacement_analysis
cargo test -p hmm-ports --test replacement_adapter
cargo test -p hmm-games-mhw --test armor_retarget
cargo clippy -p hmm-core -p hmm-ports -p hmm-games-mhw --all-targets -- -D warnings
```

这些测试锁定 `/`/`\\` 规范化、严格 `f_equip` 模板、危险/畸形路径拒绝、`m_equip`/混合/多 source
阻断、普通非 Armor 包的不适用 warning、unknown target/binding mismatch、package identity 保留、
只替换 slot 段，以及 action/source/target/重复最终路径不变量。AR2 不测试 staging 或真实复制；这些
从 AR3 开始使用 temp directory fixture 覆盖。

### ARMOR_RETARGET AR3

AR3 使用 fake ports、人工 package bytes 与 temp staging/game/manifest roots，不读取真实 Mod、游戏目录
或玩家数据：

```powershell
cargo test -p hmm-core --test replacement_install
cargo test -p hmm-app --test replacement_service
cargo test -p hmm-infra --test retarget_staging
cargo test -p hmm-app
cargo test -p hmm-infra
```

这些测试锁定原 `PackageFileId` provenance、最终 target conflict key、batch staging containment、
大小写不敏感碰撞、symlink/junction escape、sibling `.partial` 发布和失败清理；同时覆盖 snapshot
serde/legacy default、Mod/profile/revision 归属、plan/token hash、manifest merge/uninstall/rollback、
真正重装 candidate replacement 与跨重启 recovery recognition。普通 install 的 revision mismatch 和
真正重装的 candidate revision mismatch 都必须在 source read、game write 和 manifest save 前零 I/O
阻断。

### ARMOR_RETARGET AR4

AR4 的 Tauri/app 测试继续使用 fake ports、人工 package bytes 与 temp game/staging/manifest roots；
前端测试只消费稳定 DTO 和人工 Mod 数据，不读取真实游戏目录或第三方 Mod：

```powershell
cargo test -p hmm-tauri replacement_dto_tests
cargo test -p hmm-tauri replacement_commands
cargo test -p hmm-app --test replacement_service
cargo test -p hmm-app replacement_task::tests
node --test src/features/replacements/*.test.mjs
cmd /c corepack pnpm run test
```

这些测试锁定四个窄 command 的 camelCase DTO、未知字段拒绝、后端 display revision/source 解析、
profile 全量 recovery admission、锁外分析/staging、锁内 `not_installed` 二次校验、失败/取消清理、
task id/phase 匹配，以及前端 loading/error/empty/warning/conflict/installed fail-closed 状态。浏览器 smoke
覆盖详情 Tab、右键直达、modal 层级和 `1440x900`/`480x800` 响应式。

2026-07-16 已在 disposable Windows Sandbox 使用人工 game root 和单文件 armor ZIP 完成 AR4 真正
Tauri 成功态验收：`pl121_0000` source 被识别为单一 `pl/f_equip` 资源，选择 `pl129_0000` 后预览为
1 个动作、0 个阻断冲突；首次安装只生成 target 槽位文件，source 槽位保持不存在，target 长度/hash
与原人工字节一致。完全关闭并重开应用后仍恢复为已安装，普通安装入口和 AR4 首次 retarget 安装入口
均 fail closed，真正 target switch 明确留给 AR5。该结果只验收 AR4 切片，不代表 Gate B certified。

### ARMOR_RETARGET AR5

AR5 自动化继续只使用人工 package bytes、temp game/staging/manifest/backup roots 与 fake ports：

```powershell
cargo test -p hmm-core recovery_transaction_allows_only_a_proven_same_revision_replacement_target_switch
cargo test -p hmm-app replacement_target_switch
cargo test -p hmm-app retarget_reinstall
cargo test -p hmm-app --test replacement_service workflow_rebuilds_the_installed_revision_with_stable_binding_lineage_for_target_switch
cargo test -p hmm-tauri headless_composition_switches_retarget_with_true_reinstall_and_uninstalls_to_baseline
cargo test -p hmm-tauri retarget_reinstall
node --test src/features/replacements/replacementApi.test.mjs src/features/replacements/replacementWorkflow.test.mjs src/features/replacements/replacementDetailUi.test.mjs
cmd /c corepack pnpm run test
```

这些测试锁定普通同 revision 重装继续阻断、只有同 lineage 且 target 变化的 binding 才可切换、
installed revision 从 manifest 解析且不隐式升级、operation-scoped staging/RAII cleanup、写锁内 token
revalidation、四类 target 计数、失败 rollback/recovery，以及同 revision 内的
`Fatalis Alpha + target -> Fatalis Beta + target -> restart -> uninstall
-> exact pre-Armor baseline`。Tauri/前端测试同时锁定窄 DTO、稳定错误、严格 taskId、确认对话框、
取消安全阶段和 blocked/current-target/stale-token fail closed。manifest 查询测试还必须锁定：仅可信
installed 状态返回唯一 `installedTargetId`，歧义或不安全状态不产生可执行 target；前端重启加载后
标记“当前已安装”，当前 target 不得作为切换候选。

首个 AR5 artifact 已在 disposable Windows Sandbox 完成首次 retarget 安装 -> 选择不同 target 真正重装
-> 完全重启 -> manifest 卸载 -> exact baseline，并确认 source/旧 target 不残留、staging/recovery 为零；
该轮发现并修复了重启后 replacement Tab 未标记当前 target 的缺陷。

2026-07-16，修复后的最终 artifact（commit `48f913f`；验收 ZIP SHA-256
`C28AA2656888E4E624525DDE0C62A720004DBE20683851E1B50E5AB2FFDDA156`；主程序 SHA-256
`91006F26BFA1CE629569E64D264495A235456D6B0782AD1D7A000715B48D65F1`）已在全新 disposable
Windows Sandbox 完成第二轮完整验收：`Before` 为全新 AppData 和唯一 23-byte baseline exe，SHA-256
为 `00A9E27855EDC182AD0EB4C16C71C845D6AA096023AAEE2705F526799A959606`。Fatalis Alpha + 首次安装后
只有 `pl129_0000` target，payload 为 28 bytes，SHA-256 为
`62A74A0A3A1C24EEC25E29A1C9FE38771F0B9A58914603B5A8D9CD1C182B740E`；完全重启后自动恢复并标记
Alpha 当前目标。切换到 Fatalis Beta + 的预览为 retained/replaced/added/stale `0/0/1/1` 且安全预检
通过；真正重装后只有 `pl129_0010` target，payload 长度/hash 不变，再次完全重启后自动恢复并标记
Beta 当前目标。最终 manifest 卸载恢复为与 `Before` 长度和 SHA-256 完全一致的一文件 baseline；所有
关键阶段 staging/recovery entry 均为 0，source 和旧 target 均不残留。
全程只使用人工 fixture，未使用真实 MHW:I、第三方 Mod、Steam userdata 或玩家存档。结合本节自动化、
完整 `scripts/verify.ps1`、clippy 与 findings-first 安全复审，Gate B 已标记为 `certified`。

## 存档备份

适用范围：

- 手动备份
- 自动备份
- 备份恢复
- 保留策略

必须覆盖：

- 默认备份目录。
- 用户自选备份目录。
- 每个 profile 的独立备份子目录。
- 稳定文件命名和同秒重名序号。
- 备份 manifest。
- manifest 不包含完整本地路径、Steam ID 或真实存档内容。
- 恢复前校验。
- 保留数量限制。
- 备份目录不可写。
- 源目录与备份目录包含关系拒绝。
- symlink/junction 逃逸拒绝。
- 大小写路径碰撞拒绝。
- `save_backup.*` 任务事件携带 `taskId`。
- 前端 typed API 只传 `gameId`、`profileId`、`note` 和 `limit`，不传路径、manifest、backup ref、sandbox/cache 或 hash。

测试要求：

- 使用临时目录模拟存档目录。
- 不读取或写入真实玩家存档。
- 不依赖真实 MHW:I 安装目录、真实 Steam userdata 或真实玩家存档。
- 存档目录自动发现测试必须使用 temp Steam root、fake HTTP/profile transport 和人工 XML fixture；不得依赖真实 Steam 账号、真实游戏安装、真实网络或真实存档目录。
- 手动备份后端 MVP 至少运行聚焦测试：

```powershell
cargo test -p hmm-app --test save_backup
cargo test -p hmm-app --test save_backup_task
cargo test -p hmm-infra --test save_backup_repository
cargo test -p hmm-infra --test save_backup_writer
cargo test -p hmm-tauri save_backup
cmd /c corepack pnpm run test -- src/features/profiles/profileApi.test.mjs
```

自动备份调度状态与后台保护状态查询切片至少运行聚焦测试：

```powershell
cargo test -p hmm-app --test save_backup_scheduler
cargo test -p hmm-infra --test save_backup_scheduler_repository
cargo test -p hmm-infra game_running
cargo test -p hmm-games-mhw adapter_reports
cargo test -p hmm-tauri save_backup
cmd /c corepack pnpm run test -- src/features/profiles/profileFrontendIntegration.test.mjs
```

要求：调度器测试使用 fake repository / fake clock / fake game running detector；scheduler state repository 测试使用临时 SQLite；游戏运行检测测试只用 fixture 字符串，不依赖真实进程或真实游戏；`get_save_backup_background_status` 的 DTO 测试必须断言序列化结果不含 `leaseOwner`、`leaseExpiresAt`、`workerInstanceId` 或任何路径字段。

P7.1 后台备份 headless worker 与调度租约基础能力至少运行以下可复制的聚焦验证：

```powershell
cargo test -p hmm-app --test save_backup_background_worker
cargo test -p hmm-app --test save_backup_scheduler
cargo test -p hmm-app --test save_backup_task
cargo test -p hmm-infra --test save_backup_scheduler_repository
cargo test -p hmm-tauri background_worker
cargo check -p hmm-tauri --bin hmm-save-backup-worker
```

要求：worker 与 scheduler 测试使用 fake ports、固定 clock 和临时 SQLite/目录；不得使用真实 Windows Scheduled Task、真实游戏进程、真实 MHW 安装、Steam userdata 或玩家存档。该切片验证的是 `tray_only` 下的单次 `--once` worker、持久化 lease/heartbeat 与既有任务链路复用，不证明主客户端退出后已经自动运行，也不构成 `protected` 或完整后台保障。

P7.2a Windows Scheduled Task 平台核心、独立 heartbeat、健康派生和 sidecar 至少运行：

```powershell
cargo test -p hmm-core background_registration_statuses_have_stable_codes
cargo test -p hmm-ports background_registry_errors_have_stable_codes
cargo test -p hmm-infra --test save_backup_scheduler_repository
cargo test -p hmm-infra save_backup_background_registry::tests
cargo test -p hmm-app --test save_backup_background
cargo test -p hmm-app --test save_backup_background_worker
cargo test -p hmm-app --test save_backup_scheduler
cargo test -p hmm-app --test save_backup_task
cargo test -p hmm-tauri save_backup
cargo check -p hmm-tauri --bin hmm-save-backup-worker
node --test scripts/prepare-save-backup-worker-sidecar.test.mjs
```

要求：平台注册自动化只能使用 fake registry/command runner；健康矩阵使用 fixed clock；repository 使用临时 SQLite；sidecar 测试只检查构建配置和 Cargo metadata。普通测试和 `verify.ps1` 不得创建、更新、启动或删除真实 Scheduled Task。`get_save_backup_background_status` 必须保持只读，并覆盖 exact + fresh、future、stale、drift、permission 和 unsupported 等 fail-closed 状态。

真实 Windows 验收只允许人工在一次性本地账户或 VM 按 [Windows 存档后台任务人工 Smoke](testing/windows-save-backup-scheduled-task-smoke.md) 执行。只有安装态 sibling worker、任务真实触发、fresh heartbeat 和最终 cleanup 全部通过，才能记录 Windows runtime acceptance；不得在开发者日常账户为了完成 checklist 运行 ignored smoke。

P7.2b 全局用户意图、Settings/Profile 边界和统一退出保护至少运行：

```powershell
cargo test -p hmm-core background
cargo test -p hmm-ports background
cargo test -p hmm-infra --test save_backup_background_settings_repository
cargo test -p hmm-infra --test save_backup_scheduler_repository
cargo test -p hmm-app --test save_backup_background
cargo test -p hmm-app --test save_backup_background_worker
cargo test -p hmm-app --test save_backup_exit_guard
cargo test -p hmm-tauri save_backup
cargo test -p hmm-tauri window_lifecycle
node --test src/features/settings/backgroundProtectionApi.test.mjs src/features/settings/backgroundProtectionPanel.test.mjs
node --test src/features/profiles/profileFrontendIntegration.test.mjs src/features/profiles/profileApi.test.mjs
node --test src/app/window-lifecycle/windowLifecycleUi.test.mjs src/app/window-lifecycle/windowClosePreference.test.mjs
```

要求：SQLite repository 使用临时数据库；service/worker/exit guard 使用 fake registry、fake repositories 和 fixed/sequence clock；enable/disable 必须覆盖并发转换串行，global heartbeat 必须覆盖 cycle completion timestamp 与正常业务 skip。前端测试锁定 Settings 唯一控制入口、Profile 只读、稳定 status/reason/code、未知 runtime 值的 fail-closed fallback 和 unsafe no-remember。普通自动化与 `verify.ps1` 仍不得创建、更新、启动或删除真实 Scheduled Task，也不得读取真实游戏、Steam userdata 或玩家存档。`starting` 5 分钟与 `protected` 45 分钟边界必须覆盖；真实安装态 runtime acceptance 仍按上一段人工 gate 执行。

存档目录自动发现切片至少运行聚焦测试：

```powershell
cargo test -p hmm-core save_directory
cargo test -p hmm-games-mhw save_directory
cargo test -p hmm-infra save_directory_scanner
cargo test -p hmm-infra steam_profile
cargo test -p hmm-infra pending_save_directory
cargo test -p hmm-app --test save_directory_discovery
cargo test -p hmm-tauri save_directory_discovery
cmd /c corepack pnpm run test -- src/features/profiles/profileSaveDirectoryDiscovery.test.mjs src/features/profiles/profileFrontendIntegration.test.mjs src/features/profiles/profileApi.test.mjs
```

## 并发与任务系统

适用范围：

- TaskManager
- event bus
- cancellation
- game write lock
- database transaction

必须覆盖：

- 多个扫描任务并行。
- 同一游戏实例写入串行。
- 不同游戏实例可并行准备。
- 任务取消后状态一致。
- 进度事件携带 task id。
- 安装失败不会留下半写入 manifest。

测试建议：

- 使用可控的 fake file system。
- 使用临时目录和小文件。
- 对锁顺序写单元测试或集成测试。

## 日志与审计

适用范围：

- logging / telemetry 初始化
- redaction helper
- task event
- audit log writer
- diagnostic export

必须覆盖：

- home 路径脱敏。
- 游戏目录路径脱敏。
- Steam ID 脱敏。
- token、API key、cookie 脱敏。
- 任务日志和进度事件都携带同一个 `task_id`。
- 写入、覆盖、删除、备份、恢复、manifest、回滚都会产生 Audit Log。
- 诊断包不包含真实存档、第三方 Mod 包、完整本地路径或明显敏感信息。

测试要求：

- 使用人工构造的路径和临时目录。
- 不读取真实游戏目录、真实存档或真实 Mod 包。
- 不把未脱敏日志写入仓库。

L1 安全 App Log 聚焦验证：

```powershell
cargo test -p hmm-infra app_log
cargo test -p hmm-app support_diagnostics
cargo test -p hmm-infra task_log
cargo test -p hmm-infra diagnostics_health

# Windows 上运行 Tauri 测试前先生成 ignored development sidecar。
cmd /c corepack pnpm run prepare:save-backup-worker-sidecar:dev
cargo test -p hmm-tauri
```

聚焦用例必须覆盖字段白名单/未知字段拒绝、home/game path、用户名、Steam ID、token/cookie/API key、
JSONL reader 兼容、UTC 日轮转、14 天保留、初始化/运行时写入失败稳定健康码、queued task 注册和游戏发现
聚合摘要。日志文件系统测试还必须覆盖 app-data capability 根内的 handle-relative 创建/打开/保留清理、
祖先目录替换或链接无法把写入/删除引向根外、外部 sentinel 保持不变，以及 Unix `logs`/`logs/app`
目录 `0700`、日文件 `0600`。测试只使用临时 app data、fixed clock 和人工敏感字符串；sidecar、日志和
诊断包均为 ignored 生成物，不能提交。

## 游戏适配器

适用范围：

- MHW:I adapter
- 后续 Rise / Wilds adapter
- 替换目标 catalog
- 前置依赖规则
- 游戏目录发现

必须覆盖：

- Steam library 扫描。
- 手动目录校验。
- 运行进程路径识别。
- `nativePC` 规则。
- 根目录 DLL 规则。
- 外观、武器、语音替换目标解析。
- 前置依赖检测。

测试要求：

- 平台相关逻辑用 trait 隔离。
- 不能要求测试机实际安装游戏才能跑基础测试。
- 真实游戏验证只作为手动 smoke test 记录。
- 前置依赖检测必须使用临时游戏目录 fixture，不读取真实用户游戏目录，也不能依赖 `D:\G\mh\mod-config` 之类的本地测试路径。
- MHW:I 前置依赖首批场景至少覆盖：
  - 必需文件缺失。
  - `loader-config.json` 无法读取。
  - `loader-config.json` 不是合法 JSON。
  - `enablePluginLoader` 不等于 `true`。
  - 已知签名命中后进入 `installed_verified`。
  - 签名未命中时降级为 `installed_unverified`，且只做 warning。
  - 本地规则文件缺失或损坏时映射为稳定的 `rules_unavailable` / `storage_*` 语义。
- 这一类改动建议至少运行以下聚焦验证：

```powershell
cargo test -p hmm-games-mhw prerequisite
cargo test -p hmm-app game_setup
cargo test -p hmm-tauri prerequisite
cmd /c corepack pnpm run test -- src/features/game-setup/gamePrerequisite.test.mjs src/features/dashboard/dashboardSetupStatusPanel.test.mjs
```

## 发布与打包

适用范围：

- `.github/workflows/`
- 打包脚本
- Tauri 配置
- 版本号

最小验证：

```powershell
cmd /c corepack pnpm run build
cargo test --workspace
```

建议补充：

```powershell
cmd /c corepack pnpm run prepare:save-backup-worker-sidecar:dev
cmd /c corepack pnpm run prepare:save-backup-worker-sidecar
cmd /c corepack pnpm run tauri:build
```

必须人工确认：

- 产物名称是否正确。
- Windows 打包是否正常。
- Windows 安装目录是否同时包含 GUI 主程序和 sibling `hmm-save-backup-worker.exe`。
- target-triple sidecar 源产物是否保持 ignored/untracked。
- installer 自动 cleanup 是否作为独立 gate 验证，不能由“bundle 包含 sidecar”代替。
- Linux / Steam Deck 相关说明是否仍为实验性。
- 自动更新策略是否与安全策略一致。

## 结果记录约定

最终回复、PR 描述或提交说明中应记录：

- 已执行：实际运行过的命令或手动验证。
- 未执行：因为脚手架缺失、依赖缺失、平台缺失或设备缺失而无法执行的验证。
- 风险：仍未覆盖但需要后续补测的路径。

不要把“应该能通过”写成“已通过”。
