# 核心 Mod 生命周期优先级计划

- 日期：2026-07-12
- 状态：Gate A 已于 2026-07-15 标记为 `certified`；AR1-AR5 与最终 Sandbox 纵向复验已于
  2026-07-16 完成，Gate B 已标记为 `certified`；Gate B 后优先级复审选定的 T19 已于 2026-07-18
  完成，当前主线为 T18 Mod 库分页 Slice 1
- 适用范围：安装、卸载、真正重装、ARMOR_RETARGET 及其直接前置
- 决策目的：在继续扩展外围能力前，先把已有安全基础转化为可重复验收的玩家核心闭环

## 1. 决策

项目立即切换到“核心 Mod 生命周期优先”执行模式：

1. **P0 / Gate A：** 认证安装与卸载闭环，并实现真正重装。
2. **P1 / Gate B：** 完成第一条 MHW:I ARMOR_RETARGET 纵向闭环。
3. 只允许为 Gate A/Gate B 补充必要的 manifest、repair、preflight、staging 和测试能力。
4. Gate A/B 执行期间暂停 P7.2c、备份中心、分页、批量迁移、批量操作、任务队列和非必要视觉增强；
   Gate B 通过后先重新排序，不自动恢复实施。

暂停不是取消。已完成代码、设计和测试继续保留；满足本文恢复门禁后，再按产品发布需要恢复。
本文只覆盖执行顺序，不降低 `InstallPlan -> backup -> commit -> manifest -> rollback/recover`
安全链、crate 边界、日志脱敏、写锁或测试要求。

## 2. 为什么现在重排

以下是 2026-07-12 决定重排优先级时的历史基线；当前完成状态以本文开头和第 3 节为准：

- 当时安装提交、manifest、backup、rollback、恢复记录、任务事件和前端入口已经存在。
- 当时 manifest 驱动的最小安全卸载与前端单选卸载已经存在。
- 当时自动化主要证明 temp/fake 环境中的安全规则，还没有独立的 Mod 生命周期端到端验收记录。
- 当时 UI 的“安装 / 重装”复用普通安装入口，还没有独立重装 use case。
- 当时 manifest merge 会保留新计划未触达的旧条目，不能处理新版本删减文件的真正重装。
- 当时 ARMOR_RETARGET 已有完整设计；replacement/binding/catalog、纯分析/计划、受控 staging 与
  InstallPlan/manifest binding snapshot 已进入源码，Tauri/前端与 target switch 尚未落地。

因此，接下来继续扩大备份后台、安装器 cleanup、分页、批量迁移或 UI 完整度，不能像核心闭环
那样直接证明产品价值。当前最重要的成果不是新增模块数量，而是让一条玩家工作流可安装、可
重装、可卸载、可恢复，并能在 ARMOR_RETARGET 场景中复用。

## 3. 状态词汇

后续文档和交付统一使用以下状态，避免把不同层次的“完成”混为一谈：

| 状态 | 含义 |
| --- | --- |
| `implemented` | 代码和聚焦自动化存在，但未完成本文要求的完整闭环证据 |
| `completed` | 任务自身的完成定义已满足，且计划内的独立 review 与合并均已完成；不自动等同于 Gate 的 `certified` |
| `certified` | 自动化验收矩阵、桌面手动 smoke、状态恢复和清理证据均通过 |
| `planned` | 已有设计/实施计划，但没有运行时代码 |
| `paused` | 资产保留，当前不得主动实施；满足恢复门禁后再继续 |
| `blocked` | 已开始核心切片，但被明确技术/环境前置阻断 |
| `current` | 仅表示路线图中当前唯一执行主线，不作为实现程度或验收成熟度证据 |

当前基线：安装、卸载和真正重装均为 `implemented`，Core Mod Lifecycle Gate A 为
`certified`；ARMOR_RETARGET AR1-AR5 为 `implemented`，修复当前 target 呈现缺陷后的最终 artifact
已通过全新 Sandbox 纵向复验，Gate B 为 `certified`；T19 为 `completed`，T18 Slice 1 为当前主线，
P7.2c 等其余候选继续按各自恢复门禁评审。

## 4. 立即执行顺序

