---
name: pwf-status
description: Show the active Helsincy Plan With Files task status. Invoke with /pwf-status.
user-invocable: true
allowed-tools: "Bash"
---

# /pwf-status

Run:

```powershell
python .codex\skills\planning-with-files\scripts\plan.py status
```

中文模式：如果用户希望中文输出，先设置 `PWF_LANG=zh-CN`，再运行相同命令。

Summarize the active plan, current phase, and recent progress in plain language.
