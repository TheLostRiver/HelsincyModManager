# HelsincyModManager 后续任务总纲

创建时间：2026-06-27
基于 HEAD：`7b1f9be` (main)
最近同步：2026-07-26，基于 `0439f70`（T17 Slice 1/2/3/4A/4B 已合并；Slice 4C 结果、重试、性能与最终加固由 PR #199 交付）

---

## 文档结构说明

本文件是**任务总纲**，所有后续工作的优先级、依赖关系和状态都在此追踪。

### 哪些任务需要独立文档？

| 条件 | 是否需要独立文档 | 放在哪里 |
|------|-----------------|---------|
| 涉及架构决策、多种方案需要取舍 | 需要 | `docs/<TOPIC>_DECISION.md` |
| 跨多模块、多 PR 的大型 feature | 需要 | `docs/<FEATURE>_DESIGN.md` |
| 单一切片、边界清晰、一次 PR 可完成 | 不需要 | 本文件中的任务条目足够 |
| 纯 UI 工作、样式调整 | 不需要 | 本文件条目 + 参考图即可 |

### 已有独立文档的任务

- ARMOR_RETARGET: `docs/ARMOR_RETARGET_DESIGN.md` + `docs/ARMOR_RETARGET_IMPLEMENTATION.md` + `docs/ARMOR_RETARGET_REVIEW.md`
- InstallPlan MVP: `docs/INSTALL_PLAN_STATUS.md` + `docs/INSTALL_PLAN_MVP_TODO.md`
- 核心 Mod 生命周期优先级: `docs/CORE_MOD_LIFECYCLE_PRIORITY_PLAN.md`
- 核心 Mod 生命周期产品化加固: `docs/CORE_MOD_LIFECYCLE_PRODUCTIZATION_PLAN.md`
- 恢复受控动作: `docs/INSTALL_RECOVERY_CONTROLLED_ACTIONS_PLAN.md`
- Mod 预览图: `docs/MOD_PREVIEW_IMAGE_PIPELINE_DESIGN.md` + `docs/MOD_PREVIEW_IMAGE_IMPLEMENTATION_PLAN.md`
- 第三方 Mod 管理器批量迁移: `docs/EXTERNAL_MOD_MANAGER_BATCH_IMPORT_DESIGN.md`
- Mod 库分页: `docs/MOD_LIBRARY_PAGINATION_DESIGN.md`
- 自主迭代任务队列: `docs/AUTONOMOUS_ITERATION_ROADMAP.md`
- T8 存档目录自动发现: `docs/SAVE_DIRECTORY_AUTO_DISCOVERY_DESIGN.md` + `docs/superpowers/plans/2026-07-05-save-directory-auto-discovery-implementation.md`

### 需要新建独立文档的任务

- ~~**T2 持久化方案**: → `docs/PERSISTENCE_DECISION.md` — SQLite vs JSON 架构决策~~ ✅ 已创建
- ~~**T8 存档备份**: → `docs/SAVE_BACKUP_DESIGN.md` — 涉及定时调度、保留策略等设计~~ ✅ 已创建，首个实现切片建议为手动备份 MVP

---

## 优先级定义

| 级别 | 含义 | 节奏 |
|------|------|------|
| **P0** | 关键路径 — 不做会阻塞后续多个 feature | 立即推进 |
| **P1** | 核心 MVP — 用户可感知的必要能力 | P0 完成后紧接 |
| **P2** | 重要增强 — 提升完整度和用户体验 | P1 基本就绪后推进 |
| **P3** | 长线 feature — Phase 4+ 的大型功能 | 按 Roadmap 节奏 |

### 当前执行覆盖规则（2026-07-30）

[核心 Mod 生命周期优先级计划](docs/CORE_MOD_LIFECYCLE_PRIORITY_PLAN.md) 已完成 Gate A/B 优先级覆盖
目标，但不覆盖架构和玩家数据安全规则：

1. P0：安装/卸载/真正重装闭环已通过 Gate A `certified`。
2. P1：ARMOR_RETARGET 最窄纵向闭环已通过 Gate B `certified`。
3. T9/T10 的 Gate A/B 最小 manifest/preflight 子集已经落地，不在本次认证后自动扩张。
4. Gate B 后优先级复审选择的 T19“核心 Mod 生命周期产品化加固”已完成；T18 Mod 库分页已由
   PR #192 完成最后的 Slice 4C rebase 合并。T17 Slice 1/2/3/4A/4B 已合并，PR #199 交付最后的
   Slice 4C；完整 T17 已具备分页结果、partial success、服务端重试和大批次门禁。
5. 2026-07-30 优先级复审已把 T13 恢复为 P0，但仍与 T17 正交：先执行 T13-00 独立设计与安全评审，
   再按 T13-01 至 T13-08 推进，不能把 T17 import-only 编排当成批量安装实现。

---

## 任务依赖图

