# InstallPlan MVP 待办

本文档维护 `InstallPlan` / Mod 安装 MVP 的后续切片、验收标准和安全门禁。它不是一次性 PR 计划，而是安装能力继续推进时的任务入口。

当前实现事实以 [InstallPlan 模块现状](INSTALL_PLAN_STATUS.md) 为准；长期方案和可选后端设计参考 [Mod 安装方案规划](mod_installation_strategy.md)；前后端通信形状参考 [前后端通信契约](FRONTEND_BACKEND_CONTRACT.md)。

## 目标

MVP 的目标不是一次性完成所有安装管理能力，而是先形成一条可测试、可审计、可回滚的最小安全链路：

```text
已导入 Mod
  -> 受控 sandbox
  -> 后端重建 InstallPlan
  -> 冲突和前置条件检查
  -> 用户确认
  -> 安装任务
  -> backup / commit / manifest
  -> 失败回滚或恢复提示
```

所有后续切片都必须保持这个边界：

- 前端只展示后端返回的状态和摘要，不拼接安装路径。
- Tauri command 只接收内部 id、用户选择和受控参数，不接收真实目录或最终目标路径。
- `hmm-core` 不感知 `nativePC`、MHW slot、retarget catalog 或真实文件系统。
- 真实游戏目录写入只能发生在提交服务或其后续受控执行器内。
- 卸载、修复和恢复只能基于 manifest、备份记录和受控审计信息，不根据当前 Mod 包重新猜测。

## 当前基线

已经落地：

- Mod 导入分析、预览图处理、导入结果持久化和 Mod 库查询。
- 前端 Mod 库消费 `get_mod_library()`；后端返回空数组时不再显示 mock 数据。
- `InstallPlan` 领域模型、目标路径校验、冲突模型和只读计划预览。
- 后端驱动的 `preview_imported_mod_install_plan`，从已导入 Mod 的受控 sandbox 和 game adapter 生成计划输入。
- 最小前端 typed API 和计划预览 UI。
- 安装提交服务、JSON manifest 仓储、备份和失败回滚骨架。
- JSON manifest 仓储可读取已有 profile manifest；安装提交会按目标路径合并 manifest 条目，保留未触达的旧条目，并在替换已有托管目标时保留旧 `backup_ref` 恢复语义。
- 新写入的 manifest entry 会记录 `installed_file` 摘要（写入内容 size + SHA-256），作为后续安全卸载、恢复扫描和真实 `repair_required` 检测的目标状态事实；旧 manifest 缺少该字段时兼容读取，但不能自动承诺可安全卸载。
- Rich manifest domain/JSON 兼容基础：`InstallManifest` 已支持 `manifest_id`、`schema_version`、`schema_migration`、`backend`、`status`、`created_at`、`completed_at` 和 `plan_hash` 字段；旧 manifest 缺少 rich 字段时兼容读取并默认 `manifest_id = profile:<profile_id>`、`schema_version = 1`、`status = completed`；安装提交成功会写入 schema metadata、`backend = "install_plan"`、`status = completed`、`completed_at` 和真实 `plan_hash`。
- Tauri `start_install_task`、`TaskKind::Install`、安装任务事件、game/profile 写锁和最小 Audit Log。
- 后端最小 manifest 驱动卸载：`UninstallModService` 只处理指定 Mod 的 manifest entries，要求 `installed_file` 摘要匹配，新增文件删除、覆盖文件从 backup 恢复，目标不一致、缺少摘要或 backup 缺失时阻断；`start_uninstall_task` 提供只接收短 id 的 Tauri 任务入口。
- 后端只读恢复扫描摘要：`scan_install_recovery` 只接收 `gameId`、`profileId`、`modIds`，基于 durable recovery record、受控 manifest、目标文件摘要和 backup 是否存在返回 `completed`、`rollback_required`、`repair_required`、`unknown` 或 `not_installed`，以及不含路径/backup ref 的聚合 issue code；当 `modIds` 为空时，后端扫描该 profile manifest 内全部已知托管 Mod，并补入只有 recovery record、尚无 manifest 的半完成安装。
- Manifest 状态摘要已接入恢复扫描事实：`get_install_manifest_status` 可选接收 `gameId`；传入 `gameId` 时复用只读 recovery scan 并把 `completed` 映射为 `installed`，把 `rollback_required` / `repair_required` / `unknown` 作为安装摘要状态返回；未传 `gameId` 时保留旧的 manifest-only fallback。
- Durable recovery record 基础：`hmm-core` 已提供 `InstallRecoveryRecord`、`InstallRecoveryRecordEntry`、`InstallRecoveryRecordStatus` 和受控状态迁移；`hmm-ports` 已提供窄 `InstallRecoveryRecordRepository`；`hmm-infra` 已提供受控 app data root 下的 JSON 仓储；安装 commit 已受控写入 `planned` / `committing` / `completed`，并且只在写入窗口后 rollback 失败时留下 `rollback_required`。当前恢复扫描已只读消费该记录；`rollback_required` 只来自 durable recovery record 的 `committing` / `rollback_required` 受控状态，不能由目录内容猜测。
- 受控回滚任务前置安全加固：当安装替换已有托管目标并在写入窗口后失败且 rollback 失败时，`committing` / `rollback_required` recovery record 会使用本次提交前创建的 pending backup ref 作为恢复来源；manifest 保存成功后的 `completed` record 则重新同步为 manifest entry 的长期 backup 语义。
- 后端受控回滚任务：`start_recovery_action_task` 只接收 `gameId`、`profileId`、`modId` 和 `actionKind`，复用同一 `gameId/profileId` 写锁执行 `rollback_install`，执行前重新验证目标摘要和 backup，可删除新增文件或从 backup 恢复覆盖文件，把 durable recovery record 标记为 `rolled_back`，并在已有 rich manifest 时移除该 Mod 的 stale entries、把 manifest status 持久化为 `rolled_back`。恢复中心已启用逐 Mod 写入型按钮，前端先预览、再确认、再按 `taskId` 跟踪任务。
- 前端最小安装任务工作流：从 Mod 库触发 `start_install_task`，按 `taskId` 订阅安装任务事件，展示 queued / planning / committing / completed / failed / cancelled，并处理进度事件早于 command 返回的竞态。
- 前端最小卸载 UI：只在后端 manifest 摘要为 `installed` 时启用单选卸载入口，确认后调用 `start_uninstall_task`，按 `taskId` 展示 `install.uninstall.*` 任务状态，并在完成后刷新 manifest 摘要。
- 前端 Mod 库恢复扫描入口：`get_install_manifest_status` 传入 `gameId` 后可直接返回 `rollback_required` / `repair_required` / `unknown` 等不安全状态；随后仍调用只读 `scan_install_recovery` 获取 issue code、计数和恢复中心所需聚合详情，并阻断安装/卸载入口。
- Dashboard 入口恢复健康摘要：游戏目录配置完成后调用只读 `scan_install_recovery`，用空 `modIds` 扫描当前 profile 全部托管 Mod，并在右侧状态栏展示只含聚合计数的健康摘要。
- App Frame 全局恢复告警：游戏目录配置完成后复用空 `modIds` 全量 profile 扫描聚合，只在需要处理、状态未知或扫描不可用时显示轻量告警，并提供恢复中心导航。
- 独立恢复中心入口：游戏目录配置完成后调用只读 `scan_install_recovery`，用空 `modIds` 扫描当前 profile 全部托管 Mod，并展示 profile 聚合摘要、rich repair summary、人工处理决策面板和每个托管 Mod 的安全状态摘要；对 `rollback_required` Mod 提供逐 Mod 受控回滚入口；恢复中心还提供用户主动触发的完整支持诊断包导出联动。

仍未完成：

- 卸载 rich repair summary、批量/profile 工作流和真正的受控修复入口。
- 恢复中心更丰富的 repair workflow；实施边界已细化到 [安装恢复受控动作实施计划](INSTALL_RECOVERY_CONTROLLED_ACTIONS_PLAN.md)，durable recovery record、安装 commit 写入、扫描消费、只读动作预览、后端受控回滚任务、恢复中心逐 Mod 写入型入口和任务 UI 编排均已落地。
- ARMOR_RETARGET staging 接入 InstallPlan。
- rich manifest 的 replacement binding snapshot、`game_id` / `game_instance_id` / 顶层 `mod_id` 语义、写侧状态机门禁和真实修复检测。
- dependency/preflight 阻断。

## 已完成切片记录

以下切片已经完成，后续工作不应重复开同类 PR，除非是在修 bug 或补边界：

