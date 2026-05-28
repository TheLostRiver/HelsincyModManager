#!/bin/sh
set -u

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

if [ "$#" -gt 0 ]; then
    exec python "${SCRIPT_DIR}/plan.py" --root "${PWD}" switch "$1"
fi

exec python "${SCRIPT_DIR}/plan.py" --root "${PWD}" switch
