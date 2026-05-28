#!/usr/bin/env python3
from __future__ import annotations

import subprocess
import sys
from pathlib import Path

import codex_hook_adapter as adapter
import planning_state


def _run_session_catchup(root: Path) -> str:
    hook_dir = Path(__file__).resolve().parent
    skill_dir = hook_dir.parent / "skills" / "planning-with-files"
    script = skill_dir / "scripts" / "session-catchup.py"
    if not script.is_file():
        return ""

    result = subprocess.run(
        [sys.executable, str(script), str(root)],
        text=True,
        capture_output=True,
        check=False,
    )
    return result.stdout.strip()


def main() -> None:
    payload = adapter.load_payload()
    root = adapter.cwd_from_payload(payload)

    if not adapter.is_session_attached(root, adapter.session_id_from_payload(payload)):
        return

    parts = [_run_session_catchup(root), planning_state.render_prompt_context(root)]
    output = "\n\n".join(part for part in parts if part)
    if output:
        adapter.emit_json(
            {
                "hookSpecificOutput": {
                    "hookEventName": "SessionStart",
                    "additionalContext": output,
                }
            }
        )


if __name__ == "__main__":
    raise SystemExit(adapter.main_guard(main))
