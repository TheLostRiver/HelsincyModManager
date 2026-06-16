#!/usr/bin/env python3
from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import sys
from dataclasses import dataclass, replace
from datetime import datetime, timezone
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
import codex_hook_adapter  # noqa: E402


REQUIRED_HOOK_ENTRYPOINTS = [
    ".codex/hooks/session_start.py",
    ".codex/hooks/user_prompt_submit.py",
    ".codex/hooks/pre_tool_use.py",
    ".codex/hooks/post_tool_use.py",
    ".codex/hooks/stop.py",
]
DEFAULT_COMPACT_THRESHOLD = 100
LEGACY_BIND_SESSION_UNSUPPORTED = (
    "legacy plans do not support session binding; create a named .planning task "
    "with plan.py init \"Task Name\" --bind-session instead."
)


@dataclass(frozen=True)
class TaskSummary:
    plan_id: str
    path: Path
    short_id: str
    title: str
    current_phase: str | None
    phase_counts: tuple[int, int, int, int] | None
    workspace_active: bool
    session_bound: bool
    lease_owner: str | None
    lease_status: str
    shared: bool
    visible: bool
    reason: str


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
        "compact_keep_source": "keep records: {count} (source: {source}, profile={profile})",
        "compact_kept": "kept recent auto records: {count}",
        "compact_no_plan": "No active plan found. Create or switch to a plan before compacting progress.",
        "compact_not_needed": "progress compaction not needed",
        "compact_active": "active progress: {path}",
        "compact_archive_custom_unsupported": "archive paths are generated automatically; --archive is no longer supported",
        "compact_total": "auto records: {count}",
        "compact_would_archive": "would archive auto records: {count}",
        "compact_would_keep": "would keep recent auto records: {count}",
        "compacted": "rolled over progress records",
        "context_cleared": "context settings cleared: {key}",
        "context_invalid_notice": "unsupported context notice mode: {notice}",
        "context_invalid_profile": "unsupported context profile: {profile}",
        "context_missing_session": "session id: unavailable; context commands only affect the current session",
        "context_notice_set": "context notice set: {notice}",
        "context_profile_set": "context profile set: {profile}",
        "context_paused": "context injection paused for this session; PostToolUse progress recording still works",
        "context_pause_already_paused": "context injection already paused for this session; nothing to do",
        "context_resumed": "context injection resumed for this session",
        "context_resume_not_paused": "context injection was not paused for this session; nothing to resume",
        "context_paused_status": "context paused: {paused}",
        "created": "created {label}: {plan_id}",
        "current_phase": "current phase: {phase}",
        "effective_plan": "effective plan: {plan_id}",
        "findings_path": "findings: {path}",
        "help_attest": "Lock, show, or clear plan attestation",
        "help_capture": "Append external context to findings.md",
        "help_compact": "Roll over old progress auto records into append-only active/archive segments",
        "help_context": "Manage current-session context profile",
        "help_doctor": "Diagnose hooks, active plan, and attestation",
        "help_force": "Overwrite existing planning files",
        "help_init": "Create a new planning session",
        "help_legacy": "Create root-level planning files",
        "help_bind_session": "Bind the created plan to the current session",
        "help_no_bind_session": "Do not bind the created plan to the current session",
        "help_no_workspace_active": "Do not update .planning/.active_plan",
        "help_root": "Project root to inspect",
        "help_status": "Show active plan status",
        "help_switch": "Set or show active plan",
        "hook_files_missing": "hook files: missing {paths}",
        "hook_files_ok": "hook files: ok",
        "hooks_json": "hooks.json: {status}",
        "language_unsupported": "language: warning unsupported PWF_LANG={lang}",
        "legacy_plan_label": "legacy plan",
        "missing_session_id": "session id: unavailable; set PWF_SESSION_ID or run from a hook payload",
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
        "plan_source": "plan source: {source}",
        "session_attached_count": "attached sessions: {count}",
        "session_binding": "session binding: {key} -> {plan_id}",
        "session_binding_cleared": "session binding cleared: {key}",
        "session_binding_missing": "session binding: unavailable (no session_id)",
        "session_binding_none": "session binding: none for current session",
        "session_binding_required": "session binding required: {value}",
        "session_binding_set": "session binding set: {key} -> {plan_id}",
        "session_binding_auto_unavailable": (
            "session id: unavailable; created workspace active plan without session binding"
        ),
        "session_binding_required_for_no_workspace": (
            "session binding is required when --no-workspace-active is used"
        ),
        "session_binding_unavailable_no_workspace": (
            "session id: unavailable; cannot create a task with --no-workspace-active unless session binding is enabled"
        ),
        "session_released": "session binding released: {key}",
        "session_dir_ignored": "session mode: sessions directory ignored unless PWF_SESSION_MODE=strict",
        "session_mode": "session mode: {mode}",
        "session_mode_unsupported": "session mode: warning unsupported PWF_SESSION_MODE={mode}",
        "task_lease": "task lease: owner={owner} status={status} shared={shared}",
        "task_lease_conflict": "task is owned by another session: owner={owner} status={status} shared={shared}; rerun with --force-claim if you mean to take ownership.",
        "task_lease_error": "task lease error: {message}",
        "task_lease_released": "task lease released: {key} -> {plan_id}",
        "workspace_active_plan": "workspace active plan: {plan_id}",
        "workspace_active_plan_missing": "workspace active plan: missing",
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
        "compact_keep_source": "保留记录数: {count}（来源: {source}, profile={profile}）",
        "compact_kept": "保留最近 auto records: {count}",
        "compact_no_plan": "未找到当前计划。请先创建或切换计划，再压缩 progress。",
        "compact_not_needed": "progress 暂不需要压缩",
        "compact_active": "active progress: {path}",
        "compact_archive_custom_unsupported": "archive paths are generated automatically; --archive is no longer supported",
        "compact_total": "auto records: {count}",
        "compact_would_archive": "将归档 auto records: {count}",
        "compact_would_keep": "将保留最近 auto records: {count}",
        "compacted": "已轮转 progress records",
        "context_cleared": "context settings cleared: {key}",
        "context_invalid_notice": "unsupported context notice mode: {notice}",
        "context_invalid_profile": "unsupported context profile: {profile}",
        "context_missing_session": "session id: 不可用；context 命令只影响当前会话",
        "context_notice_set": "context notice set: {notice}",
        "context_profile_set": "context profile set: {profile}",
        "context_paused": "已暂停当前会话的上下文注入；PostToolUse 的 progress 记录仍然继续",
        "context_pause_already_paused": "当前会话的上下文注入已暂停，无需重复暂停",
        "context_resumed": "已恢复当前会话的上下文注入",
        "context_resume_not_paused": "当前会话的上下文注入未暂停，无需恢复",
        "context_paused_status": "context paused: {paused}",
        "created": "已创建{label}: {plan_id}",
        "current_phase": "当前阶段: {phase}",
        "effective_plan": "effective plan: {plan_id}",
        "findings_path": "findings: {path}",
        "help_attest": "锁定、查看或清除计划 attestation",
        "help_capture": "将外部上下文追加到 findings.md",
        "help_compact": "将旧 progress auto records 轮转到 append-only active/archive segments",
        "help_context": "管理当前会话的 context profile",
        "help_doctor": "诊断 hooks、当前计划和 attestation",
        "help_force": "覆盖已有 planning 文件",
        "help_init": "创建新的 planning 会话",
        "help_legacy": "创建根目录级 planning 文件",
        "help_bind_session": "将新建计划绑定到当前会话",
        "help_no_bind_session": "不要将新建计划绑定到当前会话",
        "help_no_workspace_active": "不要更新 .planning/.active_plan",
        "help_root": "要检查的项目根目录",
        "help_status": "显示当前计划状态",
        "help_switch": "设置或显示当前计划",
        "hook_files_missing": "hook 文件: 缺失 {paths}",
        "hook_files_ok": "hook 文件: ok",
        "hooks_json": "hooks.json: {status}",
        "language_unsupported": "language: warning unsupported PWF_LANG={lang}",
        "legacy_plan_label": "legacy plan",
        "missing_session_id": "session id: 不可用；请设置 PWF_SESSION_ID 或从 hook payload 运行",
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
        "plan_source": "plan source: {source}",
        "session_attached_count": "attached sessions: {count}",
        "session_binding": "session binding: {key} -> {plan_id}",
        "session_binding_cleared": "session binding cleared: {key}",
        "session_binding_missing": "session binding: unavailable (no session_id)",
        "session_binding_none": "session binding: none for current session",
        "session_binding_required": "session binding required: {value}",
        "session_binding_set": "session binding set: {key} -> {plan_id}",
        "session_binding_auto_unavailable": "session id: 不可用；已创建 workspace active plan，但未绑定会话",
        "session_binding_required_for_no_workspace": "使用 --no-workspace-active 时必须启用 session binding",
        "session_binding_unavailable_no_workspace": (
            "session id: 不可用；没有 session binding 时不能创建 --no-workspace-active 任务"
        ),
        "session_released": "session binding released: {key}",
        "session_dir_ignored": "session mode: sessions directory ignored unless PWF_SESSION_MODE=strict",
        "session_mode": "session mode: {mode}",
        "session_mode_unsupported": "session mode: warning unsupported PWF_SESSION_MODE={mode}",
        "task_lease": "task lease: owner={owner} status={status} shared={shared}",
        "task_lease_conflict": "task is owned by another session: owner={owner} status={status} shared={shared}; rerun with --force-claim if you mean to take ownership.",
        "task_lease_error": "task lease error: {message}",
        "task_lease_released": "task lease released: {key} -> {plan_id}",
        "workspace_active_plan": "workspace active plan: {plan_id}",
        "workspace_active_plan_missing": "workspace active plan: missing",
    },
}


