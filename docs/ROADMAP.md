# 路线图

## 当前执行焦点（2026-07-16）

当前路线由 [核心 Mod 生命周期优先级计划](CORE_MOD_LIFECYCLE_PRIORITY_PLAN.md) 约束：

1. Gate A 已完成独立复审、完整验证并标记为 `certified`。
2. Gate B 的 AR1 领域模型/catalog、AR2 单 source `f_equip` parser/analyzer/`RetargetPlan`、AR3
   staging materialize/InstallPlan/binding snapshot 与 AR4 Tauri typed contract/最小受控 UI 已实现；
   当前下一项是 AR5 真正重装 target switch、卸载闭环与 Gate B 认证。
3. Gate B 前暂停自动备份剩余切片、installer cleanup、分页、批量迁移、批量操作、任务队列和
   非必要视觉增强。

旧 Phase 编号继续表示产品能力层次，不再表示当前实施顺序。安全、构建或 Gate A/B 直接阻断
可以插入；其他工作满足恢复门禁后重新排序。

## Phase 0：架构基线

- 初始化仓库。
- 沉淀架构、模块边界和 MVP 范围。
- 沉淀协作、安全、测试、发布相关文档。
- 确认第一版技术栈。
- 确认首个游戏适配目标：《怪物猎人：世界 冰原》。

## Phase 1：项目脚手架

- 创建 Tauri 2 应用脚手架。
- 接入 React + TypeScript 前端。
- 创建 Rust workspace crates。
- 加入格式化、lint 和基础 CI。
- 添加 SQLite migration 基础结构。
- 落地统一验证脚本和发布脚本骨架。

## Phase 2：MHW:I MVP Core

- 实现游戏目录检测。
- 实现手动选择游戏目录。
- 实现压缩包检查和沙盒解压。
- 实现 `nativePC`、DLL、图片、readme 检测。
- 实现安全的预览图提取。
- 实现分类和标签存储。
- 实现分类管理页面。
- 实现安装计划生成。
- 实现带安装清单和基础回滚能力的安装执行器。
- 实现基础冲突检测。
- 实现手动存档备份。
- 实现一键启动游戏。

InstallPlan 当前落地状态见 [InstallPlan 模块现状](INSTALL_PLAN_STATUS.md)，后续切片见 [InstallPlan MVP 待办](INSTALL_PLAN_MVP_TODO.md)。

本 Phase 的安装/卸载代码、固定人工 fixture 的 temp-root acceptance、真正重装 use case 和
disposable Windows Sandbox 桌面 smoke 已完成；retained/replaced/added/stale、重启、manifest 卸载、
baseline 恢复、诊断脱敏和 cleanup 已有 L1/L2/L3 证据；CL4 独立本地 review、完整验证与 clippy
也已通过，Gate A 已标记为 `certified`。AR1/AR2/AR3/AR4 已于 2026-07-16 标记为 `implemented`，
当前唯一下一项是 Gate B 的 AR5；真正重装 target switch、卸载扩展与 Gate B 验收尚未完成。

## Phase 3：玩家工作流扩展（Gate B 前暂停）

- 添加 Profile 支持。
- 添加前置依赖规则 catalog。
- 添加缺失前置警告。
- 添加自动存档备份调度。
- 添加 Mod 批量启用 / 禁用。
- 添加 Mod 库后端查询分页、稳定排序和本页选择语义，支撑大规模 Mod 库。详见 [Mod 库分页设计](MOD_LIBRARY_PAGINATION_DESIGN.md)。
- 添加第三方 Mod 管理器批量迁移，首个兼容来源为狩技盒子目录，默认只导入而不安装或启用。详见 [第三方 Mod 管理器批量迁移设计](EXTERNAL_MOD_MANAGER_BATCH_IMPORT_DESIGN.md)。
- 添加任务进度和取消 UI。

已完成能力继续保留。未完成扩展不在当前实施队列中；只有 Gate A/B 直接需要的最小
manifest/preflight/UI 子集可以提前。

## Phase 4：核心差异能力（Gate A 后立即执行）

- 已添加 MHW:I 最小 versioned armor target catalog、稳定 replacement identity/binding 与只读查询 port。
- 已完成单 source `f_equip` 严格路径分析和纯 `RetargetPlan` 外观替换映射。
- 已完成受控 batch staging materialize、containment 与失败清理。
- 已把 retarget final targets 交给 InstallPlan，并持久化 Mod/profile/revision-owned binding snapshot。
- 已完成四个窄 Tauri command、feature-local typed API，以及 `Mod 详情 -> 替换目标` Tab/右键直达；
  首次安装只接受稳定 identity，并对 installed/unsafe/unknown 状态 fail closed。
- 下一步完成真正重装 target switch 和卸载闭环；切换目标必须复用 Gate A 的真正重装。
- 通过 Gate B 后再扩展完整 catalog、本地化筛选和其他资源类型。

Gate B 后续范围：

- 添加武器替换映射。
- 添加语音替换映射。
- 添加感知绑定关系的冲突检测。
- 扩展多 source、男体路径和更复杂 transformer。

## Phase 5：跨平台准备

- 添加 Linux 路径抽象。
- 添加 Linux Steam library 扫描。
- 打包 Linux 版本。
- 通过社区测试验证 Steam Deck Desktop Mode。

## Phase 6：更多游戏

- 添加《怪物猎人：崛起》适配器。
- 等《怪物猎人：荒野》的 Mod 结构稳定后添加适配器。
- 抽取《怪物猎人》系列共享适配工具。
