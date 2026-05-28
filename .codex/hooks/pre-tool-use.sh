#!/bin/bash
# planning-with-files: Pre-tool-use hook for Codex

HOOK_DIR="$(cd "$(dirname "$0")" 2>/dev/null && pwd)"
PLAN_DIR="$(sh "${HOOK_DIR}/resolve-plan-dir.sh" 2>/dev/null)"
PLAN_FILE="${PLAN_DIR:+${PLAN_DIR}/}task_plan.md"

if [ -f "$PLAN_FILE" ]; then
    # Log plan context to stderr so the Codex adapter can surface it as systemMessage.
    head -30 "$PLAN_FILE" >&2
fi

echo '{"decision": "allow"}'
exit 0
