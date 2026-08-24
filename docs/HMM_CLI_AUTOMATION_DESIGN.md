# HMM CLI 与自动化测试设计

> 状态：设计已确认；CLI-0A、CLI-0B、CLI-1A/1B、CLI-2A/2B/2C、CLI-4 Slice B/C、
> CLI-3B（四条单项 lifecycle 的 Production command-level 开放）与
> CLI-3C（batch Production：per-installation secret 签名 token）已实现
>
> 日期：2026-08-02
>
> 范围：后端能力命令化、CLI 契约、自动化测试入口与生产写入安全门禁

## 背景

Helsincy Mod Manager 的主要业务能力已经位于 Rust 后端。React/Tauri 前端负责展示、交互、参数收集
和任务进度消费，不应承担游戏目录、Mod 包、存档或日志的事实判断。为每个后端能力提供稳定的命令行
入口，可以让开发者和 CI 不启动 WebView 就完成跨 crate 的集成验证，也能为故障诊断和后续独立
`hmm` 工具提供基础。

CLI 不是第二套后端，也不是对 Tauri command 的脚本封装。它必须与桌面端、存档备份 worker 共享
同一组 application service、ports、infra adapter、安全校验、任务事件、Task Log 和 Audit Log。

本文建立在以下现状之上：

- 共享 composition、configured executor 和 headless 生命周期测试已迁入 `hmm-runtime`。
- Tauri `AppState` 是 `HmmRuntime` 的薄包装；worker 直接构造 runtime，不再复用 Tauri state。
- 安装、卸载和重装的 Tauri command 仍负责启动线程、发送事件和接入 Task Log。
- 任务 runner 当前主要返回 `Vec<TaskProgressEvent>`，事件通常在 runner 返回后才被适配层消费。
- `GameProfileWriteLockRegistry` 是进程内 `Mutex<HashMap<...>>`，不能协调 GUI、CLI 和 worker。
- CLI-0A 已新增 `hmm-runtime` 与 `hmm-cli`；CLI-0B 已把真实 application service composition
  接入 runtime；CLI-1A 已增加不构造完整 runtime 的只读 game facade。参数解析使用 `clap`，
  CLI-1B 已增加独立的 install、backup 和 diagnostics 只读 facade；`hmm` 当前开放
  `runtime status`、四个 `game` 只读命令、install plan/status/recovery scan/preview、
  backup list/background status、diagnostics snapshot，以及 Sandbox
  `install batch plan/apply/result/retry`。
- T17 已实现第三方管理器批量导入，但默认只导入 HMM，不安装到游戏目录。
- T13-00 已冻结批量安装、卸载和真正重装的领域语义；T13-01/T13-02 已提供批量安装
  BatchPlan、journal、runner 和 retry；CLI-4 Slice B/C 已把 install、uninstall、reinstall 接入
  Sandbox。T13-06/T13-07 已实现窄 Tauri/typed API 与前端 adapter，仍不能由 CLI 循环单项命令来冒充。

项目整体能力与缺口见 [项目任务状态快照](PROJECT_TASK_STATUS.md)。

## 目标

- 为只读扫描、校验、前置检查、安装计划和恢复状态提供稳定机器入口。
- 为人工 fixture 和临时目录提供可重复的导入、安装、卸载、重装和备份端到端测试。
- 让桌面端、CLI 和后台 worker 复用同一 Tauri-free runtime composition。
- 为短命令定义 JSON 契约，为长任务定义 JSONL 事件流。
- 让 CLI 事件、Tauri event、Task Log 和 Audit Log 使用同一个 `task_id` 和稳定 code。
- 明确真实游戏目录、真实 app data、真实存档和 Windows user 级 Scheduled Task 的安全门禁。
- 为 T13 批量安装、卸载和真正重装定义最小 CLI 接入要求，但不提前伪造尚未实现的业务能力。

## 非目标

- CLI-0A 不实现业务命令、共享 composition、跨进程锁或 T13。
- 不把 Tauri command 变成可复用后端 API。
- 不让 CLI 绕过 `InstallPlan`、manifest、backup、rollback 或 recovery。
- 不开放任意目标路径、manifest 路径、backup ref、sandbox 路径或 Scheduled Task 参数。
- 不读取测试机上的真实 Steam userdata、真实玩家存档或真实第三方 Mod 包。
- 不通过 CLI 提供 Steam Cloud、OAuth、跨设备同步或存档恢复。
- 不在普通 CI 中创建、更新、运行或删除真实 Windows Scheduled Task。
- 不把进程退出码扩展成所有业务错误码的镜像。

## 架构决策

### 统一 composition root

CLI-0A 已新增两个 workspace member：

```text
src-tauri/crates/hmm-runtime/   # Tauri-free runtime policy、共享 composition 与 observer contract
src-tauri/crates/hmm-cli/       # hmm 可执行文件和 CLI transport
```

CLI-0B 已落实的依赖方向：

```text
React / Tauri commands -----\
hmm CLI ---------------------+--> hmm-runtime
save backup worker ---------/       |--> hmm-app -------> hmm-ports
                                    |       \-----------> hmm-core
                                    |--> hmm-infra -----> hmm-ports / hmm-core
                                    \--> game adapters -> hmm-ports / hmm-core
```

约束：

- `hmm-runtime` 可以依赖 `hmm-app`、`hmm-ports`、`hmm-infra`、`hmm-core` 和具体 game adapter。
- `hmm-runtime` 不依赖 Tauri、WebView、React DTO 或 CLI 参数类型。
- `hmm-cli` 依赖 `hmm-runtime` 和专用 CLI contract，不依赖 `hmm-tauri`。
- `hmm-tauri` 只保留桌面生命周期、Tauri command/DTO、event emit 和窗口能力。
- `hmm-app` 不依赖 `hmm-runtime`、Tauri 或 CLI。
- 游戏专属发现、前置和路径规则继续位于对应 game adapter。

可共享服务集合和装配逻辑现已抽为 `HmmRuntime`。Tauri `AppState` 只保留 app-data 解析、
GUI-only 启动行为和对 runtime 的解引用；Tauri commands/DTO/event emit 仍留在桌面壳。
`hmm-save-backup-worker` 直接构造 runtime，因此不初始化 WebView 或 Tauri state。

`HmmRuntimeBuilder` 当前提供显式 app-data root 和受控 manifest repository 故障注入。Sandbox
进程内 write capability 已由 CLI-2B 落地；Production 跨进程 admission 与 Test fake capability
模式仍是后续目标。不能把 builder 的路径参数当作 CLI Production 数据根覆盖能力。

### Runtime 模式

CLI-0B 及后续的 `HmmRuntimeBuilder` 目标上至少支持以下受控模式：

| 模式 | 数据位置 | 允许能力 |
| --- | --- | --- |
| `Production` | 操作系统解析的 HMM app data | 只读命令 + CLI-3B 逐项认证的单项 lifecycle 写入（token/双确认/跨进程 admission/锁内重验） |
| `Sandbox` | 调用方显式提供的临时根 | fixture 读写和故障注入 |
| `Test` | 测试进程创建的临时根和 fake ports | Rust 单元/集成测试 |