- [x] `hmm-core` 最小 `InstallPlan`、目标路径校验和冲突模型。
- [x] `hmm-app` 只读安装计划预览服务。
- [x] `preview_install_plan` Tauri DTO/command 与契约文档。
- [x] 后端从已导入 Mod 的受控 sandbox 和 game adapter 生成安装计划输入。
- [x] 前端 feature-local typed API 与最小计划预览 UI。
- [x] 安装提交服务、JSON manifest 仓储、备份和失败回滚骨架。
- [x] 安装任务入口、写锁、审计日志和 `start_install_task`。
- [x] manifest 读取与按目标路径合并基础能力。
- [x] 前端最小安装任务流程与进度事件竞态处理。
- [x] Manifest 状态摘要查询 command、前端 typed API 和 Mod 库状态恢复展示。
- [x] Manifest entry 写入 `installed_file` size/SHA-256 摘要，并兼容读取缺少摘要的旧 manifest。
- [x] Rich manifest domain/JSON 兼容基础：`manifest_id`、`schema_version`、`schema_migration`、`backend`、`status`、`created_at`、`completed_at`、`plan_hash` 字段，旧 manifest 默认 `manifest_id = profile:<profile_id>`、`schema_version = 1`、`status = completed`，安装提交成功写入 schema metadata、`install_plan` 后端和完成时间。
- [x] 安装提交成功写入真实 `plan_hash`：使用稳定 `sha256:` 摘要绑定本次提交消费的计划事实，不记录完整本地路径、backup root/ref、manifest path、sandbox/cache path 或第三方 Mod 内容。
- [x] 后端最小 manifest 驱动卸载服务、backup 受控读取、卸载任务 runner 和 `start_uninstall_task` Tauri 入口。
- [x] 前端最小卸载 UI、`startUninstallTask` typed API、`install.uninstall.*` 任务展示和完成后 manifest 摘要刷新。
- [x] 后端只读恢复扫描摘要 command：`scan_install_recovery`。
- [x] 前端 Mod 库只读恢复扫描入口、聚合人工处理提示和不安全状态安装/卸载阻断。
- [x] `scan_install_recovery` 支持空 `modIds` 扫描当前 profile manifest 内全部已知托管 Mod，作为启动级恢复检查或独立恢复中心的后端基础。
- [x] Dashboard 入口只读恢复健康摘要，基于空 `modIds` 全量 profile 扫描展示聚合健康状态。
- [x] App Frame 全局恢复告警，基于空 `modIds` 全量 profile 扫描聚合只显示安全告警和恢复中心导航。
- [x] 独立恢复中心入口，基于空 `modIds` 全量 profile 扫描展示 profile 聚合摘要和逐 Mod 安全状态摘要。
- [x] 恢复中心 rich repair summary，基于稳定 issue code 展示风险等级、阻断理由和人工处理建议；写入型动作仅限 `rollback_required` Mod 的受控回滚入口。
- [x] 恢复中心诊断导出联动，复用 `export_support_diagnostics` 展示完整支持诊断包的安全导出摘要。
- [x] 恢复中心人工处理决策面板，提供重新扫描、导出诊断，并在存在 `rollback_required` Mod 时引导用户到逐 Mod 受控回滚入口。
- [x] 安装恢复受控动作实施计划，明确 durable recovery record / rich status、只读动作预览、受控回滚任务和恢复中心 UI 启用的后续拆分边界。
- [x] Durable recovery record 基础模型、port 和 JSON 仓储：只提供后端内部状态事实的持久化基础，不新增 command、前端按钮、恢复执行或 `rollback_required` 扫描分支。
- [x] 安装 commit 写入 durable recovery record：提交编排受控写入 `planned` / `committing` / `completed`，并且仅在写入窗口后 rollback 失败时留下 `rollback_required`；不新增 command、DTO、前端按钮、恢复执行或 `rollback_required` 扫描分支。
- [x] 只读恢复扫描消费 durable recovery record：`scan_install_recovery` 可由 `committing` / `rollback_required` record 返回 `rollback_required`，空 `modIds` 全量扫描会补入只有 recovery record 的半完成安装；不新增恢复执行、前端按钮、task phase 或 manifest 写入。
- [x] Manifest 状态摘要消费只读恢复扫描事实：`get_install_manifest_status` 可选接收 `gameId`，传入后复用只读 recovery scan 并返回 `rollback_required` / `repair_required` / `unknown` 等不安全安装摘要；未传 `gameId` 时保留 manifest-only fallback。
- [x] 只读恢复动作预览：`preview_recovery_action` 可预览 `rollback_install` 是否满足受控回滚前置条件，只返回 `available` / `blocked`、聚合计数和稳定阻断 reason code；不新增恢复执行、task phase、Audit Log、manifest 写入或恢复中心写入型按钮。
- [x] 受控回滚任务前置安全加固：`committing` / `rollback_required` record 对覆盖文件保留本次 pending backup 作为“安装前一刻”的回滚来源，`completed` record 才恢复为 manifest 长期 backup 语义；不新增 command、DTO、task phase、Audit Log、manifest 写入或恢复中心写入型按钮。
- [x] 后端受控回滚任务：`start_recovery_action_task` 可执行 `rollback_install`，发送 `install.recovery.*` task phase，写入 `rollback_install` Audit Log，并将 durable recovery record 标记为 `rolled_back`；恢复中心已启用逐 Mod 受控回滚按钮。
- [x] Rich manifest `rolled_back` 同步：受控 `rollback_install` 成功后，在已有 manifest 中移除该 Mod 的 stale entries 并把 manifest status 持久化为 `rolled_back`；manifest 或 recovery record 保存失败时会 best-effort 回滚文件动作并避免持久状态互相矛盾。
- [x] Rich manifest schema metadata：`InstallManifest` 新增 `manifest_id`、`schema_version` 和可选 `schema_migration`；旧 manifest 缺字段时兼容读取，新写出的安装提交 manifest 会携带稳定 profile-scoped `manifest_id` 和 schema version，commit merge / uninstall 会保留已有 schema metadata。
- [x] Rich manifest 读侧状态机消费规则：`InstallManifestStatus::consumption()` 在 hmm-core 定义统一分类，manifest 状态摘要查询 fallback 和只读恢复扫描都先消费 profile 级 manifest status（`rollback_required` / `repair_required` → 对应失败态，`planned` / `committing` → unknown，`completed` / `rolled_back` → 继续按 entries / 文件校验），保证失败状态不会被误报为已完成。

### 2026-06-27 进度详情：Durable recovery record 基础

本切片完成 [安装恢复受控动作实施计划](INSTALL_RECOVERY_CONTROLLED_ACTIONS_PLAN.md) 中“切片 1”的第一步基础设施，但没有启用任何破坏性恢复动作。

已落地范围：

- `hmm-core` 新增 `InstallRecoveryRecord`、`InstallRecoveryRecordEntry` 和 `InstallRecoveryRecordStatus`。状态使用稳定 `snake_case` 序列化，并通过 `transition_to` 限制 `planned -> committing -> completed`、`committing -> rollback_required`、`rollback_required -> rolled_back` 等迁移。
- `hmm-ports` 新增 `InstallRecoveryRecordRepository`，只按 `profileId` / `modId` 读写删除恢复记录。
- `hmm-infra` 新增 `JsonInstallRecoveryRecordRepository`，在受控 recovery root 下使用 profile/mod id 的 SHA-256 派生文件名持久化记录，避免把任意 id 直接作为路径片段；仓储读写继续拒绝 symlink / 非普通文件等不安全目标。

仍明确未完成：

- 安装 commit 尚未写入 `planned`、`committing`、`completed` 或 `rollback_required` recovery record。
- 该切片结束时 `scan_install_recovery` 尚未消费 recovery record，也不会返回 `rollback_required`；后续扫描消费切片已补齐该只读状态。
- 没有新增 Tauri command、DTO、task phase、前端 UI、动作预览或受控回滚执行。

验证记录：

- 聚焦 core：`cargo test -p hmm-core recovery_record`。
- 聚焦 infra：`cargo test -p hmm-infra json_recovery_record_repository`。

### 2026-06-27 进度详情：安装 commit 写入 durable recovery record

本切片继续完成 [安装恢复受控动作实施计划](INSTALL_RECOVERY_CONTROLLED_ACTIONS_PLAN.md) 中“切片 1”的安装提交编排部分。它只写后端内部 recovery record，不启用恢复按钮、动作预览或对外 `rollback_required` 扫描状态。

已落地范围：

- `InstallCommitService` 新增可选 `InstallRecoveryRecordRepository` 注入；旧构造函数保持兼容，生产 `start_install_task` 组合根接入 `JsonInstallRecoveryRecordRepository`，记录保存在 app data 下的 `install/recovery`。
- 安装 commit 在 manifest 读取成功后按 `profileId/modId` 写入 `planned`，进入真实写入窗口前写入 `committing`，manifest 保存成功后 best-effort 写入 `completed`。
- 如果进入写入窗口后失败，现有 best-effort rollback 成功时会 best-effort 清理本次 recovery record，避免制造假的待恢复状态；只有 rollback 失败时才通过受控迁移留下 `rollback_required`。
- recovery record entry 只记录受控 target path、package file id、backup ref 和 installed file 摘要，不新增前端 DTO，也不向任务事件暴露路径、backup root、manifest root、sandbox/cache 路径或第三方 Mod 内容。

仍明确未完成：

- 该切片结束时 `scan_install_recovery` 尚未消费 recovery record，也不会返回 `rollback_required`；后续扫描消费切片已补齐该只读状态。
- 没有新增 Tauri command、DTO、task phase、前端 UI、只读动作预览或受控回滚执行。
- 回滚成功后的 `rolled_back` rich 状态、只读动作预览和受控回滚任务当时仍需后续切片；这些能力后续已分步补齐。

验证记录：

- TDD RED：`cargo test -p hmm-app recovery_record` 先失败于缺少 `InstallCommitService::new_with_recovery_records`。
- 聚焦 recovery record：`cargo test -p hmm-app recovery_record`（覆盖成功生命周期、rollback 成功清理记录、rollback 失败留下 `rollback_required`）。
- 聚焦 commit plan 回归：`cargo test -p hmm-app commit_plan`。
- Tauri 安装契约回归：`cargo test -p hmm-tauri install`。
- 全量门禁：`powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify.ps1`。

### 2026-06-27 进度详情：恢复扫描消费 durable recovery record

本切片继续完成 [安装恢复受控动作实施计划](INSTALL_RECOVERY_CONTROLLED_ACTIONS_PLAN.md) 中“切片 1”的扫描消费部分。它只让现有只读 `scan_install_recovery` 消费后端内部 recovery record，不启用恢复按钮、动作预览、回滚执行、task phase 或 manifest 写入。

已落地范围：

- `InstallRecoveryScanService` 可注入 `InstallRecoveryRecordRepository`；生产 `scan_install_recovery` 组合根读取 app data 下 `install/recovery` 的 JSON recovery records。
- `scan_install_recovery` 仅在 durable recovery record 为 `committing` 或 `rollback_required` 时返回对外 `rollback_required`；`planned`、`completed` 和 `rolled_back` 不会被提升为待回滚状态，避免从尚未进入写入窗口的计划状态制造误报。
- 当 `modIds` 为空时，全量 profile 扫描会合并 manifest 内已知 Mod 与 recovery record 内的 Mod；即使半完成安装尚未写入 manifest，也能被只读恢复中心发现。
- `InstallRecoveryStatusDto` 和前端 `InstallRecoveryStatus` 增加稳定 `rollback_required`；Mod 库、Dashboard/App Frame 健康摘要和恢复中心都把它当作需要关注的阻断状态展示，但不新增任何写入型动作。

仍明确未完成：

- 只读动作预览、受控回滚任务、恢复中心真正写入动作和 `rolled_back` rich 状态当时仍需后续切片；这些能力后续已分步补齐。
- 该切片结束时 `get_install_manifest_status` 尚未自动消费 recovery scan 结果；后续 manifest 状态消费切片已补齐该只读映射。
- 没有新增恢复按钮、自动回滚、删除/恢复文件、task phase 或 manifest 写入。

验证记录：

- TDD RED：前端聚焦测试先失败于 `rollback_required` 被健康摘要误判为 `healthy`、恢复中心未计入 attention、ModLibraryPage 阻断条件未覆盖该状态。
- 聚焦 recovery record 扫描：`cargo test -p hmm-app recovery_record`。
- 聚焦 JSON recovery record 仓储：`cargo test -p hmm-infra json_recovery_record_repository`。
- Tauri DTO 序列化：`cargo test -p hmm-tauri install_recovery`。
- 聚焦前端：`cmd /c corepack pnpm exec node --test "src/features/dashboard/installRecoveryHealth.test.mjs" "src/features/install-recovery/recoveryCenterViewModel.test.mjs" "src/features/mods/modLibraryLoadState.test.mjs" "src/features/mods/modInstallPlanApi.test.mjs"`。
- 全量门禁：`powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify.ps1`。

### 2026-06-27 进度详情：只读恢复动作预览

本切片完成 [安装恢复受控动作实施计划](INSTALL_RECOVERY_CONTROLLED_ACTIONS_PLAN.md) 中“切片 2：只读恢复动作预览”。它只判断受控回滚动作是否具备前置条件，不执行删除、恢复、回滚、manifest 写入或 recovery record 写入。

