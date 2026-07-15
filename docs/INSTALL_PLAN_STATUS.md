# InstallPlan 模块现状

本文档记录当前 `InstallPlan` 模块已经落地的能力、尚未落地的边界和后续切片顺序。它用于回答“现在能依赖什么”，长期设计仍参考 [Mod 安装方案规划](mod_installation_strategy.md)，跨前后端通信契约参考 [前后端通信契约](FRONTEND_BACKEND_CONTRACT.md)。

当前实施顺序由 [核心 Mod 生命周期优先级计划](CORE_MOD_LIFECYCLE_PRIORITY_PLAN.md) 覆盖：安装、
卸载和真正重装现均为 `implemented`，下一项是 CL4 独立复审，使 Core Mod Lifecycle Gate A 达到
`certified`；通过后立即进入 ARMOR_RETARGET 最窄纵向切片。本文的能力清单仍是实现事实来源，
但旧的后续建议不再优先于该计划。

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
- `src-tauri/crates/hmm-app/src/reinstall.rs`

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
- `src-tauri/crates/hmm-app/src/reinstall_commit.rs`

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

普通 install 的 manifest 合并规则仍保持 MVP 范围：提交服务只按本次实际写入的目标路径替换旧条目，并保留未触达的旧条目。替换已有托管目标时，新的 manifest entry 会继承旧条目的长期 `backup_ref` 语义；本次提交为中间状态创建的 pending backup 只用于失败回滚，提交成功后会 best-effort 清理。它不会因为 `modId` 相同就删除旧条目；版本删减由独立真正重装的 entry-set replacement 处理，卸载和恢复扫描则继续以 manifest/recovery 为事实来源。

真正重装不复用上述普通 install merge。独立 `ReinstallPreviewService` / `ReinstallCommitService` 从
installed manifest、candidate revision plan、当前目标摘要和 original backup 构建 retained/replaced/
added/stale 四类计划，并在共享写锁内原子替换指定 Mod 的 entry set。提交前完整预读 source 与
snapshot，失败时恢复 pre-reinstall revision；跨进程未收敛状态由 durable reinstall transaction 驱动
reconciliation，不能由当前 package 或 task 内存猜测。

新写入的 manifest entry 会记录 `installed_file` 摘要：写入内容的字节数和 SHA-256。该字段只描述本工具本次写入到目标路径的内容，不记录完整本地路径、sandbox/cache 路径或文件内容。旧 manifest 缺少该字段时仍可兼容读取，但后续自动卸载或修复检测不能把缺少摘要的旧 entry 当作可安全删除/恢复的充分事实。

manifest 已具备最小 rich metadata 兼容基础：`manifest_id`、`schema_version`、`schema_migration`、`backend`、`status`、`created_at`、`completed_at` 和 `plan_hash` 字段可被 JSON 读写；旧 manifest 缺少这些字段时会兼容读取，并把 `manifest_id` 默认为 `profile:<profile_id>`、`schema_version` 默认为 `1`、`status` 默认为 `completed`。profile 级 `status` 已有统一读侧消费规则（`InstallManifestStatus::consumption()`）：manifest 状态摘要查询 fallback 和只读恢复扫描先消费 manifest status，`rollback_required` / `repair_required` 映射为对应失败摘要、`planned` / `committing` 映射为 `unknown`，`completed` / `rolled_back` 才继续按 entries / 文件校验消费，保证失败状态不会被误报为已完成。当前安装提交成功会写入稳定 profile-scoped `manifest_id`、`schema_version = 1`、`backend = "install_plan"`、`status = completed`、`completed_at` 和真实 `plan_hash`；commit merge / uninstall 会保留已有 schema metadata。`plan_hash` 使用稳定 `sha256:` 摘要绑定本次 commit 消费的计划事实，只包含相对 target、mod id、package file id 和 layer 信息，不包含完整本地路径、backup root/ref、manifest path、sandbox/cache path 或第三方 Mod 内容。

当前回滚能力：

