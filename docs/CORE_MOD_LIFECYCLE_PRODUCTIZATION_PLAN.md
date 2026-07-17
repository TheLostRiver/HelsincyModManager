# 核心 Mod 生命周期产品化加固实施计划

- 任务编号：T19
- 状态：`implemented`（A1-L3 七切片均已实现，待 L3 独立 review/合并后最终收尾）
- 规划日期：2026-07-17
- 前置：[核心 Mod 生命周期优先级计划](CORE_MOD_LIFECYCLE_PRIORITY_PLAN.md)中的 Gate A/B 均已 `certified`
- 实施边界：每个切片独立 PR、独立 review；未满足当前完成定义前不启动下一切片

## 1. 决策

Gate A 已证明普通 Mod 的安装、卸载和真正重装闭环，Gate B 已证明 ARMOR_RETARGET 的首次安装、
切换目标、重启恢复和卸载闭环。下一条主线不再扩展新的写入能力，而是先把这些已认证能力变成
可持续回归、可诊断且交互清晰的产品能力。

T19 固定拆为三条轨道：

- **Acceptance**：把现有核心生命周期场景收敛为正式、可发现、不能 `0 tests` 假绿的验收入口。
- **Logging / Diagnostics**：落地安全 App/Task 文件日志、审计降级可见性和用户可导出的诊断入口。
- **Feedback UI**：按交互语义拆分 Dialog、Detail Sheet、Task Notice、Toast 和 Inline state。

实施顺序固定为：

```text
A1 生命周期自动验收入口
  -> L1 安全 App Log
  -> U1 共享反馈基元 + 游戏目录 Dialog
  -> U2 安装计划 Sheet / 卸载 Modal / Task Notice / Toast
  -> L2 Task Log 与 Audit 降级可见性
  -> U3 跨 feature 通知迁移
  -> L3 日志与诊断页面
  -> T18 Mod 库分页
  -> T17 第三方 Mod 管理器批量迁移
  -> T13 批量安装/卸载
```

每个切片使用独立 PR 和独立 review。上一切片未满足完成定义时，不把下一切片顺手塞入同一 PR。
T19 是 Gate B 后优先级复审的结果，不是 Gate C，也不重新打开 Gate A/B 的认证状态。

当前进度：

- [x] A1 生命周期自动验收入口。
- [x] L1 安全 App Log：专用安全事件层、JSONL writer、UTC 日轮转、14 天保留、稳定健康退化码、
  reader/support diagnostics 兼容与敏感输入测试已落地。
- [x] U1 共享反馈基元与游戏目录 Dialog：单一 overlay host、稳定层级、共享 focus trap、live region、
  reduced motion 和首个语义明确的游戏目录决策 Dialog 已落地。
- [x] U2 核心 Mod 操作反馈：安装计划 Detail Sheet、危险卸载 Dialog、严格按 `taskId` 的安装/卸载
  Task Notice，以及 durable refresh 后的完成/普通失败 Toast 已落地；恢复风险继续持久阻断。
- [x] L2 已完成 Task Log/Audit 降级可见性；U3 已完成跨 feature 短时通知迁移；L3 已完成只读诊断页面、窄 snapshot command 与受控导出入口。

## 2. 事实基线

### 2.1 已可依赖

- `src-tauri/src/state_core_mod_lifecycle_tests.rs` 已有 6 个 `headless_composition_*` 场景。
- 这些场景已覆盖导入后重启重建计划、普通安装/卸载、V1 -> V2 真正重装、ARMOR 首次安装、
  ARMOR target switch/卸载，以及 manifest 保存失败后的回滚。
- 安装、卸载、重装和 retarget 已通过窄 Tauri command、后端事实查询、`taskId` 事件和受控 UI 使用。
- Audit JSONL writer/reader、App/Task Log 安全 reader 和 support diagnostics 导出已有基础。
- `ReinstallPlanPreviewPanel` 已具备 modal、focus trap、`aria-modal` 和窄 typed state，可作为共享 Dialog
  行为的事实参考。

### 2.2 当前缺口

- 直接使用宽泛 cargo filter 可能得到绿色 `0 tests`；没有正式脚本断言验收场景实际被发现和执行。
- 目前没有真实 App Log / Task Log 文件 writer，也没有已启用的 `tracing-subscriber` /
  `tracing-appender` 文件层。
- 若直接启用 subscriber，现有携带原始平台错误的 tracing 事件可能把本地路径或用户名写入文件。
- 多个高风险 runner 使用 best-effort Audit 写入并静默丢弃失败，用户和支持人员无法知道审计证据退化。
- `InstallPlanPreviewPanel` 同时承载计划、恢复阻断、卸载确认、任务进度、完成和失败，导致不同语义的
  信息与页面内容混在一起。
