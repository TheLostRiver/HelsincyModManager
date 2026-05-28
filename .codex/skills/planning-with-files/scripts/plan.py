#!/usr/bin/env python3
from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import sys
from datetime import datetime
from pathlib import Path
from typing import Any, Iterable


CODEX_DIR = Path(__file__).resolve().parents[3]
HOOKS_DIR = CODEX_DIR / "hooks"
SKILL_DIR = CODEX_DIR / "skills" / "planning-with-files"
TEMPLATES_DIR = SKILL_DIR / "templates"
if str(HOOKS_DIR) not in sys.path:
    sys.path.insert(0, str(HOOKS_DIR))

import planning_state  # noqa: E402
import progress_lifecycle  # noqa: E402


REQUIRED_HOOK_ENTRYPOINTS = [
    ".codex/hooks/session_start.py",
    ".codex/hooks/user_prompt_submit.py",
    ".codex/hooks/pre_tool_use.py",
    ".codex/hooks/post_tool_use.py",
    ".codex/hooks/stop.py",
]
DEFAULT_COMPACT_THRESHOLD = 100
CLI_MESSAGES = {
    "en": {
        "active_plan_missing": "active plan: missing",
        "active_plan": "active plan: {plan_id}",
        "active_plan_ok": "active plan: ok {path}",
        "active_plan_set_to": "active plan set to: {plan_id}",
        "attestation_clear_missing": "[plan-attest] No attestation to clear.",
        "attestation_cleared": "[plan-attest] Cleared attestation for {path}.",
        "attestation_locked": "[plan-attest] Locked {path}",
        "attestation_missing_for_plan": "[plan-attest] No attestation set for {path}.",
        "attestation_no_plan": "[plan-attest] No task_plan.md found. Create a plan first.",
        "attestation_path_label": "Attestation: {path}",
        "attestation_plan_label": "Plan: {path}",
        "attestation_will_block": (
            "[plan-attest] Hooks will block injection if the file is modified "
            "without re-running this command."
        ),
        "capture_no_plan": "No active plan found. Create or switch to a plan before capturing external context.",
        "captured_external_context": "captured external context: {kind}",
        "compact_archive": "archive: {path}",
        "compact_archived": "archived auto records: {count}",
        "compact_dry_run": "progress compaction dry run",
        "compact_keep": "keep records: {count}",
        "compact_kept": "kept recent auto records: {count}",
        "compact_no_plan": "No active plan found. Create or switch to a plan before compacting progress.",
        "compact_not_needed": "progress compaction not needed",
        "compact_total": "auto records: {count}",
        "compact_would_archive": "would archive auto records: {count}",
        "compact_would_keep": "would keep recent auto records: {count}",
        "compacted": "compacted progress.md",
        "created": "created {label}: {plan_id}",
        "current_phase": "current phase: {phase}",
        "findings_path": "findings: {path}",
        "help_attest": "Lock, show, or clear plan attestation",
        "help_capture": "Append external context to findings.md",
        "help_compact": "Archive old progress.md auto records",
        "help_doctor": "Diagnose hooks, active plan, and attestation",
        "help_force": "Overwrite existing planning files",
        "help_init": "Create a new planning session",
        "help_legacy": "Create root-level planning files",
        "help_root": "Project root to inspect",
        "help_status": "Show active plan status",
        "help_switch": "Set or show active plan",
        "hook_files_missing": "hook files: missing {paths}",
        "hook_files_ok": "hook files: ok",
        "hooks_json": "hooks.json: {status}",
        "language_unsupported": "language: warning unsupported PWF_LANG={lang}",
        "legacy_plan_label": "legacy plan",
        "no_active_plan": "no active plan set",
        "path": "path: {path}",
        "phases": "phases: {complete}/{total} complete",
        "plan_already_exists": "{label} already exists: {path}",
        "plan_dir_missing": "plan directory not found: {path}",
        "plan_label": "plan",
        "planning_files_missing": "planning files: missing",
        "planning_files_missing_list": "planning files: missing {paths}",
        "planning_files_ok": "planning files: ok",
        "progress": "progress: {count} auto records{suffix}",
        "progress_compact_suffix": ", compact recommended",
        "progress_warning": "[warn] progress.md has {count} auto records; run /pwf-compact or plan.py compact",
        "python_runtime_ok": "python runtime: ok",
        "python_runtime_warning": "python runtime: warning python3 command in hooks.json",
    },
    "zh-CN": {
        "active_plan_missing": "当前计划: 缺失",
        "active_plan": "当前计划: {plan_id}",
        "active_plan_ok": "当前计划: ok {path}",
        "active_plan_set_to": "已将当前计划设为: {plan_id}",
        "attestation_clear_missing": "[plan-attest] 没有可清除的 attestation。",
        "attestation_cleared": "[plan-attest] 已清除 {path} 的 attestation。",
        "attestation_locked": "[plan-attest] 已锁定 {path}",
        "attestation_missing_for_plan": "[plan-attest] {path} 未设置 attestation。",
        "attestation_no_plan": "[plan-attest] 未找到 task_plan.md。请先创建计划。",
        "attestation_path_label": "Attestation: {path}",
        "attestation_plan_label": "计划: {path}",
        "attestation_will_block": "[plan-attest] 如果文件被修改且没有重新运行此命令，hooks 会阻止注入。",
        "capture_no_plan": "未找到当前计划。请先创建或切换计划，再捕获外部上下文。",
        "captured_external_context": "已捕获外部上下文: {kind}",
        "compact_archive": "归档文件: {path}",
        "compact_archived": "已归档 auto records: {count}",
        "compact_dry_run": "progress 压缩预演",
        "compact_keep": "保留记录数: {count}",
        "compact_kept": "保留最近 auto records: {count}",
        "compact_no_plan": "未找到当前计划。请先创建或切换计划，再压缩 progress。",
        "compact_not_needed": "progress 暂不需要压缩",
        "compact_total": "auto records: {count}",
        "compact_would_archive": "将归档 auto records: {count}",
        "compact_would_keep": "将保留最近 auto records: {count}",
        "compacted": "已压缩 progress.md",
        "created": "已创建{label}: {plan_id}",
        "current_phase": "当前阶段: {phase}",
        "findings_path": "findings: {path}",
        "help_attest": "锁定、查看或清除计划 attestation",
        "help_capture": "将外部上下文追加到 findings.md",
        "help_compact": "归档旧的 progress.md auto records",
        "help_doctor": "诊断 hooks、当前计划和 attestation",
        "help_force": "覆盖已有 planning 文件",
        "help_init": "创建新的 planning 会话",
        "help_legacy": "创建根目录级 planning 文件",
        "help_root": "要检查的项目根目录",
        "help_status": "显示当前计划状态",
        "help_switch": "设置或显示当前计划",
        "hook_files_missing": "hook 文件: 缺失 {paths}",
        "hook_files_ok": "hook 文件: ok",
        "hooks_json": "hooks.json: {status}",
        "language_unsupported": "language: warning unsupported PWF_LANG={lang}",
        "legacy_plan_label": "legacy plan",
        "no_active_plan": "未设置当前计划",
        "path": "路径: {path}",
        "phases": "阶段: {complete}/{total} 已完成",
        "plan_already_exists": "{label}已存在: {path}",
        "plan_dir_missing": "计划目录不存在: {path}",
        "plan_label": "计划",
        "planning_files_missing": "planning 文件: 缺失",
        "planning_files_missing_list": "planning 文件: 缺失 {paths}",
        "planning_files_ok": "planning 文件: ok",
        "progress": "进度: {count} 条 auto records{suffix}",
        "progress_compact_suffix": "，建议压缩",
        "progress_warning": "[warn] progress.md 已有 {count} 条 auto records；请运行 /pwf-compact 或 plan.py compact",
        "python_runtime_ok": "Python runtime: ok",
        "python_runtime_warning": "Python runtime: warning hooks.json 中存在 python3 command",
    },
}