```text
已完成安全基础（T1-T7 + InstallPlan/manifest/backup/rollback MVP）
  -> CL0 生命周期验收基线
  -> CL1 install/uninstall 自动化纵向闭环
  -> CL2 桌面 smoke
  -> CL3 真正 reinstall
  -> Gate A: Core Mod Lifecycle certified
  -> T11 ARMOR_RETARGET 最窄纵向切片
       ├──> T9 最小 replacement binding snapshot / write-state gate
       └──> T10 最小 path-family / source / target preflight
  -> Gate B: ARMOR install/switch-target/uninstall certified
  -> T19 产品化加固 [completed]
  -> T18 Mod 库分页 Slice 1 [completed, PR #186]
  -> T18 Mod 库分页 Slice 2 [completed, PR #187]
  -> T18 Mod 库分页 Slice 3 [completed, merged]
  -> T18 Mod 库分页 Slice 4A [completed, PR #190]
  -> T18 Mod 库分页 Slice 4B [completed, PR #191]
  -> T18 Mod 库分页 Slice 4C [completed, PR #192]
  -> T17 第三方批量迁移 Slice 1 [completed]
  -> T17 Slice 2 只读扫描与分页预览 [completed, PR #194]
  -> T17 Slice 3 安全物化与批量导入编排 [completed, PR #195]
  -> T17 Slice 4A 外部来源与只读预览 [completed, PR #196；PR #197 补齐 review 遗漏]
  -> T17 Slice 4B selection/decision/start/progress [completed, PR #198]
  -> T17 Slice 4C result/retry/performance/hardening [completed, PR #199]
  -> T13-00 批量语义设计 [P0 ready]
  -> CLI-2A/2B/2C Sandbox 自动化基础
  -> CORE-PREF-01 单项 preflight 一致化
  -> T13-01..08 批量安装/卸载/真正重装与 Gate C
```

---

## P0 — 关键路径

### Core Mod Lifecycle Gate A

**状态**: CL0-CL4 已完成；Gate A 已于 2026-07-15 标记为 `certified`
**独立文档**: `docs/CORE_MOD_LIFECYCLE_PRIORITY_PLAN.md` + `docs/CORE_MOD_LIFECYCLE_CL0_ACCEPTANCE.md`

- [x] CL0：固定 `v1/v2` 人工 fixture、test-only AppState import/plan/restart harness、acceptance matrix、桌面 smoke 文档和缺口清单
- [x] CL1：认证导入记录 -> InstallPlan -> install -> restart -> uninstall -> baseline 自动化闭环
- [x] CL2：实际 Tauri 桌面 smoke、状态恢复、错误脱敏和清理证明
- [x] CL3：独立真正重装 use case，覆盖 retained/replaced/added/stale 与失败恢复
  - [x] Task 1-9：classifier、revision catalog、manifest/recovery、preview/commit/task/DTO/UI 与 L2 AppState 闭环
  - [x] Task 10：Windows Sandbox L3、诊断脱敏、containment cleanup 与 CL3 closeout
- [x] CL4：Gate A 本地 review、完整验证和 `certified` 状态记录

Gate A、Gate B、T19、T18 与 T17 均已完成；P7.2c、批量操作、任务队列和新的非阻断视觉工作仍按各自恢复门禁评审，
不因前置能力完成自动开工。

---

### T19: 核心 Mod 生命周期产品化加固

**优先级**: P0 发布加固，已完成
**前置**: Gate A/B certified
**状态**: 已完成（2026-07-18；A1-L3 七切片均经独立 review 合并，完成定义全部满足）
**预估**: 大，固定拆为 7 个独立 review 切片
**独立文档**: **已创建** -> `docs/CORE_MOD_LIFECYCLE_PRODUCTIZATION_PLAN.md`

范围:
- [x] 规划 Acceptance、Logging/Diagnostics、Feedback UI 三条轨道及共享安全边界
- [x] A1：固化不少于 6 个 `headless_composition_*` 场景的正式验收脚本、非零断言和 CI 入口
- [x] L1：安全结构化事件、脱敏、App Log JSONL writer、UTC 日轮转、14 天保留和稳定健康退化码
- [x] U1：共享 Dialog/Detail Sheet/Task Notice/Toast 基元，首个迁移游戏目录 Dialog
- [x] U2：安装计划 Sheet、卸载 Modal、按 `taskId` 的 Task Notice、durable refresh 后的完成/普通失败 Toast
- [x] L2：Task Log、Audit 写入失败显式策略和诊断健康摘要
- [x] U3：导入、游戏发现、Profile、备份、诊断导出等跨 feature 短时通知迁移
- [x] L3：只读日志/诊断页面和受控导出入口

硬边界:
- 不重新打开 Gate A/B，不重写 InstallPlan/manifest/backup/rollback/recovery/retarget
- 破坏性动作和最终安装事实仍由后端及 manifest/recovery 驱动；前端只消费稳定 DTO/phase/code
- 日志/诊断默认脱敏，不记录完整路径、用户名、Steam ID、token、存档或第三方 Mod 内容
- 安全风险和恢复阻断保持持久告警，不降级为自动消失 Toast
- 每个切片独立 PR；U2 未提前实现 L2、U3、L3 或其他后续切片

