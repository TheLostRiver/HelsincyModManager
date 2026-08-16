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

修改 `policy/`、`scripts/check-*.ps1`、`scripts/check-policy.mjs` 或 `.github/CODEOWNERS` 时，
还需运行治理 fixture：

```powershell
node --test scripts/check-policy.test.mjs
```

该测试使用临时 Git 仓库和人工最小文件；Windows 上会同时执行 Node 与 PowerShell 生产入口，
其他平台至少验证 CI 使用的 Node 入口。测试不得写入真实凭据、玩家数据或本地私有路径。

修改 `scripts/verify.ps1`、`scripts/verify.sh` 或 `.github/workflows/verify.yml` 时，还需运行统一入口
契约测试：

```powershell
node --test scripts/verify-entrypoints.test.mjs
```

该测试锁定 PowerShell/Bash 的命令顺序、非零退出传播和 CI 对 `verify.sh` 的委托。完整
`verify.ps1`/`verify.sh` 已包含 frontend tests 与
`cargo clippy --workspace --all-targets -- -D warnings`；运行完整入口后不需要为这两项再做一次重复的
全量手工补跑。针对当前改动的聚焦测试仍须单独执行并记录。

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

### T17 Slice 4C 批量导入结果与重试

Slice 4C 只使用人工、无路径 DTO，不读取真实第三方 Mod、来源目录、游戏目录、玩家存档或 AppData。聚焦入口：

```powershell
node --test src/features/mods/external-import/externalImportResultModel.test.mjs src/features/mods/external-import/useExternalImportTaskProgress.test.mjs src/features/mods/external-import/useExternalImportResultWorkflow.test.mjs src/features/mods/external-import/ExternalImportAction.test.mjs src/features/mods/external-import/externalImportApi.test.mjs
cmd /c corepack pnpm run test
cmd /c corepack pnpm run typecheck
cmd /c corepack pnpm run lint
cmd /c corepack pnpm run build
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\check-frontend-boundaries.ps1
```

必须覆盖：

- terminal taskId -> cursor `0` 权威结果查询、foreign/late request 与 batch/task generation 丢弃。
- exact-key result DTO、稳定 status/reason、opaque ID、页大小、重复 candidate 与路径/未知字段 fail closed。
- cursor append 去重、load-more/result-query/retry 重入保护和 partial success 汇总不依赖 event 计数。
- retry 只提交 `batchId + sealed selectionId`，新 taskId 复用 listener、early-event buffer、取消与终态状态机。
- 每个 terminal taskId 在首屏结果验证后至多刷新一次 Mod 库；刷新失败不覆盖导入/result 事实。
- 10,000 条人工脱敏 result 固定 5 次 warmup、40 次 sample，每次只验证最多 100 项的页，并输出 p95；
  固定同机预算为 `250 ms`，不得通过减样本、删字段或展开全量 DOM 制造虚假提升。该值不是跨机器 SLA。

浏览器 smoke 覆盖桌面和 `<=600px` 窄窗口下的 result loading、query failed、empty、partial-success、
load-more failed 与 retrying 状态；确认 heading/list、live status、alert、disabled action、长 ID 换行和
操作区堆叠可用，且 UI 不显示路径、XML、fingerprint、sandbox/cache/materialization 或安装事实。

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
- `src-tauri/crates/hmm-runtime/`
- `src-tauri/crates/hmm-cli/`

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

### CLI/runtime contract

CLI-0A/0B/1A/1B contract、只读 automation 与 CLI-2A/2B/2C 单项 lifecycle 的聚焦入口：

```powershell
cargo test -p hmm-app game_setup --no-fail-fast
cargo test -p hmm-app game_prerequisites --lib
cargo test -p hmm-infra prerequisite --no-fail-fast
cargo test -p hmm-infra contained_discovery --no-fail-fast
cargo test -p hmm-runtime game_automation --no-fail-fast
cargo test -p hmm-runtime install_automation --no-fail-fast
cargo test -p hmm-runtime backup_automation --no-fail-fast
cargo test -p hmm-runtime diagnostics_automation --no-fail-fast
cargo test -p hmm-runtime sandbox_write --no-fail-fast
cargo test -p hmm-runtime lifecycle_automation --lib
cargo test -p hmm-runtime composition::core_mod_lifecycle_tests
cargo test -p hmm-infra read_only_open --no-fail-fast
cargo test -p hmm-infra read_only_mod_import_catalog --no-fail-fast
cargo test -p hmm-app install_task --lib
cargo test -p hmm-app reinstall_task --lib
cargo test -p hmm-cli --lib cancellation
cargo test -p hmm-cli --test cli_contract
cargo test -p hmm-cli --no-fail-fast
cargo test -p hmm-tauri install_preflight_dto_tests --lib
cargo test -p hmm-tauri reinstall_dto --lib
cargo clippy -p hmm-app -p hmm-infra -p hmm-runtime -p hmm-cli -p hmm-tauri --all-targets -- -D warnings
cargo run -p hmm-cli -- --format json runtime status
```

CLI-0A contract 测试必须确认：

- `hmm-cli` 不依赖 Tauri，`runtime status` 不构造真实 composition 或访问真实文件系统。
- Production 禁止 `--data-dir`，并始终报告 `productionWritesAllowed=false` 与
  `writeCommandPolicy=disabled`。
- Sandbox 缺少数据根、使用相对路径、文件系统根或包含 `.` / `..` 时 fail closed。
- `sandbox_only` 只是策略声明；测试不得创建 marker、签发写 permit 或把它表述成写能力已开放。
- JSON/JSONL 每次输出完整合法 object，stdout 不混入日志，机器解析错误使用稳定脱敏 envelope。
- CLI-0A 的 clap human/help/error 输出不含 ANSI；`--no-color` 不改变机器契约。
- stdout/stderr 不回显 sandbox 绝对路径、用户名、Steam ID、真实 Mod/存档内容或内部错误文本。
- CLI-2C 的 `install apply/uninstall/reinstall/recovery apply` 具有稳定 parser contract，但
  Production 必须在 CLI policy 和 runtime 两层拒绝；`backup create/restore/background
  enable|disable` 和 `diagnostics export` 仍在 parser 边界不可达。

CLI-1A binary contract 使用测试进程创建的临时根：

```text
<data-dir>/
  config/games.json
  fixtures/
    steam/steamapps/...
    games/mhw-minimal/...
```

测试必须确认：

- `game status|scan|validate|prerequisites --game mhw` 只返回聚合状态、稳定 evidence/issue code 和计数。
- snapshot/envelope 不包含 game root、candidate root、prerequisite rule path、display label、自由文本
  message、用户名或 Steam ID。
- Sandbox 保存目录先限制在 `fixtures` canonical 边界；Steam VDF 声明的隔离根外 library 在读取
  app manifest 前被拒绝。
- prerequisite override 缺失时直接解析 bundled rules，不 seed 文件、不创建目录/lock/temp。
- 四命令执行前后的目录与文件内容树完全一致；JSONL 短命令恰好输出一个 terminal envelope。
- 自动测试不得执行 Production game 命令，避免读取测试机真实 AppData、Steam registry/library 或
  游戏目录；Production 只读行为通过纯路径策略、依赖边界和 Sandbox 等价 composition 验证。

CLI-1B install 子切片使用以下人工树：

```text
<data-dir>/
  config/games.json
  fixtures/games/mhw-minimal/...
  mod-import/results.json
  mod-import/sandboxes/<package-id>/nativePC/...
  install/
    manifests/             # 可省略
    recovery/              # 可省略
    reinstall-recovery/    # 可省略
    backups/               # 可省略
```

测试必须确认：

