#!/usr/bin/env python3
from __future__ import annotations

import codex_hook_adapter as adapter
import planning_state


def main() -> None:
    payload = adapter.load_payload()
    root = adapter.cwd_from_payload(payload)

    if not adapter.is_session_attached(root, adapter.session_id_from_payload(payload)):
        return

    message = planning_state.stop_message(root)
    if not message:
        return

    if bool(payload.get("stop_hook_active")):
        adapter.emit_json({"systemMessage": message})
        return

    adapter.emit_json({"decision": "block", "reason": message})


if __name__ == "__main__":
    raise SystemExit(adapter.main_guard(main))