---

### T1: 恢复中心写入型动作 UI 启用

**状态**: 已完成（后端任务、前端 typed API、恢复中心逐 Mod 受控回滚 UI、任务事件跟踪和完成后重新扫描均已落地）
**预估**: 已交付
**独立文档**: 不需要（`docs/INSTALL_RECOVERY_CONTROLLED_ACTIONS_PLAN.md` 已覆盖）

已就绪的后端基础:
- `start_recovery_action_task` Tauri command
- `preview_recovery_action` 只读预览
- `install.recovery.*` task phase
- 前端 `startRecoveryActionTask` / `previewRecoveryAction` typed API
- 恢复中心人工处理决策面板在存在 `rollback_required` Mod 时提供 `controlled_recovery` 入口，并引导用户到逐 Mod 受控回滚按钮

本切片交付:
- [x] 恢复中心决策面板启用 `rollback_install` 按钮
- [x] 点击前调用 `previewRecoveryAction` 判断可执行性
- [x] `available` 时启用确认流程，`blocked` 时展示阻断 reason code
- [x] 确认后调用 `startRecoveryActionTask` 启动后端受控回滚
- [x] 按 `taskId` 订阅 `install.recovery.*` phase，展示排队/执行中/完成/失败
- [x] 完成后触发恢复中心重新扫描 + 刷新 profile 摘要和全局告警

安全边界:
- 前端只提交 `gameId`、`profileId`、`modId`、`actionKind`
- 不展示 target path、backup ref/root、manifest 正文
- 按钮只在 `rollback_required` 状态 Mod 上可见

---

### T2: 持久化方案决策与实施

**状态**: 已完成（决策文档、SQLite 基础设施、migration 001、Mod metadata / categories / mod_categories 基线已落地）
**预估**: 已交付；后续 Profile / ReplacementBinding 继续追加 SQLite migration
**独立文档**: **需要** → `docs/PERSISTENCE_DECISION.md`

当前 JSON 持久化:

| 仓储 | 实现 | 够用? |
|------|------|-------|
| `GameConfigRepository` | JSON 单文件 | 够 |
| `InstallManifestRepository` | JSON per profile | MVP 够 |
| `InstallRecoveryRecordRepository` | JSON per record | 够 |
| `AppSettingsRepository` | JSON 单文件 | 够 |
| Mod 导入结果 | JSON per mod | 只读够 |

JSON 做不好的需求:

| 需求 | 困难点 |
|------|--------|
| Mod 元数据编辑（overlay） | 需更新导入快照或新建层 |
| 分类标签多对多 | 关系查询困难 |
| Profile CRUD + 关联 | 关系查询 |
| ReplacementBinding | 关联 Mod + Profile + Target |
| 备份历史 | 带时间线的列表查询 |

两条路线:

| | 路线 A: 继续 JSON + overlay | 路线 B: 引入 SQLite |
|---|---|---|
| 优点 | 无新依赖，迁移成本低 | 关系模型天然适合，事务保障，查询灵活 |
| 缺点 | 分类/Profile 实现复杂，数据量大时性能差 | 新增依赖，学习成本 |
| 建议 | | **推荐**，渐进引入 |

建议策略: SQLite 只管"用户可编辑数据"（元数据 overlay、分类、Profile、绑定），安装链路的 manifest/recovery record 暂保留 JSON（已验证稳定）。

交付:
- [x] 决策文档：范围、依赖选型（rusqlite vs sqlx）、迁移策略
- [x] `hmm-infra` 引入 SQLite + migration 基础设施
- [x] 初始 schema：`mod_metadata`、`categories`、`mod_categories`
- [x] `hmm-ports` 新增 `ModMetadataRepository` trait
- [x] `hmm-infra` 实现 SQLite 版仓储
- [x] 不改变现有 JSON 仓储，两套共存

---

## P1 — 核心 MVP 功能

### T3: Mod 元数据更新后端

**前置**: T2
**状态**: 已完成（Mod metadata overlay 后端、Tauri commands、前端 typed API、`get_mod_library` overlay 合并已落地）
**预估**: 已交付
**独立文档**: 不需要

交付:
- [x] `hmm-core` 定义 `ModMetadata` 可编辑字段（名称、作者、版本、备注、nexus_mod_id）
- [x] `hmm-ports` 新增 `ModMetadataRepository` trait（read/update/delete overlay）
- [x] `hmm-app` 新增 `ModMetadataService`（读取导入快照 + 合并用户 overlay）
- [x] `hmm-tauri` 新增 `update_mod_metadata` / `delete_mod_metadata` 窄 command
- [x] `get_mod_library` 返回合并后数据（用户编辑值优先）

---

### T4: 分类标签 CRUD

