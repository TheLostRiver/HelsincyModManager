---
name: pwf-context-notice-auto
description: Automatically show useful PWF context injection notices for the current session. Invoke with /pwf-context-notice-auto.
user-invocable: true
allowed-tools: "Bash"
---

# /pwf-context-notice-auto

Run:

```bash
python .codex/skills/planning-with-files/scripts/plan.py context notice auto
```

中文模式：如果用户希望中文输出，先设置 `PWF_LANG=zh-CN`，再运行相同命令。

This changes only the current session notice setting. Auto mode shows notices for expanded, deep, and session-start context recovery.
