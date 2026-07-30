# Codex 目标模式提示词

本文档用于维护者暂时离开时的自主迭代。目标模式以交付可运行能力和关闭 release blocker
为衡量标准，不以运行时长、代码行数、提交数或 PR 数量衡量进度。

任务顺序以[自主迭代路线图](AUTONOMOUS_ITERATION_ROADMAP.md)为准，当前事实以
[项目任务状态快照](PROJECT_TASK_STATUS.md)、源码和测试为准。本轮只处理 Windows + MHW:I，
排除 Linux / Steam Deck。

目标模式可以在项目任务完成、切片进入人工验收、需要维护者决策或遇到安全边界时暂停。
暂停不是失败，不需要为了保持运行而制造微型任务。

## 主提示词

```text
目标：按照 docs/AUTONOMOUS_ITERATION_ROADMAP.md 自主推进 Helsincy Mod Manager 的
Windows + MHW:I 任务，直到项目任务完成，或当前能力需要维护者人工验收/决策。

开始：
1. 读取 AGENTS.md 和当前任务真正涉及的架构、安全、测试及专题文档，不重复加载无关规则。
2. 检查 git status、当前分支、已有 worktree、open PR 和 CI。保留并隔离用户或其他任务的改动。
3. 从路线图选择第一个 ready 的纵向产品切片；先用源码和测试确认缺口，避免重复实现。
4. 一个纵向切片维护一个 PWF task。只记录阶段转换、重要 finding、决策、阻塞和恢复点。

执行：
1. 交付最小但完整的端到端能力，不把文档同步、测试搬迁、dead-code 或几行清理单独开 PR。
2. 一个纵向产品切片通常使用一个分支和 PR。当前工作区 clean 且没有并行冲突时不强制新 worktree；
   需要隔离 dirty 改动或并行任务时才创建 worktree。
3. commit 用于形成真实的 review 边界，不设数量目标，不为形式上的小步骤制造提交。
4. 开发期间运行聚焦测试。只有高风险写入、安全、并发、公共契约或治理变更在 PR ready 前
   运行一次完整 verify；小型 review 修复只重跑受影响检查，除非旧完整证据已经失效。
5. 不做切片外重构。发现相邻小问题时，只有它阻塞当前能力且修复风险可控才一并处理。

安全边界：
1. 游戏目录写入继续遵守 InstallPlan -> planned recovery intent -> backup ->
   Committing facts -> commit -> atomic manifest -> cleanup/recovery。
2. 同一 game/profile 写入串行；scan/hash/extract/analyze 留在写锁外。
3. 原始 Mod 只读，派生产物只进入 staging/sandbox；uninstall 使用 manifest，不猜测包内容。
4. 前端和 CLI 只调用 app use case，不自行计算路径、重定向、备份、回滚或批量事务规则。
5. 测试使用 temp/fake/人工 fixture，不触碰真实游戏、Mod、Steam userdata 或玩家存档。
6. 未完成跨进程 admission 和明确授权前，Production CLI 写命令保持不可达。

PR 与合并：
1. PR ready 前执行一次 findings-first 自审；真实 bug 必须修复并补测试，误报必须留下可复核证据。
2. required CI 必须 terminal success。等待可以记录后暂停并在下一轮恢复，不重复轮询制造进度。
3. 获取全部 review、inline thread 和评论。CodeRabbit 缺席不等于批准，但也不无限等待；
   对完整 diff 做一次独立自审即可补位。
4. 普通合并优先。只有当前目标已明确授权、全部门禁满足，且唯一阻挡是已知 approval ruleset 时，
   才允许 --admin；绝不绕过 CI、真实 bug、未解决线程或人工决策。
5. 合并后只更新一次权威任务队列；其他状态文档只在里程碑变化时同步。

暂停并汇报：
1. 需要维护者选择产品行为、安全策略、数据来源或许可方案。
2. 需要真实 Windows/Steam 账号、头像网络状态、Scheduled Task、真实 Mod/存档或可视化体验验收。
3. 工作区改动归属不明且无法安全隔离，或当前修复需要明显扩大切片范围。
4. required CI 持续失败且已确认不是当前范围内可修复的问题。
5. 当前切片已达到完成定义并进入人工验收，或路线图没有 ready 任务。

不要为了持续运行而扩大权限、范围或风险，也不要因为时间和上下文限制降低数据安全及合并门禁。
```