**前置**: T2
**状态**: 已完成（分类 CRUD 后端、Tauri commands、前端 typed API、分类合并逻辑、分类管理页面与管理工作台 UI 重构均已落地）
**预估**: 已交付
**独立文档**: schema 包含在 T2 决策文档中

交付:
- [x] 领域模型：`Category`（id, name, color?, sort_order）和 `CategoryLabel`
- [x] 多对多：Mod ↔ Category
- [x] `hmm-ports` + `hmm-app` + `hmm-tauri` CRUD command
- [x] 前端 typed API
- [x] `get_mod_library` 返回真实分类标签
- [x] `/categories` 分类管理页面（新建/编辑/删除/悬浮色板/review follow-up、搜索/排序/批量管理、悬浮新建面板）

---

### T5: Mod 信息编辑面板 (前端)

**前置**: T3 + T4
**状态**: 已完成（Mod 详情悬浮面板、信息编辑、分类多选、右键入口与保存刷新已落地）
**预估**: 已交付
**独立文档**: 不需要（参考图由维护者在任务上下文中提供）

> 注意：`docs/CATEGORY_MANAGEMENT_TODO.md` 曾把“分类管理页面”称为分类专题的 T5 当前切片；该页面已完成。本文档中的 T5 始终指 Mod 信息编辑面板。

设计要点:
- 统一悬浮对话框，合并参考图两个面板
- 当前包含"信息编辑"section；ARMOR_RETARGET 就绪后追加"替换目标"section
- 视觉远超参考图水平

交付:
- [x] `ModDetailDialog.tsx` — 悬浮对话框
- [x] 信息编辑：名称、作者、版本、备注、NexusMods ID
- [x] 分类选择：多选 chip（消费 T4 数据）
- [x] 预览图展示（只读）
- [x] 右键菜单 `info-settings` 打开此面板
- [x] 右键菜单 `edit-files` 暂显示"功能开发中"
- [x] 保存后刷新 Mod 库

---

### T6: Profile 基础管理

**前置**: T2
**状态**: 已完成（后端基础、前端 Profile 管理 UI、存档设置控制台重做与 active profile 接入均已落地）
**预估**: 中-大
**独立文档**: 建议（语义较多）

交付:
- [x] 领域模型：`Profile`（id, name, description, created_at, updated_at, is_active）
- [x] SQLite migration：`profiles` 表 + `default` active profile
- [x] 后端 CRUD + 切换活跃 profile
- [x] Tauri commands + `ProfileDto` 契约
- [x] 前端 Profile 列表、创建、切换、删除
- [x] App Shell 展示当前活跃 profile
- [x] 安装/卸载/恢复操作使用活跃 profile 而非硬编码 `"default"`
- [x] Profile 存档设置控制台视觉重做，包含目录配置、自动备份策略和只读备份历史预览

---

### T7: 一键启动游戏

**前置**: 无
**状态**: 已完成（`GameLauncher` port、MHW:I Steam protocol launcher、`launch_game` command、Dashboard / App Header 启动入口已落地）
**预估**: 小
**独立文档**: 不需要

交付:
- [x] `hmm-ports` 新增 `GameLauncher` trait
- [x] `hmm-games-mhw` 实现 MHW:I launcher（Steam protocol 优先）
- [x] `hmm-tauri` 新增 `launch_game` command
- [x] Dashboard / App Header 启动按钮

---

## P2 — 重要增强（Gate B 前暂停或受限）

### T8: 存档备份系统

**前置**: T2
**状态**: 已完成部分保留；P7.2a 安装态 acceptance、P7.2c 实现、retention 扩展和备份中心在 T19
完成后仍按各自发布门禁评审，当前未恢复
**预估**: 大
**独立文档**: **已创建** → `docs/SAVE_BACKUP_DESIGN.md`、`docs/SAVE_BACKUP_BACKGROUND_AUTOMATION_DESIGN.md`、`docs/SAVE_DIRECTORY_AUTO_DISCOVERY_DESIGN.md`、`docs/superpowers/plans/2026-07-05-save-directory-auto-discovery-implementation.md`、`docs/superpowers/specs/2026-07-12-save-backup-installer-cleanup-design.md`、`docs/superpowers/plans/2026-07-12-save-backup-installer-cleanup-implementation.md`

