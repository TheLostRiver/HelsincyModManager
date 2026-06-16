#!/usr/bin/env python3
from __future__ import annotations

import codex_hook_adapter as adapter
import planning_state


def main() -> None:
    payload = adapter.load_payload()
    root = adapter.cwd_from_payload(payload)

    session_id = adapter.session_id_from_payload(payload)
    if adapter.emit_session_denial_if_needed(root, session_id):
        return

    ownership_denial = planning_state.planning_access_denial(root, session_id)
    if ownership_denial:
        adapter.emit_json({"systemMessage": ownership_denial})
        return

    context = planning_state.render_pre_tool_context(root, session_id=session_id)
    if context:
        adapter.emit_json({"systemMessage": context})


if __name__ == "__main__":
    raise SystemExit(adapter.main_guard(main))
