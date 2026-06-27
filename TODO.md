# HelsincyModManager 后续任务总纲

创建时间：2026-06-27
基于 HEAD：`e1d4e868` (main)

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
- 恢复受控动作: `docs/INSTALL_RECOVERY_CONTROLLED_ACTIONS_PLAN.md`
- Mod 预览图: `docs/MOD_PREVIEW_IMAGE_PIPELINE_DESIGN.md` + `docs/MOD_PREVIEW_IMAGE_IMPLEMENTATION_PLAN.md`

### 需要新建独立文档的任务

- **T2 持久化方案**: → `docs/PERSISTENCE_DECISION.md` — SQLite vs JSON 架构决策
- **T8 存档备份**: → `docs/SAVE_BACKUP_DESIGN.md` — 涉及定时调度、保留策略等设计

---

## 优先级定义

| 级别 | 含义 | 节奏 |
|------|------|------|
| **P0** | 关键路径 — 不做会阻塞后续多个 feature | 立即推进 |
| **P1** | 核心 MVP — 用户可感知的必要能力 | P0 完成后紧接 |
| **P2** | 重要增强 — 提升完整度和用户体验 | P1 基本就绪后推进 |
| **P3** | 长线 feature — Phase 4+ 的大型功能 | 按 Roadmap 节奏 |

---

## 任务依赖图

```text
T1 恢复中心写入型 UI ───(无前置)───> 可立即开始
                                      │
T2 持久化方案决策 ───(无前置)───> 可与 T1 并行
    │
    ├──> T3 Mod 元数据更新后端
    │       └──> T5 Mod 信息编辑面板
    │
    ├──> T4 分类标签 CRUD
    │       └──> T5 Mod 信息编辑面板 (分类选择)
    │
    ├──> T6 Profile 管理
    │
    ├──> T8 存档备份 (备份历史需要存储)
    │
    └──> T11 ARMOR_RETARGET (binding 持久化)

T7 一键启动游戏 ───(无前置)───> 独立，可任意时机插入

T9 Rich Manifest ───(T1 完成后)───> 在 MVP manifest 上扩展

T10 Dependency/Preflight ───(T3 + T4)───> 需要元数据和分类

T11 ARMOR_RETARGET ───(T2 + T9 + InstallPlan staging)───> 最重依赖链

T12 Mod 详情统一面板完整版 ───(T5 + T11 部分就绪)───> 合并信息 + 替换目标
```

---

## P0 — 关键路径

### T1: 恢复中心写入型动作 UI 启用

**状态**: 后端就绪，前端 typed API 已接入，差最后一步 UI
**预估**: 小（1 个 PR）
**独立文档**: 不需要（`docs/INSTALL_RECOVERY_CONTROLLED_ACTIONS_PLAN.md` 已覆盖）

已就绪的后端基础:
- `start_recovery_action_task` Tauri command
- `preview_recovery_action` 只读预览
- `install.recovery.*` task phase
- 前端 `startRecoveryActionTask` / `previewRecoveryAction` typed API
- 恢复中心只读人工处理决策面板已有 `controlled_recovery` 不可用占位

本切片交付:
- [ ] 恢复中心决策面板启用 `rollback_install` 按钮
- [ ] 点击前调用 `previewRecoveryAction` 判断可执行性
- [ ] `available` 时启用确认流程，`blocked` 时展示阻断 reason code
- [ ] 确认后调用 `startRecoveryActionTask` 启动后端受控回滚
- [ ] 按 `taskId` 订阅 `install.recovery.*` phase，展示排队/执行中/完成/失败
- [ ] 完成后触发恢复中心重新扫描 + 刷新 profile 摘要和全局告警

安全边界:
- 前端只提交 `gameId`、`profileId`、`modId`、`actionKind`
- 不展示 target path、backup ref/root、manifest 正文
- 按钮只在 `rollback_required` 状态 Mod 上可见

---

### T2: 持久化方案决策与实施