概要:
- [x] 手动备份后端 MVP（`start_save_backup_task` / `list_save_backups`、zip + manifest、SQLite 历史、任务事件、最小审计）
- [x] Profile 页面接入手动备份按钮、任务进度和历史刷新
- [x] 存档目录自动发现：后端扫描 MHW:I Steam userdata，唯一高置信候选自动写入，多 Steam 用户候选默认推荐最近修改项但必须确认，Profile UI 和启动自检悬浮提示已接入
- [x] 客户端运行期自动备份（可配置间隔、持久化调度状态、scheduler lease 去重、自动任务触发）
- [x] 自动备份游戏运行保护（运行中或状态未知时延后，不获取 lease、不启动备份任务）
- [x] P7.1 headless worker 基础（固定 `--once` 入口、共享 scheduler/备份链路、heartbeat 与 fallback registry；当前仍为 `tray_only`）
- [x] P7.2a Windows 平台核心（用户级 Scheduled Task 注册/更新/移除、read-back、独立 heartbeat、双条件 `protected` 派生和 sidecar；自动化仅使用 fake/临时依赖）
- [ ] P7.2a Windows 安装态 runtime acceptance（暂停；T19 完成后仍按发布风险另行评审）
- [x] P7.2b Settings 全局后台保障开关、Profile 只读状态、5 分钟 `starting` / 45 分钟 `protected` 健康派生与统一退出保护
- [x] P7.2c NSIS/WiX owned Scheduled Task 卸载 cleanup 设计规格与实施计划（本项仅代表 docs 完成）
- [ ] P7.2c helper、NSIS PREUNINSTALL、WiX custom action 与 disposable VM gate（已规划、暂停；不得删除 foreign task）
- [x] 保留策略（数量）
- [ ] 保留策略（时间/空间，暂停）
- [x] 备份目录可选择（未手动选择时使用默认 app data）
- [ ] 前端 `features/backups/` 页面（暂停）

---

### T9: Rich Manifest 与状态机

**前置**: T1
**状态**: 已有基础保留；Gate B binding snapshot 已落地，其他仅允许 Gate A/B 直接阻断的最小子集
**预估**: 中
**独立文档**: 不需要（`docs/INSTALL_PLAN_MVP_TODO.md` 已有设计）

概要:
- [x] 已落地：`manifest_id`、`schema_version` / `schema_migration`、`backend`、`status`、`created_at`、`completed_at`、`plan_hash` JSON 兼容层；安装提交成功会写入 schema metadata、`backend`、完成时间和真实 `plan_hash`
- [x] Gate B 必需：replacement binding snapshot
- [ ] 延后：与 Gate A/Gate B 无关的 `game_id` / `game_instance_id` / 顶层 `mod_id` 泛化
- [x] `get_install_manifest_status` 消费 recovery scan
- [x] rich manifest 读侧状态机消费规则：`InstallManifestStatus::consumption()` 分类（completed/rolled_back→信任 entries，planned/committing→unknown，rollback_required/repair_required→对应失败态），manifest 状态摘要查询 fallback 与恢复扫描均已消费；写侧门禁另行切片
- [x] `rolled_back` 状态持久化：受控 `rollback_install` 成功后同步 rich manifest status，并移除已回滚 Mod 的 stale entries

---

### T10: 前置依赖检查

**前置**: T3 + T4
**状态**: 受限；只做 Gate A/Gate B 的 source/target/path-family/冲突阻断，通用依赖平台延后
**预估**: 中
**独立文档**: 不需要

概要:
- [ ] 依赖规则 catalog（JSON/TOML 随 `hmm-games-mhw` 发布）
- [ ] 安装前检查 + 阻断/警告
- [ ] 前端展示依赖检查结果

---

### T13: 批量操作

**前置**: T6 + Gate A certified + CLI-2 Sandbox 写基础 + 单项 preflight 一致化
**状态**: P0 待实施；按 T13-00 至 T13-08 独立 task 推进
**预估**: 大
**独立文档**: **待创建**；任务边界见 `docs/AUTONOMOUS_ITERATION_ROADMAP.md`

概要:
- [ ] T13-00：sealed input、跨 Mod conflict、失败/取消/partial/retry 领域语义
- [ ] T13-01：服务端 `BatchPlan`、digest、资源预算与只读预览
- [ ] T13-02：批量安装，每个 Mod 独立事务，默认首次失败停止
- [ ] T13-03：manifest/recovery 驱动的批量卸载
- [ ] T13-04：复用真正重装事务的批量重装与 Armor target switch
- [ ] T13-05：CLI Sandbox 批量 JSON/JSONL contract
- [ ] T13-06：窄 Tauri commands、DTO 和 feature-local typed API
- [ ] T13-07：前端多选、预览、确认、进度、结果和 retry
- [ ] T13-08：disposable Windows Sandbox Gate C 纵向验收

硬边界:

- 不能由 CLI、Tauri 或前端循环调用单项 command 来冒充批量用例。
- 同一 game/profile 写入严格串行；每项继续走
  `InstallPlan -> preflight -> backup -> commit -> manifest -> rollback/recovery`。
- 不宣称整批全局原子；已成功项保留真实成功事实，partial result 必须明确。
- 取消只停止启动新项；正在 commit 的项目在安全点完成一致性收尾。
- retry 只消费 sealed batch 中 retryable 项，成功项不重复提交。

当前前端状态（T13 开工时需一并恢复）:

「启用全部 MOD」「禁用全部 MOD」两个按钮曾常驻快捷操作栏，但无条件返回「暂不可用」，
且 `handleAction` 里没有对应 case —— 任何状态下都点不动，只是常驻噪音，已移除。
T13 落地批量能力时连同批量语义一起重新加入，不要只把按钮加回来。

