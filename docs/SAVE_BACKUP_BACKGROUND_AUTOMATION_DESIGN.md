# 自动备份后台保障设计

本文档定义存档自动备份在主客户端关闭后的保障语义、后台执行架构、调度规则、UI 提示、安全边界和分阶段落地计划。它补充 [存档备份系统设计](SAVE_BACKUP_DESIGN.md)，不替代手动备份、manifest、历史记录、恢复流程或存档目录自动发现设计。

## 当前实施状态（P7.2b 应用级用户流程）

P7.1 的单次 worker 基础上，P7.2a 已实现 Windows 平台核心：

- 用户级 Scheduled Task adapter 支持受控 inspect、幂等 register/update、逐字段 read-back 和 ownership-checked unregister；固定任务每 15 分钟运行，并在用户登录后延迟 1 分钟触发。应用内通过当前进程 token 原生读取用户 SID，并使用 Task Scheduler COM 读取完整 task definition、状态和账户身份，再把账户名归一化为 SID；缺失任务、权限不足和任一字段/枚举/COM 异常仍分别映射为受控结果或 fail closed，整个只读链路不启动 PowerShell。
- task action 只能启动内部定位的 sibling worker，参数严格固定为 `--once`；前端和外部输入不能提供 task name、SID、命令、路径、参数、PowerShell 或 XML。
- `register()` 在写入后先由 PowerShell 返回完整 read-back，再由 Rust 复验 task spec 与 canonical non-link worker；只有该层确认 exact 后，才进入独立的内部首次启动阶段。首次启动对需要启动的 `Ready` task 在 PowerShell 内执行两次 exact-owned read-back，只通过 `Start-ScheduledTask -InputObject` 启动该对象，并在启动后再次复验 exact 与 `Ready/Running/Queued` 状态。若 task 已经 `Running/Queued` 则不重复启动。任一 read-back、worker identity、状态或启动命令失败都 fail closed，不返回 `Registered`，也不伪造 heartbeat。
- `worker_heartbeat_at` 已与 scheduler `last_checked_at` 分离。只有后台保护已启用、read-back 完全匹配且 heartbeat 位于 `[now - 45m, now]` 时才派生 `protected`。
- `get_save_backup_background_status` 只读执行注册检查和健康派生，不注册、不修复、不启动任务，也不获取 scheduler lease。
- worker sidecar 的 dev/release 准备脚本和 Windows `externalBin` 已接入；target-triple 源产物被 Git 忽略。
- 自动化测试仅使用 fake registry/command runner、固定 clock、临时 SQLite/目录和人工 fixture，不创建、更新、启动或删除真实 Scheduled Task。

P7.2b 已在上述平台核心上接入应用级用户流程：

- 全局 SQLite 设置持久化 `desired_enabled`、`enabled_at`、`last_worker_heartbeat_at` 和更新时间；worker 在禁用时立即 no-op，不枚举 Profile、不触发备份、不写 heartbeat。
- Settings 是唯一的全局启停入口；Profile 只读展示当前 profile 的备份节奏和全局后台保护状态，不提供第二个开关。
- Settings 只允许直接点击开关控件启停，说明行本身不触发状态变化；检查、启用和停用期间必须显示可见的旋转/不定进度、实时耗时与完成/失败耗时。当前应用会话保留最近一次控制状态，离开再返回 Settings 不自动执行平台检查；用户点击“重新检查”可以随时强制刷新。
- 启用成功先进入 `starting`。当前启用周期在 20 分钟内尚无有效 heartbeat 时保持“正在验证”；只有注册 read-back 完全匹配且 heartbeat 位于 `[now - 45m, now]` 时才显示 `protected`。启停 command 的确认若短暂失败，前端必须以一次权威状态重读判断是否已经收敛，已收敛时清除旧操作错误，不要求用户再次手动检查。
- 启用后只在当前仍挂载的 Settings 页面按约 0.75 秒、3 秒、1、5、10、16 分钟节点自动复查 `starting`；3 秒节点用于覆盖首次 Windows task inspect 尚未结束时 worker heartbeat 已经落盘的短暂陈旧窗口。临时读取失败不得覆盖最近一次成功读取的开关与 control status，只显示“本次检查未完成”的临时警告并继续使用剩余节点。只有当前会话从未取得过有效 control status 时才显示全局“状态不可用”。离开页面会取消这些复查，返回页面仍只展示会话缓存，不自动触发平台查询。
- 所有真正退出入口统一经过后端 exit guard。普通退出只在安全时继续；非保护状态显示原因明确的危险退出对话框，默认留在托盘，用户只能为当次显式 override，不能保存危险退出偏好。guard 每次都使用当前 Task Scheduler COM 精确读回和 fresh heartbeat，不缓存或延用平台保护事实；查询签发短时一次性授权，退出命令消费该授权以避免重复检查。授权缺失、过期、错配或已消费时仍回退到完整 guard并刷新危险确认，保持 fail-closed。普通对话框临时写入的“记住完全退出”偏好若被 guard 或 command 错误阻断，必须恢复原值。
- `starting` 时 override 不注销任务、不清除启用意图；Windows 仍会在约 1 分钟后按登录 trigger 尝试运行 worker。

