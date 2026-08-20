# 项目任务状态快照

本文档记录 Helsincy Mod Manager 在 **2026-08-16** 的 Windows 项目任务全景，基准包含
Slice A-D 交付并由 Gate C 认证的 batch Sandbox 玩家路径，以及 WR-04 Gate D 认证的人工 Weapon
install/target switch/uninstall 玩家路径。此前包含
CLI-0A 至 CLI-1B、PR #211 至 #214 的工程治理，以及 QG-01/PR #215 合并后的 frontend
tests/workspace clippy 统一门禁。

本文件是一次证据快照，用于回答“当前已经具备什么、还缺什么、下一步先做什么”。持续变化的活跃
执行状态以 [Windows 自主迭代路线图](AUTONOMOUS_ITERATION_ROADMAP.md) 为唯一真源；[任务总纲](../TODO.md)
与 [路线图](ROADMAP.md) 维护产品 backlog 和里程碑，只在纵向切片合并或里程碑变化时同步。功能设计
和安全边界以对应专题文档、当前源码和测试为准。

## 状态口径

| 状态 | 含义 |
| --- | --- |
| 已完成 | 目标范围已落地并有当前自动化或验收证据 |
| Certified | 除实现与自动化外，已完成该 Gate 要求的独立复审和受控环境纵向验收 |
| 部分完成 | 核心子集已落地，但完整产品范围仍有明确缺口 |
| 仅设计完成 | 规格或实施计划存在，但运行时代码、安装器接入或验收尚未完成 |
| 暂停 | 尚未达到恢复门禁，不能因前置任务完成自动开工 |
| 待评审 | 尚未决定是否进入实施 |
| 缺真实环境验收 | fake/temp 自动化通过，但不能据此宣称真实 Windows 安装态可发布 |

## 总体结论

项目已经越过单纯脚手架阶段。Mod 导入、安装计划、安装、卸载、真正重装、manifest、备份、
回滚/恢复、Armor Retarget、Mod 库分页、第三方管理器批量迁移，以及 App/Task/Audit 日志和诊断页
已经形成可测试的后端链路。Gate A、Gate B、Gate C 和 Gate D 均为 `certified`。

核心 Mod 生命周期的批量能力已完成：T13-00 至 T13-06 的 sealed plan、runner、journal、CLI、窄
Tauri/typed API 已落地；T13-07 批量 UI 与 4 viewport smoke 已完成；T13-08 已在 disposable Windows
Sandbox 完成主链和受控 partial failure -> retry 补充链，Gate C 为 `certified`。CAT-01 装备数据治理、
WR-01 武器重定向设计、WR-02A 纯解析、WR-03A 人工 binary transformer、WR-03B
staging/InstallPlan/manifest/recovery 集成与 WR-04 受控 Tauri/UI/Gate D 均已完成；Gate D 于
2026-08-06 标记为 `certified`。LOG-01 Task/Audit retention、LOG-02 日志总空间上限与 LOG-03 Debug Log
均已完成；AR6/WR-02B 仍等待具有明确再分发权的审计数据。完整 catalog 未到位前只允许人工最小 developer/Sandbox seed；Sandbox
认证不开放 Production 写入。
Windows 后台存档保障的 SAVE-02 与 installer ownership cleanup 的 SAVE-03 安装态验收均已完成。
SAVE-04 玩家存档恢复代码、temp/artificial fixture 自动化、完整 verify、findings-first review 和
disposable Windows 人工验收均已完成并标记为 `certified`；SAVE-05 retention/备份中心也已完成实现、
完整验证、全 diff 自审和 disposable Windows synthetic 人工验收并标记为 `certified`。CLI-3A 三类
跨进程写入 admission 已于 2026-08-16 完成工程实现、本地完整验证、findings-first 全 diff 审查、
Ubuntu required CI 和 disposable Windows synthetic 多进程 gate，并标记为 `certified`；Production CLI
command-level admission 与开放仍未完成。
后端命令化已完成 CLI-2C：`hmm-runtime` 已承载真实共享 composition，
桌面端与固定 `--once` worker 复用同一装配；独立只读 facade 已支持游戏状态、扫描、已保存目录
校验、前置检查、安装计划/状态、恢复扫描/预览、备份历史、后台保护状态和诊断快照；仅 Sandbox
开放单项安装、卸载、真正重装和恢复 apply。备份创建/恢复、后台启停、诊断导出和任何 Production
写入仍不可达。

快照时：

- Slice A 基线为 `main@f60f29d`；后续任务以最新 `main` 创建独立 branch/worktree。
- PR #196 已合并；其 review 遗漏由 PR #197 补齐。
- PR #199 已完成 T17 Slice 4C，T17 全部切片已交付。
- PR #210 已合并并交付 CLI-0B/1A/1B；PR #211 至 #214 已完成 GOV-01 至 GOV-04。
- QG-01 已由 PR #215 完成 CI、review、评论处理和合并；统一 frontend tests/workspace clippy
  门禁已经成为主干基线。
- T13-00 已完成批量生命周期设计与规划契约；Slice A 已完成，Slice B 从 T13-01 sealed batch
  preview 开始。
- PR #222 已完成 T13-05 install batch CLI 子集，Slice B/C 已补齐批量卸载、真正重装与
  same-revision Armor switch contract；T13-07 由 PR #225 合并，Slice D/Gate C 已认证。CAT-01 和
  WR-01/WR-02A/WR-03A/WR-03B/WR-04 已完成，Gate D 已认证；LOG-01、LOG-02 与 LOG-03 已完成；
  AR6/WR-02B 仍等待已授权数据。

## 任务矩阵

