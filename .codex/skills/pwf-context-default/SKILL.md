---
name: pwf-context-default
description: Switch the current session to default PWF context injection. Invoke with /pwf-context-default.
user-invocable: true
allowed-tools: "Bash"
---

# /pwf-context-default

Run:

```bash
python .codex/skills/planning-with-files/scripts/plan.py context set default
```

中文模式：如果用户希望中文输出，先设置 `PWF_LANG=zh-CN`，再运行相同命令。

This changes only the current session context profile. It does not change other sessions or workspace active plan.