def _message(key: str, **values: object) -> str:
    text = CLI_MESSAGES[planning_state.current_lang()][key]
    return text.format(**values) if values else text


def _help(key: str) -> str:
    return _message(f"help_{key}")


def _unsupported_language_warning() -> str:
    lang = os.environ.get("PWF_LANG", "").strip()
    if lang and lang not in planning_state.SUPPORTED_LANGS:
        return CLI_MESSAGES["en"]["language_unsupported"].format(lang=lang)
    return ""


def _collect_hook_commands(value: Any) -> list[str]:
    commands: list[str] = []
    if isinstance(value, dict):
        command = value.get("command")
        if isinstance(command, str):
            commands.append(command)
        for child in value.values():
            commands.extend(_collect_hook_commands(child))
    elif isinstance(value, list):
        for child in value:
            commands.extend(_collect_hook_commands(child))
    return commands


def _load_hooks_json(root: Path) -> tuple[dict[str, Any] | None, str]:
    hooks_json = root / ".codex" / "hooks.json"
    if not hooks_json.is_file():
        return None, "missing"
    try:
        payload = json.loads(hooks_json.read_text(encoding="utf-8"))
    except json.JSONDecodeError:
        return None, "invalid"
    return payload if isinstance(payload, dict) else None, "ok"


