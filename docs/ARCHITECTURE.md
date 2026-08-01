# 架构设计

## 项目定位

Helsincy Mod Manager 不是一个简单的压缩包解压工具，而是一个本地游戏 Mod 管理平台。

第一阶段会以《怪物猎人：世界 冰原》为首个支持目标，但整体架构必须允许后续扩展到《怪物猎人：崛起》《怪物猎人：荒野》以及其他具有类似资源替换机制的游戏。

第一版以 Windows 可用为重点，同时通过平台抽象保留 Linux / Steam Deck 支持空间。

## 核心原则

- UI 和核心业务逻辑分离。
- 应用用例依赖 trait / interface，而不是依赖具体实现。
- 游戏差异全部收敛到游戏适配器。
- 安装必须基于安装计划和安装清单，不能随手复制文件。
- 面向玩家的规则尽量数据驱动。
- 重任务必须后台执行，并通过事件向前端汇报进度。
- 同一个游戏实例的写入操作必须串行。
- 所有破坏性操作都必须可回滚或可恢复。

## 总体分层

```text
前端 UI
  React + TypeScript
  负责展示、交互、筛选、弹窗、进度展示

Tauri Commands
  前端与 Rust 后端之间的薄边界
  负责参数校验和 DTO 转换

CLI Transport
  独立的 hmm 命令行入口
  负责参数解析、机器输出契约和退出码，不依赖 Tauri

Runtime Boundary
  Tauri-free 的运行环境策略与共享 composition 边界
  CLI-0A 落地环境策略；CLI-0B 迁移共享装配；CLI-1A/1B 增加独立只读 automation facade

Application 应用层
  导入 Mod、安装 Mod、禁用 Mod、备份存档、启动游戏等用例

Domain 领域层
  Mod、Game、Profile、InstallPlan、Conflict、Manifest、Dependency、ReplacementTarget

Ports / Traits 接口层
  文件系统、压缩包、数据库仓储、游戏适配器、启动器、任务系统

Infrastructure 基础设施层
  SQLite、真实文件系统、压缩工具、hash、Steam 库扫描、平台 API

Game Adapters 游戏适配器
  首先支持 Monster Hunter: World - Iceborne
  后续扩展 Monster Hunter Rise / Wilds
```

## Rust Workspace 规划

```text
src-tauri/              # Tauri 应用 crate，包名 hmm-tauri
  src/                  # Tauri commands、state、events、应用启动
  crates/
    hmm-core/          # 纯领域模型和规则，不接触真实系统 API
    hmm-ports/         # 应用层依赖的 traits/interfaces
    hmm-app/           # 应用用例和流程编排
    hmm-infra/         # SQLite、文件系统、压缩包、hash、Steam 扫描
    hmm-games-mhw/     # MHW:I 适配器和游戏规则
    hmm-runtime/       # Tauri-free runtime policy、共享 composition 与 task observer contract
    hmm-cli/           # hmm binary、CLI parser 与 JSON/JSONL contract
    hmm-games-rise/    # 后续 Rise 适配器和游戏规则
    hmm-games-wilds/   # 后续 Wilds 适配器和游戏规则
    hmm-games-common/  # 可选，怪物猎人系列共享适配工具
```

`src-tauri/` 本身作为 Tauri 应用 crate，包名为 `hmm-tauri`。这样可以保留 Tauri CLI 默认约定，避免额外配置成本；可复用业务 crate 放在 `src-tauri/crates/` 下。

`hmm-runtime` 与 `hmm-cli` 已在 CLI-0A 加入 workspace。CLI-0B 已把真实 adapter composition、
configured executors、共享 repositories、`TaskManager` 与 game/profile 写锁迁入 `hmm-runtime`。
Tauri `AppState` 现在只是解析 app data、启动 GUI-only 缩略图维护并解引用到 `HmmRuntime` 的薄包装；
固定 `--once` worker 直接构造 `HmmRuntime`，不再通过 Tauri state 获取后台备份服务。

`TaskProgressObserver` 以 `hmm-app::TaskProgressEvent` 为输入。install、uninstall、reinstall 和
recovery runner 会在阶段实际推进时调用 observer，同时继续返回 `Vec<TaskProgressEvent>` 兼容既有
调用方。Tauri adapter 按原顺序转换安全 DTO、写 Task Log、记录 queued task 注册并发送
`hmm://task-progress`；CLI adapter 从 0 分配单调 `sequence`，先写共享 Task Log，再 flush 脱敏
JSONL。observer 失败不改变 task 状态、commit、rollback 或玩家文件事实。取消 terminal 由发起取消的
transport 发送，runner 只停止后续安全阶段，避免重复 terminal。

