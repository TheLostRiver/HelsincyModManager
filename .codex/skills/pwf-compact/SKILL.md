---
name: pwf-compact
description: Roll over old progress.md auto records into append-only active/archive segments. Invoke with /pwf-compact.
user-invocable: true
allowed-tools: "Bash"
---

# /pwf-compact

Roll over old objective auto records into a newly created archive segment under `progress-archive/<session-key>/`, then continue recent hot records in a newly created active segment under `progress-active/<session-key>/`. The command appends to `progress-index.ndjson` and does not delete or overwrite existing progress/archive files.

Run:

```powershell
python .codex\skills\planning-with-files\scripts\plan.py compact
```

中文模式：如果用户希望中文输出，先设置 `PWF_LANG=zh-CN`，再运行相同命令。

If the user asks for a custom keep count, pass `--keep-records <N>`.
If the user asks to preview only, pass `--dry-run`.

After it completes, summarize archived count, kept count, archive path, and active progress path.
