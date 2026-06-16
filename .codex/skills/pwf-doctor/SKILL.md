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

`/pwf-doctor` also audits append-only progress storage. It checks `progress-index.ndjson`, active/archive directory roles, missing indexed files, hash mismatches, and orphan generated segments. It is report-only: it prints `No automatic repair was attempted.` and never deletes, moves, overwrites, compacts, or recreates progress files. Use `plan.py doctor --verbose` for effect/action details, `--json` for machine-readable output, and `--strict` to fail on warnings.

`/pwf-doctor` 还会审计 append-only progress storage：检查 `progress-index.ndjson`、`progress-active/` 与 `progress-archive/` 的目录角色、索引文件缺失、hash mismatch，以及未被 index 引用的 generated segment。它只报告，不自动修复；输出会明确包含 `No automatic repair was attempted.`，并且不会删除、移动、覆盖、compact 或重建任何 progress 文件。需要细节时用 `plan.py doctor --verbose`，需要机器可读输出时用 `--json`，需要 CI 式严格失败时用 `--strict`。

Report the diagnostic output clearly. If any check is not healthy, explain the next safe fix before editing files.
