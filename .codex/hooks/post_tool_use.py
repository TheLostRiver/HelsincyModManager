#!/usr/bin/env python3
from __future__ import annotations

import codex_hook_adapter as adapter
import planning_state


def main() -> None:
    payload = adapter.load_payload()
    root = adapter.cwd_from_payload(payload)

    if not adapter.is_session_attached(root, adapter.session_id_from_payload(payload)):
        return

    if planning_state.append_progress(root, payload):
        message = planning_state.message("post_tool_recorded")
        notice = planning_state.progress_compaction_notice(root)
        if notice:
            message = f"{message} {notice}"
        adapter.emit_json(
            {
                "systemMessage": message
            }
        )


if __name__ == "__main__":
    raise SystemExit(adapter.main_guard(main))