| 任务 | 状态 | 当前边界 |
| --- | --- | --- |
| T1 恢复中心写入型 UI | 已完成 | 受控回滚预览、确认、taskId 跟踪和完成后刷新已落地 |
| T2 持久化方案 | 已完成 | SQLite 负责用户可编辑/关系数据，JSON 事实仓储继续保留 |
| T3 Mod 元数据后端 | 已完成 | overlay、Tauri command、typed API 和库查询合并已落地 |
| T4 分类标签 | 已完成 | CRUD、关联、typed API 和分类管理页已落地 |
| T5 Mod 信息面板 | 已完成 | 信息、分类、预览图、右键入口和保存刷新已落地 |
| T6 Profile 管理 | 已完成 | CRUD、活跃 Profile、生命周期与存档设置接入已落地 |
| T7 一键启动 | 已完成 | `GameLauncher` port、MHW:I Steam 启动和 UI 入口已落地 |
| Core Mod Lifecycle Gate A | Certified | 安装、卸载、真正重装、重启恢复、失败恢复和 exact baseline 已验收 |
| T8 存档备份 | 已完成 / SAVE-02 至 SAVE-05 Certified | 备份、后台核心、安装态保护、installer cleanup、玩家恢复、时间/空间 retention 与独立备份中心均已认证 |
| T9 Rich Manifest | 部分完成 | Gate 所需 metadata、状态消费、plan hash、binding snapshot 已落地；完整泛化和写侧门禁未完成 |
| T10 前置依赖检查 | 单项 lifecycle 已完成 / 平台待扩展 | MHW:I bundled rules、诊断查询、install/reinstall 的 blocked/warning decision、锁内重验和 UI/CLI 展示已落地；更多依赖类型、自动修复与完整平台仍未完成 |
| T11 装备 Retarget | Armor / Weapon 流程均 Certified | AR1-AR5 与 WR-04 Gate D 已认证；CAT-01、WR-01、WR-02A、WR-03A、WR-03B 已完成；完整 bundled armor/weapon catalog 仍受 AR6/WR-02B 数据门禁 |
| T12 Mod 详情完整版 | 部分完成、其余暂停 | Gate 所需替换目标 Tab 已完成；完整扩展范围未恢复 |
| T13 批量操作 | Certified（Gate C） | sealed plan/preview、batch runner、SQLite journal、retry、故障证据、Sandbox CLI、6 个窄 Tauri command（含 capability 投影）、typed API 与批量 UI 均已落地；4 viewport smoke、主链、受控 partial/retry、重启/recovery、批量卸载与 exact baseline 已验收 |
| T14 任务队列 UI | 暂停 | 依赖 T13 的真实多任务需求 |
| T15 Linux / Steam Deck | 本轮排除 | 不进入本轮任务、实现、验收或发布判断 |
| T16 Rise / Wilds | 远期 | 每个游戏需要独立 adapter 与设计 |
| T17 第三方管理器批量迁移 | 已完成 | Windows + MHW:I、狩技盒子来源、只导入不安装 |
| T18 Mod 库分页 | 已完成 | 后端权威分页、projection、freshness gate 和 10,000 条性能门禁已落地 |
| T19 生命周期产品化加固 | 已完成 | A1-L3：headless acceptance、日志/诊断与反馈 UI 均已交付 |
| T20 浮层动画共享基元 | 待评审 | 下次新增浮层前或出现第三处重复实现时再启动 |
| CLI 自动化入口 | CLI-2C 已实现；CLI-3A Certified；CLI-3B Ready | 已有只读 game/install/backup/diagnostics 命令，以及仅 Sandbox 的单项 install/uninstall/reinstall/recovery apply；5 分钟 token、双确认、写锁内重验、取消、失败恢复与 Production 双层拒绝已覆盖。CLI-3A 已接入 game/save/background 三类跨进程 admission、稳定错误码和共享 GUI/CLI/worker composition，并通过 Ubuntu CI 与 disposable Windows synthetic gate；Production command-level 写入仍未开放 |
| 工程治理 GOV-01 至 GOV-04 | 已完成 | DTO 测试外置、重装 lint 抑制清理、Tauri 契约防回归和治理检查加固已由 PR #211 至 #214 交付 |
| LOG-01 Task/Audit retention | 已完成 | Task 30 天、Audit 90 天；共享 runtime composition、capability-relative no-follow 清理、稳定 health code/count 与 temp-root junction 负测已落地 |
| LOG-02 日志总空间上限 | 已完成 | 128 MiB 默认/1 MiB 下限、Debug/Task -> App -> 30 天外 Audit 优先级、16 KiB Audit reserve、稳定 health/count 与 no-follow 复验已落地 |
| LOG-03 Debug Log | 已完成 | 默认关闭、持久化开关、受控 schema、7 天 retention、诊断 reader/export、runtime 重启与 no-follow 负测已落地 |

### Gate A / Gate B / Gate C / Gate D

Gate A 已覆盖认证导入记录到 `InstallPlan`、安装、重启、manifest 驱动卸载、真正重装、
失败恢复和 baseline 闭环。Gate B 已覆盖 Armor source 分析、目标选择、首次安装、同 revision
目标切换、两次重启状态恢复和 manifest 卸载。Gate C 已覆盖批量安装/卸载/真正重装、Armor target
switch、partial result/retry、重启持久化、recovery 归零和 exact baseline。Gate D 已覆盖人工 Weapon
source 分析、initial install、same-revision `one001 -> one002` true reinstall、两次 GUI 重启、当前 target
持久化、manifest 卸载、recovery 归零和 10 文件/316 bytes exact baseline。

三者的共同安全边界保持不变：

```text
analyze
  -> build InstallPlan
  -> conflict / preflight
  -> backup
  -> commit
  -> manifest
  -> rollback / recover
```

前端和未来 CLI 都不能绕过这条链路直接复制、覆盖或删除游戏文件。

### T17 与 T13 的边界

T17 已实现第三方 Mod 管理器的批量迁移，包括只读来源扫描、分页预览、sealed selection、显式决定、
安全物化、partial success、服务端重试和按 `taskId` 的进度/结果。它默认只把 Mod 导入 HMM，
不会安装、启用或写游戏目录。