```text
CL0 事实基线与可重复验收入口
  -> CL1 安装 / 卸载自动化纵向闭环
  -> CL2 桌面应用手动 smoke 与缺口修复
  -> CL3 真正重装 contract 与实现
  -> CL4 Gate A 复审和认证
  -> AR1 ARMOR 领域模型 / binding / 最小 catalog [implemented]
  -> AR2 MHW:I 单源 f_equip 分析与 RetargetPlan [implemented]
  -> AR3 staging materialize + InstallPlan + manifest snapshot [implemented]
  -> AR4 Tauri typed contract + 选择目标 / 预览 / 安装 UI [implemented]
  -> AR5 真正重装 target switch / 卸载 / 当前 target 重启呈现 [implemented]
  -> Gate B 最终 artifact 全新 Sandbox 复验 [certified]
  -> T19 产品化加固 [completed]
  -> T18 Mod 库分页 Slice 1 [current]
```

任一时刻只推进一条主切片。安全缺陷、构建阻断和当前切片必需的小修可以插入；其他“顺手”
功能不得混入。

## 5. Gate A：核心 Mod 生命周期认证

### CL0：建立事实基线与验收入口

**状态（2026-07-12）：** 已完成。fixture contract、test-only AppState import/plan/restart harness、
验收矩阵、composition 缺口与桌面 smoke 见
[CL0 验收基线](CORE_MOD_LIFECYCLE_CL0_ACCEPTANCE.md)。CL0 不执行 install/uninstall game writes，
因此不代表 CL1、CL2 或 Gate A 完成。

目标不是重写已有安装模块，而是建立一套能证明完整闭环的证据：

- 固定两个完全人工构造的 Mod fixture：`v1` 与 `v2`。
- fixture 同时覆盖“新增文件”和“覆盖已有文件”；不包含第三方 Mod 内容。
- `v2` 至少包含一个内容变更、一个新增 target、一个从 `v1` 消失的 stale target。
- 使用临时 MHW:I-like game root、受控 sandbox、backup root、manifest root 和 SQLite/AppData。
- 从持久化导入记录进入真实 app/infra composition，不只调用孤立 domain helper。
- 每个阶段记录磁盘摘要、manifest 摘要、backup 事实和公开 task/status，不记录完整本地路径。

CL0 的产物应是一份可执行 smoke/acceptance 文档和自动化 harness，不是新的安装抽象。

### CL1：安装与卸载自动化纵向闭环

**状态（2026-07-13）：** 已完成。L2 AppState composition 已使用人工 v1 zip、temp AppData 和
temp game root 完成 install -> restart -> uninstall -> baseline；L1 已直接覆盖 source read 与
backup store failure 在任何 target mutation 前停止，并复用既有 write/manifest/rollback/drift
安全证据。成功 install/uninstall Audit Log 只保留稳定 id 和计数。CL1 不代表 CL2、CL3 或 Gate A
完成。

至少覆盖：

1. 导入 `v1`，由后端重建 InstallPlan。
2. 预览结果只返回相对路径/计数/冲突摘要，不泄露真实 root。
3. 安装 `v1`：新增文件写入，已有文件先备份后覆盖，manifest 完成。
4. 重建 app service/repository，状态仍从持久化事实恢复为 installed。
5. 卸载 `v1`：新增文件删除，被覆盖文件按长期 backup 恢复。
6. 再次重建服务，状态为 not installed，临时 game root 与基线逐字节一致。
7. source read、backup、write、manifest save 分别注入失败，证明 rollback/recovery 状态正确。
8. 当前 target 摘要漂移、backup 缺失、旧 manifest 无摘要时，破坏性动作 fail closed。

自动化不得依赖真实 MHW:I 安装、真实玩家文件、Steam userdata、第三方 Mod 或日常账户。

### CL2：桌面应用手动 smoke

**状态（2026-07-13）：** 已完成。Windows Sandbox 中的实际 Tauri 应用使用人工 TEMP game/archive
完成 picker -> import -> 4-action preview -> install -> restart -> uninstall -> restart -> baseline；
支持诊断包只含短 id、计数和稳定 operation/result，受控 TEMP 与 disposable AppData 已清理。首次
执行发现的 import UI 缺口和 mapped-folder commit/rollback 安全缺口已最小修复并回归。未执行真实
游戏 smoke，也未验证 v1 -> v2；因此 CL2 不代表 CL3 或 Gate A 完成。

在专用测试环境运行实际 Tauri 应用，验证：

- 游戏目录选择、人工 fixture 导入、计划预览、安装确认和任务进度可完成。
- 关闭并重开应用后，Mod 库从 manifest/recovery 事实恢复安装状态。
- 卸载确认、任务进度和完成后状态刷新可完成。
- 错误/阻断文案不显示完整路径、backup ref、manifest 正文或 hash。
- smoke 结束时专用游戏目录回到基线，fixture、backup 和任务状态均按文档清理。

