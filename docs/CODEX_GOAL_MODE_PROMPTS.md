# Codex 目标模式提示词

本文档提供 Helsincy Mod Manager 在维护者暂时离开时使用的目标模式提示词。任务来源和顺序以
[自主迭代路线图](AUTONOMOUS_ITERATION_ROADMAP.md) 为准，项目事实以
[项目任务状态快照](PROJECT_TASK_STATUS.md) 和当前源码为准。

本轮只处理 Windows + MHW:I。Linux / Steam Deck 不进入任务选择、实现、验收或发布判断。

## 使用顺序

1. 首次启动目标模式时使用“主提示词”。
2. 上下文中断或目标模式续跑时使用“继续提示词”。
3. PR 出现评论、CI 变化或 CodeRabbit 缺席时，使用对应专项提示词。
4. 只有“合并提示词”的全部门禁满足后，才允许执行合并。

提示词中的“继续”不扩大权限。它只允许推进路线图内已经定义的任务。

## 主提示词

```text
目标：按照 docs/AUTONOMOUS_ITERATION_ROADMAP.md 自主推进 Helsincy Mod Manager 的
Windows + MHW:I 任务队列，直到队列耗尽、遇到硬停止条件，或需要维护者作出不可替代的产品/安全决策。

开始前：
1. 读取 AGENTS.md、README.md、docs/ARCHITECTURE.md、docs/ROADMAP.md、
   CONTRIBUTING.md、docs/TESTING.md、docs/GOVERNANCE.md、SECURITY.md。
2. 使用 planning-with-files，并读取项目路由、guardrails、相关边界 skill 和 review gate。
3. 读取 docs/PROJECT_TASK_STATUS.md、docs/AUTONOMOUS_ITERATION_ROADMAP.md、
   docs/HMM_CLI_AUTOMATION_DESIGN.md 以及当前任务指定的专题文档。
4. 检查 git status、当前分支、open PR 和当前 CI；保留所有用户或其他任务的未提交改动。
5. 本轮明确排除 Linux / Steam Deck，不创建相关实现、适配、打包或验收任务。

任务选择：
1. 从自主路线图选择第一个状态为 ready 且前置已满足的任务。
2. QG-01 是当前第一个 ready task；它合并后再从最新 main 启动 T13-00，不把未合并治理分支
   作为产品 task 的隐式基线。
3. 不重新实现已标记 completed/certified 的能力；先根据源码和测试确认真实缺口。
4. 一个 task 使用一个独立 hy/ 分支、独立 worktree 和独立 PR。
5. 大 task 按路线图切片；每完成一个可独立验证的步骤就立即提交 Git。
6. 不把多个 task 塞进同一 PR，不顺手做无关重构。

实现边界：
1. 游戏目录写入必须复用 InstallPlan -> preflight -> backup -> commit -> manifest ->
   rollback/recovery。
2. 批量安装、卸载和重装不能在 CLI 或前端循环调用单项命令来冒充；必须消费服务端 sealed batch plan。
3. 同一 game/profile 写入严格串行；scan/hash/extract/analyze 留在写锁外。
4. 原始 Mod 输入只读，派生产物只进入 staging/sandbox。
5. 前端只负责展示、交互和 typed API；不计算路径、重定向目标、备份或回滚规则。
6. Production CLI 写命令在跨进程 admission 完成前保持不可达。
7. 所有测试只使用 temp/fake/人工 fixture，不使用真实游戏、Mod、Steam userdata 或玩家存档。
8. 正式代码、文档、提交、PR 和评论不得出现未授权外部项目的名称、路径、来源说明或复制代码。

每个 task 的完成循环：
1. 写清任务目标、允许修改范围、禁止修改范围、风险和验收命令。
2. 先补或更新测试，再做最小实现；高风险写入链路必须有失败、取消、回滚/恢复和脱敏测试。
3. 每个可独立验证步骤单独 commit；提交信息保持单一职责。
4. 运行路线图指定的聚焦验证。
5. 运行 powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify.ps1。
6. 只要当前 CI 尚未覆盖前端测试和 clippy，还必须显式运行：
   cmd /c corepack pnpm run test
   cargo clippy --workspace --all-targets -- -D warnings
7. 执行 hmm-review-gate 的 findings-first 本地自审，检查完整 task diff、边界、测试、
   文档同步、禁入产物、secret、私有路径和残余风险。
8. 有真实 finding 就修复、补测试、提交并从第 4 步重新执行；误报必须记录源码/测试/契约证据。
9. 推送分支并创建 PR，PR 正文列出范围、提交、已执行验证、未执行验证、风险和 Windows-only 影响。
10. 等待 CI 到 terminal 状态；pending/running/queued 时只等待，不合并。
11. 获取所有 review、inline thread 和评论，逐条分析。CodeRabbit 没有 review 不等于没有问题，
    必须完成独立全 diff 自审。
12. 评论若为真实 bug：修复、补测试、commit、push，重新等待全部 CI 并重新 review。
13. 评论若为误报：必须在 PR 中写出可复核证据后才能 resolve，不能凭“我觉得没问题”忽略。
14. 只有满足 docs/CODEX_GOAL_MODE_PROMPTS.md 的合并门禁时才合并。
15. 合并后更新任务状态和正式文档，从最新 main 创建下一个独立 task。

硬停止：
1. 任何 required check 失败、取消、超时、被跳过或仍未完成。
2. 存在未处理的 Critical/Important finding 或 unresolved review thread。
3. 需要真实玩家数据、真实第三方 Mod、日常 Windows 账户中的 Scheduled Task 或未经授权的外部状态。
4. 需要改变批量原子性、删除/覆盖安全策略、存档恢复确认策略或 Production CLI 写入门禁，
   但路线图/设计没有明确决定。
5. 发现当前任务依赖未合并的高风险行为变化，或工作区不能与用户改动安全隔离。

不要因为时间、上下文长度或 CI 等待而降低门禁。目标模式应持续等待、修复和复审，而不是提前宣布完成。
```

