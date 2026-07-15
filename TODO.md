# HelsincyModManager 后续任务总纲

创建时间：2026-06-27
基于 HEAD：`e1d4e868` (main)
最近同步：2026-07-12，基于 `9618cfc`（P7.2c docs-only 规划基线）

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
- 恢复受控动作: `docs/INSTALL_RECOVERY_CONTROLLED_ACTIONS_PLAN.md`
- Mod 预览图: `docs/MOD_PREVIEW_IMAGE_PIPELINE_DESIGN.md` + `docs/MOD_PREVIEW_IMAGE_IMPLEMENTATION_PLAN.md`
- 第三方 Mod 管理器批量迁移: `docs/EXTERNAL_MOD_MANAGER_BATCH_IMPORT_DESIGN.md`
- Mod 库分页: `docs/MOD_LIBRARY_PAGINATION_DESIGN.md`
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

### 当前执行覆盖规则（2026-07-12）

[核心 Mod 生命周期优先级计划](docs/CORE_MOD_LIFECYCLE_PRIORITY_PLAN.md) 在 Gate B 完成前覆盖本文件
旧的推荐执行顺序，但不覆盖架构和玩家数据安全规则：

1. P0：认证安装/卸载闭环并实现真正重装（Gate A）。
2. P1：完成 ARMOR_RETARGET 最窄纵向切片（Gate B）。
3. T9/T10 只做解除 Gate A/Gate B 阻断的最小 manifest/preflight 子集。
4. 其他未完成 feature 暂停实施；已完成能力和既有设计继续保留。

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
  -> 重新排序并恢复 T8/T12/T13/T14/T17/T18
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

Gate A 已通过；Gate B 完成前仍暂停 P7.2c、分页、批量迁移、批量操作、任务队列和新的非阻断视觉工作。

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
**状态**: 已完成部分保留；P7.2a 安装态 acceptance、P7.2c 实现、retention 扩展和备份中心暂停到 Gate B 后重排
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
- [ ] P7.2a Windows 安装态 runtime acceptance（暂停；Gate B 后按发布风险重排）
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
**状态**: 已有基础保留；仅允许 Gate A 重装/写侧状态门禁和 Gate B binding snapshot 的最小阻断子集
**预估**: 中
**独立文档**: 不需要（`docs/INSTALL_PLAN_MVP_TODO.md` 已有设计）

概要:
- [x] 已落地：`manifest_id`、`schema_version` / `schema_migration`、`backend`、`status`、`created_at`、`completed_at`、`plan_hash` JSON 兼容层；安装提交成功会写入 schema metadata、`backend`、完成时间和真实 `plan_hash`
- [ ] Gate B 必需：replacement binding snapshot
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

**前置**: T6
**状态**: 暂停；单项 install/uninstall/reinstall certified 后重新设计批量队列语义
**预估**: 中
**独立文档**: 不需要

概要:
- [ ] 批量安装/卸载任务队列
- [ ] 前端多选 + 批量按钮
- [ ] 批量进度面板

---

### T17: 第三方 Mod 管理器批量迁移（狩技盒子兼容）

**前置**: 单包安全导入链路 + TaskManager/取消 + Mod 导入结果持久化
**状态**: 已规划、暂停；Gate B 和单包生命周期认证后恢复
**预估**: 大，建议拆为 4 个独立 review 切片
**独立文档**: **已创建** → `docs/EXTERNAL_MOD_MANAGER_BATCH_IMPORT_DESIGN.md`

范围:
- [x] 设计来源 adapter、批次预览、去重/冲突、取消/重试和安全边界
- [ ] Slice 1：无路径领域/selection 契约、批次选择上限、ports、仓储基准/决策和人工 fixtures
- [ ] Slice 2：`hunting_box_directory_v1` 只读扫描、内容指纹、分页预览和 scan task
- [ ] Slice 3：安全物化、复用单包导入链路、partial success、幂等和恢复对账
- [ ] Slice 4：来源选择、候选选择/服务端全选、分类映射、冲突决定、进度/结果 UI 和完整加固

硬边界:
- 默认只导入，不自动安装、启用或写游戏目录
- 不读取第三方数据库/账号，不同步启用状态、优先级或安装事实
- 前端不接触来源路径、XML 解析、hash、sandbox/cache 或去重规则
- 与 T13 的批量安装/卸载队列正交，不能借批量导入绕过 `InstallPlan` / manifest / backup / rollback
- 自动化只用临时目录与人工 fixture，不提交或读取真实第三方 Mod

---

### T18: Mod 库分页