- `install plan|status|recovery scan|recovery preview` 支持 human/json/jsonl 与
  `hmm.cli/v1`，短命令 JSONL 只有一个完整 envelope。
- plan 复用 `InstallPlanningService` 和 game adapter，只返回稳定 ID、逻辑相对 target 与聚合计数；
  不返回 package file id、source/sandbox root、absolute target、manifest path 或 backup ref。
- Mod catalog read-only reader 在 catalog 缺失时不创建父目录；读取 v1 时只做内存投影，不回写 v2、
  不创建 `.lock`；任何 mutator fail closed。
- status/recovery 复用 app query services，读取前校验 Sandbox 固定 state roots 和 game root
  containment；profile/mod 路径型 ID fail closed 且错误 envelope 不回显输入。
- recovery 全量扫描拒绝持久化状态中的路径型/非规范 Mod ID；plan 投影拒绝含控制字符的 target，
  防止篡改状态或第三方文件名破坏 machine/human 输出契约。
- 四命令执行前后的完整目录/文件树相同，不创建 install、manifest、recovery、backup、marker、
  lock 或 temp 文件。
- 自动测试不得执行 Production install 命令，不读取真实 AppData、游戏、Mod、Steam 或存档；
  Production 零写入通过独立只读 composition、无 SQLite 依赖和 Sandbox 等价 no-write 测试证明。

CLI-1B backup/diagnostics 子切片使用以下人工树：

```text
<data-dir>/
  hmm.db                              # 测试创建的最小 SQLite schema/rows
  hmm.db-wal / hmm.db-shm             # 仅 fail-closed 用例创建的人工 sidecar
  fixtures/background/status.json   # fake registration status + fixed clock
  logs/
    app/app-YYYY-MM-DD.log
    tasks/task-<safe-id>.log
    audit/audit-YYYY-MM-DD.log
```

测试必须确认：

- `backup list --profile <id> [--limit N]` 只返回稳定 backup/game/profile ID、trigger/status、
  created/size/file count；不返回 archive/manifest 文件名、备份/存档目录、source label、notes、
  hash 或 Steam ID。
- backup facade 只读取已 checkpoint 且没有 `hmm.db-wal`/`hmm.db-shm` sidecar 的既有
  `hmm.db`，使用 percent-encoded immutable URI、SQLite read-only flags 和 connection-local
  query-only mode；缺失 DB 不创建文件/父目录，repository mutator 无法通过该 connection 写入，
  也不运行 migration/default seed。
- 任一 WAL/SHM sidecar 存在时，`backup list` 必须以 runtime unavailable 退出码和脱敏
  `backup_database_unavailable` fail closed；不得 checkpoint、修复、创建或修改 DB/WAL/SHM，
  stdout 不得回显路径，执行前后的完整目录树和文件 bytes 必须一致。
- immutable opener 不构成跨进程快照锁；自动测试只证明静止 fixture 和已存在 sidecar 的零写入
  行为，不证明桌面 writer 在 sidecar preflight 后启动时的 snapshot 一致性。需要一致结果的
  Production 人工验证必须先关闭桌面端。
- `backup background status` 只调用 registry `inspect` 和状态派生，不调用 register/unregister、
  不启动 worker、不获取 scheduler lease、不写 Audit Log；Sandbox 只使用 fixed JSON registry/clock。
- background projection 不包含 lease owner/expiry、worker instance、task name、SID、worker path、
  PowerShell/XML 或 raw platform output；持久化错误码和 ID 在输出前重新校验。
- `diagnostics snapshot` 复用 `/diagnostics` 的独立分类读取语义，只返回 bounded platform summary、
  App/Task/Audit status 与计数；不返回日志正文、来源文件名、Audit fields、完整本机信息或 export path。
- 三条命令支持 human/json/jsonl，JSONL 各输出一个完整 envelope；成功或 fail-closed 执行前后的
  完整目录/文件树相同，不创建 DB/WAL/SHM、日志目录、marker、lock、temp、备份或导出文件。
- 普通自动化不得执行 Production backup/diagnostics 命令，不读取真实 AppData/日志/存档，也不得
  inspect/register/update/start/delete 真实 Windows Scheduled Task。Production 行为由依赖边界、
  Sandbox 等价 composition 和 disposable VM 人工 gate 分开验证。

CLI-2B Sandbox 写许可测试只使用测试进程创建的临时根，必须确认：

- `runtime status` 和 CLI-1 只读命令仍不创建 marker；只有显式申请 write capability 时，空根才创建
  固定 `.hmm-sandbox.json` v1 marker。
- 非空无 marker、marker schema/内容篡改、marker link、Sandbox 根 link/junction/reparse point 均
  fail closed；Production 没有 capability 构造路径。
- capability 保留 no-follow 根目录句柄、canonical root 和稳定目录身份。Windows 必须阻止存活期间
  的祖先替换；允许 rename 的平台必须在重验时返回稳定 `sandbox_root_replaced`。
- 本次操作使用的 app-data、game、save、backup 根逐项执行 lexical + canonical containment；
  任一根在隔离范围外或经过 link/junction/reparse point 时整体拒绝。
- 正向、拒绝、marker 篡改、根替换和 link/junction fixture 都验证 Sandbox 外 sentinel bytes 不变。
- 测试不得读取真实 Steam、AppData、游戏、存档、日志或 Scheduled Task；marker/capability 不得被
  表述为 Production admission，也不得自动解锁 backup create 或 diagnostics export。

CLI-2C 单项 lifecycle binary E2E 继续只使用 TEMP/artificial fixture，必须确认：

- install/uninstall/reinstall/recovery apply 缺少 `--commit`、`--yes` 或 token 时不写；ready preview
  只签发 5 分钟 path-free token，过期或计划/source/manifest/recovery 变化后旧 token fail closed。
- install blocking plan 不签发 token；uninstall/recovery E2E 必须覆盖聚合计数不变但结构化
  manifest/recovery record 变化时旧 token 仍在 task/game write 前拒绝。
- install/reinstall preview 必须投影同一个 `prerequisiteDecision`。required missing 与 rules
  unavailable 为 `blocked` 且不签 token；`signature_unverified` 为显式 `warning` 且可继续。
  decision token facts 必须绑定 status、stable codes 与 rules version。
- 桌面 install/reinstall/initial-retarget runner 在获取 game/profile 写锁前完成最终 decision 重读；
  blocked 或 preview 后漂移必须在 manifest/source/snapshot/commit/staging/game write 前拒绝，
  并用测试证明 prerequisite provider 的规则读取、配置解析和 hash 不在写锁内发生。DTO 与 CLI
  projection 不得包含 issue path、自由文本 message、配置正文或本地绝对路径。
- apply 在 capability 装配前验证 token，并在共享 game/profile 写锁内重建业务事实、重验 token 和
  containment；写入继续复用 InstallPlan、backup、manifest、rollback/recovery 和 Audit。
- uninstall/recovery 还必须覆盖 `prepare_*` 已验证 token 后、runner 获锁前的同数量结构化状态漂移；
  锁内 state binding 重读应在 executor 前拒绝，并保持所有原目标和未确认目标不变。
- 真实 `hmm` binary 跨独立进程完成 install -> uninstall exact baseline，以及
  v1 -> v2 true reinstall -> uninstall exact baseline；reinstall 覆盖 retained/replaced/added/stale。
- manifest save failure 发出 rollback phase、恢复 v1 且 recovery scan 仍可解释；stale token 在
  task/game write 前拒绝；所有成功和失败路径保持 Sandbox 外 sentinel 不变。
- CLI JSONL 从 0 单调编号，每个已启动任务只有一个 terminal；Ctrl+C 首次协作式取消、确认后返回
  130，第二次中断不伪造 cancelled，不可抢占 commit 仍以真实 terminal 为准。