def _message(message_key: str, **values: object) -> str:
    text = CLI_MESSAGES[planning_state.current_lang()][message_key]
    return text.format(**values) if values else text


def _help(key: str) -> str:
    return _message(f"help_{key}")


def _unsupported_language_warning() -> str:
    lang = os.environ.get("PWF_LANG", "").strip()
    if lang and lang not in planning_state.SUPPORTED_LANGS:
        return CLI_MESSAGES["en"]["language_unsupported"].format(lang=planning_state.safe_env_value(lang))
    return ""


def _unsupported_session_mode_warning() -> str:
    mode = os.environ.get("PWF_SESSION_MODE", "").strip()
    if mode and mode.lower() not in codex_hook_adapter.SESSION_MODES:
        return _message("session_mode_unsupported", mode=planning_state.safe_env_value(mode))
    return ""


def _current_session_id() -> str | None:
    for name in ("PWF_SESSION_ID", "CODEX_THREAD_ID"):
        value = os.environ.get(name, "").strip()
        if value:
            return value
    return None


def _binding_payload(session_id: str, plan_id: str, source: str) -> dict[str, object]:
    now = datetime.now(timezone.utc).replace(microsecond=0).isoformat().replace("+00:00", "Z")
    return {
        "version": 1,
        "session_id": session_id,
        "plan_id": plan_id,
        "created_at": now,
        "updated_at": now,
        "source": source,
    }


def _write_session_binding(root: Path, session_id: str, plan_id: str, source: str) -> str:
    key = planning_state.session_key(session_id)
    binding_dir = root / ".planning" / "session-bindings"
    binding_dir.mkdir(parents=True, exist_ok=True)
    target = binding_dir / f"{key}.json"
    tmp = binding_dir / f"{key}.json.tmp"
    tmp.write_text(
        json.dumps(_binding_payload(session_id, plan_id, source), ensure_ascii=True, indent=2),
        encoding="utf-8",
        newline="\n",
    )
    tmp.replace(target)
    return key


def _clear_session_binding(root: Path, session_id: str) -> str:
    key = planning_state.session_key(session_id)
    binding = root / ".planning" / "session-bindings" / f"{key}.json"
    if binding.exists():
        binding.unlink()
    return key


def _read_session_binding_plan_id(root: Path, session_id: str) -> str | None:
    key = planning_state.session_key(session_id)
    binding = root / ".planning" / "session-bindings" / f"{key}.json"
    if not binding.is_file():
        return None
    try:
        payload = json.loads(binding.read_text(encoding="utf-8", errors="replace"))
    except (OSError, json.JSONDecodeError):
        return None
    plan_id = payload.get("plan_id") if isinstance(payload, dict) else None
    return plan_id if isinstance(plan_id, str) and planning_state.valid_plan_id(plan_id) else None


def _session_context_path(root: Path, session_id: str) -> Path:
    return planning_state.session_context_path(root, session_id)


