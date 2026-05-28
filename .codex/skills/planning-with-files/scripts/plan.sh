#!/bin/sh
set -u

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
exec python "${SCRIPT_DIR}/plan.py" "$@"
