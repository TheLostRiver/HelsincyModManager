#!/usr/bin/env python3
from __future__ import annotations

import codex_hook_adapter as adapter
import planning_state


def main() -> None:
    payload = adapter.load_payload()
    root = adapter.cwd_from_payload(payload)

    if not adapter.is_session_attached(root, adapter.session_id_from_payload(payload)):
        return

    context = planning_state.render_pre_tool_context(root)
    if context:
        adapter.emit_json({"systemMessage": context})


if __name__ == "__main__":
    raise SystemExit(adapter.main_guard(main))
