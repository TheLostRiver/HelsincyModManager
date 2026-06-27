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
- `InstalledFileSummary`
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

新写入的 manifest entry 会记录 `installed_file` 摘要：写入内容的字节数和 SHA-256。该字段只描述本工具本次写入到目标路径的内容，不记录完整本地路径、sandbox/cache 路径或文件内容。旧 manifest 缺少该字段时仍可兼容读取，但后续自动卸载或修复检测不能把缺少摘要的旧 entry 当作可安全删除/恢复的充分事实。

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

恢复记录基础：

- `hmm-core` 已新增 game-independent `InstallRecoveryRecord`、`InstallRecoveryRecordEntry` 和 `InstallRecoveryRecordStatus`，状态值使用稳定 `snake_case` 序列化。
- `InstallRecoveryRecordStatus` 已约束 `planned -> committing -> completed`、`committing -> rollback_required`、`rollback_required -> rolled_back` 等受控迁移；没有进入 `committing` 的持久化事实时，不能直接生成 `rollback_required`。
- `hmm-ports` 已新增窄 `InstallRecoveryRecordRepository` trait；`hmm-infra` 已提供 `JsonInstallRecoveryRecordRepository`，在受控 app data root 下用 profile/mod id 的派生文件名持久化记录，避免把任意 id 当作路径片段。
- 安装 commit 编排已接入该 repository：manifest 读取成功后写入 `planned`，进入真实写入窗口前写入 `committing`，manifest 保存成功后 best-effort 写入 `completed`；若写入窗口后失败且 best-effort rollback 失败，才留下 `rollback_required`。rollback 成功的失败路径会 best-effort 清理本次 recovery record，避免制造假的待恢复状态。
- 替换已有托管目标时，manifest entry 仍继承旧条目的长期 `backup_ref` 语义；但如果写入窗口后失败且 rollback 失败，留下的 `rollback_required` recovery record 会保留本次提交前创建的 pending backup ref，用于后续受控回滚恢复到“安装前一刻”的文件状态。若 `committing` 已保存后才更新某个 entry 的 pending backup，active recovery record 会立即重新持久化，避免崩溃恢复读取到旧 backup 语义。manifest 保存成功后，`completed` recovery record 会重新同步为 manifest entry 的长期 `backup_ref`，避免 completed 状态指向随后会被 best-effort 清理的 pending backup。
- `scan_install_recovery` 已只读消费 durable recovery record：`committing` 或 `rollback_required` record 会对外返回 `rollback_required`，`planned`、`completed` 和 `rolled_back` 不会被提升为待回滚状态；空 `modIds` 全量扫描也会包含只有 recovery record、尚无 manifest 的半完成安装。
- `preview_recovery_action` 已提供只读恢复动作预览：当前支持 `rollback_install`，只接收 `gameId`、`profileId`、`modId` 和 `actionKind`，复用同一 `gameId/profileId` 写锁读取 durable recovery record、当前目标摘要和 backup 可读性，并只返回 `available` / `blocked`、删除/恢复/backup 聚合计数和稳定阻断 reason code。该能力不执行删除、恢复、回滚、写 manifest、写 recovery record、发送 task phase 或写 Audit Log。
- `start_recovery_action_task` 已提供后端受控回滚任务入口：当前支持 `rollback_install`，只接收 `gameId`、`profileId`、`modId` 和 `actionKind`，后台 runner 复用同一 `gameId/profileId` 写锁重新验证 durable recovery record、目标摘要和 backup 可读性，随后删除新增文件或从 backup 恢复覆盖文件，并将 recovery record 标记为 `rolled_back`。该能力已写最小 Audit Log；恢复中心写入型按钮尚未启用。

### Tauri command 与任务入口

位置：

- `src-tauri/src/install_commands.rs`
- `src-tauri/src/dto.rs`
- `src-tauri/src/state.rs`
- `src-tauri/crates/hmm-app/src/install_recovery.rs`
- `src-tauri/crates/hmm-app/src/install_task.rs`

已包含 command：

- `preview_install_plan`
- `preview_imported_mod_install_plan`
- `start_install_task`
- `start_uninstall_task`
- `get_install_manifest_status`
- `scan_install_recovery`
- `preview_recovery_action`
- `start_recovery_action_task`

`start_install_task` 只接收：

- `gameId`
- `modId`
- `profileId`
- `layerName`
- `layerPriority`

`start_uninstall_task` 只接收：

- `gameId`
- `modId`
- `profileId`

`scan_install_recovery` 只接收：

- `gameId`
- `profileId`
- `modIds`

`preview_recovery_action` 只接收：

- `gameId`
- `profileId`
- `modId`
- `actionKind`