- 写入新文件后失败：删除已写入的新文件。
- 覆盖旧文件后失败：恢复旧文件内容。
- manifest 保存失败：回滚已写入文件。
- 写入失败且已生成备份：清理 pending backup。

### ports 与 infra

接口位置：

- `src-tauri/crates/hmm-ports/src/install.rs`
- `src-tauri/crates/hmm-ports/src/reinstall.rs`

已包含：

- `InstallSourceFileReader`
- `InstallGameFileSystem`
- `InstallBackupStore`
- `InstallManifestRepository`

文件系统实现位置：

- `src-tauri/crates/hmm-infra/src/install_commit.rs`
- `src-tauri/crates/hmm-infra/src/reinstall.rs`

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
- 安装 commit 编排已接入该 repository：manifest 读取成功后写入 `planned`，进入真实写入窗口前写入 `committing`；manifest 保存成功后先 best-effort 写入 `completed` 关闭崩溃窗口，再 best-effort 清理本次 active recovery record。若写入窗口后失败且 best-effort rollback 失败，才留下 `rollback_required`；rollback 成功的失败路径也会 best-effort 清理，避免制造假的待恢复状态。
- 替换已有托管目标时，manifest entry 仍继承旧条目的长期 `backup_ref` 语义；但如果写入窗口后失败且 rollback 失败，留下的 `rollback_required` recovery record 会保留本次提交前创建的 pending backup ref，用于后续受控回滚恢复到“安装前一刻”的文件状态。若 `committing` 已保存后才更新某个 entry 的 pending backup，active recovery record 会立即重新持久化，避免崩溃恢复读取到旧 backup 语义。manifest 保存成功后，record 会先以 manifest entry 的长期 `backup_ref` 和 `installed_file` 摘要写入 `completed`，再 best-effort 删除；若删除失败，残留 `completed` 仍按兼容无害状态消费。
- `scan_install_recovery` 已只读消费 durable recovery record：`committing` 或 `rollback_required` record 会对外返回 `rollback_required`，`planned`、`completed` 和 `rolled_back` 不会被提升为待回滚状态；空 `modIds` 全量扫描也会包含只有 recovery record、尚无 manifest 的半完成安装。
- `preview_recovery_action` 已提供只读恢复动作预览：`rollback_install` 会读取 durable recovery record、当前目标摘要和 backup 可读性并返回 `available` / `blocked`、聚合计数和稳定 reason；`reconcile_reinstall` 在没有专用 preview 证明时保持 blocked。该能力不执行删除、恢复、回滚、写 manifest、写 recovery record、发送 task phase 或写 Audit Log。
- `start_recovery_action_task` 已提供后端受控恢复任务入口：`rollback_install` 重新验证普通安装 recovery/target/backup 后删除新增文件或恢复覆盖文件；`reconcile_reinstall` 消费 durable reinstall transaction，受控收敛 post-commit cleanup 或恢复 pre-reinstall revision，无法证明时进入 `repair_required`。两类动作复用同一 `gameId/profileId` 写锁并写最小 Audit Log。
- revision catalog 已保存稳定 logical Mod、不可变 package revisions 与 origin/display revision；installed revision 仍由 completed manifest entry set 决定。独立 `ReinstallRecoveryTransactionRepository` 保存 pre-reinstall entry set、四类目标事实、snapshot ownership 和受控状态，JSON/FS adapter 继续使用受控 root、原子保存和 containment 校验。

### Tauri command 与任务入口

位置：

- `src-tauri/src/install_commands.rs`
- `src-tauri/src/dto.rs`
- `src-tauri/src/reinstall_commands.rs`
- `src-tauri/src/reinstall_dto.rs`
- `src-tauri/src/state.rs`
- `src-tauri/crates/hmm-app/src/install_recovery.rs`
- `src-tauri/crates/hmm-app/src/install_task.rs`
- `src-tauri/crates/hmm-app/src/reinstall_task.rs`

已包含 command：

- `preview_install_plan`
- `preview_imported_mod_install_plan`
- `start_install_task`
- `start_uninstall_task`
- `get_install_manifest_status`
- `scan_install_recovery`
- `preview_recovery_action`
- `start_recovery_action_task`
- `start_import_mod_revision_task`
- `get_mod_revisions`
- `preview_reinstall_plan`
- `start_reinstall_task`

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

