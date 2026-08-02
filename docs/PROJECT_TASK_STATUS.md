# 项目任务状态快照

本文档记录 Helsincy Mod Manager 在 **2026-08-02** 的 Windows 项目任务全景，基准包含
Slice B/C 交付的 install/uninstall/reinstall batch Sandbox CLI。此前包含
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
已经形成可测试的后端链路。Gate A 和 Gate B 均为 `certified`。

当前开发优先级已经调整为核心 Mod 生命周期的批量能力：T13-00 已完成批量语义和规划契约，T13-01
sealed BatchPlan/preview 已落地；Slice A 已交付 CLI-2A/2B/2C、Sandbox 单项生命周期 E2E 和
CORE-PREF-01。T13-02 的 batch runner、SQLite journal、retry、failure/cancel 证据与入口
fail-closed 规则已落地；PR #222 交付 install batch 子集，Slice C 已把批量卸载、真正重装和
same-revision Armor switch 接入同一 Sandbox `install batch plan/apply/result/retry`。后续切片进入
Tauri/前端工作流与 Windows Sandbox 纵向验收。
Windows 后台存档保障的真实安装态验收和卸载清理仍是发布缺口；完整前置依赖平台、玩家存档恢复、
日志全量保留策略和 Debug Log 也仍未完成。
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
- PR #222 已完成 T13-05 install batch CLI 子集，Slice B 交付闭环完成；Slice C 已补齐批量卸载、
  真正重装与 same-revision Armor switch contract。下一 ready 单元是 Slice D。

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
| T8 存档备份 | 部分完成 | 备份与后台核心已落地；玩家存档恢复、完整 retention、安装态验收和卸载清理未完成 |
| T9 Rich Manifest | 部分完成 | Gate 所需 metadata、状态消费、plan hash、binding snapshot 已落地；完整泛化和写侧门禁未完成 |
| T10 前置依赖检查 | 单项 lifecycle 已完成 / 平台待扩展 | MHW:I bundled rules、诊断查询、install/reinstall 的 blocked/warning decision、锁内重验和 UI/CLI 展示已落地；更多依赖类型、自动修复与完整平台仍未完成 |
| T11 Armor Retarget | Certified（流程）/ 数据待扩容 | AR1-AR5 流程已认证；bundled armor catalog 仍是最小 seed，武器重定向未实现 |
| T12 Mod 详情完整版 | 部分完成、其余暂停 | Gate 所需替换目标 Tab 已完成；完整扩展范围未恢复 |
| T13 批量操作 | Slice C 已完成 / Slice D 待实施 | sealed plan/preview、batch runner、SQLite journal、retry、取消、故障证据与 Sandbox install/uninstall/reinstall `install batch plan/apply/result/retry` 已落地；Tauri/typed API、UI 与 Windows Gate C 仍待 T13-06 至 T13-08 |
| T14 任务队列 UI | 暂停 | 依赖 T13 的真实多任务需求 |
| T15 Linux / Steam Deck | 本轮排除 | 不进入本轮任务、实现、验收或发布判断 |
| T16 Rise / Wilds | 远期 | 每个游戏需要独立 adapter 与设计 |
| T17 第三方管理器批量迁移 | 已完成 | Windows + MHW:I、狩技盒子来源、只导入不安装 |
| T18 Mod 库分页 | 已完成 | 后端权威分页、projection、freshness gate 和 10,000 条性能门禁已落地 |
| T19 生命周期产品化加固 | 已完成 | A1-L3：headless acceptance、日志/诊断与反馈 UI 均已交付 |
| T20 浮层动画共享基元 | 待评审 | 下次新增浮层前或出现第三处重复实现时再启动 |
| CLI 自动化入口 | CLI-2C 已实现 | 已有只读 game/install/backup/diagnostics 命令，以及仅 Sandbox 的单项 install/uninstall/reinstall/recovery apply；5 分钟 token、双确认、写锁内重验、取消、失败恢复与 Production 双层拒绝已覆盖 |
| 工程治理 GOV-01 至 GOV-04 | 已完成 | DTO 测试外置、重装 lint 抑制清理、Tauri 契约防回归和治理检查加固已由 PR #211 至 #214 交付 |