Production 模式禁止通过通用 `--data-dir` 改写 app data。Sandbox 模式必须显式提供隔离根，并满足：

- 根目录为空，或已包含 HMM 创建的版本化 sandbox marker。
- canonical path 位于调用方声明的临时测试根内。
- 拒绝 symlink、junction、reparse point 和祖先替换逃逸。
- 游戏目录、存档目录、备份目录和 app data 都位于该隔离根内。
- 不回退到系统 Steam root、真实用户 home 或生产 app data。

fixture 只能描述人工文件和稳定 ID，不能携带真实用户名、Steam ID、token、cookie 或本机私有路径。

CLI-0A 固定了不访问文件系统的环境策略：

- Production 禁止 `--data-dir`，数据根模式为 `system`；写命令策略在 CLI-0A 至 CLI-3A 期间
  固定为 `disabled`，CLI-3B 起演进为 `production_command_level`（见跨进程写入门禁一节）。
- Sandbox 必须提供显式绝对路径，并拒绝文件系统根以及包含 `.` / `..` 的词法不安全路径。
- `sandbox_only` 只表示未来写命令的准入范围，不代表已创建 marker、完成 canonical containment
  或签发写 capability。
- CLI-0A 时两种环境只允许 `runtime status`，`productionWritesAllowed` 与
  `businessCommandsAvailable` 均固定为 `false`。CLI-1A 接入只读 game 命令后，
  `productionWritesAllowed` 仍为 `false`，`businessCommandsAvailable` 改为 `true`。

CLI-2B 已补齐 Sandbox 写许可基础：

- `--data-dir` 是调用方声明的隔离 capability 根；只有显式 Sandbox 环境可以申请
  `SandboxWriteCapability`，Production 固定返回 `sandbox_write_production_forbidden`。
- 申请只发生在 CLI-2C lifecycle 写命令运行期，`runtime status` 和 CLI-1 只读命令不会创建 marker。空根会通过
  no-follow 句柄创建固定 `.hmm-sandbox.json` v1 marker；非空根必须已包含完全匹配的 marker。
- marker 不是 capability，也不是可持久化授权秘密。capability 本身构造器和字段私有、不可序列化，
  并保留打开的目录句柄、canonical root 与 volume/file identity 或 dev/inode identity。
- `SandboxWriteRoots` 逐项校验本次操作实际使用的 app-data、game、save、backup 根；词法逃逸、
  canonical 逃逸、symlink、junction、reparse point、marker 篡改与祖先替换全部 fail closed。
- `SandboxWriteAdmission` 借用原 capability，写侧必须在进入安全写阶段前重新验证。Windows 的打开
  句柄直接阻止祖先 rename；允许 rename 的平台通过目录身份变化返回 `sandbox_root_replaced`。

CLI-2B 只建立 CLI-2C 的必要门禁。CLI-2C 已显式接入四条单项 lifecycle 命令，但 capability 仍不
替代 InstallPlan、backup、manifest、rollback/recovery、Audit Log 或 game/profile 写锁，也不自动
开放其他写命令。

### 应用层任务观察器

CLI-0B 已新增 transport-neutral 抽象：

```rust
pub trait TaskProgressObserver: Send + Sync {
    type Error;

    fn observe(&self, event: &TaskProgressEvent) -> Result<(), Self::Error>;
}
```

当前 install、uninstall、reinstall 和 recovery runner 已在任务推进时逐阶段调用 observer，同时继续
返回 `Vec<TaskProgressEvent>` 兼容既有调用方。Tauri adapter 通过该 observer 完成领域事件到
Task Log、App Log 和 Tauri event 的转换，wire DTO 未变化；CLI adapter 则将同一领域事件转换为
`hmm.cli/v1` JSONL。

- `TaskManager` 仍拥有 task 状态迁移和取消事实。
- Tauri observer 继续负责转换安全 DTO、写 Task Log、发送 `hmm://task-progress`。
- CLI observer 从 0 单调分配 `sequence`，先复用同一个 Task Log writer，再 flush JSONL stdout。
- CLI observer 拒绝第二个 terminal 和 terminal 后事件；无效事件不消耗序号。
- 取消 terminal 由发起取消的 transport 发送，runner 只停止后续安全阶段，不能再补第二个 cancelled。
- 测试 observer 收集事件，用于顺序、terminal event 和脱敏断言。
- Audit Log 仍由提交服务和高风险用例写入，不能由 CLI 文案代替。

observer 失败不能改变已经提交的玩家文件事实。Task/Audit 写入失败继续遵守
[日志与审计设计](LOGGING.md) 中的 best-effort 和 `report_after_commit` 语义。

### Contract 分离

领域类型、Tauri DTO 和 CLI schema 是三个边界：

```text
domain/app result -> Tauri DTO -> invoke/event
                  -> CLI schema -> JSON/JSONL
```

CLI 不能直接序列化：

- Tauri DTO。
- 内部 Rust error 的 `Display` 或 `Debug`。
- manifest、recovery transaction 或 backup manifest 正文。
- 完整 game/save/backup/app-data 路径。
- sandbox/cache 路径、backup ref、hash 列表或第三方内容。

CLI contract 独立使用 `schemaVersion` 版本化。字段只允许追加兼容演进；删除、改名或改变语义需要
提升 schema major 版本。

## 命令模型

### 全局参数

建议首个可执行文件名为 `hmm`：

```text
hmm <group> <command> [options]
```

全局参数：

```text
--format human|json|jsonl
--environment production|sandbox
--data-dir <path>             # 仅 sandbox
--no-color                    # 仅影响 human/stderr
```

默认：

- 交互式终端默认 `human`。
- CI 和测试必须显式指定 `json` 或 `jsonl`。
- 长任务使用 `jsonl`；对长任务请求 `json` 时，只允许输出最终 envelope，不能把进度混入 stdout。
- `NO_COLOR` 和 `--no-color` 只影响 human 输出，不改变机器契约。

### 建议命令树

```text
hmm
  runtime
    status
  game
    status
    scan
    validate
    prerequisites
  mod
    import
  external-import
    scan
    plan
    apply
  install
    plan
    apply
    status
    uninstall
    reinstall
    recovery
      scan
      preview
      apply
    batch
      plan
      apply
      result
      retry
  backup
    list
    create
    background
      status
      enable
      disable
  diagnostics
    snapshot
    export
```

命令名表达用户动作，具体 service/DTO 名不进入 CLI。首版不承诺一次实现整棵命令树。

### 分期开放矩阵