`start_recovery_action_task` 只接收：

- `gameId`
- `profileId`
- `modId`
- `actionKind`

安装、卸载和恢复动作任务已接入 `TaskKind::Install`。安装 commit 阶段、卸载删除/恢复阶段、受控回滚执行阶段均按 `gameId/profileId` 写锁串行。plan build、sandbox 文件扫描和只读分析不持有写锁。

当前安装任务阶段：

- `install.queued`
- `install.plan.building`
- `install.commit.processing`
- `install.completed`
- `install.failed`
- `install.cancelled`
- `install.uninstall.queued`
- `install.uninstall.processing`
- `install.uninstall.completed`
- `install.uninstall.failed`
- `install.recovery.queued`
- `install.recovery.planning`
- `install.recovery.processing`
- `install.recovery.completed`
- `install.recovery.failed`

当前最小卸载能力基于 manifest entries、`installed_file` 摘要和 backup ref。自动卸载只处理指定 `modId` 的 manifest entries；缺少 `installed_file`、当前目标文件 size/SHA-256 与 manifest 不匹配、目标文件缺失或 backup 缺失时会阻断，不根据当前 Mod 包内容猜测。

当前只读恢复扫描能力基于 durable recovery record、manifest entries、`installed_file` 摘要、当前目标文件摘要和 backup 是否存在。`scan_install_recovery` 会按 `modId` 返回 `completed`、`rollback_required`、`repair_required`、`unknown` 或 `not_installed` 摘要，以及不含路径或 backup ref 的聚合 issue code；`rollback_required` 只能来自 durable recovery record 的 `committing` / `rollback_required` 受控状态，不能由目录内容猜测。当 `modIds` 为空时，后端会扫描该 profile manifest 内全部已知托管 Mod，并补入只有 recovery record、尚无 manifest 的半完成安装，作为 Dashboard 入口级恢复健康摘要、App Frame 全局告警和独立恢复中心入口的基础。扫描会复用安装/卸载同一份 `gameId/profileId` 写锁，避免在 commit / uninstall 写入窗口内读取半完成状态。它只做检测，不自动删除、恢复、回滚或写 manifest。

当前恢复动作预览与执行能力基于 durable recovery record、当前目标文件摘要和 backup 可读性。`preview_recovery_action` 当前仅支持只读预览 `rollback_install`，会在同一 `gameId/profileId` 写锁下重新验证候选 entry；无 recovery record、状态不在 `committing` / `rollback_required`、缺少 `installed_file`、目标缺失、目标摘要变化、目标读取失败、backup 缺失或 backup 读取失败都会返回 `blocked`，并仅暴露稳定 reason code 与聚合计数。`start_recovery_action_task` 当前仅支持执行 `rollback_install`，会在持锁区重新验证上述条件后删除新增文件或从 backup 恢复覆盖文件，并将 durable recovery record 标记为 `rolled_back`。它不写 rich manifest，不暴露 target path、backup ref/root、manifest root/path、sandbox/cache 路径或第三方 Mod 内容；恢复中心写入型按钮仍需后续切片启用。

任务事件和 Audit Log 不应携带完整本地路径、用户名、Steam ID、sandbox/cache 路径、真实 Mod 包内容或 manifest 正文。

### 前端最小接入

位置：

- `src/features/mods/modInstallPlanApi.ts`
- `src/features/mods/modInstallPlanTypes.ts`
- `src/features/mods/modLibraryLoadState.ts`
- `src/features/mods/modLibraryTypes.ts`
- `src/features/mods/InstallPlanPreviewPanel.tsx`
- `src/features/mods/ModLibraryPage.tsx`
- `src/features/mods/CompactActionPanel.tsx`
- `src/features/mods/ModPosterCard.tsx`

已包含：