P7.2a 安装态 runtime acceptance 已于 2026-08-07 在一次性 Windows Sandbox 完成：安装目录 sibling
worker、真实 user Scheduled Task 的 exact/幂等注册、Task Scheduler 人工 Run、fresh heartbeat、一次
synthetic automatic backup 与 ownership-checked 幂等 cleanup 均有证据。Terminal A 的 stdin
acknowledgement 未生效，最终 unregister leg 使用 dedicated cleanup smoke 完成并经 UI 确认无残留；
该偏差保留在 smoke 记录中。此 gate 证明安装态执行链，不替代长期 cadence/升级 soak，也不代表
P7.2c disposable VM runtime gate 已完成。所有后续工作继续复用
`SaveBackupTaskRunner -> SaveBackupService -> SaveBackupWriter/Repository/AuditLog`，不得建立第二套备份写入链路。

### P7.2c 卸载 cleanup 实施状态

P7.2c 已完成 [设计规格](superpowers/specs/2026-07-12-save-backup-installer-cleanup-design.md) 与
[实施计划](superpowers/plans/2026-07-12-save-backup-installer-cleanup-implementation.md)。helper、双
Windows sidecar、NSIS hook、WiX custom action 和 fake/static/build gate 已完成；disposable VM
runtime gate 已完成 WiX 核心卸载矩阵，最终候选包的尾部矩阵仍待人工执行。实现固定以下边界：

- 安装器调用独立、无参数的 installer cleanup helper；不调用 Settings `disable()`，也不扩展
  worker 固定 `--once` CLI。
- helper 直接复用 infra 的 current-user identity、固定 ownership marker、受控 unregister 和
  post-delete read-back；不读取 AppData、SQLite、Audit Log、Profile/save/backup/game 路径或网络。
- helper 在 Rust 中派生当前用户 task identity 后，只启动一个 cleanup PowerShell 进程；该进程内依次
  执行两次 ownership/state 复核、一次 owned unregister 和 post-delete missing read-back，避免为每个
  阶段重复导入 ScheduledTasks 模块造成超时抖动。
- 应用内 registry 在进程生命周期缓存已验证的当前用户 SID；普通 register/update 在一个 PowerShell
  进程内完成 ownership 检查、写入和 exact read-back。Rust 校验该 read-back 后，再用一个仅限 infra
  内部的受控 PowerShell 操作对 exact-owned task 执行两次 read-back 和一次首次启动；启动后最终 read-back
  仍由 Rust 复验。unregister 在一个进程内完成 ownership 检查、删除和 missing read-back。port 的成功返回
  已经代表注册与首次启动后置条件，app service 不再追加重复 inspect。
- missing、owned exact 和 owned drift 允许幂等清理；foreign task 必须保留并允许产品卸载继续。
- owned task running/queued，或 identity、ownership、state、delete/read-back 无法确认时，真正
  卸载 fail closed；不得强杀正在备份的 worker。
- 升级、repair、modify 跳过 cleanup。NSIS 使用生成模板证明过的 `NSIS_HOOK_PREUNINSTALL`；
  WiX 必须以生成模板证明 custom action 在 `RemoveFiles` 前、以发起卸载的交互用户身份运行。