- 短时结果提示、长任务状态、破坏性确认和复杂计划详情尚无统一的跨 feature 展示规则。

## 3. 目标与非目标

### 3.1 目标

- 维护者可用一个稳定命令验证核心 Mod 生命周期，且场景缺失时立即失败。
- 安装、卸载、重装、retarget 和备份问题产生可关联 `task_id` 的脱敏诊断证据。
- 审计写入失败不再静默，同时不为“补一条日志”而反向制造新的玩家文件写入。
- 复杂计划、破坏性确认、运行中任务和短时结果使用符合语义的浮层或通知。
- 所有最终安装事实继续来自 manifest/recovery 查询，而不是 Toast、页面内存状态或日志。

### 3.2 非目标

- 不重写 InstallPlan、manifest、backup、rollback/recovery 或 ARMOR_RETARGET 引擎。
- 不用 UI 自动化替代 Rust 应用层/组合层验收。
- 不做远程遥测、崩溃自动上传或后台发送诊断包。
- 不把 Audit Log、Task Log 或 App Log 变成安装事实来源。
- 不在本任务中实现分页、批量迁移、批量安装/卸载、完整任务队列或新游戏适配。
- 不把所有消息统一成一种“悬浮卡片”。

## 4. 跨切片硬边界

### 4.1 安装与任务安全

- 真实写入继续遵循
  `analyze -> InstallPlan -> conflict/preflight -> backup -> commit -> manifest -> rollback/recover`。
- 卸载继续只消费 manifest/recovery 事实，不根据当前 Mod 包猜测目标。
- 同一 game/profile 的写入继续串行；长时间扫描、hash、解压和分析不在持有写锁时执行。
- 所有运行中状态必须按 `taskId` 关联；监听到其他任务事件时不得更新当前 UI。
- 自动化只使用人工 fixture、fake port 和临时目录，不读取真实游戏、Steam userdata、玩家存档或
  第三方 Mod 包。

### 4.2 日志与隐私

- 文件日志只接受稳定事件名、短内部 ID、聚合计数、耗时、稳定 error code 和已校验逻辑路径。
- 禁止记录完整本地路径、用户名、Steam ID、token、cookie、API key、真实存档内容、Mod 内容、
  manifest 正文、backup ref/root、sandbox/cache 路径或原始平台错误。
- 前端不直接写日志文件；Tauri command 也不接受日志路径或任意导出路径。
- 日志失败不得绕过 manifest/backup/rollback，不得把原本成功的文件状态仅为了日志而再次改写。

### 4.3 反馈层语义

| 信息类型 | 容器 | 行为 |
| --- | --- | --- |
| 需要决策或破坏性确认 | Modal Dialog | 捕获焦点、明确主次操作、阻断背景交互 |
| 安装计划、冲突和较长详情 | Floating Detail Sheet | 可滚动、保留上下文、展示后端返回的聚合/受控逻辑路径 |
| 运行中的长任务 | Task Notice | 按 `taskId` 常驻，完成/失败后转换或移交结果提示 |
| 完成、普通失败和短时消息 | Toast | 不改变页面布局，可关闭，必要时提供一个明确动作 |
| 字段校验、持续局部状态 | Inline | 靠近所属控件，不进入全局通知队列 |
| 数据安全风险或恢复阻断 | 持久告警/恢复入口 | 不自动消失，不得降级为普通 Toast |

共享反馈层只管理展示生命周期、层级、焦点和可访问性，不重新解释 install status、conflict、phase、
error code 或 retarget target。

## 5. 切片 A1：生命周期自动验收入口

### 5.1 交付

- 新增正式脚本 `scripts/verify-core-mod-lifecycle.ps1`。
- 脚本先列出 `hmm-tauri` 的 `headless_composition_*` 测试，断言发现数量不低于当前基线 6；数量为
  0、少于基线或缺少固定场景时直接失败。
- 通过同一公共前缀一次执行全部场景，并保留 cargo 的非零退出码。
- Windows 新 worktree 在测试前准备开发 sidecar；生成物保持 ignored/untracked。
- 将脚本接入 CI/release 验证入口，并在 `docs/TESTING.md` 记录命令、fixture 边界和预期场景数。

固定基线场景：

| 场景 | 证明内容 |
| --- | --- |
| `headless_composition_imports_v1_and_rebuilds_plan_after_restart` | 导入记录和重启后计划重建 |
| `headless_composition_installs_restarts_uninstalls_and_restores_baseline` | 普通安装、重启、卸载、baseline |
| `headless_composition_reinstalls_v1_to_v2_and_restores_baseline` | retained/replaced/added/stale 真正重装 |
| `headless_composition_retargets_staging_commits_and_persists_binding_snapshot` | ARMOR 首次安装与 binding snapshot |
| `headless_composition_switches_retarget_with_true_reinstall_and_uninstalls_to_baseline` | target switch、重启、卸载 |
| `headless_composition_rolls_back_v1_when_reinstall_manifest_save_fails` | manifest 失败与回滚 |