T13 才是批量安装/卸载/真正重装。T17 完成不代表 T13 产品能力已完成，也不能借 T17 的批处理编排绕过单项安装的
`InstallPlan`、manifest、backup、rollback、锁和审计语义。

## 装备重定向数据

Armor Retarget 的流程认证与 catalog 完整度是两个状态：

- AR1-AR5 已证明 armor source 分析、结构化 slot 改写、staging、InstallPlan、binding snapshot、
  真正重装 target switch、重启恢复和 manifest 卸载安全链。
- 当前 bundled armor catalog 仍只有最小稳定 seed，不代表完整防具目标已经进入产品。
- 本地候选防具数据有 272 条安全相对路径；display name 存在重复，因此名称不能作为稳定 ID。
- 本地候选武器数据覆盖 14 类、3125 个展示名称，但只有 603 个唯一目标路径；同一路径最多对应
  48 个名称，必须建模为稳定 target + aliases，而不是重复安装目标。
- 武器目标属于独立的 MHW:I weapon catalog/path parser/adapter。不能塞进
  `MhwArmorReplacementAdapter`，也不能让前端解析 `nativePC/wp`。
- WR-04 已认证人工 developer/Sandbox weapon seed 的 Tauri/UI 与完整生命周期；这不等于完整 production
  weapon catalog 已获许可或开放，WR-02B 仍保持 `blocked-external-data`。

两份候选数据都不是运行时信任源。接入前必须完成 schema、路径安全、大小写碰撞、重复项、stable
ID、alias/localization、dummy 条目、版本和可分发权利审计，再生成 versioned bundled artifact。

## Steam 多账号存档

### 已实现

MHW:I adapter 和基础设施会扫描本机 Steam 根下的：

```text
userdata/<account_id_32>/582010/remote
```

发现流程具备以下边界：

- 唯一高置信候选可以自动保存到当前 HMM Profile。
- 多个候选时会推荐最近修改项，但必须由用户明确确认。
- 真实路径和 account id 保留在后端 pending cache。
- 前端只接收 opaque candidate id、脱敏标签和经过白名单校验的公开资料摘要。
- 网络资料补全失败只降低展示信息，不阻断本地候选选择。
- 测试使用 temp Steam root、fake transport 和人工 XML，不读取真实 Steam 账号或存档。

### 未实现或不在范围

- Steam Cloud 同步。
- Steam OAuth、API key 或登录态接入。
- 跨设备存档同步。
- Steam 账号与 HMM Profile 自动绑定。
- Steam Cloud/账号协议层恢复；SAVE-04 只恢复当前 HMM Profile 已确认的本地存档目录，不是 Steam 云存档客户端。

因此当前能力是“本机多账号存档目录安全发现与显式选择”，不是 Steam 云存档客户端。

## 存档备份

### 已实现

- 手动备份和客户端运行期自动备份。
- zip archive、backup manifest、SQLite 历史记录和稳定备份 ID。
- 默认 app data 备份目录和用户自选目录。
- 每个 Profile 独立配置与历史。
- 按数量 retention。
- `save_backup.*` task、`taskId`、进度、取消和 task log。
- scheduler state、跨进程 lease、heartbeat 与游戏运行保护。
- 游戏运行中或运行状态未知时延后，不错误启动备份。
- 手动、自动、pre-install backup、retention pruning 等 Audit Log。
- SAVE-04 manifest-backed 玩家存档恢复：`preview_save_restore` / `start_save_restore_task`、5 分钟
  preview token、统一 Modal、独立 `TaskKind::SaveRestore`、严格 taskId/phase listener、默认开启并按
  Profile 持久化的 pre-restore 安全备份、独立 `pre-restore/` 目录、共享 game/profile 写锁、目录交换、
  rollback/recovery-required 与 post-commit evidence degradation。
- migration 012 为既有 Profile 默认开启 `preRestoreBackupEnabled`，普通数量 retention 排除
  `trigger = pre_restore`；自动测试只使用 temp/artificial fixture。

### 未完成

- 按时间/空间的存档备份 retention。
- 独立 `features/backups/` 备份中心页面。
- 真实玩家数据环境验收。普通测试只证明 temp/fake 链路。

## Windows User 级后台自动备份

### 已实现的软件核心

- 固定参数的 headless worker：`hmm-save-backup-worker.exe --once`。
- worker 与 GUI 复用 scheduler、backup、lease、heartbeat 和 Audit Log 链路。
- user 级 Windows Scheduled Task 注册、更新、移除和 exact read-back。
- register 写入后先由 Rust 复验完整 read-back 与 canonical worker，再由 infra 内部受控操作双重读回并
  首次启动 exact-owned task；启动失败或 TOCTOU 漂移时 fail closed，inspect 仍保持纯只读。
- Scheduled Task action 指向同目录 sibling worker，不接受任意 path/profile/lease 参数。
- 登录触发延迟 1 分钟、每 15 分钟执行、单次上限 1 小时、`IgnoreNew`、`StartWhenAvailable`，
  不依赖网络、不唤醒机器，也不因电池状态停止。
- 全局 Settings 唯一开关、Profile 只读保护状态和统一退出保护。
- `starting` / `protected` / unhealthy 等 fail-closed 健康派生。
- enable 后 5 分钟内允许 `starting`；`protected` 需要 exact registration，以及不早于本次启用且
  45 分钟内的新鲜 heartbeat。
- ownership conflict、permission、drift、stale/future heartbeat 和 unsupported 平台的稳定错误。

### 安装态验收已完成

P7.2a 已于 2026-08-07 在一次性 Windows Sandbox 完成完整安装态链路：

```text
安装产物中的 sibling worker
  -> 注册真实 user Scheduled Task
  -> 人工触发
  -> 写入 fresh heartbeat
  -> 幂等 unregister / cleanup
```

