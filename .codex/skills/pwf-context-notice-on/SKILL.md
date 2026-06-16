---
name: pwf-context-notice-on
description: Always show PWF context injection notices for the current session. Invoke with /pwf-context-notice-on.
user-invocable: true
allowed-tools: "Bash"
---

# /pwf-context-notice-on

Run:

```bash
python .codex/skills/planning-with-files/scripts/plan.py context notice on
```

中文模式：如果用户希望中文输出，先设置 `PWF_LANG=zh-CN`，再运行相同命令。

This changes only the current session notice setting. It does not change other sessions or workspace active plan.