- NSIS 与 WiX 的 static/build/runtime gate 分别记录；自动化只使用 fake runner 和静态检查，真实
  task 验收只在一次性 Windows 账户或 VM 执行。

在 disposable VM 矩阵完成前，不得把 P7.2c 标为 runtime acceptance 完成，也不得用该实现替代
P7.2a 安装态 worker/heartbeat runtime acceptance。

2026-08-09 的首轮 NSIS runtime 矩阵中，`missing` 和 `owned exact` 的 interactive/silent 变体通过；
`owned drift` 首次交互卸载在 owner marker 与 `Ready` 状态仍可确认时返回 `21/ownership_unverified`。
失败路径正确保留 task、worker 和安装目录，但暴露了四次独立 PowerShell/ScheduledTasks 启动造成的
延迟与 timeout 抖动。实现已收敛为上述两进程结构。

2026-08-12 的 WiX `0.1.9` 复验已覆盖 `missing`、`owned exact`、`owned drift`、`foreign` 和
`owned running` 的 interactive/silent 变体。前三类均完成清理，foreign task 均保留；running 变体均以
MSI `1603` fail closed，保留安装目录、三个 sibling EXE 和运行中 task，任务自然回到 `Ready` 后重试
卸载成功并清除 owned task。WiX 对外部 EXE custom action 的所有非零退出统一报告 MSI `1722/1603`，
无法稳定向 UI 透传 helper 原始 `20`；模板因此提供固定、非敏感且可操作的 `1722` 文案，提示关闭 HMM、
等待后台备份结束后重试。该文案不放宽 `Return="check"` 或回滚语义。

最终 HEAD 的 `0.1.10` NSIS/WiX debug 候选包已重建：NSIS 为 `13,578,498` bytes、SHA-256
`40E00C74BF7FDC44179538BA952E6BF36DC6E026D95CB53549E4C38BE59420A6`；MSI 为 `20,156,416`
bytes、SHA-256 `696E000AF732519780EB38A028B4B07DA7A20E111DED363A4E2111D709D73131`。最终 MSI 数据库确认
三个 sibling 均在 `File` 表，`RunInstallerCleanup` sequence `3499` 位于 `RemoveFiles` sequence `3500`
之前，条件为 `REMOVE="ALL" AND NOT UPGRADINGPRODUCTCODE`，固定 `1722` 文案已写入 `Error` 表。

该 `0.1.10` 已完成 WiX upgrade/repair、最终 owned 卸载、自动备份产物和 NSIS payload 等尾部矩阵，
但 NSIS 在任务被重新创建后暴露出新的首次运行缺口：任务为 exact `Ready`，UI 长时间停留在 `starting`，
因为注册流程没有保证本轮 worker 立即运行。上述两阶段 exact-owned 首次启动修复已进入新候选门禁；在新包
完成 NSIS 重新启用自动收敛、owned 卸载和 running fail-closed 复验前，不把 P7.2c 标记为 runtime acceptance。

首次运行修复的 `0.1.11` debug 候选已从提交 `7b9154d` 构建：NSIS 为 `13,575,580` bytes、SHA-256
`0EB248CBFCC5969621765CE58D287E7B4E9F3F90EE0B5C3590BA13E5092047FD`；MSI 为 `20,164,608`
bytes、SHA-256 `84DD4D66E6274331E0304334A477823D61DB0F69DBF92EF053765D494573E062`。NSIS 生成脚本确认三个
sibling、update mode 跳过与 pre-uninstall fail-closed hook；MSI 数据库确认 ProductVersion `0.1.11`、
三个 sibling、`RunInstallerCleanup` type `18` / sequence `3499`、`RemoveFiles` sequence `3500`、
`REMOVE="ALL" AND NOT UPGRADINGPRODUCTCODE` 条件和固定 `1722` 文案。该证据只关闭 build/static gate，
不替代 disposable Windows runtime gate。

## 背景

