#!/usr/bin/env bash
set -Eeuo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
DERIVE_REPO="${CKB_IDL_DERIVE_REPO:-$REPO_ROOT/../ckb-idl-derive}"
CLIENT_REPO="${CKB_IDL_CLIENT_REPO:-$REPO_ROOT/../ckb-idl-client}"
SCRIPTS_REPO="${CKB_IDL_SCRIPTS_REPO:-$REPO_ROOT/../ckb_sudt_script}"

DERIVE_COMMIT="e7ee35766b9084099e9d840ccd37d2b5d40074a1"
CLIENT_COMMIT="7d883e0abccba56d423449b673567ee817747936"
SCRIPTS_COMMIT="33bc56d84e8a181d855da5b82a87740825017f29"

usage() {
  cat <<'USAGE'
Usage: scripts/cellscript_ls_idl_upstream_acceptance.sh \
  [--derive-repo <path>] [--client-repo <path>] [--scripts-repo <path>]

Runs the opt-in LS-IDL compatibility check against clean, pinned upstream
checkouts. It validates upstream IDL bytes, runs upstream schema/wire tests,
and executes the actual ckb-idl-client Rust crate against CellScript Registry's
/idl/:code_hash compatibility handler.

This script is not part of any CellScript release gate.
USAGE
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --derive-repo)
      DERIVE_REPO="${2:?missing value for --derive-repo}"
      shift 2
      ;;
    --derive-repo=*)
      DERIVE_REPO="${1#*=}"
      shift
      ;;
    --client-repo)
      CLIENT_REPO="${2:?missing value for --client-repo}"
      shift 2
      ;;
    --client-repo=*)
      CLIENT_REPO="${1#*=}"
      shift
      ;;
    --scripts-repo)
      SCRIPTS_REPO="${2:?missing value for --scripts-repo}"
      shift 2
      ;;
    --scripts-repo=*)
      SCRIPTS_REPO="${1#*=}"
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "unknown argument: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

for command in cargo git node npm sha256sum; do
  if ! command -v "$command" >/dev/null 2>&1; then
    echo "missing required command: $command" >&2
    exit 127
  fi
done

node_major="$(node --version | sed -E 's/^v([0-9]+).*/\1/')"
if [[ "$node_major" != "22" ]]; then
  echo "LS-IDL upstream acceptance requires Node.js 22; found $(node --version)" >&2
  exit 1
fi

require_pinned_repo() {
  local label="$1" path="$2" expected_commit="$3" actual_commit tracked_changes
  if [[ ! -d "$path/.git" ]]; then
    echo "$label checkout is missing: $path" >&2
    exit 1
  fi
  actual_commit="$(git -C "$path" rev-parse HEAD)"
  if [[ "$actual_commit" != "$expected_commit" ]]; then
    echo "$label must be at $expected_commit; found $actual_commit" >&2
    exit 1
  fi
  tracked_changes="$(git -C "$path" status --short --untracked-files=no)"
  if [[ -n "$tracked_changes" ]]; then
    echo "$label checkout has tracked changes: $path" >&2
    echo "$tracked_changes" >&2
    exit 1
  fi
}

require_sha256() {
  local path="$1" expected="$2" actual
  if [[ ! -f "$path" ]]; then
    echo "pinned LS-IDL fixture is missing: $path" >&2
    exit 1
  fi
  actual="$(sha256sum "$path" | awk '{print $1}')"
  if [[ "$actual" != "$expected" ]]; then
    echo "raw-byte SHA-256 mismatch for $path: expected $expected, found $actual" >&2
    exit 1
  fi
}

require_pinned_repo "ckb-idl-derive" "$DERIVE_REPO" "$DERIVE_COMMIT"
require_pinned_repo "ckb-idl-client" "$CLIENT_REPO" "$CLIENT_COMMIT"
require_pinned_repo "ckb_sudt_script" "$SCRIPTS_REPO" "$SCRIPTS_COMMIT"

require_sha256 "$CLIENT_REPO/test-vectors.json" "a9a6dca4fd0c5fcd2ca7aea6468784be7fdb29d6274049f07090cbab0ce9c1bb"
require_sha256 "$DERIVE_REPO/example-idls/multisig-2of2-nonce/idl.json" "587098bbe12e37a7394d06ff711a59242f033759e9ba7f5b62b8f6a234275063"
require_sha256 "$DERIVE_REPO/example-idls/pow-lock/idl.json" "d551803734459f28b2849f13b2111778d3753b518701a86a434e9438df86e2d6"
require_sha256 "$DERIVE_REPO/example-idls/schnorr-pubkey-recovery/idl.json" "b37329b5fb13b25de94ef068724839f356096bc3516dda461b516ee983a8d371"
require_sha256 "$DERIVE_REPO/example-idls/secp256k1-timelock/idl.json" "056bc4f2b11bc7f0dfead9f2dcc0ec5097b42b353d4577b3836ef872b121710f"
require_sha256 "$DERIVE_REPO/example-idls/simple-lock/idl.json" "d28abead992546908eb483c24667e58302f193c00e08f6cbed1a6302995ca1c0"
require_sha256 "$SCRIPTS_REPO/contracts/simple-lock/idl.json" "6fd2ab0171167c6862582c4e95a6de7b1cd153f77a936af7e52be6599ddddd31"
require_sha256 "$SCRIPTS_REPO/contracts/timelock-lock/idl.json" "18ae57828b5fbd0c8df0900eed1153e7585587d4049900c50729616227a9beda"

cargo test --locked --manifest-path "$DERIVE_REPO/Cargo.toml"
cargo test --locked --manifest-path "$CLIENT_REPO/Cargo.toml" --lib
cargo test --locked --manifest-path "$SCRIPTS_REPO/Cargo.toml" -p tests witness_validation
cargo test --locked --manifest-path "$SCRIPTS_REPO/Cargo.toml" -p tests test_idl_has_three_fields

idl_files=(
  "$DERIVE_REPO/example-idls/multisig-2of2-nonce/idl.json"
  "$DERIVE_REPO/example-idls/pow-lock/idl.json"
  "$DERIVE_REPO/example-idls/schnorr-pubkey-recovery/idl.json"
  "$DERIVE_REPO/example-idls/secp256k1-timelock/idl.json"
  "$DERIVE_REPO/example-idls/simple-lock/idl.json"
  "$SCRIPTS_REPO/contracts/simple-lock/idl.json"
  "$SCRIPTS_REPO/contracts/timelock-lock/idl.json"
)
for idl_file in "${idl_files[@]}"; do
  cargo run --quiet --locked --manifest-path "$REPO_ROOT/Cargo.toml" -p cellscript --bin cellc -- \
    artifact ls-idl validate --idl "$idl_file"
done

cargo test --locked --manifest-path "$REPO_ROOT/Cargo.toml" -p cellscript --test ls_idl_upstream
CELLSCRIPT_CKB_IDL_CLIENT_REPO="$CLIENT_REPO" \
CELLSCRIPT_LS_IDL_CARGO_TARGET_DIR="$REPO_ROOT/target/ls-idl-upstream-client" \
  npm --prefix "$REPO_ROOT/services/registry-api" test -- \
    test/registry-api.test.ts -t "interoperates with the pinned upstream Rust client"

echo "Pinned LS-IDL upstream compatibility acceptance passed."