### Gate A / Gate B

Gate A 已覆盖认证导入记录到 `InstallPlan`、安装、重启、manifest 驱动卸载、真正重装、
失败恢复和 baseline 闭环。Gate B 已覆盖 Armor source 分析、目标选择、首次安装、同 revision
目标切换、两次重启状态恢复和 manifest 卸载。

二者的共同安全边界保持不变：

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
- 玩家存档恢复。

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

### 未完成

- 玩家存档恢复。当前 Profile 中的恢复入口仍是禁用占位；安装恢复中心只处理 Mod 安装事实，
  不能替代玩家存档恢复。
- 按时间/空间的存档备份 retention。
- 独立 `features/backups/` 备份中心页面。
- 真实玩家数据环境验收。普通测试只证明 temp/fake 链路。

## Windows User 级后台自动备份

### 已实现的软件核心

- 固定参数的 headless worker：`hmm-save-backup-worker.exe --once`。
- worker 与 GUI 复用 scheduler、backup、lease、heartbeat 和 Audit Log 链路。
- user 级 Windows Scheduled Task 注册、更新、移除和 exact read-back。
- Scheduled Task action 指向同目录 sibling worker，不接受任意 path/profile/lease 参数。
- 登录触发延迟 1 分钟、每 15 分钟执行、单次上限 1 小时、`IgnoreNew`、`StartWhenAvailable`，
  不依赖网络、不唤醒机器，也不因电池状态停止。
- 全局 Settings 唯一开关、Profile 只读保护状态和统一退出保护。
- `starting` / `protected` / unhealthy 等 fail-closed 健康派生。
- enable 后 5 分钟内允许 `starting`；`protected` 需要 exact registration，以及不早于本次启用且
  45 分钟内的新鲜 heartbeat。
- ownership conflict、permission、drift、stale/future heartbeat 和 unsupported 平台的稳定错误。

### 缺真实环境验收

P7.2a 仍缺一次性 Windows 账户或 disposable VM 中的完整安装态链路：

```text
安装产物中的 sibling worker
  -> 注册真实 user Scheduled Task
  -> 人工触发
  -> 写入 fresh heartbeat
  -> 幂等 unregister / cleanup
```

现有 ignored smoke 和 fake runner 自动化不能代替这项验收。在该 gate 完成前，不能宣称“退出 GUI
后的 Windows 后台保护”已经达到发布验收状态。

### 仅设计完成

P7.2c 已有 ownership-checked installer cleanup 规格和实施计划，但以下内容尚未实现：

- 独立 cleanup helper。
- NSIS `PREUNINSTALL` 接入。
- WiX custom action。
- disposable VM 安装/运行/卸载矩阵。

卸载规则必须保留 foreign task；owned task 若处于 running/unknown 状态必须 fail closed。

## 日志、审计与诊断

日志系统不是单一文件，而是四类目标能力和一条受控诊断导出链路。

| 能力 | 当前状态 | 说明 |
| --- | --- | --- |
| App Log | 已实现 | 安全 JSONL、UTC 日轮转、14 天 retention、白名单字段和 health code |
| Task Log | 已实现 | 每个 task 独立 JSONL，与 progress 共用 taskId/kind/status/phase/current/total |
| Audit Log | 已实现 | 高风险操作、后台注册/worker/退出 override 和诊断导出均有最小审计 |
| Debug Log | 未实现 | 7 天 retention 目前只是设计要求 |
| `/diagnostics` 页面 | 已实现 | App/Task/Audit 分类读取，单类失败不阻断其他安全类别 |
| support diagnostics export | 已实现 | 平台摘要和固定上限 App/Task/Audit 数据，用户主动导出、默认脱敏 |