### 5.2 完成定义

- 本地和 CI 都能用同一条命令执行。
- 验收输出明确列出发现数和执行结果，不能以 `0 tests` 成功退出。
- fixture 与输出不包含真实本地私有路径、真实 Mod 或存档。
- 不修改产品运行时代码，不新建第二套安装实现。

## 6. 切片 L1：安全 App Log

### 6.1 交付

- 定义统一的安全结构化事件 envelope、字段白名单、稳定 error code 和 redaction/validation helper。
- 在启用文件 subscriber 前，清理现有会输出原始平台错误的 tracing 事件。
- 在 app data/state 下初始化按日 App Log writer、轮转和保留策略；不写游戏、Mod、存档或仓库目录。
- subscriber/file sink 只消费已校验字段；禁止任意模块用 `%error`、`Debug` 或拼接字符串绕过脱敏层。
- 为应用启动、配置/数据库初始化、游戏发现摘要、任务注册和普通稳定错误补最小事件。

### 6.2 完成定义

- 人工构造含 home path、用户名、Steam ID、token/cookie/API key 的错误，落盘内容不包含原文。
- 日志初始化失败不会导致未受控 panic，也不会改变玩家文件状态；应用以稳定 code 暴露诊断退化。
- App Log reader 和 support diagnostics 能读取新 writer 产生的受控日志。
- 默认没有远程传输。

## 7. 切片 U1：共享反馈基元与游戏目录 Dialog

### 7.1 交付

- 在 `src/shared/feedback/` 建立最小 `Dialog`、`DetailSheet`、`TaskNotice`、`ToastViewport` 容器契约。
- 提供单一 overlay host、稳定 z-index、焦点进入/返回、Escape/背景关闭策略、live region 和 reduced
  motion 行为。
- 复用已验证的 `ReinstallPlanPreviewPanel` focus trap 行为，避免在多个 feature 复制键盘逻辑。
- 首个迁移对象只选游戏目录决策界面，使其成为视觉和语义明确的 Dialog；现有目录选择、自动检测、
  重试和错误 view model 保持不变。

### 7.2 完成定义

- 键盘焦点不会逃逸到背景，关闭后回到触发控件；图标按钮有可访问名称。
- `960x640`、`1366x768`、`1440x900` 和窄屏下无文字/按钮重叠。
- 不新增宽泛 filesystem API；目录选择仍通过既有受控插件和后端校验。
- 不在本切片迁移 Mod 库、备份、恢复中心或所有历史弹窗。

## 8. 切片 U2：核心 Mod 操作反馈

### 8.1 交付

- 将安装计划和冲突详情从页面 inline panel 迁移到 `DetailSheet`。
- 将卸载确认迁移到危险操作 `Dialog`，保留托管文件数、备份恢复点和后端状态阻断。
- 安装/卸载运行态迁移到按 `taskId` 常驻的 `TaskNotice`。
- 安装/卸载完成和普通失败迁移到 Toast；安全风险、`rollback_required`、`repair_required` 和
  `unknown` 继续使用持久恢复告警，不自动消失。
- 重装和 retarget 继续消费既有 typed DTO、phase、preview token 和 manifest/recovery 刷新流程。

### 8.2 完成定义

- 关闭 Toast 或 Sheet 不会取消任务，也不会把页面内存态当作最终安装事实。
- 完成后仍重新查询 manifest/recovery；重启后动作可用性与后端事实一致。
- 卸载确认不可被背景点击误触；任务活跃时关闭策略明确且不会丢失任务关联。
- 覆盖 ready/blocked/running/completed/failed/cancelled/recovery-required 和窄屏视觉 smoke。

实现结果（2026-07-17）：U2 复用 U1 单一 feedback host；starting 尚无后端 identity 时不伪造任务通知，
只有 `running + taskId` 进入常驻 Task Notice。terminal event 先重查 manifest/recovery，verified false、
持久恢复状态或 completed 与最终状态矛盾时均抑制普通 Toast，并把无法确认的目标 Mod fail closed 为
持久 `unknown`。Toast 保持 feature-local 单条、手动关闭；队列、去重和 auto-dismiss 仍归 U3。

## 9. 切片 L2：Task Log 与 Audit 降级可见性

### 9.1 交付

- 每个长任务建立带 `task_id` 的 Task Log span/file，记录稳定 phase、进度、耗时、结果和 error code。
- 审计写入不再使用无说明的 `let _ = ...`；每个 runner 明确处理 audit unavailable/write failed。
- 若高风险动作尚未进入写入窗口，可按既有安全规则 fail closed；若文件与 manifest 已成功提交，不得
  仅为补审计而再次改写玩家文件，应报告稳定的 `audit_write_failed_after_commit` 诊断退化并保留
  可支持人员识别的健康摘要。
