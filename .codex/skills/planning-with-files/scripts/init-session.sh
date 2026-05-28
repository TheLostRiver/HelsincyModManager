#!/bin/sh
set -u

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_NAME="${1:-project}"
if [ "$#" -gt 0 ]; then
    shift
fi

exec python "${SCRIPT_DIR}/plan.py" --root "${PWD}" init "${PROJECT_NAME}" "$@"