## 继续提示词

```text
继续当前 Helsincy Mod Manager 目标。

先恢复而不是重新开始：
1. 读取当前 planning-with-files 的 task_plan.md、findings.md、progress.md。
2. 检查当前分支/worktree、git status、最近 commit、对应 PR、最新 CI、review threads 和评论。
3. 对照 docs/AUTONOMOUS_ITERATION_ROADMAP.md 确认当前 task、依赖和完成定义。
4. 汇报当前处于实现、验证、review 修复、CI 等待还是合并门禁阶段。
5. 从尚未完成的最早步骤继续；不要重复已经成功且仍然适用于当前 commit 的操作。

若最后一次验证后代码、文档或依赖发生变化，按影响范围重新运行验证。
若最后一次 push 后出现新评论或 CI 失败，先处理它们，不开始下一 task。
所有 Git、CI、review、误报证据和合并门禁继续遵守 docs/CODEX_GOAL_MODE_PROMPTS.md。
```

## PR 评论处理提示词

```text
处理当前 PR 的全部 review feedback。

1. 获取 PR review、inline thread、普通评论和当前 resolution 状态，不只读取最新评论。
2. 每条反馈分类为：真实 bug、测试缺口、契约/文档缺口、可维护性问题、误报、需维护者决策。
3. 按 Critical、Important、Moderate、Minor 排序，先处理会影响玩家数据、安全链路和公共契约的问题。
4. 不要因为评论来自机器人就自动采纳，也不要因为自己初看没问题就忽略。
5. 真实 bug：说明根因，做最小修复，补能在修复前失败的测试，commit、push。
6. 误报：引用具体源码、测试、契约或运行输出，说明判断为何不成立；把证据回复到 PR 后再 resolve。
7. 需维护者决策：停止该 PR 的合并，不替用户选择会扩大行为或安全范围的方案。
8. 每次 push 后重新等待全部 CI，并复审新增 diff；安全链路、公共契约或治理规则变化时重新做完整自审。
9. 最终输出 findings-first 状态，不隐藏未解决项。
```

## CodeRabbit 缺席提示词