CLI-1A 在 `hmm-runtime` 中增加 `ReadOnlyGameAutomation`。它不构造会打开/迁移 SQLite 或执行恢复
装配的完整 `HmmRuntime`，只装配 game config reader、MHW:I adapter、只读 prerequisite rules、
directory probe 和 Steam discovery。facade 返回的安全 snapshot 类型不包含游戏/candidate/rule
路径、自由文本 message、Steam ID 或用户名。

CLI-1B 的只读安装子切片增加 `ReadOnlyInstallAutomation`。它独立装配 install planning、
manifest status、recovery scan/preview 所需的 app services 和只读 infra readers，不打开 SQLite、
不使 projection dirty，也不装配 install/uninstall/recovery executor 或 task service。Mod revision
catalog 使用显式 read-only 模式：查询不创建 lock，不把 v1 内存迁移写回 v2，所有 mutator fail
closed。Sandbox 在读取前校验固定 config/catalog/sandbox/manifest/recovery/backup 根仍位于显式
data root，recovery 使用的 game root 仍必须位于 `<data-dir>/fixtures` canonical 边界。

CLI-1B 的备份/诊断子切片增加 `ReadOnlyBackupAutomation` 与
`ReadOnlyDiagnosticsAutomation`。备份 facade 只读取已 checkpoint 且没有
`hmm.db-wal`/`hmm.db-shm` sidecar 的既有 SQLite：infra 通过 percent-encoded immutable URI、
read-only flags 和 connection-local query-only mode 打开数据库。任一 sidecar 存在都 fail
closed，不尝试读取 live WAL，也不 checkpoint、修复、创建或修改 DB/WAL/SHM，不执行
migration/default seed。该可用性取舍避免普通 SQLite read-only 查询创建 sidecar 或修改 SHM
bytes，但不提供跨进程只读快照锁；需要一致结果的自动化应在桌面端关闭、数据库静止后运行。
Sandbox 的平台注册状态和时钟来自固定 `fixtures/background/status.json`，不会触碰真实 Scheduled
Task。诊断 facade 复用独立的 `DiagnosticsPageSnapshotService`、
`FileSystemTextLogReader` 与只读 `FileSystemAuditLogReader`，只投影受控平台摘要、分类状态和
聚合计数，不返回日志正文、来源文件名、Audit fields 或导出路径。

`hmm-cli` 当前开放 `hmm runtime status` 与
`hmm game status|scan|validate|prerequisites --game mhw`，以及
`hmm install plan|status|recovery scan|recovery preview`、
`hmm backup list|background status` 和 `hmm diagnostics snapshot`，继续使用 `hmm.cli/v1`、单对象
JSON/JSONL、stdout/stderr 和稳定退出码契约。Production 只读取系统 HMM app data 和平台 Steam
discovery，并允许后台状态执行只读平台注册 inspect；Sandbox 只读取显式数据根下的受控
config/state/logs 与 `<data-dir>/fixtures`，其中 Steam root 固定为 `fixtures/steam`。保存的 game
root、VDF library、discovery candidate、install/backup state roots 和日志目录在读取前执行词法与
canonical containment。Production 与 Sandbox 的所有只读命令都不创建 marker，也不签发写
capability。

CLI-2C 另外只在 Sandbox 开放 `hmm install apply|uninstall|reinstall` 与
`hmm install recovery apply`。未携带完整 `--commit --yes` 时命令仍为 preview；提交时必须消费
5 分钟 opaque lifecycle token。`SandboxLifecycleAutomation` 在构造写侧 runtime 前验证 token，
runner 取得共享 game/profile 写锁后，再由 configured committer/admission 重建计划或
manifest/recovery facts、重验 token 和 capability，之后才进入既有写入事务。Production 在 CLI
policy 和 runtime composition 两层固定拒绝。