- Production 四条 lifecycle 写命令在 CLI policy 和 runtime 两层拒绝；测试不得读取或写入真实
  Steam、游戏、存档、AppData、Scheduled Task 或第三方 Mod。

CLI-3A 跨进程写入 admission 的聚焦入口：

```powershell
cargo test -p hmm-infra --test cross_process_write_admission -- --nocapture
cargo test -p hmm-app --no-fail-fast
cargo test -p hmm-runtime --no-fail-fast
cargo test -p hmm-cli --no-fail-fast

# 新 worktree 首次运行根 Tauri 测试前，先生成 ignored development sidecars。
cmd /c corepack pnpm run prepare:windows-sidecars:dev
cargo test -p hmm-tauri install_recovery_write_admission_errors_preserve_stable_codes_without_paths -- --nocapture
cargo check -p hmm-infra -p hmm-app -p hmm-runtime -p hmm-cli -p hmm-tauri --all-targets
```

要求：

- 独立子进程竞争同一 scope 时，一个持有，另一个在 deadline 返回 `write_admission_busy`；不同 profile
  和不同 scope 不得错误互斥。
- waiter cancellation 返回 `write_admission_cancelled`；逆序或同 scope 重入在平台等待前返回
  `write_admission_order_violation`。
- owner 不执行 guard Drop 而直接退出后，Windows 必须报告 `abandoned_owner`，Unix 必须报告
  `stale_owner_metadata`，后续仍执行正常锁内事实重验。
- 非法 namespace、link/reparse/symlink escape 和平台错误必须 fail closed 为
  `write_admission_unavailable`；machine/UI/Task 投影不得包含 mutex 名、lock path、SID、app-data path
  或原始平台错误。
- install/backup/background fake admission 的 busy 分支不得进入 committer、backup executor 或 registry
  mutation；restore 必须保持 `save -> game -> process-local game mutex`。
- runtime composition 测试确认 GUI、Sandbox CLI 与固定 worker 使用同一 coordinator；Production
  parser/runtime 写门禁继续拒绝，CLI-3A 不自动开放新 command。
- Unix `cap-std`/`fs2` 分支必须由 Ubuntu required CI 实际编译运行。Windows 本机结果不能声称覆盖
  Unix file-lock、no-follow 或路径替换回归测试。
- 自动化只使用 temp/artificial app-data 与受控 helper 子进程，不读取真实游戏、Steam、玩家存档或
  Scheduled Task。PR candidate 还必须运行一次完整 `scripts/verify.ps1`；安装态竞争和后台注册 scope
  另在 disposable Windows 环境执行人工 gate。

CLI-3A 首次认证 gate 已于 2026-08-16 完成：Ubuntu required CI run `31910573714` 实际覆盖 Unix
file-lock/no-follow/path-replacement；disposable Windows synthetic 环境覆盖 helper timeout/cancel/
abandoned owner、CLI game scope 竞争与释放、GUI/worker save scope busy fail-closed、释放后 backup 增长、
background registration enable/disable 双向竞争。最终 owned task 为 `Ready`、archive/manifest 为
`3/3`、live gate process 为 `0`。这些证据只认证 CLI-3A 共享互斥基础，不替代 CLI-3B 每个 Production
写命令自己的 capability、token、Audit、锁内事实和 Windows 验收。

CLI-0B shared composition 的聚焦入口：

```powershell
cargo check -p hmm-runtime -p hmm-tauri --all-targets
cargo test -p hmm-runtime -p hmm-tauri
cargo clippy -p hmm-runtime -p hmm-tauri --all-targets -- -D warnings
```

CLI-0B 测试必须确认：

- `HmmRuntime` 装配真实 repositories/services，但所有测试只使用 temp/fake/人工 fixture。
- Tauri `AppState` 的 headless/GUI 生命周期选择保持不变，GUI-only 维护不会被 worker 启动。
- worker 直接构造 `HmmRuntime`，参数仍只接受固定 `--once`。
- manifest repository 故障通过显式 builder 注入，重装失败仍回滚到原 manifest/baseline。
- install/reinstall/retarget/uninstall/recovery 继续共享 `TaskManager` 与 game/profile 写锁。
- Tauri observer 继续使用同一领域事件、Task Log allowlist、queued App Log 和 wire DTO。
- 测试不得访问真实游戏目录、Steam userdata、玩家存档、用户 AppData 或 Windows Scheduled Task。

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

### T18 Slice 4C production projection query

Slice 4C 继续只使用人工 Mod/metadata/category/manifest fixture、临时 JSON 和临时 SQLite。聚焦入口：

```powershell
cargo test -p hmm-app mod_library_projection -- --nocapture
cargo test -p hmm-app mod_library_query -- --nocapture
cargo test -p hmm-infra mod_library_projection -- --nocapture
cargo test -p hmm-tauri mod_library_commands -- --nocapture
cargo test -p hmm-tauri --release mod_library_read_model_baseline -- --ignored --nocapture
```

必须覆盖：

- production command 只调用 `AppState.mod_library_query`，不现场构造兼容 JSON 查询或静默回退。
- global/profile dirty、missing generation、fingerprint 不一致、未知 manifest status 和 dirty marker 写入失败均 fail closed。
- catalog/metadata/category 权威写入前后都标 global dirty；manifest 提交前后 best-effort 标 profile dirty，projection 失败不改变已提交 manifest 事实，但必须通过 freshness guard 阻断 stale query。
- count、clamp 后 page、rows、labels 和 profile status 在同一短 SQLite read transaction 与同一 complete generation 中完成。
- 搜索、category/status filter、NFKC query key、稳定 name/modId 排序、稀疏 `not_installed` 和完整 page DTO 与兼容语义一致。
- release harness 保持 1,000/10,000 fixture、5 次 warmup、40 次 sample 和完整 status filter；10,000 条 `sqliteProjectionStatusFilterQueryTotal` p95 固定不高于 `14.23 ms`，不得通过删字段、减样本或放宽门槛改善结果。

2026-07-22 最终代码同机复验：1,000 条 projection p95=`1.2963 ms`，10,000 条 p95=`9.2966 ms`，门禁通过。该 wall-clock 结果不是跨机器 SLA，ignored benchmark 不由普通 `cargo test --workspace` 自动执行。

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

### T13 批量生命周期分阶段矩阵

T13 的权威语义见 [批量 Mod 生命周期领域设计](BATCH_MOD_LIFECYCLE_DESIGN.md)。T13-00 只交付
设计与规划契约；T13-01/T13-02 已交付 `hmm-core` 批量计划模型、`hmm-ports` 事实/封存端口、
`hmm-app` service、批量 runner、journal 和 retry；T13-03 已交付批量卸载 facts/executor 与
锁内 manifest snapshot revalidation；T13-04 已交付 app 层批量真正重装 facts/executor、Mod 级稳定
摘要与结构化 recovery/committed 分类；T13-05 已把 install/uninstall/reinstall 接入 Sandbox
runtime/CLI，并为 same-revision retarget 提供纯只读 preview facts；T13-06 已落地 5 个窄 Tauri
command、camelCase/严格未知字段拒绝 DTO、feature-local typed API 与同步 terminal event（仅 Sandbox
模式可用）；T13-07 已落地批量 workflow（跨页选择、策略选择、preview/seal/start/result/retry
状态机）、预览/结果 UI、行为测试与 4 viewport smoke。T13-08 disposable Windows Gate C 已于
2026-08-05 通过主链和受控 partial failure -> retry 补充链，并标记为 `certified`。