安装、卸载、真正重装和恢复动作任务已接入 `TaskKind::Install`。安装 commit 阶段、卸载删除/恢复
阶段、真正重装 commit/rollback 阶段和受控恢复执行阶段均按 `gameId/profileId` 写锁串行。plan build、
sandbox 文件扫描和只读分析不持有写锁。

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
- `install.reinstall.queued`
- `install.reinstall.plan.building`
- `install.reinstall.preflight.processing`
- `install.reinstall.commit.processing`
- `install.reinstall.rollback.processing`
- `install.reinstall.completed`
- `install.reinstall.failed`
- `install.reinstall.cancelled`
- `install.recovery.queued`
- `install.recovery.planning`
- `install.recovery.processing`
- `install.recovery.completed`
- `install.recovery.failed`

当前最小卸载能力基于 manifest entries、`installed_file` 摘要和 backup ref。自动卸载只处理指定 `modId` 的 manifest entries；缺少 `installed_file`、当前目标文件 size/SHA-256 与 manifest 不匹配、目标文件缺失或 backup 缺失时会阻断，不根据当前 Mod 包内容猜测。

当前只读恢复扫描能力基于 durable recovery record、manifest entries、`installed_file` 摘要、当前目标文件摘要和 backup 是否存在。`scan_install_recovery` 会按 `modId` 返回 `completed`、`rollback_required`、`repair_required`、`unknown` 或 `not_installed` 摘要，以及不含路径或 backup ref 的聚合 issue code；`rollback_required` 只能来自 durable recovery record 的 `committing` / `rollback_required` 受控状态，不能由目录内容猜测。当 `modIds` 为空时，后端会扫描该 profile manifest 内全部已知托管 Mod，并补入只有 recovery record、尚无 manifest 的半完成安装，作为 Dashboard 入口级恢复健康摘要、App Frame 全局告警和独立恢复中心入口的基础。扫描会复用安装/卸载同一份 `gameId/profileId` 写锁，避免在 commit / uninstall 写入窗口内读取半完成状态。它只做检测，不自动删除、恢复、回滚或写 manifest。

当前恢复动作预览与执行能力基于 durable recovery record / reinstall transaction、当前目标摘要和
backup/snapshot 可读性。`rollback_install` 仍要求预览 available 后才允许执行；`reconcile_reinstall`
没有专用 preview 时保持 blocked，但后端任务可在重装恢复状态驱动下收敛 completed cleanup 或
pre-reinstall rollback，无法证明时进入 `repair_required`。两类动作都在持锁区重新验证，不暴露
target path、backup/snapshot ref、manifest root/path、sandbox/cache 路径或第三方 Mod 内容。

任务事件和 Audit Log 不应携带完整本地路径、用户名、Steam ID、sandbox/cache 路径、真实 Mod 包内容或 manifest 正文。

### 前端最小接入

位置：