安装 bundle 中存在主程序 sibling worker；真实 user Scheduled Task 的 initial missing、register exact、
幂等 register、人工 Run、新鲜 heartbeat 和幂等 cleanup 均有证据。有效 worker 运行还完成了一个
1 文件 synthetic automatic backup。Terminal A 未接收 stdin acknowledgement，最终 unregister leg
使用 dedicated ownership-checked cleanup smoke 完成；Task Scheduler UI 刷新确认无残留。Sandbox 已
销毁，宿主 synthetic fixture 已移入回收站。验收应用为 `0.1.0-alpha.0`，Windows 10 Enterprise build
`19041`，架构 `AMD64`；未使用真实游戏、Steam userdata 或玩家存档。

ignored smoke 和 fake runner 自动化仍不能单独代替安装态验收；本次 `certified` 结论来自上述真实
disposable Windows 链路，不开放 Production CLI 写入，也不代表 P7.2c disposable VM runtime gate
已经完成。

### P7.2c installer cleanup runtime gate 已认证

P7.2c 的 installer cleanup 与首次运行修复已于 2026-08-14 完成 disposable Windows Sandbox runtime
gate：

- 独立、无参数、ownership-checked cleanup helper 与双 Windows sidecar。
- NSIS `PREUNINSTALL` 接入及非零 helper exit code 的 fail-closed 处理。
- WiX `CustomActionRef`、pre-`RemoveFiles` custom action 和最终 MSI 反编译证据。
- helper/registry、sidecar、NSIS/WiX 静态测试和 debug artifact 构建。
- `0.1.10` 尾部矩阵发现 NSIS 重新启用后 task 已 exact 注册但没有立即产生本轮 heartbeat；现已在 Rust
  exact read-back 后增加内部首次启动阶段，并在启动前双重复验 owned task，避免按名字盲启或伪报已保护。
- 最终 `0.1.12` NSIS/WiX debug artifact 完成版本、三个 sibling、NSIS PREUNINSTALL、MSI
  `RunInstallerCleanup=3499` / `RemoveFiles=3500`、卸载条件和固定 `1722` 文案审计；WSB 只映射
  synthetic save/backup fixture。
- NSIS 重新启用后台保护后自动产生 fresh heartbeat，Settings 无需手动检查即收敛为“已保护”；每日
  cadence 对 1 文件 synthetic save 生成 1 个 ZIP 和 1 个 manifest。
- owned exact interactive/silent 卸载清理安装目录与 owned task，并保留 synthetic save、ZIP 与
  manifest；foreign task 保持 Ready 且 marker 不变。
- owned task 为 Running 时 direct `_?=` 诊断模式返回稳定 `20`，安装目录、payload 和 task 完整保留；
  任务回到 Ready 后先覆盖修复至 4 文件，再用正常 NSIS wrapper 卸载，安装目录和 owned task 均消失。
- `_?=` 会关闭 NSIS 临时副本/self-delete，只允许用于 blocked exit-code 诊断；成功、missing 和 recovery
  路径必须使用正常 wrapper，并用安装目录、task 和 synthetic 数据 read-back 判断结果。

卸载规则继续要求保留 foreign task；owned task 若处于 running/unknown 状态必须 fail closed。日常自动化
仍不得操作真实 Scheduled Task，后续回归继续使用 disposable Windows 环境。SAVE-03 标记为
`certified`；SAVE-04 与 CLI-3A 均已完成各自独立设计、实现和验收，后续 Production 写能力仍按 CLI-3B
逐 command 复核，不因共享 admission 已认证而自动开放。

## 日志、审计与诊断

日志系统不是单一文件，而是四类目标能力和一条受控诊断导出链路。

| 能力 | 当前状态 | 说明 |
| --- | --- | --- |
| App Log | 已实现 | 安全 JSONL、UTC 日轮转、14 天 retention、白名单字段和 health code |
| Task Log | 已实现 | 每个 task 独立 JSONL，与 progress 共用 taskId/kind/status/phase/current/total |
| Audit Log | 已实现 | 高风险操作、后台注册/worker/退出 override 和诊断导出均有最小审计 |
| Debug Log | 已实现 | 默认关闭；显式开启后写受控 JSONL，7 天 UTC retention，并纳入 diagnostics 与总空间预算 |
| 日志总空间预算 | 已实现 | 可配置 128 MiB 默认预算；按 Debug/Task、App、30 天外 Audit 收敛并投影稳定健康状态 |
| `/diagnostics` 页面 | 已实现 | App/Debug/Task/Audit 分类读取，单类失败不阻断其他安全类别 |
| support diagnostics export | 已实现 | 平台摘要和固定上限 App/Debug/Task/Audit 数据，用户主动导出、默认脱敏 |

### 已落实的安全边界

- Task Log 不记录自由文本 message、原始 error、result ref 或路径。
- Audit Log 只记录短 ID、聚合计数、稳定 operation/result/error code。
- 不记录完整 home/game/save 路径、用户名、Steam ID、token、cookie、真实存档或第三方 Mod 内容。
- Audit 写入失败不会篡改已经提交的玩家文件事实；`report_after_commit` 会把证据降级显式暴露为
  `audit_write_failed_after_commit`。
- 诊断快照中单类读取失败只返回稳定状态；完整导出若任何必需类别失败则整体失败，并 best-effort
  写最小失败审计。

Debug 设置只暴露 `{ enabled }`；禁用时不创建目录，损坏 settings 默认关闭。事件拒绝、写入失败和
retention 失败通过独立稳定 health/count 投影，不会改变安装或恢复事实。

## CLI 自动化

### CLI-0A / CLI-0B / CLI-1A / CLI-1B / CLI-2A / CLI-2B / CLI-2C / CORE-PREF-01 已实现