| Task | 实现后必须覆盖的自动化 |
| --- | --- |
| T13-00 | policy、Markdown links、secret、whitespace、文件大小、`git diff --check`、完整 `verify.ps1` 与 findings-first 全 diff 自审 |
| T13-01 | 规范 item 顺序与 deterministic digest；operation/policy/revision/binding/target/preflight 任一变化使 digest 变化；duplicate、101 items、50,001 actions、超过 16 MiB plan 整体拒绝；Windows 规范化后的跨 item target conflict；preview 零写入；stop/continue 的 ready/blocked 与 token 规则；preview/plan token 过期、环境或 digest 不匹配 fail closed；seal 重读事实时 request/token/fact 任一漂移均返回 `batch_plan_stale`，且不留下部分 snapshot、journal、attempt 或 projection；原始 token 不持久化 |
| T13-02 | 批量 install 全成功、默认预检 blocker 整批零写入、首项成功后次项失败保留首项、continue 只越过 pre-write/rollback-succeeded failure；同一 attempt 重复 start 幂等返回同一 task；manifest save/rollback/journal/Audit before/after commit fault matrix；sandbox 外 sentinel 不变 |
| T13-03 | 批量 uninstall 只消费 manifest/installed summary/backup；target changed/missing/read failure、backup unavailable、invalid manifest 和 remove/restore overlap 阻断；同 revision binding/entry 漂移在写锁内拒绝；中途失败不伪回滚已成功项；restart 后可区分 succeeded/retryable/recovery required |
| T13-04 | 批量 true reinstall 的 retained/replaced/added/stale 与单项计划一致；installed/candidate revision、binding、target、original backup stale；manifest failure 回滚旧 revision；不完整 rollback 进入 recovery required；同 revision retarget 复用既有 snapshot/transaction |
| T13-05 | CLI JSON/JSONL schema、唯一 terminal event、exit code、partial result/retry、parser write gate、Sandbox containment、stale preview 零副作用和机器输出脱敏；CLI 不循环调用单项 command |
| T13-06 | 五个窄 Tauri command 的 camelCase DTO、未知字段拒绝、stable code、taskId/phase serialization、按 attemptNumber 绑定的分页（默认 50、最大 100）和 typed API wrapper；seal→start→result 端到端、重复 start 幂等、Production 拒绝；tauriContractCoverage 证明所有注册 command 已在契约文档登记 |
| T13-07 | 跨页多选累积（翻页保留、搜索/筛选/刷新清空）、批量 preview/确认（策略显式单选、blocked 项确认）、start 进度、分页 result、partial success 和 retry UI；选择变化使旧 batch plan 失效；manifest installedRevisionId 数据源；前端不计算 target/retryable/文件规则；1440x900/1366x768/1280x800/480x800 视觉 smoke（人工） |
| T13-08 | disposable Windows Sandbox 中用人工 fixture 完成 batch install -> restart -> batch true reinstall（含一个 Armor target switch）-> 受控 partial failure -> retry retryable 项 -> restart -> recovery 检查 -> batch uninstall -> exact baseline；核对 task/Audit/journal、manifest/binding、backup/recovery/staging 清理与 evidence health |

T13-05 Slice B/C 当前聚焦入口：

```powershell
cargo test -p hmm-cli --test cli_contract batch -- --nocapture
cargo test -p hmm-cli --lib --no-fail-fast
cargo test -p hmm-runtime batch --no-fail-fast
cargo test -p hmm-core batch --lib
cargo test -p hmm-app batch_install --lib --no-fail-fast
cargo test -p hmm-app batch_uninstall --lib --no-fail-fast
cargo test -p hmm-app batch_reinstall --lib --no-fail-fast
cargo test -p hmm-infra batch_lifecycle_repository --lib
```

T13-06 Tauri/typed API 聚焦入口：

```powershell
cargo test -p hmm-tauri --lib batch_mod_lifecycle
cargo test -p hmm-runtime batch --no-fail-fast
cmd /c corepack pnpm run test
cmd /c corepack pnpm run typecheck
```

T13-07 批量 UI/workflow 聚焦入口：

```powershell
cmd /c corepack pnpm run test
cmd /c corepack pnpm run typecheck
cmd /c corepack pnpm run lint
cargo test -p hmm-app install_manifest_query --lib
cargo test -p hmm-tauri --lib install_manifest
```

T13-07/T13-08 最终验收证据：

- release artifact SHA-256：
  `08EF5FF15DAFDC00790C0975FAA160C792AF487D47C186271E93D09D84AB8C8D`。
- `1440x900`、`1366x768`、`1280x800`、`480x800` 实际窗口 smoke 全部复验；没有路径泄漏，已发现的
  480x800 stacking、浅色面板和终态刷新缺陷在最终 artifact 修复后重新通过。
- Gate C 主链完成 install/restart/Alpha v2 true reinstall/Armor target switch/restart/recovery/uninstall，
  最终 9 文件/212 字节 baseline 的路径、大小和 SHA-256 全部一致。
- 受控 partial/retry batch `batch-94eedbc4-3006-4f76-aa39-b0d1bae71650`：attempt 0 task
  `install-1785897638158-0` 为 `completed_with_errors`（0/1/2，三项 retryable）；attempt 1 task
  `install-1785897713997-0` 为 3 成功。卸载 batch `batch-aab2d50e-7412-4694-9a7f-5433eed50b89`、task
  `install-1785897949309-0` 为 3 成功。
- 补充场景最终 manifest entries/bindings 与 profile status 投影为空，Recovery Center 归零，backup/
  recovery 标准目录为空且无 staging；10 文件/243 字节 baseline 精确一致，三个 attempt 的 evidence
  health 均未降级。
- 全部验收只使用人工 fixture 和 disposable Sandbox；不得用这些步骤替代真实玩家数据保护边界，也不
  因 Gate C 通过而开放 Production 写命令。

T13-03 app 层批量卸载聚焦入口：

```powershell
cargo test -p hmm-core batch --lib
cargo test -p hmm-app batch_uninstall --lib --no-fail-fast
cargo test -p hmm-app install_uninstall --lib --no-fail-fast
cargo test -p hmm-app install_task --lib --no-fail-fast
cargo test -p hmm-app batch_install --lib --no-fail-fast
cargo test -p hmm-runtime uninstall --no-fail-fast
```

这些测试只使用 fake/temp/人工 manifest 与字节，覆盖 provider 零写入、exact revision、Mod 级 manifest
snapshot digest、同 revision replacement binding 漂移、target/backup/manifest read failure、共享 ownership、
未知 sentinel、玩家修改文件、partial result、retry/recovery 分类、Audit degradation 和 commit cancellation
barrier；不读取真实游戏、Steam、存档或第三方 Mod。runtime/CLI batch uninstall E2E 已由 T13-05
跨进程 contract 覆盖，但不能替代上述 app-level 故障矩阵。

T13-04 app 层批量真正重装聚焦入口：

```powershell
cargo test -p hmm-app reinstall --lib --no-fail-fast
cargo test -p hmm-app reinstall_task --lib --no-fail-fast
cargo test -p hmm-app batch_reinstall --lib --no-fail-fast
cargo test -p hmm-app batch_install --lib --no-fail-fast
cargo check -p hmm-runtime
```

这些测试使用 fake/temp/人工 manifest、source、target、backup 和 binding，覆盖真实单项 preparation 的
retained/replaced/added/stale 投影、无关 Mod manifest 变化不误判 stale、candidate/source/target/
original backup/binding/prerequisite 漂移、same-revision retarget 分派、完整 token 的锁内复用、mixed
result、retry selection、rollback/recovery 分类、commit 取消屏障和 commit 后证据降级。既有 durable
reinstall fault matrix 继续覆盖 manifest failure 回滚旧 revision、rollback/repair required 与重启恢复。
runtime/CLI batch reinstall 与 same-revision Armor switch E2E 已由 T13-05 跨进程 contract 覆盖，
但不能替代这些 app-level 故障矩阵。