真实游戏 smoke 只在维护者明确授权的专用测试副本/账户中执行。普通自动化和默认手动 smoke
继续使用临时 game root；不得为了勾选 checklist 操作玩家日常游戏目录。

### CL3：真正重装

**实施状态（2026-07-15）：** CL3 Task 1-10 已完成，正式 contract/spec 与逐任务实施记录见
[CL3 真正重装设计](superpowers/specs/2026-07-14-core-mod-lifecycle-cl3-true-reinstall-design.md) 和
[CL3 真正重装实施计划](superpowers/plans/2026-07-14-core-mod-lifecycle-cl3-true-reinstall-implementation.md)。
Rust/Tauri/frontend/migration、L1/L2 自动化和 disposable Windows Sandbox L3 均已落地并执行；同一
logical Mod 的 v1 -> v2 真正重装、重启、manifest 卸载、baseline 恢复、诊断脱敏和 cleanup 已有
证据，CL3 状态为 `implemented`；后续 CL4 独立复审已完成，Gate A 已标记为 `certified`。

真正重装使用独立 backend use case/task，不再是普通 install 的 UI 别名。其
事实来源为“旧 manifest + 新 InstallPlan + 当前目标摘要 + backup”，不能根据展示名或当前包
内容直接覆盖。

#### 5.3.1 计划分类

重装计划必须先把指定 Mod 的旧条目与新计划按最终 target 分类：

| 分类 | 含义 | 预期动作 |
| --- | --- | --- |
| retained | 新旧计划都管理同一 target，内容无需变化 | 保留受控事实 |
| replaced | 新旧计划都管理同一 target，但 provider/content 改变 | 写入新内容，保留原始长期 backup 语义 |
| added | 只在新计划中出现 | 按普通安装新增/覆盖规则处理 |
| stale | 只在旧 manifest 中出现 | 摘要匹配后删除新增文件，或从长期 backup 恢复原始文件 |

#### 5.3.2 安全顺序

1. 在 mutation 前完整读取新 source、旧 manifest、当前目标摘要和所需 backup。
2. 对全部 replaced/stale target 做 preflight；任一不确定则零写入阻断。
3. 在同一 `gameId/profileId` 写锁下执行一个 ReinstallPlan。
4. 为本次重装创建 pending recovery facts，使失败能恢复到“重装前版本”，而不是错误恢复到游戏
   原始基线。
5. 成功后原子更新该 Mod 的 manifest entry set：stale entries 消失，retained/replaced/added
   成为新事实；长期 backup 仍能支持未来完整卸载。
6. 失败时恢复重装前磁盘和 manifest 状态；无法确认时进入受控 recovery，而不是报告 completed。

#### 5.3.3 对外契约

- 新增明确的 `start_reinstall_task` 或等价窄用例，不再把 `start_install_task` 称为完整重装。
- 前端只提交 `gameId`、`profileId`、`modId` 和必要的受控选择，不提交路径或删除列表。
- UI 必须在预览中展示 retained/replaced/added/stale 聚合计数，并明确这是版本替换。
- 真正重装完成前，现有按钮应按真实能力表述为安装/覆盖更新，或对已安装 Mod 禁用“重装”承诺。

### CL4：Gate A 完成定义

Gate A 只有同时满足以下条件才可标记 `certified`：

- CL1 自动化矩阵全部通过，fixture 与临时目录可重复、可清理。
- CL2 桌面 smoke 实际执行并记录结果；未执行真实游戏 smoke 的边界如实说明。
- CL3 对 `v1 -> v2` 的 retained/replaced/added/stale 全部有测试。
- `v1 -> v2 -> uninstall` 后，game root 与安装前基线逐字节一致。
- 应用重启后安装事实、recovery 状态和 UI 动作可用性正确。
- 安装、卸载、重装失败均不产生误导性的 completed manifest/task 状态。
- Audit Log/Task Log 只包含稳定 id、计数、phase/result/error code，不泄露敏感路径或内容。
- 完整 `verify.ps1`、边界聚焦测试和本地 review gate 通过。

