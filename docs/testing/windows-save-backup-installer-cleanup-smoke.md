# Windows 存档后台任务安装器清理人工 Smoke（P7.2c）

本 Smoke 只在一次性 Windows 账户或 disposable Windows Sandbox 执行，用于验证 NSIS 与 WiX
安装器在真实安装/卸载生命周期中调用同一个无参数 cleanup helper。它不能在开发者日常账户运行，
也不能读取真实游戏、Steam userdata、玩家存档、真实 Mod 或真实备份。

## 安全边界

- 构建产物从只读映射目录提供；证据写入单独的可写映射目录。
- Sandbox 不预装 Visual C++ Redistributable；两个 Windows sidecar 必须静态链接 MSVC CRT，构建准备
  脚本必须在打包前拒绝仍包含动态 CRT 导入的产物。
- 只使用人工构造的 profile、save、backup 和 task fixture。
- 不执行 `schtasks`、`Stop-ScheduledTask` 或任何强制终止 worker 的操作。
- 不把任务名、SID、完整本地路径、XML、PowerShell、原始 stdout/stderr 或存档内容写入报告。
- 任一 helper 非零退出、状态无法确认、worker 仍在运行或卸载后 read-back 不一致时立即停止，
  保留现场并记录稳定 reason/exit code。

## 每种安装器的前置步骤

1. 启动全新的 VM/账户，确认没有 HMM 安装、HMM Scheduled Task 或旧证据。
2. 安装一个 NSIS 或 WiX 包，确认安装目录同时存在 GUI、`hmm-save-backup-worker.exe` 和
   `hmm-save-backup-installer-cleanup.exe`，且启动 helper 时不会出现缺少 `VCRUNTIME140.dll` 等
   平台运行库错误。
3. 启动 HMM，创建人工 profile 和最小 synthetic save fixture；启用后台保护并等待状态稳定。
4. 使用 GUI“完全退出应用程序”，确认 5 秒内 `hmm-tauri` 与其 `msedgewebview2` 子进程都已消失；
   App Log 依次出现 `application.exit_requested`、`application.exit_request_received`、
   `application.exit_started`、`application.event_loop_stopped`。若进程仍残留或缺少后两项，记录为
   退出生命周期失败并停止该 case，不使用卸载器关闭提示、CIM 或 `taskkill` 代替正常退出。
5. HMM 完全退出后，在任务计划程序中只查看该任务的状态，不复制任务名、SID 或 XML 到证据。
6. 每个 case 开始前重新启动 HMM 并确认当前安装器/版本和 evidence root。

## Case 矩阵

| Case | 准备状态 | 交互卸载 | 静默卸载 | 预期 |
| --- | --- | --- | --- | --- |
| missing | 没有 HMM task | 继续 | 继续 | helper exit `0`，卸载成功 |
| owned exact | HMM owned、Ready/Disabled | 继续 | 继续 | helper exit `0`，task 消失 |
| owned drift | owner 仍匹配、配置故意漂移 | 继续 | 继续 | helper exit `0`，task 消失 |
| foreign | task 存在但 owner 不匹配 | 继续 | 继续 | helper exit `0`，foreign task 保留 |
| owned running/queued | owned task 正在运行或排队 | 阻断 | 阻断 | 非零稳定 reason，task 与 worker 均保留 |
| permission/unknown | 权限、状态或删除 read-back 无法确认 | 阻断 | 阻断 | fail closed，不二次删除、不强杀 |
| upgrade | 执行升级路径 | 不适用 | 不适用 | 不调用 cleanup，原 task 保持 |
| repair/modify | 执行修复或修改路径 | 不适用 | 不适用 | 不调用 cleanup，原 task 保持 |

每个真正卸载 case 都要分别执行 interactive 和 silent 变体；NSIS、WiX 两种安装器各完整跑一遍。
`foreign` case 必须证明产品仍可卸载但第三方任务未被删除；`running/queued` case 必须证明 worker
没有被强杀。

WiX 使用 `Return="check"` 执行外部 cleanup helper。Windows Installer 会把 helper 的任意非零码统一
投影为 MSI `1722/1603`，因此 WiX 报告不得伪造原始 helper exit code；必须结合安装目录 sibling 数量、
task 聚合状态和重试结果确认是否命中 fail-closed。交互文案应提示关闭 HMM、等待后台备份完成并重试，
但不得暴露 task name、SID、路径或 helper 输出。

