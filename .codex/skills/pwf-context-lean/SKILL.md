---
name: pwf-context-lean
description: Switch the current session to lean PWF context injection. Invoke with /pwf-context-lean.
user-invocable: true
allowed-tools: "Bash"
---

# /pwf-context-lean

Run:

```bash
python .codex/skills/planning-with-files/scripts/plan.py context set lean
```

中文模式：如果用户希望中文输出，先设置 `PWF_LANG=zh-CN`，再运行相同命令。

This changes only the current session context profile. Use this when you want a smaller PWF context payload for the current conversation.