- `previewInstallPlanForImportedMod`
- `getInstallManifestStatus`
- `scanInstallRecovery`
- `startInstallTask`
- `startUninstallTask`
- `previewRecoveryAction`
- `startRecoveryActionTask`
- 最小安装计划预览面板。
- 从 Mod 库触发最小安装任务。
- 按 `taskId` 订阅 `hmm://task-progress` 安装事件。
- 展示 `install.queued`、`install.plan.building`、`install.commit.processing`、`install.completed`、`install.failed` 和 `install.cancelled`。
- 处理 `start_install_task` 返回前进度事件先到达的竞态。
- 通过 `get_install_manifest_status` 在 Mod 库加载成功和安装任务完成后刷新 manifest 状态摘要。
- 展示 `not_installed`、`installed`、`repair_required`、`unknown` 等后端摘要状态；当前 MVP 会根据匹配 entries 派生 `installed`，缺失 manifest 或无匹配 entry 显示 `not_installed`。`installed_file` 摘要已写入新 manifest，但 manifest 查询尚未执行目标文件 hash/backup 完整性校验。
- 在 manifest 摘要刷新后调用只读 `scan_install_recovery`，把 `completed` 映射为前端 `installed`，把 `rollback_required` / `repair_required` / `unknown` 作为不安全安装状态展示。
- 对 `rollback_required` / `repair_required` / `unknown` 只展示托管文件数、backup 计数、聚合 issue code 和计数，不展示 target path、game root、backup ref/root、manifest root/path、sandbox/cache 路径、manifest 正文或第三方 Mod 内容。
- 当恢复扫描返回 `rollback_required` / `repair_required` / `unknown` 时，Mod 库会阻断安装/重装入口和自动卸载入口，并展示人工处理提示。
- Dashboard 入口在游戏目录已配置后调用只读 `scan_install_recovery`，使用空 `modIds` 扫描当前 profile 的全部托管 Mod，并在右侧状态栏展示 profile 级健康摘要。该摘要只展示扫描 Mod 数、需处理数、未知数、问题计数和聚合 issue 分类，不提供恢复、删除、回滚或 manifest 写入动作。
- App Frame 全局告警在游戏目录已配置后复用同一只读 profile 级恢复扫描聚合；只有 `rollback_required` / `repair_required` / `unknown` 聚合为需要关注，或扫描不可用时显示轻量告警并提供恢复中心导航。告警不展示 target path、game root、backup ref/root、manifest root/path、sandbox/cache、目标文件 hash、manifest 正文或第三方 Mod 内容，也不触发自动恢复、删除、回滚或 manifest 写入。
- 独立恢复中心入口在游戏目录已配置后调用只读 `scan_install_recovery`，使用空 `modIds` 扫描当前 profile 的全部托管 Mod，并展示 profile 级聚合摘要、只读 rich repair summary、只读人工处理决策面板、每个托管 Mod 的状态、托管文件计数、backup 计数、issue 计数、稳定 issue 分类和人工处理提示。人工处理决策面板只提供重新扫描、导出诊断等安全动作，并把后续受控修复标记为不可用；该页面不提供自动恢复、删除、回滚或 manifest 写入动作，也不展示 target path、game root、backup ref/root、manifest root/path、sandbox/cache 路径、目标文件 hash、manifest 正文或第三方 Mod 内容。
- 恢复中心提供用户主动触发的完整支持诊断包导出入口，复用已有 `export_support_diagnostics` 后端 command。前端导出前先展示已脱敏类别确认，导出后只展示 `exportId`、`fileName`、`sizeBytes`、App/Task 日志行数和 Audit event 计数，不接受输出路径、日志路径或类别参数，也不展示诊断包完整路径、日志正文、审计事件正文、manifest/backup/root、sandbox/cache 路径或第三方 Mod 内容。
- 只在后端 manifest 摘要显示 `installed` 时启用单选卸载入口。
- 从 Mod 库触发最小卸载确认流程，并通过 `start_uninstall_task` 启动后端任务。
- 展示 `install.uninstall.queued`、`install.uninstall.processing`、`install.uninstall.completed` 和 `install.uninstall.failed`。
- 卸载完成后复用 manifest 状态摘要查询刷新安装事实。
- `previewRecoveryAction` feature-local typed API 已接入，只提交 `gameId`、`profileId`、`modId` 和 `actionKind`，并只接收 action kind、`available` / `blocked`、删除/恢复/backup 聚合计数和稳定阻断 reason code。
- `startRecoveryActionTask` feature-local typed API 已接入，只提交 `gameId`、`profileId`、`modId` 和 `actionKind`，并返回标准 `TaskStartedDto`；当前尚未接恢复中心按钮或写入型恢复 UI。

前端只能展示后端返回的计划摘要、冲突摘要、任务事件状态、manifest 查询摘要、只读恢复扫描摘要和只读恢复动作预览摘要，不应推断 MHW 路径规则或自行拼接安装/卸载/恢复路径。安装/卸载/恢复扫描/动作预览/恢复动作任务 UI 只提交 `gameId`、`modId`、`profileId`、`modIds` 和 `actionKind` 等短 id，不提交 target path、game root、backup ref/root、manifest root/path、sandbox/cache 路径或 Mod 包路径。任务状态仍是页面内存态；页面刷新、重新进入后的安装事实应通过 manifest 查询和只读 recovery scan 恢复，而不是依赖内存任务状态或 mock 数据。

## 尚未落地能力

以下能力仍不能视为已完成：