这些测试必须覆盖 `plan -> apply -> result/retry` 的跨进程 journal 路径、Production 写入拒绝、
脱敏 projection、`--commit --yes --preview-token` 门禁、partial exit code `5`，以及 stale
preview 在构造写 runtime 前失败且不创建 `hmm.db` 的零副作用行为。还必须覆盖两个独立 SQLite
连接竞争同一 game/profile admission 时最多一个 sealed attempt 原子进入 queued；同 scope sibling
active attempt 不阻断指定 batch/attempt 的只读 result；retry 创建新 attempt 后若最终 admission
竞争失败，只安全回收仍 sealed、没有 item result 且 verifier 匹配的未执行 attempt，无法证明时
fail closed。该测试只证明 Sandbox batch journal 的 admission，不代表 Production 通用写 admission。

所有实现切片还必须覆盖以下横向不变量：

- queued、prepare、item 间、commit 中和 rollback/recovery 中取消。Commit 成功时 item 必须是
  `succeeded`，取消只阻止后项启动。
- retry 只由后端从 sealed batch 选择 retryable item；成功项和 `recovery_required` 项不重放，
  revision/target/policy 变化必须创建新 batch；expected attempt 不匹配时拒绝，两个并发 retry 最多一个
  创建下一 attempt。
- 同一 game/profile 写入严格串行；plan/scan 在写锁外；item 间释放写锁。不同 game/profile 的
  并行仍受资源预算和现有 coordination 控制。
- 一个 batch task 恰好一个 terminal event，不能依赖 progress event 携带 item 明细。当前 T13-05
  Slice B/C 受每批最多 100 项约束，`result` 返回完整 bounded snapshot；后续 T13-06/T13-07 分页按
  `ordinal` 默认 50、最大 100。无论是否分页，result query/cursor 都绑定确切 attempt，retry 不能
  让旧结果或分页漂移。
- `plan` 的直接 response 可以返回 opaque `previewToken`；除此之外，result/progress/event/其他
  DTO、CLI stdout/JSON/JSONL、Task/Audit/diagnostics 不含完整路径、Windows 用户名、Steam ID、
  token/digest、target/hash 列表、backup/snapshot ref、manifest/source 正文或原始 error。原始
  preview/plan token 不持久化、不写日志，只允许保存单向 verifier/metadata。
- 所有测试只用 temp/fake/人工 fixture；不得读取真实 MHW:I、真实 Steam userdata、玩家存档或第三方
  Mod 包，也不得在普通 CI 操作真实 Windows Scheduled Task。

### Core Mod Lifecycle Gate A

核心 Mod 生命周期的正式自动验收入口为：

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify-core-mod-lifecycle.ps1
```

PowerShell 7 或非 Windows 环境使用同一脚本：

```powershell
pwsh -NoProfile -File ./scripts/verify-core-mod-lifecycle.ps1
```

脚本先从 `hmm-runtime` 测试列表发现 `headless_composition_*` 场景，要求发现数不少于固定基线 6，
并逐项确认 T19 计划记录的 6 个固定场景仍然存在；发现数为 0、少于基线、缺少固定场景或场景被
标记为 ignored 时均失败。发现成功后，脚本通过同一个公共前缀一次执行全部场景，并保留 cargo 的
非零退出码。Windows 新 worktree 会在发现测试前通过仓库既有 helper 准备 debug sidecar；生成的
`target/`、`node_modules/` 和 `src-tauri/binaries/` 内容仍为 ignored 运行时产物，不得提交。

GitHub `Verify` workflow 调用同一脚本，不在 YAML 中复制测试清单。场景继续只使用人工 zip、temp
AppData、temp MHW:I-like game root、fake port 和受控 staging/backup/manifest roots；该入口不得读取
真实 MHW:I、Steam userdata、玩家存档或第三方 Mod 包。

CL0/CL1 test-only composition harness 使用人工 zip、temp AppData 与 temp MHW:I-like game root。
CL0 覆盖 fixture 分类、真实 importer、持久化 import/game config、MHW adapter InstallPlan 和
runtime restart；CL1 在同一 harness 上覆盖 install -> restart -> uninstall -> baseline、manifest/
recovery counts、task identity/phase、Audit Log 字段白名单和公开证据脱敏：

```powershell
cargo test -p hmm-runtime composition::core_mod_lifecycle_tests
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
cargo test -p hmm-runtime composition::core_mod_lifecycle_tests
cargo test -p hmm-runtime composition::core_mod_lifecycle_tests::headless_composition_reinstalls_v1_to_v2_and_restores_baseline -- --nocapture
cargo test -p hmm-runtime composition::core_mod_lifecycle_tests::headless_composition_rolls_back_v1_when_reinstall_manifest_save_fails -- --nocapture
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

### CAT-01 装备 Catalog 候选数据治理

CAT-01 只校验人工 JSON 字符串和 schema 常量，不读取真实候选 catalog、第三方 Mod、游戏目录或
玩家数据：

```powershell
cargo test -p hmm-games-mhw --test equipment_catalog_candidate --no-fail-fast
cargo clippy -p hmm-games-mhw --all-targets -- -D warnings
```

测试锁定 candidate schema version、完整 SHA-256 stable ID、legacy ID 兼容字段、locale/alias 归一化、
active/hidden/dummy、provenance/licensing 与显式 bundling gate；负测覆盖绝对路径、`..`、大小写碰撞、
重复 stable ID、重复展示名、错误 path family、缺失许可审核事实和报告不回显候选值。CAT-01 只验证
`nativePC/wp/<family>/...` 的安全路径与 family 一致性；14 类 family、part/parser 与 transformer
契约已由 WR-01 设计冻结，运行时实现已从 WR-02A 开始。

### WR-01 / WR-02A 武器 Family、Parser 与人工最小 Catalog

WR-01 是文档设计，聚焦检查为 Markdown link/whitespace。WR-02A 已实现，固定聚焦入口为：

```powershell
cargo test -p hmm-games-mhw --test weapon_retarget --no-fail-fast
cargo clippy -p hmm-core -p hmm-ports -p hmm-games-mhw --all-targets -- -D warnings
```

测试只使用人工 family/main/part 路径和人工最小 catalog。覆盖 14 类普通/`bs_` main id、六类已知
副件映射、stable ID、alias、legacy id、unknown family/part、missing MOD3/MRL3 pair、大小写碰撞、
多 source、混合 family 和混合 install payload。完整 603-target catalog 不得从来源未明私有数据生成。

2026-08-05 候选验证：WR-02A 固定入口 15/15；当时 `cargo test -p hmm-games-mhw --no-fail-fast`
共 63 个 unit/integration test 与 doc-tests 全部通过；上述三 crate all-targets clippy 通过。WR-02A
交付范围没有 bundled weapon data、文件系统 I/O、binary transformer、staging 或真实游戏写入。

### WR-03A 武器 Binary Parser 与 Transformer

WR-03A 只使用完全人工构造的内存 bytes，固定入口为：

```powershell
cargo test -p hmm-games-mhw --test weapon_binary --no-fail-fast
cargo test -p hmm-games-mhw --no-fail-fast
cargo clippy -p hmm-core -p hmm-ports -p hmm-games-mhw --all-targets -- -D warnings
```

