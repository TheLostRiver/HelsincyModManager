# 路线图

> 无人值守自主迭代的具体任务队列见 [自主迭代路线图](AUTONOMOUS_ITERATION_ROADMAP.md)。
> 本文件描述产品阶段，那份描述在没有人盯着时可以安全推进哪些工作。

## 当前执行焦点（2026-08-08）

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
5. T17 第三方 Mod 管理器批量迁移已完成：Slice 1/2/3/4A、PR #198 的 Slice 4B 与 PR #199 的 Slice 4C
   共同交付 `hunting_box_directory_v1` 只读来源扫描、durable preview、后端 selection snapshot、
   显式决定、sealed batch start、严格按 `taskId` 的 import progress、权威分页结果、partial success 和
   服务端 retryable 重试。4C 复用同一 task progress 状态机处理重试返回的新 taskId，并在每个终态 task
   的首屏权威结果验证后至多刷新一次 Mod 库；10,000 条人工脱敏 result 的本机 p95=`3.937 ms`，低于固定
   `250 ms` 预算。默认仍只导入，不安装、启用或写游戏目录。
   T17 范围保持 Windows + MHW:I；Linux/Steam Deck 和更多游戏不进入本轮。T13 与 T17 继续正交，
   已在 2026-07-30 优先级复审后从独立设计任务 T13-00 恢复，不复用 T17 的 import-only 编排。
6. QG-01 已由 PR #215 合并并补齐 CI 质量门禁。[批量 Mod 生命周期领域设计](BATCH_MOD_LIFECYCLE_DESIGN.md)
   是 T13 的权威语义。Slice A-D 已完成：Sandbox 单项与批量 install/uninstall/true reinstall、窄
   Tauri/typed API、批量前端工作流和 4 viewport smoke 均已落地；最终 artifact 在 disposable
   Windows Sandbox 通过主链、受控 partial failure -> retry、重启、recovery 和 exact baseline，
   T13-08 Gate C 已于 2026-08-05 标记为 `certified`。CAT-01 装备数据治理也已完成。
7. 装备重定向排在 Gate C 之后：CAT-01 已交付 candidate schema/validator、stable ID、名称/状态和
   provenance/licensing 门禁；WR-01 已完成设计，WR-02A 已交付 14-family/part registry、严格路径与
   source closure parser、纯内存 catalog-source validator，WR-03A 已交付有界 MOD3/MRL3 preflight、
   pair compatibility 与纯 transformer，WR-03B 已完成版本化 registry、transform-aware staging、
   InstallPlan/manifest/recovery/Audit facts 与 temp-root exact-baseline 生命周期。WR-04 已完成窄 Tauri/
   typed API、Mod 详情目标工作流、4 viewport/theme smoke，并在全新 disposable Windows Sandbox 通过
   `one001` 安装 -> 重启 -> `one002` true reinstall target switch -> 重启 -> manifest 卸载 ->
   10 文件/316 bytes exact baseline，Gate D 于 2026-08-06 标记为 `certified`。AR6 防具扩容和 WR-02B
   完整武器 catalog 仍等待明确可再分发的审计数据；LOG-01 Task/Audit retention、LOG-02 日志总空间
   上限和 LOG-03 Debug Log 均已完成。SAVE-02 已在 disposable Windows Sandbox 完成安装态 sibling
   worker、真实 user Scheduled Task、人工触发、fresh heartbeat 与 ownership-checked 幂等 cleanup，并于
    2026-08-07 标记为 `certified`。SAVE-03 installer ownership cleanup 的 helper、双 Windows
    sidecar、NSIS PREUNINSTALL 和 WiX pre-`RemoveFiles` build/static gate 已完成。disposable Windows
    VM runtime gate 已推进：WiX `0.1.9` 的 `missing`、`owned exact`、`owned drift`、`foreign` 和
    `owned running` interactive/silent 变体均符合安全预期；running 在 MSI `1603` 阻断后保留任务与
    安装目录，任务自然回到 `Ready` 后重试卸载成功。WiX 已增加兼容其他 setup action 的固定 `1722`
    诊断文案，但其技术边界仍无法向 UI 透传 helper 原始 `20`。Settings 已收窄开关触发范围、增加动态
    反馈/耗时、复用会话状态，并将 register/unregister 收敛为受控 mutation + 最终读回；新修复又在
    约 3 秒增加一次有限自动读回，覆盖首次 inspect 与 heartbeat 的竞争窗口。最终 `0.1.10` 已通过 WiX
    upgrade/repair、owned 卸载、自动备份产物和 NSIS payload 尾部矩阵，但 NSIS 重新注册 exact `Ready`
    task 后没有保证本轮 worker 首次运行，导致 UI 长时间停留在 `starting`。当前已实现 Rust read-back
    校验后的独立 exact-owned 首次启动，并在启动操作内再次双读回防 TOCTOU；等待新候选的 NSIS 自动
    收敛、owned 卸载和 running fail-closed 复验，runtime gate 仍未完成。
   完整 catalog 未到位前仍只能使用人工最小 developer/Sandbox seed，不开放 Production 写入。
   防具 AR1-AR5 已认证不等于完整防具
   数据或完整武器链路已实现。
