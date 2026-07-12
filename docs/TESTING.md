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