- `src/features/mods/modInstallPlanApi.ts`
- `src/features/mods/modInstallPlanTypes.ts`
- `src/features/mods/modImportApi.ts`
- `src/features/mods/modLibraryApi.ts`
- `src/features/mods/modReinstallApi.ts`
- `src/features/mods/modReinstallTypes.ts`
- `src/features/mods/useModReinstallWorkflow.ts`
- `src/features/mods/modLibraryLoadState.ts`
- `src/features/mods/modLibraryTypes.ts`
- `src/features/mods/InstallPlanPreviewPanel.tsx`
- `src/features/mods/ReinstallPlanPreviewPanel.tsx`
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
- `startImportModRevisionTask`
- `getModRevisions`
- `previewReinstallPlan`
- `startReinstallTask`
- 最小安装计划预览面板。
- 从 Mod 库触发最小安装任务。
- 按 `taskId` 订阅 `hmm://task-progress` 安装事件。
- 展示 `install.queued`、`install.plan.building`、`install.commit.processing`、`install.completed`、`install.failed` 和 `install.cancelled`。
- 处理 `start_install_task` 返回前进度事件先到达的竞态。
- 通过 `get_install_manifest_status` 在 Mod 库加载成功和安装任务完成后刷新安装状态摘要。Mod 库会传入 `gameId`，因此该摘要会复用只读 recovery scan，把 `completed` 映射为前端 `installed`，并把 `rollback_required` / `repair_required` / `unknown` 作为不安全安装状态展示；未传 `gameId` 的调用仍保留 manifest-only fallback。
- 展示 `not_installed`、`installed`、`rollback_required`、`repair_required`、`unknown` 等后端摘要状态。`installed_file` 摘要已写入新 manifest；带 `gameId` 的状态摘要会读取目标文件和 backup 做只读一致性检测，不带 `gameId` 的 manifest-only 路径仍只根据匹配 entries 派生 `installed` / `not_installed`。
- 在状态摘要刷新后继续调用只读 `scan_install_recovery` 获取 issue code、计数和恢复中心所需聚合详情。
- 对 `rollback_required` / `repair_required` / `unknown` 只展示托管文件数、backup 计数、聚合 issue code 和计数，不展示 target path、game root、backup ref/root、manifest root/path、sandbox/cache 路径、manifest 正文或第三方 Mod 内容。
- 当恢复扫描返回 `rollback_required` / `repair_required` / `unknown` 时，Mod 库会阻断安装/重装入口和自动卸载入口，并展示人工处理提示。
- Dashboard 入口在游戏目录已配置后调用只读 `scan_install_recovery`，使用空 `modIds` 扫描当前 profile 的全部托管 Mod，并在右侧状态栏展示 profile 级健康摘要。该摘要只展示扫描 Mod 数、需处理数、未知数、问题计数和聚合 issue 分类，不提供恢复、删除、回滚或 manifest 写入动作。
- App Frame 全局告警在游戏目录已配置后复用同一只读 profile 级恢复扫描聚合；只有 `rollback_required` / `repair_required` / `unknown` 聚合为需要关注，或扫描不可用时显示轻量告警并提供恢复中心导航。告警不展示 target path、game root、backup ref/root、manifest root/path、sandbox/cache、目标文件 hash、manifest 正文或第三方 Mod 内容，也不触发自动恢复、删除、回滚或 manifest 写入。
- 独立恢复中心入口在游戏目录已配置后调用只读 `scan_install_recovery`，使用空 `modIds` 扫描当前 profile 的全部托管 Mod，并展示 profile 级聚合摘要、rich repair summary、人工处理决策面板、每个托管 Mod 的状态、托管文件计数、backup 计数、issue 计数、稳定 issue 分类和人工处理提示。人工处理决策面板提供重新扫描、导出诊断，并在存在 `rollback_required` Mod 时引导到逐 Mod 受控回滚入口；真正写入型按钮只在单个 `rollback_required` Mod 行上出现，且必须先调用 `preview_recovery_action`，后端返回 `available` 后才允许确认并调用 `start_recovery_action_task`。该页面不展示 target path、game root、backup ref/root、manifest root/path、sandbox/cache 路径、目标文件 hash、manifest 正文或第三方 Mod 内容。
- 恢复中心提供用户主动触发的完整支持诊断包导出入口，复用已有 `export_support_diagnostics` 后端 command。前端导出前先展示已脱敏类别确认，导出后只展示 `exportId`、`fileName`、`sizeBytes`、App/Task 日志行数和 Audit event 计数，不接受输出路径、日志路径或类别参数，也不展示诊断包完整路径、日志正文、审计事件正文、manifest/backup/root、sandbox/cache 路径或第三方 Mod 内容。
- 只在后端 manifest 摘要显示 `installed` 时启用单选卸载入口。
- 从 Mod 库触发最小卸载确认流程，并通过 `start_uninstall_task` 启动后端任务。
- 展示 `install.uninstall.queued`、`install.uninstall.processing`、`install.uninstall.completed` 和 `install.uninstall.failed`。
- 卸载完成后复用 manifest 状态摘要查询刷新安装事实。
- `previewRecoveryAction` feature-local typed API 已接入，只提交 `gameId`、`profileId`、`modId` 和 `actionKind`，并只接收 action kind、`available` / `blocked`、删除/恢复/backup 聚合计数和稳定阻断 reason code。
- `startRecoveryActionTask` feature-local typed API 已接入，只提交 `gameId`、`profileId`、`modId` 和 `actionKind`，并返回标准 `TaskStartedDto`；恢复中心逐 Mod 按钮和任务 UI 编排已接入。
- Mod 库支持在一张 logical Mod 卡上导入和选择 candidate revision，真正重装使用独立 strict preview/confirm 状态，展示 retained/replaced/added/stale 聚合并按 `taskId` 跟踪 `install.reinstall.*`；installed revision 和动作可用性来自 manifest/recovery 查询，而不是 display revision 或页面 task 内存。