2026-07-15 CL4 独立复审结论：以上条件全部满足，Gate A 标记为 `certified`。本轮从
`origin/main@0b1119f` 建立隔离 worktree，重新审计 core/app/ports/infra/Tauri/frontend、共享写锁、
manifest/backup/rollback/recovery、Task/Audit/diagnostics 和 L1/L2/L3 证据；未发现需要修改生产代码的
Gate A 阻断。生命周期聚焦矩阵、全部前端测试、完整 `scripts/verify.ps1` 和
`cargo clippy --workspace --all-targets -- -D warnings` 均通过。

## 6. Gate B：ARMOR_RETARGET 最窄纵向闭环

**状态（2026-07-16）：** 已完成并标记为 `certified`。最终 artifact 在全新 disposable Windows
Sandbox 中满足第 6.3 节全部完成定义；只使用人工 fixture，未操作真实 MHW:I、第三方 Mod、Steam
userdata 或玩家存档。

Gate A 完成后，ARMOR_RETARGET 立即成为唯一 P1 主线。第一版只证明一个真实产品工作流，不追求
完整 transformer 平台。

### 6.1 固定范围

- 只支持 MHW:I `nativePC/pl/f_equip/<slot>/arm/mod/<filename>` 路径族。
- 单个 Mod 只接受一个明确 source slot；多 source、`m_equip` 和未知 path family fail closed。
- 只做结构化 slot 段替换，不修改 `.mod3`、`.mrl3`、`.tex` 二进制内容。
- 原始导入包只读；materialized 文件只写受控 staging。
- 最终写入继续经过 InstallPlan、conflict、backup、manifest、rollback/recovery。
- 最小 versioned catalog 足以支撑 fixture 和首批受控目标；完整本地化/筛选可后续扩展。

### 6.2 实施切片

1. **AR1（已完成）：** 稳定 `ReplacementTarget` / `ReplacementBinding` / catalog 领域模型、只读
   catalog ports，以及 MHW:I 最小 versioned catalog 与 Unicode/search normalization。
2. **AR2（已完成）：** 严格 armor path parser、单 source `f_equip` analyzer 与纯 `RetargetPlan`。
3. **AR3（已完成）：** staging materialize、containment、final target conflict、binding persistence 和
   manifest snapshot；只补这条链路必需的 rich manifest 字段。
4. **AR4（已完成）：** Tauri typed contract 与最小 UI：分析 source、选择 target、预览、安装。
5. **AR5（已完成）：** 切换 target 调用 Gate A 的真正重装；重启呈现当前 target；卸载后恢复游戏基线。

### 6.3 Gate B 完成定义

- 人工 fixture 能从一个 `f_equip` source slot 选择另一个受控 target。
- 预览显示最终 target/conflict 摘要，前端不拼接 `nativePC` 或 slot。
- 安装后 manifest 持久化 binding snapshot；删除 staging、重启应用后仍能恢复选择和安装事实。
- 切换 target 时旧 target 被安全清理/恢复，新 target 被安装；失败可回到切换前状态。
- 最终卸载后 game root 回到 ARMOR 安装前基线。
- parser、catalog、staging containment、conflict、reinstall、manifest、Audit Log 和 UI contract tests
  通过；真实 MHW smoke 仍只在专用测试环境执行。

## 7. 只允许的直接前置

T9 Rich Manifest 和 T10 Dependency/Preflight 不再作为可独立扩张的主线，只允许按阻断点取最小
切片：

| 前置 | Gate A 允许范围 | Gate B 允许范围 | 明确延后 |
| --- | --- | --- | --- |
| Rich manifest | reinstall entry-set replacement、write-state gate、recovery facts | replacement binding snapshot | 与当前闭环无关的顶层字段泛化 |
| Repair detection | 阻断不安全 install/uninstall/reinstall 所需状态 | target switch 前一致性判断 | 完整自动修复中心 |
| Preflight | fixture 所需的游戏目录/target/source/冲突检查 | `f_equip` path family、source/target/catalog gate | 通用依赖图平台和非核心 loader catalog |
| UI | 计划/任务/阻断/确认/结果 | source/target/preview/switch/uninstall | 批量、跨页、复杂筛选和视觉重构 |

若一个提议不能指出它解除 Gate A 或 Gate B 的哪项阻断，就不进入当前实施队列。

## 8. 暂停清单与恢复门禁

Gate B 已通过，因此表中的 Gate B 时间门禁已经满足；任务仍需经明确优先级复审才能恢复。复审已先
选择并完成 T19，随后按 T19 完成定义把 T18 恢复为当前主线；其他候选继续保持下表状态。