- workspace 已新增 `hmm-runtime` 与 `hmm-cli`，CLI dependency tree 不包含 Tauri。
- `hmm runtime status` 支持 `human|json|jsonl`、Production/Sandbox 环境和稳定退出码。
- CLI-0A human/help/error 输出统一无 ANSI，`--no-color` 保留为稳定全局参数。
- Production 禁止 `--data-dir`，写命令策略固定为 `disabled`。
- Sandbox 要求显式绝对数据根并拒绝 root、`.` 和 `..`；只读命令不创建 marker，CLI-2C lifecycle
  写命令才显式申请受控 write capability。
- JSON/JSONL 使用 `hmm.cli/v1`；机器模式的 runtime/parse 错误使用稳定脱敏 envelope。
- `HmmRuntime` 已装配真实 repositories/services、`TaskManager` 与 game/profile 写锁。
- Tauri `AppState` 已变为 runtime 薄包装；固定 `--once` worker 直接构造 runtime。
- `TaskProgressObserver` 已逐阶段接入 install/uninstall/reinstall/recovery runner；Tauri event、
  CLI JSONL、Task Log 与 queued App Log 共享 task id、phase 和顺序事实。
- `hmm game status|scan|validate|prerequisites --game mhw` 已开放，支持 `human|json|jsonl`。
- `ReadOnlyGameAutomation` 不构造完整 `HmmRuntime`，避免 SQLite migration、projection/recovery 等
  初始化写副作用。
- Sandbox 只扫描 `<data-dir>/fixtures/steam`，并拒绝配置、Steam library 或候选逃逸
  `<data-dir>/fixtures`；输出不含路径、规则 path、自由文本或 Steam ID。
- prerequisite 查询使用无 seed 的只读 repository；四命令已有 binary no-write 树快照测试。
- `hmm install plan|status|recovery scan|recovery preview` 已开放，复用 app services 和 MHW:I
  adapter；只返回安全相对 target、稳定状态/code 与聚合计数。
- `ReadOnlyInstallAutomation` 不构造完整 runtime；Mod catalog 查询不创建 lock、不落盘迁移，
  manifest/recovery readers 不创建目录，Sandbox state/game roots 在读取前做 canonical containment。
- install 四命令已有 human/json/jsonl、路径型 ID、脱敏、parser write gate 和整树 no-write 测试。
- `hmm backup list|background status` 只读取已 checkpoint 且没有 WAL/SHM sidecar 的 SQLite；
  immutable/read-only/query-only opener 读取历史、scheduler/settings 和 fake/Production registry
  inspect。任一 sidecar 存在时以 `backup_database_unavailable` fail closed，不 checkpoint、修复、
  创建/迁移/seed 或修改 DB/WAL/SHM，也不返回 archive、manifest、存档/备份路径、Steam ID、
  worker/lease/task 平台细节。
- `hmm diagnostics snapshot` 复用 reader-only 页面快照服务，只返回 bounded platform summary、
  App/Task/Audit 分类状态和计数，不返回日志正文、来源文件名、Audit fields 或 export path。
- backup/diagnostics 三命令已有 human/json/jsonl、人工 SQLite/fake registry/fixed clock/log fixture、
  parser write gate、敏感 canary 与整树 no-write 测试。
- `hmm install apply|uninstall|reinstall` 与 `hmm install recovery apply` 已仅在 Sandbox 开放；
  ready preview 签发 5 分钟 opaque token，提交要求 `--commit --yes`，锁内重建并重验计划、
  capability 和 recovery facts；manifest/recovery 内容即使计数不变也会使旧 token 失效。
- lifecycle 写入复用既有 application runner、InstallPlan、backup、manifest、rollback/recovery、
  Task/Audit Log 和共享写锁；Ctrl+C 通过 TaskManager 协作式取消，第二次中断不伪造 cancelled。
- install/reinstall preview 和桌面 runner 复用同一个 app-level prerequisite decision provider。
  required missing、rules unavailable/corrupted 与无法证明的状态为 blocked 且不签 token；
  `signature_unverified` 为显式 warning。token 与锁内重验绑定 status、stable codes 和 rules version。
- CLI/Tauri/frontend 只投影 `prerequisiteDecision`，不返回 issue path、display message、配置正文
  或本地绝对路径，也不复制 MHW:I adapter 规则。
- `hmm install batch plan|apply|result|retry` 已在 Sandbox 接入 T13-02 app service；plan 返回脱敏
  projection 和短期 opaque `previewToken`，apply 需要 `--commit --yes --preview-token`，并在构造
  runtime/journal 前先做只读 stale 验证，seal 时再重验。apply/retry 的最终 admission 在 SQLite
  `BEGIN IMMEDIATE` 短事务中按 game/profile 检查 active attempt 并原子进入 queued，两个独立连接
  竞争时最多一个成功；retry 竞争失败只安全回收未执行的 sealed retry attempt。result 只读取指定
  batch/attempt，不被 sibling active attempt 阻断。partial result 使用退出码 `5`；当前 result 返回
  完整 bounded snapshot，尚未实现 cursor/limit。JSONL apply/retry 先输出 parent terminal event，
  再输出 command result，两者共用同一 taskId。遗留 `queued/running/stopping` attempt 继续阻断新
  写入并返回 `batch_attempt_reconciliation_required`，不会自动续跑或收敛。
- Production 四条 lifecycle 写命令和 batch 写命令在 CLI policy/runtime 双层拒绝；backup create/restore/background
  enable|disable 和 diagnostics export 仍未开放。

### 下一步