前端只能展示后端返回的计划/冲突/revision 聚合、任务状态、manifest/recovery 摘要和恢复动作预览，
不应推断 MHW 路径规则或自行拼接安装/卸载/重装/恢复路径。相关 UI 只提交 `gameId`、`modId`、
`profileId`、`modIds`、`candidateRevisionId`、opaque `planToken`、layer 和 `actionKind` 等受控字段；
只有系统 picker 驱动的 revision import command 可提交 `archivePath`，其他重装/安装/卸载/恢复入口
均不提交 target path、game root、backup/snapshot ref、manifest root/path、sandbox/cache 或 Mod 包路径。
任务状态仍是页面内存态；重启后的安装事实必须通过 manifest/recovery 查询恢复。

## 尚未落地能力

以下能力仍不能视为已完成：

- 核心生命周期认证：CL0-CL3 已有独立、可重复的 L1/L2 自动化和 disposable Windows Sandbox L3；
  安装、卸载与真正重装均为 `implemented`，但 CL4 独立复审和 Gate A `certified` 记录仍未完成。
- 卸载后续工作流：后端最小 manifest 驱动卸载任务入口、前端最小单选卸载 UI 和不安全恢复状态阻断已落地，但尚未实现批量/profile 切换或卸载专用 rich repair summary。
- 恢复中心写入型工作流：只读 `scan_install_recovery` 摘要已能检测 `completed`、`rollback_required`、`repair_required`、`unknown` 和 `not_installed`，也支持空 `modIds` 扫描当前 profile manifest 内全部已知托管 Mod，并会补入只有 durable recovery record 的半完成安装；Mod 库加载后已会消费该摘要并展示人工处理提示，Dashboard 入口已展示 profile 级健康摘要，App Frame 已提供全局告警，独立恢复中心已提供入口、逐 Mod 安全摘要、rich repair summary、完整支持诊断包导出联动和人工处理决策面板。`preview_recovery_action` 已能只读预览 `rollback_install` 是否可执行，`start_recovery_action_task` 已能后端执行受控 `rollback_install`；恢复中心写入型按钮、任务 UI 编排和操作完成后的恢复中心/全局健康刷新均已实现。
- Profile 工作流：`profileId` 已进入链路，但 profile 启用/禁用、批量切换、优先级管理仍未完成。
- 依赖和前置检查：尚未在安装提交前接入完整 dependency/preflight 阻断。
- ARMOR_RETARGET staging：设计上依赖 InstallPlan，但当前尚未把 retarget materialize 产物接入 InstallPlan 输入。
- Manifest rich 状态检测：当前已提供只读 manifest 状态摘要 command、只读 recovery scan command 和前端 manifest 摘要展示，新 manifest entry 已记录写入内容的 size/SHA-256；manifest JSON 已兼容 `manifest_id`、`schema_version`、`schema_migration`、`backend`、`status`、`created_at`、`completed_at` 和 `plan_hash` 字段，旧 manifest 缺少 rich 字段时默认读取为 profile-scoped manifest id、schema v1 和 `completed`。`scan_install_recovery` 已能读取 durable recovery record、真实目标文件和 backup 做只读一致性检测，并可返回由受控记录驱动的 `rollback_required`；`get_install_manifest_status` 在传入 `gameId` 时已消费同一只读恢复扫描结果并映射为安装摘要状态，未传 `gameId` 时保留 manifest-only fallback。两条路径都先消费 profile 级 rich manifest `status`（读侧状态机消费规则），manifest 失败/进行中状态优先于逐 entry 事实检查。旧 manifest 可能缺少 `installed_file` 摘要，后续破坏性操作必须阻断或进入修复流程。
- Rich manifest：当前已落地 domain 字段、JSON 兼容基础、`manifest_id` / schema metadata、真实 `plan_hash` 计算、受控回滚成功后的 `rolled_back` status 持久化，以及读侧状态机消费规则；replacement binding snapshot、写侧状态机门禁以及更完整的 `repair_required` 检测仍待后续切片。
- Crash recovery：当前提交失败会 best-effort rollback，但不等同于跨进程崩溃恢复能力。

