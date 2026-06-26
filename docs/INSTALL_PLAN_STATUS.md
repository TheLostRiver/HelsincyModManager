# InstallPlan 模块现状

本文档记录当前 `InstallPlan` 模块已经落地的能力、尚未落地的边界和后续切片顺序。它用于回答“现在能依赖什么”，长期设计仍参考 [Mod 安装方案规划](mod_installation_strategy.md)，跨前后端通信契约参考 [前后端通信契约](FRONTEND_BACKEND_CONTRACT.md)。

## 模块目标

`InstallPlan` 是 Mod 安装链路的安全边界之一。任何真实游戏目录写入都必须从后端生成或重建的安装计划开始，再进入备份、提交、manifest 和回滚链路。

当前目标是先交付可测试的安全物理复制 MVP：

```text
已导入 Mod 的受控 sandbox
  -> game adapter 声明允许安装根
  -> InstallPlan
  -> InstallCommitService
  -> backup / write / manifest
  -> rollback on failure
```

## 已落地能力

### 领域模型

位置：

- `src-tauri/crates/hmm-core/src/install.rs`

已包含：

- `ModId`
- `ProfileId`
- `PackageFileId`
- `InstallTargetPath`
- `FileLayer`
- `InstallFileProvider`
- `InstallAction`
- `InstallConflict`
- `InstallPlan`
- `InstallManifestEntry`
- `InstallManifest`

当前 `InstallTargetPath` 只接受规范化后的相对目标路径，并拒绝：

- 空路径。
- 绝对路径。
- `..` 父级穿越。
- Windows 盘符前缀。
- 空段或 `.` 段。
- 不在 adapter 允许根下的路径。

`InstallPlan::from_providers` 已按目标路径聚合 provider。当同一目标路径存在相同 layer priority 的多个 provider 时，会产生阻断冲突；当 priority 不同时，会生成按 priority 排序的安装动作。

### 应用层计划生成

位置：

- `src-tauri/crates/hmm-app/src/install.rs`

已包含：

- `BuildInstallPlanRequest`
- `BuildImportedModInstallPlanRequest`
- `InstallPlanningService`

当前有两种计划输入：

- 测试和低层验证使用的显式文件输入：调用方传入 `allowed_target_roots` 和文件摘要。
- 正式前端优先使用的后端驱动输入：前端只提交 `gameId`、`modId` 和 layer 摘要，后端从已持久化导入结果定位受控 sandbox，再结合 game adapter 的允许安装根生成 `InstallPlan`。

正式前端不应传入最终安装路径、sandbox/cache 路径、导入包路径、游戏目录路径、备份路径或 manifest 路径。

### 应用层提交服务

位置：

- `src-tauri/crates/hmm-app/src/install.rs`

已包含：

- `CommitInstallPlanRequest`
- `InstallCommitService`
- `InstallCommitPhase`
- `InstallCommitError`

当前提交流程：

```text
检查 plan 是否存在阻断冲突
读取同 profile 的旧 manifest
读取 source file
读取目标文件旧状态
覆盖前备份已有文件
写入目标文件
生成并合并 InstallManifest
保存 manifest
失败时 best-effort rollback
```

manifest 合并规则仍保持 MVP 范围：提交服务只按本次实际写入的目标路径替换旧条目，并保留未触达的旧条目。替换已有托管目标时，新的 manifest entry 会继承旧条目的长期 `backup_ref` 语义；本次提交为中间状态创建的 pending backup 只用于失败回滚，提交成功后会 best-effort 清理。它不会因为 `modId` 相同就删除旧条目，避免在重装包内容变少时让 manifest 忘掉仍留在游戏目录里的托管文件。卸载、修复扫描和 rich status 仍需后续切片补齐。

当前回滚能力：

- 写入新文件后失败：删除已写入的新文件。
- 覆盖旧文件后失败：恢复旧文件内容。
- manifest 保存失败：回滚已写入文件。
- 写入失败且已生成备份：清理 pending backup。

### ports 与 infra

接口位置：

- `src-tauri/crates/hmm-ports/src/install.rs`

已包含：

- `InstallSourceFileReader`
- `InstallGameFileSystem`
- `InstallBackupStore`
- `InstallManifestRepository`

文件系统实现位置：

- `src-tauri/crates/hmm-infra/src/install_commit.rs`

已包含：

- 受控 source root 下的文件读取。
- 受控 game root 下的目标文件读写和删除。
- 受控 backup root 下的备份写入和清理。
- 受控 manifest root 下的 JSON manifest 读取和保存；读取会拒绝不安全 profile id、非真实目录的 manifest root、manifest symlink 和 profile id 不匹配的内容。

文件系统实现会拒绝路径穿越、绝对路径、Windows 盘符前缀、symlink 目标和 symlink ancestor 逃逸。测试使用临时目录，不依赖真实 MHW:I 安装目录、真实存档或真实第三方 Mod 包。

### Tauri command 与任务入口

位置：

- `src-tauri/src/install_commands.rs`
- `src-tauri/src/dto.rs`
- `src-tauri/src/state.rs`
- `src-tauri/crates/hmm-app/src/install_task.rs`

