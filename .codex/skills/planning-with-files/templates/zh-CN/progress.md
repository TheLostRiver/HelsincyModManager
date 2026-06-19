# 进度日志
<!--
  用途：由 planning hooks 写入的客观 auto records。
  归属：不要在这里手写日常行动总结；写入/编辑工具完成后由 hooks 追加事实记录。
  使用：阶段和状态写在 task_plan.md；测试结论、错误分析、决策和理由写在 findings.md。
-->

## Hook 写入的 Auto Records
<!--
  PostToolUse hooks 会追加类似记录：

  ### Auto Record: 2026-01-15 10:35:47
  - Tool: apply_patch
  - Session: unavailable
  - Plan-Source: workspace
  - Result: success
  - Files:
    - `src/example.py` (update)

  这些记录是事实审计条目，便于恢复上下文；解释性总结请写入 findings.md。
-->

## 5 问恢复检查
<!--
  这是只读恢复指引，不是要求手写 progress 记录。

  1. 我在哪里？ -> task_plan.md 中的当前阶段/状态
  2. 我要去哪里？ -> task_plan.md 中的剩余阶段
  3. 目标是什么？ -> task_plan.md 中的目标
  4. 我学到了什么？ -> findings.md
  5. 哪些文件发生过变化？ -> 本文件中的 hook auto records
-->