已落地范围：

- `hmm-app` 新增 `InstallRecoveryActionPreviewService`，当前仅支持 `rollback_install`。服务读取 durable recovery record，只有 `committing` / `rollback_required` 状态可进入候选判断；每个 entry 必须有 `installed_file` 摘要，当前目标文件摘要必须匹配，覆盖文件所需 backup 必须存在且可读。
- Tauri 新增 `preview_recovery_action` 窄 command；DTO 只接收 `gameId`、`profileId`、`modId` 和 `actionKind`，组合根复用安装/卸载/恢复扫描同一份 `gameId/profileId` 写锁，并只装配受控 game filesystem、backup store 和 recovery record repository。
- 返回 DTO 只包含 action kind、`available` / `blocked`、将删除的新文件数、将恢复的覆盖文件数、backup 计数、阻断 issue 总数和稳定阻断 reason code：`rollback_state_missing`、`missing_installed_file_summary`、`target_missing`、`target_changed`、`target_read_failed`、`backup_missing`、`backup_read_failed`。
- 前端新增 feature-local `previewRecoveryAction` typed API 和对应 TypeScript 类型；当前没有把它接到恢复中心按钮或任何写入型 UI。

仍明确未完成：

- 受控回滚任务、`install.recovery.*` task phase、Audit Log 写入和执行前持锁重校验仍需后续切片。
- 恢复中心 UI 已显示逐 Mod 可点击回滚按钮；人工处理决策面板中的 `controlled_recovery` 只负责滚动到 Mod 列表，真正写入动作仍由单个 `rollback_required` Mod 行触发。
- `rolled_back` rich 状态和 manifest/recovery record 执行后状态更新当时仍未实现；manifest `rolled_back` 同步已在 2026-07-02 切片补齐。

验证记录：

- TDD RED：`cargo test -p hmm-tauri install_recovery_action_preview` 先失败于缺少 request DTO、command 映射、错误映射和 AppState previewer。
- 聚焦 app：`cargo test -p hmm-app preview_rollback_action`。
- 聚焦 Tauri：`cargo test -p hmm-tauri install_recovery_action_preview`、`cargo test -p hmm-tauri recovery_action_preview_waits_for_shared_game_profile_write_lock`。
- 聚焦前端：`cmd /c corepack pnpm exec node --test "src/features/mods/modInstallPlanApi.test.mjs"`。

### 2026-06-27 进度详情：受控回滚任务前置安全加固

本切片原计划继续推进“切片 3：受控回滚任务”，但在梳理执行前置时发现一个必须先修正的状态事实问题：替换已有托管目标时，manifest entry 需要继承旧条目的长期 `backup_ref`，而 `rollback_required` recovery record 需要保存本次提交前创建的 pending backup，才能让未来受控回滚任务恢复到安装前一刻，而不是误恢复到更早的原始游戏文件。

已落地范围：

- `InstallCommitService` 在进入写入窗口后更新 active recovery record 时，使用本次 pending backup ref 作为 rollback record entry 的 `backup_ref`。
- 如果 `committing` record 已经保存，后续 action 更新 rollback entry 的 pending backup 时会立即重新持久化 active record，避免崩溃恢复读取到旧 manifest 的长期 backup 语义。
- manifest 保存成功后，`completed` recovery record 会重新同步为 manifest entries 的长期 `backup_ref` 和 `installed_file` 摘要，避免 completed record 指向随后会被 best-effort 清理的 pending backup。
- 新增回归测试覆盖“替换已有托管目标、manifest 保存失败且 rollback 失败”场景，断言 `rollback_required` record 使用 pending backup ref，而不是旧 manifest 的长期 backup ref。
- 新增回归测试覆盖“后续 action 才拿到 pending backup 且 commit 成功”场景，断言最后持久化的 `committing` record 也已经包含 pending backup ref。

仍明确未完成：

- 本切片不新增受控回滚任务、`install.recovery.*` task phase、Tauri command、前端写入型入口、Audit Log 或 manifest 写入动作。
- 后续仍应继续从 [安装恢复受控动作实施计划](INSTALL_RECOVERY_CONTROLLED_ACTIONS_PLAN.md) 的“切片 3：受控回滚任务”推进，但执行服务必须消费上述 rollback backup 语义。

验证记录：

- TDD RED：`cargo test -p hmm-app commit_plan_rollback_record_uses_pending_backup_when_replacing_managed_target` 先失败于 recovery record 保存旧 manifest 的长期 `backup_ref`。
- TDD RED：`cargo test -p hmm-app commit_plan_persists_committing_record_after_later_pending_backup_update` 先失败于最后持久化的 `committing` record 仍保存旧 manifest 的长期 `backup_ref`。
- 聚焦 recovery record：`cargo test -p hmm-app recovery_record`。
- 聚焦 commit plan：`cargo test -p hmm-app commit_plan`。
- 聚焦动作预览：`cargo test -p hmm-app preview_rollback_action`。
- 全量门禁：`powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify.ps1`。

### 2026-06-27 进度详情：后端受控回滚任务

本切片完成 [安装恢复受控动作实施计划](INSTALL_RECOVERY_CONTROLLED_ACTIONS_PLAN.md) 中“切片 3：受控回滚任务”的后端优先最小版本。它启用后端/Tauri 任务入口，但仍不启用恢复中心写入型按钮。

已落地范围：

- `hmm-app` 新增 `InstallRecoveryActionService`，当前仅支持 `rollback_install`。服务读取 durable recovery record，只允许 `committing` / `rollback_required` 进入执行；每个 entry 都会重新校验 `installed_file` 摘要，覆盖文件还要求 backup 存在且可读。
- `rollback_install` 执行时删除本工具新增文件，或从 recovery record 中的 backup 恢复覆盖文件；执行后将 durable recovery record 标记为 `rolled_back`。如果 recovery record 保存失败，会 best-effort 回滚已经执行的文件动作。
- `RecoveryActionTaskRunner` 已接入 `TaskKind::Install`，复用安装/卸载同一 `gameId/profileId` 写锁，发送 `install.recovery.queued`、`install.recovery.planning`、`install.recovery.processing`、`install.recovery.completed` 和 `install.recovery.failed` 阶段事件。
- Tauri 新增 `start_recovery_action_task` 窄 command；DTO 只接收 `gameId`、`profileId`、`modId` 和 `actionKind`，不接受 target path、backup ref/root、manifest root/path、sandbox/cache 路径或本地路径。
- 前端新增 feature-local `startRecoveryActionTask` typed API，仅封装上述短 id 入参；恢复中心后续切片已将其接入逐 Mod 受控回滚按钮。
- Audit Log 新增 `rollback_install` operation，字段只包含 `task_id`、`game_id`、`mod_id`、`profile_id`、`remove_file_count`、`restore_file_count` 和 `backup_count` 等短 id/计数。

仍明确未完成：

- 恢复中心逐 Mod 写入型按钮已启用；该 UI 必须只在后端 preview/action 条件满足时允许确认，并在任务完成后重新扫描。
- Rich manifest `rolled_back` 持久化当时尚未完成；后续 2026-07-02 切片已补齐受控回滚成功后的 manifest 同步。
- 不做后台自动恢复，不根据当前 Mod 包内容猜测恢复动作。

验证记录：

- 聚焦 action：`cargo test -p hmm-app rollback_install_action`。
- 聚焦 app task：`cargo test -p hmm-app recovery_action`。
- 聚焦 Tauri：`cargo test -p hmm-tauri recovery_action`。
- 聚焦前端 typed API：`cmd /c corepack pnpm exec node --test "src/features/mods/modInstallPlanApi.test.mjs"`。
- 全量门禁：`powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify.ps1`。

### 2026-07-02 进度详情：Rich manifest rolled_back 同步

本切片完成 T9 Rich Manifest 的一个小范围状态机消费点：受控 `rollback_install` 成功后，后端不仅持久化 durable recovery record 的 `rolled_back` 状态，也会同步已有 rich manifest，避免后续恢复扫描把已删除/已恢复的 stale manifest entries 误判为 `repair_required`。

已落地范围：

- `InstallRecoveryActionService` 新增 manifest repository 注入入口；生产 `start_recovery_action_task` 组合根接入 `JsonInstallManifestRepository`。
- `rollback_install` 在持锁执行并通过目标/backup 重新验证后，会删除本工具新增文件或从 recovery record 的 backup 恢复覆盖文件；随后在已有 manifest 中移除该 `modId` 的 entries，并把 manifest status 标记为 `rolled_back`。
- 如果 manifest 保存失败，后端会 best-effort 回滚已执行的文件动作，并保留 recovery record 的 `rollback_required` 状态。
- 如果 manifest 已保存但 recovery record 保存失败，后端会 best-effort 回滚已执行的文件动作，并把原 manifest 写回，避免文件、manifest 和 recovery record 状态互相矛盾。

仍明确未完成：

- 不新增 Tauri command、DTO、task phase、前端入口或 Audit Log 字段。
- 不处理 `manifest_id`、schema/migration metadata、replacement binding snapshot；前两项已在后续 2026-07-02 schema metadata 切片补齐。
- 不实现完整 `rollback_required` rich manifest 持久化、批量 repair workflow 或真实 `repair_required` 自动修复。

验证记录：

- TDD RED：`cargo test -p hmm-app run_rollback_install_action_persists_manifest_rolled_back_without_stale_mod_entries` 先失败于 `InstallRecoveryActionService::new_with_manifest` 不存在。
- 聚焦 action：`cargo test -p hmm-app run_rollback_install_action`。
- 聚焦 recovery scan/action：`cargo test -p hmm-app install_recovery`。
- 聚焦 Tauri 桥接：`cargo test -p hmm-tauri recovery_action`。
- 全量门禁：本切片完成前需再次执行 `powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify.ps1`。

### 2026-07-02 进度详情：Rich manifest 读侧状态机消费规则

本切片让已持久化的 profile 级 rich manifest `status` 参与只读消费路径。此前 `rollback_required` / `repair_required` / `planned` / `committing` 的 manifest 在状态摘要查询 fallback 和恢复扫描里会被误报为 `installed` / `completed`，违反「失败状态不会被误报为已完成」验收标准。

已落地范围：

- `hmm-core` 新增 `InstallManifestStatusConsumption` 读侧消费分类和 `InstallManifestStatus::consumption()`：`completed` / `rolled_back` → 信任 entries，`planned` / `committing` → in-flight，`rollback_required` / `repair_required` → 对应失败态。
- `InstallManifestQueryService`（manifest-only fallback）：有 entries 的 Mod 先经消费规则映射（失败态 → `rollback_required` / `repair_required`，in-flight → `unknown`），无 entries 的 Mod 仍报 `not_installed`。
- `InstallRecoveryScanService::scan_mod`：durable recovery record 仍然优先；无 record 时在逐 entry 文件校验前先消费 manifest status，失败/in-flight 状态直接返回对应摘要状态且不产生 issue code。
- `rolled_back` 对剩余 Mod 不降级：受控回滚只移除该 Mod 的 entries，剩余 entries 继续按文件校验消费。