| 阶段 | 命令 | 环境 | 说明 |
| --- | --- | --- | --- |
| CLI-0A | `runtime status`、`--help`、schema/contract tests | Production/Sandbox | 建立 runtime policy、envelope、JSONL 和退出码；不装配业务服务 |
| CLI-0B | 无新增业务命令 | Test/Sandbox | 抽取共享 composition，迁移 Tauri/worker，接入 task observer |
| CLI-1 | `game status/scan/validate/prerequisites` | Production 只读、Sandbox | 扫描和前置诊断，不接受任意 game root |
| CLI-1 | `install plan/status/recovery scan/recovery preview` | Production 只读、Sandbox | 只返回计划、状态和聚合问题 |
| CLI-1 | `backup list/background status` | Production 只读、Sandbox | 不触发备份或注册变更 |
| CLI-1 | `diagnostics snapshot` | Production 只读、Sandbox | 复用受控安全快照，不返回日志文件路径 |
| CLI-2A | 无新增写命令 | Test/Sandbox | runner 逐阶段 observer、唯一 terminal event 与 JSONL 顺序事实 |
| CLI-2 后续切片 | `mod import`、`external-import apply` | 仅 Sandbox | 使用人工 archive/source fixture；不自动归入单项 lifecycle task |
| CLI-2B/2C | `install apply/uninstall/reinstall/recovery apply` | 仅 Sandbox | 验证完整安全链路和失败恢复 |
| CLI-2 后续切片 | `backup create`、`diagnostics export` | 仅 Sandbox | 写入隔离 app data/backup 根；需各自独立安全评审 |
| CLI-3B（已实现） | `install apply/uninstall/reinstall/recovery apply` | Production | production token + `--commit --yes` + 跨进程 admission + 锁内 token/事实/游戏根重验 |
| CLI-3C（已实现） | `install batch plan/apply/result/retry` | Production | per-installation secret 签名 token + 双确认 + 跨进程 admission + 锁内根/digest 重验 |
| CLI-3 后续 | `backup background enable/disable`、`backup create`、`diagnostics export` 等 | Production | 按 command 独立评审后开放 |
| CLI-3 | `backup background enable/disable` | Production | 复用固定 registry 用例，不接受 task/path/XML 参数 |
| CLI-4（Slice B/C） | `install batch --operation install\|uninstall\|reinstall` | Sandbox；Production 受 CLI-3 门禁 | 三种 operation 共用 plan/apply/result/retry 与 batch service |

CLI-1 的只读结果不得被外部脚本当作后续写入的永久授权。生命周期写命令必须在获取 game/profile
写锁前重新读取可能包含规则解析或 hash 的前置状态；取得写 admission 和写锁后，再对已封存 token、
manifest/recovery、目标摘要和计划事实做有界重验。

## 命令语义

### 游戏扫描和前置检查

`game scan` 复用 `GameSetupService` 和 game adapter，只允许使用后端受控的 Steam library discovery、
已保存配置或 sandbox fixture。Production 不接受 `--game-root`。

`game validate` 校验已保存候选；sandbox 可通过 fixture 建立候选，但仍由 adapter 判定合法性。

`game prerequisites` 复用已落地的 MHW:I bundled rule 和稳定状态。它是诊断入口，不得把当前尚未
实现的完整安装前依赖阻断描述成已完成。后续接入安装 preflight 时，CLI 与 Tauri 必须共享同一用例。

CLI-1A 的实际命令形态为：

```text
hmm game status --game mhw
hmm game scan --game mhw
hmm game validate --game mhw
hmm game prerequisites --game mhw
```

四个命令不接受 game root。Sandbox 使用 `<data-dir>/config/games.json` 建立已保存候选，
`game scan` 只读取 `<data-dir>/fixtures/steam`。保存目录、VDF library 与 discovery candidate 都必须
落在 `<data-dir>/fixtures` 的 canonical 边界内。CLI 使用不 seed 文件的 prerequisite rule reader；
machine result 只返回聚合状态、evidence/issue code 和计数。

### Mod 导入

`mod import` 可以接受 source archive，因为 archive 是导入输入；它不能接受安装目标、sandbox 输出
路径或最终 metadata 文件路径。Production 写入开放前，该命令仅能在 Sandbox 使用。

导入必须复用 archive 校验、资源预算、受控解压、sandbox、分析、预览图安全处理和持久化链路。原始
archive 保持只读，不能在 source 目录内生成修正版本。

T17 的 `external-import` 可以成为 CLI-2 sandbox 自动化入口，但语义仍是“批量导入 HMM”。它不能
设置 installed/enabled 状态，也不能写游戏目录。

### 安装计划与提交

`install plan` 是 dry-run 入口。它只提交稳定 game/profile/mod/revision/binding 标识，由后端从已保存
配置和导入事实构建 `InstallPlan`。输出可以包含：

- action/conflict/warning 聚合计数。
- 稳定 target family 或逻辑相对路径。
- prerequisite 状态和稳定 code。
- 版本化、短期有效的 opaque `planToken`，或只用于 stale guard 的 plan digest。Digest 本身不是写入
  授权，不能替代 admission。

输出禁止包含 game root、source root、target absolute path、backup ref、manifest path、sandbox path
或第三方内容。

CLI-1B 当前实际命令形态为：

```text
hmm install plan --game mhw --mod <mod-id>
hmm install status [--game mhw] --profile <profile-id> --mod <mod-id> [--mod <mod-id>...]
hmm install recovery scan --game mhw --profile <profile-id> [--mod <mod-id>...]
hmm install recovery preview --game mhw --profile <profile-id> --mod <mod-id> \
  --action rollback-install|reconcile-reinstall
```

CLI-2C 在 Sandbox 落地、CLI-3B 起对 Production 开放同一形态（Production 使用 production
环境签发的 token，不接受 `--data-dir`）：

```text
hmm install apply --game mhw --profile <profile-id> --mod <mod-id> \
  --plan-token <token> --commit --yes
hmm install uninstall --game mhw --profile <profile-id> --mod <mod-id> \
  --plan-token <token> --commit --yes
hmm install reinstall --game mhw --profile <profile-id> --mod <mod-id> \
  --candidate-revision <revision-id> --plan-token <token> --commit --yes
hmm install recovery apply --game mhw --profile <profile-id> --mod <mod-id> \
  --action rollback-install|reconcile-reinstall --plan-token <token> --commit --yes
```

省略任一确认参数时，四条命令只返回同源 preview。只有 ready/available preview 签发 5 分钟
`hmm-lifecycle-plan-v1` opaque token；CLI-3B 起 Sandbox 与 Production preview 各自签发，
环境标签参与 token digest，跨环境重放一律 `plan_token_invalid`。token 不包含路径、manifest、
backup/recovery ref 或内部 reinstall token。uninstall/recovery token 额外绑定 repository 读取出的完整结构化
manifest/install-recovery/reinstall-recovery 状态摘要，因此内容变化即使聚合计数相同也会使旧 token
失效。

plan 当前使用后端固定 base layer，并只输出经 `InstallTargetPath` 校验的逻辑相对 target、priority 和
聚合计数；不输出 package file id 或自由 layer 名。status 省略 `--game` 时只读 manifest，提供 game
时使用 recovery-aware 状态。recovery scan 省略 `--mod` 时扫描受控 profile 状态，preview 只做聚合
可用性判断，不执行动作。持久化状态枚举出的 Mod ID 会在投影前重新校验，逻辑 target 含控制字符时
fail closed，避免篡改状态或第三方文件名进入 CLI 输出。

`install apply` 不是复制命令。它必须：

1. 获取对应写 admission。
2. 在锁外重新读取 prerequisite 规则、配置和文件 hash，并与 token 绑定的 decision 比较。
3. 在锁内重新校验 game/profile、recovery、manifest、候选 revision 和受控目标摘要。
4. 重建计划并校验 `planToken` 或等价计划事实。
5. 执行 conflict/preflight、backup、commit、manifest、rollback/recovery。
6. 写 Task Log 和 Audit Log。
7. 只返回稳定任务结果和聚合摘要。