CLI-4 Slice B 在 Sandbox 增加 `hmm install batch plan|apply|result|retry`。批量 plan 使用
`BatchPlanService` 生成脱敏 projection 和短期 opaque `previewToken`；apply 先通过不初始化
`HmmRuntime` 的只读 facts service 验证 preview，验证通过后才创建 SQLite batch journal，并在
`seal` 阶段再次读取 facts、验证 token 后持久化 sealed batch。批量 runner 复用
`InstallPlan`、backup、manifest、rollback/recovery、Task/Audit Log 和 game/profile 写锁；
start/retry 的最终 admission 在 SQLite `BEGIN IMMEDIATE` 短事务内验证 batch/attempt/token，检查同一
game/profile 的 `queued/running/stopping` attempt，并原子完成 sealed -> queued。两个独立进程因此
最多一个能取得同 scope 的 batch admission；retry 在竞争失败时只回收仍 sealed、没有 item result
且 verifier 匹配的未执行新 attempt。`result` 不执行 scope reconciliation，只读取调用方明确指定的
batch/attempt，使遗留 active attempt 的诊断结果保持可读。该原子性只覆盖 Sandbox batch journal，
不等于 Production 通用写 admission；Production 在 CLI policy 和 runtime composition 两层继续
fail closed。批量卸载、真正重装、Tauri command 与前端工作流仍按 T13-03 至 T13-07 的依赖开放。

CORE-PREF-01 将 `GamePrerequisiteDecisionProvider` 固定为单项安装/重装的 app-level 单一事实源。
`ImportedModInstallPreflightService`、`ReinstallPreviewService`、桌面 task runner 和
`ReadOnlyInstallAutomation` 复用 runtime 中同一个 provider。preview 和提交前最终重验都在写锁外
读取规则、hash 和配置，返回 `ready | warning | blocked`、stable codes 与 rules version；runner
在获取 game/profile 写锁前立即比较最终 decision 与 preview/token facts。取得写锁后只校验已封存的
plan/token、identity、containment 和当前 manifest/目标状态，不再执行 prerequisite 规则读取或 hash。
blocked 或任何 decision 漂移都在 commit、staging 和游戏目录写入前 fail closed。Tauri、CLI 和
React 只投影该 decision，不重算 MHW:I 规则，也不返回 issue path、自由文本 message 或配置正文。

CLI-2B 在 `hmm-runtime` 增加了 `SandboxWriteCapability`。只有显式 Sandbox 环境可以通过
`RuntimeEnvironment::acquire_sandbox_write_capability` 申请；Production 没有 capability 构造路径。
空 Sandbox 根会在申请时通过 no-follow 目录句柄创建固定 `.hmm-sandbox.json` v1 marker，非空根
必须已经包含完全匹配的 marker。marker 不是授权秘密，不能替代 capability；真正的授权对象字段和
构造器私有、不可序列化，并在存活期间保留打开的根目录句柄、canonical root 和目录身份。
`SandboxWriteRoots` 对本次操作实际使用的 app-data、game、save、backup 根执行词法、canonical、
symlink/junction/reparse-point containment，返回的 `SandboxWriteAdmission` 绑定原 capability
生命周期。写侧可在安全阶段前重新调用 `revalidate`；Windows 通过打开句柄阻止祖先替换，其他平台
在允许替换时通过目录身份变化 fail closed。该能力只由明确接线的 Sandbox lifecycle composition
消费，不自动开放备份、诊断或 Production 写入。

CLI lifecycle adapter 复用 `TaskManager` 处理 Ctrl+C。首个 signal 可以在 runtime/task 建立前锁存，
task 出现后请求协作式取消；确认取消时 observer 只发一个 `install.cancelled` terminal。第二个 signal
允许以 130 强制退出，但明确提示调用 recovery/status 重新确认状态；不可抢占 commit 开始后，signal
不能把成功或受控失败伪装成 cancelled。

目标依赖方向：

```text
hmm-tauri -----\
hmm-cli --------+--> hmm-runtime
backup worker --/       |--> hmm-app -------> hmm-ports
                       |       \-----------> hmm-core
                       |--> hmm-infra -----> hmm-ports / hmm-core
                       \--> hmm-games-* ---> hmm-ports / hmm-core
```

`hmm-cli` 不依赖 `hmm-tauri`；`hmm-runtime` 不依赖 Tauri、WebView 或 CLI 参数类型。Production 的 CLI
写命令在跨进程 admission 完成前保持不可达。Sandbox 单项 lifecycle 命令已经复用完整 application
service、InstallPlan、backup、manifest、rollback/recovery、Audit Log 和写锁；其他写命令仍需按各自
安全边界逐项开放。

