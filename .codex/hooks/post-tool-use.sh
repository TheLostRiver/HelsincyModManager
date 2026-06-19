#!/bin/bash
# planning-with-files: Post-tool-use hook for Codex

HOOK_DIR="$(cd "$(dirname "$0")" 2>/dev/null && pwd)"
PLAN_DIR="$(sh "${HOOK_DIR}/resolve-plan-dir.sh" 2>/dev/null)"
PLAN_FILE="${PLAN_DIR:+${PLAN_DIR}/}task_plan.md"

if [ -f "$PLAN_FILE" ]; then
    echo "[planning-with-files] Objective PostToolUse auto records are maintained by hooks. If a phase is now complete, update task_plan.md status; put interpretive notes in findings.md."
fi
exit 0