- support diagnostics 加入日志/审计健康状态和聚合计数，不返回原始错误或路径。

### 9.2 完成定义

- install、uninstall、reinstall、retarget、recovery 和 save backup runner 均有显式 audit failure 测试。
- Task Log 的 `task_id`、phase 和 task event 一致；并发任务不会串日志。
- 诊断包可区分业务失败、回滚失败和审计/日志证据退化。
- `RollbackFailed` / `DataSafetyRisk` 仍保持持久告警，不降级为普通完成 Toast。

## 10. 切片 U3：跨 feature 通知迁移

### 10.1 交付

- 逐 feature 迁移短时成功/失败消息，优先覆盖导入、游戏发现、Profile、备份和诊断导出。
- 保留字段级错误、页面加载错误、恢复中心决策面板和全局安全告警的 inline/持久语义。
- 通知去重按稳定 event key 或 `taskId`，不按展示文案比较。
- 建立队列上限、相同事件合并、自动关闭暂停、键盘关闭和一个可选动作的规则。

### 10.2 完成定义

- 页面布局不再因短时消息插入/移除而跳动。
- 多任务消息保持来源和 task 关联，不出现旧任务覆盖新任务。
- 每个迁移 feature 都有状态测试；不以一次全仓大重写完成迁移。

## 11. 切片 L3：日志与诊断页面

### 11.1 交付

- 提供只读日志/诊断页面，展示 App/Task/Audit 健康摘要、受控最近事件和诊断导出入口。
- Task Log 只按后端返回的安全 task summary 查询；前端不接受或拼接日志路径。
- 导出前展示包含的类别；导出结果只显示安全文件名、大小和聚合计数。
- 提供复制稳定 error code/task id 的操作，不复制原始本地错误或路径。

### 11.2 完成定义

- 页面在日志目录缺失、文件损坏、部分读取失败和审计退化时均有稳定状态。
- reader、DTO、typed API 和 UI 继续拒绝任意路径、原始日志正文和未校验事件。
- 诊断 zip 通过敏感片段扫描，且导出动作本身写最小 Audit Log；失败时不误报成功。

## 12. 验证矩阵

| 改动 | 最小验证 |
| --- | --- |
| A1 | 新验收脚本、场景发现数断言、6 个场景执行、CI 同命令 |
| L1 | redaction/allowlist 单测、writer/rotation/retention、support diagnostics、workspace Rust checks |
| U1 | typecheck/lint/build/test、focus/keyboard/a11y、四档 viewport smoke |
| U2 | Mod 库状态测试、taskId 过滤、manifest/recovery 刷新、install/uninstall/reinstall 聚焦 Rust 测试 |
| L2 | 每类 runner audit failure injection、Task Log 并发隔离、诊断包脱敏 |
| U3 | feature-local 状态测试、通知去重/队列/auto-dismiss、响应式 smoke |
| L3 | reader/DTO/command/typed API 测试、损坏输入、诊断 zip secret/path 扫描 |

每个切片最终都运行与风险相称的聚焦检查，并在准备合并前优先运行
`powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify.ps1`。任何未执行检查都必须在 PR
中如实记录，不能用历史结果代替。

## 13. T19 完成定义与后续队列

只有同时满足以下条件，T19 才能从 `planned` 更新为完成：

- A1-L3 七个切片均经独立 review 合并。
- 生命周期正式验收入口在 CI 中实际执行不少于 6 个场景。
- App/Task/Audit 诊断链路默认脱敏，Audit 降级不再静默。
- 核心 Mod 操作的计划、确认、运行和结果反馈按本文语义分层。
- 完整验证、前端视觉 smoke 和一次受控 Windows 桌面复验通过。
- `docs/TESTING.md`、`docs/LOGGING.md`、`docs/FRONTEND_BACKEND_CONTRACT.md` 与实际契约同步。

T19 完成后，下一顺序固定为 T18 -> T17 -> T13：先解决大库可操作性和查询边界，再恢复默认只导入
的第三方迁移，最后在单项操作、验收、日志和反馈均稳定后设计批量破坏性队列。P7.2c、T8、T12、
T14 等任务仍按各自发布门禁评审，不因 T19 规划完成自动开工。

## 14. 本规划 PR 的提交边界

本 PR 只新增本文并同步 `README.md`、`TODO.md`、`docs/ROADMAP.md`。不修改 Rust、TypeScript、CSS、
Tauri config、依赖、migration、脚本、CI、fixture 或生成物。第一个产品代码切片是 A1，必须另开任务和
独立 PR。