def _read_session_context_payload(root: Path, session_id: str) -> dict[str, object]:
    path = _session_context_path(root, session_id)
    if not path.is_file():
        return {}
    try:
        payload = json.loads(path.read_text(encoding="utf-8", errors="replace"))
    except (OSError, json.JSONDecodeError):
        return {}
    return payload if isinstance(payload, dict) else {}


def _read_existing_paused(payload: dict[str, object]) -> bool:
    value = payload.get("paused")
    return value if isinstance(value, bool) else False


def _valid_context_profiles_text() -> str:
    return ", ".join(sorted(planning_state.SESSION_CONTEXT_PROFILES))


def _valid_context_notice_modes_text() -> str:
    return ", ".join(sorted(planning_state.CONTEXT_NOTICE_MODES))


def _normalized_existing_context_value(
    payload: dict[str, object],
    key: str,
    allowed: set[str],
    default: str,
) -> str:
    value = payload.get(key)
    return value if isinstance(value, str) and value in allowed else default


def _write_session_context(
    root: Path,
    session_id: str,
    *,
    profile: str | None = None,
    notice: str | None = None,
    paused: bool | None = None,
) -> str:
    key = planning_state.session_key(session_id)
    context_dir = root / ".planning" / "session-context"
    context_dir.mkdir(parents=True, exist_ok=True)
    existing = _read_session_context_payload(root, session_id)
    now = datetime.now(timezone.utc).replace(microsecond=0).isoformat().replace("+00:00", "Z")
    existing_profile = _normalized_existing_context_value(
        existing,
        "profile",
        planning_state.SESSION_CONTEXT_PROFILES,
        "default",
    )
    existing_notice = _normalized_existing_context_value(
        existing,
        "notice",
        planning_state.CONTEXT_NOTICE_MODES,
        "auto",
    )
    existing_paused = _read_existing_paused(existing)
    payload = {
        "version": 1,
        "session_id": session_id,
        "profile": profile if profile is not None else existing_profile,
        "notice": notice if notice is not None else existing_notice,
        "paused": paused if paused is not None else existing_paused,
        "created_at": existing.get("created_at", now),
        "updated_at": now,
        "source": "plan.py context",
    }
    target = context_dir / f"{key}.json"
    tmp = context_dir / f"{key}.json.tmp"
    tmp.write_text(json.dumps(payload, ensure_ascii=True, indent=2), encoding="utf-8", newline="\n")
    tmp.replace(target)
    return key


def _clear_session_context(root: Path, session_id: str) -> str:
    key = planning_state.session_key(session_id)
    path = _session_context_path(root, session_id)
    if path.exists():
        path.unlink()
    return key


def _workspace_active_plan_id(root: Path) -> str | None:
    active_file = root / ".planning" / ".active_plan"
    if not active_file.is_file():
        return None
    value = active_file.read_text(encoding="utf-8", errors="replace").strip()
    return value or None


def _attached_session_count(root: Path) -> int:
    sessions_dir = root / ".planning" / "sessions"
    if not sessions_dir.is_dir():
        return 0
    return len(list(sessions_dir.glob("*.attached")))


def _session_status_lines(root: Path) -> list[str]:
    mode = codex_hook_adapter.session_mode(root)
    lines = [_message("session_mode", mode=mode)]
    warning = _unsupported_session_mode_warning()
    if warning:
        lines.append(warning)
    sessions_dir = root / ".planning" / "sessions"
    if mode == "strict":
        lines.append(_message("session_attached_count", count=_attached_session_count(root)))
        required = "yes" if codex_hook_adapter.strict_requires_binding(root) else "no"
        lines.append(_message("session_binding_required", value=required))
    elif sessions_dir.is_dir():
        lines.append(_message("session_dir_ignored"))
    return lines


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


def _task_lease_line(root: Path, plan_id: str | None, session_id: str | None = None) -> str | None:
    if not plan_id or plan_id == "legacy":
        return None
    lease = planning_state.read_task_lease(root, plan_id)
    if lease is None:
        return "task lease: none"
    status = planning_state.task_lease_status(root, lease)
    shared = str(lease.shared).lower()
    current_key = planning_state.session_key(session_id) if session_id else None
    if current_key and lease.owner_session_key != current_key and not lease.shared and status != "released":
        return f"task lease: conflict owner={lease.owner_session_key} status={status} shared={shared}"
    return f"task lease: owner={lease.owner_session_key} status={status} shared={shared}"


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


def _paths_for_plan_dir(plan_dir: Path) -> planning_state.PlanningPaths:
    return planning_state.PlanningPaths(
        root=plan_dir,
        task_plan=plan_dir / "task_plan.md",
        progress=plan_dir / "progress.md",
        findings=plan_dir / "findings.md",
    )


def _iter_plan_ids(root: Path) -> list[str]:
    planning_root = root / ".planning"
    if not planning_root.is_dir():
        return []
    plan_ids = []
    for child in planning_root.iterdir():
        if not child.is_dir():
            continue
        if not planning_state.valid_plan_id(child.name):
            continue
        if (child / "task_plan.md").is_file():
            plan_ids.append(child.name)
    return sorted(plan_ids)


def _task_title(plan_dir: Path) -> str:
    task_plan = plan_dir / "task_plan.md"
    if not task_plan.is_file():
        return ""
    for line in task_plan.read_text(encoding="utf-8", errors="replace").splitlines():
        stripped = line.strip()
        if stripped.startswith("# Task Plan:"):
            return stripped.removeprefix("# Task Plan:").strip()
        if stripped.startswith("# "):
            return stripped[2:].strip()
    return ""


def _phase_counts_for_plan_dir(plan_dir: Path) -> tuple[int, int, int, int] | None:
    task_plan = plan_dir / "task_plan.md"
    if not task_plan.is_file():
        return None
    text = task_plan.read_text(encoding="utf-8", errors="replace")
    total = len(re.findall(r"^### Phase", text, flags=re.MULTILINE))
    complete = text.count("**Status:** complete")
    in_progress = text.count("**Status:** in_progress")
    pending = text.count("**Status:** pending")

    if complete == 0 and in_progress == 0 and pending == 0:
        complete = text.count("[complete]")
        in_progress = text.count("[in_progress]")
        pending = text.count("[pending]")

    return total, complete, in_progress, pending


def _task_short_id(plan_id: str, length: int = 6) -> str:
    return hashlib.sha256(plan_id.encode("utf-8")).hexdigest()[:length]