def _uses_python3(command: str) -> bool:
    words = command.replace("\\", "/").split()
    return bool(words and words[0].endswith("python3"))


def _short_digest(value: str | None) -> str:
    return value[:12] if value else "unknown"


def _plan_id(root: Path, paths: planning_state.PlanningPaths) -> str:
    try:
        if paths.root.resolve() == root.resolve():
            return "legacy"
    except OSError:
        pass
    return paths.root.name


def _current_phase(paths: planning_state.PlanningPaths) -> str | None:
    if not paths.task_plan.is_file():
        return None
    lines = paths.task_plan.read_text(encoding="utf-8", errors="replace").splitlines()
    in_comment = False
    for index, line in enumerate(lines):
        if line.strip() != "## Current Phase":
            continue
        for value in lines[index + 1 :]:
            candidate = value.strip()
            if candidate.startswith("<!--"):
                in_comment = True
                if candidate.endswith("-->"):
                    in_comment = False
                continue
            if in_comment:
                if candidate.endswith("-->"):
                    in_comment = False
                continue
            if candidate:
                return candidate
        return None
    return None


def _compact_threshold() -> int:
    raw = os.environ.get("PWF_COMPACT_THRESHOLD", "").strip()
    if not raw:
        return DEFAULT_COMPACT_THRESHOLD
    try:
        value = int(raw)
    except ValueError:
        return DEFAULT_COMPACT_THRESHOLD
    return value if value >= 1 else DEFAULT_COMPACT_THRESHOLD


def _progress_record_count(paths: planning_state.PlanningPaths | None) -> int:
    if paths is None:
        return 0
    return progress_lifecycle.count_auto_records(paths.progress)


def _progress_status_line(paths: planning_state.PlanningPaths | None) -> str:
    count = _progress_record_count(paths)
    suffix = _message("progress_compact_suffix") if count >= _compact_threshold() else ""
    return _message("progress", count=count, suffix=suffix)


def _progress_doctor_warning(paths: planning_state.PlanningPaths | None) -> str:
    count = _progress_record_count(paths)
    if count < _compact_threshold():
        return ""
    return _message("progress_warning", count=count)


def _slugify(value: str) -> str:
    slug = re.sub(r"[^a-z0-9]+", "-", value.lower()).strip("-")
    if value.isascii():
        return slug or "plan"
    digest = hashlib.sha256(value.encode("utf-8")).hexdigest()[:8]
    return f"{slug or 'plan'}-{digest}"


def _template_path(filename: str) -> Path:
    lang = planning_state.current_lang()
    localized = TEMPLATES_DIR / lang / filename
    if localized.is_file():
        return localized
    return TEMPLATES_DIR / filename


def _read_template(filename: str, fallback: str) -> str:
    path = _template_path(filename)
    if path.is_file():
        return path.read_text(encoding="utf-8")
    return fallback


def _render_template(text: str, name: str | None = None) -> str:
    if name is None:
        return text
    return (
        text.replace("[Brief Description]", name)
        .replace("[简要描述]", name)
        .replace("[One sentence describing the end state]", "Define the desired end state.")
        .replace("[用一句话描述最终要达成的状态]", "定义本任务要达成的最终状态。")
    )


def _task_plan_fallback(name: str) -> str:
    return "\n".join(
        [
            f"# Task Plan: {name}",
            "",
            "## Goal",
            "Define the desired end state.",
            "",
            "## Current Phase",
            "Phase 1",
            "",
            "## Phases",
            "",
            "### Phase 1: Discovery",
            "- [ ] Confirm requirements",
            "- [ ] Record findings",
            "- **Status:** in_progress",
            "",
            "### Phase 2: Implementation",
            "- [ ] Make changes",
            "- [ ] Run verification",
            "- **Status:** pending",
            "",
            "## Decisions Made",
            "| Decision | Rationale |",
            "|----------|-----------|",
            "",
            "## Errors Encountered",
            "| Error | Resolution |",
            "|-------|------------|",
            "",
        ]
    )


