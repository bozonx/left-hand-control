#!/usr/bin/env bash
set -euo pipefail
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
workspace="$(cd "$root/../.." && pwd)"
bin="${LHC_BIN:-$workspace/target/debug/left-hand-control}"
popup="${1:-emoji}"
count="${2:-20}"
[[ "$popup" == emoji || "$popup" == quick ]] || exit 2
[[ "$count" =~ ^[1-9][0-9]*$ ]] || exit 2
for ((i=0; i<count; i++)); do
    "$bin" show "$popup"
    sleep "${LHC_DWELL:-0.5}"
    "$bin" hide
    sleep 0.2
done
