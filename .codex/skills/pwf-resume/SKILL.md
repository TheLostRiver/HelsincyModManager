---
name: pwf-resume
description: Resume Helsinsky Plan With Files context injection for the current session. Invoke with /pwf-resume.
user-invocable: true
allowed-tools: "Bash"
---

# /pwf-resume

Run:

```bash
python .codex/skills/planning-with-files/scripts/plan.py context resume
```

中文模式：如果用户希望中文输出，先设置 `PWF_LANG=zh-CN`，再运行相同命令。

Restores planning context injection for the current session after a `/pwf-pause`. If the session was not paused, this prints `not paused; nothing to resume` and changes nothing — that hint makes the command visible instead of silently no-oping.

Only affects the current session. Other sessions and the workspace active plan are not touched.