`hmm-games-rise/`、`hmm-games-wilds/` 和 `hmm-games-common/` 是规划边界，不要求在 MVP 阶段立即创建。只有当对应游戏适配或共享工具真实落地时，才新增 crate，避免空目录和空抽象。

前端按功能拆分：

```text
src/
  app/
  features/
    dashboard/
    mods/
    categories/
    profiles/
    conflicts/
    backups/
    games/
    settings/
  shared/
    api/
    components/
    state/
    types/
```

## 多游戏扩展边界

Helsincy Mod Manager 的扩展方式是“一个 app + 多个游戏适配器”，不是“一个游戏复制一套 app”。世界冰原、崛起、荒野的规则代码不能混放在同一个适配器文件或通用核心模块里。

通用流程放在核心和应用层：

- Mod 导入、压缩包安全校验和沙盒解压。
- 预览图提取和基础元数据分析。
- 分类、标签、Profile、任务队列和日志审计。
- `InstallPlan`、`InstallManifest`、冲突检测框架、备份和回滚流程。
- Tauri command、前端 API 封装和通用页面状态。

游戏差异放在独立 adapter：

- 游戏目录识别和启动方式差异。
- `nativePC`、根目录 DLL、loader、前置依赖规则。
- 官方外观、武器、语音替换目标 catalog。
- 存档路径规则、资源编号解析、retarget / 重定向规则。
- 游戏专属 Mod 结构识别和包分析扩展。

应用层只依赖 `GameAdapter` trait 和数据 catalog，不直接依赖 `hmm-games-mhw`、`hmm-games-rise` 或 `hmm-games-wilds` 的具体实现。具体 adapter 只在组合根或注册表中被装配。这样新增 Rise 或 Wilds 时，应新增 adapter crate 并注册能力，而不是修改 MHW:I adapter 来兼容其他游戏。

前端默认使用游戏无关 feature 页面，例如 `features/mods/`、`features/backups/`、`features/replacements/`、`features/settings/`。这些页面通过当前游戏的 capability、dependency rules、replacement catalog 和任务状态来改变展示内容。只有当某个游戏确实需要无法由通用页面表达的专属交互时，才新增 `features/games/<game-id>/` 下的专属 UI。

禁止的耦合方式：

- 在 `hmm-core`、`hmm-app` 或通用前端 feature 中写死 MHW:I 路径、资源编号或前置文件名。
- 在 MHW:I adapter 中加入 Rise / Wilds 的判断分支。
- 前端根据游戏名拼接安装路径或替换目标路径。
- 为每个游戏复制一整套 Mod 管理、备份、任务和设置页面。

## 主要模块

### 游戏发现

游戏目录识别需要支持多种策略：

- 扫描 Steam library
- 扫描正在运行的进程
- 玩家手动选择目录

发现模块返回 `GameInstance`，不能假设游戏只有一个固定路径。

### 游戏启动

启动逻辑由平台和游戏适配器决定：

- 优先通过 Steam 协议启动
- 必要时直接启动游戏 exe
- 后续 Linux / Steam Deck 通过独立平台实现处理

启动前可以检查：

- 是否缺少必要前置
- 当前 profile 是否存在冲突
- 是否有未完成的安装任务
- 游戏目录是否仍然有效

### Mod 导入流水线

导入压缩包不能直接安装，必须先经过安全流水线：

```text
选择压缩包
检查压缩包信息
拒绝危险路径
解压到沙盒缓存目录
分析文件结构
提取并校验预览图
推断 Mod 类型
生成元数据
生成候选安装计划
```

导入器必须防御：

- `../` 路径穿越
- 绝对路径
- 压缩包炸弹
- 不支持或可疑的文件类型
- 伪装图片扩展名
- 大小写不敏感平台上的路径冲突

### 包分析器

包分析器识别 Mod 内容，例如：

- `nativePC` 文件
- 游戏根目录 DLL
- exe 或辅助工具
- INI / JSON / config 文件
- readme 文件
- 预览图片
- 外观、武器、语音替换相关的资源编号

包分析器输出结构化信息，不能把安装规则塞进前端。

### 分类和标签

分类和标签必须支持多对多关系。

默认分类可以包括：

- 外观
- 主角外观
- NPC 外观
- 随从外观
- 武器替换
- 语音替换
- 功能性 Mod
- 武器特效
- 前置
- 工具