自动备份如果只依赖主窗口打开期间的前端定时器或 Tauri 主进程内 tick，就无法覆盖用户真正退出客户端后的时间段。对存档管理工具来说，“用户以为开启了自动备份，但退出客户端后实际没有备份”是不可接受的产品风险。

因此，正式自动备份必须把“关闭窗口”“退出主客户端”“系统未运行”区分清楚，并提供可解释、可验证、可审计的后台保障。

## 目标

- 用户关闭主窗口后，自动备份仍应继续生效。
- 用户真正退出主客户端后，自动备份仍应由本机后台守护或系统计划任务接管。
- 后台触发的备份必须复用现有 `SaveBackupService` 执行链路。
- 后台备份必须写入同一份 manifest、SQLite 备份历史、Audit Log 和保留策略结果。
- 主客户端再次打开时，能展示后台备份的最近结果、失败原因和是否仍受后台保障。
- 当后台保障未启用、失效或权限不足时，UI 必须明确提醒用户，不得让用户误以为自动备份仍受保护。

## 非目标

首个后台保障切片不实现：

- 云端备份或 Steam Cloud 同步。
- 跨设备调度。
- 系统级 Windows Service。
- 备份恢复写入。
- 游戏运行期间强制复制存档。
- 前端直接调度文件系统备份。

Windows Service 通常需要额外权限、安装/卸载治理和更复杂的升级策略，不作为第一阶段默认方案。首选用户级后台守护和用户级计划任务。

## 产品语义

### 关闭窗口

点击窗口关闭按钮默认只隐藏主窗口并保留托盘常驻。此时主客户端进程仍在，自动备份可以由客户端内调度器继续执行。

UI 必须提供清晰入口说明当前状态是“后台运行中”，而不是“程序已退出”。托盘菜单至少应包含：

- 打开 Helsincy。
- 暂停自动备份。
- 立即检查备份计划。
- 退出程序。

### 退出程序

用户选择“退出程序”表示主客户端进程结束。所有真正退出入口必须调用同一个后端退出命令；该命令先隐藏主窗口，再执行一次结构化 exit guard。普通退出只能在没有自动计划或全局状态为 `protected` 时继续；`starting`、未启用、注册失败、worker 不健康、权限不足、不支持或状态不可用时，窗口恢复并显示原因明确的危险退出提示：

Windows 安装态验收把“窗口立即隐藏”和“进程实际退出”分开判定：安全退出必须在 5 秒内结束 `hmm-tauri` 及其 `msedgewebview2` 子进程。App Log 的 `application.exit_guard_evaluated.duration_ms` 用于定位实时检查耗时，进程观测记录真实退出耗时；不能以窗口隐藏代替进程退出成功。

```text
退出主客户端后，自动备份将不再受后台保障。
```

提示应提供：

- 留在托盘，作为默认和初始焦点操作。
- 仍然退出，仅对当次生效。
- 取消退出。

危险退出提示不显示“记住选择”，也不能写入退出偏好。用户需要启用或修复后台保障时，应回到 Settings 的唯一全局开关；退出对话框不重复实现注册控制。

### 系统未运行

电脑关机、系统未登录、用户会话不存在或磁盘不可用时，程序不能执行备份。此时设计目标是“追赶”，不是虚假承诺。

下一次用户登录或后台守护恢复时，应检查错过的计划：

- 若存在 overdue 自动备份，且源/目标仍有效，则排队执行一次追赶备份。
- 若游戏正在运行，则记录 pending，等待游戏退出或下次调度窗口。
- 若源目录失效、目标不可写或设置不完整，则写入失败历史和轻量告警。

## 推荐架构

```text
主客户端 Tauri App
  UI、托盘、设置、历史展示、手动备份入口
  可运行客户端内调度器

后台备份守护进程
  无 UI 或极小托盘状态
  按用户会话启动
  读取同一份本地配置和数据库
  复用应用层备份服务

系统计划入口
  Windows: 用户级 Scheduled Task
  Linux: systemd user timer 或 desktop autostart
  Steam Deck: 后续实验性适配

共享后端能力
  ProfileSaveSettingsRepository
  SaveBackupService
  SaveBackupRepository
  SaveBackupWriter
  AuditLogWriter
  Game running detector
```

关键原则：

