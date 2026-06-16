---
name: pwf-context-expanded
description: Switch the current session to expanded PWF context injection. Invoke with /pwf-context-expanded.
user-invocable: true
allowed-tools: "Bash"
---

# /pwf-context-expanded

Run:

```bash
python .codex/skills/planning-with-files/scripts/plan.py context set expanded
```

中文模式：如果用户希望中文输出，先设置 `PWF_LANG=zh-CN`，再运行相同命令。

This changes only the current session context profile. It does not change other sessions or workspace active plan.
