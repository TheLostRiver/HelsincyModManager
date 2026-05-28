---
name: pwf-capture
description: Capture external or visual context into findings.md. Invoke with /pwf-capture.
user-invocable: true
allowed-tools: "Bash"
---

# /pwf-capture

Use any text after `/pwf-capture` as the capture metadata.

If kind, source, or summary is missing, ask the user for the missing fields. Otherwise run:

```powershell
python .codex\skills\planning-with-files\scripts\plan.py capture --kind <web|browser|image|pdf|file|note> --source <source> --summary <summary>
```

中文模式：如果用户希望中文输出，先设置 `PWF_LANG=zh-CN`，再运行相同命令。

Keep the summary factual and concise. Capture only context that should persist beyond the current model context window.