**前置**: 现有 Mod 库 + Profile install/recovery 状态查询
**状态**: 已规划、暂停；Gate B 后仅在大库数据证明成为主要阻塞时恢复
**预估**: 中-大，建议拆为 4 个独立 review 切片
**独立文档**: **已创建** → `docs/MOD_LIBRARY_PAGINATION_DESIGN.md`

范围:
- [x] 设计后端权威查询分页、搜索/filter/稳定排序、page-local selection 和 UI 状态边界
- [ ] Slice 1：app-level query/filter/sort/page 类型、兼容聚合服务和 fake repository 测试
- [ ] Slice 2：`query_mod_library` Tauri DTO、稳定错误、feature-local typed API 和 contract 文档
- [ ] Slice 3：数字分页 footer、debounce/stale response、loading/error/empty 和本页选择 UI
- [ ] Slice 4：与 T17 共用可查询 read model/持久化决策、大库基准、视觉 smoke 和性能门禁

关键语义:
- 默认每页 24，可选 12/24/48/96；使用 1-based 数字页
- 搜索、分类/状态筛选、稳定排序、总数和 page 必须由同一后端查询快照决定
- 首版只提供“选择本页/反选本页”；跨页全选和批量写操作留给 T13
- T18 不依赖 T17，但应在 T17 Slice 4 完整迁移 UI 对外完成前落地
- 当前 JSON 全量读取只允许作为兼容阶段，不能把 bridge payload 变小等同于大库性能完成
- 本轮不修改前端、Tauri、Rust、依赖或 migration

---

## P1 核心差异能力 / P3 长线 Feature

### T11: ARMOR_RETARGET 全链路

**优先级**: P1，Gate A 通过后立即开始
**状态**: AR1 已实现；当前下一项 AR2，Gate B 尚未完成
**前置**: Gate A certified + T9/T10 最小直接前置 + InstallPlan staging
**预估**: 大（12 Task，3-5 个 PR）
**独立文档**: **已有** → `docs/ARMOR_RETARGET_IMPLEMENTATION.md`

执行顺序（遵循已有实施计划）:
- [x] AR1：Core replacement identity / binding / versioned catalog 领域模型
- [x] AR1：只读 Replacement catalog list/find/search ports
- [x] AR1：MHW armor catalog + Unicode/search normalization
- [ ] AR2：MHW armor path parser 与单 source analyzer
- [ ] AR2：MHW RetargetPlan builder
- [ ] AR3：Application replacement service 与 staging materialize
- [ ] AR3：Manifest + InstallPlan + binding snapshot 集成
- [ ] AR4：Tauri commands / DTO 与前端 typed API / 受控 UI
- [ ] AR5：真正重装 target switch、卸载与 Gate B 认证

---

### T12: Mod 详情统一面板 (完整版)

**前置**: T5 + T11 部分就绪
**状态**: 暂停；Gate B 内只实现 ARMOR 最小 target 选择 UI
**预估**: 中
**独立文档**: 不需要

概要:
- [ ] 在 T5 基础上追加"替换目标"tab
- [ ] 展示源槽位 + armor catalog + 搜索筛选
- [ ] 选择目标后预览 retarget plan
- [ ] 右键 `edit-files` 打开此 tab

---

### T14: 任务队列 UI

**前置**: T13
**状态**: 暂停；T13 恢复且出现真实多任务需求后再开始
**预估**: 小-中
**独立文档**: 不需要

---

## 远期 (P4)

- **T15**: 跨平台 Linux / Steam Deck — 需要独立文档
- **T16**: 更多游戏 Rise / Wilds — 每个游戏一份文档

---

## 推荐执行顺序

```text
当前: T11 ARMOR_RETARGET AR2（单 source f_equip parser / analyzer / RetargetPlan）
  -> AR3-AR5 最窄纵向闭环
  -> Gate B 认证
  -> 重新评审并排序 P7.2c、T8、T12、T13、T14、T17、T18
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
| T9 Rich Manifest | P0/P1 支撑 | 仅允许 Gate A/B 最小阻断子集 | |
| T10 依赖检查 | P0/P1 支撑 | 仅允许 Gate A/B 最小 preflight | |
| T11 ARMOR_RETARGET | P1 | AR1 已实现，当前下一项为 AR2 | |
| T12 Mod 详情完整版 | P3 | 暂停；仅 Gate B 最小 UI 例外 | |
| T13 批量操作 | P2 | 暂停 | |
| T14 任务队列 UI | P3 | 暂停 | |
| T17 第三方管理器批量迁移 | P2 | 已规划、暂停 | |
| T18 Mod 库分页 | P2 | 已规划、暂停 | |