def _expand_colliding_short_ids(summaries: list[TaskSummary]) -> list[TaskSummary]:
    lengths = {summary.plan_id: 6 for summary in summaries}
    for _attempt in range(4):
        short_ids = {
            summary.plan_id: _task_short_id(summary.plan_id, lengths[summary.plan_id])
            for summary in summaries
        }
        groups: dict[str, list[str]] = {}
        for plan_id, short_id in short_ids.items():
            groups.setdefault(short_id, []).append(plan_id)
        colliding = [plan_ids for plan_ids in groups.values() if len(plan_ids) > 1]
        if not colliding:
            break
        changed = False
        for plan_ids in colliding:
            for plan_id in plan_ids:
                if lengths[plan_id] < 12:
                    lengths[plan_id] += 2
                    changed = True
        if not changed:
            break
    return [
        replace(summary, short_id=_task_short_id(summary.plan_id, lengths[summary.plan_id]))
        for summary in summaries
    ]


def _task_summaries(root: Path, include_all: bool = False, session_id: str | None = None) -> list[TaskSummary]:
    current_key = planning_state.session_key(session_id) if session_id else None
    bound_plan = _read_session_binding_plan_id(root, session_id) if session_id else None
    workspace_plan = _workspace_active_plan_id(root)
    summaries: list[TaskSummary] = []
    for plan_id in _iter_plan_ids(root):
        plan_dir = root / ".planning" / plan_id
        lease = planning_state.read_task_lease(root, plan_id)
        status = planning_state.task_lease_status(root, lease) if lease else "none"
        owner = lease.owner_session_key if lease else None
        shared = bool(lease.shared) if lease else False
        session_bound = plan_id == bound_plan
        workspace_active = plan_id == workspace_plan
        visible = False
        reason = "unowned"

        if session_bound:
            visible = True
            reason = "session-bound"
        elif lease is None:
            visible = False
            reason = "unowned"
        elif current_key and owner == current_key:
            visible = True
            reason = "owned-by-current-session"
        elif shared:
            visible = session_bound
            reason = "shared" if visible else "shared-not-joined"
        elif status == "released":
            visible = session_bound
            reason = "released" if visible else "released-not-bound"
        else:
            reason = "stale-owner" if status == "stale" else "owned-by-other-session"

        summary = TaskSummary(
            plan_id=plan_id,
            path=plan_dir,
            short_id=_task_short_id(plan_id),
            title=_task_title(plan_dir),
            current_phase=_current_phase(_paths_for_plan_dir(plan_dir)),
            phase_counts=_phase_counts_for_plan_dir(plan_dir),
            workspace_active=workspace_active,
            session_bound=session_bound,
            lease_owner=owner,
            lease_status=status,
            shared=shared,
            visible=visible,
            reason=reason,
        )
        if include_all or summary.visible:
            summaries.append(summary)
    return _expand_colliding_short_ids(summaries)


def _task_summary_to_json(summary: TaskSummary) -> dict[str, object]:
    return {
        "plan_id": summary.plan_id,
        "path": str(summary.path),
        "short_id": summary.short_id,
        "title": summary.title,
        "current_phase": summary.current_phase,
        "phase_counts": summary.phase_counts,
        "workspace_active": summary.workspace_active,
        "session_bound": summary.session_bound,
        "lease_owner": summary.lease_owner,
        "lease_status": summary.lease_status,
        "shared": summary.shared,
        "visible": summary.visible,
        "reason": summary.reason,
    }


def _resolve_task_selector(
    root: Path,
    selector: str,
    *,
    include_all: bool,
    session_id: str | None,
) -> tuple[TaskSummary | None, str | None]:
    summaries = _task_summaries(root, include_all=include_all, session_id=session_id)
    exact = [summary for summary in summaries if summary.plan_id == selector]
    if len(exact) == 1:
        return exact[0], None
    matches = [summary for summary in summaries if summary.short_id.startswith(selector)]
    if len(matches) == 1:
        return matches[0], None
    if not matches:
        all_matches = _task_summaries(root, include_all=True, session_id=session_id)
        hidden = [
            summary
            for summary in all_matches
            if summary.plan_id == selector or summary.short_id.startswith(selector)
        ]
        if hidden:
            return (
                None,
                "task is not visible to current session; run /pwf-tasks or use --claim explicitly.",
            )
        return None, "task selector not found; run /pwf-tasks to list visible tasks."
    candidates = ", ".join(f"{summary.short_id}={summary.plan_id}" for summary in matches)
    return None, f"task selector is ambiguous: {candidates}"


def _compact_threshold() -> int:
    raw = os.environ.get("PWF_COMPACT_THRESHOLD", "").strip()
    if not raw:
        return DEFAULT_COMPACT_THRESHOLD
    try:
        value = int(raw)
    except ValueError:
        return DEFAULT_COMPACT_THRESHOLD
    return value if value >= 1 else DEFAULT_COMPACT_THRESHOLD


# Default keep-records per context profile. The active segment after rollover
# should roughly match what the profile will actually inject, so expanded/deep
# profiles keep more recent records available without re-reading the archive.
PROFILE_DEFAULT_KEEP_RECORDS = {
    "lean": 10,
    "default": 30,
    "expanded": 60,
    "deep": 100,
}


def _default_keep_records_for_profile(profile: str) -> int:
    return PROFILE_DEFAULT_KEEP_RECORDS.get(profile, PROFILE_DEFAULT_KEEP_RECORDS["default"])


def _env_keep_records() -> int | None:
    raw = os.environ.get("PWF_COMPACT_KEEP_RECORDS", "").strip()
    if not raw:
        return None
    try:
        value = int(raw)
    except ValueError:
        return None
    return value if value >= 1 else None


def _progress_record_count(paths: planning_state.PlanningPaths | None) -> int:
    if paths is None:
        return 0
    return progress_lifecycle.count_auto_records(planning_state.current_progress_path(paths))


def _progress_status_line(paths: planning_state.PlanningPaths | None) -> str:
    count = _progress_record_count(paths)
    suffix = _message("progress_compact_suffix") if count >= _compact_threshold() else ""
    return _message("progress", count=count, suffix=suffix)


def _progress_doctor_warning(paths: planning_state.PlanningPaths | None) -> str:
    count = _progress_record_count(paths)
    if count < _compact_threshold():
        return ""
    return _message("progress_warning", count=count)