- 不能有两套备份实现。
- 不能让前端定时器成为自动备份事实来源。
- 不能让后台守护接收前端传入的真实路径。
- 不能绕过备份执行前的源目录、目标目录、包含关系、symlink/junction、大小上限和保留策略校验。

## 进程模型

### 主客户端

主客户端负责：

- 展示自动备份设置。
- 展示后台保障状态。
- 提供启用/停用后台保障入口。
- 触发手动备份。
- 展示后台备份历史和失败提醒。
- 在托盘常驻时执行轻量调度。

主客户端不负责在进程退出后继续备份。

### 后台守护

后台守护负责：

- 在用户登录后启动。
- 周期性检查已启用自动备份的 profile。
- 对 overdue 或 due 的计划创建备份任务。
- 在游戏运行时延后，而不是强行备份。
- 将结果写入同一份 SQLite 历史和 Audit Log。
- 记录自身健康状态，供主客户端下次读取。

后台守护可以是同一 Tauri/Rust workspace 打包出的单独二进制，也可以是主二进制的 headless 子命令。首选单独二进制或明确的 headless mode，避免无 UI 运行时仍初始化完整 WebView。

### 系统计划任务

Windows P7.2a 已实现用户级 Scheduled Task 平台核心：

- 用户登录后延迟 1 分钟执行一次。
- 每隔 15 分钟唤醒一次 worker 执行单次检查。
- 不要求管理员权限。
- 注册、更新、检查和移除由受控后端流程维护；安装器自动 cleanup 尚未接入。

计划任务只负责拉起守护，不负责备份逻辑。

## 数据模型补充

现有 `ProfileSaveSettings.schedule` 表达备份节奏。后台保障需要额外持久化调度状态，避免只靠内存判断。

建议新增受控表或等价 repository：

```text
save_backup_scheduler_state
  game_id TEXT NOT NULL
  profile_id TEXT NOT NULL
  enabled INTEGER NOT NULL
  background_protection_enabled INTEGER NOT NULL
  last_checked_at INTEGER
  last_attempt_at INTEGER
  last_success_at INTEGER
  next_due_at INTEGER
  pending_reason TEXT
  last_error_code TEXT
  worker_instance_id TEXT
  lease_owner TEXT
  lease_expires_at INTEGER
  updated_at INTEGER NOT NULL
  PRIMARY KEY (game_id, profile_id)
```

字段约束：

- 不保存完整本地路径。
- 不保存 Steam ID。
- 不保存存档内容或 manifest 正文。
- `pending_reason` 和 `last_error_code` 使用稳定短码。
- `worker_instance_id` 是本机内部短 id，只用于诊断同一时间是否有多个 worker 竞争。
- `lease_owner` 和 `lease_expires_at` 是持久化调度租约字段，用于主客户端调度器和后台守护之间去重；重启后过期租约可被接管，未过期租约必须等待或由健康检查判定失效后释放。

## 调度规则

### due 判断

调度器只基于后端持久化设置判断：

- profile 是否存在。
- profile 是否启用自动备份。
- schedule 是否为 daily / weekly 等自动节奏。
- `next_due_at` 是否已到。
- 上次成功时间是否已满足间隔。

前端展示用 view model 可以格式化这些信息，但不能计算最终 due 事实。

### 追赶策略

如果错过多个计划窗口，不应一次性补多个备份。默认只执行一次追赶备份，然后根据当前 schedule 计算下一次 `next_due_at`。

原因：

- 避免刚开机时连续写多个重复备份。
- 避免保留策略被短时间大量触发。
- 存档文件只有当前状态，多次补跑通常没有价值。

### 游戏运行冲突

自动备份默认不在游戏运行时执行，避免复制游戏正在写入的存档。

调度结果：

- 游戏未运行：正常执行。
- 游戏运行中：设置 `pending_reason = game_running`，延后。
- 游戏退出后：执行一次 pending 备份。
- 无法判断游戏是否运行：保守延后，并记录 `game_running_unknown`。

手动备份可以在 UI 中要求用户确认风险；自动备份不应在无人值守场景下要求确认。

### 并发与锁

