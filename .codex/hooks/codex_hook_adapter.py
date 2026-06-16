#!/usr/bin/env python3
from __future__ import annotations

import json
import os
import sys
from pathlib import Path
from typing import Any


HOOK_DIR = Path(__file__).resolve().parent
if str(HOOK_DIR) not in sys.path:
    sys.path.insert(0, str(HOOK_DIR))

import planning_state  # noqa: E402

SESSION_MODES = {"workspace", "strict"}
DEFAULT_SESSION_MODE = "workspace"


def _session_policy_path(root: Path) -> Path:
    return root / ".planning" / "session-policy.json"


def _session_policy(root: Path) -> dict[str, Any]:
    policy = _session_policy_path(root)
    if not policy.is_file():
        return {}
    try:
        payload = json.loads(policy.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError):
        return {}
    return payload if isinstance(payload, dict) else {}


def _session_mode_from_policy_file(root: Path) -> str | None:
    payload = _session_policy(root)
    mode = payload.get("mode")
    if isinstance(mode, str):
        return mode.strip().lower()
    return None


def strict_requires_binding(root: Path) -> bool:
    env_value = os.environ.get("PWF_STRICT_REQUIRES_BINDING", "").strip().lower()
    if env_value in {"1", "true", "yes", "on"}:
        return True
    if env_value in {"0", "false", "no", "off"}:
        return False
    policy_value = _session_policy(root).get("require_binding")
    return policy_value is True


def session_mode(root: Path) -> str:
    env_mode = os.environ.get("PWF_SESSION_MODE", "").strip().lower()
    raw_mode = env_mode or _session_mode_from_policy_file(root) or DEFAULT_SESSION_MODE
    return raw_mode if raw_mode in SESSION_MODES else DEFAULT_SESSION_MODE


def load_payload() -> dict[str, Any]:
    raw = sys.stdin.read().strip()
    if not raw:
        return {}
    try:
        payload = json.loads(raw)
    except json.JSONDecodeError:
        return {}
    return payload if isinstance(payload, dict) else {}


def cwd_from_payload(payload: dict[str, Any]) -> Path:
    cwd = payload.get("cwd")
    if isinstance(cwd, str) and cwd:
        return Path(cwd)
    return Path.cwd()


def session_id_from_payload(payload: dict[str, Any]) -> str | None:
    sid = payload.get("session_id")
    if isinstance(sid, str) and sid:
        return sid
    for name in ("PWF_SESSION_ID", "CODEX_THREAD_ID"):
        env_sid = os.environ.get(name, "").strip()
        if env_sid:
            return env_sid
    return None


def is_session_attached(root: Path, session_id: str | None) -> bool:
    """Return True if this hook event should receive plan context."""
    if session_mode(root) != "strict":
        return True

    sessions_dir = root / ".planning" / "sessions"
    if not session_id:
        return False
    if not (sessions_dir / f"{session_id}.attached").exists():
        return False
    if strict_requires_binding(root):
        return planning_state.session_has_valid_binding(root, session_id)
    return True


def session_denial_message(root: Path, session_id: str | None) -> str:
    if session_mode(root) != "strict":
        return ""
    if not session_id:
        return (
            "[planning-with-files] session isolation is strict but hook payload has "
            "no session_id; planning context was not injected."
        )
    if strict_requires_binding(root):
        return (
            "[planning-with-files] session isolation is strict and requires a "
            "session plan binding; planning context was not injected."
        )
    return (
        "[planning-with-files] session isolation is strict and session_id is not "
        "attached; planning context was not injected."
    )


def emit_session_denial_if_needed(root: Path, session_id: str | None) -> bool:
    if is_session_attached(root, session_id):
        return False
    message = session_denial_message(root, session_id)
    if message:
        emit_json({"systemMessage": message})
    return True


def emit_json(payload: dict[str, Any]) -> None:
    if not payload:
        return
    json.dump(payload, sys.stdout, ensure_ascii=True)
    sys.stdout.write("\n")


def main_guard(func) -> int:
    try:
        func()
    except Exception as exc:  # pragma: no cover
        print(f"[planning-with-files hook] {exc}", file=sys.stderr)
        return 0
    return 0