单选约束仍在：所有生命周期操作在选中数大于 1 时禁用。`选择本页`/`反选本页` 仍保留
（它们确实改变选择状态），但在批量能力就绪前，多选状态没有任何操作可消费；
禁用文案已改为说明这是暂时限制而非产品规则。

`compactActionAvailability.test.mjs` 有一条断言：**凡是由 `getCompactActionDisabledReason`
判定可用性的动作**，在完全就绪状态下都必须可用，防止再次引入「永远点不动」的占位按钮。
T13 新增批量按钮时该断言会强制它们真正可用。

该断言不覆盖 `add` / `add-revision`：这两个的可用性由 `ModImportAction` 自行判定，不走上述函数。
同一条测试另有一组显式存在性断言，锁定 `select-all` / `invert` 等动作不被误删——
只遍历「已存在的动作」的话，删掉某个动作只会让循环少跑一轮而静默通过。

---

### T17: 第三方 Mod 管理器批量迁移（狩技盒子兼容）

**前置**: 单包安全导入链路 + TaskManager/取消 + Mod 导入结果持久化
**状态**: 已完成；Slice 1/2/3/4A/4B 已合并，Slice 4C 由 PR #199 交付
**预估**: 大，按 6 个独立 review 切片推进（Slice 4 拆为 4A/4B/4C）
**独立文档**: **已创建** → `docs/EXTERNAL_MOD_MANAGER_BATCH_IMPORT_DESIGN.md`

范围:
- [x] 设计来源 adapter、批次预览、去重/冲突、取消/重试和安全边界
- [x] Slice 1：无路径领域/selection 契约、批次选择上限、ports、仓储基准/决策和人工 fixtures（已完成）
- [x] Slice 2：`hunting_box_directory_v1` 只读扫描、内容指纹、分页预览和 scan task（PR #194 已合并）
- [x] Slice 3：安全物化、复用单包导入链路、partial success、幂等和恢复对账（PR #195 已合并）
- [x] Slice 4A：来源选择、scan task 状态和只读分页预览（PR #196；PR #197 补齐 review 遗漏）
- [x] Slice 4B：候选选择/服务端全选、分类映射、冲突决定、sealed batch start 与严格按 `taskId` 的导入进度（PR #198）
- [x] Slice 4C：权威分页结果、partial success、sealed selection 重试、新 taskId 进度复用、10,000 条人工性能门禁与最终加固（PR #199）

硬边界:
- 默认只导入，不自动安装、启用或写游戏目录
- 不读取第三方数据库/账号，不同步启用状态、优先级或安装事实
- 前端不接触来源路径、XML 解析、hash、sandbox/cache 或去重规则
- 与 T13 的批量安装/卸载队列正交，不能借批量导入绕过 `InstallPlan` / manifest / backup / rollback
- 自动化只用临时目录与人工 fixture，不提交或读取真实第三方 Mod
- 当前排期仅覆盖 Windows + MHW:I；Linux/Steam Deck、Rise/Wilds 和更多游戏适配不纳入 T17 工期

---

### T18: Mod 库分页

**前置**: 现有 Mod 库 + Profile install/recovery 状态查询
**状态**: 已完成；Slice 1/2 已完成（PR #186/#187），Slice 3 已合并，Slice 4A 已完成（PR #190），Slice 4B 已完成（PR #191），Slice 4C 已由 PR #192 rebase 合并
**预估**: 中-大，建议拆为 6 个独立 review 切片（Slice 4 拆为 4A/4B/4C）
**独立文档**: **已创建** → `docs/MOD_LIBRARY_PAGINATION_DESIGN.md`

范围:
- [x] 设计后端权威查询分页、搜索/filter/稳定排序、page-local selection 和 UI 状态边界
- [x] Slice 1：app-level query/filter/sort/page 类型、兼容聚合服务和 fake repository 测试（PR #186）
- [x] Slice 2：`query_mod_library` Tauri DTO、稳定错误、feature-local typed API 和 contract 文档（PR #187）
- [x] Slice 3：数字分页 footer、250ms debounce/latest-request gate、loading/error/empty、本页选择和当前页 durable overlay；本地统一验证、四视图/四窗口视觉 smoke、独立复审与合并已完成
- [x] Slice 4A：1,000/10,000 条人工读路径基准、JSON provenance + SQLite projection 决策、Unicode/profile status 策略（PR #190）
- [x] Slice 4B：projection schema/rebuild、ports、infra writer 与 T17 批量写入协调（PR #191）
- [x] Slice 4C：生产 query switch、同事务 count/page、fail-closed freshness tracking、固定 10,000 条性能门禁与回归（PR #192）