计划过期、配置变化、目标摘要变化或前置状态变化时必须 fail closed，要求重新执行 `install plan`。

### 卸载、重装和恢复

`uninstall` 只消费受控 manifest、`installed_file` 摘要和 backup 事实，不根据当前 Mod 包猜测。

`reinstall` 复用真正重装的 retained/replaced/added/stale 计划、snapshot 和 recovery transaction。
CLI layer 固定为 `base@0`，只接受 candidate revision ID；外层 lifecycle token 绑定同源 preview 和
既有内部 reinstall token，但 machine output 不公开内部 token。

`recovery scan/preview` 只返回状态、issue code 和聚合计数。`recovery apply` 只执行后端已经证明
available 的受控动作，并在持锁区重新验证。

这些命令都不能提供“强制删除”“忽略 hash”“指定 backup ref”或“直接编辑 manifest”选项。

### 存档备份和 Windows 后台保护

`backup create` 复用 `SaveBackupTaskRunner -> SaveBackupService -> SaveBackupWriter/Repository/AuditLog`，
不建立第二条备份写入链路。它只接受 game/profile 和受控 note，不接受 source save path、目标目录、
archive 文件名、manifest 文件名或文件列表。

Steam 多账号存档目录仍通过既有后端发现和显式选择流程管理。CLI 不能通过 account id 或绝对路径
跳过 pending candidate 确认，也不实现 Steam Cloud 或存档恢复。

`backup background status` 是只读状态查询，不注册、不修复、不启动 worker、不获取 scheduler lease。
后续 `enable/disable` 必须复用全局应用用例和固定 Windows registry adapter，不接受 task name、SID、
worker path、PowerShell、XML、profile、save path 或 lease 参数。

`hmm-save-backup-worker.exe --once` 保持独立、固定、最小的系统入口。通用 `hmm` CLI 不取代 worker，
也不增加任意路径参数。普通 CLI/CI 测试使用 fake registry、fixed clock 和临时 SQLite；P7.2a 的真实
Scheduled Task 验收只能在一次性 Windows 账户或 disposable VM 中执行，并必须完成幂等 cleanup。

### 诊断与日志

`diagnostics snapshot` 复用 `/diagnostics` 的受控读取服务。`diagnostics export` 复用
`export_support_diagnostics`，只返回 export ID、文件名、大小和聚合计数，不接受输出路径或日志路径。

CLI 自身不得新增“打印原始错误”“显示内部路径”“dump manifest”或“读取任意日志文件”开关。

## 写命令确认协议

所有会修改 sandbox、app data、游戏目录、备份目录、数据库或系统注册状态的命令默认 dry-run。

真正提交必须同时满足：

```text
--commit --yes
```

规则：

- 缺少任一参数时不写入，只返回计划或稳定拒绝 code。
- 非交互环境不允许通过 stdin 隐式确认。
- `--yes` 只确认本次已展示/已绑定的计划，不绕过冲突、前置、安全校验或锁。
- plan/apply 之间必须使用后端生成的 opaque token，或版本化 digest 作为 stale guard 防止 TOCTOU；
  裸 digest 不是 capability，不能授权写入或替代 admission。
- token 绑定 command、game、profile、mod/revision、计划摘要、环境和 schema version。
- production token 不能在 sandbox 使用，反之亦然。
- 写入开始前必须再次验证 token、配置和目标状态。

不提供全局 `--force`。确有必要的业务选择必须建模为有名字、有审计、有测试的领域选项。

## 跨进程写入门禁

### CLI-3A 当前状态

`GameProfileWriteLockRegistry` 与 `SaveProfileMaintenanceScopeRegistry` 继续负责进程内串行；CLI-3A 已在
其外层增加共享跨进程 admission。`hmm-ports` 定义窄 port，`hmm-infra` 提供平台实现，
`HmmRuntime::from_builder` 为 GUI、Sandbox CLI 和固定 worker 装配同一 coordinator。当前区分：

| scope | 保护对象 |
| --- | --- |
| `game-profile-write` | 安装、卸载、重装、retarget、安装恢复 |
| `save-profile-write` | 手动/自动备份、retention、存档恢复 |
| `background-registration-write` | 后台保护 enable/disable 和 owned task registration |

已落地约束：

- lock key 只使用稳定 game/profile ID 或固定全局 scope，不使用完整路径。
- acquisition 有明确 timeout、取消和稳定 `write_admission_*` code。
- Windows 使用当前用户命名空间内的 `Global\\HelsincyModManager.WriteAdmission.v1.<digest>` mutex；
  `WAIT_ABANDONED` 是 owner 强退后的恢复证据。
- Unix 使用 canonical app-data 下 capability-relative、no-follow 打开的 `write-admission/` lock file；OS
  advisory lock 是互斥 authority，残留 owner record 只投影 stale-owner evidence。
- 同一 scope 在 GUI、CLI 和 worker 之间互斥。
- 获取全序固定为 `background < save < game`，同 rank identity 再按稳定 game/profile ID 排序；逆序和
  同 scope 重入在平台等待前返回 `write_admission_order_violation`。
- 禁止在持有 game scope 或游戏写锁时执行长时间扫描、hash 或 archive 分析。
- 进入锁后重新读取所有安全事实；锁外 plan 只用于预览。
- release 失败进入安全日志，但不能伪造业务回滚。
- 自动备份现有 scheduler lease 继续负责 due claim，不能被误当成所有 save 写入的通用互斥锁。

两个独立进程竞争同一 scope、timeout、取消、不同 scope/profile、owner 强退恢复和非法 namespace 已有
集成测试。Windows 本机覆盖 named mutex 分支；Unix capability/file-lock 分支由 Ubuntu required CI
编译运行，不能用 Windows 结果替代。2026-08-16 disposable Windows synthetic gate 进一步覆盖
GUI/CLI/worker 的安装、存档和后台注册竞争、释放后写入以及最终 task/evidence 清理，CLI-3A 已认证。

### CLI-3B 后仍然保留的 Production 硬门禁

CLI-3A 建立共享互斥基础；CLI-3B（2026-08-24）按 command 复核后开放了四条单项 lifecycle
命令的 Production 写入。以下硬门禁不变：

- Production 继续禁止 `--data-dir`，数据根仅由操作系统解析；不允许隐藏环境变量、debug flag
  或未文档化参数改变数据根或开启额外写命令。
- 已开放的四条命令各自要求 production token（环境标签参与 digest，与 sandbox token 互不通用）、
  显式 `--commit --yes`、`game-profile-write` 跨进程 admission，以及锁内 token/事实/游戏根
  一致性重验；游戏根只能来自已保存配置，锁内重载与锁外记录不一致或已消失时 fail closed。
- `install batch` 已由 CLI-3C（2026-08-24）开放 Production：token 由 per-installation
  secret 签名（不可离线伪造），纯语法预检前置于一切数据根读取；
  `backup create/restore/background enable|disable` 与 `diagnostics export` 仍在 parser 边界
  不可达。后续每个 command 仍须独立复核 capability、token、Audit、锁内事实和 disposable
  Windows 验收，GUI 测试、单进程锁测试或仅取得跨进程 guard 都不是授权证据。