```text
当前 PR 没有 CodeRabbit review。不要把缺席视为批准，也不要猜测是额度、配置还是队列原因。

1. 检查是否存在任何 CodeRabbit 状态、评论或失败记录，但不因缺席无限阻塞。
2. 使用 hmm-review-gate 对完整 PR diff 做一次独立 findings-first 自审。
3. 对安全、安装/卸载/重装、批量、存档、日志、Tauri contract、并发和游戏 adapter 分别检查边界。
4. 检查测试是否覆盖成功、失败、取消、回滚/恢复、并发、脱敏和负向 containment。
5. 运行当前任务的聚焦验证、完整 verify、前端测试和 clippy。
6. 在 PR 留下“外部机器人 review 缺席，已完成独立自审”的证据摘要，包括 commit SHA 和实际命令。
7. 仍有任何已确认的真实 bug、测试/契约缺口、Critical/Important finding、未解决线程或
   不确定的高风险行为时禁止合并。
```

## CI 等待与失败处理提示词

```text
等待当前 PR 的 CI 到 terminal 状态。

1. pending、queued、in_progress 时继续等待，不创建“应该会通过”的结论。
2. required check 的 success 才算通过；failure、cancelled、timed_out、action_required、
   skipped 或 neutral 都不能当作成功。
3. CI 失败时读取失败 job 和日志，区分当前 diff 引入、既有主干问题、环境波动和基础设施故障。
4. 当前 diff 引入：修复并补测试，commit、push，重新等待所有 CI。
5. 既有问题或环境问题：必须给出可复核证据；若 required check 仍未成功，保持不合并。
6. 不重跑来掩盖确定性失败；只有证据表明是瞬时基础设施故障时才允许重跑。
7. CI 成功后仍要检查 review threads、自审 findings、分支是否落后和本地额外门禁。
```

## 合并提示词

```text
评估并合并当前 PR。必须逐项证明：

1. PR 范围只包含当前 task，提交按独立步骤拆分，工作区无禁入或无关产物。
2. 分支已基于最新目标分支，或 GitHub 明确判定可合并且没有未处理的基线变化。
3. 所有 required checks 已到 terminal success；没有 pending、failure、cancelled、timed_out、
   action_required、skipped 或 neutral required check。
4. 当前 commit 已执行任务聚焦测试、完整 verify，以及尚未纳入 CI 的前端测试和 clippy。
5. 已完成最后一次 push 后的完整本地自审；所有已确认的真实 bug、测试或契约缺口均已处理，
   Critical/Important finding 为零。
6. 所有 review thread 和评论已处理；真实 bug 已修复并补测试，误报已有可复核证据。
7. CodeRabbit 缺席时已经执行并记录独立全 diff 自审，不能用“无评论”代替 review。
8. 没有需要维护者决策的产品、安全、许可或数据来源问题。
9. 治理、安全门禁、CI、workflow、policy、AGENTS 或核心安全文档变更已明确标注影响，并在
   CodeRabbit 缺席或限额时完成更严格的独立增量自审；没有把机器人 success 或“无评论”当作批准。
10. 高风险 Windows 安装态验收若是该 task 的完成定义，必须在 disposable VM/一次性账户完成并 cleanup。

合并动作：
1. 先尝试普通合并。
2. 只有上述 10 项全部满足、当前目标已有明确的 admin 合并授权、普通合并仅被已知的分支保护批准
   规则阻挡时，才允许使用 gh pr merge --admin。
3. --admin 绝不用于绕过未完成/失败 CI、过期分支、未解决评论、缺失测试、真实 bug或人工决策。
4. 合并成功后确认目标分支包含 PR commit，更新路线图状态，再开始下一个 task。
```

## 单 Task 提示词模板

```text
执行任务：<task id 和标题>

来源：docs/AUTONOMOUS_ITERATION_ROADMAP.md
前置：<依赖 task / gate>
允许修改：<目录和文件>
禁止修改：<目录和文件>
风险：<low/medium/high>
完成定义：<可验证行为>
聚焦验证：<命令>
人工/环境验收：<无，或 disposable Windows VM gate>

严格执行：
- 一个 task 一个 hy/ 分支、worktree 和 PR。
- 每个可独立验证步骤单独 commit。
- 不实现 task 外行为。
- 完成后按主提示词执行 verify、自审、CI、评论和合并循环。
```

## 任务耗尽

路线图中没有 `ready` 任务时：

- 不自拟新功能。
- 汇总已合并、待 review、被阻塞和需要维护者决策的任务。
- 保留所有未合并 PR 和证据，不用 `--admin` 清空队列。
- 将目标标记为完成的前提是：路线图范围全部完成，且没有 required task、CI 或 review 工作残留。
