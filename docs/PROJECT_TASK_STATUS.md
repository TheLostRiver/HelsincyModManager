# 项目任务状态快照

本文档记录 Helsincy Mod Manager 在 **2026-07-29** 的项目任务全景，基准为
`main@beb22ae9015c4f1ca77ee4b4c6ba903404d9bca7`，并包含当前工作区的 CLI-0A contract、
CLI-0B shared runtime composition、CLI-1A read-only game automation 与完整 CLI-1B read-only
install/backup/diagnostics automation。

本文件是一次证据快照，用于回答“当前已经具备什么、还缺什么、下一步先做什么”。持续变化的任务
优先级、依赖和实施状态仍以 [任务总纲](../TODO.md) 与 [路线图](ROADMAP.md) 为准；功能设计和安全
边界以对应专题文档、当前源码和测试为准。

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

当前最主要的发布缺口集中在 Windows 后台存档保障的真实安装态验收和卸载清理，而不是基础 Mod
生命周期。完整前置依赖平台、批量安装/卸载、玩家存档恢复、日志全量保留策略和 Debug Log 仍未完成。
后端命令化已完成 CLI-1A 和 CLI-1B：`hmm-runtime` 已承载真实共享 composition，
桌面端与固定 `--once` worker 复用同一装配；独立只读 facade 已支持游戏状态、扫描、已保存目录
校验、前置检查、安装计划/状态、恢复扫描/预览、备份历史、后台保护状态和诊断快照。安装提交、
卸载、重装、恢复执行、备份创建/恢复、后台启停、诊断导出和任何 Production 写入仍不可达。

快照时：

- `main == origin/main == beb22ae`。
- GitHub 没有 open PR 或 open issue。
- PR #196 已合并；其 review 遗漏由 PR #197 补齐。
- PR #199 已完成 T17 Slice 4C，T17 全部切片已交付。

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
| T10 前置依赖检查 | 部分完成 | MHW:I bundled rules 与状态查询、Gate A/B preflight 已有；安装前完整阻断/警告和产品展示未完成 |
| T11 Armor Retarget | Certified | AR1-AR5、目标切换、重启恢复、manifest 卸载和 Sandbox 纵向验收已完成 |
| T12 Mod 详情完整版 | 部分完成、其余暂停 | Gate 所需替换目标 Tab 已完成；完整扩展范围未恢复 |
| T13 批量操作 | 暂停 | 批量安装/卸载队列、进度和产品语义尚未设计落地 |
| T14 任务队列 UI | 暂停 | 依赖 T13 的真实多任务需求 |
| T15 Linux / Steam Deck | 远期 | 需要独立设计和社区/设备验证 |
| T16 Rise / Wilds | 远期 | 每个游戏需要独立 adapter 与设计 |
| T17 第三方管理器批量迁移 | 已完成 | Windows + MHW:I、狩技盒子来源、只导入不安装 |
| T18 Mod 库分页 | 已完成 | 后端权威分页、projection、freshness gate 和 10,000 条性能门禁已落地 |
| T19 生命周期产品化加固 | 已完成 | A1-L3：headless acceptance、日志/诊断与反馈 UI 均已交付 |
| T20 浮层动画共享基元 | 待评审 | 下次新增浮层前或出现第三处重复实现时再启动 |
| CLI 自动化入口 | CLI-1B 已实现 | 已有四个只读 game 命令、install plan/status/recovery scan/preview、backup list/background status 与 diagnostics snapshot；Sandbox containment/no-write、checkpointed sidecar-free SQLite/log readers 与脱敏 contract 已覆盖，写命令未接入 |

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

T13 才是批量安装/卸载。T17 完成不代表 T13 已完成，也不能借 T17 的批处理编排绕过单项安装的
`InstallPlan`、manifest、backup、rollback、锁和审计语义。

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

### CLI-0A / CLI-0B / CLI-1A / CLI-1B 已实现