**状态**: 完全空白 — 零 SQLite 依赖，所有持久化均为 JSON
**预估**: 中-大（方案决策 + 基础设施）
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
- [ ] 决策文档：范围、依赖选型（rusqlite vs sqlx）、迁移策略
- [ ] `hmm-infra` 引入 SQLite + migration 基础设施
- [ ] 初始 schema：`mod_metadata`、`categories`、`mod_categories`
- [ ] `hmm-ports` 新增 `ModMetadataRepository` trait
- [ ] `hmm-infra` 实现 SQLite 版仓储
- [ ] 不改变现有 JSON 仓储，两套共存

---

## P1 — 核心 MVP 功能

### T3: Mod 元数据更新后端

**前置**: T2
**预估**: 小-中
**独立文档**: 不需要

交付:
- [ ] `hmm-core` 定义 `ModMetadata` 可编辑字段（名称、作者、版本、备注、nexus_mod_id）
- [ ] `hmm-ports` 新增 `ModMetadataRepository` trait（read/update/delete overlay）
- [ ] `hmm-app` 新增 `ModMetadataService`（读取导入快照 + 合并用户 overlay）
- [ ] `hmm-tauri` 新增 `update_mod_metadata` 窄 command
- [ ] `get_mod_library` 返回合并后数据（用户编辑值优先）

---

### T4: 分类标签 CRUD

**前置**: T2
**预估**: 中
**独立文档**: schema 包含在 T2 决策文档中

交付:
- [ ] 领域模型：`Category`（id, name, color?, sort_order）、`Tag`（id, name）
- [ ] 多对多：Mod ↔ Category、Mod ↔ Tag
- [ ] `hmm-ports` + `hmm-app` + `hmm-tauri` CRUD command
- [ ] 前端 typed API
- [ ] `get_mod_library` 返回真实分类标签

---

### T5: Mod 信息编辑面板 (前端)

**前置**: T3 + T4
**预估**: 中
**独立文档**: 不需要（参考图 `C:\Users\Helsincy\Pictures\mod-manager\mod-info.png`）

设计要点:
- 统一悬浮对话框，合并参考图两个面板
- 当前包含"信息编辑"section；ARMOR_RETARGET 就绪后追加"替换目标"section
- 视觉远超参考图水平

交付:
- [ ] `ModDetailDialog.tsx` — 悬浮对话框
- [ ] 信息编辑：名称、作者、版本、备注、NexusMods ID
- [ ] 分类选择：多选 chip（消费 T4 数据）
- [ ] 预览图展示（只读）
- [ ] 右键菜单 `info-settings` 打开此面板
- [ ] 右键菜单 `edit-files` 暂显示"功能开发中"
- [ ] 保存后刷新 Mod 库

---

### T6: Profile 基础管理

**前置**: T2
**预估**: 中-大
**独立文档**: 建议（语义较多）

交付:
- [ ] 领域模型：`Profile`（id, name, description, created_at, is_active）
- [ ] 后端 CRUD + 切换活跃 profile
- [ ] 前端 Profile 列表、创建、切换、删除
- [ ] App Shell 展示当前活跃 profile
- [ ] 安装/卸载/恢复操作使用活跃 profile 而非硬编码 `"default"`

---

### T7: 一键启动游戏

**前置**: 无
**预估**: 小
**独立文档**: 不需要

交付:
- [ ] `hmm-ports` 新增 `GameLauncher` trait
- [ ] `hmm-games-mhw` 实现 MHW:I launcher（Steam protocol 优先）
- [ ] `hmm-tauri` 新增 `launch_game` command
- [ ] Dashboard / App Header 启动按钮

---

## P2 — 重要增强

### T8: 存档备份系统

**前置**: T2
**预估**: 大
**独立文档**: **需要** → `docs/SAVE_BACKUP_DESIGN.md`

概要:
- [ ] 手动备份 + 自动备份（可配置间隔）
- [ ] 保留策略（数量/时间/空间）
- [ ] 备份目录可选择（默认 app data）
- [ ] 前端 `features/backups/` 页面