## 文档现状与分工

- [架构设计](ARCHITECTURE.md)：记录安装必须经过计划、manifest、备份和回滚的原则。
- [Mod 安装方案规划](mod_installation_strategy.md)：记录长期方案和可选后端，不代表当前全部已实现。
- [前后端通信契约](FRONTEND_BACKEND_CONTRACT.md)：记录当前 Tauri command、DTO、错误码和任务事件契约。
- [InstallPlan MVP 待办](INSTALL_PLAN_MVP_TODO.md)：记录后续切片、验收标准、安全门禁，以及 manifest 状态、卸载/恢复、安装 UI、retarget staging 和测试矩阵的细化规则。
- [安装恢复受控动作实施计划](INSTALL_RECOVERY_CONTROLLED_ACTIONS_PLAN.md)：记录 `rollback_required`、只读动作预览、受控回滚任务和恢复中心写入动作启用前的安全拆分。
- [核心 Mod 生命周期优先级计划](CORE_MOD_LIFECYCLE_PRIORITY_PLAN.md)：记录当前 Gate A/Gate B、真正重装 contract、暂停清单和恢复门禁。
- [CL0 验收基线](CORE_MOD_LIFECYCLE_CL0_ACCEPTANCE.md)：记录人工 fixture、test-only AppState harness、证据矩阵、当前缺口和桌面 smoke 安全边界。
- 本文档：记录当前实现状态和后续切片判断。

## 后续建议切片

建议继续按下面顺序推进：

1. **CL0（已完成）：** 已固定 `v1/v2` 人工 fixture、test-only AppState import/plan/restart
   harness、acceptance matrix、桌面 smoke 文档和 composition 缺口清单。
2. **CL1（已完成）：** 已认证 import record -> InstallPlan -> install -> restart -> uninstall ->
   baseline 自动化闭环、准备阶段 fault ordering 与审计脱敏证据。
3. **CL2（已完成）：** 已在 disposable Windows Sandbox 执行 Tauri 桌面 smoke 与清理证明。
4. **CL3（已完成）：** 已落地独立真正重装 use case/task、四类 entry-set replacement、失败恢复和
   L1/L2/L3 验收。
5. **CL4 / Gate A（下一项）：** 完整验证、安全复审和 `certified` 状态记录。
6. **ARMOR_RETARGET Gate B：** 按最窄 `f_equip` 单 source 纵向切片接入 staging、InstallPlan、
   binding snapshot、选择目标、安装、切换目标和卸载。

Rich manifest、repair 和 preflight 只在解除上述步骤阻断时取最小切片。批量/profile 卸载、完整
repair 中心和通用依赖 catalog 延后；恢复中心写入型 UI 已落地，不再作为下一项工作重复实施。

## 验证基线

涉及 InstallPlan、manifest、backup、rollback、Tauri command 或任务事件时，至少参考 [测试指南](TESTING.md) 中“安装、卸载与回滚”“Tauri / Rust 桥接改动”“并发与任务系统”“日志与审计”章节。

提交前优先执行：

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify.ps1
```

如只修改文档，至少应检查 Markdown 内链、空白和文档职责是否重复。