关键语义:
- 默认每页 24，可选 12/24/48/96；使用 1-based 数字页
- 搜索、分类/状态筛选、稳定排序、总数和 page 必须由同一后端查询快照决定
- 首版只提供“选择本页/反选本页”；跨页全选和批量写操作留给 T13
- T18 不依赖 T17，但应在 T17 Slice 4 完整迁移 UI 对外完成前落地
- 当前 JSON 全量读取只允许作为兼容阶段，不能把 bridge payload 变小等同于大库性能完成
- Slice 1/2 已完成 app-level 查询服务与 Tauri typed contract；Slice 3 已把页面消费者迁移到
  当前页查询和 durable overlay，并已合并
- Slice 4A 只处理可重复基准与持久化决策；4B/4C 完成 projection、生产切换和性能门禁，T18 已随 PR #192 合并完成

---

## P1 核心差异能力 / P3 长线 Feature

### T11: ARMOR_RETARGET 全链路

**优先级**: P1，Gate A 通过后立即开始
**状态**: AR1-AR5 已完成；修复当前 target 呈现缺陷后的最终 artifact 已通过全新 Sandbox 纵向复验，Gate B 已 `certified`
**前置**: Gate A certified + T9/T10 最小直接前置 + InstallPlan staging
**预估**: 大（12 Task，3-5 个 PR）
**独立文档**: **已有** → `docs/ARMOR_RETARGET_IMPLEMENTATION.md`

执行顺序（遵循已有实施计划）:
- [x] AR1：Core replacement identity / binding / versioned catalog 领域模型
- [x] AR1：只读 Replacement catalog list/find/search ports
- [x] AR1：MHW armor catalog + Unicode/search normalization
- [x] AR2：MHW armor path parser 与单 source analyzer
- [x] AR2：MHW RetargetPlan builder
- [x] AR3：Application replacement service 与 staging materialize
- [x] AR3：Manifest + InstallPlan + binding snapshot 集成
- [x] AR4：Tauri commands / DTO 与前端 typed API / 受控 UI
- [x] AR4：Windows Sandbox 首次 retarget 安装纵向验收（source analyze -> target select -> preview -> install -> restart recovery）
- [x] AR5：真正重装 target switch、同 revision binding/entry 原子替换、重启恢复与 manifest 卸载自动化
- [x] AR5：已安装状态的 target switch preview/confirm/taskId/cancel 受控 UI
- [x] AR5：首个 Sandbox artifact 完成首次 retarget -> switch target -> restart -> uninstall -> 精确 baseline 文件闭环
- [x] AR5：修复重启后当前 installed target 的窄 DTO/UI 呈现与同目标切换阻断
- [x] Gate B：最终 artifact 在全新 disposable Windows Sandbox 重验完整闭环并标记 `certified`

---

### T12: Mod 详情统一面板 (完整版)

**前置**: T5 + T11 部分就绪
**状态**: 完整版暂停；Gate B 所需首次 target 选择与已安装 target switch UI 已由 AR4/AR5 实现
**预估**: 中
**独立文档**: 不需要

概要:
- [x] 在 T5 基础上追加"替换目标"tab
- [x] 展示源槽位 + armor catalog + 搜索筛选
- [x] 选择目标后预览 retarget plan
- [x] 右键 `edit-files` 打开此 tab

---

### T14: 任务队列 UI

**前置**: T13
**状态**: 暂停；T13 恢复且出现真实多任务需求后再开始
**预估**: 小-中
**独立文档**: 不需要

---

### T20: 浮层进出场动画收敛到共享基元

**前置**: 无
**状态**: 待评审；下次新增浮层前处理，或出现第三处重复实现时立即处理
**预估**: 中（属重构，会触及已稳定的模态框 / Sheet 链路）
**独立文档**: 不需要

背景:

`src/shared/feedback/` 已有 `Dialog` / `ModalSurface` / `FeedbackPortal` 等基元，但 `ModDetailDialog`
自带 `createPortal` 与整套样式，没有走这些基元。结果是"浮层进出场动画"在项目里有两套彼此独立的实现。

退场动画无法由纯 CSS 完成（React 卸载后节点已不存在），必须由组件实现
"先标记退场 → 等动画播完 → 再真正移除"的两段式，并各自处理重入保护、卸载清理、
内容被复用时取消退场、退场期间屏蔽交互。这套逻辑目前在 `FeedbackToast` 和 `ModDetailDialog`
各写了一遍，"CSS 时长必须等于组件移除延迟"的隐性契约也各自用测试锁定。

风险:

- 每新增一个浮层就要重写一遍同样的两段式逻辑，且很容易只写入场、漏写退场
  （入场与退场不对称比完全没有动画更突兀）。
- 两套实现的时长、缓动与动效降级策略会各自漂移，浮层之间观感不一致。

交付:

- [ ] 把两段式退场能力收敛为共享 hook 或基元，统一时长、缓动与 `prefers-reduced-motion` 策略
- [ ] `ModDetailDialog` 迁移到共享基元，移除自带 portal 与重复动画实现
- [ ] 契约测试从"每个浮层各锁一份"收敛为基元层的单一断言
- [ ] 迁移不得改变既有无障碍语义（`role` / `aria-modal` / focus trap / ESC 行为）