仍明确未完成：

- 不新增 Tauri command、DTO、issue code、task phase、前端入口或 Audit Log 字段（既有 DTO 变体已覆盖全部输出状态）。
- 不做写侧状态机门禁（manifest 失败态时阻断安装 commit / 卸载），需要新错误码与 contract 变更，另行切片。
- 不处理 replacement binding snapshot、真实 `repair_required` 自动修复或 schema 迁移。

验证记录：

- TDD RED：`cargo test -p hmm-core manifest_status_consumption` 先失败于缺少 `InstallManifestStatusConsumption`。
- TDD RED：`cargo test -p hmm-app query_reports` 3 个测试先失败于 fallback 忽略 manifest status；`cargo test -p hmm-app scan_reports_rollback_required_when_manifest_status` 先失败于 `left: Completed / right: RollbackRequired`。
- 聚焦 core：`cargo test -p hmm-core`（25 通过）。
- 聚焦 app：`cargo test -p hmm-app`（164 通过，含 9 个新消费测试）。
- 全量：`cargo test --workspace` 通过；全量门禁 `powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify.ps1` 于切片完成前执行。
- 文件大小硬性线预防：`install_recovery.rs` 内联 tests 拆分到 `install_recovery_tests.rs`（998 / 1176 行）。

### 2026-07-02 进度详情：Rich manifest schema metadata

本切片补齐 T9 Rich Manifest 的最小 schema/identity metadata：`InstallManifest` 新增稳定 `manifest_id`、`schema_version` 和可选 `schema_migration` 字段，旧 JSON 缺字段时兼容读取，新写出的安装提交 manifest 会携带这些字段。

已落地范围：

- `manifest_id` 当前采用 profile-scoped 稳定 ID：`profile:<profile_id>`，匹配现有 profile 聚合 manifest 文件语义。
- `schema_version` 默认并写出为 `1`；旧 manifest 缺字段时按 v1 兼容读取。
- `schema_migration` 是可选字段；当前不执行真实迁移，缺失时保持 `None`。
- 安装提交合并已有 manifest、卸载保留剩余 entries 时，会保留已有 `manifest_id` / `schema_version` / `schema_migration`，避免后续迁移信息被重置。

仍明确未完成：

- 不新增 Tauri command、DTO、task phase、前端入口或 UI 展示。
- 不处理 replacement binding snapshot。
- 不在本切片定稿 `game_id`、`game_instance_id` 或顶层 `mod_id` 语义；这些字段需要结合 profile 聚合 manifest 和未来多游戏/profile 存储边界另行设计。
- 不实现完整 `planned` / `committing` / `rollback_required` rich manifest 状态持久化或 repair workflow。

验证记录：

- TDD RED：`cargo test -p hmm-core manifest_metadata_serializes_with_stable_schema_fields` 先失败于 `InstallManifest` 缺少 `manifest_id` / `schema_version` / `schema_migration` 字段。
- TDD RED：`cargo test -p hmm-app preserves_ -- --nocapture` 先失败于 commit merge / uninstall 将已有 `manifest_id` 重置为默认值。
- 聚焦 core：`cargo test -p hmm-core manifest`。
- 聚焦 install/app：`cargo test -p hmm-app install`。
- 聚焦 infra manifest 仓储：`cargo test -p hmm-infra json_manifest_repository`。
- 全量门禁：本切片完成前需再次执行 `powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify.ps1`。

### 2026-06-26 进度详情：PR #87 Manifest 状态摘要查询

PR #87 已合并，完成了 P0 “Manifest 查询与安装状态摘要”切片。它把 Mod 库安装状态从页面内存任务态推进到后端 manifest 摘要查询，但仍保持 MVP 边界：当前只根据已有 manifest entries 派生 `installed` / `not_installed` 等摘要，不做目标文件 hash 校验、backup 完整性校验、跨进程恢复扫描或安全卸载。

已落地范围：

- 后端新增 `InstallManifestQueryService`，通过受控 `InstallManifestRepository` 读取 profile manifest；缺失 manifest 时返回 `not_installed`，匹配 entry 时返回 `installed`、`managed_file_count` 和 `backup_count`。
- Tauri 新增 `get_install_manifest_status` 窄 command；最初 DTO 只接收 `profileId` 和 `modIds`，读取失败使用稳定错误码 `install_manifest_unavailable`；后续已补充可选 `gameId`，用于在保持旧 manifest-only fallback 的同时消费只读 recovery scan 事实。
- 前端新增 feature-local typed API；Mod 库加载成功和 `install.completed` 后刷新 manifest 摘要。
- Mod 卡片展示 `not_installed`、`installed`、`repair_required`、`unknown` 等后端摘要状态；安装事实不再来自 mock 数据或页面内存任务状态。
- CodeRabbit 评论修复：`applyInstallManifestStatusSummaries` 保留已有 `disabled` / `conflict` UI 状态，同时把 manifest 事实写入 `installSummary.status`；`repair_required` 仅在 `managedFileCount > 0` 时追加文件数，避免显示“需要修复 · 0 文件”。

验证记录：

- 聚焦回归：`cmd /c corepack pnpm exec node --test "src/features/mods/modLibraryLoadState.test.mjs" "src/features/mods/modPreviewImage.test.mjs"`。
- 前端：`cmd /c corepack pnpm run typecheck`、`cmd /c corepack pnpm run lint`、`cmd /c corepack pnpm run test`。
- 全量门禁：`git diff --check`、`powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify.ps1`。
- 本地 review 记录位于 `.planning/reviews/2026-06-26-install-manifest-query-review.md` 和 `.planning/reviews/2026-06-26-install-manifest-query-coderabbit-fix-review.md`，该目录为运行时上下文，不提交。

仍明确未完成：

- 基于 manifest 的 uninstall。
- Crash/recovery 扫描。
- Rich manifest 状态机、目标/backup 校验和真实 `repair_required` 检测。
- ARMOR_RETARGET staging 接入 InstallPlan。
- Dependency / preflight 阻断。

### 2026-06-26 进度详情：Manifest installed file 摘要

本切片完成 P1 “基于 manifest 的 uninstall” 的安全前置：新提交的 manifest entry 记录本工具实际写入到目标路径的 `installed_file` 摘要，包含 `size_bytes` 和 SHA-256。这样后续卸载可以先比较当前目标文件摘要与 manifest 事实，再决定删除新增文件或用 backup 恢复覆盖文件，避免根据当前 Mod 包内容重新猜测。

已落地范围：

- `hmm-core` 新增 `InstalledFileSummary`，并在 `InstallManifestEntry.installed_file` 上使用可选字段，旧 manifest 缺少该字段时仍能读取。
- `hmm-app` 的安装提交服务在写入 manifest entry 时记录写入内容 size 和 SHA-256。
- 现有 manifest 查询 DTO 不暴露 hash，也不把摘要返回前端；查询服务仍只返回状态和计数摘要。

仍明确未完成：

- Manifest 摘要切片本身未实现自动卸载；当前最小 manifest 驱动卸载任务入口见下一小节。
- 查询服务尚未读取真实目标文件或 backup 做 hash 校验。
- 缺少 `installed_file` 的旧 manifest 后续必须阻断自动卸载或转入修复流程，不能视为安全可卸载。

### 2026-06-26 进度详情：后端最小 manifest 驱动卸载

本切片完成 P1 “基于 manifest 的卸载” 的后端最小安全路径和 Tauri 任务入口。它仍保持 MVP 边界：不根据当前 Mod 包内容猜测，不删除 manifest 未记录文件，不接前端传入路径，也不把 hash、backup ref 或 manifest 正文返回前端。

已落地范围：

- `hmm-app` 新增 `UninstallModService`。服务读取 profile manifest，只处理指定 `modId` 的 entries；每个 entry 必须有 `installed_file` 摘要，且当前目标文件 size/SHA-256 必须与 manifest 匹配。
- 新增文件卸载时删除目标文件；覆盖文件卸载时读取受控 backup 并恢复原文件；manifest 保存失败时 best-effort 回滚已删除或已恢复的目标。
- `hmm-ports` / `hmm-infra` 为 `InstallBackupStore` 增加受控 `read_backup`，实现层继续执行 backup root containment、普通文件校验和 traversal 拒绝。
- `hmm-app` 新增 `UninstallTaskService` / `UninstallTaskRunner`，复用 `TaskKind::Install` 和 `gameId/profileId` 写锁，发送 `install.uninstall.processing`、`install.uninstall.completed`、`install.uninstall.failed`，并写入只含短 id/计数的最小 Audit Log。
- Tauri 新增 `start_uninstall_task`，DTO 只接收 `gameId`、`modId`、`profileId`，queued phase 为 `install.uninstall.queued`；组合根通过 `GameConfigRepository` 解析 game root，再装配受控 game file system、backup store 和 manifest repository。

仍明确未完成：

- 前端 Mod 库的最小单选卸载 UI 已在下一小节落地；仍缺少 rich repair summary、批量/profile 工作流和恢复扫描入口。
- 卸载失败当前只通过任务失败 phase 和稳定错误前缀表达，尚未提供 rich repair summary。
- `get_install_manifest_status` 仍不读取目标文件或 backup 做真实 `repair_required` 检测。
- 跨进程崩溃恢复扫描仍未实现。

### 2026-06-26 进度详情：前端最小卸载 UI 接入

本切片完成 P1 “基于 manifest 的卸载” 的前端最小接入。它只消费后端 manifest 摘要和任务事件，不把卸载规则、目标路径、backup ref 或 manifest 正文带到前端。

已落地范围：

- 前端新增 feature-local `startUninstallTask` typed API，只调用 `start_uninstall_task` 并提交 `gameId`、`modId`、`profileId`。
- Mod 库只在单选条目的 `installSummary.status === "installed"` 时启用卸载入口；该状态来自 `get_install_manifest_status`，不是页面内存任务态或 mock 数据。
- 卸载操作先展示确认面板，确认后启动后端任务；确认摘要只展示托管文件数和备份恢复点数量，不展示目标路径、backup ref、manifest 路径或第三方 Mod 内容。
- 前端集中维护安装/卸载任务 phase 映射，按 `taskId` 和 operation 归属 `install.*` / `install.uninstall.*` 进度事件，继续处理 command 返回前事件先到达的竞态。
- `install.uninstall.completed` 后复用 manifest 摘要查询刷新安装事实；失败只显示稳定失败摘要，不尝试在前端推断修复动作。

仍明确未完成：

