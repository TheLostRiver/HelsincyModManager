---
name: pwf-use
description: Bind the current session to a visible Helsincy Plan With Files task. Invoke with /pwf-use.
user-invocable: true
allowed-tools: "Bash"
---

# /pwf-use

Use any text after `/pwf-use` as the required task selector and optional flags.

Run:

```powershell
python .codex\skills\planning-with-files\scripts\plan.py use <task selector and optional flags>
```

中文模式：如果用户希望中文输出，先设置 `PWF_LANG=zh-CN`，再运行相同命令。

The selector may be a plan id or short id from `/pwf-tasks`. By default it only resolves tasks visible to the current session. Use explicit `--claim` only when the user intentionally wants to take over a task whose owner has gone stale. Cross-session task sharing was removed (see docs/REMOVED_CROSS_SESSION_SHARE.md).