def _relative_to_plan_root(paths: planning_state.PlanningPaths, path: Path) -> str:
    try:
        return path.relative_to(paths.root).as_posix()
    except ValueError:
        return str(path)


def _progress_storage_status(report: progress_lifecycle.ProgressDoctorReport) -> str:
    if report.has_errors:
        return "error"
    if report.has_warnings:
        return "warning"
    if report.issues:
        return "info"
    return "ok"


def _progress_storage_summary_lines(
    paths: planning_state.PlanningPaths,
    report: progress_lifecycle.ProgressDoctorReport,
    *,
    verbose: bool = False,
) -> list[str]:
    status = _progress_storage_status(report)
    event_word = "event" if report.rollover_events == 1 else "events"
    lines = [
        f"progress storage: {status}",
        f"progress active: {_relative_to_plan_root(paths, report.active_path)}",
        f"progress index: {report.rollover_events} rollover {event_word}",
    ]
    for issue in report.issues:
        if issue.severity == "info" and not verbose:
            continue
        lines.append(f"[{issue.severity}] {issue.code}: {issue.message}")
        lines.append(f"  path: {issue.path}")
        if verbose:
            lines.append(f"  effect: {issue.effect}")
            lines.append(f"  action: {issue.action}")
    lines.append("No automatic repair was attempted.")
    return lines


def _progress_storage_json(
    paths: planning_state.PlanningPaths,
    report: progress_lifecycle.ProgressDoctorReport,
) -> dict[str, object]:
    return {
        "status": _progress_storage_status(report),
        "active_path": _relative_to_plan_root(paths, report.active_path),
        "index_path": _relative_to_plan_root(paths, report.index_path),
        "index_exists": report.index_exists,
        "rollover_events": report.rollover_events,
        "referenced_paths": list(report.referenced_paths),
        "orphan_paths": list(report.orphan_paths),
        "issues": [
            {
                "severity": issue.severity,
                "code": issue.code,
                "path": issue.path,
                "message": issue.message,
                "effect": issue.effect,
                "action": issue.action,
            }
            for issue in report.issues
        ],
    }


def _findings_context_enabled() -> tuple[bool, str | None]:
    return planning_state.env_bool("PWF_INCLUDE_FINDINGS", default=False)


def _context_progress_text(limits: planning_state.ContextLimits) -> str:
    if limits.progress_recent_records > 0:
        return f"{limits.progress_recent_records} records"
    return f"tail {limits.progress_tail_lines} lines"


def _context_findings_text(limits: planning_state.ContextLimits) -> str:
    enabled, _warning = _findings_context_enabled()
    if not enabled:
        return "off"
    return f"tail {limits.findings_tail_lines}"


def _context_status_lines(root: Path, session_id: str | None) -> list[str]:
    limits = planning_state.context_limits(root=root, session_id=session_id)
    source = planning_state.context_settings_source(root=root, session_id=session_id)
    paused = planning_state.is_session_paused(root, session_id)
    lines = [
        (
            f"context: profile={limits.profile}, "
            f"plan=head {limits.plan_head_lines} tail {limits.plan_tail_lines}, "
            f"progress={_context_progress_text(limits)}, "
            f"findings={_context_findings_text(limits)}, "
            f"max={limits.context_max_chars} chars"
        ),
        f"context source: {source.profile_source}",
        f"context notice: {source.notice}",
        _message("context_paused_status", paused=str(paused).lower()),
    ]
    if source.session_profile_overridden and source.session_profile:
        lines.append(f"context session profile: {source.session_profile} overridden")
    lines.extend(source.warnings)
    return lines


def _context_source_lines(root: Path, session_id: str | None) -> list[str]:
    limits = planning_state.context_limits(root=root, session_id=session_id)
    source = planning_state.context_settings_source(root=root, session_id=session_id)
    paused = planning_state.is_session_paused(root, session_id)
    progress_mode = (
        f"record-aware {limits.progress_recent_records} records"
        if limits.progress_recent_records > 0
        else f"line tail {limits.progress_tail_lines}"
    )
    lines = [
        "session context:",
        f"  profile: {limits.profile}",
        f"  source: {source.profile_source}",
    ]
    if source.session_profile_overridden and source.session_profile:
        lines.append(f"  session profile: {source.session_profile}, currently overridden")
    elif source.session_profile:
        lines.append(f"  session profile: {source.session_profile}")
    else:
        lines.append("  session profile: none")
    lines.extend(
        [
            f"  notice: {source.notice}",
            f"  notice source: {source.notice_source}",
            f"  paused: {str(paused).lower()}",
            f"  progress mode: {progress_mode}",
            f"  plan: head {limits.plan_head_lines} tail {limits.plan_tail_lines}",
            f"  findings: {_context_findings_text(limits)}",
            f"  max: {limits.context_max_chars} chars",
        ]
    )
    lines.extend(source.warnings)
    return lines


def _context_doctor_lines(root: Path, session_id: str | None) -> list[str]:
    limits = planning_state.context_limits(root=root, session_id=session_id)
    source = planning_state.context_settings_source(root=root, session_id=session_id)
    paused = planning_state.is_session_paused(root, session_id)
    findings_enabled, findings_warning = _findings_context_enabled()
    lines = [
        f"context profile: {limits.profile}",
        f"context profile source: {source.profile_source}",
        f"context notice: {source.notice}",
        f"context notice source: {source.notice_source}",
        _message("context_paused_status", paused=str(paused).lower()),
        (
            f"context findings: on tail {limits.findings_tail_lines}"
            if findings_enabled
            else "context findings: off"
        ),
        (
            f"context progress mode: record-aware {limits.progress_recent_records} records"
            if limits.progress_recent_records > 0
            else f"context progress mode: line tail {limits.progress_tail_lines}"
        ),
    ]
    if limits.profile == "custom" and not any(name in os.environ for name in planning_state.NUMERIC_OVERRIDE_FIELDS):
        lines.append("context custom: no overrides; using default limits")
    if source.session_profile_overridden and source.session_profile:
        lines.append(f"context session profile: {source.session_profile} overridden")
    if source.session_notice_overridden and source.session_notice:
        lines.append(f"context session notice: {source.session_notice} overridden")
    lines.extend(limits.warnings)
    if findings_warning:
        lines.append(findings_warning)
    return lines


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