- rich repair summary、恢复扫描入口和批量/profile 卸载工作流。
- 该切片结束时 `get_install_manifest_status` 尚未读取目标文件或 backup 做真实 `repair_required` 检测；后续带 `gameId` 的状态摘要已通过只读 recovery scan 补齐该检测。
- 卸载失败后的人工修复建议仍需要后续后端结构化摘要支撑。

### 2026-06-26 进度详情：只读恢复扫描摘要

本切片完成 P1 “崩溃恢复扫描” 的后端只读摘要基础。它不执行自动回滚或修复，只基于 manifest、当前目标文件摘要和 backup 可读性判断状态，避免根据当前 Mod 包内容重新猜测安装结果。

已落地范围：

- `hmm-app` 新增 `InstallRecoveryScanService`，依赖 `InstallManifestRepository`、`InstallGameFileSystem` 和 `InstallBackupStore`，只读扫描指定 `profileId` / `modIds`；当 `modIds` 为空时，会从该 profile manifest entries 中去重并按稳定顺序扫描全部已知托管 Mod。
- 扫描返回 `completed`、`repair_required`、`unknown`、`not_installed` 摘要；`repair_required` 覆盖缺少 `installed_file` 摘要、目标缺失、目标摘要变化或 backup 缺失，`unknown` 用于目标或 backup 读取失败等无法安全判断状态。
- 摘要只返回短 id、托管文件计数、backup 计数、聚合 issue 数和稳定 issue code，不返回 target path、game root、backup ref/root、manifest root/path、sandbox/cache 路径、manifest 正文或第三方 Mod 内容。
- Tauri 新增 `scan_install_recovery` 窄 command，只接收 `gameId`、`profileId`、`modIds`；`modIds` 可为空，表示扫描当前 profile manifest 内全部已知托管 Mod。组合根通过已配置 game instance 构造受控 game filesystem，通过 app data 构造受控 backup store 和 manifest repository，并复用安装/卸载同一份 `gameId/profileId` 写锁避免读取半写状态。
- 稳定错误码：未配置或无法读取 game instance 返回 `game_instance_unavailable`；manifest 仓储不可用返回 `install_recovery_unavailable`。

仍明确未完成：

- 应用启动级自动调用恢复扫描；Mod 库加载成功后的只读扫描入口已在下一小节落地。
- 自动回滚、自动恢复执行和 `rollback_required` rich 状态机。
- 独立恢复中心只读入口已在后续切片落地；自动处理动作和 rich repair summary 仍未完成。
- 该切片结束时 `get_install_manifest_status` 尚未消费 recovery scan 结果来自动显示真实 `repair_required`；后续 manifest 状态消费切片已补齐。

### 2026-06-26 进度详情：前端恢复扫描入口

本切片完成 P1 “崩溃恢复扫描” 的 Mod 库前端入口。它只消费后端只读 `scan_install_recovery` 摘要，不执行自动回滚、恢复、删除或 manifest 写入；遇到不安全状态时阻断安装/卸载，并提示玩家人工处理。

已落地范围：

- 前端新增 feature-local `scanInstallRecovery` typed API，只调用 `scan_install_recovery` 并提交 `gameId`、`profileId`、`modIds`。
- Mod 库在 `get_install_manifest_status` 摘要刷新后调用只读恢复扫描；扫描失败时把已有非 `not_installed` manifest 摘要降级为不安全 `unknown`，不把失败回退为 mock 安装事实。
- `applyInstallRecoverySummaries` 把后端 `completed` 映射为前端 `installed`，把 `repair_required` / `unknown` 合并为不安全安装状态，同时保留 `disabled` / `conflict` 等非 manifest UI 状态。
- 安装/重装入口在 `repair_required` / `unknown` 时被阻断；卸载仍只允许后端摘要为 `installed` 的单选条目，不根据 Mod 包内容、页面内存态或展示标签推断可卸载。
- 人工处理面板和卡片状态只展示托管文件数、backup 计数、聚合 issue code 与计数，不展示 target path、game root、backup ref/root、manifest root/path、sandbox/cache 路径、manifest 正文或第三方 Mod 内容。

验证记录：

- 聚焦前端回归：`cmd /c corepack pnpm exec node --test "src/features/mods/modInstallPlanApi.test.mjs" "src/features/mods/modLibraryLoadState.test.mjs"`。
- 前端：`cmd /c corepack pnpm run typecheck`、`cmd /c corepack pnpm run lint`、`cmd /c corepack pnpm run build`、`cmd /c corepack pnpm run test`。
- 全量门禁：`git diff --check`、`powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify.ps1`。

仍明确未完成：

- 应用启动级或独立恢复中心扫描入口。
- 自动回滚、自动恢复执行和 `rollback_required` rich 状态机。
- 后端 rich repair summary、人工修复决策流和批量/profile 工作流。
- 该切片结束时 `get_install_manifest_status` 尚未在后端内部消费 recovery scan 结果；后续已补齐带 `gameId` 的后端映射，Mod 库仍额外调用 recovery scan 获取 issue code 和计数。

### 2026-06-26 进度详情：Recovery scan 全量 profile 扫描基础

本切片补齐 P1 “崩溃恢复扫描” 的启动级/恢复中心基础能力：前端或后续恢复中心可以用空 `modIds` 请求扫描当前 profile manifest 内全部已知托管 Mod，而不必先加载 Mod 库或在前端维护一份待扫描 id 列表。

已落地范围：

- `InstallRecoveryScanService` 在 `modIds` 为空时，从 manifest entries 中按 `mod_id` 去重并稳定排序，再复用既有只读扫描逻辑返回每个 Mod 的恢复摘要。
- `scan_install_recovery` Tauri DTO 映射允许空 `modIds`；非空 id 仍逐项执行空白校验，`get_install_manifest_status` 仍要求显式 `modIds`，避免改变普通 manifest 摘要查询语义。
- 契约仍只暴露 `gameId`、`profileId`、`modIds` 和聚合摘要，不返回 target path、game root、backup ref/root、manifest root/path、sandbox/cache 路径、manifest 正文或第三方 Mod 内容。

仍明确未完成：

- Dashboard 入口级健康摘要已在下一小节落地；全局后台启动告警仍未完成。
- 独立恢复中心只读页面已在后续切片落地；自动处理动作和 rich repair summary 仍未完成。
- 自动回滚、自动恢复执行和 `rollback_required` rich 状态机。

### 2026-06-26 进度详情：Dashboard 入口恢复健康摘要

本切片把 P1 “崩溃恢复扫描” 的空 `modIds` 全量 profile 扫描能力接入 Dashboard 入口。它只做只读健康摘要，不提供恢复、回滚、删除、重写 manifest 或自动修复动作。

已落地范围：

- Dashboard 在游戏目录配置完成后调用只读 `scan_install_recovery`，提交 `gameId`、`profileId` 和空 `modIds`，由后端扫描当前 profile manifest 内全部已知托管 Mod。
- 新增前端聚合 helper，把 `completed` / `not_installed` 归为健康或空记录，把 `repair_required` / `unknown` 归为需要关注，并聚合扫描 Mod 数、需处理数、未知数、托管文件数、backup 计数和 issue 计数。
- 右侧设置状态栏新增安装健康摘要，只展示聚合计数和稳定 issue 分类标签；不展示 target path、game root、backup ref/root、manifest root/path、sandbox/cache 路径、目标文件 hash、manifest 正文或第三方 Mod 内容。
- 扫描失败时显示状态未知，不把失败解释为健康，也不启用自动处理动作。

验证记录：

- 聚焦前端回归：`cmd /c corepack pnpm exec node --test "src/features/dashboard/installRecoveryHealth.test.mjs" "src/features/dashboard/dashboardInstallRecoveryHealth.test.mjs"`。
- 前端：`cmd /c corepack pnpm run typecheck`、`cmd /c corepack pnpm run lint`、`cmd /c corepack pnpm run build`、`cmd /c corepack pnpm run test`。
- Dashboard 边界：`powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\check-frontend-boundaries.ps1`。

仍明确未完成：

- 独立恢复中心只读页面已在下一小节落地；自动处理动作和 rich repair summary 仍未完成。
- 自动回滚、自动恢复执行和 `rollback_required` rich 状态机。
- 全局后台启动告警、跨 profile 批量健康摘要和更完整的恢复决策流。

### 2026-06-26 进度详情：独立恢复中心只读入口

本切片把 P1 “崩溃恢复扫描” 的空 `modIds` 全量 profile 扫描能力接入独立恢复中心入口。它只做只读展示，不提供恢复、回滚、删除、重写 manifest 或自动修复动作。

已落地范围：

- 新增 `recovery` route 和 “恢复中心”导航项，复用现有 App Frame、单一 `navItems` 和 route registry，不按侧边栏模式复制页面。
- 恢复中心在游戏目录配置完成后调用只读 `scan_install_recovery`，提交 `gameId`、`profileId` 和空 `modIds`，由后端扫描当前 profile manifest 内全部已知托管 Mod。
- 新增前端 view model helper，把 `completed` / `not_installed` 归为正常或空记录，把 `repair_required` / `unknown` 归为需要关注，并聚合扫描 Mod 数、正常数、需处理数、未知数、托管文件数、backup 计数和 issue 计数。
- 页面展示 profile 级摘要、聚合 issue 分类，以及每个托管 Mod 的短 id、状态、托管文件计数、backup 计数、issue 计数和稳定 issue 分类；不展示 target path、game root、backup ref/root、manifest root/path、sandbox/cache 路径、目标文件 hash、manifest 正文或第三方 Mod 内容。
- 未配置游戏目录时恢复中心不调用恢复扫描，只提示先完成受控游戏实例配置。

仍明确未完成：

- 受控恢复/回滚动作和人工修复决策流。
- 自动回滚、自动恢复执行和 `rollback_required` rich 状态机。
- 全局后台启动告警、跨 profile 批量健康摘要和更完整的恢复决策流。

### 2026-06-26 进度详情：恢复中心只读 rich repair summary

本切片在独立恢复中心内补充只读 rich repair summary。它只消费现有 `scan_install_recovery` DTO 和稳定 issue code，在前端 view model 中派生风险等级、阻断原因和人工处理建议，不新增 Tauri command/DTO/Rust 写路径，也不执行恢复、删除、回滚或 manifest 写入。

已落地范围：

- `deriveRecoveryCenterViewModel` 为 profile 和逐 Mod 条目派生 `repairSummary`，区分 `clear`、`manual_required` 和 `unknown`，并给出安全的 `blockingReason` 与 `actionLabel`。
- 每个稳定 issue code 增加只读展示 metadata：标签、风险等级和人工处理指引；这些指引只描述“保持阻断、刷新确认、保留现场、等待受控流程”等安全动作，不承诺自动恢复。
- 恢复中心页面展示“恢复处理摘要”和 issue 指引，仍不展示 target path、game root、backup ref/root、manifest root/path、sandbox/cache 路径、目标 hash、manifest 正文或第三方 Mod 内容。
- 页面没有新增 `start_install_task`、`start_uninstall_task`、恢复、删除、回滚或 manifest 写入入口；刷新仍只是重新读取只读恢复扫描。