8. Windows 存档后续重点是已确认 Steam 账号目录的回归门禁、SAVE-02 已认证基线之上的
   ownership-checked installer cleanup、玩家存档恢复和 retention/备份中心。玩家存档恢复必须使用统一
   悬浮确认，默认先在独立 `pre-restore/` 目录创建安全备份，成功后才允许覆盖；用户可以关闭该开关，
   但必须看到高风险警告并额外确认。账号昵称/头像和多候选显式选择已完成，不重新实现。
9. Task/Audit retention、日志总空间上限和 Debug Log 已完成。Production CLI 写入继续等待跨进程
   admission；Sandbox CLI 已成为单项与批量核心生命周期自动化入口，批量 Tauri/前端体验已由
   Slice D 完成。Gate C 认证不开放 Production 写入。
10. GOV-01 至 GOV-04 已由 PR #211 至 #214 完成；DTO 测试外置、重装 dead-code 抑制清理、
    Tauri command 契约覆盖和治理检查加固成为后续任务必须保持的工程基线。

本轮任务、实现和验收只覆盖 Windows + MHW:I。Linux / Steam Deck 不进入当前队列，也不阻塞上述
Windows 任务。详细依赖与 PR/CI/review 门禁见 [Windows 自主迭代路线图](AUTONOMOUS_ITERATION_ROADMAP.md)。

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

T13 Slice A-D 已在该基线上完成；T13-07 的批量 UI 与 4 viewport smoke 为 `completed`，T13-08
disposable Windows Sandbox Gate C 已覆盖批量安装、重启、真正重装、Armor target switch、受控
partial failure/retry、recovery、批量卸载和 exact baseline，并于 2026-08-05 标记为 `certified`。

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
Slice 1/2/3/4A/4B/4C、T17 Slice 1/2/3/4A/4B/4C 与 T13 Slice A-D 均已完成。批量破坏性操作继续
受 [批量 Mod 生命周期领域设计](BATCH_MOD_LIFECYCLE_DESIGN.md) 约束，只在显式 Sandbox capability
下开放；T17 保持 Windows + MHW:I 与 import-only 边界，不扩张到 Linux/Steam Deck 或更多游戏。
CAT-01 装备数据治理、WR-01 武器重定向设计、WR-02A 纯解析、WR-03A 人工 binary transformer 与
WR-03B staging/InstallPlan/manifest 集成和 WR-04 受控 Tauri/UI/Gate D 均已完成，Gate D 为
`certified`；AR6/WR-02B 等待可再分发的审计数据，LOG-01 Task/Audit retention、LOG-02 日志总空间
上限和 LOG-03 Debug Log 均已完成；SAVE-02 安装态后台保护验收已 `certified`，当前下一无人值守
`ready` 任务为 SAVE-03。完整 catalog 未到位前只使用人工 developer/Sandbox seed。

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
- 已完成 CAT-01 candidate schema/validator、资源路径 SHA-256 stable ID、localization/alias、
  dummy/hidden 与 provenance/licensing 门禁；未授权候选数据继续禁止进入 bundled catalog。
- 已完成 WR-01 独立武器重定向设计；14 类 family、part pair、MOD3/MRL3 能力和 fail-closed 矩阵见
  [MHW:I 武器重定向设计](WEAPON_RETARGET_DESIGN.md)。
- 已完成 WR-02A：14-family/part registry、严格 resource/model path parser、source closure 和只读
  catalog-source validator 已落地；完整 catalog、staging 与真实写入仍未实现。
- 已完成 WR-03A：有界 MOD3/MRL3 preflight、JAMCRC pair compatibility、安全 game-resource reference
  parser、`mhw.weapon.mrl3-texture-path.v1` 纯 transformer、changed-range postcondition 与脱敏 digest/
  error projection 已由完全人工 bytes 覆盖。
- 已完成 WR-03B：通用 versioned invocation/registry、transform-aware sibling `.partial` staging、
  source/dependency/output/mapping digest 重验，以及 InstallPlan/reinstall/batch/manifest/recovery/Audit facts
  集成已落地；人工 temp-root 已通过安装、重启、same-revision target switch、再次重启、manifest 卸载和
  exact baseline。未增加 production weapon catalog、Tauri/UI 或 Production 写入。
- 已完成 WR-04：Production 继续保持 Armor-only；显式 GUI Sandbox 才启用人工 weapon seed 和受控写入
  admission。最终 artifact 已通过 contract/build/完整验证、light/dark/system 响应式 smoke，以及
  disposable Windows Sandbox 安装、两次重启、target switch、manifest 卸载、recovery 归零和 exact
  baseline，Gate D 已标记为 `certified`。
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
