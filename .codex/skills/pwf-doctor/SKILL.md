---
name: pwf-doctor
description: Run Helsincy Plan With Files diagnostics. Invoke with /pwf-doctor.
user-invocable: true
allowed-tools: "Bash"
---

# /pwf-doctor

Run:

```powershell
python .codex\skills\planning-with-files\scripts\plan.py doctor
```

中文模式：如果用户希望中文输出，先设置 `PWF_LANG=zh-CN`，再运行相同命令。

Report the diagnostic output clearly. If any check is not healthy, explain the next safe fix before editing files.