硬边界:

- 属纯前端重构，不改任何任务语义、DTO 或后端契约
- 不得借此扩大浮层能力范围（不新增浮层类型、不改变现有浮层的交互语义）

---

## 远期 (P4)

- **T15**: 跨平台 Linux / Steam Deck — 本轮明确排除，不进入任务和验收
- **T16**: 更多游戏 Rise / Wilds — 每个游戏一份文档

---

## 推荐执行顺序

```text
已完成: T11 ARMOR_RETARGET Gate B certified -> T19 A1 -> L1 -> U1 -> U2 -> L2 -> U3 -> L3
  -> 已完成: T18 Mod 库分页 Slice 1（PR #186）
  -> 已完成: T18 Mod 库分页 Slice 2（PR #187）
  -> 已完成: T18 Mod 库分页 Slice 3（已合并）
  -> 已完成: T18 Mod 库分页 Slice 4A（PR #190）
  -> 已完成: T18 Mod 库分页 Slice 4B（PR #191）
  -> 已完成: T18 Mod 库分页 Slice 4C（PR #192）
  -> 已完成: T17 第三方批量迁移 Slice 1
  -> 已完成: T17 第三方批量迁移 Slice 2（PR #194）
  -> 已完成: T17 第三方批量迁移 Slice 3（PR #195）
  -> 已完成: T17 第三方批量迁移 Slice 4A（PR #196；PR #197 补齐 review 遗漏）
  -> 已完成: T17 Slice 4B（selection/decision/start/progress，PR #198）
  -> 已完成: T17 Slice 4C（result/retry/performance/hardening，PR #199）
  -> 优先独立开启: QG-01 CI 门禁（仅等人工治理 review 时不阻塞产品队列）
  -> 当前产品 P0: T13-00 批量设计
  -> CLI-2A/2B/2C observer、Sandbox 写许可和单项生命周期 E2E
  -> CORE-PREF-01 单项 preflight 一致化
  -> T13-01/02/03/04 BatchPlan、安装、卸载、真正重装
  -> T13-05/06/07 CLI、Tauri/typed API、前端工作流
  -> T13-08 disposable Windows Sandbox Gate C
  -> 装备数据治理、防具 catalog 扩容、独立武器重定向
  -> Windows 存档后台发布加固、日志治理和 Production CLI admission
```

---

## 状态追踪

| 任务 | 优先级 | 状态 | 关联 PR |
|------|--------|------|---------|
| T1 恢复中心写入型 UI | P0 | 已完成 | #108 |
| T2 持久化方案 | P0 | 已完成 | `ce9c486` / `dff6457` |
| T3 Mod 元数据后端 | P1 | 已完成 | `1e2c3b6` |
| T4 分类标签 | P1 | 已完成 | #112 / #113 / #114 |
| T5 Mod 信息面板 | P1 | 已完成 | #116 / `649a6cb` / `7ac8fb6` |
| T6 Profile 管理 | P1 | 已完成 | #122 |
| T7 一键启动 | P1 | 已完成 | #125 |
| Core Mod Lifecycle Gate A | P0 | 已 certified（CL0-CL4、L1/L2/L3 与完整验证通过） | |
| T8 存档备份 | P2 | 已完成部分保留，未完成部分暂停 | |
| T9 Rich Manifest | P0/P1 支撑 | Gate B binding snapshot 已落地；其余范围未被当前复审选中 | |
| T10 依赖检查 | P0/P1 支撑 | Gate A/B 最小 preflight 已完成；其余范围未被当前复审选中 | |
| T11 ARMOR_RETARGET | P1 | Gate B 已 certified（AR1-AR5、最终 Sandbox 纵向复验与完整验证通过） | |
| T12 Mod 详情完整版 | P3 | 最小替换目标 Tab 已实现；其余完整版范围暂停 | |
| T13 批量操作 | P0 | 待实施（T13-00 至 T13-08） | |
| T14 任务队列 UI | P3 | 暂停 | |
| T17 第三方管理器批量迁移 | P2 | 已完成（Slice 1/2/3/4A/4B/4C；4C 由 PR #199 交付） | #194（Slice 2）/ #195（Slice 3）/ #196（Slice 4A）/ #197（4A review 补救）/ #198（Slice 4B）/ #199（Slice 4C） |
| T18 Mod 库分页 | P2 | 已完成（Slice 1/2/3/4A/4B/4C；最后切片 PR #192） | #186（Slice 1）/ #187（Slice 2）/ #190（Slice 4A）/ #191（Slice 4B）/ #192（Slice 4C） |
| T19 核心生命周期产品化加固 | P0 发布加固 | 已完成（A1-L3 独立 review/合并与完成证据齐备） | #184（最终 L3 收尾） |
| T20 浮层动画收敛到共享基元 | P3 | 待评审（下次新增浮层前处理，或出现第三处重复实现时立即处理） | |