def _task_plan_fallback_zh(name: str) -> str:
    return "\n".join(
        [
            f"# 任务计划: {name}",
            "",
            "## 目标",
            "定义本任务要达成的最终状态。",
            "",
            "## Current Phase",
            "Phase 1",
            "",
            "## 阶段",
            "",
            "### Phase 1: 需求与发现",
            "- [ ] 确认用户意图",
            "- [ ] 识别约束和需求",
            "- [ ] 将发现记录到 findings.md",
            "- **Status:** in_progress",
            "",
            "### Phase 2: 实现",
            "- [ ] 按计划修改文件",
            "- [ ] 增量运行验证",
            "- **Status:** pending",
            "",
            "## 决策记录",
            "| 决策 | 理由 |",
            "|------|------|",
            "",
            "## 遇到的错误",
            "| 错误 | 解决方式 |",
            "|------|----------|",
            "",
        ]
    )


def _task_plan_template(name: str) -> str:
    fallback = _task_plan_fallback_zh(name) if planning_state.current_lang() == "zh-CN" else _task_plan_fallback(name)
    return _render_template(_read_template("task_plan.md", fallback), name)


def _progress_template() -> str:
    fallback = "# 进度日志\n\n" if planning_state.current_lang() == "zh-CN" else "# Progress Log\n\n"
    return _read_template("progress.md", fallback)


def _findings_template() -> str:
    fallback = "# 研究发现\n\n" if planning_state.current_lang() == "zh-CN" else "# Findings\n\n"
    return _read_template("findings.md", fallback)


def _planning_files_status(paths: planning_state.PlanningPaths | None) -> tuple[str, bool]:
    if paths is None:
        return _message("planning_files_missing"), False

    required = [
        paths.task_plan,
        paths.progress,
        paths.findings,
    ]
    missing = [path.name for path in required if not path.is_file()]
    if missing:
        return _message("planning_files_missing_list", paths=", ".join(missing)), False
    return _message("planning_files_ok"), True


def _attestation_status(root: Path, paths: planning_state.PlanningPaths | None) -> tuple[str, bool]:
    if paths is None:
        return "attestation: not checked", False

    status = planning_state.plan_attestation_status(root, paths)
    if status.valid is None:
        return "attestation: not set", True
    if status.valid:
        return f"attestation: ok sha256={_short_digest(status.actual)}", True
    return (
        "attestation: tampered "
        f"expected={_short_digest(status.expected)} actual={_short_digest(status.actual)}",
        False,
    )


def doctor(root: Path) -> int:
    lines: list[str] = []
    ok = True
    language_warning = _unsupported_language_warning()
    if language_warning:
        lines.append(language_warning)

    hooks_payload, hooks_status = _load_hooks_json(root)
    if hooks_status == "ok":
        lines.append(_message("hooks_json", status="ok"))
    else:
        lines.append(_message("hooks_json", status=hooks_status))
        ok = False

    missing_hook_files = [
        path for path in REQUIRED_HOOK_ENTRYPOINTS if not (root / path).is_file()
    ]
    if missing_hook_files:
        lines.append(_message("hook_files_missing", paths=", ".join(missing_hook_files)))
        ok = False
    else:
        lines.append(_message("hook_files_ok"))

    commands = _collect_hook_commands(hooks_payload) if hooks_payload is not None else []
    if any(_uses_python3(command) for command in commands):
        lines.append(_message("python_runtime_warning"))
    else:
        lines.append(_message("python_runtime_ok"))

    paths = planning_state.planning_paths(root)
    if paths is None:
        lines.append(_message("active_plan_missing"))
        ok = False
    else:
        lines.append(_message("active_plan_ok", path=paths.root))

    planning_line, planning_ok = _planning_files_status(paths)
    lines.append(planning_line)
    ok = ok and planning_ok

    attestation_line, attestation_ok = _attestation_status(root, paths)
    lines.append(attestation_line)
    ok = ok and attestation_ok

    warning = _progress_doctor_warning(paths)
    if warning:
        lines.append(warning)

    print("\n".join(lines))
    return 0 if ok else 1