def doctor(root: Path, *, verbose: bool = False, as_json: bool = False, strict: bool = False) -> int:
    lines: list[str] = []
    ok = True
    progress_report = None
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

    lines.extend(_session_status_lines(root))

    session_id = _current_session_id()
    resolution = planning_state.resolve_planning_context(root, session_id=session_id)
    paths = resolution.paths if resolution is not None else None
    if paths is None:
        lines.append(_message("active_plan_missing"))
        ok = False
    else:
        lines.append(_message("active_plan_ok", path=paths.root))

    task_line = _task_lease_line(root, resolution.plan_id if resolution is not None else None, session_id)
    if task_line:
        lines.append(task_line)
        if task_line.startswith("task lease: conflict"):
            lines.append(
                "[warn] workspace active plan is owned by another session; bind this session "
                "with plan.py switch <plan-id> --session or create a new task with "
                "plan.py init \"Task Name\" --bind-session"
            )

    planning_line, planning_ok = _planning_files_status(paths)
    lines.append(planning_line)
    ok = ok and planning_ok

    attestation_line, attestation_ok = _attestation_status(root, paths)
    lines.append(attestation_line)
    ok = ok and attestation_ok

    if paths is not None:
        progress_report = progress_lifecycle.doctor_progress_storage(paths.progress)
        if progress_report.has_errors or (strict and progress_report.has_warnings):
            ok = False
        lines.extend(_progress_storage_summary_lines(paths, progress_report, verbose=verbose))

    lines.extend(_context_doctor_lines(root, session_id))

    warning = _progress_doctor_warning(paths)
    if warning:
        lines.append(warning)

    if as_json:
        payload: dict[str, object] = {
            "ok": ok,
            "strict": strict,
            "checks": lines,
        }
        if paths is not None and progress_report is not None:
            payload["progress_storage"] = _progress_storage_json(paths, progress_report)
        print(json.dumps(payload, ensure_ascii=True, indent=2))
        return 0 if ok else 1

    print("\n".join(lines))
    return 0 if ok else 1


def status(root: Path) -> int:
    session_id = _current_session_id()
    resolution = planning_state.resolve_planning_context(root, session_id=session_id)
    paths = resolution.paths if resolution is not None else None
    workspace_plan = _workspace_active_plan_id(root)
    print(
        _message("workspace_active_plan", plan_id=workspace_plan)
        if workspace_plan
        else _message("workspace_active_plan_missing")
    )
    if session_id:
        key = planning_state.session_key(session_id)
        if resolution is not None and resolution.source == "session":
            print(_message("session_binding", key=key, plan_id=resolution.plan_id))
        else:
            print(_message("session_binding_none"))
    else:
        print(_message("session_binding_missing"))
    if resolution is not None:
        print(_message("effective_plan", plan_id=resolution.plan_id))
        print(_message("plan_source", source=resolution.source))
    if session_id:
        key = planning_state.session_key(session_id)
        lease_path = planning_state.session_lease_path(root, session_id)
        print(f"session lease: {'active ' + key if lease_path.is_file() else 'missing ' + key}")
    else:
        print("session lease: unavailable (no session_id)")
    task_line = _task_lease_line(root, resolution.plan_id if resolution is not None else workspace_plan, session_id)
    if task_line:
        print(task_line)

    if paths is None:
        print(_message("active_plan_missing"))
        print(_message("planning_files_missing"))
        return 1

    print(_message("active_plan", plan_id=_plan_id(root, paths)))
    print(_message("path", path=paths.root))

    phase = _current_phase(paths)
    if phase:
        print(_message("current_phase", phase=phase))

    counts = planning_state.phase_counts(root, session_id=session_id)
    if counts is not None:
        total, complete, _in_progress, _pending = counts
        print(_message("phases", complete=complete, total=total))

    planning_line, planning_ok = _planning_files_status(paths)
    print(planning_line)

    attestation_line, attestation_ok = _attestation_status(root, paths)
    print(attestation_line)
    print(_progress_status_line(paths))
    for line in _context_status_lines(root, session_id):
        print(line)
    return 0 if planning_ok and attestation_ok else 1


def context(root: Path, action: str, value: str | None = None) -> int:
    session_id = _current_session_id()
    if action == "status":
        print("\n".join(_context_source_lines(root, session_id)))
        return 0

    if not session_id:
        print(_message("context_missing_session"))
        return 1

    if action == "set":
        profile = (value or "").lower()
        if profile not in planning_state.SESSION_CONTEXT_PROFILES:
            print(
                f"{_message('context_invalid_profile', profile=profile)}; "
                f"valid profiles: {_valid_context_profiles_text()}"
            )
            return 1
        _write_session_context(root, session_id, profile=profile)
        print(_message("context_profile_set", profile=profile))
        print("\n".join(_context_source_lines(root, session_id)))
        return 0

    if action == "notice":
        notice = (value or "").lower()
        if notice not in planning_state.CONTEXT_NOTICE_MODES:
            print(
                f"{_message('context_invalid_notice', notice=notice)}; "
                f"valid notice modes: {_valid_context_notice_modes_text()}"
            )
            return 1
        _write_session_context(root, session_id, notice=notice)
        print(_message("context_notice_set", notice=notice))
        print("\n".join(_context_source_lines(root, session_id)))
        return 0

    if action == "clear":
        key = _clear_session_context(root, session_id)
        print(_message("context_cleared", key=key))
        return 0

    if action == "pause":
        if planning_state.is_session_paused(root, session_id):
            print(_message("context_pause_already_paused"))
            return 0
        _write_session_context(root, session_id, paused=True)
        print(_message("context_paused"))
        print("\n".join(_context_source_lines(root, session_id)))
        return 0

    if action == "resume":
        if not planning_state.is_session_paused(root, session_id):
            print(_message("context_resume_not_paused"))
            return 0
        _write_session_context(root, session_id, paused=False)
        print(_message("context_resumed"))
        print("\n".join(_context_source_lines(root, session_id)))
        return 0

    raise ValueError(f"unsupported context action: {action}")


