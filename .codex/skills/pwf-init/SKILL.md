---
name: pwf-init
description: Create a new Helsincy Plan With Files task. Invoke with /pwf-init.
user-invocable: true
allowed-tools: "Bash"
---

# /pwf-init

Use any text after `/pwf-init` as the task name and options.

If no task name was provided, ask the user for the task name. Otherwise run:

```powershell
python .codex\skills\planning-with-files\scripts\plan.py init <task name and options>
```

中文模式：如果用户希望中文输出和中文 planning 模板，先设置 `PWF_LANG=zh-CN`，再运行相同命令。

After it completes, summarize the active plan path and the created planning files.