## 批量安装/卸载/真正重装

本节服从 [批量 Mod 生命周期领域设计](BATCH_MOD_LIFECYCLE_DESIGN.md)。T13-00 冻结语义；
T13-05 已在 Sandbox 开放 `install batch` 的 install、uninstall 和 reinstall 三种 operation。

### 不允许的实现

以下实现不满足 T13，也不能进入 `hmm install batch apply`：

```text
for mod in mods:
  invoke single install command
```

它无法提供统一快照、跨 Mod 冲突、固定锁顺序、批次取消、计划摘要、幂等重试和一致结果。

### 最小业务语义

T13 至少需要：

- 只读 `preview` 规范化完整 request、生成 batch plan 摘要和短期 opaque preview token；不得写
  journal、projection、Audit、manifest、backup、recovery 或临时产物。
- `seal` 重新读取当前事实并验证 request、preview token 和 digest；一致时才持久化不可变 sealed
  batch、签发 `batchId` 与短期 opaque plan token。
- Preview 只使用 `ready/blocked`：默认策略要求零 item blocker；continue 策略允许存在 isolated
  blocked item，但至少要有一个 ready item。Blocked preview 不签发 token。
- `start` 只接受 `batchId + planToken`，不会让 CLI 提交 target path、manifest generation、
  backup ref、hash 或任意 item ID。
- 同一 batch attempt 通过后端 CAS 只获得一次执行 admission；重复 start 不得重复写入。
- 在一个一致的 profile/manifest 快照上计算跨 Mod target 冲突。
- 同一 game/profile 的写入串行，不同 game/profile 是否并行由 task policy 决定。
- 每个 Mod 仍经过 InstallPlan、backup、manifest、rollback/recovery。
- 默认在任何阻断项存在时于写入前终止整个批次。
- 执行阶段不宣称全局原子事务；已成功项保留真实成功事实。
- 失败策略为默认 `stop_on_failure` 或显式 `continue_on_item_failure`；continue 只能越过未写入或
  rollback 已证明成功的可隔离 item failure，不能越过 global blocker、recovery required 或证据故障。
- 取消停止启动新项；运行中项只在安全检查点取消并完成一致性收尾。
- 单项结果区分 `succeeded`、`blocked`、`failed`、`recovery_required`、`cancelled` 和 `skipped`；
  `retryable` 是独立布尔事实。
- 重试只接收 `batchId + expectedAttemptNumber`，不接收 item IDs；由后端从同一 sealed batch 和已有
  终态计算 retryable 项。已成功项和 recovery-required 项不重复提交，并发 retry 最多一个创建下一
  attempt。
- Audit Log 既有 per-item 证据，也有不含路径/内容的 batch 聚合证据。

批量安装、卸载和真正重装使用同一 `preview -> seal -> start -> result/retry` 协议，但 item 输入与
事实来源不同：安装绑定候选 revision/layer/binding；卸载只绑定已安装 revision 并消费
manifest/recovery；真正重装同时绑定 installed/candidate revision、layer 和可选 binding，复用既有
retained/replaced/added/stale 与 durable transaction。跨 item 最终 target/remove/restore 重叠、
backup ownership 不明确或旧 manifest 缺少摘要都是 global blocker，不能由执行顺序或 continue 策略绕过。

首版固定上限是每批 100 项、50,000 个 target action、16 MiB canonical plan；当前 result 返回完整
bounded snapshot，尚未实现 cursor/limit 分页参数；preview token 默认 30 分钟。机器输出只公开短 ID、稳定 status/code、聚合计数和
retryable，不返回路径、Steam ID、digest、backup/snapshot ref、manifest/source 正文、hash
列表或原始错误。`plan` 的直接响应可以返回短期 opaque `previewToken`，供用户确认后作为
`apply --preview-token` 输入；该 token 不写入 journal、Task Log、Audit Log 或其他持久化状态。
Sandbox batch token 是可伪造的 stale/plan tag（其安全性由 sandbox 隔离承担）；Production
batch token 自 CLI-3C 起由 per-installation secret（app data `secrets/` 下首次生成的随机
256-bit key，损坏即轮换、旧 token 失效）签名，不可离线伪造，且跨环境互不通用。写入仍经
CLI-3A 跨进程 admission 与锁内逐项 digest/游戏根重验。
`seal` 返回的 plan token 只在 adapter 内存中传递，不作为 CLI 参数或机器输出暴露。
Result 查询绑定确切 attempt；CLI 在 retry 后必须使用新 attempt，不能查询隐式“最新结果”。成功读取
result 时，非终态 `sealed/queued/running/stopping` 返回 `0`；terminal 状态与 apply/retry 使用同一退出码
映射，其中 `completed_with_errors` 返回 partial exit code `5`，blocked/failed/recovery required 等不会
伪装成成功查询。

CLI-4 的共同基线是 T13-00、CLI-2A、CLI-2B、CLI-2C 和 CORE-PREF-01。Sandbox 的
`install batch plan/apply/result/retry` 已通过批次级 `--operation` 支持 install、uninstall 和
reinstall。CLI 只映射同一 app use case，不能自行决定批量原子性、retryable 谓词或写入顺序。
Production 继续等待 CLI-3 跨进程 admission。

同一 `install batch` 命令组通过批次级 `--operation install|uninstall|reinstall` 暴露同一组子命令：

```text
plan
apply
result --batch-id <id> --attempt <n>
retry --batch-id <id> --attempt <n>
```

`plan` 只执行 preview 并返回脱敏摘要及短期 opaque `previewToken`。`apply` 要求
`--commit --yes --preview-token <token>`，在构造写上下文前先以只读 facts 验证 token；验证通过后
才初始化 Sandbox journal，并在真正 `seal` 时再次重读 facts 和验证 token，防止 stale preview
留下数据库副作用或跨越 TOCTOU。`result` 必须查询显式 attempt，不能使用隐式 latest。`retry`
成功后返回新 task identity 和 attempt number；调用方必须使用该 attempt number 重新执行 `result`。
当前 `result` 返回完整 bounded snapshot，不接受 cursor/limit。
JSONL `apply`/`retry` 先输出一个 parent batch terminal event，再输出 command result；两行共用同一
`taskId`，不会伪造每个 item 的 child task event。
`apply`/`retry` 保留只读 scope 预检，用于在创建新副作用前尽早拒绝遗留 active attempt；最终准入
由 SQLite `BEGIN IMMEDIATE` 短事务完成。事务在同一连接内验证目标 batch/attempt/token，检查同一
game/profile 的 `queued`、`running` 或 `stopping` attempt，再原子完成 sealed -> queued，因此两个
独立进程竞争同一 scope 时最多一个成功。最终 retry admission 竞争失败时，只回收仍 sealed、没有
item result 且 token verifier 匹配的未执行 retry attempt；清理无法证明时保持 fail closed。
`result` 不做 scope-wide reconciliation，只读取调用方明确指定的 batch/attempt，所以即使 sibling
batch 留有 active attempt，已存在结果仍可安全查询和诊断。当前不会自动续跑或把遗留 attempt 标记为
终态。上述原子 admission 只属于 Sandbox batch journal，不会解除 Production CLI-3 对通用
game/profile 跨进程 admission 的依赖。

