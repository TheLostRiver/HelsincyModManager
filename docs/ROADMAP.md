# 路线图

## 当前执行焦点（2026-07-26）

当前路线由 [核心 Mod 生命周期优先级计划](CORE_MOD_LIFECYCLE_PRIORITY_PLAN.md) 和
[核心 Mod 生命周期产品化加固实施计划](CORE_MOD_LIFECYCLE_PRODUCTIZATION_PLAN.md) 共同约束：

1. Gate A 已完成独立复审、完整验证并标记为 `certified`。
2. Gate B 的 AR1-AR5、真正重装 target switch、同 revision binding/entry 原子替换、重启恢复、
   manifest 卸载和受控 UI 已完成；修复当前 target 呈现缺陷后的最终 artifact 已在全新 disposable
   Windows Sandbox 通过首次 Alpha retarget 安装 -> Beta target switch -> 两次重启状态恢复 ->
   manifest 卸载 -> exact baseline 纵向复验，Gate B 已标记为 `certified`。
3. Gate B 后优先级复审选择的 T19“核心 Mod 生命周期产品化加固”已于 2026-07-18 完成；A1、L1、
   U1、U2、L2、U3、L3 七切片均经独立 review 合并，CI 验收、默认脱敏诊断、分层反馈、完整验证、
   视觉 smoke、受控 Windows 桌面复验和契约同步均已满足完成定义。
4. T18 Mod 库分页已完成：Slice 1 的 app-level 查询服务由 PR #186 完成，Slice 2 的 Tauri DTO、稳定错误、
   feature-local typed API 和 contract 由 PR #187 完成，Slice 3 已合并，Slice 4A/4B 分别由 PR #190/#191
   完成，最后的 Slice 4C 已由 PR #192 rebase 合并。其生产 query switch、同事务 count/page、fail-closed
   freshness tracking 和性能门禁的最终 10,000 条 full status-filter query p95 为 `9.2966 ms`，低于固定
   `14.23 ms` 同机预算。
5. T17 第三方 Mod 管理器批量迁移已完成：Slice 1/2/3/4A、PR #198 的 Slice 4B 与当前独立 Slice 4C
   共同交付 `hunting_box_directory_v1` 只读来源扫描、durable preview、后端 selection snapshot、
   显式决定、sealed batch start、严格按 `taskId` 的 import progress、权威分页结果、partial success 和
   服务端 retryable 重试。4C 复用同一 task progress 状态机处理重试返回的新 taskId，并在每个终态 task
   的首屏权威结果验证后至多刷新一次 Mod 库；10,000 条人工脱敏 result 的本机 p95=`3.937 ms`，低于固定
   `250 ms` 预算。默认仍只导入，不安装、启用或写游戏目录。
   T17 范围保持 Windows + MHW:I；Linux/Steam Deck、更多游戏和 T13 批量安装/卸载仍须单独评审，
   不因 T17 完成自动开工。

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
也已通过，Gate A 已标记为 `certified`。AR1-AR5 的代码、自动化与受控 UI 已于 2026-07-16 标记为
`implemented`；修复后最终 artifact 的全新 Sandbox 纵向复验已完成，Gate B 同日标记为
`certified`。

Gate A/B 之后执行的 [核心 Mod 生命周期产品化加固](CORE_MOD_LIFECYCLE_PRODUCTIZATION_PLAN.md) 已于
2026-07-18 标记为 `completed`：不少于 6 个 headless 生命周期场景已有正式 CI 验收入口，安全 App/Task
日志、审计降级可见性和分层操作反馈均已落地，A1-L3 七切片的独立 review、完整验证、视觉 smoke、
受控 Windows 桌面复验和契约同步均已完成。

## Phase 3：玩家工作流扩展（Gate B 后已重新排序）

- 添加 Profile 支持。
- 添加前置依赖规则 catalog。
- 添加缺失前置警告。
- 添加自动存档备份调度。
- 添加 Mod 批量启用 / 禁用。
- 添加 Mod 库后端查询分页、稳定排序和本页选择语义，支撑大规模 Mod 库。详见 [Mod 库分页设计](MOD_LIBRARY_PAGINATION_DESIGN.md)。
- 添加第三方 Mod 管理器批量迁移，首个兼容来源为狩技盒子目录，默认只导入而不安装或启用。详见 [第三方 Mod 管理器批量迁移设计](EXTERNAL_MOD_MANAGER_BATCH_IMPORT_DESIGN.md)。
- 添加任务进度和取消 UI。

已完成能力继续保留。Gate A/B 直接需要的最小 manifest/preflight/UI 子集、T19 产品化加固、T18
Slice 1/2/3/4A/4B/4C 与 T17 Slice 1/2/3/4A/4B/4C 均已完成。批量破坏性操作仍属于 T13，必须另行
评审；T17 保持 Windows + MHW:I 与 import-only 边界，不扩张到 Linux/Steam Deck 或更多游戏。

## Phase 4：核心差异能力（Gate A 后立即执行）

- 已添加 MHW:I 最小 versioned armor target catalog、稳定 replacement identity/binding 与只读查询 port。
- 已完成单 source `f_equip` 严格路径分析和纯 `RetargetPlan` 外观替换映射。
- 已完成受控 batch staging materialize、containment 与失败清理。
- 已把 retarget final targets 交给 InstallPlan，并持久化 Mod/profile/revision-owned binding snapshot。
- 已完成六个窄 Tauri command、feature-local typed API，以及 `Mod 详情 -> 替换目标` Tab/右键直达；
  首次安装只接受稳定 identity，已安装 target switch 只走 Gate A 真正重装，两者均对不安全状态
  fail closed。
- 已完成 AR5 同 revision target switch、重启恢复、manifest 卸载与当前 installed target 呈现；最终
  artifact 已通过全新 disposable Windows Sandbox Gate B 复验并标记为 `certified`。
- 完整 catalog、本地化筛选和其他资源类型已满足 Gate B 时间门禁，但仍需经过后续优先级复审。

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