- 同一 `gameId/profileId` 的备份任务必须串行。
- 主客户端调度器和后台守护可能同时存在，必须通过数据库锁、任务锁或持久化调度租约避免重复执行。
- 获得调度租约不等于获得文件写入许可；备份执行前仍由 `SaveBackupService` 重新校验所有目录与安全上限。
- 长时间扫描、压缩、hash 不应持有不必要的全局锁。

租约协议使用 `save_backup_scheduler_state` 中的持久化字段：

```text
lease_owner
lease_expires_at
```

租约过期后其他 worker 可以接管；接管前必须重新读取状态并再次判断 due。守护或主客户端重启时不得把本地内存状态视为有效租约，只能以数据库中的 `lease_owner`、`lease_expires_at` 和 worker 健康状态为准。

## 后台保障状态

主客户端应展示一个稳定状态：

```text
protected
tray_only
not_enabled
starting
registration_failed
worker_unhealthy
permission_required
unsupported_platform
```

含义：

- `protected`：后台保护已启用、Scheduled Task read-back 完全匹配，且 worker heartbeat 位于 `[now - 45m, now]`。
- `tray_only`：主客户端托盘常驻可执行自动备份，但真正退出后不受保护。
- `not_enabled`：用户未启用后台保障。
- `starting`：任务已完成注册 read-back，但当前启用周期仍在 20 分钟启动宽限内，尚无有效 worker heartbeat；该窗口覆盖首次 15 分钟周期和调度抖动，且不能提前声称已保护。
- `registration_failed`：计划任务或自启动注册失败。
- `worker_unhealthy`：已注册但最近没有心跳或连续失败。
- `permission_required`：当前环境需要额外权限或系统设置。
- `unsupported_platform`：当前平台尚未实现后台保障。

状态进入 `registration_failed`、`worker_unhealthy`、`permission_required` 或 `unsupported_platform` 时，Profile 页面和设置页必须有清晰提示。主界面打开时可以出现居中偏上的悬浮 UI，但不能只依赖悬浮提示；页面内状态也必须可见。

## 用户界面

### 设置页

设置页是后台保障的唯一全局控制入口，应提供：

- 后台保障全局开关。
- 当前保障状态。
- 当前启用时间。
- 最近 worker heartbeat 时间。
- 最近失败原因。
- “重新检查”按钮。
- 检查、启用和停用过程的动态反馈与实时耗时，以及完成后的本次耗时。

启停请求返回错误时，UI 不能直接把一次 transport/确认失败等同于最终系统状态。前端应立即强制读取一次
权威 control status：若启用已收敛为 `starting`/`protected`，或停用已收敛为 `not_enabled`，则清除旧操作错误并
告知用户状态已自动重新同步；只有未收敛时才保留稳定错误提示。`starting` 仍不代表已保护，前端不得提前显示
`protected`。

启用后，当前 Settings 页面在约 0.75 秒和 3 秒执行两次短周期非阻塞权威复查，再在约 1、5、10、16 分钟节点
有限复查，使注册状态快速收敛，并在首次 heartbeat 到达后无需用户再次点击。后端 `control_status()` 必须在实时
Scheduled Task inspect 之后重新读取全局设置；这样 heartbeat 或停用意图若在 inspect 期间提交，同一次响应
就能看到新事实，而不会返回开始检查前的陈旧快照。临时读取失败不能取消剩余节点；其他操作进行中时延后当前节点而
不消耗它。自动复查必须在页面卸载时取消，重新进入 Settings 不自动查询；这样既避免页面切换触发十几秒平台检查，
也避免长期轮询 Scheduled Task。

系统启用“减少动画”时，普通装饰动画应继续降级；但检查、启用和停用中的状态图标与不定进度不能完全静止，
否则会被误认为界面卡住。此类必要反馈改用更慢的阶梯动画，并继续配合实时耗时文字表达进度。

当用户启用自动备份但未启用后台保障时，应显示明确提示：

```text
当前仅在客户端运行时自动备份。启用后台保障后，退出主客户端也会继续检查备份计划。
```

### Profile 页

Profile 页只读展示与当前 profile 相关的：