验证记录：

- TDD RED：新增恢复中心 view model 与页面 source 测试后，聚焦测试先失败于缺少 `repairSummary` / `RepairSummaryPanel`。
- 聚焦验证：`cmd /c corepack pnpm exec node --test "src/features/install-recovery/recoveryCenterViewModel.test.mjs" "src/features/install-recovery/recoveryCenterRoute.test.mjs"`。
- 前端验证：`cmd /c corepack pnpm run typecheck`、`cmd /c corepack pnpm run lint`、`powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\check-frontend-boundaries.ps1`、`cmd /c corepack pnpm run build`、`cmd /c corepack pnpm run test`。
- 浏览器覆盖：当前环境没有 Playwright/browser 自动化工具可用；已确认 `http://127.0.0.1:5173/` HTTP 可访问，后续完整提交前仍需记录未执行真实截图/交互 smoke 的原因。

仍明确未完成：

- 受控恢复/回滚动作和人工修复决策流。
- 自动回滚、自动恢复执行和 `rollback_required` rich 状态机。
- 全局后台启动告警、跨 profile 批量健康摘要和更完整的恢复决策流。

### 2026-06-26 进度详情：恢复中心诊断导出联动

本切片在独立恢复中心内补充用户主动触发的完整支持诊断包导出入口。它复用已有 `export_support_diagnostics` 后端 command 和 DTO，不新增 Tauri command、Rust 写入链路、恢复/回滚动作或 manifest 写入。

已落地范围：

- 前端新增 `src/features/install-recovery/recoveryDiagnosticsApi.ts` 与 `recoveryDiagnosticsTypes.ts`，feature-local wrapper 只调用 `export_support_diagnostics`，不传入输出路径、日志路径、类别选择、行数或事件数量参数。
- 新增 `useRecoveryDiagnosticsExport` 管理 `idle` / `confirming` / `exporting` / `exported` / `failed` 五种页面状态；失败只展示安全失败摘要，不展示原始错误文本。
- 恢复中心页面新增“导出诊断”按钮，先展示已脱敏类别确认，确认后导出并只展示 `exportId`、`fileName`、`sizeBytes`、App/Task 日志行数和 Audit event 计数。
- 页面仍不展示诊断包完整路径、日志正文、审计事件正文、target path、game root、backup ref/root、manifest root/path、sandbox/cache 路径、目标 hash、manifest 正文或第三方 Mod 内容；也不提供自动恢复、删除、回滚或 manifest 写入入口。

验证记录：

- TDD RED：新增恢复中心页面 source 测试后，聚焦测试先失败于缺少 `recoveryDiagnosticsApi.ts`。
- 聚焦验证：`cmd /c corepack pnpm exec node --test "src/features/install-recovery/recoveryCenterViewModel.test.mjs" "src/features/install-recovery/recoveryCenterRoute.test.mjs"`。

仍明确未完成：

- 受控恢复/回滚动作和人工修复决策流。
- 自动回滚、自动恢复执行和 `rollback_required` rich 状态机。
- 全局后台启动告警、跨 profile 批量健康摘要和更完整的恢复决策流。

### 2026-06-26 进度详情：恢复中心只读人工处理决策面板

本切片在独立恢复中心内补充只读人工处理决策面板。它只基于现有 `scan_install_recovery` 摘要、issue code 和前端 view model 派生安全下一步，不新增 Tauri command、Rust 写入链路、恢复/回滚动作或 manifest 写入。

已落地范围：

- `deriveRecoveryCenterViewModel` 为 profile 级摘要派生 `manualDecision`：在 `repair_required` / `unknown` 存在时标记为 blocked，说明自动安装、卸载和恢复动作保持阻断。
- 决策面板只提供两个可用安全动作：重新扫描只触发现有只读 scan refresh，导出诊断只进入已有 `export_support_diagnostics` 确认流程。
- “受控修复”仅作为不可用占位展示，等待后续 manifest 状态机和恢复执行器支持。
- 页面仍不展示 target path、game root、backup ref/root、manifest root/path、sandbox/cache 路径、目标 hash、manifest 正文或第三方 Mod 内容；也不提供自动恢复、删除、回滚或 manifest 写入入口。

验证记录：

- TDD RED：新增恢复中心 view model 与页面 source 测试后，聚焦测试先失败于缺少 `manualDecision` / `ManualHandlingPanel`。
- 聚焦验证：`cmd /c corepack pnpm exec node --test "src/features/install-recovery/recoveryCenterViewModel.test.mjs" "src/features/install-recovery/recoveryCenterRoute.test.mjs"`。

仍明确未完成：

- 真正的受控恢复/回滚动作。
- 自动回滚、自动恢复执行和 `rollback_required` rich 状态机。
- 跨 profile 批量健康摘要和后端恢复决策执行流。

### 2026-06-26 进度详情：App Frame 全局只读恢复告警

本切片把 P1 “崩溃恢复扫描” 的空 `modIds` 全量 profile 扫描能力接入 App Frame 全局告警。它只展示安全聚合提示和恢复中心导航，不提供恢复、回滚、删除、重写 manifest 或自动修复动作。

已落地范围：

- 将 Dashboard 私有的恢复健康聚合 helper / hook 迁到 `src/features/install-recovery/`，Dashboard 保留兼容 re-export，App Frame 和 Dashboard 共享同一套只读聚合逻辑。
- App Frame 在游戏目录配置完成后调用只读 `scan_install_recovery`，提交 `gameId`、`profileId` 和空 `modIds`，由后端扫描当前 profile manifest 内全部已知托管 Mod。
- 新增全局告警 view model：`healthy`、`empty`、`idle` 和 `loading` 不显示；`repair_required` / `unknown` 聚合为需要关注时显示告警；扫描失败显示状态未知。
- 告警只显示需处理数、未知数和 issue 聚合描述，并提供“打开恢复中心”导航；不展示 target path、game root、backup ref/root、manifest root/path、sandbox/cache 路径、目标文件 hash、manifest 正文或第三方 Mod 内容。
- 告警不调用 `start_install_task`、`start_uninstall_task` 或任何恢复、删除、回滚、manifest 写入 command；恢复中心导航只是进入现有只读页面。

验证记录：

- TDD RED：新增 `installRecoveryGlobalAlert.test.mjs` 后，聚焦测试先失败于缺少 `installRecoveryGlobalAlert.ts`、`InstallRecoveryGlobalAlert.tsx` 和 App Frame 接线。
- 聚焦验证：`cmd /c corepack pnpm exec node --test "src/features/install-recovery/installRecoveryGlobalAlert.test.mjs" "src/features/dashboard/installRecoveryHealth.test.mjs" "src/features/dashboard/dashboardInstallRecoveryHealth.test.mjs"`。
- PR #100 本地自审修复：补充 `.app-surface:has(.install-recovery-global-alert)` 三行布局，避免告警出现时占用主内容的 `1fr` 行；新增 source-level 回归断言，并通过完整 `scripts/verify.ps1`。

仍明确未完成：

- 真正的受控恢复/回滚动作。
- 自动回滚、自动恢复执行和 `rollback_required` rich 状态机。
- 跨 profile 批量健康摘要和后端恢复决策执行流。

### 2026-06-26 进度详情：安装恢复受控动作实施计划

本切片补充 [安装恢复受控动作实施计划](INSTALL_RECOVERY_CONTROLLED_ACTIONS_PLAN.md)，用于约束后续真正会写游戏目录、恢复 backup、删除目标文件或写 manifest/recovery record 的高风险恢复动作。它不新增 Tauri command、不新增 DTO、不改变 Rust 服务、不执行恢复/删除/回滚或 manifest 写入。

已明确的后续拆分：

- 先补 durable recovery record / rich status 基础，确保 `rollback_required` 只能来自持久化状态事实，而不是目录内容猜测。
- 再补只读恢复动作预览，只返回可执行性、聚合计数和稳定阻断 reason code，不返回 target path、backup ref、manifest path、hash 或第三方 Mod 内容。
- 最后才补受控回滚任务和恢复中心 UI 启用；执行任务必须复用同一 `gameId/profileId` 写锁，执行前重新验证目标摘要和 backup，并写 Audit Log。

仍明确未完成：

- durable recovery record 扫描消费 / rich manifest 状态代码。
- `rollback_required` scan 分支。
- `install.recovery.*` 任务事件和受控回滚 command。
- 恢复中心中的实际恢复按钮。

## 设计细化规则

本节用于约束后续 InstallPlan PR 的“怎么做”。如果后续实现发现这里的规则与代码事实冲突，应先更新本文档并说明取舍，再修改实现。

### Manifest schema 与状态规则

当前 MVP manifest 只应承担安装事实记录，不应提前变成 UI 状态缓存或日志替代品。后续 rich manifest 可以分两层演进：

| 层级 | 字段 | 用途 |
| --- | --- | --- |
| MVP 必需 | `manifest_id`、`game_id`、`profile_id`、`mod_id` | 定位一次受控安装事实，供查询、卸载和恢复扫描使用。 |
| MVP 必需 | `files` | 记录最终目标相对路径、动作类型、`installed_file` 写入摘要和是否覆盖旧文件。 |
| MVP 必需 | `backups` | 记录覆盖前备份引用，卸载或回滚必须通过它恢复旧文件。 |
| MVP 必需 | `plan_hash` | 绑定本次提交消费的计划摘要，避免安装后被误判为另一个计划。 |
| Rich manifest | `backend`、`status`、`created_at`、`completed_at` | 支持状态机、恢复扫描、迁移和 UI 摘要。 |
| Rich manifest | `replacement_bindings` | 记录玩家选择的替换目标快照，而不是依赖当前 staging 目录推断。 |
| Rich manifest | `schema_version`、`migrated_from` | 支持旧 manifest 兼容读取和一次性迁移。 |

状态含义：

| 状态 | 含义 | UI 可展示摘要 | 后续动作 |
| --- | --- | --- | --- |
| `planned` | 计划已持久化但尚未开始真实写入。 | 等待安装 | 可取消或重新生成计划。 |
| `committing` | 已进入真实写入窗口，进程中断时可能留下半完成状态。 | 安装中或需要检查 | 启动恢复扫描，不能直接显示为已安装。 |
| `completed` | manifest 与目标状态摘要一致，安装完成。 | 已安装 | 可查询、卸载、重装或 retarget。 |
| `rollback_required` | 写入失败且回滚未确认完成。 | 需要恢复 | 阻断卸载和再次安装，先执行恢复/回滚。 |
| `rolled_back` | 回滚完成，目标状态已恢复或清理。 | 已回滚 | 可重新安装；保留审计记录。 |
| `repair_required` | manifest、backup 或目标状态不一致，无法安全自动判断。 | 需要人工处理 | 阻断破坏性操作，提供诊断和人工修复建议。 |