### 已落实的安全边界

- Task Log 不记录自由文本 message、原始 error、result ref 或路径。
- Audit Log 只记录短 ID、聚合计数、稳定 operation/result/error code。
- 不记录完整 home/game/save 路径、用户名、Steam ID、token、cookie、真实存档或第三方 Mod 内容。
- Audit 写入失败不会篡改已经提交的玩家文件事实；`report_after_commit` 会把证据降级显式暴露为
  `audit_write_failed_after_commit`。
- 诊断快照中单类读取失败只返回稳定状态；完整导出若任何必需类别失败则整体失败，并 best-effort
  写最小失败审计。

### 未完成

- Task Log 30 天 retention。
- Audit Log 90 天 retention。
- Debug Log writer、reader、开关和 7 天 retention。
- 可配置的日志总空间上限与按优先级清理。

`docs/LOGGING.md` 中的 14/30/90/7 天是目标默认值，不能把尚未落地的 Task/Audit/Debug retention
当作产品完成。

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

Slice A 已完成当前单项 Sandbox lifecycle 闭环与 CORE-PREF-01；PR #222 已把 T13-01/T13-02 与
T13-05 install batch 子集收敛为 Slice B；Slice C 已完成 T13-03/T13-04 与 T13-05 其余 contract。
下一纵向切片是 T13-06/T13-07/T13-08 的 Tauri、前端和 Windows Gate C，但本轮 PR 收尾后不自动开工。

backup immutable opener 当前没有跨进程只读快照锁；需要一致结果时先关闭桌面端。后续如果要支持
GUI 与 CLI 并行查询，应单独设计 snapshot/admission，而不是放宽 WAL/SHM fail-closed 门禁。

Production 写命令仍依赖跨进程 admission；当前 Sandbox lifecycle capability 不自动解锁 backup、
background registration 或 diagnostics export。

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
- T13-05 已提供 Sandbox install/uninstall/reinstall batch CLI；Tauri command、前端工作流和 Gate C
  仍未接入，不能把后端自动化契约描述为完整玩家产品能力。
- T13-02 当前不自动收敛启动级遗留非终态 `queued/running/stopping` attempt；T13-05 CLI
  在 apply/retry/new apply 保留只读预检，并以 SQLite 原子 scope admission 最终阻断并发新写入。
  result 只读取指定 batch/attempt，保留遗留状态的安全诊断能力。后续若要自动 reconciliation，必须
  单独设计安全终态、证据和恢复验收；Sandbox batch admission 也不等于 Production 通用写 admission。
- Windows 后台保护的 fake/temp 自动化不能替代安装态 VM 验收；installer cleanup 也不能只凭
  bundle 中存在 sibling worker 就标记完成。
- GOV-01 至 GOV-04 已完成；后续变更需保留对应文件大小、secret、CODEOWNERS 和 Tauri
  command 契约回归门禁。

## 建议执行顺序

1. 保持遗留非终态 `queued/running/stopping` attempt 的 fail-closed 门禁；如需自动 reconciliation，
  先单独完成安全设计和验收。
2. 完成 T13-06/T13-07 Tauri 与前端工作流以及 T13-08 Gate C；Gate C 前不抢跑新的高风险写入链路。
3. 完成装备数据治理、防具 catalog 扩容和独立武器重定向链路。
4. T17 只做条件式脱敏真实来源 smoke 或明确 bugfix，不重新实现。
5. 完成 Windows 多账号备份回归、安装态 Scheduled Task 验收、installer cleanup 和存档恢复。
6. 补齐 Task/Audit retention、日志空间上限和 Debug Log，再评审 Production CLI 跨进程写入。

完整 task 依赖和合并门禁见 [Windows 自主迭代路线图](AUTONOMOUS_ITERATION_ROADMAP.md)。
