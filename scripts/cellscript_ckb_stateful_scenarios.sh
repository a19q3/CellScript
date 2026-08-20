#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

if [[ -n "${CELLSCRIPT_CKB_REPO:-}" ]]; then
    exec "$SCRIPT_DIR/ckb_cellscript_acceptance.sh" --production --stateful-scenarios \
        --ckb-repo "$CELLSCRIPT_CKB_REPO" "$@"
fi

exec "$SCRIPT_DIR/ckb_cellscript_acceptance.sh" --production --stateful-scenarios "$@"