测试覆盖 MOD3/MRL3 magic/version/count/offset/bounds、texture/material/resource table、JAMCRC material
pair compatibility、unsafe/absolute/traversal/control reference、精确 source/target root、六类副件 normal
到 `bs_` mapping、ambiguous tail、255-byte 容量、跨 family、opaque timestamp、changed-range
postcondition、确定性 source/output/mapping digest 和错误/`Debug` 脱敏。2026-08-05 候选验证：固定入口
9/9；`hmm-games-mhw` 共 72 项及 doc-tests 全过；上述三 crate all-targets clippy 通过。

### WR-03B Transformer Staging / InstallPlan / Manifest

WR-03B 只使用人工 bytes、fake services 与 temp roots，固定聚焦入口为：

```powershell
cargo test -p hmm-core -p hmm-ports -p hmm-infra -p hmm-games-mhw -p hmm-app -p hmm-runtime
cargo clippy -p hmm-core -p hmm-ports -p hmm-infra -p hmm-games-mhw -p hmm-app -p hmm-runtime --all-targets -- -D warnings
cargo test -p hmm-runtime --test weapon_transform_lifecycle --no-fail-fast
```

测试覆盖 invocation/adapter facts 的 schema、上限与旧 JSON 缺省兼容，registry duplicate/unknown/stale
fail-closed，source/dependency/output/mapping digest 漂移，`.partial` 清理，大小写碰撞、target escape 和
symlink/junction containment。plan hash、reinstall token、batch digest、manifest/recovery 与 Audit
projection 都有直接断言；Audit 不得包含 digest、invocation 参数、texture path 或本地路径。

temp-root lifecycle 使用人工 MOD3/MRL3 bytes 证明 install -> JSON manifest restart -> same-revision target
switch -> JSON manifest restart -> manifest uninstall -> byte-for-byte baseline；既有事务测试继续覆盖 commit
failure/rollback success 和 rollback failure/recovery-required。2026-08-05 候选聚焦验证中，`hmm-app`
431 项、`hmm-infra` 308 项（另 3 项环境型 ignored）、`hmm-runtime` 66 项与 weapon lifecycle 1 项通过，
受影响六 crate 的 tests/doc-tests 和 all-targets clippy 全部通过。自动测试不得读取游戏原始 binary、
真实 Mod、真实游戏目录、AppData 或玩家数据。

### WR-04 Weapon Tauri / UI / Windows Gate D

WR-04 继续只使用人工 MOD3/MRL3 bytes、fake services、temp roots 和 disposable Windows Sandbox。
Production composition 必须保持 Armor-only；只有显式 `HMM_SANDBOX_DATA_DIR` 环境可以同时启用人工
Weapon seed 与 lifecycle root admission。固定聚焦入口为：

```powershell
cargo test -p hmm-games-mhw -p hmm-app -p hmm-runtime -p hmm-tauri
cargo test -p hmm-games-mhw developer_router_builds_content_sealed_weapon_plan_from_artificial_bytes
cargo test -p hmm-runtime composition::tests
cargo clippy -p hmm-core -p hmm-ports -p hmm-infra -p hmm-games-mhw -p hmm-app -p hmm-runtime -p hmm-tauri --all-targets -- -D warnings
cmd /c corepack pnpm run typecheck
cmd /c corepack pnpm run lint
cmd /c corepack pnpm run test
cmd /c corepack pnpm run build
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\check-frontend-boundaries.ps1
```

契约测试必须断言 `list_replacement_targets({ gameId, modId, query? })`、camelCase DTO、稳定错误码、
`catalogScope = production | developer_sandbox`，并拒绝 relative path、path-family、staging、digest 和
transform invocation 泄漏。人工 Weapon 正向测试应证明 2 个动作中恰有 1 个 sealed transform；Production
同类 source 返回 `weapon_developer_seed_unavailable`。Sandbox capability 正负测试还应覆盖合法 marker
与 app/game containment 放行、Sandbox 外 game root 拒绝、link/reparse/marker 篡改 fail closed，以及
Production environment 无法启用 developer seed。

Gate D 必须从最终 Tauri artifact 和全新的 disposable Sandbox root 开始。先记录人工 game fixture 的
相对路径集合、文件大小和 SHA-256，再通过 GUI 导入人工 Weapon fixture，完成 initial target install ->
关闭并重启 GUI -> same-revision true reinstall `one001 -> one002` target switch -> 关闭并重启 GUI ->
manifest uninstall。每一步记录 UI analysis/preview/result、task id、installed target、manifest/recovery 与
脱敏日志；最终断言 manifest 和 recovery 为空，backup/staging 无残留，game fixture 的路径集合、大小和
SHA-256 与初始 baseline 完全一致。不得使用真实游戏、存档、AppData 或第三方 Mod。

视觉 smoke 在 replacement analysis、preview、running/result、warning/error 等实际状态下检查
`1440x900`、`1366x768`、`1280x800`、`480x800`，并覆盖 light/dark/system。重点确认 modal 高于全局
“当前游戏”顶栏，窄屏可以滚动到全部内容和操作按钮，文本不截断、不重叠，不显示 package/game/
staging 路径或 path-family。只有 automated checks、视觉 smoke 和上述 exact-baseline 生命周期全部通过，
WR-04 才可标记 `certified`。

2026-08-06 Gate D 已按上述门禁标记为 `certified`：最终 artifact SHA-256 为
`156c42118c6620d803c1611397c55c1847ab782bb6505cd713c56a17398ea2af`，完整 `verify.ps1` 退出码 0，
Tauri 188 passed / 1 ignored。Sandbox task 为 initial install `install-1785952182807-1`、target switch
`install-1785953522595-0`、uninstall `install-1785955067791-0`；对应 Audit Log 均为 success。最终
manifest entries/bindings 与 recovery/staging 为空，10 文件/316 bytes 的路径、大小和 SHA-256 baseline
差异均为 0。light 覆盖四个固定 viewport，dark 覆盖 1280x800/480x800，system 覆盖 1366x768。

本次认证保留以下非阻断残余风险：全局顶栏目录状态陈旧、无元数据 Mod 的技术型 fallback 名称、空
NexusMods ID 显示 `null`、`weapon_binary_pair_incompatible` 的通用错误投影、设置页缺少主题入口，
以及 `max-width: 1360px` 下 `.window-tools` 被隐藏导致窄屏主题菜单不可达。后续相关修复应补聚焦 UI/
contract 测试；这些缺陷不改变本次已验证的 replacement modal 层级、滚动、路径脱敏和生命周期结果。

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
- 源根与递归子项的 symlink/junction/reparse point 逃逸拒绝；根外 sentinel 不得进入 archive。
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

SAVE-04 玩家存档恢复至少运行以下聚焦测试：

```powershell
cargo test -p hmm-app --test save_restore -- --nocapture
cargo test -p hmm-app --test save_restore_task -- --nocapture
cargo test -p hmm-infra save_restore -- --nocapture
cargo test -p hmm-infra --test save_restore_validator -- --nocapture
cargo test -p hmm-tauri save_restore -- --nocapture
cmd /c corepack pnpm run test -- src/features/profiles/profileSaveRestoreApi.test.mjs src/features/profiles/profileSaveRestoreTaskState.test.mjs src/features/profiles/profileSaveRestoreUi.test.mjs src/features/profiles/profileFrontendIntegration.test.mjs
node --test src/app/window-lifecycle/windowLifecycleUi.test.mjs
```