def status(root: Path) -> int:
    paths = planning_state.planning_paths(root)
    if paths is None:
        print(_message("active_plan_missing"))
        print(_message("planning_files_missing"))
        return 1

    print(_message("active_plan", plan_id=_plan_id(root, paths)))
    print(_message("path", path=paths.root))

    phase = _current_phase(paths)
    if phase:
        print(_message("current_phase", phase=phase))

    counts = planning_state.phase_counts(root)
    if counts is not None:
        total, complete, _in_progress, _pending = counts
        print(_message("phases", complete=complete, total=total))

    planning_line, planning_ok = _planning_files_status(paths)
    print(planning_line)

    attestation_line, attestation_ok = _attestation_status(root, paths)
    print(attestation_line)
    print(_progress_status_line(paths))
    return 0 if planning_ok and attestation_ok else 1


def init(root: Path, name: str, legacy: bool = False, force: bool = False) -> int:
    if legacy:
        target = root
        label = _message("legacy_plan_label")
        plan_id = "legacy"
    else:
        plan_id = f"{datetime.now().strftime('%Y-%m-%d')}-{_slugify(name)}"
        target = root / ".planning" / plan_id
        label = _message("plan_label")

    existing = [target / "task_plan.md", target / "progress.md", target / "findings.md"]
    if any(path.exists() for path in existing) and not force:
        print(_message("plan_already_exists", label=label, path=target))
        return 1

    target.mkdir(parents=True, exist_ok=True)
    (target / "task_plan.md").write_text(_task_plan_template(name), encoding="utf-8", newline="\n")
    (target / "progress.md").write_text(_progress_template(), encoding="utf-8", newline="\n")
    (target / "findings.md").write_text(_findings_template(), encoding="utf-8", newline="\n")

    if not legacy:
        active_file = root / ".planning" / ".active_plan"
        active_file.parent.mkdir(parents=True, exist_ok=True)
        active_file.write_text(plan_id, encoding="utf-8")

    print(_message("created", label=label, plan_id=plan_id))
    print(_message("path", path=target))
    return 0


def switch(root: Path, plan_id: str | None) -> int:
    plan_root = root / ".planning"
    active_file = plan_root / ".active_plan"

    if not plan_id:
        if active_file.is_file():
            active = active_file.read_text(encoding="utf-8", errors="replace").strip()
            print(_message("active_plan", plan_id=active) if active else _message("no_active_plan"))
            return 0 if active else 1
        print(_message("no_active_plan"))
        return 1

    plan_dir = plan_root / plan_id
    if not (plan_dir / "task_plan.md").is_file():
        print(_message("plan_dir_missing", path=plan_dir))
        return 1

    active_file.parent.mkdir(parents=True, exist_ok=True)
    active_file.write_text(plan_id, encoding="utf-8")
    print(_message("active_plan_set_to", plan_id=plan_id))
    print(_message("path", path=plan_dir))
    return 0


def _sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def attest(root: Path, show: bool = False, clear: bool = False) -> int:
    paths = planning_state.planning_paths(root)
    if paths is None:
        print(_message("attestation_no_plan"))
        return 1

    attestation = planning_state.attestation_path(root, paths)

    if show:
        if not attestation.is_file():
            print(_message("attestation_missing_for_plan", path=paths.task_plan))
            return 1
        print(_message("attestation_plan_label", path=paths.task_plan))
        print(_message("attestation_path_label", path=attestation))
        print(f"SHA-256: {attestation.read_text(encoding='utf-8', errors='replace').strip()}")
        return 0

    if clear:
        if attestation.is_file():
            attestation.unlink()
            print(_message("attestation_cleared", path=paths.task_plan))
        else:
            print(_message("attestation_clear_missing"))
        return 0

    digest = _sha256(paths.task_plan)
    attestation.write_text(digest, encoding="ascii")
    print(_message("attestation_locked", path=paths.task_plan))
    print(f"[plan-attest] SHA-256: {digest[:12]}... (stored in {attestation})")
    print(_message("attestation_will_block"))
    return 0


