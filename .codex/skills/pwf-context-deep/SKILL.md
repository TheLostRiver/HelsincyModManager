---
name: pwf-context-deep
description: Switch the current session to deep PWF context injection. Invoke with /pwf-context-deep.
user-invocable: true
allowed-tools: "Bash"
---

# /pwf-context-deep

Run:

```bash
python .codex/skills/planning-with-files/scripts/plan.py context set deep
```

中文模式：如果用户希望中文输出，先设置 `PWF_LANG=zh-CN`，再运行相同命令。

This changes only the current session context profile. Use this for deliberate recovery after heavy context compaction. It injects more context than expanded mode.
