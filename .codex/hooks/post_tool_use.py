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

    result = planning_state.append_progress(root, payload, session_id=session_id)
    if result.recorded or result.warning:
        parts = []
        if result.recorded:
            parts.append(planning_state.message("post_tool_recorded"))
            notice = planning_state.progress_compaction_notice(root, session_id=session_id)
            if notice:
                parts.append(notice)
        if result.warning:
            parts.append(result.warning)
        adapter.emit_json({"systemMessage": " ".join(parts)})


if __name__ == "__main__":
    raise SystemExit(adapter.main_guard(main))