## 输出协议

### stdout 与 stderr

- `stdout` 只输出 command result 或 JSONL event。
- `stderr` 输出 human 提示和经过脱敏的安全诊断。
- `json/jsonl` 模式不输出 banner、颜色、进度条、日志前缀或内部 stack trace。
- tracing、App Log、Task Log 和 Audit Log 不写入 stdout。
- 机器模式即使失败也必须输出一个合法 terminal envelope，除非进程在读取参数前无法启动。
- clap 解析失败在能够识别 `--format json|jsonl` 时输出 `command=cli.parse`、
  `error.code=cli_usage_error` 的脱敏 envelope；不会回显原始参数、路径或 clap 错误文本。未指定机器
  格式时保留标准 human help/error 行为。

### JSON envelope

短命令使用：

```json
{
  "schemaVersion": "hmm.cli/v1",
  "command": "game.prerequisites",
  "ok": true,
  "taskId": null,
  "result": {
    "gameId": "mhw",
    "status": "warning",
    "issueCodes": ["prerequisite_unverified"]
  },
  "error": null
}
```

失败示例：

```json
{
  "schemaVersion": "hmm.cli/v1",
  "command": "install.apply",
  "ok": false,
  "taskId": "install-opaque-id",
  "result": null,
  "error": {
    "code": "write_admission_busy",
    "category": "user_action_required",
    "retryable": true
  }
}
```

要求：

- `command` 使用稳定点分 ID，不复制 shell 参数原文。
- `taskId` 只在任务已经登记时出现。
- `error.code` 是稳定白名单 code，不是内部错误文本。
- `category` 使用有限稳定集合，例如 `user_action_required`、`recoverable`、
  `rollback_succeeded`、`rollback_failed`、`data_safety_risk`、`internal_bug`。
- `result` 只含当前 command 的安全 projection。

### JSONL 长任务

长任务每行一个完整 JSON object：

```jsonl
{"schemaVersion":"hmm.cli/v1","type":"started","command":"install.apply","sequence":0,"taskId":"install-opaque-id","status":"queued","phase":"install.queued"}
{"schemaVersion":"hmm.cli/v1","type":"progress","command":"install.apply","sequence":1,"taskId":"install-opaque-id","status":"running","phase":"install.plan.building"}
{"schemaVersion":"hmm.cli/v1","type":"completed","command":"install.apply","sequence":2,"taskId":"install-opaque-id","status":"completed","phase":"install.completed","result":{"managedFileCount":3}}
```

约束：

- `sequence` 在单个进程/任务内从 0 单调递增。
- 每个已启动任务恰好有一个 terminal event：`completed`、`failed` 或 `cancelled`。
- `phase` 与应用层稳定 phase 一致。
- `current`、`total`、`result` 和 `error` 仅在语义适用时出现，缺席时省略。
- task event 的 `error` 仅允许 `{ "code": "<stable_code>" }`，且只出现在 `failed` 事件。
- `message` 自由文本、原始 error 和 `result_ref` 不进入 JSONL。
- Ctrl+C 请求协作式取消；CLI 等待任务到达 terminal state 后退出。再次中断可强制退出，但必须在
  stderr 提示“状态需通过 recovery/status 重新确认”，不能伪造 cancelled。

### 退出码

退出码只表达少量脚本级类别：

| code | 含义 |
| --- | --- |
| `0` | 成功 |
| `2` | 参数、schema 或用法错误 |
| `3` | 前置/冲突/安全门禁拒绝，未执行写入 |
| `4` | 操作失败但已进入受控失败或恢复语义 |
| `5` | 批次部分成功 |
| `6` | runtime、存储或内部能力不可用 |
| `130` | 任务已确认取消，或收到终止信号后退出 |

脚本要判断具体原因时读取 `error.code`，不能依赖新增退出码。未执行写入的参数、
前置条件或安全门禁拒绝使用 `3`，即使 error category 为 `data_safety_risk`；
只有已经进入受控写入或恢复语义的 `rollback_failed` / `data_safety_risk` 才使用
`4`，并且必须由 error category 和 Audit Log 明确标识。CLI-1A/CLI-1B 只读命令以及 CLI-2C 在
task/write 前发生的 token、containment、前置或 Production 拒绝都使用退出码 `3`，不会伪装成已经
开始执行的受控失败。

生命周期 facade 只持有稳定 task failure code 和 task id、无法证明底层回滚结果时，必须保守投影为
`data_safety_risk`，不能声称 `rollback_succeeded`。机器消费者应结合 Task/Audit Log 或后续
recovery/status 查询确认真实恢复状态。

## 自动化测试设计

### 测试层级

| 层级 | 目标 | 建议入口 |
| --- | --- | --- |
| Domain/unit | 纯规则、计划、状态机、错误映射 | `cargo test -p hmm-core/-p hmm-app` |
| Runtime composition | Tauri-free 真实 adapter 装配 | `cargo test -p hmm-runtime` |
| CLI contract | 参数、JSON/JSONL、退出码、stdout/stderr | `cargo test -p hmm-cli` |
| Sandbox E2E | import -> plan -> install -> restart -> uninstall/recovery | 启动真实 `hmm` binary |
| Cross-process | admission、取消、并发、重启恢复 | 两个或更多 `hmm` 子进程 |
| Windows acceptance | sibling worker、Scheduled Task、heartbeat、cleanup | disposable VM，非普通 CI |

CLI contract 测试可以使用 `assert_cmd`、`serde_json`、`tempfile` 等成熟测试库；最终依赖选择在实现
切片中按 workspace 版本策略确认。

### Fixture 规范

建议目录：

```text
tests/fixtures/cli/
  games/mhw-minimal/
  saves/mhw-account-a/
  archives/valid-minimal.zip
  archives/path-traversal.zip
  archives/case-collision.zip
  external-import/valid-source/
  scenarios/
```

fixture 必须：

- 是人工构造的最小样本。
- 不包含真实 Mod、存档、Steam XML、用户名或 account id。
- 用稳定 clock、稳定 ID generator 和受控 adapter 输出保证可重复。
- 对 archive bomb 使用生成式边界测试或小型伪实现，不提交巨大二进制。
- 所有写入落在测试创建的临时根。
- 测试结束后验证外部 sentinel 未变化。

### 核心场景矩阵

