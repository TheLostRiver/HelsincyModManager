#!/bin/sh
# planning-with-files: PreCompact compatibility wrapper.

set -u

SCRIPT_DIR="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
PYTHON_BIN="$(command -v python3 2>/dev/null || command -v python 2>/dev/null || true)"

[ -n "$PYTHON_BIN" ] || exit 0
"$PYTHON_BIN" "$SCRIPT_DIR/pre_compact.py"
exit 0