NSIS 计时和机器结果必须直接运行安装目录中的 uninstaller，并附带 Tauri 维护流程使用的
`_?=<安装目录>` 参数，例如 `uninstall.exe /S _?=C:\...\Helsincy Mod Manager`。只运行裸
`uninstall.exe` 会经过 self-extractor wrapper；wrapper 可能对外返回 `0`，即使内部 cleanup 已以
`20/21/22/23/64` 阻断。interactive 仍以 GUI 固定 reason 为准，silent 以 direct-uninstaller exit code
加安装目录/task read-back 为准；报告中不要记录完整路径。

## 记录格式

每个 case 记录以下脱敏字段：

- installer：`nsis` 或 `wix`
- mode：`interactive` 或 `silent`
- case：上表稳定标识
- result：`continued`、`blocked` 或 `skipped`
- helper exit code 与稳定 reason
- 卸载总耗时；超过 20 秒时单独记录为性能异常，但不得因此放宽 fail-closed 结果
- 卸载前后安装目录中 sibling 数量
- task 状态的聚合结论：`absent`、`owned-preserved`、`foreign-preserved` 或 `unknown`
- 时间戳、截图路径和脱敏日志路径

不要记录原始任务详情、用户目录、完整安装路径、PowerShell/XML 或存档内容。

## 2026-08-12 WiX `0.1.9` 阶段记录

| Case | Interactive | Silent | 聚合结论 |
| --- | --- | --- | --- |
| missing | exit `0`，19.3s | exit `0`，14.2s | 安装目录消失，owned task absent |
| owned exact | exit `0`，17.3s | exit `0`，15.2s | owned task 清除 |
| owned drift | exit `0`，17.3s | exit `0`，17.3s | marker 匹配时允许受控清除 |
| foreign | exit `0`，14.2s | exit `0`，12.2s | foreign task 保持 `Ready` |
| owned running | MSI exit `1603`，40.6s | MSI exit `1603`，11.2s | 安装目录、三个 sibling 与 running task 均保留 |
| running retry | exit `0`，16.2s | 不适用 | task 自然回到 `Ready` 后卸载成功，owned task absent |

该阶段证明 WiX 核心卸载矩阵的安全行为，但不是完整 runtime acceptance：upgrade/repair 仍待复验；
modify 因当前 MSI 明确设置 `ARPNOMODIFY`/`ARPNOREPAIR`，若系统维护 UI 只提供 Remove，应记录为
产品配置下不适用，不得伪造 PASS。

## 最终 `0.1.10` 候选包 build/static 记录

- NSIS：`13,578,498` bytes，SHA-256 `40E00C74BF7FDC44179538BA952E6BF36DC6E026D95CB53549E4C38BE59420A6`。
- MSI：`20,156,416` bytes，SHA-256 `696E000AF732519780EB38A028B4B07DA7A20E111DED363A4E2111D709D73131`。
- NSIS 生成脚本包含 GUI、worker、cleanup helper 三个 sibling，并在真正卸载时调用
  `NSIS_HOOK_PREUNINSTALL`；update mode 跳过 cleanup，所有非零 helper 结果阻断卸载。
- 最终 MSI 数据库的 `File` 表包含三个 sibling；`RunInstallerCleanup` 为同步检查结果的 EXE action，
  sequence `3499`，位于 `RemoveFiles` sequence `3500` 前，条件严格为
  `REMOVE="ALL" AND NOT UPGRADINGPRODUCTCODE`。
- 最终 MSI `Error` 表包含固定 `1722` 操作建议，不包含 task name、SID、路径、XML、PowerShell 或
  helper 原始输出。

上述仅完成 build/static gate。仍需在 disposable VM 复验 `0.1.10` NSIS 受影响路径、WiX
upgrade/repair 跳过 cleanup，以及新包 Settings 后台保护自动收敛。

`owned drift` 应只修改非 ownership 属性，并在卸载前确认 marker 仍匹配且状态为 `Ready/Disabled`。
如果仍返回 `ownership_unverified`，先确认 task、worker 和安装目录均被保留，再停止该 artifact 的后续矩阵；
不得通过重复卸载、手工删除 task 或扩大 timeout 把失败改记为通过。

## 完成条件

只有 NSIS 与 WiX 的所有适用 case 都符合预期，并且卸载后没有 HMM sibling、staging、backup 或
recovery 残留，才可将 P7.2c 标记为 runtime acceptance。任一 case 未执行、失败或证据不完整，
路线图必须继续保持“build/static gate 完成，runtime gate 待人工”。