T13 Slice A-D 已完成并通过 Gate C。CAT-01 已交付装备候选输入 schema、validator、stable ID、
alias/localization、dummy/隐藏条目策略、版本与 provenance/licensing 门禁。WR-01 已完成设计；WR-02A
已交付 14-family/part registry、严格 path/source closure parser 和纯内存 catalog-source validator。
WR-03A 已交付人工 MOD3/MRL3 有界 preflight、pair compatibility、纯 transformer 与脱敏 digest/error
projection。WR-03B 已交付 versioned registry、transform-aware staging、InstallPlan/manifest/recovery/
Audit facts 与 temp-root exact-baseline 生命周期。AR6/WR-02B 因缺少明确可再分发的审计数据而 blocked；
WR-04 受控 Tauri/UI/Gate D 已认证；完整 catalog 未到位前仍仅使用人工 Sandbox seed。LOG-01、LOG-02、
LOG-03、SAVE-02、SAVE-03、SAVE-04 与 SAVE-05 已完成并认证。CLI-3A 已实现
`background-registration-write`、`save-profile-write` 与 `game-profile-write`，并接入 GUI、Sandbox CLI
和固定 worker；本地完整验证、findings-first 全 diff 审查、Ubuntu required CI run `31910573714` 与
2026-08-16 disposable Windows synthetic gate 均已通过，现标记为 `certified`。

backup immutable opener 当前没有跨进程只读快照锁；需要一致结果时先关闭桌面端。后续如果要支持
GUI 与 CLI 并行查询，应单独设计 snapshot/admission，而不是放宽 WAL/SHM fail-closed 门禁。

Production 写命令仍依赖 CLI-3B 的 command-level capability、token、Audit、锁内重验和 Windows 验收；
CLI-3A 的共享互斥基础不自动解锁 backup、restore、background registration 或 diagnostics export。

## 验证证据

本轮 CLI-1B install 只读子切片实际执行并记录：

- `cargo test --workspace`：通过。
- `cargo check --workspace`：通过。
- `cargo clippy --workspace --all-targets -- -D warnings`：通过。
- `cargo test -p hmm-runtime install_automation --lib`：7/7 通过。
- `cargo test -p hmm-infra read_only_mod_import_catalog --lib`：3/3 通过。
- `cargo test -p hmm-cli`：6 个 unit 与 23 个 binary contract 通过。
- policy、文档链接、禁入文件、secret、空白、文件大小、增量 rustfmt 与 `git diff --check` 通过。
- 上述 CLI/runtime 自动化只使用 temp/fake/人工 fixture，不执行 Production install/recovery 查询，
  不读取真实游戏、Steam、AppData、Mod、存档或 Scheduled Task。

CLI-1B backup/diagnostics 子切片当前聚焦证据：

- `cargo test -p hmm-infra read_only_open --no-fail-fast`：6/6 通过。
- `cargo test -p hmm-app support_diagnostics --no-fail-fast`：7/7 通过。
- `cargo test -p hmm-runtime backup_automation --no-fail-fast`：3/3 通过。
- `cargo test -p hmm-runtime diagnostics_automation --no-fail-fast`：Windows 可运行的 1/1 通过；
  Unix-only symlink escape fixture不在 Windows 账户执行。
- `cargo test -p hmm-cli --no-fail-fast`：6 个 unit 与 30 个 binary contract 通过。
- 自动化只使用 temp SQLite、fake registry/fixed clock 和人工日志；未执行 Production
  backup/diagnostics 命令，未读取真实 AppData/日志/存档，也未查询或修改真实 Scheduled Task。

CLI-2A/2B/2C 与 CORE-PREF-01 当前聚焦证据：

- `cargo clippy -p hmm-app -p hmm-runtime -p hmm-cli --all-targets -- -D warnings`：通过。
- `cargo test -p hmm-cli --test cli_contract`：36/36 通过；`cargo test -p hmm-cli --lib
  cancellation`：5/5 通过。
- `cargo test -p hmm-app install_task --lib`：32/32 通过；`cargo test -p hmm-app reinstall_task
  --lib`：16/16 通过。
- `cargo test -p hmm-runtime lifecycle_automation --lib`：3/3 通过；
  `cargo test -p hmm-runtime composition::core_mod_lifecycle_tests`：9/9 通过。
- E2E 只使用 TEMP/fake/artificial fixture，未读取或写入真实 Steam、游戏、存档、AppData、
  Scheduled Task 或第三方 Mod。

### T13-02 批量安装当前聚焦证据

- `cargo check -p hmm-app -p hmm-infra -p hmm-runtime`：通过。
- `cargo test -p hmm-app batch_install --lib -- --nocapture`：26/26 通过。
- `cargo test -p hmm-app task_manager --lib -- --nocapture`：15/15 通过。
- `cargo test -p hmm-app install_task --lib -- --nocapture`：37/37 通过。
- `cargo test -p hmm-app reinstall_task --lib -- --nocapture`：16/16 通过。
- `cargo test -p hmm-app install_tests --lib -- --nocapture`：82/82 通过。
- `cargo test -p hmm-core batch --lib -- --nocapture`：9/9 通过。
- `cargo test -p hmm-infra batch_lifecycle_repository --lib -- --nocapture`：14/14 通过。
- `git diff --check`、文件大小、secret、禁入文件和文档链接门禁：通过。
- `powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify.ps1`：第二轮完整通过；
  包含 policy、hygiene、frontend typecheck/lint/404 tests/build、Rust workspace
  tests/check/clippy。
- 测试只使用 fake/temp/人工 fixture，未读取真实游戏、Steam、AppData、存档或第三方 Mod。

### T13-05 Sandbox batch CLI 当前聚焦证据

- `cargo test -p hmm-cli --test cli_contract batch -- --nocapture`：14/14 通过，覆盖 Production
  写入拒绝、plan 脱敏 projection、`--commit --yes`/preview token 门禁、非法 ID 零副作用、
  active result 安全可读、写入口 fail closed、跨连接原子 admission、跨进程 apply/result/retry、
  JSONL parent terminal event、批量卸载 partial、跨 revision reinstall/uninstall、same-revision Armor
  switch、stale preview，以及 active install recovery 对批量重装的全局零副作用阻断。
- `cargo test -p hmm-cli --lib --no-fail-fast`：28/28 通过；terminal partial result 返回 exit `5`，
  legacy `running` result 查询保持 exit `0`。