def tasks(root: Path, include_all: bool = False, as_json: bool = False) -> int:
    session_id = _current_session_id()
    summaries = _task_summaries(root, include_all=include_all, session_id=session_id)
    if as_json:
        print(json.dumps([_task_summary_to_json(summary) for summary in summaries], ensure_ascii=True, indent=2))
        return 0
    if not summaries:
        print("tasks: none visible for current session")
        return 0
    for summary in summaries:
        markers = []
        if summary.session_bound:
            markers.append("session-bound")
        if summary.workspace_active:
            markers.append("workspace-active")
        markers.append(summary.reason)
        if summary.lease_owner:
            markers.append(f"owner={summary.lease_owner}")
        marker_text = ", ".join(markers)
        title = f" {summary.title}" if summary.title else ""
        print(f"{summary.short_id}  {summary.plan_id}{title} [{marker_text}]")
    return 0


def use(root: Path, selector: str, *, claim: bool = False) -> int:
    session_id = _current_session_id()
    if not session_id:
        print(_message("missing_session_id"))
        return 1
    summary, error = _resolve_task_selector(
        root,
        selector,
        include_all=claim,
        session_id=session_id,
    )
    if summary is None:
        print(error or "task selector not found")
        return 1
    return switch(root, summary.plan_id, session=True, force_claim=claim)


def init(
    root: Path,
    name: str,
    legacy: bool = False,
    force: bool = False,
    bind_session: bool | None = None,
    workspace_active: bool = True,
) -> int:
    session_id = _current_session_id()
    auto_bind = bind_session is None and not legacy and session_id is not None
    effective_bind_session = bind_session is True or auto_bind

    if legacy and effective_bind_session:
        print(LEGACY_BIND_SESSION_UNSUPPORTED)
        return 1

    if not legacy and not workspace_active and not effective_bind_session:
        if not session_id:
            print(_message("session_binding_unavailable_no_workspace"))
        else:
            print(_message("session_binding_required_for_no_workspace"))
        return 1

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

    lease = None
    if effective_bind_session:
        if not session_id:
            print(_message("missing_session_id"))
            return 1
        lease, conflict = planning_state.claim_task_lease_for_rewrite(
            root,
            plan_id,
            session_id,
            source="plan.py init --bind-session",
        )
        if lease is None:
            print(_message("task_lease_error", message=conflict))
            return 1
        if conflict:
            status = planning_state.task_lease_status(root, lease)
            print(
                _message(
                    "task_lease_conflict",
                    owner=lease.owner_session_key,
                    status=status,
                    shared=str(lease.shared).lower(),
                )
            )
            return 1

    target.mkdir(parents=True, exist_ok=True)
    (target / "task_plan.md").write_text(_task_plan_template(name), encoding="utf-8", newline="\n")
    (target / "progress.md").write_text(_progress_template(), encoding="utf-8", newline="\n")
    (target / "findings.md").write_text(_findings_template(), encoding="utf-8", newline="\n")

    if not legacy and workspace_active:
        active_file = root / ".planning" / ".active_plan"
        active_file.parent.mkdir(parents=True, exist_ok=True)
        active_file.write_text(plan_id, encoding="utf-8")

    print(_message("created", label=label, plan_id=plan_id))
    print(_message("path", path=target))
    if effective_bind_session:
        assert session_id is not None
        assert lease is not None
        key = _write_session_binding(root, session_id, plan_id, "plan.py init --bind-session")
        print(_message("session_binding_set", key=key, plan_id=plan_id))
        status = planning_state.task_lease_status(root, lease)
        print(
            _message(
                "task_lease",
                owner=lease.owner_session_key,
                status=status,
                shared=str(lease.shared).lower(),
            )
        )
    elif bind_session is None and not legacy and session_id is None:
        print(_message("session_binding_auto_unavailable"))
    return 0


def switch(
    root: Path,
    plan_id: str | None,
    session: bool = False,
    workspace: bool = False,
    clear_session: bool = False,
    release_session: bool = False,
    force_claim: bool = False,
) -> int:
    plan_root = root / ".planning"
    active_file = plan_root / ".active_plan"

    if release_session:
        session_id = _current_session_id()
        if not session_id:
            print(_message("missing_session_id"))
            return 1
        key = planning_state.session_key(session_id)
        bound_plan_id = _read_session_binding_plan_id(root, session_id)
        lease = None
        if bound_plan_id:
            lease, conflict = planning_state.release_task_lease_for_session(root, bound_plan_id, session_id)
            if lease is None and conflict:
                print(_message("task_lease_error", message=conflict))
                return 1
        _clear_session_binding(root, session_id)
        print(_message("session_released", key=key))
        if bound_plan_id:
            if lease and lease.owner_session_key == key:
                print(_message("task_lease_released", key=key, plan_id=bound_plan_id))
        return 0

    if clear_session:
        session_id = _current_session_id()
        if not session_id:
            print(_message("missing_session_id"))
            return 1
        key = _clear_session_binding(root, session_id)
        print(_message("session_binding_cleared", key=key))
        return 0

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

    if session:
        session_id = _current_session_id()
        if not session_id:
            print(_message("missing_session_id"))
            return 1
        lease, conflict = planning_state.claim_task_lease(
            root,
            plan_id,
            session_id,
            force=force_claim,
            source="plan.py switch --session",
        )
        if lease is None:
            print(_message("task_lease_error", message=conflict))
            return 1
        if conflict:
            status = planning_state.task_lease_status(root, lease)
            print(
                _message(
                    "task_lease_conflict",
                    owner=lease.owner_session_key,
                    status=status,
                    shared=str(lease.shared).lower(),
                )
            )
            return 1
        key = _write_session_binding(root, session_id, plan_id, "plan.py switch --session")
        print(_message("session_binding_set", key=key, plan_id=plan_id))
        status = planning_state.task_lease_status(root, lease)
        print(
            _message(
                "task_lease",
                owner=lease.owner_session_key,
                status=status,
                shared=str(lease.shared).lower(),
            )
        )
        print(_message("path", path=plan_dir))
        return 0

    active_file.parent.mkdir(parents=True, exist_ok=True)
    active_file.write_text(plan_id, encoding="utf-8")
    print(_message("active_plan_set_to", plan_id=plan_id))
    print(_message("path", path=plan_dir))
    return 0


def _sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def attest(root: Path, show: bool = False, clear: bool = False) -> int:
    session_id = _current_session_id()
    paths = planning_state.planning_paths(root, session_id=session_id)
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
    session_id = _current_session_id()
    paths = planning_state.planning_paths(root, session_id=session_id)
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