| 场景 | 关键断言 |
| --- | --- |
| game scan 无候选/单候选/多候选 | 稳定状态、无真实路径、无隐式多账号选择 |
| prerequisite 缺失/损坏/verified/unverified | 稳定 code、严重级别正确、无原始配置内容 |
| archive traversal/symlink/collision/预算超限 | 写入前拒绝、sandbox 外 sentinel 不变 |
| install dry-run | 无文件/DB/manifest/Audit 写入 |
| install success | plan、backup、commit、manifest、Task/Audit taskId 一致 |
| manifest save 失败 | rollback 结果和 recovery 状态正确 |
| uninstall 文件被玩家修改 | fail closed，不删除不匹配文件 |
| reinstall 失败/重启 | durable transaction 可扫描、可受控收敛 |
| backup success/retention failure | 新备份事实正确，retention warning 不篡改成功 |
| manual backup 与 worker 并发 | `save-profile-write` admission 串行 |
| GUI/CLI 竞争安装 | 仅一个获得 `game-profile-write` admission |
| Ctrl+C | terminal event 唯一，状态与 TaskManager 一致 |
| Task/Audit writer 失败 | 业务事实不被伪回滚，evidence health 明确降级 |
| JSON/JSONL golden | schema 稳定、stdout 无日志、stderr 无敏感数据 |
| production write gate | CLI-2 对 production 一律拒绝 |
| batch partial success | 已成功项不重放，可重试项来自 sealed plan |

### 日志和脱敏断言

每个 CLI E2E 都应扫描：

- stdout。
- stderr。
- App Log。
- 对应 Task Log。
- Audit Log。
- diagnostics export。

断言不得出现：

- temp 根的完整绝对路径。
- 人工用户名/Steam ID/token/cookie/API key canary。
- manifest/backup/sandbox ref。
- archive README 或伪存档内容 canary。
- Rust `Debug` error、panic backtrace 或第三方工具原始 stdout/stderr。

同时断言 task event、Task Log 和 Audit Log 使用同一 `task_id`，且 terminal event 只有一个。

## 实施切片

### CLI-0A：runtime policy 与 contract 骨架

已实现：

- 新增不依赖 Tauri 或业务 crate 的 `hmm-runtime` 环境/写策略类型。
- 新增 `hmm-cli`、`clap` 参数解析、`hmm.cli/v1`、JSON/JSONL writer 和稳定退出码。
- 新增只读 `hmm runtime status`，并使尚未开放的业务命令在 parser 边界拒绝。
- CLI-0A 的 human/help/error 输出统一禁用 ANSI；`--no-color` 保留为向后兼容的全局参数。
- 覆盖 Production 禁止数据根覆盖、Sandbox 显式绝对根、文件系统根/`.`/`..` 拒绝、机器错误
  envelope、stdout/stderr 分离和路径不回显。

完成定义：

- CLI-0A 交付时 `hmm-cli` dependency tree 不包含 Tauri、`hmm-tauri`、`hmm-infra` 或真实文件系统
  adapter；CLI-0B 接入共享 runtime 后会传递包含业务/infra crate，但仍不得包含 Tauri。
- CLI-0A 至 CLI-3A 期间 Production 报告 `productionWritesAllowed=false`、
  `writeCommandPolicy=disabled`；CLI-3B 起演进为 `true` / `production_command_level`。
- `sandbox_only` 不被当作已具备安全写 capability。
- 聚焦 format/test/clippy 通过，测试不访问真实游戏、Steam、存档、AppData 或 Scheduled Task。

### CLI-0B：共享 composition 与 task observer

已实现：

- 扩展 `hmm-runtime`，从 `hmm-tauri::state` 抽取共享 composition、configured executors 和测试。
- Tauri `AppState` 改为 `HmmRuntime` 薄包装；`hmm-save-backup-worker` 直接构造 runtime。
- 引入 `TaskProgressObserver`，Tauri adapter 保持现有 event、Task Log 和 queued App Log 行为。
- manifest repository 故障改用显式 builder 注入，不再使用 thread-local override。
- headless lifecycle 测试继续覆盖安装、重装、retarget、卸载、恢复、baseline、共享锁和后台备份。

已满足：

- `hmm-cli` 不依赖 Tauri。
- worker 不初始化 WebView/Tauri state。
- workspace dependency direction 检查通过。
- 现有桌面 command contract、DTO 和 task phase 未变化。
- 聚焦 check/test/clippy 通过，测试未访问真实游戏、Steam、存档、AppData 或 Scheduled Task。

后续 CLI-2A 已完成这里保留的边界：runner 会在阶段推进时实时调用 observer，同时继续返回事件集合
兼容旧调用方。

### CLI-1A：只读游戏自动化入口

已实现：

- `game status/scan/validate/prerequisites`，支持 `human|json|jsonl` 与 `hmm.cli/v1`。
- 独立 `ReadOnlyGameAutomation` composition；不构造会打开 SQLite、恢复批次或使 projection dirty
  的完整 `HmmRuntime`。
- Production app-data identifier 在 Rust runtime 集中定义，CLI 与固定 `--once` worker 复用；
  测试约束其与 Tauri config 一致。
- Sandbox fixed Steam root、VDF library/candidate containment、已保存目录 canonical admission。
- 只读 prerequisite rules fallback，不 seed override；规则 path 只允许安全相对路径。
- path-free snapshots、稳定错误/退出码、单行短命令 JSONL、binary no-write 树快照测试。

### CLI-1B：其余只读自动化入口

- [x] 实现 install plan/status/recovery scan/preview。
- [x] 实现 backup list/background status 和 diagnostics snapshot。
- [x] 延续 JSON/JSONL contract、脱敏、parser write gate 和 Sandbox no-write 测试。

已实现的 backup facade 只读取已 checkpoint 且没有 `hmm.db-wal`/`hmm.db-shm` sidecar 的既有
SQLite，并通过 percent-encoded immutable URI、read-only flags 与 connection-local query-only
mode 打开。任一 sidecar 存在时返回脱敏 `backup_database_unavailable` 并 fail closed；CLI 不读取
live WAL，不 checkpoint、修复、创建或修改 DB/WAL/SHM，也不迁移或 seed schema。Sandbox
background status 使用固定 JSON registry/clock，只允许 inspect，register/unregister fail closed。
immutable opener 不提供跨进程快照锁；需要一致结果的 backup 查询必须在桌面端关闭、数据库静止后
执行。diagnostics facade 复用从 support export 中抽出的 reader-only page snapshot service，
只输出受控平台摘要、分类状态和计数，不返回日志正文、来源名、Audit fields 或路径。

完成定义：

- 可在没有 WebView、没有真实游戏安装的环境运行所有 fixture。
- Production 模式没有写命令可达路径。
- 所有错误通过稳定 code 表达。

### CLI-2A/2B/2C：Observer、Sandbox 写许可与单项 E2E

- CLI-2A 已让 install/uninstall/reinstall/recovery runner 逐阶段调用 observer，并锁定 task id、
  sequence、phase、取消 ownership、唯一 terminal、Task Log 顺序和脱敏 JSONL；未新增写命令。
- CLI-2B 已建立 Sandbox marker/capability、canonical containment、目录身份与写侧重验。
- CLI-2C 已接入 install/uninstall/reinstall/recovery apply，使用人工 repository/package fixture，
  并覆盖独立进程 token、失败注入、取消、重启恢复和 sentinel containment。
- backup create 和 diagnostics export 仍按独立安全切片开放，不能被生命周期写许可自动解锁。

完成定义：

- 完整 Gate A 类链路可由 `hmm` binary 在临时根端到端执行。
- sandbox marker、路径 containment 和 production gate 均有负向测试。
- 不读取真实 Steam、AppData、游戏或存档。

### CORE-PREF-01：单项 lifecycle 前置 decision