## 继续提示词

```text
继续当前 Helsincy Mod Manager 目标。

1. 读取当前 PWF 的 task_plan.md 和 findings.md，检查 git/PR/CI/review 的实时状态。
2. 确认当前纵向切片、最后一个有效验证点和最早未完成步骤。
3. 已成功且仍适用于当前 commit 的操作不重复执行。
4. 代码或依赖变化后按影响范围验证；小修不无条件重复完整 verify。
5. dirty 改动归属不明时先核对或隔离，不清理、不重置、不覆盖。
6. 若已进入人工验收或维护者决策点，汇报证据并暂停。
```

## PR 评论处理

```text
处理当前 PR 的全部 review、inline thread 和普通评论。

1. 每条反馈分类为真实 bug、测试/契约缺口、可维护性问题、误报或需维护者决策。
2. 先处理玩家数据、安全链路和公共契约问题，不因评论来自机器人就自动采纳或忽略。
3. 当前切片内的真实 bug做最小修复并补测试；范围外问题记录后暂停拆分判断。
4. 误报引用源码、测试、契约或运行证据回复后再 resolve。
5. 每次 push 后重跑受影响检查并等待 CI；只有旧证据失效时才重复完整 verify。
6. 最终 findings-first 汇报未解决项，不用“应该没问题”代替证据。
```

## CodeRabbit 缺席

```text
当前 PR 没有 CodeRabbit review。

1. 不把缺席视为批准，也不猜测具体原因或无限等待额度恢复。
2. 使用 hmm-review-gate 对完整 PR diff 做一次独立 findings-first 自审。
3. 高风险切片重点检查成功、失败、取消、回滚/恢复、并发、脱敏和 containment。
4. 在 PR 记录 commit SHA、自审结论和实际验证命令。
5. 仍有真实 bug、测试/契约缺口、未解决线程或不确定的高风险行为时禁止合并。
```

## CI 处理

```text
检查当前 PR 的 CI。

1. required check 只有 terminal success 才算通过。
2. 当前 diff 引入的失败应修复并补测试；确定性失败不能靠重跑掩盖。
3. 只有证据表明是瞬时基础设施故障时才重跑。
4. 既有或环境问题必须给出证据；required check 未成功时保持不合并。
5. 长时间 pending 可以记录状态并暂停，下一轮继续等待，不重复轮询制造进度。
6. CI 成功后仍检查 review threads、自审 findings 和基线变化。
```

## 合并提示词

```text
评估当前 PR，仅在以下条件全部满足时合并：

1. PR 只包含当前纵向切片或 release blocker，没有无关改动和禁入产物。
2. 当前 commit 的聚焦测试已通过；高风险/公共契约/治理切片有仍然有效的完整 verify 证据。
3. required CI 全部 terminal success。
4. 所有评论和 review thread 已处理，真实 bug 和测试/契约缺口为零。
5. CodeRabbit 缺席时已有当前 commit 的独立全 diff 自审记录。
6. 没有等待维护者决定的产品、安全、许可、数据来源或人工验收问题。

先尝试普通合并。只有当前目标已有明确 admin 授权、以上条件全部满足且唯一阻挡为已知
approval ruleset 时才使用 --admin。合并后确认目标分支包含结果，并一次性更新权威任务队列。
```

## 纵向切片模板

```text
切片：<id 和可演示能力>
内部工作包：<task ids>
前置：<已满足的依赖>
范围：<允许和禁止修改的边界>
风险：<low/medium/high>
完成定义：<用户或 CLI 可验证行为>
聚焦验证：<命令>
完整验证：<需要，或不需要及原因>
人工验收：<无，或明确环境/操作者>

执行一个产品切片、一个 PWF task、一个分支和通常一个 PR。
小型文档、测试整理和内部前置并入该切片；不为微小步骤拆 PR 或强制 commit。
```

## 任务耗尽

路线图没有 `ready` 切片时，不自拟新功能。汇总已合并、待 review、阻塞和需要人工验收的切片。
只有路线图范围全部完成且没有 required CI、review 或验收工作残留时，才能把项目目标标记为完成。
