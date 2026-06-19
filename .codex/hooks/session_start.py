#!/usr/bin/env python3
from __future__ import annotations

import subprocess
import sys
from pathlib import Path

import codex_hook_adapter as adapter
import planning_state


def _run_session_catchup(root: Path, planning_dir: Path | None) -> str:
    hook_dir = Path(__file__).resolve().parent
    skill_dir = hook_dir.parent / "skills" / "planning-with-files"
    script = skill_dir / "scripts" / "session-catchup.py"
    if not script.is_file():
        return ""

    command = [sys.executable, str(script), str(root)]
    if planning_dir is not None:
        command.extend(["--planning-dir", str(planning_dir)])

    result = subprocess.run(
        command,
        text=True,
        capture_output=True,
        check=False,
    )
    return result.stdout.strip()


def main() -> None:
    payload = adapter.load_payload()
    root = adapter.cwd_from_payload(payload)

    session_id = adapter.session_id_from_payload(payload)
    if adapter.emit_session_denial_if_needed(root, session_id):
        return

    access = planning_state.resolve_planning_access(root, session_id=session_id)
    if not access.allowed:
        if access.warning:
            adapter.emit_json({"systemMessage": access.warning})
        return

    planning_dir = access.resolution.paths.root if access.resolution is not None else None
    catchup_context = _run_session_catchup(root, planning_dir)
    prompt_context = planning_state.render_prompt_context(root, session_id=session_id, event="SessionStart")
    parts = [catchup_context, prompt_context]
    output = "\n\n".join(part for part in parts if part)
    if output:
        payload = {
            "hookSpecificOutput": {
                "hookEventName": "SessionStart",
                "additionalContext": output,
            }
        }
        notice_source = prompt_context or output
        notice = planning_state.render_context_notice(
            output,
            root=root,
            session_id=session_id,
            event="SessionStart",
            status_context=notice_source,
        )
        if notice:
            payload["systemMessage"] = notice
        adapter.emit_json(payload)


if __name__ == "__main__":
    raise SystemExit(adapter.main_guard(main))