已包含 command：

- `preview_install_plan`
- `preview_imported_mod_install_plan`
- `start_install_task`

`start_install_task` 只接收：

- `gameId`
- `modId`
- `profileId`
- `layerName`
- `layerPriority`

安装任务已接入 `TaskKind::Install`，commit 阶段按 `gameId/profileId` 写锁串行。plan build、sandbox 文件扫描和只读分析不持有写锁。

当前安装任务阶段：

- `install.queued`
- `install.plan.building`
- `install.commit.processing`
- `install.completed`
- `install.failed`
- `install.cancelled`

任务事件和 Audit Log 不应携带完整本地路径、用户名、Steam ID、sandbox/cache 路径、真实 Mod 包内容或 manifest 正文。

### 前端最小接入

位置：

- `src/features/mods/modInstallPlanApi.ts`
- `src/features/mods/modInstallPlanTypes.ts`
- `src/features/mods/InstallPlanPreviewPanel.tsx`
- `src/features/mods/ModLibraryPage.tsx`

已包含：

- `previewInstallPlanForImportedMod`
- `startInstallTask`
- 最小安装计划预览面板。
- 从 Mod 库触发最小安装任务。
- 按 `taskId` 订阅 `hmm://task-progress` 安装事件。
- 展示 `install.queued`、`install.plan.building`、`install.commit.processing`、`install.completed`、`install.failed` 和 `install.cancelled`。
- 处理 `start_install_task` 返回前进度事件先到达的竞态。
- 通过 `get_install_manifest_status` 在 Mod 库加载成功和安装任务完成后刷新 manifest 状态摘要。
- 展示 `not_installed`、`installed`、`repair_required`、`unknown` 等后端摘要状态；当前 MVP 会对旧 manifest 根据匹配 entries 派生 `installed`，缺失 manifest 或无匹配 entry 显示 `not_installed`。

当前前端只能展示后端返回的计划摘要、冲突摘要、任务事件状态和 manifest 查询摘要，不应推断 MHW 路径规则或自行拼接安装路径。任务状态仍是页面内存态；页面刷新、重新进入后的安装事实应通过 manifest 查询恢复，而不是依赖内存任务状态或 mock 数据。

## 尚未落地能力

以下能力仍不能视为已完成：

- 卸载：尚未实现基于 manifest 的 uninstall。
- 恢复扫描：尚未实现启动时扫描半完成安装、`rollback_required` 或 `repair_required` 状态。
- Profile 工作流：`profileId` 已进入链路，但 profile 启用/禁用、批量切换、优先级管理仍未完成。
- 依赖和前置检查：尚未在安装提交前接入完整 dependency/preflight 阻断。
- ARMOR_RETARGET staging：设计上依赖 InstallPlan，但当前尚未把 retarget materialize 产物接入 InstallPlan 输入。
- Manifest rich 状态检测：当前已提供只读 manifest 状态摘要 command 和前端展示，但旧 MVP manifest 尚未记录 hash/status，暂不能检测目标文件缺失、backup 缺失或外部修改并自动标记 `repair_required`。
- Rich manifest：当前 manifest 仍是 MVP 形态，尚未包含 backend、status、hash、replacement binding snapshot、created/completed time 等长期字段。
- Crash recovery：当前提交失败会 best-effort rollback，但不等同于跨进程崩溃恢复能力。

## 文档现状与分工

- [架构设计](ARCHITECTURE.md)：记录安装必须经过计划、manifest、备份和回滚的原则。
- [Mod 安装方案规划](mod_installation_strategy.md)：记录长期方案和可选后端，不代表当前全部已实现。
- [前后端通信契约](FRONTEND_BACKEND_CONTRACT.md)：记录当前 Tauri command、DTO、错误码和任务事件契约。
- [InstallPlan MVP 待办](INSTALL_PLAN_MVP_TODO.md)：记录后续切片、验收标准、安全门禁，以及 manifest 状态、卸载/恢复、安装 UI、retarget staging 和测试矩阵的细化规则。
- 本文档：记录当前实现状态和后续切片判断。

## 后续建议切片

建议继续按下面顺序推进：

1. 基于 manifest 的 uninstall：不根据当前 Mod 包猜测已安装文件。
2. Crash/recovery 扫描：启动或进入安装页时识别半完成状态，并给出恢复或人工处理路径。
3. Rich manifest / repair 检测：补齐 backend、status、hash、replacement binding snapshot 和时间字段，支持 `repair_required` 的真实检测。
4. ARMOR_RETARGET staging 接入：让 retarget 产物作为受控 provider 输入 InstallPlan。
5. 依赖/preflight：在提交前阻断缺失必需前置和高风险安装状态。

## 验证基线

涉及 InstallPlan、manifest、backup、rollback、Tauri command 或任务事件时，至少参考 [测试指南](TESTING.md) 中“安装、卸载与回滚”“Tauri / Rust 桥接改动”“并发与任务系统”“日志与审计”章节。

提交前优先执行：

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify.ps1
```

如只修改文档，至少应检查 Markdown 内链、空白和文档职责是否重复。