| 工作 | 当前状态 | 最早恢复条件 |
| --- | --- | --- |
| P7.2c installer cleanup | helper/installer 接入与 static/build gate 已完成；disposable VM runtime gate 待人工 | runtime acceptance 完成后再进入 Windows beta packaging |
| P7.2a 安装态后台备份验收与备份中心 | 暂停 | Gate B 后按发布风险重排 |
| T18 Mod 库分页 | 已恢复，Slice 1 为当前主线 | T19 完成；先落地 app-level query/filter/sort/page 兼容切片 |
| T17 第三方管理器批量迁移 | 设计保留，实施暂停 | Gate B 通过，且单包生命周期已 certified |
| T13 批量安装/卸载 | 暂停 | 单项 install/uninstall/reinstall certified 后重新设计队列语义 |
| T14 任务队列 UI | 暂停 | T13 恢复且出现多个真实长任务需求 |
| T12 Mod 详情完整版 | 仅允许 ARMOR 最小 target UI | Gate B 后恢复其余面板 |
| 存档 retention 时间/空间、完整 backups 页面 | 暂停 | Gate B 后重新评估 |
| 非阻断视觉重构/新主题 | 暂停 | Gate B 后恢复 |

以下情况可以打断暂停：

- Critical/Important 玩家数据安全或 secret 问题。
- main/CI/统一验证入口损坏。
- 当前 Gate 的直接编译、测试、packaging 或运行阻断。
- 维护者明确记录的新发布硬门禁。

## 9. 执行纪律

- 每个切片开始前写清楚玩家可观察结果、输入 fixture、失败注入和清理断言。
- 每个 PR/提交只解决一个可验收增量；避免顺手扩展共享框架。
- 优先复用现有 core/app/ports/infra/Tauri/feature-local 边界，不因重排进行无关重构。
- 自动化默认只用人工 fixture、fake ports、临时目录；真实游戏/真实任务操作必须单独授权。
- 任一破坏性动作必须先 revalidate，并保留 manifest/backup/recovery/Audit Log 证据。
- 文档状态只根据当前已执行证据更新，不因代码行数、计划完整或单元测试数量宣称 certified。

## 10. 当前下一项任务

CL0-CL4 与 AR1-AR5 已完成，Gate A/B 均标记为 `certified`。最终 artifact 的全新 Sandbox 证据已
覆盖首次 retarget 安装、Alpha -> Beta 真正重装 target switch、两次重启状态恢复、manifest 卸载和
逐字节 baseline 恢复，并确认 source/旧 target/staging/recovery 无残留。

Gate B 后优先级复审选定的
[T19 核心 Mod 生命周期产品化加固](CORE_MOD_LIFECYCLE_PRODUCTIZATION_PLAN.md) 已于 2026-07-18
满足完成定义。当前下一项固定为 **T18 Mod 库分页 Slice 1**：只实现 app-level query/filter/sort/page
类型、兼容聚合服务和 fake repository 测试；Tauri contract、前端分页 UI、可查询 read model、T17
和 T13 均保留到各自独立切片。

CL1 的已执行范围和证据见
[CL1 实施计划](superpowers/plans/2026-07-12-core-mod-lifecycle-cl1-implementation.md) 与
[CL0/CL1/CL2/CL3 验收基线](CORE_MOD_LIFECYCLE_CL0_ACCEPTANCE.md)。真正重装已由 CL3 标记为
`implemented`，并在 CL4 独立复审后随 Gate A 标记为 `certified`。

CL3 的实施入口见
[真正重装设计](superpowers/specs/2026-07-14-core-mod-lifecycle-cl3-true-reinstall-design.md) 与
[真正重装实施计划](superpowers/plans/2026-07-14-core-mod-lifecycle-cl3-true-reinstall-implementation.md)。
CL3 已执行证据与 CL4 独立 review/certification 共同完成 Gate A；AR1-AR5 已建立 Gate B 的模型、
catalog、分析、纯计划、staging、manifest binding、受控 UI 与真正重装/卸载闭环；最终 artifact 的
全新 Sandbox 复验与完整验证共同完成 Gate B certification。

## 11. 优先级重排提交边界（历史）

2026-07-12 的优先级重排提交只创建和同步规划文档，未修改 Rust、TypeScript、Tauri config、
数据库 migration、依赖、installer、fixture 或测试代码。后续 CL0 已按第 5 节新增 test-only harness；
该历史边界不限制后续切片，但 Gate A/Gate B 仍须按各自完成定义认证。