- 卸载后续工作流：后端最小 manifest 驱动卸载任务入口、前端最小单选卸载 UI 和不安全恢复状态阻断已落地，但尚未实现批量/profile 切换或卸载专用 rich repair summary。
- 恢复中心写入型工作流：只读 `scan_install_recovery` 摘要已能检测 `completed`、`rollback_required`、`repair_required`、`unknown` 和 `not_installed`，也支持空 `modIds` 扫描当前 profile manifest 内全部已知托管 Mod，并会补入只有 durable recovery record 的半完成安装；Mod 库加载后已会消费该摘要并展示人工处理提示，Dashboard 入口已展示 profile 级健康摘要，App Frame 已提供全局只读告警，独立恢复中心已提供只读入口、逐 Mod 安全摘要、只读 rich repair summary、完整支持诊断包导出联动和只读人工处理决策面板。`preview_recovery_action` 已能只读预览 `rollback_install` 是否可执行，`start_recovery_action_task` 已能后端执行受控 `rollback_install`；但恢复中心写入型按钮、任务 UI 编排和操作完成后的恢复中心刷新仍未实现。
- Profile 工作流：`profileId` 已进入链路，但 profile 启用/禁用、批量切换、优先级管理仍未完成。
- 依赖和前置检查：尚未在安装提交前接入完整 dependency/preflight 阻断。
- ARMOR_RETARGET staging：设计上依赖 InstallPlan，但当前尚未把 retarget materialize 产物接入 InstallPlan 输入。
- Manifest rich 状态检测：当前已提供只读 manifest 状态摘要 command、只读 recovery scan command 和前端 manifest 摘要展示，新 manifest entry 已记录写入内容的 size/SHA-256；`scan_install_recovery` 已能读取 durable recovery record、真实目标文件和 backup 做只读一致性检测，并可返回由受控记录驱动的 `rollback_required`。`get_install_manifest_status` 尚未自动消费 recovery scan 结果。旧 manifest 可能缺少 `installed_file` 摘要，后续破坏性操作必须阻断或进入修复流程。
- Rich manifest：当前 manifest 仍是 MVP 形态，尚未包含 backend、status、replacement binding snapshot、created/completed time、plan hash 等长期字段。
- Crash recovery：当前提交失败会 best-effort rollback，但不等同于跨进程崩溃恢复能力。

## 文档现状与分工

- [架构设计](ARCHITECTURE.md)：记录安装必须经过计划、manifest、备份和回滚的原则。
- [Mod 安装方案规划](mod_installation_strategy.md)：记录长期方案和可选后端，不代表当前全部已实现。
- [前后端通信契约](FRONTEND_BACKEND_CONTRACT.md)：记录当前 Tauri command、DTO、错误码和任务事件契约。
- [InstallPlan MVP 待办](INSTALL_PLAN_MVP_TODO.md)：记录后续切片、验收标准、安全门禁，以及 manifest 状态、卸载/恢复、安装 UI、retarget staging 和测试矩阵的细化规则。
- [安装恢复受控动作实施计划](INSTALL_RECOVERY_CONTROLLED_ACTIONS_PLAN.md)：记录 `rollback_required`、只读动作预览、受控回滚任务和恢复中心写入动作启用前的安全拆分。
- 本文档：记录当前实现状态和后续切片判断。

## 后续建议切片

建议继续按下面顺序推进：

1. Crash/recovery 后续：按 [安装恢复受控动作实施计划](INSTALL_RECOVERY_CONTROLLED_ACTIONS_PLAN.md) 已补 durable recovery record 的领域模型、port、JSON 仓储、安装 commit 写入、只读扫描消费、只读动作预览和后端受控回滚任务；下一步应补恢复中心写入型 UI 启用和操作完成后的重新扫描编排。已落地的 App Frame 全局告警、恢复中心人工处理面板和动作预览仍只是只读提示/决策面，不绕过 manifest、backup、Audit Log 和恢复扫描事实。
2. Rich manifest / repair 检测：补齐 backend、status、replacement binding snapshot、plan hash 和时间字段，支持 `rollback_required` 和更完整的 `repair_required` 状态机。
3. 卸载后续 UI：补充批量/profile 工作流和更明确的人工修复入口。
4. ARMOR_RETARGET staging 接入：让 retarget 产物作为受控 provider 输入 InstallPlan。
5. 依赖/preflight：在提交前阻断缺失必需前置和高风险安装状态。

## 验证基线

涉及 InstallPlan、manifest、backup、rollback、Tauri command 或任务事件时，至少参考 [测试指南](TESTING.md) 中“安装、卸载与回滚”“Tauri / Rust 桥接改动”“并发与任务系统”“日志与审计”章节。

提交前优先执行：

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify.ps1
```

如只修改文档，至少应检查 Markdown 内链、空白和文档职责是否重复。