允许的状态迁移：

```text
planned -> committing -> completed
planned -> rolled_back
committing -> completed
committing -> rollback_required
committing -> repair_required
rollback_required -> rolled_back
rollback_required -> repair_required
repair_required -> planned
completed -> rollback_required
completed -> repair_required
```

兼容规则：

- 读取旧 manifest 时必须走兼容层，缺少 rich 字段不能直接当作损坏。
- 缺少 `installed_file` 摘要的旧 manifest 可以用于只读安装状态摘要，但不能自动执行删除或恢复；后续卸载应返回需要修复/迁移/人工确认的安全状态。
- 提交服务合并 MVP manifest 时只按本次写入的目标路径替换旧条目；未触达目标必须保留，不能因为 `modId` 相同就遗忘仍在游戏目录中的托管文件。
- 替换已有托管目标时，新 manifest entry 必须继承旧条目的长期 `backup_ref` 语义；本次提交为了失败回滚创建的中间状态 backup 不能覆盖原始恢复引用，提交成功后应 best-effort 清理。
- 缺少 `status` 的旧 manifest 可以按只读摘要处理，但不能自动承诺可安全卸载。
- 新增字段默认向后兼容；删除或重命名字段必须带迁移测试。
- Manifest 只能记录受控安装事实和必要快照，不记录完整本地路径、sandbox/cache 路径、备份绝对路径或第三方 Mod 内容。

### 卸载与恢复决策表

卸载、恢复和修复扫描必须基于 manifest、backup 和受控目标状态，不能基于当前 Mod 包内容重新猜测。

| 场景 | 判断依据 | 动作 | 阻断行为 | 审计/日志 |
| --- | --- | --- | --- | --- |
| `completed` 且目标文件摘要匹配 | manifest `files` 与当前目标摘要一致 | 可执行 manifest 驱动卸载 | 不阻断 | 记录卸载计划、删除/恢复动作和结果。 |
| `completed` 但目标文件缺失 | manifest 有记录，目标不存在 | 标记 `repair_required` | 阻断自动卸载 | 记录状态不一致摘要，不输出完整路径。 |
| 新增文件由本工具安装 | manifest 标记为新增，无 backup ref | 卸载时删除该文件 | 若当前摘要不匹配则阻断 | 记录删除结果和 hash/大小摘要。 |
| 覆盖文件由本工具安装 | manifest 有 backup ref | 卸载时恢复 backup | backup 缺失或校验失败时阻断 | 记录恢复结果；backup 错误进入 Audit Log。 |
| 目标文件被外部修改 | 当前摘要与 manifest 不一致 | 标记 `repair_required` | 阻断删除/覆盖 | 给出人工处理提示，避免误删玩家或其他工具文件。 |
| manifest 丢失但疑似有写入 | task/audit 摘要显示写入过 | 不自动删除 | 阻断并提示人工确认 | 按 `DataSafetyRisk` 写入 Audit Log。 |
| `committing` 后进程中断 | manifest/status 或 task state 未完成 | 运行恢复扫描 | 阻断新安装和卸载 | 记录扫描来源和恢复建议。 |
| `rollback_required` | 失败状态未被消解 | 优先执行回滚或修复 | 阻断安装/卸载 | 回滚成功/失败均进入 Audit Log。 |
| manifest 未记录的未知文件 | 目标目录存在额外文件 | 保留 | 不把未知文件纳入卸载 | 只记录聚合摘要，避免泄露目录内容。 |

### 安装 UI 状态契约

前端只消费后端 DTO 与任务事件，不计算最终安装路径、MHW 规则、backup 路径或 manifest 路径。

| UI 状态 | 来源 | 可见行为 | 注意事项 |
| --- | --- | --- | --- |
| `idle` | 无活动任务 | 显示可安装入口或状态摘要 | 不根据本地 mock 推断已安装。 |
| `previewing` | 调用预览 command | 显示加载态 | 预览失败不启动安装。 |
| `preview_ready` | 后端返回 plan summary | 展示动作数、冲突和阻断原因 | 只展示摘要，不展示路径正文。 |
| `install_queued` | `install.queued` | 显示排队/等待写锁 | 必须匹配 `taskId`。 |
| `install_planning` | `install.plan.building` | 显示计划构建中 | 不显示 sandbox/cache 路径。 |
| `install_committing` | `install.commit.processing` | 显示安装中，允许受控取消提示 | Commit 阶段取消可能转为失败或恢复状态。 |
| `install_completed` | `install.completed` | 显示安装成功摘要 | 成功后可触发 manifest 查询刷新。 |
| `install_failed` | `install.failed` | 显示稳定错误码和可读提示 | 不展示原始错误文本中的敏感路径。 |
| `install_cancelled` | `install.cancelled` | 显示已取消 | 区分用户取消和失败回滚。 |
| `uninstall_confirming` | manifest 摘要 `installed` | 显示卸载确认和托管文件/备份计数 | 只允许单选已安装条目，不展示路径或 backup ref。 |
| `uninstall_queued` | `install.uninstall.queued` | 显示等待卸载 | 必须匹配 `taskId` 和卸载 operation。 |
| `uninstall_processing` | `install.uninstall.processing` | 显示卸载中 | 不展示目标路径、manifest 正文或 backup 路径。 |
| `uninstall_completed` | `install.uninstall.completed` | 显示卸载完成摘要 | 成功后触发 manifest 查询刷新。 |
| `uninstall_failed` | `install.uninstall.failed` | 显示稳定错误摘要 | 不在前端推断修复动作，等待后端 rich repair summary。 |
| `repair_required` | manifest 查询或恢复扫描 | 显示需要修复 | 阻断再次安装/卸载入口，直到后端状态消解。 |

UI 约束：

- 任务事件必须按 `taskId` 归属，不能因为当前页面只有一个任务就接收所有 install 事件。
- 卸载入口只能来自后端 manifest 摘要的 `installed` 状态；前端不能根据安装按钮点击、任务内存态、Mod 包内容或展示标签推断“已安装”。
- 如果页面切换、刷新或重新进入，应通过 manifest 查询恢复可展示状态，而不是依赖内存任务状态。
- Cancel 按钮只能调用受控任务取消入口；前端不自行中断文件操作或清理 staging。
- 错误展示使用稳定错误码和后端给出的安全摘要；禁止展示完整本地路径、manifest 正文、backup root 或第三方 Mod 内容。

### ARMOR_RETARGET staging 输入契约

Retarget 接入 InstallPlan 时，staging 是可丢弃的中间产物，不是事实来源。

| 输入 | 所属边界 | 说明 |
| --- | --- | --- |
| 原始导入 metadata | import / analyzer | 只读事实来源，描述原始包和可替换资产。 |
| `ReplacementBinding` | profile / game adapter | 玩家选择的“Mod 资源 -> 官方目标”结构化绑定。 |
| retarget materialized files | staging provider | 根据绑定生成的可安装文件，只能位于受控 staging root。 |
| final target relative path | game adapter / provider | 交给 InstallPlan 的最终相对目标路径，用于冲突检测。 |
| binding snapshot | manifest | 安装完成后记录本次选择，供卸载、重装和恢复判断。 |

接入规则：

- Retarget 只能写 staging，不能直接写游戏目录。
- `InstallPlan` 只消费 provider 暴露的最终相对目标路径、layer 和 source ref。
- 冲突检测基于最终目标路径，不基于原始包路径或 staging 物理路径。
- Staging 可删除、可重建；恢复和卸载必须依赖 manifest 与 backup，不依赖 staging 是否仍存在。
- MHW 的 `nativePC`、`plNNN_VVVV`、slot 解析和 catalog 归一化留在 `hmm-games-mhw` 或专属 retarget 模块，不进入通用 core 或前端。

### 测试矩阵

后续 InstallPlan PR 至少按改动范围覆盖下表中对应项。文档或纯 DTO 改动可以只跑文档/类型检查，但 PR 描述必须说明未触达真实写入链路。

| 场景 | 最小测试 | fixture 类型 | 禁止依赖 |
| --- | --- | --- | --- |
| 目标路径校验 | 单元测试覆盖空路径、绝对路径、`..`、盘符、大小写冲突 | 人工相对路径样本 | 真实游戏目录 |
| 计划冲突 | 同目标同 priority 阻断、不同 priority 排序 | fake provider | 第三方 Mod 包 |
| 安装新增文件 | 临时目录写入、manifest entry、Audit Log | temp game root | 真实 MHW 安装 |
| 覆盖并备份 | 旧文件备份、写入新文件、manifest backup ref | temp game root + fake backup root | 真实玩家文件 |
| 写入失败回滚 | 注入写入/manifest 保存失败，校验回滚结果 | fake FS 或受控临时目录 | 手动修改真实文件 |
| Manifest 查询 | 只返回摘要和状态，不返回路径/root/正文 | fake manifest repo | manifest 绝对路径 |
| 卸载新增文件 | 只删除 manifest 记录且摘要匹配的文件 | temp game root | 未记录文件 |
| 卸载覆盖文件 | backup ref 恢复旧文件，backup 缺失阻断 | fake backup store | 任意猜测恢复 |
| 恢复扫描 | `completed`、`rollback_required`、`repair_required`、`unknown`，含空 `modIds` 合并 recovery record 的全量扫描 | fake manifest、target bytes、backup store、recovery record repo | Task Log 作为唯一事实来源 |
| Retarget staging | staging containment、最终目标路径冲突、binding snapshot | fake retarget provider | 前端拼接 `nativePC` |
| 任务取消 | plan 阶段可取消、commit 阶段状态一致 | fake task manager / temp FS | 无 `taskId` 的事件 |
| 审计与脱敏 | 写入、覆盖、删除、备份、manifest、回滚事件脱敏 | 人工路径和 ID | 完整本地路径、Steam ID、token |

## 后续切片优先级

### P0：安装 UI 状态恢复与安装状态摘要

状态：已落地 MVP。当前实现会在 Mod 库加载成功和安装任务完成后调用带 `gameId` 的 `get_install_manifest_status`，并展示后端返回的摘要状态；该摘要已能消费只读 recovery scan 事实并返回 `rollback_required` / `repair_required` / `unknown` 等不安全状态。更完整的 rich manifest schema、迁移和 replacement binding snapshot 仍待后续切片。

目标：让用户在 Mod 库重新进入、刷新或安装任务结束后，能看到来自后端 manifest 摘要的安装状态，而不是只依赖页面内存里的任务事件。

范围：

- 复用已落地的最小安装任务入口和 `taskId` 事件归属逻辑。
- 在安装完成、页面重新进入或 Mod 库刷新时调用 Manifest 查询。
- 展示 `not_installed`、`installed`、`repair_required`、`unknown` 等后端摘要状态。
- 将 failed / cancelled 任务态与 manifest 查询状态区分展示。
- 安装失败时继续只展示稳定错误状态，不展示真实路径、sandbox 路径、manifest 正文或第三方 Mod 内容。