- schedule 摘要。
- 下一次计划时间。
- 是否有 pending 备份。
- 最近一次自动备份结果。
- 后台保障是否覆盖该 profile。

Profile 页不得启用、停用或重试全局注册；失败状态只导航到 Settings。

### 悬浮 UI

悬浮 UI 用于短时提醒：

- 后台保障启用成功。
- 后台守护不可用。
- 自动备份失败。
- 启动后发现错过备份并已追赶。

位置延续当前项目约定：界面正中上方一点。正常信息几秒后自动消失；高风险失败可以保留页面内状态，而不是让悬浮 UI 长期停留。

## 错误码

建议新增或复用稳定错误码：

```text
save_backup_scheduler_unavailable
save_backup_background_not_enabled
save_backup_background_registration_failed
save_backup_background_worker_unhealthy
save_backup_background_permission_required
save_backup_background_unsupported_platform
save_backup_auto_skipped_game_running
save_backup_auto_skipped_game_running_unknown
save_backup_auto_source_invalid
save_backup_auto_destination_unavailable
save_backup_auto_task_conflict
```

错误 message 不包含完整本地路径、Steam ID、Windows 用户名、存档内容、manifest 正文或备份根目录。

## 审计与日志

以下事件应进入 Audit Log：

- 启用或停用后台保障。
- 注册或移除用户级计划任务。
- 自动备份开始。
- 自动备份成功。
- 自动备份失败。
- 自动备份因游戏运行延后。
- 后台守护健康状态变更。

允许字段：

- `game_id`
- `profile_id`
- `task_id`
- `backup_id`
- `trigger = auto`
- `result`
- `error_code`
- `file_count`
- `archive_size_bytes`
- `scheduler_state`

`scheduler_state` 不能序列化完整调度状态对象，只能是显式白名单内的小型标量摘要：

```text
status
pending_reason
last_error_code
background_protection_enabled
worker_health
lease_age_seconds
```

这些字段只能使用布尔值、短枚举、短错误码或整数秒数，不得包含路径、账号标识、manifest、hash 列表、原始错误文本或任意嵌套对象。

禁止字段：

- 完整存档目录。
- 完整备份目录。
- Steam ID。
- Windows 用户名。
- 存档内容。
- manifest 正文。
- hash 列表。

## 安全边界

后台守护和主客户端都必须遵守同一安全边界：

- 不从前端接收备份源路径或目标路径。
- 不直接读取任意文件系统路径。
- 不绕过 profile save settings repository。
- 不绕过 game adapter 的存档目录规则。
- 不在游戏运行中无人值守备份。
- 不把备份写入游戏目录内部。
- 不跟随 symlink/junction。
- 不记录敏感路径或账号信息。
- 不把失败的保留策略清理反向判定为本次备份失败，但必须记录 warning/audit。

## 平台策略

### Windows

MVP 平台。推荐：

- 主客户端支持托盘常驻。
- 后台守护通过用户级 Scheduled Task 注册。
- 计划任务在用户登录时启动，必要时按固定间隔唤醒。
- 不默认要求管理员权限。

### Linux / Steam Deck

后续实验性支持。候选方案：

- systemd user service + timer。
- XDG autostart。
- Steam Deck Desktop Mode 下先提供托盘/客户端内调度，再逐步接入 user service。

跨平台实现必须通过 port/trait 抽象注册、卸载、健康检查和状态查询，不把 Windows 计划任务细节泄漏到应用层。

## 测试要求

实现后台保障时至少覆盖：

- 后台保障开启/关闭会更新调度状态。
- 注册失败返回稳定错误码。
- due profile 会触发一次 `SaveBackupTrigger::Auto`。
- 错过多个窗口只追赶一次。
- 游戏运行中自动备份被延后。
- 游戏退出后 pending 备份被执行。
- 主客户端调度器和后台守护并发时不会重复备份同一 profile。
- 后台触发仍会执行源目录、目标目录、包含关系、symlink/junction、大小上限校验。
- 自动备份历史 DTO 不返回完整路径、Steam ID、manifest 正文或 hash 列表。
- Audit Log 不包含完整本地路径、Steam ID、用户名或存档内容。

