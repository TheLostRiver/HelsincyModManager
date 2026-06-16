---
name: pwf-context-status
description: Show the current session PWF context profile and notice settings. Invoke with /pwf-context-status.
user-invocable: true
allowed-tools: "Bash"
---

# /pwf-context-status

Run:

```bash
python .codex/skills/planning-with-files/scripts/plan.py context status
```

中文模式：如果用户希望中文输出，先设置 `PWF_LANG=zh-CN`，再运行相同命令。

Session context is selected from `PWF_SESSION_ID` when it is available; without it, the command shows default and environment-derived context settings.

Show the effective profile, source, session profile, notice mode, notice source, progress mode, plan head/tail limits, findings state, and max chars for the current session.