明确不做：

- 不新增卸载。
- 不实现 retarget。
- 不让前端构造 `targetPath`。
- 不在前端根据 MHW 规则判断文件是否可安装。
- 不把当前页面内存任务状态当作安装事实。

验收标准：

- 前端 typed API 不包含路径字段。
- 任务状态严格按 `taskId` 匹配。
- UI 能区分 failed、cancelled 和后端查询到的安装事实。
- 页面刷新后不会把未知状态误报为已安装。
- 前端 typecheck、lint、build 通过。
- Rust command/DTO 测试仍通过。

### P0：Manifest 查询与安装状态摘要

状态：已落地 MVP。当前 command 按 `profileId` + `modIds` 返回安装摘要，并可选接收 `gameId`。传入 `gameId` 时复用只读 recovery scan，把 `completed` 映射为 `installed`，并返回 `rollback_required` / `repair_required` / `unknown` 等不安全状态；未传 `gameId` 时保留 manifest-only fallback，只根据匹配 entry 派生 `installed` / `not_installed`。新 manifest entry 已记录 `installed_file` size/SHA-256，但该 command 不返回目标 hash、backup ref 或卸载计划。

目标：让前端能展示某个 profile / mod 的安装状态，但不暴露 manifest 文件路径或原始 manifest 正文。

范围：

- 增加后端查询服务，读取受控 manifest 仓储。
- 提供窄 Tauri command，例如按 `profileId` / `modId` 查询安装摘要。
- DTO 只返回状态、动作数量、冲突摘要、可恢复状态和必要的短 id。
- 前端展示“未安装 / 已安装 / 需要修复 / 状态未知”等摘要。

明确不做：

- 不把 manifest 文件路径返回给前端。
- 不返回完整本地路径。
- 不把 manifest 当作日志替代品。

验收标准：

- command 不接受路径参数。
- 查询失败使用稳定错误码。
- DTO 不含备份路径、manifest 路径或 sandbox/cache 路径。
- 文档同步更新 `FRONTEND_BACKEND_CONTRACT.md`。

### P1：基于 manifest 的卸载

状态：后端最小安全卸载、`start_uninstall_task` 任务入口和前端最小单选卸载 UI 已落地；rich repair summary、批量/profile 工作流和恢复扫描仍待后续切片。

目标：提供第一版安全卸载能力，删除或恢复本工具安装过的文件。

范围：

- 根据 manifest entries 计算卸载计划。
- 只对存在 `installed_file` 摘要且当前目标摘要匹配的 entries 自动执行破坏性动作。
- 对本工具新增的文件执行删除。
- 对覆盖过的文件使用 backup ref 恢复。
- 对未知或不一致状态给出阻断或修复提示。
- 写入 Audit Log。
- 前端从 manifest 摘要确认可卸载状态，启动卸载任务并展示 `install.uninstall.*` 进度。

明确不做：

- 不根据当前 Mod 包重新猜测安装过什么。
- 不删除 manifest 未记录的文件。
- 不对缺少 `installed_file` 摘要的旧 manifest 自动删除或恢复。
- 不做批量 profile 切换。
- 不在前端展示或提交 target path、backup ref/root、manifest root/path、sandbox/cache 路径或 Mod 包路径。

验收标准：

- 覆盖“新增文件卸载”“覆盖文件恢复”“backup 缺失阻断”“manifest 不一致阻断”。
- 只使用临时目录或 fake file system 测试。
- 卸载失败不会留下误导性的 completed 状态。
- 前端卸载入口只在后端 manifest 摘要为 `installed` 时可用，并在完成后刷新 manifest 摘要。

### P1：崩溃恢复扫描

状态：后端只读恢复扫描摘要、`scan_install_recovery` 窄 command、空 `modIds` 全量 profile 扫描基础、durable recovery record 消费、Mod 库加载后的前端扫描入口、Dashboard 入口健康摘要、App Frame 全局恢复告警、独立恢复中心入口、恢复中心 rich repair summary 和诊断导出联动已落地；受控恢复/回滚动作的实施计划已落地到 [安装恢复受控动作实施计划](INSTALL_RECOVERY_CONTROLLED_ACTIONS_PLAN.md)，durable recovery record 的基础模型/仓储、安装 commit 写入、只读扫描消费、只读动作预览、后端 `start_recovery_action_task` 受控回滚任务、恢复中心逐 Mod 写入型按钮、任务 UI 编排，以及受控回滚成功后的 rich manifest `rolled_back` 同步均已落地。

目标：启动或进入安装页时发现半完成安装，并给出可恢复、可重试或人工处理的明确状态。

范围：

- 扫描 durable recovery record、manifest、备份记录和任务状态摘要。
- 已能识别 `completed`、`rollback_required`、`repair_required`、`unknown` 和 `not_installed`；`rollback_required` 只能来自 durable recovery record 的 `committing` / `rollback_required` 受控状态，不能只从目录内容推断。
- 已提供后端 command 返回只读恢复摘要；空 `modIds` 可扫描当前 profile manifest 内全部已知托管 Mod，并补入只有 recovery record、尚无 manifest 的半完成安装。
- Mod 库已在加载成功后展示人工处理提示并阻断不安全安装/卸载；Dashboard 已展示 profile 级聚合健康摘要；App Frame 已提供全局恢复告警；独立恢复中心已提供入口、逐 Mod 安全状态摘要、rich repair summary、完整支持诊断包导出联动和逐 Mod 受控回滚动作。

明确不做：

- 不自动删除未知文件。
- 不依赖当前 Mod 包内容猜测恢复动作。
- 不把 Task Log 当作唯一事实来源。

验收标准：

- 恢复判断来源清晰：manifest、backup、task state、审计摘要。
- 无法安全恢复时阻断并提示人工处理。
- 不输出本地真实路径。

### P1：ARMOR_RETARGET staging 接入

目标：让 retarget materialize 产物作为受控 provider 输入 `InstallPlan`，而不是绕过安装链路。

范围：

- retarget 只写 staging，不写游戏目录。
- `InstallPlan` 看到的是 retarget 后最终目标相对路径。
- 冲突检测基于最终目标路径。
- manifest 记录必要的 replacement binding snapshot。

明确不做：

- 不把 MHW slot parsing 放进通用 core。
- 不把 retarget 产物当成事实来源。
- 不让前端拼接 `nativePC` 或 `plNNN_VVVV` 路径。

验收标准：

- 原始导入包保持只读。
- staging 可丢弃、可重建。
- 事实来源仍是原始包 metadata、ReplacementBinding 和 InstallManifest。

### P2：Rich manifest 与状态机

目标：把当前 MVP manifest 扩展为可支撑卸载、恢复、修复、retarget 和后续虚拟映射的事实记录。

状态：domain 字段、JSON 向后兼容基础、`manifest_id` 与 schema/migration metadata、真实 `plan_hash` 计算、受控 `rollback_install` 成功后的 rich manifest `rolled_back` 同步，以及读侧状态机消费规则（状态摘要查询 fallback 与恢复扫描消费 profile 级 manifest status）已落地；replacement binding snapshot、写侧状态机门禁和修复检测仍待后续切片。

候选字段：

- `manifest_id`
- `game_id`
- `game_instance_id`
- `profile_id`
- `mod_id`
- `backend`
- `status`
- `created_at`
- `completed_at`
- `files`
- `backups`
- `replacement_bindings`
- `plan_hash`

候选状态：

- `planned`
- `committing`
- `completed`
- `rollback_required`
- `rolled_back`
- `repair_required`

验收标准：

- 旧 manifest 能被迁移或兼容读取。
- 状态变更有测试覆盖。
- 失败状态不会被误报为已完成。
- 安装提交成功写出的 rich metadata 不暴露 target path、backup root、manifest path、sandbox/cache 路径或第三方 Mod 内容。

### P2：Dependency / preflight

目标：安装提交前检查必需前置、风险文件、loader 要求和 profile 冲突。

范围：

- 后端基于已导入 metadata、adapter 规则和 manifest 摘要判断。
- 缺失必需前置时阻断安装。
- 可选前置或弱风险给出警告。
- 前端只展示后端给出的结构化结果。

明确不做：

- 不让前端自行匹配依赖。
- 不根据展示名直接判定已安装。
- 不把 dependency graph 查询结果升级成安装事实。

验收标准：

- 必需前置缺失可阻断。
- warning 和 blocking conflict 明确区分。
- 文档同步说明错误码和 UI 行为。

## 文件边界

后续切片应优先保持以下边界：

- `src-tauri/crates/hmm-core/src/install.rs`：领域模型、目标路径校验、冲突规则。
- `src-tauri/crates/hmm-app/src/install.rs`：计划生成、提交编排、manifest/backup/rollback 用例。
- `src-tauri/crates/hmm-app/src/install_task.rs`：安装任务、阶段事件、写锁、审计编排。
- `src-tauri/crates/hmm-ports/src/install.rs`：安装 source reader、game filesystem、backup store、manifest repository trait。
- `src-tauri/crates/hmm-infra/src/install_commit.rs`：文件系统实现和 root containment。
- `src-tauri/src/install_commands.rs`：窄 Tauri command 和 DTO 映射。
- `src/features/mods/`：Mod 管理 feature-local typed API 和 UI 状态展示。

不应新增的捷径：

- `copy_file` / `delete_path` / `write_any_file` 这类宽泛 Tauri command。
- 前端传入 `targetPath`、game root、backup root、manifest root 或 sandbox/cache 路径。
- 通用 core 识别 MHW 专属路径语义。
- 在 install executor 之外写入、覆盖或删除游戏目录文件。

## 安全门禁

每个切片都必须确认：

- 是否可能触碰真实游戏目录写入。
- 是否可能覆盖、删除或恢复文件。
- 是否影响 manifest、backup 或 rollback。
- 是否影响 task event、Audit Log 或错误码。
- 是否影响 frontend/backend contract。
- 是否涉及 retarget、staging 或 MHW adapter 规则。

只要涉及真实写入、卸载、恢复、retarget staging 或 manifest 状态机，就必须补充聚焦测试；无法测试时必须说明原因和风险边界。

## 验证要求

文档改动最小验证：

```powershell
git diff --check
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\check-whitespace.ps1
```

涉及 Rust 安装链路：

```powershell
cargo test --workspace
cargo check --workspace
```

涉及前端 API 或 UI：

```powershell
cmd /c corepack pnpm run typecheck
cmd /c corepack pnpm run lint
cmd /c corepack pnpm run build
```

最终交付前优先执行：

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify.ps1
```

## PR 描述建议

涉及 InstallPlan 的 PR 至少说明：

- 本 PR 对应本文档哪个切片。
- 是否触碰真实游戏目录写入。
- 是否改变 command / DTO / task phase / error code。
- 是否改变 manifest、backup、rollback 或 Audit Log。
- 已执行哪些验证。
- 哪些能力仍明确不做。
