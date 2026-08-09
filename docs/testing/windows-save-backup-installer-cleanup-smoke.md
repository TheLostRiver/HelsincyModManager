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
4. 关闭 HMM 后，在任务计划程序中只查看该任务的状态，不复制任务名、SID 或 XML 到证据。
5. 每个 case 开始前重新启动 HMM 并确认当前安装器/版本和 evidence root。

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

`owned drift` 应只修改非 ownership 属性，并在卸载前确认 marker 仍匹配且状态为 `Ready/Disabled`。
如果仍返回 `ownership_unverified`，先确认 task、worker 和安装目录均被保留，再停止该 artifact 的后续矩阵；
不得通过重复卸载、手工删除 task 或扩大 timeout 把失败改记为通过。

## 完成条件

只有 NSIS 与 WiX 的所有适用 case 都符合预期，并且卸载后没有 HMM sibling、staging、backup 或
recovery 残留，才可将 P7.2c 标记为 runtime acceptance。任一 case 未执行、失败或证据不完整，
路线图必须继续保持“build/static gate 完成，runtime gate 待人工”。