- `cargo test -p hmm-app --lib --no-fail-fast`：420/420 通过；
  `cargo test -p hmm-runtime --lib --no-fail-fast`：61/61 通过。
- `powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify.ps1`：最终候选完整通过；
  包含 policy、hygiene、frontend typecheck/lint/tests/build、Rust workspace tests/check/clippy。
- stale preview 在构造 `HmmRuntime` 前由只读 facts service 验证；失败时沙盒目录快照不变，不创建
  `hmm.db`、journal 或 projection。上述测试均只使用 temp/fake/人工 fixture。

### CLI-3A 跨进程写入 admission 候选证据

- `cargo test -p hmm-tauri install_recovery_write_admission_errors_preserve_stable_codes_without_paths -- --nocapture`：通过。
- `cargo test -p hmm-infra --test cross_process_write_admission -- --nocapture`：Windows 本机 `4 passed / 1 ignored`，覆盖同 scope busy、不同 scope/profile、取消、owner 强退恢复和非法 namespace；ignored helper 仅由测试进程调用。
- `cargo check -p hmm-infra -p hmm-app -p hmm-runtime -p hmm-cli -p hmm-tauri --all-targets`：通过。
- `cargo test -p hmm-runtime`：76 个 lib tests 与 1 个 integration test 通过。
- `cargo test -p hmm-cli`：29 个 lib tests 与 52 个 CLI contract tests 通过；Production 写命令继续被 policy/runtime 双层拒绝。
- `cargo test -p hmm-app --no-fail-fast`：通过；覆盖 install/reinstall/recovery、backup/retention/restore 和 background 接入的 busy/error projection 与锁内重验。
- `powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify.ps1`：最终候选完整通过，终态为 `Verification passed.`；覆盖 workspace check/test/clippy、frontend lint/typecheck/tests/build 和仓库 hygiene 门禁。
- PR #226 的 Ubuntu required CI run `31910573714` 终态为 success；core Mod lifecycle acceptance 与 full verification 均通过，实际覆盖 Unix file-lock、capability-relative no-follow 和 path replacement 回归。
- findings-first 全 diff 审查未发现 Critical 或 Important 问题；锁顺序、guard 内事实重验、Production 写命令不可达和日志脱敏边界均已复核。
- Windows 本机结果不用于声称覆盖 Unix 分支；该分支已由上述 Ubuntu required CI 实际验证。disposable
  Windows gate 已覆盖 helper timeout/cancel/abandoned owner、CLI game scope 竞争与释放、GUI/worker
  save scope busy fail-closed 与释放后备份增长、background registration enable/disable 双向竞争。
- 最终 worker gate 输出 `worker-preflight=passed`、`worker-busy-count-unchanged=true`、
  `worker-release-backup-increased=true`、`worker-blocked-exit=0`、`worker-released-exit=0`；终态为
  `gate-final=ready-for-review`、owned task `Ready`、archive/manifest `3/3`、live gate processes `0`。
- 自动化只使用 temp/artificial fixture，不读取真实游戏、Steam、玩家存档、AppData 或真实 Scheduled Task；本轮 SAVE-05 Alpha retention/备份中心人工证据已独立认证，不重复纳入 CLI-3A 测试。

### T13-07 / T13-08 最终验收证据

- 最终 artifact SHA-256：
  `08EF5FF15DAFDC00790C0975FAA160C792AF487D47C186271E93D09D84AB8C8D`。
- T13-07 在 `1440x900`、`1366x768`、`1280x800`、`480x800` 实际窗口尺寸完成 preview/result smoke；
  480x800 stacking、浅色主题面板和批量终态后列表自动刷新问题均已修复复验。
- Gate C 主链完成 batch install、GUI restart、Alpha v2 true reinstall、Armor target switch、再次 restart、
  recovery 归零、batch uninstall 和 9 文件/212 字节 exact baseline。
- 受控 partial/retry batch `batch-94eedbc4-3006-4f76-aa39-b0d1bae71650` 的 attempt 0 task
  `install-1785897638158-0` 为 0 成功/1 失败/2 跳过，Alpha=`install_commit_failed`，Armor/Beta=
  `batch_stopped`，三项均 retryable；attempt 1 task `install-1785897713997-0` 为 3 成功。
- 最终卸载 batch `batch-aab2d50e-7412-4694-9a7f-5433eed50b89`、task
  `install-1785897949309-0` 为 3 成功；manifest entries/bindings 与安装状态投影均为空，Recovery Center
  全部归零，backup/recovery 标准目录为空且无 staging。补充 baseline 为 10 文件/243 字节，路径、
  大小和 SHA-256 差异均为 0；所有 attempt `evidence_health_degraded=false`。
- 两条验收链均只使用 disposable Windows Sandbox、人工 fixture 与宿主映射临时根，没有读取真实游戏、
  存档或第三方 Mod。

### WR-04 / Gate D 最终验收证据

- 最终 `hmm-tauri.exe` 为 24,209,408 bytes，SHA-256
  `156C42118C6620D803C1611397C55C1847AB782BB6505CD713C56A17398EA2AF`；完整 `verify.ps1` 通过，
  Tauri 188 passed / 1 ignored，workspace tests/check/clippy 与前端 typecheck/lint/test/build 均通过。
- 人工 archive SHA-256 为 `85CA8FB179CCAAA8B3E22D13DE8E3F2D46E0135A09CA8C5F258230AE31D4DACF`；
  initial install task `install-1785952182807-1` 为 `install.retarget.completed`，target switch task
  `install-1785953522595-0` 为 `install.reinstall.completed`，uninstall task `install-1785955067791-0`
  为 `install.uninstall.completed`，三条 Audit Log 均为 success。
- `one001 -> one002` switch 为真正重装：added 2、stale 2、retained 0、replaced 0；两次 GUI 重启后
  installed target 均从持久化 manifest/binding 正确恢复。最终 manifest `entries=[]`、
  `replacement_bindings=[]`，Recovery Center、backup、recovery、reinstall-recovery、retarget-staging 均为 0。