测试必须使用临时目录、fake game running detector、fake scheduler registry 和 fake clock。不得依赖真实 MHW:I 安装目录、真实 Steam userdata、真实玩家存档或真实系统计划任务。

## 分阶段落地

### 切片 1：设计与契约

- 新增本文档。
- 在存档备份总设计中明确自动备份后台保障要求。
- 后续实现前同步契约、测试和日志文档。

### 切片 2：客户端内自动备份

- 复用手动备份服务。
- 接入 schedule due 判断。
- 主客户端运行期间执行自动备份。
- 关闭窗口进入托盘仍可执行。
- UI 明确显示“仅客户端运行时受保护”。

### 切片 3：P7.1 后台 worker 基础能力

- 调度状态、租约去重和 worker 健康内核按 [后台自动备份调度内核实现计划](SAVE_BACKUP_BACKGROUND_SCHEDULER_CORE_PLAN.md) 先行落地。
- 已新增 SQLite scheduler state repository、持久化 lease 和 worker heartbeat。
- 已新增只接受 `--once` 的无 WebView headless worker binary，并复用既有备份链路。
- 已新增 stable registry contract 与 `UnsupportedPlatform` fallback，以及 fake/临时依赖测试。
- 当前状态固定为 `tray_only`；该 binary 是平台注册前基础能力，不代表退出后已自动运行。

### 切片 4a：P7.2a Windows 平台注册与健康核心

- 已实现 Windows 用户级 Scheduled Task inspect/register/update/unregister 和 ownership/read-back 保护。
- 已实现独立 worker heartbeat、45 分钟 TTL 和 exact registration + fresh heartbeat 的 `protected` 派生。
- 已实现 Windows worker sidecar 准备与打包配置；2026-08-07 安装态人工 smoke 已完成并记录为
  SAVE-02 `certified`。

### 切片 4b：P7.2b 主客户端状态与提示

- 已实现全局 SQLite 用户意图、启用时间和独立 worker heartbeat。
- 已实现 Settings 唯一全局启停入口，以及 Profile 只读状态展示。
- 已实现 20 分钟 `starting`、45 分钟 `protected` TTL 和 fail-closed 状态派生；启动宽限覆盖首次 15 分钟计划任务周期及调度抖动。
- 已实现启停结果的权威重读收敛、动态进度、实时/完成耗时，以及仅限当前 Settings 页面生命周期的有限自动复查；短周期节点覆盖首次 inspect 与 heartbeat 的竞争窗口，页面返回不会自动执行平台查询。
- 已实现统一 exit guard、结构化危险原因、短时一次性授权和当次 override；危险退出不保存偏好。退出命令先隐藏主窗口，安全检查失败时恢复窗口并显示确认，避免用户在等待后台检查时误以为退出未生效。授权存储或 command 失败同样必须恢复窗口并 fail closed；只有明确的 `exiting` 结果才保持隐藏。
- 已完成应用级自动化与响应式 UI 检查；安装态 runtime acceptance 已完成，P7.2c installer cleanup
  仍未完成。

### 切片 5：跨平台扩展

- 抽象 scheduler registry port。
- Linux / Steam Deck 接入 user service 或 autostart。
- 增加平台差异文档和手动 smoke test 记录。

## 验收标准

正式宣称“Windows 安装态退出后自动备份可用”前必须满足：

- 主窗口关闭后仍会按计划备份。
- 主客户端退出后，后台保障启用时仍会按计划备份。
- 后台保障不可用时，UI 明确提示自动备份不会在退出后继续。
- 后台自动备份结果能在主客户端重新打开后看到。
- 所有自动备份结果都有 manifest、历史记录和 Audit Log。
- 所有高风险路径都有临时目录或 fake 依赖测试覆盖。
- 文档、契约、测试说明和日志说明同步更新。

P7.2b 已满足应用级启停、状态 UI、退出保护和自动化门禁；SAVE-02 又完成了真实安装态 Scheduled Task
的人工触发、fresh heartbeat、实际 synthetic backup 与 cleanup。两者共同证明安装态后台执行链，
但 SAVE-02 没有做长时间 cadence/升级 soak，P7.2c installer cleanup 也仍是独立发布 gate。