玩家必须可以创建自定义分类，并把一个 Mod 放到多个分类或标签下。

### 前置依赖检查

很多怪猎 Mod 需要前置文件或 loader。依赖检查必须数据驱动。

依赖规则的大致形态：

```text
DependencyRule
  id
  display_name
  severity
  detection_rules
```

检测方式可以包括：

- 游戏根目录存在某文件
- `nativePC` 下存在某文件
- 文件 hash 匹配已知值
- 安装清单中存在某个前置 Mod

app 层把 adapter report 归一化为版本化 `GamePrerequisiteDecision`：

- required missing、规则不可用/损坏、目录或存储不可用为 `blocked`，不得签发计划 token。
- 签名未命中等可继续状态为 `warning`，允许执行但不能在 UI/CLI 中伪装成“预检通过”。
- 只有规则验证完成且没有 issue 才是 `ready`。
- install、true reinstall、retarget staging、Tauri 和 CLI 必须消费同一个 decision；runner 在获取
  写锁前完成最终 provider 重验并比较 status、stable codes 与 rules version，任何漂移都要求重新
  preview。规则读取、配置解析和文件 hash 不得在 game/profile 写锁内执行。

详细诊断 report 可以返回受控相对 issue path；生命周期 decision 只返回聚合 status、stable codes
和 rules version，不传播完整路径、display message 或配置正文。

### 替换目标映射

外观、武器、语音 Mod 经常不是单纯“安装文件”，而是把自定义资源覆盖到官方资源槽位上。管理器必须把这种关系建模为一等概念。

核心模型：

```text
ReplacementTarget
  官方游戏资源槽位
  例如：某套外观、某个部位、某把武器、某个语音槽位

ReplacementBinding
  玩家选择的“Mod 资源 -> 官方目标”的绑定关系

ReplacementBindingSnapshot
  安装计划与 manifest 持久化的稳定绑定事实；包含 Mod/profile/revision 归属和 source/target
  identity/path-family，不包含 staging/cache/sandbox 路径

ReplacementCatalog
  带稳定版本的游戏目标集合；查询和搜索规则由游戏 adapter 提供

ReplacementAnalysis
  从包内相对路径得到的 source 集合、命中计数和稳定 warning；不访问真实文件系统

RetargetPlan
  为了把 Mod 重定向到目标槽位，需要在 staging 目录执行的纯相对路径改写计划

RetargetAction
  关联 package file identity、source/final relative path 与不透明 source/target facts
```

外观替换需要支持：

- 可拆分外观：头、胸、手、腰、脚
- 固定整套外观：联动整套、不可拆分整套
- 未来高级拆分或转换流程，通过插件式 transformer 扩展

重要规则：

- 原始导入的 Mod 包永远只读。
- 包分析和 `RetargetPlan` 生成是纯操作，不携带 cache/sandbox 绝对路径。
- 重定向只发生在 staging 目录。
- `InstallPlan` 与安装清单记录玩家选择的替换绑定快照，并把快照事实纳入计划身份。
- 冲突检测基于最终目标路径，而不是原始压缩包路径。
- 玩家切换目标时，本质上是卸载旧绑定，再安装新绑定。

### 安装计划

安装前必须先生成 `InstallPlan`。

当前 `InstallPlan` 模块的已落地能力、未完成边界和后续切片见 [InstallPlan 模块现状](INSTALL_PLAN_STATUS.md)。

计划动作示例：

```text
CopyFile
CreateDirectory
BackupExistingFile
RemoveFile
WriteManifest
```

安装计划负责：

- 将包内容转换成游戏目标路径
- 应用替换目标绑定
- 检测冲突
- 检查前置依赖
- 估算任务量，用于进度展示

### 安装执行器

安装执行器负责真正修改游戏目录。

要求：

- 覆盖文件前必须备份。
- 安装完成后必须写安装清单。
- 失败时尽可能回滚。
- 同一个游戏实例的写入必须串行。
- 记录足够状态，用于崩溃或强制关闭后的恢复扫描。

### 存档备份服务

存档备份模块独立于 Mod 安装模块。

必备能力：

- 手动备份
- 自动备份
- 玩家自选备份目录
- 未选择时使用默认备份目录
- 自动备份时间间隔可配置
- 按数量、时间或空间占用设置保留策略
- 备份清单和 hash 校验

默认备份目录应位于应用数据目录下，而不是游戏目录里。