---

### T9: Rich Manifest 与状态机

**前置**: T1
**预估**: 中
**独立文档**: 不需要（`docs/INSTALL_PLAN_MVP_TODO.md` 已有设计）

概要:
- [ ] 扩展字段：`manifest_id`、`backend`、`status`、`created_at`、`completed_at`、`plan_hash`
- [ ] 状态迁移规则 + 旧 manifest 兼容层
- [ ] `get_install_manifest_status` 消费 recovery scan
- [ ] `rolled_back` 状态持久化

---

### T10: 前置依赖检查

**前置**: T3 + T4
**预估**: 中
**独立文档**: 不需要

概要:
- [ ] 依赖规则 catalog（JSON/TOML 随 `hmm-games-mhw` 发布）
- [ ] 安装前检查 + 阻断/警告
- [ ] 前端展示依赖检查结果

---

### T13: 批量操作

**前置**: T6
**预估**: 中
**独立文档**: 不需要

概要:
- [ ] 批量安装/卸载任务队列
- [ ] 前端多选 + 批量按钮
- [ ] 批量进度面板

---

## P3 — 长线 Feature

### T11: ARMOR_RETARGET 全链路

**前置**: T2 + T9 + InstallPlan staging
**预估**: 大（12 Task，3-5 个 PR）
**独立文档**: **已有** → `docs/ARMOR_RETARGET_IMPLEMENTATION.md`

执行顺序（遵循已有实施计划）:
1. Core replacement 领域模型
2. Replacement ports
3. MHW armor catalog + Unicode 归一化
4. MHW armor path parser
5. MHW RetargetPlan builder
6. Application replacement service
7. Staging materialize
8. Manifest + InstallPlan 集成
9. Tauri commands + DTO
10. 前端 typed API

---

### T12: Mod 详情统一面板 (完整版)

**前置**: T5 + T11 部分就绪
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
**预估**: 小-中
**独立文档**: 不需要

---

## 远期 (P4)

- **T15**: 跨平台 Linux / Steam Deck — 需要独立文档
- **T16**: 更多游戏 Rise / Wilds — 每个游戏一份文档

---

## 推荐执行顺序

```text
第 1 轮: T1 恢复中心写入型 UI        ← 最后一公里
         T2 持久化方案决策文档        ← 可与 T1 并行输出

第 2 轮: T2 持久化实施 (SQLite 基础)
         T7 一键启动游戏             ← 独立，与 T2 并行

第 3 轮: T3 Mod 元数据更新后端
         T4 分类标签 CRUD

第 4 轮: T5 Mod 信息编辑面板         ← 消费 T3 + T4

第 5 轮: T6 Profile 管理
         T9 Rich Manifest

第 6 轮: T10 依赖检查
         T13 批量操作
         T8 存档备份

第 7 轮: T11 ARMOR_RETARGET

第 8 轮: T12 Mod 详情完整版
         T14 任务队列 UI
```

---

## 状态追踪

| 任务 | 优先级 | 状态 | 关联 PR |
|------|--------|------|---------|
| T1 恢复中心写入型 UI | P0 | 已完成 | #108 |
| T2 持久化方案 | P0 | 待开始 | |
| T3 Mod 元数据后端 | P1 | 待开始 | |
| T4 分类标签 | P1 | 待开始 | |
| T5 Mod 信息面板 | P1 | 待开始 | |
| T6 Profile 管理 | P1 | 待开始 | |
| T7 一键启动 | P1 | 待开始 | |
| T8 存档备份 | P2 | 待开始 | |
| T9 Rich Manifest | P2 | 待开始 | |
| T10 依赖检查 | P2 | 待开始 | |
| T11 ARMOR_RETARGET | P3 | 待开始 | |
| T12 Mod 详情完整版 | P3 | 待开始 | |
| T13 批量操作 | P2 | 待开始 | |
| T14 任务队列 UI | P3 | 待开始 | |
