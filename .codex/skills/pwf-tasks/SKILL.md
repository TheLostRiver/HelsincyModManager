---
name: pwf-tasks
description: List Helsincy Plan With Files tasks visible to the current session. Invoke with /pwf-tasks.
user-invocable: true
allowed-tools: "Bash"
---

# /pwf-tasks

Use any text after `/pwf-tasks` as optional flags.

Run:

```powershell
python .codex\skills\planning-with-files\scripts\plan.py tasks <optional flags>
```

中文模式：如果用户希望中文输出，先设置 `PWF_LANG=zh-CN`，再运行相同命令。

By default, show only tasks visible to the current session. Do not show tasks owned by other sessions unless the user explicitly passes `--all`.