SAVE-04 自动化必须只使用 temp/artificial save、backup、SQLite 和 fake task/audit/clock fixture。必须覆盖
backup source 根与递归子项的 link/junction/reparse 拒绝及根外 sentinel 不归档、preview token identity/过期/stale、archive/manifest/hash/path/size/containment 拒绝、目标与 staging
摘要漂移、非 UTF-8/过长/过深相对路径、单组件长度和目录节点预算拒绝、默认开启与关闭 pre-restore 的二次确认、独立 `pre-restore/` 目录和普通 retention 排除、
pre-restore 备份失败 fail closed、共享 game/profile 写锁串行、commit barrier 取消、目录交换成功、
rollback 与 recovery-required 证据保留、`Committing -> Committed -> Completed` 持久化顺序、finalize 部分成功后的幂等重试，以及 durable `Completed` 后 Task/Audit evidence degradation
不得伪造业务失败。取消测试还必须故障注入事务终态写入失败，断言 prepared staging 不清理、未完成事务
继续阻断、runner 发送 recovery-required 且 Audit 使用稳定错误码。前端测试必须锁定 command 名、camelCase
DTO、精确 `taskId + kind + phase + status` 匹配、early-event buffer、recovery-required 覆盖乐观 cancelled、
未保存设置时恢复入口禁用、rolled-back cleanup warning 可见、Modal 终态可见和无路径/manifest/hash 字段。
退出保护还必须覆盖 active restore 拒绝完全退出、runner 释放 scope 后才可原子关闭后续 restore admission、
`blocked` DTO 不携带授权，以及 blocked UI 只显示返回应用/收起托盘而不显示 override exit。

真实存档或 Windows 桌面恢复验收只能在 disposable 一次性账户/VM 中使用人工最小 fixture；不能读取
真实 Steam userdata、游戏目录、玩家存档或真实 Scheduled Task。SAVE-04 在该人工 gate 完成前只能记录为
“实现完成、等待验收”，不得写成 `certified`。

SAVE-05 retention 与备份中心使用 fake repository 与 temp filesystem，至少覆盖：

- count/age/space 单独及组合规则、边界时间和确定性排序。
- 最新普通备份与所有 `pre_restore` 保护点不被普通 retention 删除。
- 保护点或问题项导致预算无法收敛时返回 blocked，而不是突破保护或伪报成功。
- `Completed -> RetentionPending -> DeletedByRetention | RetentionPartial` 持久化事实链。
- begin intent 失败、archive/manifest 单项缺失、单项删除失败、最终 DB 写回失败与下一次重试收敛。
- link/junction/reparse、目录或文件 identity 替换 fail closed，外部 sentinel 不受影响。
- 跨 Profile 分页、筛选、聚合、备注，以及确认 Steam 展示 snapshot 的 migration/保留/清空语义。
- 同 game/profile 的 queued/running backup、备份末尾 retention、显式 retention 与 restore 共享维护 scope；
  双向冲突均 fail closed，不同 scope 不受影响。
- task 创建失败或 panic、runner error/panic、queued restore abort 和 terminal 路径都会释放维护 scope；restore
  退出 admission 仍只统计 restore task，不把普通备份或 retention 计入 active restore。

前端聚焦测试覆盖 `/backups` 路由、loading/empty/error/partial/blocked、筛选分页、备注、整理反馈、
“立即整理”二次确认及取消默认焦点、搜索/空间预算输入边界、持久化头像 URL 二次白名单和 SAVE-04 恢复入口。
浏览器或 Windows 人工 smoke 至少覆盖 `1440x900`、`1280x800` 和 `480x800`，不得出现横向滚动才能发现
恢复操作的布局。所有 fixture 仍只能使用 temp/artificial 数据。

SAVE-05 已于 2026-08-16 按上述边界完成认证。disposable Windows synthetic gate 覆盖数量、年龄、空间、
保护点 blocked、manifest 锁定 partial/释放后重试、备份中心恢复入口和完全退出后的持久化复核；最终
archive/manifest 为 `3/3`、`pre_restore=1/1`、需处理为 0。证据与候选信息记录在
[SAVE-05 Retention 与备份中心设计](SAVE_BACKUP_RETENTION_CENTER_DESIGN.md)。

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

P7.2c installer ownership cleanup 至少运行以下聚焦检查；真实安装器验收按
[Windows 存档后台任务安装器清理人工 Smoke](testing/windows-save-backup-installer-cleanup-smoke.md)
执行：

```powershell
node --test scripts/windows-installer-cleanup-config.test.mjs scripts/prepare-windows-sidecars.test.mjs
cargo test -p hmm-infra save_backup_installer_cleanup
cargo test -p hmm-infra save_backup_background_registry::tests
cargo test -p hmm-tauri installer_cleanup
```

聚焦测试必须锁定 helper 只执行 `Identity -> InstallerCleanup` 两次受控进程调用；单个 cleanup
PowerShell 操作内部必须在删除前两次复核 owner/state，并在删除后 read-back。foreign 与 busy 分支必须在
`Unregister-ScheduledTask` 前返回，post-delete owned/foreign 分别映射为 removal/ownership unverified。

Windows sidecar 准备脚本必须仅对 `windows-msvc` 目标追加静态 CRT 构建标志，并在复制 bundle 输入前
拒绝仍导入 `VCRUNTIME140`、`MSVCP140` 或 UCRT runtime API 的产物。disposable VM 不预装 Visual C++
Redistributable；安装器 helper/worker 不能把该运行库作为隐性前提。

Windows packaging build gate 只生成并检查 artifact，不安装或运行 installer：

```powershell
corepack pnpm tauri build --bundles nsis --debug
corepack pnpm tauri build --bundles msi --debug --config tauri.msi-build-test.conf.json
```

MSI 版本覆盖文件仅用于本地验证，不能提交；使用 WiX `dark.exe` 反编译确认最终 MSI 同时包含
`RunInstallerCleanup`、cleanup helper `FileKey`、`Before="RemoveFiles"` 和
`REMOVE="ALL" AND NOT UPGRADINGPRODUCTCODE`。只有一次性 Windows 账户或 disposable VM
完成 interactive/silent uninstall、upgrade/repair/modify、foreign/running/owned task 矩阵后，
才能记录 P7.2c runtime acceptance；普通自动化不得创建、运行或删除真实 Scheduled Task。

后台保护注册后的首次运行还必须覆盖以下契约：

- register mutation 先返回完整 task read-back，Rust 必须复验 owner、SID、action、固定 `--once`、trigger、
  settings 和 canonical non-link worker；漂移或 foreign owner 时不得进入首次启动阶段。
- 首次启动只能使用 infra 内部构造的同一 `ScheduledTaskSpec`；PowerShell 对需要启动的 `Ready` task
  在启动前执行两次 exact-owned read-back，并只允许 `Start-ScheduledTask -InputObject`，不得按 task name
  盲启；启动后还要复验 exact 与 `Ready/Running/Queued` 状态。
- task 为 `Ready` 时发起一次启动；已为 `Running/Queued` 时不重复启动；其他状态 fail closed。
- 启动命令失败、启动前 TOCTOU 漂移或启动后 read-back 不 exact 时，register 不得返回 `Registered`，
  也不得写入或伪造 worker heartbeat。
- `inspect()` 保持纯只读，不启动任务。自动化使用 fake runner 与 PowerShell 静态契约；真实首次运行、
  fresh heartbeat 和 Settings 自动收敛只在 disposable Windows Sandbox/VM 验收。
WiX 会把外部 EXE custom action 的所有非零返回统一投影为 MSI `1722/1603`；人工验收必须同时读取
安装目录与 task 的聚合状态来确认 fail-closed，不得把通用 MSI 返回误记为 helper 原始 `20/21/22/23`。
交互提示只能使用固定、非敏感的操作建议，不能显示 task name、SID、路径、XML 或 helper 原始输出。

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