- `GamePrerequisiteDecisionProvider` 是 desktop、CLI、install 和 true reinstall 的 app-level
  单一事实源；transport 不解析诊断 message、路径或 MHW:I 规则。
- install/reinstall preview 都返回 `prerequisiteDecision { status, rulesVersion, codes }`。
  required missing、rules unavailable/corrupted 和无法证明的状态为 `blocked`，不签 token；
  `signature_unverified` 为显式 `warning`，允许继续但不能显示为 success。
- install lifecycle token 直接绑定 decision facts；reinstall 内部 token 绑定 decision，外层
  Sandbox token 绑定完整 preview/internal token。规则版本、codes 或 status 漂移都会使旧 token
  失效。
- 规则读取、hash、配置检查和最终 decision 重验都在 game/profile 写锁外；runner 在获取锁前立即将
  最终 decision 与 preview/token facts 比较。锁内只校验已封存 token/plan、identity、containment
  和当前 manifest/目标状态，blocked 或漂移在 staging、manifest、commit 和 game write 前 fail closed。
- CLI/Tauri DTO 不返回 issue path、display message、配置正文、完整本地路径或第三方 Mod 内容。

完成定义：

- install/reinstall 的 required-missing blocked/no-token 与 unverified warning/token 由真实 binary
  contract 固定。
- 桌面 install/reinstall runner 的锁前 decision drift tests 证明 prerequisite provider 不在写锁内
  调用，且发生漂移时 commit service 和文件写入未发生。
- Tauri DTO、frontend typed contract 和 UI warning/blocked 状态消费同一 decision。

### CLI-3A/3B：跨进程 admission 与生产写入

CLI-3A 已完成并认证（2026-08-16）：

- game/save/background registration admission ports 与 Windows/Unix 平台实现已落地。
- Tauri、Sandbox CLI 和固定 worker 通过共享 runtime composition 使用同一 admission。
- 双进程竞争、取消、timeout、owner 强退恢复、顺序拒绝、锁内重验和稳定错误投影已有自动化。
- 本地完整 `scripts/verify.ps1`、findings-first 全 diff 审查、Ubuntu required CI run `31910573714` 和
  disposable Windows synthetic gate 均已通过；Windows gate 覆盖 busy fail-closed、timeout/cancel、
  abandoned owner、释放后 worker backup 增长与 background registration 双向 mutation。
- Production parser/runtime 写门禁没有改变。

CLI-3B（2026-08-24 实现）按 command 复核后开放了 `install apply`、`install uninstall`、
`install reinstall`、`install recovery apply` 四条 Production 写命令：

- preview 在两种环境分别签发 token，环境标签参与 digest，跨环境重放一律失效；prepare 前置
  纯语法 token 预检，过期/非法 token 在触达任何数据根之前 fail closed。
- lifecycle automation 双环境化：Sandbox 走 marker/containment capability；Production 的根事实
  是 prepare 阶段从已保存配置读取的游戏根，锁内重载配置并要求与锁外记录一致且仍存在，
  配置漂移 fail closed。Audit/Task Log、跨进程 admission 与既有安全链路完全复用。
- `runtime status` 契约演进：Production `writeCommandPolicy=production_command_level`、
  `productionWritesAllowed=true`；Sandbox 报告不变。
- 未逐项认证的写命令不随本切片开放（batch 随后由 CLI-3C 单独复核开放）：`install batch`（前置 per-installation secret）、
  `backup create`、`diagnostics export`、`backup background enable/disable`。
- disposable Windows 环境的 GUI/CLI/worker 竞争与真机安装态验收按独立 gate 记录，
  不并入普通 CI 结果。

完成定义：

- CLI-3A 三类 scope 在 GUI/CLI/worker 中共享且有双进程与故障测试证据。
- 每个 CLI-3B 开放写命令都有对应 capability、跨进程 scope、锁内重验和验收证据。
- 不存在 debug/环境变量绕过。
- Windows 安装态验收与普通 CI 结果分开记录。

### CLI-4：T13 批量安装/卸载/真正重装

- 共同前置包含 T13-00、CLI-2A/2B/2C 和 CORE-PREF-01；不要求每个 Sandbox parser 等待所有
  operation 的领域实现。
- Slice B 在 T13-01 后接入批量安装 plan/parser，在 T13-02 后接入 preview/seal/start、
  sealed snapshot、冲突、partial result、retry 和对应机器 contract。
- Slice C 在 T13-03/04 后接入批量卸载、真正重装、取消/恢复和剩余机器 contract。
- 每个 CLI adapter 都映射已经存在并完成聚焦测试的 app use case，不在 shell 中抢跑领域语义。
- 增加跨 Mod 冲突、partial success、幂等重试、批量 true reinstall 和跨进程竞争测试。

完成定义：

- CLI 不循环调用单项 command。
- batch 结果可重放、可审计、可恢复。
- 文档、前后端契约、日志、测试和路线图同步更新。

## 文档同步

实施时至少同步：

- [架构设计](ARCHITECTURE.md)：新增 runtime/CLI crate 与依赖方向。
- [前后端通信契约](FRONTEND_BACKEND_CONTRACT.md)：共享 task phase 的来源和 Tauri adapter 变化。
- [日志与审计设计](LOGGING.md)：CLI sink、Task Log 和脱敏边界。
- [测试指南](TESTING.md)：CLI contract、sandbox E2E 和跨进程测试命令。
- [安全策略](../SECURITY.md)：CLI production write gate 和 fixture 约束。
- [项目任务状态快照](PROJECT_TASK_STATUS.md) 与 `TODO.md`：分期状态和 T13/P7.2a 依赖。

新增或修改核心 docs、crate boundary、Tauri command、安装写入、存档备份、日志或跨进程 admission
时，继续按项目 review gate 执行人工复审。

## 设计完成标准

本文设计进入实现前，应确认：

- runtime 与 CLI 的依赖方向没有把 Tauri 带入共享后端。
- 命令树、schema version、JSON/JSONL 和退出码已评审。
- Production/Sandbox 数据根选择和 marker 规则已评审。
- 真实数据写入硬门禁、跨进程 scope 和锁内重验已评审。
- import/install/uninstall/reinstall/recovery/backup 均未绕过既有安全链路。
- T17 与 T13 的边界明确，批量命令没有伪实现捷径。
- Windows worker 仍是固定 `--once` 入口，Scheduled Task 普通 CI 禁令保持不变。
- 日志、诊断和机器输出都遵守统一脱敏白名单。
- 测试只使用 temp/fake/人工 fixture，并验证 sandbox 外 sentinel 不变。

CLI-0A、CLI-0B、CLI-1A/1B、CLI-2A/2B/2C 与 CORE-PREF-01 已形成 Slice A：共享
composition、逐阶段 observer、Sandbox write capability、完整单项 lifecycle E2E 和统一 prerequisite
decision 均已落地。Slice B/C 已完成 T13 sealed 批量 install/uninstall/reinstall 的 Sandbox CLI
契约；T13-06/T13-07 已接入窄 Tauri command、typed API 和前端工作流，T13-08 Gate C 已认证。
后续仍不得增加 shell 循环单项命令的旁路，也不得据此开放 Production 写入。