### 任务管理器

长耗时操作必须作为后台任务执行：

- 压缩包解压
- 包扫描
- hash 计算
- 冲突分析
- 安装计划生成
- 安装执行
- 存档备份压缩

前端通过 Tauri command 启动任务，通过事件接收进度。

## 日志与审计

日志系统是任务管理、错误诊断和高风险文件操作审计的基础设施，详细规则见 [日志与审计设计](LOGGING.md)。

核心要求：

- 后端使用结构化日志，优先记录 `task_id`、`game_id`、`profile_id`、`mod_id`、操作类型、结果和错误分类。
- 前端负责展示任务进度、用户可读错误和诊断导出入口，不直接写核心日志文件。
- 游戏目录写入、覆盖、删除、备份、恢复、manifest 和回滚必须写 Audit Log。
- 日志写入前必须统一脱敏，禁止记录完整本地路径、Steam ID、token、cookie、真实存档内容或第三方 Mod 内容。
- 诊断包只能包含已脱敏日志和配置摘要，不能包含真实存档或真实 Mod 包。

## 并发模型

并发原则：

```text
读取和准备工作可以并行。
同一个游戏实例的写入必须串行。
```

建议的任务分组：

- CPU pool：hash 和冲突分析
- IO pool：扫描、解压、复制前准备
- Game write queue：每个游戏实例一个串行写入队列
- Database transaction：短事务、明确写入边界
- Event bus：进度和日志事件

采用两阶段执行：

```text
Prepare 阶段
  解压、hash、分析、依赖检查、生成计划
  可并行、可取消、不碰游戏目录

Commit 阶段
  获取游戏写锁
  重新校验当前状态
  备份、复制、删除、写清单
  短时间串行、可恢复
```

不要在持有游戏写锁时做长时间解压或 hash。

## 数据存储

SQLite 存储用户数据和运行状态：

- 游戏实例
- Mod 元数据 overlay、分类和标签
- Profile
- 替换绑定
- 安装清单
- 备份历史
- 用户设置
- 可删除、可重建的 Mod 库 query projection（不是安装事实）

JSON revision catalog 继续作为已导入 Mod 的权威来源，保存 logical Mod、revision lineage、import provenance 和展示 revision；SQLite projection 只保存查询展示所需的派生列、规范化 key、分类关系和稀疏 profile status。

JSON 或 TOML 还可存储偏规则的数据：

- 默认分类
- 官方替换目标 catalog
- 前置依赖规则
- 存档路径规则
- Mod 类型识别规则
- 备份策略默认值
- 预览图大小、压缩包大小等限制

## 关键领域模型

```text
GameDefinition
  id
  display_name
  adapter_id
  supported_platforms

GameInstance
  id
  game_id
  install_path
  platform
  launcher

ModEntry
  id
  name
  version
  package_ref
  categories
  tags
  dependencies

ModPackage
  id
  archive_path
  extracted_cache_path
  detected_type
  files
  preview_image
  metadata

ReplacementTarget
  id
  game_id
  target_type
  display_name
  aliases
  internal_id
  metadata              # 游戏专属结构化值，core 只透传

ReplacementBinding
  id
  mod_id
  profile_id
  source_id             # 游戏无关、稳定且对 core 不透明
  target_id
  created_at_unix_millis

ReplacementCatalog
  version
  game_id
  targets

InstallPlan
  id
  actions
  conflicts
  dependency_result
  replacement_bindings

InstallManifest
  id
  mod_id
  profile_id
  installed_files
  backups
  hashes
  replacement_bindings
```

## 关键 Traits

```rust
pub trait GameAdapter {
    fn game_id(&self) -> GameId;
    fn detect_instances(&self) -> Result<Vec<GameInstance>>;
    fn analyze_package(&self, package: &ModPackage) -> Result<GamePackageInfo>;
    fn build_install_plan(&self, request: InstallRequest) -> Result<InstallPlan>;
    fn dependency_rules(&self) -> Result<Vec<DependencyRule>>;
}

pub trait ReplacementCatalogProvider {
    fn game_id(&self) -> GameId;
    fn replacement_catalog(&self) -> Result<ReplacementCatalog>;
    fn find_replacement_target(&self, target_id: &ReplacementTargetId) -> Result<ReplacementTarget>;
    fn search_replacement_targets(&self, query: &str) -> Result<Vec<ReplacementTarget>>;
}

pub trait FileSystem {
    fn exists(&self, path: &Path) -> bool;
    fn copy_file(&self, from: &Path, to: &Path) -> Result<()>;
    fn remove_file(&self, path: &Path) -> Result<()>;
    fn create_dir_all(&self, path: &Path) -> Result<()>;
}

pub trait ArchiveExtractor {
    fn inspect(&self, archive: &Path) -> Result<ArchiveInfo>;
    fn extract_to(&self, archive: &Path, target: &Path) -> Result<()>;
}

pub trait ModRepository {
    fn save(&self, mod_entry: &ModEntry) -> Result<()>;
    fn get(&self, id: ModId) -> Result<Option<ModEntry>>;
}
```