要求：SQLite repository 使用临时数据库；service/worker/exit guard 使用 fake registry、fake repositories 和 fixed/sequence clock；enable/disable 必须覆盖并发转换串行，global heartbeat 必须覆盖 cycle completion timestamp 与正常业务 skip。registry 测试必须锁定当前用户 SID 在同一进程内复用；Windows 只读链路必须锁定为当前进程 token 的原生 SID 读取与 Task Scheduler COM inspect、完整字段映射、账户名到 SID 归一化和异常 fail closed，且不得启动 PowerShell。register/update、start、unregister 与 installer cleanup 仍走既有 PowerShell mutation 安全链。register/update 与 unregister 分别在单个 mutation 命令中完成 ownership 检查和最终 read-back，app service 不得在成功 mutation 后追加重复 inspect。前端测试锁定 Settings 唯一控制入口、只有开关控件可触发启停、检查/启停动态反馈、页面 remount 不自动重检、显式刷新、Profile 只读、稳定 status/reason/code、未知 runtime 值的 fail-closed fallback 和 unsafe no-remember。启用后的有限自动复查必须覆盖约 0.75 秒、3 秒、1、5、10、16 分钟累计节点，短周期读回不得降低 fresh-heartbeat 判定。普通自动化与 `verify.ps1` 仍不得创建、更新、启动或删除真实 Scheduled Task，也不得读取真实游戏、Steam userdata 或玩家存档。`starting` 20 分钟与 `protected` 45 分钟边界必须覆盖；真实安装态 runtime acceptance 仍按上一段人工 gate 执行。

Windows 安装态退出生命周期必须在 disposable Sandbox/VM 额外验证：点击“完全退出应用程序”后，
窗口应立即隐藏，且 5 秒内 `hmm-tauri` 与其 `msedgewebview2` 子进程均不存在；托盘收起/恢复仍可用；后台保护 unsafe
确认仍先经过后端 guard，明确 override 时仍先完成最小 Audit。App Log 应依次包含
`application.exit_requested`、`application.exit_request_received`、`application.exit_started` 和
`application.event_loop_stopped`。缺少后两项时先按事件循环/资源清理故障调查，不能通过 CIM、
`taskkill` 或卸载器关闭提示把该 case 记为通过。证据还必须记录 `application.exit_guard_evaluated.duration_ms`
和从点击退出到进程消失的实际耗时，以区分实时 Task Scheduler 读回与 Tauri/WebView2 资源清理延迟。
SAVE-04 人工验收还必须在 artificial fixture 的 restore queued/running 阶段请求完全退出：窗口必须恢复并显示
不可 override 的恢复保护提示，不能终止 `hmm-tauri` 或显示“仍然退出”；收起托盘后 restore 必须继续达到
terminal，随后才允许完全退出。该步骤只可在 disposable VM/Sandbox 进行，不能使用真实玩家存档。

SAVE-04 当前已按上述门禁完成认证；证据矩阵和候选哈希记录在 [SAVE-04 验收记录](SAVE_04_ACCEPTANCE.md)。

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

LOG-01 Task/Audit retention 聚焦验证：

```powershell
cargo test -p hmm-infra log_retention
cargo test -p hmm-infra task_log
cargo test -p hmm-infra audit_log
cargo test -p hmm-infra text_log
cargo test -p hmm-infra diagnostics_health
cargo test -p hmm-runtime retention --no-fail-fast
cargo test -p hmm-app support_diagnostics --no-fail-fast
cargo test -p hmm-tauri diagnostics --lib --no-fail-fast
node --test src/features/diagnostics/diagnosticsPage.test.mjs
cargo clippy -p hmm-ports -p hmm-infra -p hmm-app -p hmm-runtime -p hmm-tauri --all-targets -- -D warnings
```

必须覆盖含当天在内的 Task 30 天 mtime 边界、Audit 90 天合法 UTC 文件名边界、未知/非法/non-regular
entry 保留、Task/Audit 类别独立失败、稳定 retention health code/count、write/post-commit 严重度优先级，
以及 Windows junction / Unix symlink 下根外 sentinel 不读、不写、不删。共享 composition 测试必须证明
完整 runtime 启动执行一次清理；只读 automation 保持无清理副作用。DTO、support diagnostics JSON 与
前端类型必须包含 retention 状态/计数且不新增路径或原始错误字段。所有文件系统用例只使用 temp/fake/
人工日志，不读取或清理真实 AppData、游戏目录、存档、Steam 或第三方 Mod。

LOG-02 日志总空间预算聚焦验证：

```powershell
cargo test -p hmm-infra log_storage_budget -- --nocapture
cargo test -p hmm-infra managed_log -- --nocapture
cargo test -p hmm-infra app_settings_repository -- --nocapture
cargo test -p hmm-app app_settings -- --nocapture
cargo test -p hmm-app support_diagnostics -- --nocapture
cargo test -p hmm-runtime shared_runtime_ -- --nocapture
cargo test -p hmm-runtime invalid_persisted_log_budget -- --nocapture
cargo test -p hmm-runtime corrupted_log_settings -- --nocapture
cargo test -p hmm-tauri dto_tests -- --nocapture
cargo test -p hmm-tauri diagnostics -- --nocapture
node --test src/features/diagnostics/diagnosticsPage.test.mjs
```

必须覆盖默认 128 MiB、显式最小 1 MiB、旧 schema 缺失字段兼容和损坏/非法 settings 回退；清理顺序
必须证明 Debug/Task 同层按最旧、再 App、最后仅删除 30 天硬下限之外的 Audit。当前 UTC 日 App/Debug、
最近 30 天 Audit、未知/非法/non-regular/link/junction/reparse entry 必须保留。用例还必须覆盖 16 KiB
维护 Audit reserve、受保护或超大文件导致的 `unsatisfied`、类别独立失败、目录漂移和文件替换复验，
以及维护 Audit 至多写一条且不会递归触发第二次清理。

settings command/DTO 测试必须证明 Tauri 只接受和返回 `{ maxBytes }`，使用 camelCase，非法值返回稳定
`log_storage_max_bytes_invalid`，不暴露日志路径、文件名或删除参数。diagnostics Rust/TypeScript 契约
必须覆盖 `logStorageStatus` 与三类计数。所有测试只使用 temp/fake/人工日志；不得读取或清理真实
AppData、游戏、Steam、存档、Scheduled Task 或第三方 Mod。

LOG-03 Debug Log 聚焦验证：

```powershell
cargo test -p hmm-infra debug_log --lib
cargo test -p hmm-infra log_retention --lib
cargo test -p hmm-app app_settings --lib
cargo test -p hmm-app support_diagnostics --lib
cargo test -p hmm-runtime composition --lib
cargo test -p hmm-runtime diagnostics_automation --lib
cargo test -p hmm-tauri dto --lib
node --test src/features/settings/debugLogSettings.test.mjs src/features/diagnostics/diagnosticsPage.test.mjs
cmd /c corepack pnpm run typecheck
```

必须覆盖默认关闭、旧 settings 缺字段兼容、显式开启持久化、损坏 settings fail-closed、保存失败不改变
进程内开关，以及禁用时不创建 `logs/debug`。writer 只接受固定 schema 与稳定 code/ID/数值字段；路径型、
自由文本、credential、manifest/hash/Mod/save 内容必须拒绝且累计稳定 health。7 日 UTC retention 必须保留
边界日、非法日期、未知/non-regular/link/junction/reparse entry；Debug 类别失败不得阻断 Task/Audit。
diagnostics page/export、CLI snapshot、Tauri DTO 和 TypeScript 类型必须包含 Debug status/count/line count，
但不得返回日志目录、任意文件名参数、原始错误或正文到只读 CLI。所有文件系统用例只使用 temp/fake/人工
日志，不访问真实 AppData、游戏目录、Steam、存档或第三方 Mod。

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