- workspace 已新增 `hmm-runtime` 与 `hmm-cli`，CLI dependency tree 不包含 Tauri。
- `hmm runtime status` 支持 `human|json|jsonl`、Production/Sandbox 环境和稳定退出码。
- CLI-0A human/help/error 输出统一无 ANSI，`--no-color` 保留为稳定全局参数。
- Production 禁止 `--data-dir`，写命令策略固定为 `disabled`。
- Sandbox 要求显式绝对数据根并拒绝 root、`.` 和 `..`；当前不创建目录或 marker，也不签发写许可。
- JSON/JSONL 使用 `hmm.cli/v1`；机器模式的 runtime/parse 错误使用稳定脱敏 envelope。
- `HmmRuntime` 已装配真实 repositories/services、`TaskManager` 与 game/profile 写锁。
- Tauri `AppState` 已变为 runtime 薄包装；固定 `--once` worker 直接构造 runtime。
- `TaskProgressObserver` 已建立 transport-neutral 接缝，Tauri event/Task Log/queued App Log 保持原行为。
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
- install apply/uninstall/reinstall/recovery apply、backup create/restore/background enable|disable、
  diagnostics export 等写命令尚未开放。

### 下一步

CLI-1B 已完成。现有 runner 仍返回事件集合；首个 CLI-2 长任务 JSONL 命令开放前必须完成逐阶段
observer，不能把当前 transport 接缝误报为实时流。Sandbox 写链路还必须新增 marker/capability、
canonical containment、失败注入和完整安全链路，不得把 CLI-1B 只读结果当作后续写入授权。

backup immutable opener 当前没有跨进程只读快照锁；需要一致结果时先关闭桌面端。后续如果要支持
GUI 与 CLI 并行查询，应单独设计 snapshot/admission，而不是放宽 WAL/SHM fail-closed 门禁。

Production 写命令仍依赖跨进程 admission；Sandbox 写命令仍依赖 marker、canonical containment、
fake/temp fixture 与完整安全链路。两者都不能由当前 `sandbox_only` 策略字符串提前解锁。

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

此前总体状态审计在同一基准上还记录：

- 前端测试：400/400 通过。
- 前端 build：通过；仅有约 597 KB 主 chunk 超过 500 KB 的非阻断 warning。
- App Log、Task Log、Audit Log、diagnostics、support export、save backup 和后台 worker 等聚焦
  Rust/Node 测试通过。

统一 `scripts/verify.ps1` 未完整通过。原因不是主工作区源码 lint 失败，而是 `eslint .` 扫描了
Git ignored 的 `.worktrees/` 历史 worktree 和其中的 generated/cache/target 产物，产生 1970 个
无关错误并提前退出。临时排除 `.worktrees/**` 后，主工作区 lint 通过。该问题应作为验证稳定性缺口
处理，不能把统一验证写成已通过。

真实 Windows Scheduled Task、安装态 sibling worker、heartbeat 和 installer cleanup 没有在日常
开发账户中执行；这些检查只能在一次性账户或 disposable VM 中完成。

## 已知文档与治理偏差

- `SAVE_BACKUP_BACKGROUND_SCHEDULER_CORE_PLAN.md` 仍写 P7.2b 未实现，与较新的自动化设计、
  `TODO.md`、源码和测试不一致。
- ESLint ignore 未覆盖仓库允许存在的 `.worktrees/`，导致统一验证对本地历史 worktree 敏感。
- 无人值守治理队列 A1-A6 尚未实施：超大 `state.rs` / `dto.rs`、命令契约缺口、统一 verify
  未运行前端 tests 或 clippy 等问题仍需按治理路线处理。

## 建议执行顺序

1. 在首个长任务 CLI 命令前完成 runner 逐阶段 observer 与 JSONL 顺序/terminal 测试。
2. 完成 P7.2a disposable Windows VM 的安装态 worker -> Scheduled Task -> heartbeat -> cleanup smoke。
3. 实现 P7.2c ownership-checked cleanup helper、NSIS/WiX 接入和卸载矩阵。
4. 补齐 Task/Audit retention、日志总空间上限；决定 Debug Log 是实现还是正式延期。
5. 修复后台调度文档漂移与 ESLint `.worktrees/` ignore，继续推进治理队列。
6. 单独评审 T13 批量安装/卸载的队列、原子性、失败与取消语义。