当前已落地的首次启动 / 游戏目录配置端口位于 `hmm-ports::game_setup`，由 `hmm-app` 依赖并由 `hmm-infra`、`hmm-games-mhw` 提供实现：

- `GameAdapter`：声明游戏 id、显示名和目录校验规则；MHW:I 的 `MonsterHunterWorld.exe`、`nativePC` 等识别规则属于游戏 adapter。
- `GameDirectoryProbe` / `GameDirectoryProbeFactory`：隔离真实文件系统读取，让应用层只消费探测接口，测试可使用 fake probe。
- `GameConfigRepository`：保存和读取已配置的游戏实例；实现层负责 JSON schema、原子写入和存储错误映射。
- `GameDiscoveryService`：承载 Steam library / 运行进程扫描等发现能力；MVP 阶段允许返回明确的未实现错误。

ARMOR_RETARGET AR1 已在 `hmm-ports::replacement` 落地独立只读 `ReplacementCatalogProvider`；AR2 在
同一模块增加只携带 package file identity/相对路径的窄 `ReplacementAdapter`，没有扩张目录
`GameAdapter`。MHW:I adapter 负责 versioned catalog、Unicode/search normalization、严格
`plNNN_VVVV`/`f_equip` 路径分析和结构化 slot 替换；通用 core 只保存不透明 source/target facts 与
纯 `RetargetPlan`。AR3 已在独立 staging port/infra adapter 中实现受控 batch materialize：先写 sibling
`.partial`，完整成功后原子发布，失败清理；最终 target 进入 `InstallPlan`，原 `PackageFileId`
provenance 保留，binding snapshot 随 plan/manifest/reinstall recovery 原子保存。Tauri/frontend wiring
已由 AR4 通过四个窄 command 和 feature-local typed API 接入；入口位于 Mod 管理的 Mod 详情
“替换目标”Tab，右键“MOD 文件修改”直达。前端只提交 game/Mod/profile/target/layer identity，首次
retarget install 继续走 task id、game/profile 写锁、Audit Log、backup、manifest 和 rollback/recovery
链路，并对 installed/unsafe/unknown 状态 fail closed。AR5 在同一入口增加两个窄 command：后端从
manifest 解析 installed revision，同 revision 且 target 确实变化时复用真正重装事务原子替换旧
entry/binding，重启后恢复新 target，最终卸载仍由 manifest 驱动恢复首次 Armor 安装前 baseline。
分析响应可按 profile 从可信 manifest 附带唯一稳定 `installedTargetId`，供前端标记当前 target 并禁用
同目标切换；不暴露 binding、revision、路径或 manifest 内容。该链路的自动化与受控 UI 已实现；
修复当前 target 呈现缺陷后的最终 artifact 已在全新 disposable Windows Sandbox 完成首次 retarget
安装、真正重装 target switch、两次重启恢复和 manifest 卸载 exact baseline 纵向复验，Gate B 已
标记为 `certified`。

Tauri command 只负责参数解析、DTO 转换和调用应用用例，不直接判断某个游戏目录是否有效，也不直接承担配置文件读写细节。

## MVP 范围

第一版应包含：

- MHW:I 游戏目录识别和手动选择
- Mod 压缩包导入和安全校验
- 预览图提取和校验
- 分类和标签管理
- 基础前置依赖检查
- 安装 / 卸载 / 安装清单
- 基于最终路径的冲突检测
- 手动存档备份
- 一键启动游戏

## 后续范围

MVP 之后再加入：

- 外观、武器、语音替换目标选择
- Profile
- 自动存档备份
- 高级回滚和恢复 UI
- 任务队列 UI
- Linux / Steam Deck 实验性打包和社区测试
