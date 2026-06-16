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

## 存档备份

适用范围：

- 手动备份
- 自动备份
- 备份恢复
- 保留策略

必须覆盖：

- 默认备份目录。
- 用户自选备份目录。
- 备份 manifest。
- 恢复前校验。
- 保留数量限制。
- 备份目录不可写。

测试要求：

- 使用临时目录模拟存档目录。
- 不读取或写入真实玩家存档。

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
cmd /c corepack pnpm run tauri:build
```

必须人工确认：

- 产物名称是否正确。
- Windows 打包是否正常。
- Linux / Steam Deck 相关说明是否仍为实验性。
- 自动更新策略是否与安全策略一致。

## 结果记录约定

最终回复、PR 描述或提交说明中应记录：

- 已执行：实际运行过的命令或手动验证。
- 未执行：因为脚手架缺失、依赖缺失、平台缺失或设备缺失而无法执行的验证。
- 风险：仍未覆盖但需要后续补测的路径。

不要把“应该能通过”写成“已通过”。