- 最终 game tree 为 10 文件/316 bytes，missing 0、extra 0、size/hash mismatch 0；未读取真实游戏、
  存档、AppData 或第三方 Mod。仓库外证据 bundle 名为 `hmm-wr04-gated-20260805-2315`。
- light 主题已覆盖 1440x900/1366x768/1280x800/480x800，dark 覆盖 1280x800/480x800，system
  覆盖 1366x768；replacement modal 的层级、滚动、warning、按钮与路径脱敏通过。

2026-07-30 在独立 clean Windows QG-01 worktree 对当前治理 diff 实际执行：

- `powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify.ps1`：通过；包含 policy/docs、
  入口契约、frontend typecheck/lint/tests/build，以及 workspace tests/check/clippy；frontend
  tests 为 403/403。
- 前端失败变异为 403 pass / 1 fail，入口退出 1 且未进入 build/Rust；fixture 已删除。
- clippy `useless_vec` 失败变异使入口在最终 clippy 退出 1；fixture 已删除。
- frontend build 仍有约 597 KB 主 chunk 超过 500 KB 的既有非阻断 warning。
- 测试使用 temp/fake/人工 fixture；开发 sidecar、`node_modules`、`target` 和 `dist` 均为 ignored
  生成物，不进入本次文档提交。

真实 Windows Scheduled Task、安装态 sibling worker、heartbeat 和 installer cleanup 没有在日常
开发账户中执行；这些检查只能在一次性账户或 disposable VM 中完成。

## 已知文档与治理偏差

- `SAVE_BACKUP_BACKGROUND_SCHEDULER_CORE_PLAN.md` 仍写 P7.2b 未实现，与较新的自动化设计、
  `TODO.md`、源码和测试不一致。
- T13-02 当前不自动收敛启动级遗留非终态 `queued/running/stopping` attempt；T13-05 CLI
  在 apply/retry/new apply 保留只读预检，并以 SQLite 原子 scope admission 最终阻断并发新写入。
  result 只读取指定 batch/attempt，保留遗留状态的安全诊断能力。后续若要自动 reconciliation，必须
  单独设计安全终态、证据和恢复验收；Sandbox batch admission 也不等于 Production 通用写 admission。
- Windows 后台保护的 fake/temp 自动化不能替代安装态 VM 验收；SAVE-02 与 SAVE-03 已用 disposable
  Windows 安装态链路补齐保护与 installer cleanup 证据，后续回归仍不能只凭 bundle 中存在 sibling
  worker 就标记完成。
- Profile 删除的破坏性确认目前在卡片内联展开，而非共享悬浮确认层；这是非阻断 UX 债务。SAVE-04
  存档恢复已经使用统一 Modal，并默认先创建独立 `pre-restore/` 安全备份，不能据此视为该旧债务已解决。
- WR-04 遗留的 UI/诊断缺陷已全部关闭：空 NexusMods ID 显示 `null` 已修（`ModDetail.nexusModId`
  改为显式可空，表单统一走 `formFieldFromOptional`，并补空值往返回归测试）；
  `weapon_binary_pair_incompatible` 等 22 个武器稳定码已在
  `src/features/replacements/replacementErrorText.ts` 按可执行行动分组映射为具体文案并附诊断码。
  防复发由三层闸门承担：前端 `Record` 穷尽映射（码表内缺文案则 `tsc` 失败）、
  `hmm-games-mhw/tests/weapon_error_code_contract.rs`（Rust 新增变体则编译失败）、
  以及 `replacementErrorCodeContract.test.mjs`——只有第三层跨语言比对 Rust `code()` 与前端码表的
  集合，能挡住"补了 Rust 却没补前端文案"这种两侧各自全绿、用户却退回兜底提示的情况；
  该测试同时按命名约定扫描 `replacement_commands.rs`，保证通用码也不漏文案。
  主题入口已加入设置页"界面偏好"，顶栏文字标签限 ≥1200px 显示以避开 1060px 成对断点的余量。
  更早的"无元数据 Mod 名称回退为 `mod-import-*`"与"宽度不超过 1360px 时 `.window-tools` 被隐藏"
  两项已在 `0.1.0-alpha.0` 真机验收后修复：前者改为继承压缩包文件名（净化规则上提到 `hmm-core`
  与元数据路径共用），后者把隐藏阈值下调到 1060px 并要求与状态栏两列收缩同断点。
- GOV-01 至 GOV-04 已完成；后续变更需保留对应文件大小、secret、CODEOWNERS 和 Tauri
  command 契约回归门禁。

## 建议执行顺序

1. 保持遗留非终态 `queued/running/stopping` attempt 的 fail-closed 门禁；如需自动 reconciliation，
  先单独完成安全设计和验收。
2. 保持 CAT-01 provenance/licensing 门禁；未达到 `bundled_eligible` 且未经人审的数据不得提交为 catalog。
3. WR-04 Gate D 已认证；完整 catalog 未到位前继续只使用人工最小 seed，AR6/WR-02B 在获得明确可再分发的审计数据后恢复。
4. T17 只做条件式脱敏真实来源 smoke 或明确 bugfix，不重新实现。
5. LOG-01、LOG-02、LOG-03、SAVE-02、SAVE-03、SAVE-04 与 SAVE-05 已完成并认证；继续保持完整
   verify、findings-first review 和人工 gate 证据可追溯。
6. CLI-3A 的本地完整 verify、findings-first review、Ubuntu required CI 与 disposable Windows gate 均已
   完成并认证；进入 CLI-3B，按 command 复核 capability、token、Audit、锁内事实和 Windows 验收。
   Production 写入仍不能仅凭跨进程 guard 绕过这些门禁。

完整 task 依赖和合并门禁见 [Windows 自主迭代路线图](AUTONOMOUS_ITERATION_ROADMAP.md)。