def capture(root: Path, kind: str, source: str, summary: str, trust: str = "untrusted") -> int:
    paths = planning_state.planning_paths(root)
    if paths is None:
        print(_message("capture_no_plan"))
        return 1

    timestamp = datetime.now().strftime("%Y-%m-%d %H:%M:%S")
    block = "\n".join(
        [
            "",
            f"## External Context: {timestamp}",
            "",
            "---BEGIN EXTERNAL CONTEXT DATA---",
            f"- Kind: {kind}",
            f"- Source: {source}",
            f"- Trust: {trust}",
            f"- Summary: {summary}",
            "---END EXTERNAL CONTEXT DATA---",
            "",
        ]
    )
    paths.findings.parent.mkdir(parents=True, exist_ok=True)
    with paths.findings.open("a", encoding="utf-8", newline="\n") as handle:
        handle.write(block)

    print(_message("captured_external_context", kind=kind))
    print(_message("findings_path", path=paths.findings))
    return 0


def compact(root: Path, keep_records: int = 30, dry_run: bool = False, archive: str = "progress.archive.md") -> int:
    paths = planning_state.planning_paths(root)
    if paths is None:
        print(_message("compact_no_plan"))
        return 1

    archive_path = Path(archive)
    if not archive_path.is_absolute():
        archive_path = paths.root / archive_path

    try:
        result = progress_lifecycle.compact_progress(
            paths.progress,
            archive_path,
            keep_records=keep_records,
            dry_run=dry_run,
        )
    except ValueError as exc:
        print(str(exc))
        return 1

    if result.archived_count == 0:
        print(_message("compact_not_needed"))
        print(_message("compact_total", count=result.total_auto_records))
        print(_message("compact_keep", count=keep_records))
        return 0

    if dry_run:
        print(_message("compact_dry_run"))
        print(_message("compact_would_archive", count=result.archived_count))
        print(_message("compact_would_keep", count=result.kept_count))
    else:
        print(_message("compacted"))
        print(_message("compact_archived", count=result.archived_count))
        print(_message("compact_kept", count=result.kept_count))
    print(_message("compact_archive", path=result.archive_path))
    return 0


def main(argv: Iterable[str] | None = None) -> int:
    parser = argparse.ArgumentParser(prog="plan.py")
    parser.add_argument("--root", default=".", help=_help("root"))
    subparsers = parser.add_subparsers(dest="command", required=True)
    subparsers.add_parser("doctor", help=_help("doctor"))
    subparsers.add_parser("status", help=_help("status"))

    init_parser = subparsers.add_parser("init", help=_help("init"))
    init_parser.add_argument("name")
    init_parser.add_argument("--legacy", action="store_true", help=_help("legacy"))
    init_parser.add_argument("--force", action="store_true", help=_help("force"))

    switch_parser = subparsers.add_parser("switch", help=_help("switch"))
    switch_parser.add_argument("plan_id", nargs="?")

    attest_parser = subparsers.add_parser("attest", help=_help("attest"))
    attest_group = attest_parser.add_mutually_exclusive_group()
    attest_group.add_argument("--show", action="store_true")
    attest_group.add_argument("--clear", action="store_true")

    capture_parser = subparsers.add_parser("capture", help=_help("capture"))
    capture_parser.add_argument("--kind", required=True, choices=["web", "browser", "image", "pdf", "file", "note"])
    capture_parser.add_argument("--source", required=True)
    capture_parser.add_argument("--summary", required=True)
    capture_parser.add_argument("--trust", default="untrusted")

    compact_parser = subparsers.add_parser("compact", help=_help("compact"))
    compact_parser.add_argument("--keep-records", type=int, default=30)
    compact_parser.add_argument("--dry-run", action="store_true")
    compact_parser.add_argument("--archive", default="progress.archive.md")

    args = parser.parse_args(list(argv) if argv is not None else None)
    root = Path(args.root).resolve()

    if args.command == "doctor":
        return doctor(root)
    if args.command == "status":
        return status(root)
    if args.command == "init":
        return init(root, args.name, legacy=args.legacy, force=args.force)
    if args.command == "switch":
        return switch(root, args.plan_id)
    if args.command == "attest":
        return attest(root, show=args.show, clear=args.clear)
    if args.command == "capture":
        return capture(root, args.kind, args.source, args.summary, trust=args.trust)
    if args.command == "compact":
        return compact(root, keep_records=args.keep_records, dry_run=args.dry_run, archive=args.archive)
    return 2


if __name__ == "__main__":
    raise SystemExit(main())
