---
name: pwf-pause
description: Pause Helsinsky Plan With Files context injection for the current session. Invoke with /pwf-pause.
user-invocable: true
allowed-tools: "Bash"
---

# /pwf-pause

Run:

```bash
python .codex/skills/planning-with-files/scripts/plan.py context pause
```

中文模式：如果用户希望中文输出，先设置 `PWF_LANG=zh-CN`，再运行相同命令。

Pause suppresses planning context injection for `SessionStart`, `UserPromptSubmit`, and `PreToolUse` events in the current session only. `PostToolUse` progress recording (objective file change logging) keeps working — pauses are scoped to context injection, not to progress facts.

This is useful when you want a quiet session: for example, a side conversation where you don't want the active plan context injected every prompt, or a quick question that doesn't need the full planning notebook.

Use `/pwf-resume` to restore injection. Running `/pwf-pause` when already paused prints `already paused` and changes nothing. Other sessions and the workspace active plan are not affected.