def compact(
    root: Path,
    keep_records: int | None = None,
    dry_run: bool = False,
    archive: str | None = None,
) -> int:
    session_id = _current_session_id()
    paths = planning_state.planning_paths(root, session_id=session_id)
    if paths is None:
        print(_message("compact_no_plan"))
        return 1
    if archive is not None:
        print(_message("compact_archive_custom_unsupported"))
        return 1

    # keep_records resolution priority: env override > explicit --keep-records
    # flag > profile-derived default. This keeps the active segment size aligned
    # with the current context profile so injected progress stays available
    # without re-reading the archive.
    profile = planning_state.current_context_profile(root=root, session_id=session_id)
    profile_default = _default_keep_records_for_profile(profile)
    env_keep = _env_keep_records()
    if env_keep is not None:
        effective_keep = env_keep
        keep_source = f"env PWF_COMPACT_KEEP_RECORDS"
    elif keep_records is not None:
        effective_keep = keep_records
        keep_source = "--keep-records"
    else:
        effective_keep = profile_default
        keep_source = f"profile={profile}"
    print(_message("compact_keep_source", profile=profile, source=keep_source, count=effective_keep))

    try:
        result = progress_lifecycle.rollover_progress(
            paths.progress,
            plan_id=paths.root.name,
            session_key=planning_state.session_key(session_id) if session_id else "unavailable",
            keep_records=effective_keep,
            dry_run=dry_run,
        )
    except (FileExistsError, ValueError) as exc:
        print(str(exc))
        return 1

    if result.archived_count == 0:
        print(_message("compact_not_needed"))
        print(_message("compact_total", count=result.total_auto_records))
        print(_message("compact_keep", count=effective_keep))
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
    print(_message("compact_active", path=result.active_path))
    return 0


def main(argv: Iterable[str] | None = None) -> int:
    parser = argparse.ArgumentParser(prog="plan.py")
    parser.add_argument("--root", default=".", help=_help("root"))
    subparsers = parser.add_subparsers(dest="command", required=True)
    doctor_parser = subparsers.add_parser("doctor", help=_help("doctor"))
    doctor_parser.add_argument("--verbose", action="store_true")
    doctor_parser.add_argument("--json", action="store_true", dest="as_json")
    doctor_parser.add_argument("--strict", action="store_true")
    subparsers.add_parser("status", help=_help("status"))

    context_parser = subparsers.add_parser("context", help=_help("context"))
    context_subparsers = context_parser.add_subparsers(dest="context_command", required=True)
    context_subparsers.add_parser("status")
    context_set = context_subparsers.add_parser("set")
    context_set.add_argument("profile")
    context_notice = context_subparsers.add_parser("notice")
    context_notice.add_argument("mode")
    context_subparsers.add_parser("clear")
    context_subparsers.add_parser("pause")
    context_subparsers.add_parser("resume")

    init_parser = subparsers.add_parser("init", help=_help("init"))
    init_parser.add_argument("name")
    init_parser.add_argument("--legacy", action="store_true", help=_help("legacy"))
    init_parser.add_argument("--force", action="store_true", help=_help("force"))
    init_bind_group = init_parser.add_mutually_exclusive_group()
    init_bind_group.add_argument(
        "--bind-session",
        dest="bind_session",
        action="store_true",
        help=_help("bind_session"),
    )
    init_bind_group.add_argument(
        "--no-bind-session",
        dest="bind_session",
        action="store_false",
        help=_help("no_bind_session"),
    )
    init_parser.set_defaults(bind_session=None)
    init_parser.add_argument("--no-workspace-active", action="store_true", help=_help("no_workspace_active"))

    switch_parser = subparsers.add_parser("switch", help=_help("switch"))
    switch_parser.add_argument("plan_id", nargs="?")
    switch_group = switch_parser.add_mutually_exclusive_group()
    switch_group.add_argument("--session", action="store_true")
    switch_group.add_argument("--workspace", action="store_true")
    switch_group.add_argument("--clear-session", action="store_true")
    switch_group.add_argument("--release-session", action="store_true")
    switch_parser.add_argument("--force-claim", action="store_true")

    tasks_parser = subparsers.add_parser("tasks", help="List PWF tasks visible to the current session")
    tasks_parser.add_argument("--all", action="store_true")
    tasks_parser.add_argument("--json", action="store_true")

    use_parser = subparsers.add_parser("use", help="Bind current session to a visible PWF task")
    use_parser.add_argument("selector")
    use_parser.add_argument("--claim", action="store_true")

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
    compact_parser.add_argument(
        "--keep-records",
        type=int,
        default=None,
        help="Override the profile-derived default keep count (env PWF_COMPACT_KEEP_RECORDS also works)",
    )
    compact_parser.add_argument("--dry-run", action="store_true")
    compact_parser.add_argument("--archive", default=None, help=argparse.SUPPRESS)

    args = parser.parse_args(list(argv) if argv is not None else None)
    root = Path(args.root).resolve()

    if args.command == "doctor":
        return doctor(root, verbose=args.verbose, as_json=args.as_json, strict=args.strict)
    if args.command == "status":
        return status(root)
    if args.command == "context":
        if args.context_command == "status":
            return context(root, "status")
        if args.context_command == "set":
            return context(root, "set", args.profile)
        if args.context_command == "notice":
            return context(root, "notice", args.mode)
        if args.context_command == "clear":
            return context(root, "clear")
        if args.context_command == "pause":
            return context(root, "pause")
        if args.context_command == "resume":
            return context(root, "resume")
    if args.command == "init":
        return init(
            root,
            args.name,
            legacy=args.legacy,
            force=args.force,
            bind_session=args.bind_session,
            workspace_active=not args.no_workspace_active,
        )
    if args.command == "switch":
        return switch(
            root,
            args.plan_id,
            session=args.session,
            workspace=args.workspace,
            clear_session=args.clear_session,
            release_session=args.release_session,
            force_claim=args.force_claim,
        )
    if args.command == "tasks":
        return tasks(root, include_all=args.all, as_json=args.json)
    if args.command == "use":
        return use(root, args.selector, claim=args.claim)
    if args.command == "attest":
        return attest(root, show=args.show, clear=args.clear)
    if args.command == "capture":
        return capture(root, args.kind, args.source, args.summary, trust=args.trust)
    if args.command == "compact":
        return compact(root, keep_records=args.keep_records, dry_run=args.dry_run, archive=args.archive)
    return 2


if __name__ == "__main__":
    raise SystemExit(main())
